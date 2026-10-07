import { z } from "zod";
import { acceptFormalObjects, checkTeachingBindings, LearningObjectRefZ, ObjectProposalZ, SourceBindingZ,
  type FormalObjectProposal, type FormalObjects, type TeachingSource, type TeachingSourceBinding } from "./teaching-map";
import type { FormalObjectFragmentResult, TeachingSourceRange } from "./teaching-object-fragments";
import { projectRetrievalCatalog, retrievalCatalog, retrievalPage, type RetrievalRequest } from "./semantic-retrieval";
import { retrievalDependencies, retrievalPreparationMatches, type PreparedRetrieval } from "./semantic-retrieval-preparation";

type ObjectProposal = FormalObjectProposal["objects"][number];
const text = z.string().trim().min(1);
export const AlignmentActionZ = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("search"), query: z.string(), offset: z.number().int().nonnegative() }).strict(),
  z.object({ kind: z.literal("inspect"), key: text }).strict(),
  z.object({ kind: z.literal("read"), lid: text, start: z.number().int().nonnegative(), end: z.number().int().positive() }).strict(),
  z.object({ kind: z.literal("resolve"), keys: z.array(text).min(1), object: ObjectProposalZ }).strict(),
  z.object({ kind: z.literal("identity"), from: z.array(LearningObjectRefZ).min(1), to_keys: z.array(text).min(1),
    reason: text, source_bindings: z.array(SourceBindingZ).min(1) }).strict(),
  z.object({ kind: z.literal("finish") }).strict(),
]);
export interface ObjectAlignmentWork {
  version: "formal_object_alignment.v3";
  proposal: FormalObjectProposal;
  resolved: string[];
  redirects: Record<string, string>;
  source_ranges: Record<string, TeachingSourceRange[]>;
  inspected: string[];
  read_ranges: Array<TeachingSourceRange & { source_id: string; source_revision: string }>;
  search?: { query: string; offset: number; keys: string[]; next_offset: number | null; preparation_required?: true };
  reading?: { lid: string; start: number; end: number; text: string };
  steps_since_progress: number;
  result?: FormalObjects;
  /** A reused finish action needs materialization at the stage-close write boundary. */
  finish_requested?: true;
}
const priorKey = (id: string) => `previous/${encodeURIComponent(id)}`;
const localKey = (fragment: string, key: string) => `${fragment}/${encodeURIComponent(key)}`;
const unique = <T>(items: T[]) => [...new Set(items)];

