import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  automaticBuildPolicyMigrationReceiptPath,
  createAutomaticBuildStagePolicySet,
  freezeAutomaticBuildStagePolicySet,
  recordAutomaticBuildPolicyMigration,
} from "../src/automatic-build-policy-generation";
import { AUTOMATIC_BUILD_MODEL_INPUT_BUDGET_V1 } from "../src/automatic-build-protocol";
import {
  prepareAutomaticBuildSnapshot,
  resolveAutomaticBuildTarget,
} from "../src/build-orchestrator";
import { resolveContentProfile } from "../src/content-profile";
import type { LidNode } from "../src/generated/LidNode";
import { markdownToBlocks } from "../src/md-adapter";
import {
  PASS1_SHADOW_GRAPH_ARTIFACT_VERSION,
  createPass1ShadowTask,
  freezePass1ShadowTask,
  pass1ModelSlicePolicyMembers,
  pass1SourceFragmentPolicy,
  replayPass1ShadowInput,
  routePass1ShadowWorkUnits,
} from "../src/pass1-reduction";
import {
  automaticBuildExtractionPolicy,
  automaticBuildGenerationArtifactPath,
  buildSemanticArtifactEnvelopeV3,
  semanticContractFromExtractionPolicy,
  writeAutomaticBuildGenerationArtifact,
} from "../src/semantic-artifact";
import { estimateTokens } from "../src/window";

function filesIn(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const file = path.join(directory, entry.name);
    return entry.isDirectory() ? filesIn(file) : [file];
  });
}

function seedUnsectionedV1Generation() {
  const root = mkdtempSync(path.join(tmpdir(), "understand-book-unsectioned-recovery-"));
  const sourceFile = path.join(root, "unsectioned.md");
  const source = ["![](cover.jpg)", ...Array.from({ length: 100 }, (_, index) =>
    `Argument ${index + 1} depends on the evidence in the adjacent paragraphs.`)].join("\n\n");
  writeFileSync(sourceFile, source, "utf8");
  const target = resolveAutomaticBuildTarget(sourceFile, root);
  const sourceFingerprint = createHash("sha256").update(source).digest("hex");
  const profile = resolveContentProfile("technical_learning");
  const oldMembers = pass1ModelSlicePolicyMembers(profile).map(member => ({
    ...member,
    policy_generation_id: member.policy_generation_id.replace(/\.v\d+$/, ".v1"),
  }));
  const oldPolicySet = createAutomaticBuildStagePolicySet({
    target_ref: target.target_ref,
    stage: "pass1",
    members: oldMembers,
    frozen_at: "2026-10-28T00:00:00.000Z",
  });
  freezeAutomaticBuildStagePolicySet(target, oldPolicySet);
  // Reproduce the former heading-free layout: each source block was a root
  // leaf and therefore received its own numeric window ID.
  const oldLeaves: LidNode[] = markdownToBlocks(source).map((block, index) => ({
    lid: String(index + 1), path: [index + 1], kind: block.assetKind ?? "paragraph",
    span: { ...block.span }, children: [],
  }));
  const oldByLid = new Map(oldLeaves.map(node => [node.lid, node]));
  // Two persisted windows cover both numeric IDs reused after regrouping;
  // the remaining former one-leaf windows cannot be selected by the new route.
  const oldTasks = oldLeaves.slice(0, 2).map((leaf, index) => {
    const routed = routePass1ShadowWorkUnits({
      target: target.target_ref,
      source,
      source_fingerprint: sourceFingerprint,
      window: {
        id: index, leafLids: [leaf.lid], spans: [{ ...leaf.span }], overBudget: false,
        tokens: estimateTokens(source.slice(leaf.span.start, leaf.span.end)),
      },
      by_lid: oldByLid,
      content_profile: profile,
      whole_policy: automaticBuildExtractionPolicy("pass1", profile, "full"),
      fragment_policy: pass1SourceFragmentPolicy(profile),
      whole_budget: AUTOMATIC_BUILD_MODEL_INPUT_BUDGET_V1,
      fragment_budget: AUTOMATIC_BUILD_MODEL_INPUT_BUDGET_V1,
    });
    if (routed.status !== "routed" || routed.mode !== "whole") throw new Error("expected a tiny old window");
    const unit = routed.units[0];
    const task = createPass1ShadowTask({
      work_unit: unit,
      source_fingerprint: sourceFingerprint,
      policy_generation_id: "pass1-window.full.v1",
      source_unit_count: 1,
    });
    freezePass1ShadowTask(target, task);
    recordAutomaticBuildPolicyMigration({
      target, stage: "pass1", from_policy_generation_id: "pass1-legacy.v2",
      policy_set: oldPolicySet,
      current: { route: "model", descriptor: unit.descriptor, rendered_input: unit.rendered_input },
      now: "2026-10-28T00:00:00.000Z",
    });
    const artifact = buildSemanticArtifactEnvelopeV3({
      target: target.target_ref,
      stage: "pass1",
      work_unit_id: unit.descriptor.work_unit_id,
      input_hash: unit.descriptor.input_hash,
      policy_generation_id: task.policy_generation_id,
      semantic_contract: semanticContractFromExtractionPolicy(unit.descriptor.policy_fingerprint),
      provenance: { executor: "unsectioned-recovery-fixture", attempt: 1, generated_at: "2026-10-28T00:00:00.000Z" },
      payload: {
        version: PASS1_SHADOW_GRAPH_ARTIFACT_VERSION,
        window_id: index, role: "whole", source_unit_range: { start_ordinal: 0, end_ordinal_exclusive: 1 },
        evidence_lids: [leaf.lid], nodes: [], edges: [],
      },
    });
    writeAutomaticBuildGenerationArtifact(target, artifact);
    return task;
  });
  const oldFiles = new Map(filesIn(path.join(target.workspace_dir, ".build"))
    .map(file => [file, readFileSync(file)]));
  return { root, sourceFile, source, target, oldTasks, oldFiles, oldMembers, leafCount: oldLeaves.length };
}

