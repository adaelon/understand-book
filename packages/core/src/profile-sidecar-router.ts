import { createHash } from "node:crypto";
import { AUTOMATIC_BUILD_MODEL_INPUT_BUDGET_V1 } from "./automatic-build-protocol";
import type { BuildTargetRefV2 } from "./build-orchestrator";
import {
  blockedAutomaticBuildRoute,
  createAutomaticBuildRecoveryEnvelope,
  readyAutomaticBuildRoute,
  type AutomaticBuildRouteResult,
} from "./automatic-build-recovery";
import type { Pass1ArtifactMeta } from "./build-resume";
import type { ContentProfileDefinition } from "./content-profile";
import type { LidNode } from "./generated/LidNode";
import { inspectRenderedModelInput } from "./model-input-renderer";
import { createProfileSidecarContext, type ProfileSidecarContext } from "./profile-sidecar-context";
import { MathStructureQuoteError, restoreMathStructureSourceQuotes, validateMathStructureCandidates, type MathStructureSource } from "./math-structure";
import { ExtractorContractError } from "./extractor-contract";
import {
  renderProfileSidecarDiscourseText,
  type ProfileSidecarArtifact,
  type ProfileSidecarExtractionOutput,
} from "./profile-sidecar-build";
import type { ExtractionPolicyFingerprintV1 } from "./semantic-artifact";
import {
  buildWorkUnitCost,
  createWorkUnitDescriptor,
  workUnitPlanDigest,
  type WorkUnitDescriptorV2,
} from "./stage-work-unit";
import { estimateTokens, type Window } from "./window";

export const PROFILE_SIDECAR_ROUTER_VERSION = "profile_sidecar_semantic_units.v4" as const;
export const DEFAULT_DISCOURSE_GROUP_LIDS = 12;
export const DEFAULT_DISCOURSE_INPUT_TOKENS =
  AUTOMATIC_BUILD_MODEL_INPUT_BUDGET_V1.stage_body_limit_tokens;

export type ProfileSidecarSemanticUnitKind = "profile_sidecar_discourse" | "profile_sidecar_formula";

export interface ProfileSidecarSemanticPacketV2 {
  version: "profile_sidecar_semantic_packet.v2";
  router_version: typeof PROFILE_SIDECAR_ROUTER_VERSION;
  work_unit_id: string;
  unit_kind: ProfileSidecarSemanticUnitKind;
  visible_lids: string[];
  discourse_lids: string[];
  math_anchor_lids: string[];
  formula_lids: string[];
  text: string;
  estimated_input_tokens: number;
  estimated_rendered_tokens: number;
  input_hash: string;
  rendered_input_sha256: string;
}

export interface ProfileSidecarRoutingAccountingV1 {
  discourse_eligible_lids: number;
  discourse_skipped_lids: number;
  discourse_groups: number;
  formula_total: number;
  formula_eligible: number;
  formula_skipped: number;
  no_formula_windows: number;
}

export interface ProfileSidecarRoutingAnalysis {
  version: "profile_sidecar_routing_analysis.v2";
  router_version: typeof PROFILE_SIDECAR_ROUTER_VERSION;
  packets: Record<string, ProfileSidecarSemanticPacketV2>;
  skips: Record<string, { code: string; evidence: string[]; input_hash: string; kind: "discourse" | "formula" }>;
  accounting: ProfileSidecarRoutingAccountingV1;
}

export interface ProfileSidecarRoutingPlan extends Omit<ProfileSidecarRoutingAnalysis, "version"> {
  version: "profile_sidecar_routing_plan.v2";
  work_units: WorkUnitDescriptorV2[];
  plan_digest: string;
}

export interface ProfileSidecarRoutingStatus {
  total: number;
  eligible: number;
  committed: number;
  pending: number;
  skipped: number;
  done_ids: string[];
  pending_ids: string[];
  skipped_ids: string[];
}

export interface ProfileSidecarCandidateStatus extends ProfileSidecarRoutingStatus {
  analysis: ProfileSidecarRoutingAnalysis;
}

