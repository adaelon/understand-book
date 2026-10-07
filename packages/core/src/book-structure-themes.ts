import { z } from "zod";
import type { BookStructureCandidate } from "./book-structure";
import type { StructureCandidateCatalog } from "./book-structure-candidates";
import type { StructureSourceExcerpt } from "./book-structure-planning";
import { projectStructureRetrieval, structureRetrievalPage, STRUCTURE_RETRIEVAL_POLICY, type StructureRetrievalIndex, type StructureRetrievalChapter } from "./book-structure-retrieval";
import { retrieveHybrid } from "./semantic-retrieval";
import { retrievalDependencies, type PreparedRetrieval, type RetrievalMode } from "./semantic-retrieval-preparation";
import { STRUCTURE_RETRIEVAL } from "./book-structure-retrieval";
import { isDeepStrictEqual } from "node:util";

const text = z.string().min(1).max(600);
const refs = z.array(z.string().min(1));
const anchored = z.object({ text, evidence_lids: refs.min(1) }).strict();
const themePlan = z.object({ ref: z.string().min(1), question: text, distinction: text, unit_lids: refs.min(2), member_refs: refs }).strict();
export type StructureThemePlan = z.infer<typeof themePlan>;
const stage = z.object({ unit_lid: z.string(), development: anchored, member_refs: refs }).strict();
const resultSchema = z.object({ ref: z.string(), name: z.string().min(1).max(200), summary: anchored,
  stages: z.array(stage).min(2),
  dependencies: z.array(z.object({ unit_lid: z.string(), depends_on: z.string(),
    prerequisite: anchored, application: anchored, rationale: text }).strict()),
}).strict();
export type StructureThemeResult = z.infer<typeof resultSchema>;
export interface StructureThemeWork {
  plan: StructureThemePlan;
  query: string; offset: number; seen_refs: string[]; inspected_refs: string[];
  evidence_by_unit: Record<string, string[]>; inspecting: string[]; reading: number[]; notes: string;
  result?: StructureThemeResult;
  source_offset?: number;
}
export interface StructureThemeDirectory {
  version: "book_structure_themes.v1";
  works: StructureThemeWork[];
  reconciled: boolean;
  redirects: Record<string, string>;
  unresolved: string[];
  revision?: { ref: string; issue: string; previous_result: StructureThemeResult };
}
export interface StructureThemeContext {
  catalog: StructureCandidateCatalog;
  chapters: StructureRetrievalChapter[];
  excerpts: StructureSourceExcerpt[];
  mode: RetrievalMode;
  prepared?: PreparedRetrieval;
  planning_candidate_refs?: string[];
  source_index_page_size?: number;
}
export function structureThemePlanningInput(context: StructureThemeContext) {
  const seeds = context.planning_candidate_refs && new Set(context.planning_candidate_refs);
  if (seeds) return {
    chapters: context.chapters.map(c => ({ unit_lid: c.unit_lid, title: c.title, overview: c.overview.text,
      ...(c.question ? { question: c.question } : {}) })),
    candidate_index_scope: "chapter_macro_route_seeds", total_candidates: context.catalog.candidates.length,
    candidate_index: context.catalog.candidates.filter(c => seeds.has(c.ref)).map(c => ({
      ref: c.ref, unit_lid: c.unit_lid, meaning: c.meaning.slice(0, 32),
    })),
    preview_notice: "Chapter summaries and questions are complete. Candidate meanings are short navigation previews; inspect/search supplies full mechanisms, conditions and source evidence. Chapter citation ledgers remain in Core and are not repeated for planning.",
  };
  return { chapters: context.chapters,
    candidate_index: context.catalog.candidates.map(c => ({ ref: c.ref, unit_lid: c.unit_lid,
    meaning: c.meaning.slice(0, 100), conditions: c.conditions.join("; ").slice(0, 80) })) };
}
/** One work item per question, irrespective of the number of members. */
export function planStructureThemes(value: unknown, context: StructureThemeContext): StructureThemeDirectory {
  const { themes } = z.object({ themes: z.array(themePlan).max(16) }).strict().parse(value);
  const units = new Set(context.chapters.map(c => c.unit_lid)), candidates = new Map(context.catalog.candidates.map(c => [c.ref, c]));
  if (new Set(themes.map(t => t.ref)).size !== themes.length) throw new Error("duplicate theme reference");
  for (const t of themes) {
    if (new Set(t.unit_lids).size !== t.unit_lids.length || t.unit_lids.some(lid => !units.has(lid))) throw new Error("theme needs distinct known chapters");
    if (new Set(t.member_refs).size !== t.member_refs.length || t.member_refs.some(ref => !t.unit_lids.includes(candidates.get(ref)?.unit_lid ?? "")))
      throw new Error("theme member outside planned chapters");
  }
  return { version: "book_structure_themes.v1", works: themes.map(plan => ({ plan, query: "", offset: 0,
    seen_refs: [], inspected_refs: [], evidence_by_unit: {}, inspecting: [], reading: [], notes: "" })), reconciled: false, redirects: {}, unresolved: [] };
}
export function compactStructureThemes(directory: StructureThemeDirectory) {
  return directory.works.map(w => ({ ref: w.plan.ref, question: w.plan.question, distinction: w.plan.distinction,
    unit_lids: w.result?.stages.map(s => s.unit_lid) ?? w.plan.unit_lids, name: w.result?.name, summary: w.result?.summary,
    status: w.result ? "resolved" : "pending" }));
}
function retrieval(work: StructureThemeWork, context: StructureThemeContext, index: StructureRetrievalIndex) {
  const request = { kind: "search" as const, query: work.query };
  const prepared = context.mode === "lexical_only" ? {
    version: "prepared_formal_object_retrieval.v1" as const,
    dependencies: retrievalDependencies(index.records, request, "lexical_only", null, STRUCTURE_RETRIEVAL_POLICY, STRUCTURE_RETRIEVAL.projection_version),
    sequence: retrieveHybrid(index.records, request, [], STRUCTURE_RETRIEVAL_POLICY), diagnostics: { semantic_scores: [] },
  } : context.prepared;
  if (!prepared || prepared.dependencies.mode !== context.mode) throw new Error("theme retrieval requires preparation");
  return structureRetrievalPage(index, prepared, request, work.offset);
}
export function structureThemeInput(directory: StructureThemeDirectory, ref: string, context: StructureThemeContext) {
  const work = directory.works.find(w => w.plan.ref === ref);
  if (!work) throw new Error("unknown theme work");
  const index = projectStructureRetrieval(context.catalog, context.chapters);
  const sourceLocations = new Map(context.excerpts.flatMap((e, i) => e.lids.map(lid => [lid, i] as const)));
  const inspected = work.inspecting.map(key => {
    const location = index.locations[key];
    if (!location) throw new Error("unknown theme material");
    const material = location.kind === "candidate" ? context.catalog.candidates.find(c => c.ref === key)!
      : context.chapters.find(c => c.unit_lid === location.unit_lid)!;
    return { key, ...location, material,
      ...(context.source_index_page_size ? { source_indices: [...new Set(location.evidence_lids.flatMap(lid => {
        const index = sourceLocations.get(lid); return index === undefined ? [] : [index];
      }))] } : {}) };
  });
  const excerpts = work.reading.map(i => {
    const e = context.excerpts[i]; if (!e?.text.trim()) throw new Error("unknown theme source excerpt");
    return { index: i, ...e };
  });
  const sourceIndex = context.excerpts.map((e, i) => ({ index: i, unit_lid: e.unit_lid, lids: e.lids }));
  const offset = work.source_offset ?? 0, size = context.source_index_page_size;
  return { version: "book_structure_theme_input.v1", theme: work.plan, directory: compactStructureThemes(directory),
    ...(directory.revision?.ref === ref ? { revision: { ...directory.revision,
      constraint: "Resolve this concrete source-review issue while preserving theme identity, name and chapter membership. Other themes are already accepted; no second directory scan is needed." } } : {}),
    chapters: context.chapters.map(c => ({ unit_lid: c.unit_lid, title: c.title })), notes: work.notes,
    search: { query: work.query, ...retrieval(work, context, index) },
    source_index: size ? sourceIndex.slice(offset, offset + size) : sourceIndex,
    ...(size ? { source_page: { offset, total: sourceIndex.length, next_offset: offset + size < sourceIndex.length ? offset + size : null } } : {}),
    inspected, excerpts, previously_delivered_evidence: work.evidence_by_unit, inspected_refs: work.inspected_refs };
}
function checkResult(value: unknown, ref: string, context: StructureThemeContext, evidence: Record<string, string[]>, inspected: string[]): StructureThemeResult {
  const result = resultSchema.parse(value);
  if (result.ref !== ref) throw new Error("theme identity changed");
  const units = new Set(context.chapters.map(c => c.unit_lid)), candidates = new Map(context.catalog.candidates.map(c => [c.ref, c]));
  const checkEvidence = (lids: string[], unit: string) => {
    if (lids.some(lid => !evidence[unit]?.includes(lid))) throw new Error("theme cites undelivered or foreign evidence");
  };
  const members = result.stages.map(s => s.unit_lid);
  if (new Set(members).size < 2 || members.some(u => !units.has(u))) throw new Error("theme needs at least two known chapters");
  // Repeated units allow distinct stages in the same chapter, but not duplicated candidate membership.
  const selected = result.stages.flatMap(s => s.member_refs);
  if (new Set(selected).size !== selected.length) throw new Error("duplicate theme member");
  for (const s of result.stages) {
    checkEvidence(s.development.evidence_lids, s.unit_lid);
    if (s.member_refs.some(r => candidates.get(r)?.unit_lid !== s.unit_lid || !inspected.includes(r))) throw new Error("theme members must be inspected in their chapter");
  }
  const allowed = new Set(members.flatMap(u => evidence[u] ?? []));
  if (result.summary.evidence_lids.some(lid => !allowed.has(lid)) || members.some(u => !result.summary.evidence_lids.some(lid => evidence[u]?.includes(lid))))
    throw new Error("theme summary must cite every chapter with delivered evidence");
  for (const d of result.dependencies) {
    if (d.unit_lid === d.depends_on || !members.includes(d.unit_lid) || !members.includes(d.depends_on)) throw new Error("dependency requires two theme chapters");
    checkEvidence(d.prerequisite.evidence_lids, d.depends_on); checkEvidence(d.application.evidence_lids, d.unit_lid);
  }
  return result;
}
const actionSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("source_browse"), offset: z.number().int().nonnegative(), notes: z.string().max(4000).optional() }).strict(),
  z.object({ kind: z.literal("search"), query: z.string().max(600), offset: z.number().int().nonnegative().default(0), notes: z.string().max(4000).optional() }).strict(),
  z.object({ kind: z.literal("inspect"), refs: refs.min(1).max(6), notes: z.string().max(4000).optional() }).strict(),
  z.object({ kind: z.literal("read"), indices: z.array(z.number().int().nonnegative()).min(1).max(3), notes: z.string().max(4000).optional() }).strict(),
  z.object({ kind: z.literal("resolve"), result: resultSchema }).strict(),
]);
export function applyStructureThemeAction(directory: StructureThemeDirectory, ref: string, value: unknown, context: StructureThemeContext): StructureThemeDirectory {
  const work = directory.works.find(w => w.plan.ref === ref);
  if (!work || work.result || directory.reconciled) throw new Error("theme work unavailable or already resolved");
  const action = actionSchema.parse(value), input = structureThemeInput(directory, ref, context), next = structuredClone(directory);
  const w = next.works.find(w => w.plan.ref === ref)!;
  w.seen_refs = [...new Set([...w.seen_refs, ...input.search.items.map(c => c.key), ...work.plan.member_refs])];
  const deliver = (unit: string, lids: string[]) => w.evidence_by_unit[unit] = [...new Set([...(w.evidence_by_unit[unit] ?? []), ...lids])];
  for (const c of input.inspected) deliver(c.unit_lid, c.evidence_lids);
  for (const e of input.excerpts) deliver(e.unit_lid, e.lids);
  w.inspected_refs = [...new Set([...w.inspected_refs, ...input.inspected.map(c => c.key)])];
  if ("notes" in action && action.notes !== undefined) w.notes = action.notes;
  w.inspecting = []; w.reading = [];
  if (action.kind === "source_browse") {
    if (!context.source_index_page_size || (action.offset >= context.excerpts.length && action.offset !== 0)) throw new Error("theme source index offset out of range");
    w.source_offset = action.offset;
  } else if (action.kind === "search") {
    if (action.query !== work.query && action.offset !== 0) throw new Error("new theme query must start at offset zero");
    if (action.query === work.query && action.offset > input.search.candidate_count) throw new Error("theme search offset out of range");
    w.query = action.query; w.offset = action.offset;
  }
  else if (action.kind === "inspect") {
    if (action.refs.some(r => !w.seen_refs.includes(r))) throw new Error("inspect requires a seen theme material");
    w.inspecting = [...new Set(action.refs)];
  } else if (action.kind === "read") {
    if (action.indices.some(i => !context.excerpts[i]?.text.trim())) throw new Error("unknown theme source excerpt");
    w.reading = [...new Set(action.indices)];
  } else {
    w.result = checkResult(action.result, ref, context, w.evidence_by_unit, w.inspected_refs);
    if (next.revision) {
      const prior = next.revision.previous_result;
      if (w.result.name !== prior.name || !isDeepStrictEqual([...new Set(w.result.stages.map(s => s.unit_lid))].sort(),
        [...new Set(prior.stages.map(s => s.unit_lid))].sort())) throw new Error("targeted revision must preserve theme name and chapter membership");
      next.unresolved = next.unresolved.filter(issue => issue !== next.revision!.issue);
      next.reconciled = true;
      delete next.revision;
    }
  }
  return next;
}

