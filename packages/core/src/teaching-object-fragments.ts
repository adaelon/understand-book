import type { BookStructureSidecar, BookStructureUnitSource } from "./book-structure";
import { canonicalBuildJson } from "./build-intent";
import { evaluateModelInputBudget, type ModelInputBudgetRequestV1 } from "./model-input-budget";
import { routeModelInputSlices } from "./model-input-slice";
import { acceptFormalObjects, FormalObjectProposalZ, type FormalObjectProposal, type TeachingSource } from "./teaching-map";

export interface TeachingSourceRange { lid: string; start: number; end: number }
export interface TeachingAuxiliaryRange { item_id: string; start: number; end: number; total: number }
interface AuxiliaryItem { id: string; text: string }
export interface FormalObjectFragment {
  id: string;
  source_ranges: TeachingSourceRange[];
  visible_ranges: TeachingSourceRange[];
  auxiliary_ranges: TeachingAuxiliaryRange[];
  input: {
    mode: "fragment";
    source: TeachingSource;
    source_ranges: TeachingSourceRange[];
    visible_ranges: TeachingSourceRange[];
    candidates: Array<TeachingAuxiliaryRange & { text: string }>;
    feedback: unknown[];
    repair_revision: number;
  };
}
export interface FormalObjectFragmentResult {
  version: "formal_object_fragment.v1";
  fragment_id: string;
  source_ranges: TeachingSourceRange[];
  auxiliary_ranges: TeachingAuxiliaryRange[];
  proposal: FormalObjectProposal;
}
type Budget = Omit<ModelInputBudgetRequestV1, "rendered_input">;

function auxiliaryItems(unit: string, units: BookStructureUnitSource[], structure: BookStructureSidecar): AuxiliaryItem[] {
  const result: AuxiliaryItem[] = [];
  const add = (kind: string, values: unknown[]) => values.forEach((value, index) => result.push({
    id: `${unit}/${kind}/${index}`, text: canonicalBuildJson(value),
  }));
  add("structure", structure.spine.filter(s => s.lid === unit));
  add("throughline", structure.throughlines.filter(s => s.lids.includes(unit)));
  const source = units.find(u => u.unit_lid === unit);
  const lids = new Set(source?.leaf_lids ?? []);
  add("key_stop", structure.key_stops.filter(s => lids.has(s.lid)));
  if (source) for (const kind of ["graph_nodes", "graph_edges", "discourse_items", "formula_semantics", "pass2_edges"] as const) add(kind, source[kind]);
  return result;
}