export class ProfileSidecarRoutingBudgetBlock extends Error {
  readonly name = "ProfileSidecarRoutingBudgetBlock";

  constructor(
    readonly work_unit_id: string,
    readonly evidence_lids: string[],
    readonly estimated_tokens: number,
    readonly limit_tokens: number,
  ) {
    super("profile sidecar routing requires a budget-routable policy generation");
  }
}

function stableJson(value: unknown): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  const record = value as Record<string, unknown>;
  return `{${Object.keys(record).sort().map((key) => `${JSON.stringify(key)}:${stableJson(record[key])}`).join(",")}}`;
}

function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

function nodeText(node: LidNode, source: string): string {
  return source.slice(node.span.start, node.span.end).trim();
}

function formulaSkipReason(raw: string): string | undefined {
  const trimmed = raw.trim();
  const withoutText = trimmed.replace(/\\text\s*\{[^{}]*\}/g, "");
  if (withoutText !== trimmed && !withoutText.replace(/\$|\\underline|[{}\s]/g, "")) return "formula_text_decoration";
  const body = trimmed
    .replace(/^\$+|\$+$/g, "")
    .replace(/\\(?:mathcal|mathbf|mathrm|operatorname)\s*\{([^{}]+)\}/g, "$1")
    .replace(/[{}\s]/g, "")
    .trim();
  if (/^(?:\{\})?\^\{?(?:\d{1,3}|[*†‡]|\\dagger|\\ddagger)\}?$/.test(body) || /^\^?\d{1,3}$/.test(body)) {
    return "formula_footnote_or_page_marker";
  }
  const hasRelation = /=|\\(?:in|leq?|geq?|approx|sim|propto|to|rightarrow)|[<>]/.test(body);
  const hasStructuredComposition = /\\(?:sum|prod|int|frac|sqrt|operatorname)/.test(body);
  if (!hasRelation && !hasStructuredComposition && /^(?:\\[A-Za-z]+|[\p{L}])(?:[_^](?:[\p{L}\p{N}]|\\[A-Za-z]+))?$/u.test(body)) {
    return "formula_bare_variable";
  }
  if (!hasRelation && !hasStructuredComposition) return "formula_symbol_fragment";
  return undefined;
}

function buildPacket(input: {
  workUnitId: string;
  unitKind: ProfileSidecarSemanticUnitKind;
  visibleLids: string[];
  discourseLids: string[];
  mathAnchorLids: string[];
  formulaLids: string[];
  text: string;
}): ProfileSidecarSemanticPacketV2 {
  const identity = {
    version: "profile_sidecar_semantic_packet.v2" as const,
    router_version: PROFILE_SIDECAR_ROUTER_VERSION,
    work_unit_id: input.workUnitId,
    unit_kind: input.unitKind,
    visible_lids: input.visibleLids,
    discourse_lids: input.discourseLids,
    math_anchor_lids: input.mathAnchorLids,
    formula_lids: input.formulaLids,
    text: input.text,
  };
  const rendered = inspectRenderedModelInput({ kind: input.unitKind, input: identity });
  return {
    ...identity,
    estimated_input_tokens: estimateTokens(stableJson(identity)),
    estimated_rendered_tokens: rendered.estimated_tokens,
    input_hash: sha256(stableJson(identity)),
    rendered_input_sha256: rendered.sha256,
  };
}