/** Explicit source-review follow-up; only the named work is reopened, never an automatic rescan. */
export function reopenStructureTheme(directory: StructureThemeDirectory, ref: string, issue: string): StructureThemeDirectory {
  if (!directory.reconciled || directory.revision || !issue.trim()) throw new Error("targeted revision requires a reconciled directory and a concrete issue");
  const next = structuredClone(directory), work = next.works.find(w => w.plan.ref === ref);
  if (!work?.result) throw new Error("unknown accepted theme to revise");
  next.revision = { ref, issue, previous_result: work.result };
  delete work.result; next.reconciled = false;
  return next;
}

/** A single bounded directory pass returns edits/merges, never a rewritten book. */
export function reconcileStructureThemes(directory: StructureThemeDirectory, value: unknown, context: StructureThemeContext): StructureThemeDirectory {
  if (directory.works.some(w => !w.result)) throw new Error("resolve all planned themes before reconciliation");
  if (directory.reconciled) throw new Error("theme directory already reconciled");
  const delta = z.object({ edits: z.array(resultSchema).max(16), merges: z.array(z.object({ source_refs: refs.min(2), result: resultSchema }).strict()).max(16),
    unresolved: z.array(text).max(16) }).strict().parse(value);
  const next = structuredClone(directory), touched = new Set<string>();
  const claim = (ref: string) => {
    if (touched.has(ref)) throw new Error("overlapping theme directory changes");
    const w = directory.works.find(w => w.plan.ref === ref); if (!w) throw new Error("unknown reconciled theme"); touched.add(ref); return w;
  };
  const apply = (sources: StructureThemeWork[], result: StructureThemeResult) => {
    const evidence: Record<string, string[]> = {};
    for (const w of sources) for (const [unit, lids] of Object.entries(w.evidence_by_unit)) evidence[unit] = [...new Set([...(evidence[unit] ?? []), ...lids])];
    const inspected = [...new Set(sources.flatMap(w => w.inspected_refs))];
    const checked = checkResult(result, result.ref, context, evidence, inspected);
    const target = next.works.find(w => w.plan.ref === result.ref)!;
    target.result = checked; target.evidence_by_unit = evidence; target.inspected_refs = inspected;
  };
  for (const edit of delta.edits) apply([claim(edit.ref)], edit);
  for (const merge of delta.merges) {
    if (!merge.source_refs.includes(merge.result.ref)) throw new Error("merge must retain one source identity");
    const sources = merge.source_refs.map(claim);
    // A merge preserves accepted members and dependencies; membership edits are explicit edits.
    const resultMembers = new Set(merge.result.stages.flatMap(s => s.member_refs));
    if (sources.some(w => w.result!.stages.some(s => s.member_refs.some(r => !resultMembers.has(r))))) throw new Error("merge loses accepted theme members");
    if (sources.some(w => w.result!.dependencies.some(d => !merge.result.dependencies.some(n => isDeepStrictEqual(n, d))))) throw new Error("merge loses accepted dependency");
    apply(sources, merge.result);
    for (const old of merge.source_refs.filter(r => r !== merge.result.ref)) next.redirects[old] = merge.result.ref;
    next.works = next.works.filter(w => !merge.source_refs.includes(w.plan.ref) || w.plan.ref === merge.result.ref);
  }
  next.reconciled = true; next.unresolved = delta.unresolved;
  return next;
}