export function newObjectAlignment(fragments: FormalObjectFragmentResult[]): ObjectAlignmentWork {
  const proposal: FormalObjectProposal = { objects: [], prerequisites: [], correspondences: [], coverage: [] };
  const source_ranges: ObjectAlignmentWork["source_ranges"] = {};
  for (const fragment of fragments) {
    const key = (value: string) => localKey(fragment.fragment_id, value);
    proposal.objects.push(...fragment.proposal.objects.map(o => ({ ...o, key: key(o.key),
      participants: o.participants.map(p => ({ ...p, object_key: key(p.object_key) })), component_keys: o.component_keys.map(key) })));
    for (const object of fragment.proposal.objects) source_ranges[key(object.key)] = fragment.source_ranges.filter(r => object.source_bindings.some(b => b.lid === r.lid));
    proposal.prerequisites.push(...fragment.proposal.prerequisites.map(p => ({ ...p, target_key: key(p.target_key), prerequisite_key: key(p.prerequisite_key) })));
    for (const coverage of fragment.proposal.coverage) {
      const old = proposal.coverage.find(c => c.unit_lid === coverage.unit_lid);
      if (old) {
        old.object_keys.push(...coverage.object_keys.map(key));
        old.explanation += `\n${coverage.explanation}`;
        for (const binding of coverage.source_bindings) if (!old.source_bindings.some(b => b.lid === binding.lid)) old.source_bindings.push(binding);
      } else proposal.coverage.push({ ...coverage, object_keys: coverage.object_keys.map(key) });
    }
  }
  return { version: "formal_object_alignment.v3", proposal, resolved: [], redirects: {}, source_ranges, inspected: [], read_ranges: [], steps_since_progress: 0 };
}
function requireReadBindings(work: ObjectAlignmentWork, bindings: TeachingSourceBinding[], source: TeachingSource): void {
  for (const binding of bindings) {
    const range = binding.range_utf16 ?? { start: 0, end: source.passages.find(p => p.lid === binding.lid)!.text.length };
    if (!work.read_ranges.some(r => r.source_id === binding.source_id && r.source_revision === binding.source_revision
      && r.lid === binding.lid && r.start <= range.start && r.end >= range.end)) throw new Error("alignment cites unread source range");
  }
}
function oldObjects(previous?: FormalObjects) {
  return previous?.active_refs.map(ref => previous.objects.filter(o => o.ref.object_id === ref.object_id).at(-1)!) ?? [];
}
function unclaimed(work: ObjectAlignmentWork, previous?: FormalObjects) {
  return oldObjects(previous).filter(old => !work.proposal.objects.some(o => o.existing_ref?.object_id === old.ref.object_id)
    && !work.proposal.correspondences.some(c => c.from.some(r => r.object_id === old.ref.object_id)));
}
export function alignmentFocus(work: ObjectAlignmentWork, previous?: FormalObjects) {
  const object = work.proposal.objects.find(o => !work.resolved.includes(o.key));
  if (object) return { kind: "candidate" as const, key: object.key, object };
  const old = unclaimed(work, previous)[0];
  if (old) return { kind: "identity" as const, key: priorKey(old.ref.object_id), object: old };
  return { kind: "finish" as const };
}
function catalog(work: ObjectAlignmentWork, previous?: FormalObjects) {
  return [...work.proposal.objects.map(o => ({ key: o.key, object: o })),
    ...oldObjects(previous).map(o => ({ key: priorKey(o.ref.object_id), object: o }))];
}
export function alignmentRetrievalRequest(work: ObjectAlignmentWork, previous?: FormalObjects): RetrievalRequest {
  const focus = alignmentFocus(work, previous), focus_key = focus.kind === "finish" ? undefined : focus.key;
  return work.search ? { kind: "search", query: work.search.query, ...(focus_key ? { focus_key } : {}) }
    : focus_key ? { kind: "focus", focus_key } : { kind: "search", query: "" };
}
function requirePreparedRetrieval(work: ObjectAlignmentWork, previous: FormalObjects | undefined, prepared: PreparedRetrieval) {
  const d = prepared.dependencies;
  if (!retrievalPreparationMatches(prepared, retrievalDependencies(projectRetrievalCatalog(retrievalCatalog(work.proposal, previous)),
    alignmentRetrievalRequest(work, previous), d.mode, d.provider, d.policy))) throw new Error("alignment retrieval preparation required or stale");
}
/** The persisted ledger is complete; only a bounded focus, page and inspected records enter the model. */
export function objectAlignmentInput(work: ObjectAlignmentWork, source: TeachingSource, previous?: FormalObjects, prepared?: PreparedRetrieval) {
  if (prepared) requirePreparedRetrieval(work, previous, prepared);
  else if (work.search?.preparation_required) throw new Error("alignment retrieval preparation required");
  const focus = alignmentFocus(work, previous);
  const records = catalog(work, previous);
  const summaries = (keys: string[]) => keys.flatMap(key => {
    const record = records.find(r => r.key === key);
    return record ? [{ key, meaning: record.object.meaning.slice(0, 180), kind: record.object.kind,
      conditions: record.object.conditions.join("; ").slice(0, 180) }] : [];
  });
  const focusBindings = focus.kind === "finish" ? [] : focus.object.source_bindings;
  const page = prepared ? retrievalPage(prepared.sequence, work.search?.offset ?? 0, prepared.dependencies.policy) : undefined;
  const search = page ? { query: work.search?.query ?? (focus.kind === "finish" ? "" : focus.object.meaning),
    mode: work.search ? "explicit" : "focus", offset: page.offset, keys: page.items.map(r => r.key), next_offset: page.next_offset,
    candidate_count: page.candidate_count, semantic_truncated: page.semantic_truncated,
    items: page.items.flatMap(hit => summaries([hit.key]).map(summary => ({ ...summary, match_reasons: hit.match_reasons }))) }
    : work.search ? { ...work.search, items: summaries(work.search.keys) } : undefined;
  return { mode: "alignment", source_id: source.source_id, source_revision: source.source_revision, focus,
    remaining_candidates: work.proposal.objects.filter(o => !work.resolved.includes(o.key)).length,
    remaining_identities: unclaimed(work, previous).length,
    catalog_size: records.length, source_ranges: focus.kind === "candidate" ? work.source_ranges[focus.key] : [],
    search,
    inspected_keys: work.inspected, inspected: work.inspected.slice(-3).map(key => records.find(r => r.key === key)), reading: work.reading,
    read_ranges: work.read_ranges,
    source_previews: unique(focusBindings.map(b => b.lid)).slice(0, 3).flatMap(lid => {
      const passage = source.passages.find(p => p.lid === lid);
      return passage ? [{ lid, start: 0, end: Math.min(passage.text.length, 400), total: passage.text.length, text: passage.text.slice(0, 400) }] : [];
    }) };
}