function discoursePackets(input: {
  windows: Window[];
  byLid: Map<string, LidNode>;
  source: string;
  contentProfile: ContentProfileDefinition;
  maxGroupLids: number;
  maxInputTokens: number;
  allowOverLimitPackets: boolean;
  context: ProfileSidecarContext;
}): { packets: ProfileSidecarSemanticPacketV2[]; skips: ProfileSidecarRoutingAnalysis["skips"]; eligible: number; skipped: number } {
  const packets: ProfileSidecarSemanticPacketV2[] = [];
  const skips: ProfileSidecarRoutingAnalysis["skips"] = {};
  let eligible = 0;
  let skipped = 0;
  for (const window of input.windows) {
    const eligibleLids: string[] = [];
    const skippedLids: string[] = [];
    for (const lid of window.leafLids) {
      const node = input.byLid.get(lid)!;
      if (node.kind === "formula") continue;
      if (!input.context.discourseTargets.has(lid)) skippedLids.push(lid);
      else eligibleLids.push(lid);
    }
    eligible += eligibleLids.length;
    skipped += skippedLids.length;
    if (skippedLids.length) {
      const workUnitId = `discourse-skip-window-${window.id}`;
      skips[workUnitId] = {
        code: "non_discourse_fragments",
        evidence: skippedLids,
        input_hash: sha256(stableJson({ work_unit_id: workUnitId, evidence: skippedLids.map((lid) => [lid, nodeText(input.byLid.get(lid)!, input.source)]) })),
        kind: "discourse",
      };
    }
    let current: string[] = [];
    const packetFor = (targets: string[]) => {
      const digest = sha256(targets.join("\n")).slice(0, 12);
      const workUnitId = `discourse-${targets[0].replace(/[^A-Za-z0-9]+/g, "-")}-${digest}`;
      const mathAnchorLids = input.contentProfile.id === "technical_learning"
        ? targets.flatMap((lid) => input.context.mathAnchors.has(lid) ? [input.context.mathAnchors.get(lid)!] : []) : [];
      const visibleLids = [...new Set([...targets, ...mathAnchorLids,
        ...targets.flatMap((lid) => input.context.discourseLids(lid)),
        ...(mathAnchorLids.length ? targets.flatMap((lid) => input.context.mathContextLids(lid)) : []),
      ])];
      return buildPacket({
        workUnitId,
        unitKind: "profile_sidecar_discourse",
        visibleLids,
        discourseLids: [...targets],
        mathAnchorLids,
        formulaLids: [],
        text: renderProfileSidecarDiscourseText(input.contentProfile, input.context.render(visibleLids)),
      });
    };
    const flush = () => {
      if (!current.length) return;
      const packet = packetFor(current);
      if (packet.estimated_rendered_tokens > input.maxInputTokens && !input.allowOverLimitPackets) {
        const bodyEstimate = current.length === 1
          ? estimateTokens(nodeText(input.byLid.get(current[0])!, input.source))
          : packet.estimated_rendered_tokens;
        throw new ProfileSidecarRoutingBudgetBlock(
          packet.work_unit_id,
          packet.visible_lids,
          bodyEstimate,
          input.maxInputTokens,
        );
      }
      packets.push(packet);
      current = [];
    };
    for (const lid of eligibleLids) {
      const proposed = [...current, lid];
      if (current.length && (proposed.length > input.maxGroupLids
        || input.context.discourseTargets.get(current[0]) !== input.context.discourseTargets.get(lid)
        || packetFor(proposed).estimated_rendered_tokens > input.maxInputTokens)) flush();
      current.push(lid);
    }
    flush();
  }
  return { packets, skips, eligible, skipped };
}

