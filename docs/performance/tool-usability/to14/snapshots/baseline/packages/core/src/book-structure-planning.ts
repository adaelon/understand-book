import { z } from "zod";
import type { AnchoredText } from "./book-structure";
import { acceptStructureChapterSelection, structureAnchoredTextSchema,
  type AcceptedStructureChapterSelection, type StructureCandidateCatalog } from "./book-structure-candidates";

export interface StructureSourceExcerpt { unit_lid: string; lids: string[]; text: string }
export interface StructureOutlineInput {
  version: "book_structure_outline_input.v1";
  chapters: Array<{ unit_lid: string; title: string; overview: AnchoredText; sections?: Array<{ lid: string; title: string }> }>;
  preface: StructureSourceExcerpt[];
}
const outlineSchema = z.object({
  chapters: z.array(z.object({ unit_lid: z.string(), question: structureAnchoredTextSchema, progression: structureAnchoredTextSchema }).strict()),
  themes: z.array(z.object({ question: structureAnchoredTextSchema, unit_lids: z.array(z.string()).min(1) }).strict()),
}).strict();
export type StructureOutline = z.infer<typeof outlineSchema>;

export function acceptStructureOutline(value: unknown, input: StructureOutlineInput): StructureOutline {
  const outline = outlineSchema.parse(value);
  const order = input.chapters.map(c => c.unit_lid);
  if (outline.chapters.length !== order.length || outline.chapters.some((c, i) => c.unit_lid !== order[i])) {
    throw new Error("outline must preserve the canonical chapter order and identities");
  }
  const evidence = new Map(input.chapters.map(c => [c.unit_lid, new Set(c.overview.evidence_lids)]));
  for (const excerpt of input.preface) if (excerpt.text.trim()) {
    const set = evidence.get(excerpt.unit_lid) ?? new Set<string>();
    excerpt.lids.forEach(lid => set.add(lid)); evidence.set(excerpt.unit_lid, set);
  }
  for (const chapter of outline.chapters) for (const text of [chapter.question, chapter.progression]) {
    if (text.evidence_lids.some(lid => !evidence.get(chapter.unit_lid)?.has(lid))) throw new Error("outline chapter cites undelivered evidence");
  }
  for (const theme of outline.themes) {
    if (theme.unit_lids.some(unit => !order.includes(unit))) throw new Error("outline theme references an unknown unit");
    const allowed = new Set(theme.unit_lids.flatMap(unit => [...evidence.get(unit)!]));
    if (theme.question.evidence_lids.some(lid => !allowed.has(lid))
      || theme.unit_lids.some(unit => !theme.question.evidence_lids.some(lid => evidence.get(unit)?.has(lid)))) {
      throw new Error("outline theme needs delivered evidence for each member");
    }
  }
  return outline;
}

export const STRUCTURE_CHAPTER_PAGE_SIZE = 24;
export const structureChapterRevisionRequestSchema = z.object({
  id: z.string().min(1).max(120).regex(/^[A-Za-z0-9_-]+$/),
  unit_lid: z.string().min(1), issue: z.string().min(1).max(4000),
  candidate_refs: z.array(z.string()).min(1).max(6),
}).strict();
export type StructureChapterRevisionRequest = z.infer<typeof structureChapterRevisionRequestSchema>;
export const structureChapterSelectionContinuationSchema = z.object({
  id: z.string().min(1).max(120).regex(/^[A-Za-z0-9_-]+$/), unit_lid: z.string().min(1),
  from_action_ordinal: z.number().int().nonnegative().max(127), issue: z.string().min(1).max(4000),
}).strict();
export type StructureChapterSelectionContinuation = z.infer<typeof structureChapterSelectionContinuationSchema>;
export const STRUCTURE_CHAPTER_SELECTION_PART_SIZE = 48;
export interface StructureChapterWork {
  version: "book_structure_chapter_work.v1";
  unit_lid: string;
  offset: number;
  seen_refs: string[];
  evidence_lids: string[];
  notes: string;
  inspecting: string[];
  reading: number[];
  source_offset?: number;
  result?: AcceptedStructureChapterSelection;
  revision?: StructureChapterRevisionRequest & {
    previous_selection: AcceptedStructureChapterSelection["selection"];
    previously_inspected_refs: string[];
  };
  selection_draft?: { continuation: StructureChapterSelectionContinuation; accepted_stop_refs: string[]; show_index?: boolean };
}
export function newStructureChapterWork(unitLid: string): StructureChapterWork {
  return { version: "book_structure_chapter_work.v1", unit_lid: unitLid, offset: 0,
    seen_refs: [], evidence_lids: [], notes: "", inspecting: [], reading: [] };
}
/** Reopen only the selection; retain the completed chapter's delivery and evidence ledgers. */
export function reviseStructureChapterWork(work: StructureChapterWork, request: StructureChapterRevisionRequest,
  inspectedRefs: string[] = []): StructureChapterWork {
  const revision = structureChapterRevisionRequestSchema.parse(request);
  if (!work.result || revision.unit_lid !== work.unit_lid) throw new Error("chapter revision requires an accepted selection for its unit");
  if (revision.candidate_refs.some(ref => !work.seen_refs.includes(ref))) throw new Error("chapter revision locator must be a seen chapter candidate");
  const next = structuredClone(work);
  next.revision = { ...revision, previous_selection: work.result.selection,
    previously_inspected_refs: [...new Set(inspectedRefs)] };
  delete next.result;
  next.notes = ""; next.inspecting = []; next.reading = [];
  return next;
}
export function continueStructureChapterSelection(work: StructureChapterWork,
  value: StructureChapterSelectionContinuation): StructureChapterWork {
  const continuation = structureChapterSelectionContinuationSchema.parse(value);
  if (work.result || work.unit_lid !== continuation.unit_lid) throw new Error("chapter selection continuation requires its unfinished chapter");
  const next = structuredClone(work);
  next.selection_draft = { continuation, accepted_stop_refs: [] };
  next.notes = "";
  // Last inspect/read was requested by the preceding action. Its payload must still be delivered
  // before the first selection part can add that source evidence to the ledger.
  return next;
}
export interface StructureChapterContext {
  catalog: StructureCandidateCatalog;
  outline: StructureOutline;
  titles: Record<string, string>;
  excerpts: StructureSourceExcerpt[];
  source_index_page_size?: number;
  candidate_index_page_size?: number;
}

