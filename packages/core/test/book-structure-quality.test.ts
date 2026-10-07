import { createHash } from "node:crypto";
import { expect, it } from "vitest";
import { evaluateAutomaticBuildStageQualityV2, type AutomaticBuildStageQualityRoutingEvidenceV2 } from "../src/automatic-build-quality";
import { createAutomaticBuildStagePolicySet, resolveAutomaticBuildStagePolicyMember } from "../src/automatic-build-policy-generation";
import { automaticBuildExtractorForWorkUnitKind } from "../src/build-orchestrator";
import { createBookStructureExecutionContractsV2 } from "../src/book-structure";
import { bookStructureRelationContracts } from "../src/book-structure-relation-routing";
import { resolveContentProfile } from "../src/content-profile";
import { evaluateModelInputBudget } from "../src/model-input-budget";
import { buildSemanticArtifactEnvelopeV3, type SemanticArtifactEnvelopeV3 } from "../src/semantic-artifact";
import { buildWorkUnitCostFromBudgetProof, createWorkUnitDescriptorV3, type WorkUnitDescriptorV3 } from "../src/stage-work-unit";

const target = { version: "build_target_ref.v2" as const, workspace_dir: "C:/quality-book", book_id: "quality-book", profile_id: "technical_learning" as const, input_fingerprint: "a".repeat(64) };
const contracts = createBookStructureExecutionContractsV2({ profile: resolveContentProfile("technical_learning"), quality_profile: "full", prompts: {
  whole: "whole", fragment: "fragment", reduce: "reduce", stitch: "stitch", stitch_fragment: "local", stitch_reduce: "stitch-reduce",
} });
const delta = bookStructureRelationContracts(contracts.stitch).delta;
const members = [
  { kind: "structure_fragment" as const, policy_generation_id: "fragment.v1", policy_fingerprint: contracts.fragment.policy_fingerprint },
  { kind: "structure_reduce" as const, policy_generation_id: "reduce.v1", policy_fingerprint: contracts.reduce.policy_fingerprint },
  { kind: "structure_stitch_fragment" as const, policy_generation_id: "local.v1", policy_fingerprint: contracts.stitch_fragment.policy_fingerprint },
  { kind: "structure_relation_delta" as const, policy_generation_id: "delta.v1", policy_fingerprint: delta.policy_fingerprint },
].map(member => ({ ...member, extractor: automaticBuildExtractorForWorkUnitKind("book_structure", member.kind as WorkUnitDescriptorV3["kind"]) }));