export function materializeStructureThemes(base: BookStructureCandidate, directory: StructureThemeDirectory, context: StructureThemeContext): BookStructureCandidate {
  if (!directory.reconciled || directory.unresolved.length || directory.works.some(w => !w.result)) throw new Error("theme directory incomplete");
  const candidate = structuredClone(base), stops = new Map((candidate.key_stops ?? []).map(s => [s.id, s]));
  const units = new Set(candidate.spine?.map(u => u.lid)), lines = new Map((candidate.throughlines ?? []).map(l => [l.id, l]));
  candidate.reference_scope ??= { unit_lids: [...units], dependency_target_lids: [...units], evidence_by_unit: {} };
  for (const w of directory.works) {
    const r = checkResult(w.result, w.plan.ref, context, w.evidence_by_unit, w.inspected_refs);
    const lids = [...new Set(r.stages.map(s => s.unit_lid))], selected = r.stages.flatMap(s => s.member_refs);
    if (lids.some(lid => !units.has(lid))) throw new Error("theme references a chapter missing from materialization");
    const addEvidence = (unit: string, evidence: string[]) => {
      const scope = candidate.reference_scope!.evidence_by_unit;
      scope[unit] = [...new Set([...(scope[unit] ?? []), ...evidence])].sort();
    };
    for (const ref of selected) {
      const c = context.catalog.candidates.find(c => c.ref === ref)!;
      const stop = { id: ref, lid: c.lid, type: c.type, ...(c.title ? { title: c.title } : {}),
        reason: { text: [...new Set([c.meaning, ...c.conditions, c.reason.text])].join("\n"), evidence_lids: c.evidence_lids } };
      if (stops.has(ref) && !isDeepStrictEqual(stops.get(ref), stop)) throw new Error("theme candidate conflicts with chapter content");
      stops.set(ref, stop); addEvidence(c.unit_lid, c.evidence_lids);
    }
    const line = { id: r.ref, name: r.name, summary: r.summary, lids, key_stop_ids: selected };
    if (lines.has(r.ref) && !isDeepStrictEqual(lines.get(r.ref), line)) throw new Error("conflicting materialized theme");
    lines.set(r.ref, line);
    for (const unit of lids) addEvidence(unit, r.summary.evidence_lids.filter(lid => w.evidence_by_unit[unit]?.includes(lid)));
    for (const d of r.dependencies) {
      const unit = candidate.spine!.find(u => u.lid === d.unit_lid)!;
      unit.depends_on = [...new Set([...unit.depends_on, d.depends_on])];
      addEvidence(d.unit_lid, d.application.evidence_lids); addEvidence(d.depends_on, d.prerequisite.evidence_lids);
    }
  }
  candidate.key_stops = [...stops.values()]; candidate.throughlines = [...lines.values()];
  return candidate;
}