/** The compact index offers complete browsing; only inspect/read payloads grant evidence. */
export function structureChapterInput(work: StructureChapterWork, context: StructureChapterContext) {
  const candidates = context.catalog.candidates.filter(c => c.unit_lid === work.unit_lid);
  const page = candidates.slice(work.offset, work.offset + (context.candidate_index_page_size ?? STRUCTURE_CHAPTER_PAGE_SIZE));
  const selectionSeen = work.selection_draft ? new Set(work.seen_refs) : undefined;
  const expanded = Boolean(context.candidate_index_page_size && (work.revision && !work.selection_draft?.show_index
    || selectionSeen && !work.selection_draft!.show_index && candidates.every(c => selectionSeen.has(c.ref)) || work.inspecting.length || work.reading.length));
  const inspected = work.inspecting.map(ref => {
    const candidate = candidates.find(c => c.ref === ref);
    if (!candidate) throw new Error(`unknown inspected candidate: ${ref}`);
    if (!context.source_index_page_size) return candidate;
    const source_indices = context.excerpts.flatMap((e, index) => e.unit_lid === work.unit_lid
      && e.lids.some(lid => candidate.evidence_lids.includes(lid)) ? [index] : []);
    return { ...candidate, source_indices };
  });
  const excerpts = work.reading.map(index => {
    const excerpt = context.excerpts[index];
    if (!excerpt || excerpt.unit_lid !== work.unit_lid || !excerpt.text.trim()) throw new Error("unknown chapter excerpt");
    return { index, ...excerpt };
  });
  const sourceIndex = context.excerpts.flatMap((e, index) => e.unit_lid === work.unit_lid ? [{ index, lids: e.lids }] : []);
  const sourceOffset = work.source_offset ?? 0;
  const sourcePage = context.source_index_page_size ? sourceIndex.slice(sourceOffset, sourceOffset + context.source_index_page_size) : sourceIndex;
  return {
    version: "book_structure_chapter_input.v1" as const, unit_lid: work.unit_lid, title: context.titles[work.unit_lid] ?? "未命名单元",
    outline: context.outline.chapters.map(c => ({ ...c, title: context.titles[c.unit_lid] ?? "未命名单元" })),
    themes: context.outline.themes, notes: work.notes,
    ...(work.revision ? { revision: work.revision } : {}),
    ...(work.selection_draft ? { selection_submission: { version: "book_structure_chapter_selection_parts.v1" as const,
      continuation: work.selection_draft.continuation, max_refs_per_part: STRUCTURE_CHAPTER_SELECTION_PART_SIZE,
      accepted_stop_refs: work.selection_draft.accepted_stop_refs } } : {}),
    index: { offset: work.offset, total: candidates.length,
      next_offset: work.offset + page.length < candidates.length ? work.offset + page.length : null,
      items: (expanded ? [] : page).map(c => ({ ref: c.ref, section_lid: c.section_lid, lid: c.lid, type: c.type,
        meaning: c.meaning.slice(0, 240), conditions: c.conditions.join("; ").slice(0, 240) })),
      ...(expanded ? { preview_already_delivered: true } : {}) },
    seen_refs: work.seen_refs,
    source_index: sourcePage,
    ...(context.source_index_page_size ? { source_page: { offset: sourceOffset, total: sourceIndex.length,
      next_offset: sourceOffset + sourcePage.length < sourceIndex.length ? sourceOffset + sourcePage.length : null } } : {}),
    inspected, excerpts, previously_delivered_evidence_lids: work.evidence_lids,
  };
}
const actionSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("browse"), offset: z.number().int().nonnegative(), notes: z.string().max(4000).optional() }).strict(),
  z.object({ kind: z.literal("source_browse"), offset: z.number().int().nonnegative(), notes: z.string().max(4000).optional() }).strict(),
  z.object({ kind: z.literal("inspect"), refs: z.array(z.string()).min(1).max(6), notes: z.string().max(4000).optional() }).strict(),
  z.object({ kind: z.literal("read"), indices: z.array(z.number().int().nonnegative()).min(1).max(3), notes: z.string().max(4000).optional() }).strict(),
  z.object({ kind: z.literal("select_stops"), refs: z.array(z.string()).min(1).max(STRUCTURE_CHAPTER_SELECTION_PART_SIZE) }).strict(),
  z.object({ kind: z.literal("select"), selection: z.unknown() }).strict(),
]);

