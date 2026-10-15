import { z } from "zod";
import type { BookStructureKeyStop } from "./book-structure";
import type { TechnicalLearningDiscourseItem } from "./discourse-index";
import { checkTeachingBindings, LearningObjectRefZ, sameObjectRef, SourceBindingZ,
  type FormalObjects, type TeachingSource, type TeachingSourceBinding } from "./teaching-map";

const text = z.string().trim().min(1);
export const CognitiveMaterialZ = z.object({
  target_id: text, kind: z.enum(["definition", "comparison", "method", "causal", "reasoning_episode"]),
  object_refs: z.array(LearningObjectRefZ).min(1), purpose: text,
  steps: z.array(z.object({ id: text, content: text, source_bindings: z.array(SourceBindingZ).min(1) }).strict()).min(1),
  connections: z.array(z.object({ from: text, to: text, meaning: text, source_bindings: z.array(SourceBindingZ).min(1) }).strict()),
  conditions: z.array(text),
  gaps: z.array(z.object({ description: text, source_bindings: z.array(SourceBindingZ).min(1),
    searched_queries: z.array(text).min(1), inspected_lids: z.array(text).min(1) }).strict()),
  patterns: z.array(z.object({ name: text, organization: text, source_bindings: z.array(SourceBindingZ).min(1) }).strict()),
}).strict();
export type CognitiveMaterial = z.infer<typeof CognitiveMaterialZ>;
export const CognitiveBudgetZ = z.object({ searches: z.number().int().positive(), preview_chars: z.number().int().positive(),
  read_chars: z.number().int().positive(), context_chars: z.number().int().positive() }).strict();
export type CognitiveBudget = z.infer<typeof CognitiveBudgetZ>;
export interface CognitiveWork {
  version: "cognitive_work.v1";
  source_id: string; source_revision: string; target_id: string;
  status: "incomplete" | "complete";
  stop_reason?: "budget_exhausted";
  used: { searches: number; preview_chars: number; read_chars: number };
  searches: Array<{ query: string; offset: number; lids: string[]; summaries: string[]; ranges?: Array<{ start: number; end: number }>; exhausted: boolean }>;
  previews: Array<{ lid: string; text: string }>;
  readings: Array<{ lid: string; text: string; start?: number; end?: number }>;
  object_searches?: Array<{ query: string; offset: number; refs: Array<{ source_id: string; object_id: string }>; exhausted: boolean }>;
  inspected_object?: { source_id: string; object_id: string };
  notes?: string;
  material?: CognitiveMaterial;
}
export const CognitiveActionZ = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("search"), query: text, offset: z.number().int().nonnegative(), limit: z.number().int().min(1).max(12) }).strict(),
  z.object({ kind: z.literal("preview"), lid: text }).strict(),
  z.object({ kind: z.literal("read"), lid: text, start: z.number().int().nonnegative().optional(), end: z.number().int().positive().optional() }).strict(),
  z.object({ kind: z.literal("search_objects"), query: z.string(), offset: z.number().int().nonnegative() }).strict(),
  z.object({ kind: z.literal("inspect_object"), ref: LearningObjectRefZ }).strict(),
  z.object({ kind: z.literal("retain"), notes: z.string().max(2000) }).strict(),
  z.object({ kind: z.literal("finish"), material: CognitiveMaterialZ }).strict(),
]);
export function newCognitiveWork(source: TeachingSource, target: BookStructureKeyStop): CognitiveWork {
  return { version: "cognitive_work.v1", source_id: source.source_id, source_revision: source.source_revision,
    target_id: target.id, status: "incomplete", used: { searches: 0, preview_chars: 0, read_chars: 0 }, searches: [], previews: [], readings: [] };
}