describe("Pass1 recovery after unsectioned source regrouping", () => {
  it("prepares fresh generations without reusing or rewriting the prior numeric window artifacts", () => {
    const fixture = seedUnsectionedV1Generation();
    const oldTarget = fixture.target.target_ref;
    const recoveredTarget = resolveAutomaticBuildTarget(fixture.sourceFile, fixture.root);
    expect(recoveredTarget.target_ref).toEqual(oldTarget);
    expect(readFileSync(fixture.sourceFile, "utf8")).toBe(fixture.source);

    const prepared = prepareAutomaticBuildSnapshot(recoveredTarget, "pass1");
    expect(prepared.status).toBe("ready");
    if (prepared.status !== "ready") throw new Error("regrouped inputs must prepare in a fresh generation");
    const pass1 = prepared.value.stages.find(stage => stage.stage === "pass1")!;
    expect(pass1.closed).toBe(false);
    expect(pass1.pending_work_units).toHaveLength(2);
    expect(pass1.pending_work_units).toEqual(pass1.work_units);
    expect(pass1.quality_routing?.public_contributors).toEqual([]);
    expect(pass1.policy_set?.members.map(member => member.policy_generation_id).sort()).toEqual([
      "pass1-lid-stitch.full.v2", "pass1-source-slice.full.v2", "pass1-window.full.v2",
    ]);
    // Generation isolation changes the durable namespace only. The extraction
    // prompt, output schema and full quality contract remain the same.
    for (const member of pass1.policy_set!.members) {
      const previous = fixture.oldMembers.find(candidate => candidate.kind === member.kind)!;
      expect(member.semantic_contract).toEqual(semanticContractFromExtractionPolicy(previous.policy_fingerprint));
    }

    for (const unit of pass1.pending_work_units!) {
      const generation = pass1.generation_tasks![unit.work_unit_id];
      if (generation.kind !== "pass1") throw new Error("expected a Pass1 generation task");
      const task = generation.task;
      expect(task.policy_generation_id).toBe("pass1-window.full.v2");
      expect(task.descriptor.input_hash).not.toBe(fixture.oldTasks[Number(unit.work_unit_id)].descriptor.input_hash);
      freezePass1ShadowTask(recoveredTarget, task);
      const replay = replayPass1ShadowInput({ target: recoveredTarget, source: fixture.source, task });
      expect(replay.route.evidence_lids.length).toBeGreaterThan(1);
      expect(existsSync(automaticBuildGenerationArtifactPath(
        recoveredTarget, "pass1", task.policy_generation_id, unit.work_unit_id,
      ))).toBe(false);
      const receipt = JSON.parse(readFileSync(automaticBuildPolicyMigrationReceiptPath(
        recoveredTarget, "pass1", "pass1-legacy.v2", task.policy_generation_id, unit.work_unit_id,
      ), "utf8"));
      expect(receipt).toMatchObject({ decision: "rebuild", reason: "no_previous_artifact", current_input_hash: unit.input_hash });
    }
    expect(pass1.pending_work_units!.flatMap(unit => unit.evidence_lids)).toHaveLength(fixture.leafCount);
    expect(replayPass1ShadowInput({ target: fixture.target, source: fixture.source, task: fixture.oldTasks[0] })
      .route.evidence_lids).toEqual(["1"]);
    for (const [file, bytes] of fixture.oldFiles) expect(readFileSync(file)).toEqual(bytes);
    expect(prepareAutomaticBuildSnapshot(recoveredTarget, "pass1").status).toBe("ready");
  });
});