/** Called only after structureChapterInput was delivered. It records that delivery before applying the action. */
export function applyStructureChapterAction(work: StructureChapterWork, value: unknown, context: StructureChapterContext): StructureChapterWork {
  if (work.result) throw new Error("chapter selection already accepted");
  const action = actionSchema.parse(value);
  const delivered = structureChapterInput(work, context);
  const next = structuredClone(work);
  if (next.selection_draft) next.selection_draft.show_index = action.kind === "browse";
  next.seen_refs = [...new Set([...next.seen_refs, ...delivered.index.items.map(c => c.ref)])];
  next.evidence_lids = [...new Set([...next.evidence_lids,
    ...delivered.inspected.flatMap(c => c.evidence_lids), ...delivered.excerpts.flatMap(e => e.lids)])];
  if (next.revision) next.revision.previously_inspected_refs = [...new Set([
    ...next.revision.previously_inspected_refs, ...delivered.inspected.map(c => c.ref),
  ])];
  if ("notes" in action && action.notes !== undefined) next.notes = action.notes;
  if (action.kind === "browse") {
    if (action.offset >= delivered.index.total && action.offset !== 0) throw new Error("chapter index offset outside catalog");
    next.offset = action.offset; next.inspecting = []; next.reading = [];
  } else if (action.kind === "source_browse") {
    if (!context.source_index_page_size || action.offset >= delivered.source_page!.total && action.offset !== 0)
      throw new Error("chapter source index offset outside catalog");
    next.source_offset = action.offset; next.inspecting = []; next.reading = [];
  } else if (action.kind === "inspect") {
    if (action.refs.some(ref => !next.seen_refs.includes(ref))) throw new Error("inspect requires a browsed chapter candidate");
    next.inspecting = [...new Set(action.refs)]; next.reading = [];
  } else if (action.kind === "read") {
    if (action.indices.some(index => context.excerpts[index]?.unit_lid !== work.unit_lid)) throw new Error("read outside chapter source");
    next.reading = [...new Set(action.indices)]; next.inspecting = [];
  } else if (action.kind === "select_stops") {
    if (!next.selection_draft) throw new Error("select_stops requires the bounded chapter selection contract");
    if (action.refs.some(ref => !next.seen_refs.includes(ref)
      || !context.catalog.candidates.some(c => c.ref === ref && c.unit_lid === work.unit_lid)))
      throw new Error("selection part requires seen candidates from this chapter");
    if (new Set(action.refs).size !== action.refs.length || action.refs.some(ref => next.selection_draft!.accepted_stop_refs.includes(ref)))
      throw new Error("duplicate chapter selection part reference");
    next.selection_draft.accepted_stop_refs.push(...action.refs);
    next.inspecting = []; next.reading = [];
  } else {
    const all = context.catalog.candidates.filter(c => c.unit_lid === work.unit_lid);
    if (all.some(c => !next.seen_refs.includes(c.ref))) throw new Error("browse the complete chapter index before selecting");
    const selection = next.selection_draft ? { ...z.object({ unit_lid: z.string().min(1),
      role: z.enum(["setup", "foundation", "method", "application", "case", "synthesis"]), summary: structureAnchoredTextSchema,
      macro_stop_refs: z.array(z.string()) }).strict().parse(action.selection), accepted_stop_refs: next.selection_draft.accepted_stop_refs } : action.selection;
    next.result = acceptStructureChapterSelection(selection, context.catalog, work.unit_lid,
      { unit_lids: [work.unit_lid], dependency_target_lids: [], evidence_by_unit: { [work.unit_lid]: next.evidence_lids } });
  }
  return next;
}