export function advanceObjectAlignment(input: { work: ObjectAlignmentWork; action: unknown; source: TeachingSource;
  previous?: FormalObjects; operation_id: string; retrieval?: PreparedRetrieval }): ObjectAlignmentWork {
  const work = structuredClone(input.work), { source, previous } = input;
  if (work.version !== "formal_object_alignment.v3") throw new Error("object alignment contract is stale");
  if (work.result) throw new Error("object alignment already complete");
  const action = AlignmentActionZ.parse(input.action), focus = alignmentFocus(work, previous);
  if (input.retrieval) requirePreparedRetrieval(work, previous, input.retrieval);
  const records = catalog(work, previous);
  const visible = new Set([...work.inspected, ...(focus.kind === "finish" ? [] : [focus.key])]);
  work.steps_since_progress++;
  if (action.kind === "search") {
    if (input.retrieval || work.search?.preparation_required) {
      // SR3 explicit-preparation path. The writer records intent only; SR4 owns production scheduling.
      work.search = { query: action.query, offset: action.offset, keys: [], next_offset: null, preparation_required: true };
      return work;
    }
    const terms = action.query.toLocaleLowerCase().split(/\s+/u).filter(Boolean);
    const hits = records.filter(r => !terms.length || terms.some(t => JSON.stringify(r.object).toLocaleLowerCase().includes(t)));
    if (action.offset > hits.length) throw new Error("alignment search offset out of range");
    const page = hits.slice(action.offset, action.offset + 6);
    work.search = { query: action.query, offset: action.offset, keys: page.map(r => r.key), next_offset: action.offset + page.length < hits.length ? action.offset + page.length : null };
    return work;
  }
  if (action.kind === "inspect") {
    if (!records.some(r => r.key === action.key)) throw new Error("alignment object unavailable");
    work.inspected = [...work.inspected.filter(k => k !== action.key), action.key];
    return work;
  }
  if (action.kind === "read") {
    const passage = source.passages.find(p => p.lid === action.lid);
    if (!passage || action.start >= action.end || action.end > passage.text.length || action.end - action.start > 2000)
      throw new Error(`alignment source range invalid or too large: lid=${action.lid}; source_length_utf16=${passage?.text.length ?? "unavailable"}; require 0 <= start < end <= source_length_utf16 and end - start <= 2000`);
    work.reading = { ...action, text: passage.text.slice(action.start, action.end) };
    const samePassage = (r: ObjectAlignmentWork["read_ranges"][number]) => r.source_id === source.source_id
      && r.source_revision === source.source_revision && r.lid === action.lid;
    const ranges = [...work.read_ranges.filter(samePassage), { source_id: source.source_id,
      source_revision: source.source_revision, lid: action.lid, start: action.start, end: action.end }].sort((a, b) => a.start - b.start);
    const merged: ObjectAlignmentWork["read_ranges"] = [];
    for (const range of ranges) {
      const last = merged.at(-1);
      if (last && range.start <= last.end) last.end = Math.max(last.end, range.end);
      else merged.push(range);
    }
    work.read_ranges = [...work.read_ranges.filter(r => !samePassage(r)), ...merged];
    return work;
  }
  if (action.kind === "resolve") {
    if (focus.kind !== "candidate" || !action.keys.includes(focus.key) || !action.keys.includes(action.object.key)
      || unique(action.keys).length !== action.keys.length || action.object.existing_ref) throw new Error("alignment resolve must cover the current candidate without assigning identity");
    const selected = action.keys.map(key => {
      const object = work.proposal.objects.find(o => o.key === key);
      if (!object || !visible.has(key)) throw new Error("alignment merge requires inspected current objects");
      return object;
    });
    checkTeachingBindings(action.object.source_bindings, source);
    const allowedLids = new Set([...selected.flatMap(o => o.source_bindings.map(b => b.lid)), ...(work.reading ? [work.reading.lid] : [])]);
    if (action.object.source_bindings.some(b => !allowedLids.has(b.lid))) throw new Error("alignment cites uninspected source");
    for (const key of [...action.object.participants.map(p => p.object_key), ...action.object.component_keys]) {
      if (!work.proposal.objects.some(o => o.key === key)) throw new Error("alignment reference is unresolved");
    }
    if (action.object.kind === "relation" && action.object.participants.length < 2) throw new Error("learnable relation requires participants and roles");
    if (action.object.kind === "composite" && !action.object.component_keys.length) throw new Error("composite object requires components");
    requireReadBindings(work, action.object.source_bindings, source);
    const replacement = { ...action.object, candidate_refs: unique([...action.object.candidate_refs, ...selected.flatMap(o => o.candidate_refs)]) };
    const ranges = action.keys.flatMap(key => work.source_ranges[key] ?? []);
    for (const key of action.keys) delete work.source_ranges[key];
    work.source_ranges[replacement.key] = ranges;
    work.proposal.objects = work.proposal.objects.filter(o => !action.keys.includes(o.key));
    work.proposal.objects.push(replacement);
    const remap = (key: string) => action.keys.includes(key) ? replacement.key : key;
    for (const object of work.proposal.objects) {
      object.participants = object.participants.map(p => ({ ...p, object_key: remap(p.object_key) }));
      object.component_keys = unique(object.component_keys.map(remap));
      if (object.component_keys.includes(object.key)) throw new Error("alignment merge creates self containment");
    }
    for (const coverage of work.proposal.coverage) coverage.object_keys = unique(coverage.object_keys.map(remap));
    for (const prerequisite of work.proposal.prerequisites) {
      prerequisite.target_key = remap(prerequisite.target_key); prerequisite.prerequisite_key = remap(prerequisite.prerequisite_key);
    }
    for (const key of Object.keys(work.redirects)) work.redirects[key] = remap(work.redirects[key]);
    for (const key of action.keys) work.redirects[key] = replacement.key;
    work.resolved = unique([...work.resolved.map(remap), replacement.key]);
  } else if (action.kind === "identity") {
    if (focus.kind !== "identity" || !action.from.some(r => r.object_id === focus.object.ref.object_id)
      || unique(action.from.map(r => r.object_id)).length !== action.from.length || unique(action.to_keys).length !== action.to_keys.length) throw new Error("identity decision must cover current prior identity");
    for (const ref of action.from) if (ref.source_id !== source.source_id || !visible.has(priorKey(ref.object_id))
      || !unclaimed(work, previous).some(o => o.ref.object_id === ref.object_id)) throw new Error("identity decision requires inspected unclaimed prior identity");
    const targets = action.to_keys.map(key => {
      const object = work.proposal.objects.find(o => o.key === key);
      if (!object || !visible.has(key) || object.existing_ref || work.proposal.correspondences.some(c => c.to_keys.includes(key))) throw new Error("identity target unavailable, uninspected or already assigned");
      return object;
    });
    checkTeachingBindings(action.source_bindings, source);
    if (action.source_bindings.some(b => !targets.some(o => o.source_bindings.some(binding => binding.lid === b.lid)))) throw new Error("identity evidence must bind current objects");
    if (action.from.length > 1 && targets.length > 1) throw new Error("identity decision must be a split or merge");
    requireReadBindings(work, action.source_bindings, source);
    if (action.from.length === 1 && targets.length === 1) targets[0].existing_ref = action.from[0];
    else {
      work.proposal.correspondences.push({ kind: action.from.length > 1 ? "merge" : "split", from: action.from,
        to_keys: action.to_keys, reason: action.reason, source_bindings: action.source_bindings });
    }
  } else {
    if (focus.kind !== "finish") throw new Error("object alignment coverage incomplete");
    work.result = acceptFormalObjects({ source, proposal: work.proposal, previous, operation_id: input.operation_id });
  }
  work.steps_since_progress = 0;
  delete work.finish_requested;
  work.inspected = [];
  work.read_ranges = [];
  delete work.search; delete work.reading;
  return work;
}
