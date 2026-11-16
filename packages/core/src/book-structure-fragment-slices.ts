import type { BookStructureFragmentInputV1, BookStructureRoutedWorkUnitV2, evaluateBookStructureExecution } from "./book-structure";
import type { LidNode } from "./generated/LidNode";
import { routeModelInputSlices, type ModelInputSliceCoverageV1, type ModelInputSliceV1, type ModelInputSliceRenderContextV1 } from "./model-input-slice";
import { renderBookStructureFragmentModelInput } from "./model-input-renderer";
import { estimateTokens } from "./window";

type Evaluation = ReturnType<typeof evaluateBookStructureExecution>;
type FragmentResult = { status: "ready"; work_unit: BookStructureRoutedWorkUnitV2 }
  | { status: "blocked"; evaluation: Exclude<Evaluation, { status: "within_limit" }> };

/** Only oversized discovery packets use this path. Each page retains the supplied
 * semantic record, while evidence authorization comes from its visible excerpt. */
export function routeBookStructureFragmentSlices(input: {
  packet: BookStructureFragmentInputV1;
  source: string;
  nodes: Map<string, LidNode>;
  source_fingerprint: string;
  contract: Parameters<typeof evaluateBookStructureExecution>[0]["contract"];
  budget: Parameters<typeof evaluateBookStructureExecution>[0]["budget"];
  create: (packet: BookStructureFragmentInputV1) => FragmentResult;
}): { work_units: BookStructureRoutedWorkUnitV2[]; coverage: ModelInputSliceCoverageV1[] } | undefined {
  const workUnits: BookStructureRoutedWorkUnitV2[] = [];
  const coverage: ModelInputSliceCoverageV1[] = [];
  for (const [excerptIndex, excerpt] of input.packet.excerpts.entries()) {
    const node = input.nodes.get(excerpt.lid);
    if (!node || !excerpt.text.length) return undefined;
    const original = input.source.slice(node.span.start, node.span.end);
    const offset = original.indexOf(excerpt.text);
    if (offset < 0) throw new Error("BookStructure discovery excerpt does not match its canonical leaf");
    const start = node.span.start + offset;
    const parent = { ...node, span: { start, end: start + excerpt.text.length } };
    const packetFor = (context: ModelInputSliceRenderContextV1, slice: ModelInputSliceV1): BookStructureFragmentInputV1 => ({
      ...input.packet,
      work_unit_id: `${input.packet.work_unit_id}:excerpt:${excerptIndex}:slice:${String(slice.ordinal).padStart(4, "0")}`,
      fragment_ordinal: input.packet.fragment_ordinal + workUnits.length + slice.ordinal,
      excerpt_slice: slice,
      reading_scope: "Observe only the source text visible in this page. The excerpt_slice identifies its exact core and context spans. Graph relations and endpoint metadata are navigation; do not infer evidence from unread text or other pages.",
      excerpts: [{ lid: excerpt.lid, text: context.context_before + context.core + context.context_after }],
      graph_nodes: input.packet.graph_nodes.map(node => ({ ...node, occurrences: node.occurrences.filter(lid => lid === excerpt.lid) })),
    });
    let bodyLimit = input.budget.stage_body_limit_tokens;
    for (;;) {
      const routed = routeModelInputSlices({ source: input.source, source_fingerprint: input.source_fingerprint, parent,
        context_overlap_utf16: 160,
        budget: { ...input.budget, stage_body_limit_tokens: bodyLimit,
          router_version: input.contract.policy_fingerprint.router_version, prompt_sha256: input.contract.policy_fingerprint.prompt_sha256,
          prompt_reserve_tokens: estimateTokens(input.contract.semantic_prompt), protocol_reserve_tokens: 0 },
        render: (context, slice) => renderBookStructureFragmentModelInput(packetFor(context, slice)),
      });
      if (routed.status === "blocked") return undefined;
      const built = routed.slices.map(({ slice }) => {
        const context: ModelInputSliceRenderContextV1 = { version: "model_input_slice_render_context.v1", parent_lid: slice.parent_lid,
          ordinal: slice.ordinal, boundary_kind: slice.boundary_kind, core_span_utf16: slice.core_span_utf16,
          context_span_utf16: slice.context_span_utf16,
          context_before: input.source.slice(slice.context_span_utf16.start, slice.core_span_utf16.start),
          core: input.source.slice(slice.core_span_utf16.start, slice.core_span_utf16.end),
          context_after: input.source.slice(slice.core_span_utf16.end, slice.context_span_utf16.end) };
        return input.create(packetFor(context, slice));
      });
      const blocked = built.find(value => value.status === "blocked");
      if (blocked?.status === "blocked") {
        // The final V3 proof also accounts for transport. Refit only this excerpt
        // when that proof requires a smaller delivery than the text budget did.
        bodyLimit = Math.min(Math.floor(bodyLimit / 2), blocked.evaluation.effective_body_limit_tokens);
        if (bodyLimit < 1) return undefined;
        continue;
      }
      workUnits.push(...built.flatMap(value => value.status === "ready" ? [value.work_unit] : []));
      coverage.push(routed.coverage);
      break;
    }
  }
  return workUnits.length ? { work_units: workUnits, coverage } : undefined;
}