/** Source cores are an exact partition; candidate metadata has its own partition. */
export function routeFormalObjectFragments(input: {
  source: TeachingSource; units: BookStructureUnitSource[]; structure: BookStructureSidecar; budget: Budget;
  feedback_by_unit?: Record<string, unknown[]>;
  feedback_revision_by_unit?: Record<string, number>;
}): FormalObjectFragment[] {
  const fragments: FormalObjectFragment[] = [];
  const unitCounts = new Map<string, number>();
  const fits = (packet: FormalObjectFragment["input"]) => evaluateModelInputBudget({ ...input.budget,
    rendered_input: canonicalBuildJson(packet) }).status === "within_limit";
  const packet = (passages: TeachingSource["passages"], ranges: TeachingSourceRange[], candidates: FormalObjectFragment["input"]["candidates"], visible = ranges): FormalObjectFragment["input"] => ({
    mode: "fragment", source: { source_id: input.source.source_id, source_revision: input.source.source_revision, passages },
    source_ranges: ranges, visible_ranges: visible, candidates, feedback: input.feedback_by_unit?.[passages[0].unit_lid] ?? [],
    repair_revision: input.feedback_revision_by_unit?.[passages[0].unit_lid] ?? 0,
  });
  const push = (body: FormalObjectFragment["input"]) => {
    if (!fits(body)) throw new Error("formal object fragment exceeds task budget");
    const unit = body.source.passages[0].unit_lid, ordinal = unitCounts.get(unit) ?? 0;
    unitCounts.set(unit, ordinal + 1);
    fragments.push({ id: `${encodeURIComponent(unit)}-fragment-${ordinal}`, source_ranges: body.source_ranges, visible_ranges: body.visible_ranges,
      auxiliary_ranges: body.candidates.map(({ text: _text, ...range }) => range), input: body });
  };
  const fullRanges = (passages: TeachingSource["passages"]) => passages.map(p => ({ lid: p.lid, start: 0, end: p.text.length }));
  const metadata = (items: AuxiliaryItem[]) => items.map(item => ({ item_id: item.id, start: 0, end: item.text.length, total: item.text.length, text: item.text }));
  for (const unit of [...new Set(input.source.passages.map(p => p.unit_lid))]) {
    const passages = input.source.passages.filter(p => p.unit_lid === unit);
    const items = auxiliaryItems(unit, input.units, input.structure);
    const whole = packet(passages, fullRanges(passages), metadata(items));
    if (fits(whole)) { push(whole); continue; }
    // Pack complete passages first; only an individually oversized leaf needs slicing.
    for (let start = 0; start < passages.length;) {
      let low = start + 1, high = passages.length, end = start;
      while (low <= high) {
        const mid = Math.floor((low + high) / 2), selected = passages.slice(start, mid);
        if (fits(packet(selected, fullRanges(selected), []))) { end = mid; low = mid + 1; } else high = mid - 1;
      }
      if (end > start) { const selected = passages.slice(start, end); push(packet(selected, fullRanges(selected), [])); start = end; continue; }
      const passage = passages[start];
      const routed = routeModelInputSlices({ source: passage.text, source_fingerprint: input.source.source_revision,
        parent: { lid: passage.lid, path: [], kind: passage.kind ?? "paragraph", children: [], span: { start: 0, end: passage.text.length } },
        budget: input.budget, context_overlap_utf16: 120,
        render: part => canonicalBuildJson(packet([{ ...passage, text: part.context_before + part.core + part.context_after }],
          [{ lid: passage.lid, ...part.core_span_utf16 }], [], [{ lid: passage.lid, ...part.context_span_utf16 }])) });
      if (routed.status === "blocked") throw new Error(`formal source fragment cannot fit: ${passage.lid}/${routed.recovery.reason}`);
      for (const part of routed.slices) push(JSON.parse(part.rendered_input) as FormalObjectFragment["input"]);
      start++;
    }
    // Large candidate sets are kept as hints with bounded original context, separately
    // from source coverage. A large individual record is delivered as ordered text parts.
    const preview = [{ ...passages[0], text: passages[0].text.slice(0, 400) }];
    const visible = fullRanges(preview);
    for (let start = 0; start < items.length;) {
      let low = start + 1, high = items.length, end = start;
      while (low <= high) {
        const mid = Math.floor((low + high) / 2);
        if (fits(packet(preview, [], metadata(items.slice(start, mid)), visible))) { end = mid; low = mid + 1; } else high = mid - 1;
      }
      if (end > start) { push(packet(preview, [], metadata(items.slice(start, end)), visible)); start = end; continue; }
      const item = items[start];
      const routed = routeModelInputSlices({ source: item.text, source_fingerprint: input.source.source_revision,
        parent: { lid: item.id, path: [], kind: "paragraph", children: [], span: { start: 0, end: item.text.length } }, budget: input.budget,
        render: part => canonicalBuildJson(packet(preview, [], [{ item_id: item.id, ...part.core_span_utf16,
          total: item.text.length, text: part.core }], visible)) });
      if (routed.status === "blocked") throw new Error(`formal candidate fragment cannot fit: ${item.id}/${routed.recovery.reason}`);
      for (const part of routed.slices) push(JSON.parse(part.rendered_input) as FormalObjectFragment["input"]);
      start++;
    }
  }
  validateFormalFragmentCoverage(input.source, fragments);
  return fragments;
}

export function validateFormalFragmentCoverage(source: TeachingSource, fragments: FormalObjectFragment[]): void {
  const ids = new Set<string>();
  for (const f of fragments) {
    if (ids.has(f.id)) throw new Error("duplicate formal object fragment");
    ids.add(f.id);
    if (f.input.source.source_id !== source.source_id || f.input.source.source_revision !== source.source_revision) throw new Error("formal fragment source is stale");
  }
  const all = fragments.flatMap(f => f.source_ranges);
  if (all.some(r => !source.passages.some(p => p.lid === r.lid))) throw new Error("formal fragment covers unknown source");
  for (const passage of source.passages) {
    let cursor = 0;
    for (const r of all.filter(r => r.lid === passage.lid).sort((a, b) => a.start - b.start)) {
      if (r.start !== cursor || r.end <= r.start || r.end > passage.text.length) throw new Error("formal fragment source coverage has a gap or overlap");
      cursor = r.end;
    }
    if (cursor !== passage.text.length) throw new Error("formal fragment source coverage incomplete");
  }
  const auxiliary = fragments.flatMap(f => f.auxiliary_ranges);
  for (const id of new Set(auxiliary.map(r => r.item_id))) {
    const ranges = auxiliary.filter(r => r.item_id === id).sort((a, b) => a.start - b.start);
    let cursor = 0;
    for (const r of ranges) {
      if (r.start !== cursor || r.end <= r.start || r.total !== ranges[0].total || r.end > r.total) throw new Error("formal candidate coverage has a gap or overlap");
      cursor = r.end;
    }
    if (cursor !== ranges[0].total) throw new Error("formal candidate coverage incomplete");
  }
}

export function acceptFormalObjectFragment(fragment: FormalObjectFragment, candidate: unknown): FormalObjectFragmentResult {
  const proposal = FormalObjectProposalZ.parse(candidate);
  if (proposal.correspondences.length || proposal.objects.some(o => o.existing_ref)) throw new Error("local candidates cannot assign formal identities");
  // Reuse the object/reference contract with local temporary identities only. The
  // returned formal set is deliberately discarded; publication follows reconciliation.
  let ordinal = 0;
  acceptFormalObjects({ source: fragment.input.source, proposal, operation_id: fragment.id, allocate_id: () => `local-${ordinal++}` });
  return { version: "formal_object_fragment.v1", fragment_id: fragment.id, source_ranges: fragment.source_ranges,
    auxiliary_ranges: fragment.auxiliary_ranges, proposal };
}