/** One bounded operation per executor handoff, persisted by the existing build writer. */
export function advanceCognitiveWork(input: { work: CognitiveWork; action: unknown; source: TeachingSource;
  target: BookStructureKeyStop; objects: FormalObjects; discourse: TechnicalLearningDiscourseItem[]; budget: CognitiveBudget }): CognitiveWork {
  const { source, target, objects, budget } = input;
  const work = structuredClone(input.work);
  const action = CognitiveActionZ.parse(input.action);
  if (work.source_id !== source.source_id || work.source_revision !== source.source_revision || work.target_id !== target.id
    || objects.source_revision !== source.source_revision) throw new Error("cognitive work source is stale");
  if (work.status === "complete") throw new Error("cognitive work already completed");
  if (work.stop_reason && action.kind === "finish") throw new Error("budget interrupted work must resume reading before completion");
  delete work.stop_reason;
  if (action.kind !== "inspect_object") delete work.inspected_object;
  const exhausted = () => { work.stop_reason = "budget_exhausted"; return work; };
  if (action.kind === "retain") { work.notes = action.notes; return work; }
  if (action.kind === "inspect_object") {
    if (!objects.active_refs.some(r => sameObjectRef(r, action.ref))) throw new Error("cognitive object is not active");
    work.inspected_object = action.ref; return work;
  }
  if (action.kind === "search_objects") {
    if (work.used.searches >= budget.searches) return exhausted();
    const prior = (work.object_searches ?? []).filter(s => s.query === action.query);
    if (action.offset !== prior.reduce((n, s) => n + s.refs.length, 0) || prior.at(-1)?.exhausted) throw new Error("object search pagination must inspect consecutive result pages");
    const terms = action.query.toLocaleLowerCase().split(/\s+/u).filter(Boolean);
    const hits = activeObjects(objects).filter(o => !terms.length || terms.some(term => JSON.stringify(o).toLocaleLowerCase().includes(term)));
    (work.object_searches ??= []).push({ query: action.query, offset: action.offset, refs: hits.slice(action.offset, action.offset + 6).map(o => o.ref), exhausted: action.offset + 6 >= hits.length });
    work.used.searches++; return work;
  }
  if (action.kind === "search") {
    if (work.used.searches >= budget.searches) return exhausted();
    const prior = work.searches.filter(s => s.query === action.query);
    const expectedOffset = prior.reduce((n, s) => n + s.lids.length, 0);
    if (action.offset !== expectedOffset || prior.at(-1)?.exhausted) throw new Error("search pagination must inspect consecutive result pages");
    // Search all source passages and unlinked discourse; graph connectivity is never a filter.
    const terms = action.query.toLocaleLowerCase().split(/\s+/u);
    const hits = source.passages.filter(p => {
      const haystack = `${p.text}\n${input.discourse.find(d => d.lid === p.lid)?.local_summary ?? ""}`.toLocaleLowerCase();
      return terms.some(term => haystack.includes(term));
    });
    const selected = hits.slice(action.offset, action.offset + action.limit);
    const ranges = selected.map(p => {
      const found = terms.map(term => p.text.toLocaleLowerCase().indexOf(term)).filter(index => index >= 0);
      const start = Math.max(0, (found.length ? Math.min(...found) : 0) - 80);
      return { start, end: Math.min(p.text.length, start + 160) };
    });
    work.searches.push({ query: action.query, offset: action.offset, lids: selected.map(p => p.lid),
      summaries: selected.map((p, index) => p.text.slice(ranges[index].start, ranges[index].end)), ranges,
      exhausted: action.offset + action.limit >= hits.length });
    work.used.searches++;
    return work;
  }
  if (action.kind === "read" || action.kind === "preview") {
    const passage = source.passages.find(p => p.lid === action.lid);
    if (!passage) throw new Error("cognitive source passage unavailable");
    const start = action.kind === "read" ? action.start ?? 0 : 0;
    const end = action.kind === "read" ? action.end ?? Math.min(passage.text.length, start + 2000) : Math.min(passage.text.length, 400);
    if (start >= end || end > passage.text.length || (action.kind === "read" && end - start > 2000)) throw new Error("cognitive source range invalid or too large");
    if (action.kind === "read") {
      const prior = work.readings.findIndex(p => p.lid === action.lid && (p.start ?? 0) === start && (p.end ?? p.text.length) === end);
      if (prior >= 0) { work.readings.push(...work.readings.splice(prior, 1)); return work; }
    } else if (work.previews.some(p => p.lid === action.lid)) return work;
    const value = passage.text.slice(start, end);
    const counter = action.kind === "read" ? "read_chars" : "preview_chars";
    if (work.used[counter] + value.length > budget[counter]
      || (action.kind === "read" && value.length > budget.context_chars)) return exhausted();
    if (action.kind === "read") work.readings.push({ lid: passage.lid, text: value, start, end });
    else work.previews.push({ lid: passage.lid, text: value });
    work.used[counter] += value.length;
    return work;
  }
  const material = action.material;
  if (material.target_id !== target.id) throw new Error("material covers another target");
  if (!work.readings.some(p => p.lid === target.lid)) throw new Error("target source was not read");
  for (const ref of material.object_refs) if (!objects.active_refs.some(r => sameObjectRef(r, ref))) throw new Error("material object is not active");
  const requireRead = (bindings: TeachingSourceBinding[]) => {
    checkTeachingBindings(bindings, source);
    if (bindings.some(b => !cognitiveRangeWasRead(work, b.lid, b.range_utf16 ?? { start: 0, end: source.passages.find(p => p.lid === b.lid)!.text.length }))) throw new Error("material cites unread source range");
  };
  const stepIds = new Set(material.steps.map(s => s.id));
  if (stepIds.size !== material.steps.length) throw new Error("duplicate cognitive step");
  material.steps.forEach(s => requireRead(s.source_bindings));
  for (const connection of material.connections) {
    if (!stepIds.has(connection.from) || !stepIds.has(connection.to)) throw new Error("unresolved cognitive step");
    requireRead(connection.source_bindings);
  }
  for (const gap of material.gaps) {
    requireRead(gap.source_bindings);
    if (gap.searched_queries.some(q => !work.searches.some(s => s.query === q && s.exhausted))
      || gap.inspected_lids.some(lid => !work.readings.some(p => p.lid === lid))) throw new Error("source gap lacks completed search and reading evidence");
  }
  material.patterns.forEach(p => requireRead(p.source_bindings));
  work.material = material;
  work.status = "complete";
  return work;
}