function bibliographyLids(byLid: Map<string, LidNode>, source: string): Set<string> {
  const excluded = new Set<string>();
  const ordered = [...byLid.values()]
    .filter((node) => node.children.length === 0)
    .sort((left, right) => left.span.start - right.span.start);
  let activeLevel: number | undefined;
  for (const node of ordered) {
    const text = nodeText(node, source).replace(/\s+/g, " ");
    const heading = text.match(/^(#{1,6})\s+(.+?)\s*$/);
    if (heading) {
      if (activeLevel !== undefined && heading[1].length <= activeLevel) activeLevel = undefined;
      if (/^(?:references|bibliography|works cited)(?:\s+continued)?$/i.test(heading[2].trim())) activeLevel = heading[1].length;
    }
    if (activeLevel !== undefined) excluded.add(node.lid);
  }
  return excluded;
}

function formulaPackets(input: {
  windows: Window[];
  byLid: Map<string, LidNode>;
  source: string;
  context: ProfileSidecarContext;
}): { packets: ProfileSidecarSemanticPacketV2[]; skips: ProfileSidecarRoutingAnalysis["skips"]; total: number; skipped: number; noFormulaWindows: number } {
  const packets: ProfileSidecarSemanticPacketV2[] = [];
  const skips: ProfileSidecarRoutingAnalysis["skips"] = {};
  let total = 0;
  let skipped = 0;
  let noFormulaWindows = 0;
  const bibliography = bibliographyLids(input.byLid, input.source);
  for (const window of input.windows) {
    const formulaLids = window.leafLids.filter((lid) => input.byLid.get(lid)?.kind === "formula");
    if (!formulaLids.length) {
      noFormulaWindows += 1;
      const workUnitId = `formula-skip-window-${window.id}`;
      skips[workUnitId] = {
        code: "no_formula_in_window",
        evidence: [...window.leafLids],
        input_hash: sha256(stableJson({ work_unit_id: workUnitId, evidence: window.leafLids.map((lid) => [lid, nodeText(input.byLid.get(lid)!, input.source)]) })),
        kind: "formula",
      };
      continue;
    }
    for (const formulaLid of formulaLids) {
      total += 1;
      const node = input.byLid.get(formulaLid)!;
      let reason = bibliography.has(formulaLid) ? "formula_bibliography_fragment" : formulaSkipReason(nodeText(node, input.source));
      const context = input.context.formulaContext(formulaLid);
      const explanationLids = context.lids.filter((lid) => lid !== formulaLid);
      if (!reason && !context.hasExplanation) reason = "formula_without_explanatory_context";
      const safeLid = formulaLid.replace(/[^A-Za-z0-9]+/g, "-");
      if (reason) {
        skipped += 1;
        const workUnitId = `formula-skip-${safeLid}`;
        skips[workUnitId] = {
          code: reason,
          evidence: [formulaLid, ...explanationLids],
          input_hash: sha256(stableJson({ work_unit_id: workUnitId, formula: nodeText(node, input.source), context: explanationLids.map((lid) => [lid, nodeText(input.byLid.get(lid)!, input.source)]) })),
          kind: "formula",
        };
        continue;
      }
      const visibleLids = [...new Set([formulaLid, ...explanationLids])]
        .sort((left, right) => input.byLid.get(left)!.span.start - input.byLid.get(right)!.span.start);
      const workUnitId = `formula-${safeLid}-${sha256(formulaLid).slice(0, 10)}`;
      packets.push(buildPacket({
        workUnitId,
        unitKind: "profile_sidecar_formula",
        visibleLids,
        discourseLids: [],
        mathAnchorLids: [],
        formulaLids: [formulaLid],
        text: input.context.render(visibleLids),
      }));
    }
  }
  return { packets, skips, total, skipped, noFormulaWindows };
}

export function analyzeProfileSidecarSemanticUnits(input: {
  windows: Window[];
  byLid: Map<string, LidNode>;
  source: string;
  content_profile: ContentProfileDefinition;
  max_discourse_group_lids?: number;
  max_discourse_input_tokens?: number;
  allow_over_limit_packets?: boolean;
}): ProfileSidecarRoutingAnalysis {
  const maxGroupLids = input.max_discourse_group_lids ?? DEFAULT_DISCOURSE_GROUP_LIDS;
  const maxInputTokens = input.max_discourse_input_tokens ?? DEFAULT_DISCOURSE_INPUT_TOKENS;
  if (!Number.isSafeInteger(maxGroupLids) || maxGroupLids < 1) throw new Error("max_discourse_group_lids must be a positive safe integer");
  if (!Number.isSafeInteger(maxInputTokens) || maxInputTokens < 1) throw new Error("max_discourse_input_tokens must be a positive safe integer");
  const context = createProfileSidecarContext(input.source, input.byLid);
  const discourse = discoursePackets({
    windows: input.windows,
    byLid: input.byLid,
    source: input.source,
    contentProfile: input.content_profile,
    maxGroupLids,
    maxInputTokens,
    allowOverLimitPackets: input.allow_over_limit_packets === true,
    context,
  });
  const formula = formulaPackets({ ...input, context });
  const packetList = [...discourse.packets, ...formula.packets];
  return {
    version: "profile_sidecar_routing_analysis.v2",
    router_version: PROFILE_SIDECAR_ROUTER_VERSION,
    packets: Object.fromEntries(packetList.map((packet) => [packet.work_unit_id, packet])),
    skips: { ...discourse.skips, ...formula.skips },
    accounting: {
      discourse_eligible_lids: discourse.eligible,
      discourse_skipped_lids: discourse.skipped,
      discourse_groups: discourse.packets.length,
      formula_total: formula.total,
      formula_eligible: formula.packets.length,
      formula_skipped: formula.skipped,
      no_formula_windows: formula.noFormulaWindows,
    },
  };
}

export function routeProfileSidecarWorkUnits(input: {
  target: BuildTargetRefV2;
  windows: Window[];
  byLid: Map<string, LidNode>;
  source: string;
  content_profile: ContentProfileDefinition;
  policy_fingerprint: ExtractionPolicyFingerprintV1;
  max_discourse_group_lids?: number;
  max_discourse_input_tokens?: number;
}): ProfileSidecarRoutingPlan {
  const analysis = analyzeProfileSidecarSemanticUnits(input);
  const packetUnits = Object.values(analysis.packets).map((packet) => createWorkUnitDescriptor({
    target: input.target,
    stage: "profile_sidecar",
    work_unit_id: packet.work_unit_id,
    kind: packet.unit_kind,
    input_hash: packet.input_hash,
    policy_fingerprint: input.policy_fingerprint,
    evidence_lids: packet.visible_lids,
    cost: buildWorkUnitCost({
      estimated_input_tokens: packet.estimated_input_tokens,
      visible_lids: packet.visible_lids.length,
      formula_lids: packet.formula_lids.length,
      candidate_count: packet.unit_kind === "profile_sidecar_formula" ? 1 : packet.discourse_lids.length + packet.math_anchor_lids.length,
      expected_output_items: packet.unit_kind === "profile_sidecar_formula" ? 1 : packet.discourse_lids.length + packet.math_anchor_lids.length,
    }),
    legacy_artifact_ref: `.build/profile-sidecar/${packet.work_unit_id}.json`,
  }));
  const skipUnits = Object.entries(analysis.skips).map(([workUnitId, skip]) => createWorkUnitDescriptor({
    target: input.target,
    stage: "profile_sidecar",
    work_unit_id: workUnitId,
    kind: skip.kind === "formula" ? "profile_sidecar_formula" : "profile_sidecar_discourse",
    input_hash: skip.input_hash,
    policy_fingerprint: input.policy_fingerprint,
    evidence_lids: skip.evidence,
    cost: buildWorkUnitCost({ visible_lids: skip.evidence.length }),
    deterministic_skip: { code: skip.code, evidence: skip.evidence },
  }));
  const work_units = [...packetUnits, ...skipUnits];
  return {
    ...analysis,
    version: "profile_sidecar_routing_plan.v2",
    work_units,
    plan_digest: workUnitPlanDigest(work_units),
  };
}

export function routeProfileSidecarWorkUnitsWithRecovery(input: Parameters<
  typeof routeProfileSidecarWorkUnits
>[0]): AutomaticBuildRouteResult<ProfileSidecarRoutingPlan> {
  try {
    return readyAutomaticBuildRoute(routeProfileSidecarWorkUnits(input));
  } catch (error) {
    if (!(error instanceof ProfileSidecarRoutingBudgetBlock)) throw error;
    return blockedAutomaticBuildRoute(createAutomaticBuildRecoveryEnvelope({
      phase: "routing",
      code: "policy_generation_migration_required",
      stage: "profile_sidecar",
      target_ref: input.target,
      router_version: PROFILE_SIDECAR_ROUTER_VERSION,
      affected_work_units: [{
        work_unit_id: error.work_unit_id,
        evidence_lids: error.evidence_lids,
        estimated_tokens: error.estimated_tokens,
        limit_tokens: error.limit_tokens,
      }],
      retryable: false,
      recovery_actions: ["migrate_policy"],
    }));
  }
}

function statusForUnits(
  units: Array<{ work_unit_id: string; input_hash: string; deterministic_skip?: unknown }>,
  existing: ReadonlyMap<string, Pass1ArtifactMeta>,
): ProfileSidecarRoutingStatus {
  const done_ids: string[] = [];
  const pending_ids: string[] = [];
  const skipped_ids: string[] = [];
  for (const unit of units) {
    if (unit.deterministic_skip) skipped_ids.push(unit.work_unit_id);
    else if (existing.get(unit.work_unit_id)?.content_hash === unit.input_hash) done_ids.push(unit.work_unit_id);
    else pending_ids.push(unit.work_unit_id);
  }
  return {
    total: units.length,
    eligible: done_ids.length + pending_ids.length,
    committed: done_ids.length,
    pending: pending_ids.length,
    skipped: skipped_ids.length,
    done_ids,
    pending_ids,
    skipped_ids,
  };
}

export function computeProfileSidecarRoutingStatus(
  plan: ProfileSidecarRoutingPlan,
  existing: ReadonlyMap<string, Pass1ArtifactMeta>,
): ProfileSidecarRoutingStatus {
  return statusForUnits(plan.work_units, existing);
}

export function computeProfileSidecarCandidateStatus(input: {
  windows: Window[];
  byLid: Map<string, LidNode>;
  source: string;
  content_profile: ContentProfileDefinition;
  existing: ReadonlyMap<string, Pass1ArtifactMeta>;
  allow_over_limit_packets?: boolean;
}): ProfileSidecarCandidateStatus {
  const analysis = analyzeProfileSidecarSemanticUnits({
    ...input,
    allow_over_limit_packets: input.allow_over_limit_packets,
  });
  const units = [
    ...Object.values(analysis.packets).map((packet) => ({ work_unit_id: packet.work_unit_id, input_hash: packet.input_hash })),
    ...Object.entries(analysis.skips).map(([workUnitId, skip]) => ({ work_unit_id: workUnitId, input_hash: skip.input_hash, deterministic_skip: skip })),
  ];
  return { analysis, ...statusForUnits(units, input.existing) };
}

export function buildProfileSidecarSemanticArtifact(
  packet: ProfileSidecarSemanticPacketV2,
  output: ProfileSidecarExtractionOutput,
  source?: MathStructureSource,
): ProfileSidecarArtifact {
  const discourseItems = output.discourse_items ?? [];
  const formulaSemantics = output.formula_semantics ?? [];
  let mathItems = output.math_items ?? [];
  if (packet.unit_kind === "profile_sidecar_discourse") {
    if (output.formula_semantics !== undefined) throw new Error("discourse unit must not emit formula_semantics");
    const allowed = new Set(packet.discourse_lids);
    if (discourseItems.some((item) => !allowed.has(item.lid))) throw new Error("discourse unit emitted an out-of-scope lid");
  } else {
    if (output.discourse_items !== undefined) throw new Error("formula unit must not emit discourse_items");
    if (output.math_items !== undefined) throw new Error("formula unit must not emit math_items");
    const allowed = new Set(packet.formula_lids);
    if (formulaSemantics.some((item) => !allowed.has(item.formula_lid))) throw new Error("formula unit emitted an out-of-scope formula_lid");
  }
  if (mathItems.length) {
    if (!source) throw new Error("math structure validation requires the source");
    try {
      mathItems = restoreMathStructureSourceQuotes(mathItems, source);
      validateMathStructureCandidates(mathItems, { ...source, anchor_lids: packet.math_anchor_lids, visible_lids: packet.visible_lids });
    } catch (error) {
      if (!(error instanceof MathStructureQuoteError)) throw error;
      throw new ExtractorContractError({
        version: "automatic_build_extractor_diagnostic.v1", code: "quote_not_in_evidence",
        json_pointer: error.json_pointer, expected: error.expected, actual: null,
      });
    }
  }
  return {
    content_hash: packet.input_hash,
    discourse_items: discourseItems,
    formula_semantics: formulaSemantics,
    math_items: mathItems,
  };
}