function fixture() {
  const policySet = createAutomaticBuildStagePolicySet({ target_ref: target, stage: "book_structure", members, frozen_at: "2026-10-01T00:00:00Z" });
  const units: WorkUnitDescriptorV3[] = [];
  const artifacts: Record<string, SemanticArtifactEnvelopeV3<unknown>> = {};
  const add = (id: string, kind: string, evidence: string[], basis: WorkUnitDescriptorV3["input_basis"], aggregation?: WorkUnitDescriptorV3["aggregation"], dependencies: WorkUnitDescriptorV3["dependencies"] = []) => {
    const policy = kind === "structure_fragment" ? contracts.fragment.policy_fingerprint : kind === "structure_reduce" ? contracts.reduce.policy_fingerprint : kind === "structure_stitch_fragment" ? contracts.stitch_fragment.policy_fingerprint : delta.policy_fingerprint;
    const rendered = JSON.stringify({ id, evidence });
    const evaluated = evaluateModelInputBudget({ rendered_input: rendered, router_version: policy.router_version, prompt_sha256: policy.prompt_sha256, stage_body_limit_tokens: 5000, executor_context_floor_tokens: 8192, prompt_reserve_tokens: 512, protocol_reserve_tokens: 256, output_reserve_tokens: 1024, safety_margin_tokens: 256 });
    if (evaluated.status !== "within_limit") throw Error("fixture input must fit");
    const descriptor = createWorkUnitDescriptorV3({ target, stage: "book_structure", work_unit_id: id, kind: kind as WorkUnitDescriptorV3["kind"], input_basis: basis, input_hash: evaluated.proof.rendered_input_sha256, input_budget_proof: evaluated.proof, policy_fingerprint: policy, evidence_lids: evidence, dependencies, ...(aggregation ? { aggregation } : {}), cost: buildWorkUnitCostFromBudgetProof({ rendered_input: rendered, proof: evaluated.proof, visible_lids: evidence.length, expected_output_items: 1 }) });
    units.push(descriptor);
    const member = resolveAutomaticBuildStagePolicyMember(policySet, descriptor.kind, policy);
    artifacts[id] = buildSemanticArtifactEnvelopeV3({ target, stage: "book_structure", work_unit_id: id, input_hash: descriptor.input_hash, policy_generation_id: member.policy_generation_id, semantic_contract: member.semantic_contract, provenance: { executor: "quality-fixture", attempt: 1, generated_at: "2026-10-01T00:00:00Z" }, payload: kind === "structure_relation_delta" ? { new_throughlines: [], extend_throughlines: [], merge_throughlines: [], add_dependencies: [] } : kind === "structure_stitch_fragment" ? { spine: [{ summary: { text: "Grounded chapter summary", evidence_lids: ["1.1"] } }] } : { output: { summary: { text: "Grounded chapter summary", evidence_lids: ["1.1"] } } } });
    return descriptor;
  };
  const projection = (evidence: string[], start: number, end: number) => ({ kind: "semantic_projection" as const, projection_kind: "book_structure" as const, source_fingerprint: target.input_fingerprint, projection_sha256: "b".repeat(64), parent_lids: evidence, core_range: { start_ordinal: start, end_ordinal_exclusive: end } });
  add("fragment-0", "structure_fragment", ["1", "1.1"], projection(["1", "1.1"], 0, 2), { parent_lid: "1", role: "fragment" });
  add("fragment-1", "structure_fragment", ["1", "1.2", "1.3"], projection(["1", "1.2", "1.3"], 2, 4), { parent_lid: "1", role: "fragment" });
  const deps = (ids: string[]) => ids.map(id => ({ artifact: id, sha256: artifacts[id].artifact_hash }));
  const reduceDeps = deps(["fragment-0", "fragment-1"]);
  // A reducer delivers selected child evidence, while its dependency closure
  // covers every original leaf. Later local and relation contributions share the chapter.
  add("final", "structure_reduce", ["1", "1.1"], { kind: "artifact_reduction", parent_lids: ["1", "1.1"], dependency_artifacts: reduceDeps.map(item => ({ work_unit_id: item.artifact, artifact_hash: item.sha256 })) }, { parent_lid: "1", role: "final" }, reduceDeps);
  add("local", "structure_stitch_fragment", ["stitch", "1"], projection(["stitch", "1"], 0, 1), { parent_lid: "stitch", role: "fragment" });
  const relationDeps = deps(["local"]);
  add("relation", "structure_relation_delta", ["1"], { kind: "artifact_reduction", parent_lids: ["1"], dependency_artifacts: relationDeps.map(item => ({ work_unit_id: item.artifact, artifact_hash: item.sha256 })) }, undefined, relationDeps);
  const routing: AutomaticBuildStageQualityRoutingEvidenceV2 = Object.assign({ policy_set: policySet, coverage: [], public_contributors: [
    { contributor_id: "chapter", work_unit_id: "final", parent_lids: ["1", "1.1", "1.2", "1.3"] },
    { contributor_id: "local", work_unit_id: "local", parent_lids: ["1"] },
    { contributor_id: "relation", work_unit_id: "relation", parent_lids: ["1"] },
  ], reduction_parents: [{ parent_lid: "1", fragment_work_unit_ids: ["fragment-0", "fragment-1"], final_work_unit_ids: ["final"] }], book_structure_assembly: { local_work_unit_ids: ["local"], selection_work_unit_ids: [], relation_work_unit_ids: ["relation"] } }, { book_structure_coverage: [{ version: "book_structure_leaf_coverage.v1" as const, parent_unit_lid: "1", expected_leaf_count: 4, covered_leaf_count: 4, gap_count: 0, core_overlap_count: 0, coverage_digest: createHash("sha256").update("fixture leaf cover").digest("hex"), core_ranges: [
    { start_ordinal: 0, end_ordinal_exclusive: 2, work_unit_id: "fragment-0", leaf_lids: ["1", "1.1"] },
    { start_ordinal: 2, end_ordinal_exclusive: 4, work_unit_id: "fragment-1", leaf_lids: ["1.2", "1.3"] },
  ] }] });
  return { units, artifacts, routing };
}
const evaluate = (input: ReturnType<typeof fixture>) => evaluateAutomaticBuildStageQualityV2({ target_ref: target, stage: "book_structure", quality_profile: "full", work_units: input.units, artifacts: input.artifacts, routing: input.routing });

it("accepts exact semantic leaf coverage, one chapter final, and transitive source evidence despite later chapter contributions", () => {
  expect(evaluate(fixture())).toMatchObject({ gate_status: "passed", integrity: { violations: [] }, reduction: { missing_or_duplicate_parent_lids: 0 } });
});

it("still rejects incomplete leaf coverage, missing or changed dependencies, duplicate finals, and source outside the dependency closure", () => {
  const gap = fixture();
  gap.routing.book_structure_coverage![0].core_ranges.pop();
  expect(evaluate(gap).integrity.violations).toContain("source_slice_coverage_invalid");
  const overlap = fixture();
  overlap.routing.book_structure_coverage![0].core_ranges[1].start_ordinal = 1;
  expect(evaluate(overlap).integrity.violations).toContain("source_slice_coverage_invalid");
  const absentCoverage = fixture();
  delete absentCoverage.routing.book_structure_coverage;
  expect(evaluate(absentCoverage).integrity.violations).toContain("source_slice_coverage_invalid");
  const child = fixture();
  delete child.artifacts["fragment-1"];
  expect(evaluate(child).integrity.violations).toContain("reduction_dependency_closure_stale");
  const changed = fixture();
  changed.artifacts["fragment-1"] = buildSemanticArtifactEnvelopeV3({ ...changed.artifacts["fragment-1"], payload: { output: { summary: { text: "Changed child result", evidence_lids: ["1.2"] } } } });
  expect(evaluate(changed).integrity.violations).toContain("reduction_dependency_closure_stale");
  const duplicate = fixture();
  duplicate.routing.reduction_parents[0].final_work_unit_ids.push("local");
  expect(evaluate(duplicate).integrity.violations).toContain("public_contributor_cardinality_invalid");
  const outside = fixture();
  outside.routing.public_contributors[0].parent_lids.push("outside");
  expect(evaluate(outside).integrity.violations).toContain("incomplete_eligible_closure");
});