export function cognitiveRangeWasRead(work: CognitiveWork, lid: string, range: { start: number; end: number }): boolean {
  let cursor = range.start;
  for (const reading of work.readings.filter(r => r.lid === lid).sort((a, b) => (a.start ?? 0) - (b.start ?? 0))) {
    const start = reading.start ?? 0, end = reading.end ?? reading.text.length;
    if (start > cursor) break;
    cursor = Math.max(cursor, end);
    if (cursor >= range.end) return true;
  }
  return false;
}
const activeObjects = (objects: FormalObjects) => objects.active_refs.map(ref => objects.objects.filter(o => sameObjectRef(o.ref, ref)).at(-1)!);
/** Reading/search history is retained on disk; the current packet has bounded excerpts and paged object recall. */
export function cognitiveTaskInput(source: TeachingSource, target: BookStructureKeyStop, objects: FormalObjects, work: CognitiveWork, budget: CognitiveBudget,
  fits: (packet: unknown) => boolean = () => true) {
  const current = activeObjects(objects);
  const page = work.object_searches?.at(-1);
  const selected = page ? current.filter(o => page.refs.some(r => sameObjectRef(r, o.ref)))
    : current.filter(o => o.source_bindings.some(b => b.lid === target.lid)).slice(0, 6);
  const cursors = [...new Set(work.searches.map(s => s.query))].map(query => {
    const pages = work.searches.filter(s => s.query === query);
    return { query, next_offset: pages.reduce((n, s) => n + s.lids.length, 0), exhausted: pages.at(-1)!.exhausted };
  });
  let remaining = work.inspected_object ? 0 : Math.min(budget.context_chars, 4000);
  const readings: CognitiveWork["readings"] = [];
  for (const reading of [...work.readings].reverse()) {
    if (reading.text.length <= remaining) { readings.unshift(reading); remaining -= reading.text.length; }
  }
  const passage = source.passages.find(p => p.lid === target.lid);
  const packet = { source_id: source.source_id, source_revision: source.source_revision, target, budget,
    objects: selected.map(o => ({ ref: o.ref, meaning: o.meaning.slice(0, 400), kind: o.kind, conditions: o.conditions.join("; ").slice(0, 200),
      source_lids: o.source_bindings.slice(0, 6).map(b => b.lid) })),
    object_search: page, inspected_object: current.find(o => work.inspected_object && sameObjectRef(o.ref, work.inspected_object)),
    target_preview: passage?.text.slice(0, 400), target_length: passage?.text.length,
    work: { status: work.status, used: work.used, notes: work.notes, searches: work.searches.slice(-1), search_cursors: cursors,
      previews: work.previews.slice(-1), readings, read_ranges: work.readings.map(r => ({ lid: r.lid, start: r.start ?? 0, end: r.end ?? r.text.length })) } };
  // Keep the newest requested original/record and grounded notes; optional recall
  // summaries and older excerpts can be retrieved again without losing history.
  while (!fits(packet) && packet.work.readings.length > 1) packet.work.readings.shift();
  while (!fits(packet) && packet.objects.length) packet.objects.pop();
  if (!fits(packet)) packet.work.previews = [];
  if (!fits(packet)) packet.target_preview = undefined;
  return packet;
}

export interface CognitiveMaterials {
  version: "cognitive_materials.v1"; source_id: string; source_revision: string;
  formal_objects_revision: number; materials: CognitiveMaterial[];
  coverage: Array<{ target_id: string; status: "complete"; source_lid: string }>;
}
export function collectCognitiveMaterials(source: TeachingSource, targets: BookStructureKeyStop[], objects: FormalObjects, works: CognitiveWork[]): CognitiveMaterials {
  if (targets.some(t => works.filter(w => w.target_id === t.id && w.status === "complete" && w.material
    && w.source_id === source.source_id && w.source_revision === source.source_revision).length !== 1)) throw new Error("cognitive coverage incomplete");
  return { version: "cognitive_materials.v1", source_id: source.source_id, source_revision: source.source_revision,
    formal_objects_revision: objects.revision, materials: targets.map(t => works.find(w => w.target_id === t.id)!.material!),
    coverage: targets.map(t => ({ target_id: t.id, status: "complete", source_lid: t.lid })) };
}
