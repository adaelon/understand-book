import * as fs from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { afterEach, describe, expect, it, vi } from "vitest";
import { routeIncrementalAutomaticBuildSnapshot } from "../src/automatic-build-incremental";
import { buildAutomaticBuildSnapshot, prepareAutomaticBuildSnapshot, resolveAutomaticBuildTarget,
  type AutomaticBuildSnapshot } from "../src/build-orchestrator";
import { automaticBuildGenerationArtifactPath } from "../src/semantic-artifact";
import { freezeProfileSidecarDiscourseShadowTask, freezeProfileSidecarSemanticFastPathTask,
  writeProfileSidecarDiscourseShadowCandidate, writeProfileSidecarSemanticFastPathCandidate } from "../src/profile-sidecar-reduction";
import { closeSyntheticPass1, createSyntheticRoutabilityFixture, writePass1ProductionTaskArtifact } from "./helpers/model-input-routability-fixture";
import { freezePass1ShadowTask } from "../src/pass1-reduction";
import { runAutomaticBuildCloseStage } from "../../../skills/build/automatic-build";
import { confirmedStandardBuildPlan } from "./helpers/confirmed-build-plan";
import { automaticBuildStep, createAutomaticBuildInvocation } from "../../../skills/build/automatic-build-driver";
import { automaticBuildPolicyGenerationPath } from "../src/automatic-build-policy-generation";
import { workUnitPlanDigest } from "../src/stage-work-unit";

const meter = vi.hoisted(() => ({ reads: undefined as string[] | undefined,
  planDigests: [] as string[],
  afterRead: undefined as ((file: string) => void) | undefined }));
vi.mock("../src/stage-work-unit", async importOriginal => {
  const actual = await importOriginal<typeof import("../src/stage-work-unit")>();
  return { ...actual, workUnitPlanDigest: (...args: Parameters<typeof actual.workUnitPlanDigest>) => {
    meter.planDigests.push(args[0][0]?.stage ?? "empty");
    return actual.workUnitPlanDigest(...args);
  } };
});
vi.mock("node:fs", async importOriginal => {
  const actual = await importOriginal<typeof fs>();
  return { ...actual, readFileSync: (...args: Parameters<typeof fs.readFileSync>) => {
    meter.reads?.push(String(args[0]).replaceAll("\\", "/"));
    const value = actual.readFileSync(...args);
    meter.afterRead?.(String(args[0]).replaceAll("\\", "/"));
    return value;
  } };
});
afterEach(() => { meter.reads = undefined; meter.afterRead = undefined; meter.planDigests = []; });
function ready(result: ReturnType<typeof routeIncrementalAutomaticBuildSnapshot>) {
  expect(result.status).toBe("ready");
  if (result.status !== "ready") throw new Error(result.recovery.code);
  return result.value;
}
function comparable(snapshot: AutomaticBuildSnapshot) {
  return snapshot.stages.map(({ generation_tasks, descriptor_plan_digest, ...stage }) => stage);
}
function observed(run: () => AutomaticBuildSnapshot) {
  meter.reads = [];
  const value = run();
  const reads = meter.reads;
  meter.reads = undefined;
  return { value, reads };
}

function smallFixture(root: string) {
  const sourceFile = path.join(root, "incremental.md");
  fs.writeFileSync(sourceFile, Array.from({ length: 8 }, (_, index) =>
    `# Chapter ${index + 1}\n\nThe measured value increases with its input.\n\n$$\ny_${index} = x_${index} + 1\n$$\n`).join("\n"));
  const target = resolveAutomaticBuildTarget(sourceFile, root);
  expect(prepareAutomaticBuildSnapshot(target, "pass1").status).toBe("ready");
  const pass1 = buildAutomaticBuildSnapshot(target).stages[0];
  for (const task of Object.values(pass1.generation_tasks!)) {
    if (task.kind !== "pass1") throw new Error("Pass1 required");
    freezePass1ShadowTask(target, task.task);
    writePass1ProductionTaskArtifact({ target, policy_generation_id: task.task.policy_generation_id,
      work_unit_id: task.task.descriptor.work_unit_id, marker: "The measured value increases with its input." });
  }
  runAutomaticBuildCloseStage(sourceFile, root, "pass1");
  expect(prepareAutomaticBuildSnapshot(target, "profile_sidecar").status).toBe("ready");
  return { target, sourceFile, buildPlan: confirmedStandardBuildPlan(sourceFile, root) };
}

describe("incremental profile refill", () => {
  it("uses the index in the real driver and still rejects a changed persisted policy", () => {
    const root = fs.mkdtempSync(path.join(tmpdir(), "incremental-refill-driver-"));
    const previousRegistry = process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
    process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = path.join(root, "registry");
    try {
      const fixture = smallFixture(root);
      const planFile = path.join(root, "plan.json");
      fs.writeFileSync(planFile, JSON.stringify(fixture.buildPlan));
      const invocation = createAutomaticBuildInvocation({ version: "automatic_build_invocation_create.v1",
        target_input: fixture.sourceFile, root_dir: root, build_plan_path: planFile,
        quality_profile: "full", max_parallel: 8, created_at: new Date().toISOString() });
      const request = { version: "automatic_build_step_request.v1" as const,
        invocation_ref: invocation.invocation_ref, available_agent_slots: 8 as const };
      const first = automaticBuildStep(request);
      meter.reads = [];
      meter.planDigests = [];
      const second = automaticBuildStep(request);
      const reads = meter.reads;
      meter.reads = undefined;
      expect(second.action.kind, JSON.stringify(second.action)).toBe("SPAWN_EXECUTORS");
      expect(second.build_progress).toEqual(first.build_progress);
      expect(meter.planDigests).toEqual([]);
      expect(reads.filter(file => file.includes("/artifacts/pass1/") || file.includes("/migrations/"))).toEqual([]);
      const state = buildAutomaticBuildSnapshot(fixture.target).stages[1];
      const policyFile = automaticBuildPolicyGenerationPath(fixture.target, "profile_sidecar",
        state.policy_set!.members[0].policy_generation_id);
      const policy = JSON.parse(fs.readFileSync(policyFile, "utf8"));
      policy.target_ref.input_fingerprint = "changed-policy-input";
      fs.writeFileSync(policyFile, JSON.stringify(policy));
      const changed = automaticBuildStep(request);
      expect(changed.action.kind).toBe("NEEDS_USER");
    } finally {
      if (previousRegistry === undefined) delete process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
      else process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = previousRegistry;
    }
  }, 60_000);

  it("reads only new discourse/formula results, notices deletion, and invalidates changed upstream publication", async () => {
    const root = fs.mkdtempSync(path.join(tmpdir(), "incremental-refill-"));
    const previousRegistry = process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
    try {
      const fixture = smallFixture(root);
      const options = { quality_profile: "full" as const, public_stage_closure: fixture.buildPlan.public_stage_closure,
        reuse: fixture.buildPlan.reuse };
      const first = ready(routeIncrementalAutomaticBuildSnapshot(fixture.target, options));
      const sidecar = first.stages[1];
      const digest = workUnitPlanDigest(sidecar.work_units!);
      expect(sidecar.descriptor_plan_digest).toBe(digest);
      const source = fs.readFileSync(fixture.sourceFile, "utf8");
      const committed: string[] = [];
      const commit = (id: string) => {
        const task = sidecar.generation_tasks![id];
        if (task.kind !== "profile_sidecar_fast_path") throw new Error("expected independent input");
        freezeProfileSidecarSemanticFastPathTask(fixture.target, task.task);
        const packet = task.task.packet;
        writeProfileSidecarSemanticFastPathCandidate({ target: fixture.target, source, task: task.task,
          candidate: packet.unit_kind === "profile_sidecar_discourse"
            ? { discourse_items: packet.discourse_lids.map(lid => ({ lid, mode: "informative", relations: [] })) }
            : { formula_semantics: packet.formula_lids.map(lid => ({ formula_lid: lid,
              context_lids: packet.visible_lids.filter(other => other !== lid),
              composition: { source_lid: lid, meaning: "The value increases with its input.", terms: [], evidence_lids: [lid] } })) },
          provenance: { executor: "incremental-test", attempt: 1, generated_at: new Date().toISOString() } });
      };
      for (const kind of ["profile_sidecar_discourse", "profile_sidecar_formula"]) {
        const unit = sidecar.pending_work_units!.find(unit => unit.kind === kind)!;
        commit(unit.work_unit_id);
        committed.push(unit.work_unit_id);
      }
      meter.planDigests = [];
      const changed = observed(() => ready(routeIncrementalAutomaticBuildSnapshot(fixture.target, options)));
      expect(meter.planDigests).toEqual([]);
      expect(changed.value.stages[1].descriptor_plan_digest).toBe(digest);
      expect(changed.reads.filter(file => file.includes("/artifacts/profile_sidecar/"))).toHaveLength(2);
      expect(changed.reads.filter(file => file.includes("/artifacts/pass1/") || file.includes("/migrations/"))).toEqual([]);
      expect(changed.value.stages[1].pending_tasks).toHaveLength(sidecar.pending_tasks.length - 2);
      expect(comparable(changed.value)).toEqual(comparable(buildAutomaticBuildSnapshot(fixture.target, options)));
      const unchanged = observed(() => ready(routeIncrementalAutomaticBuildSnapshot(fixture.target, options)));
      expect(unchanged.reads.filter(file => file.includes("/artifacts/") || file.includes("/migrations/"))).toEqual([]);
      // Routine refill must not allocate one string for the entire book-sized catalog.
      expect(unchanged.reads.filter(file => /\/catalog\.(json|jsonl)$/.test(file))).toEqual([]);
      expect(unchanged.value.stages[1].generation_tasks).toEqual(changed.value.stages[1].generation_tasks);
      expect(unchanged.value.stages[1].descriptor_plan_digest).toBe(digest);
      expect(meter.planDigests).toEqual([]);
      expect(workUnitPlanDigest(unchanged.value.stages[1].work_units!)).toBe(digest);
      meter.planDigests = [];
      const task = sidecar.generation_tasks![committed[0]];
      const artifact = automaticBuildGenerationArtifactPath(fixture.target, "profile_sidecar",
        task.task.policy_generation_id, committed[0]);
      fs.unlinkSync(artifact);
      const removed = ready(routeIncrementalAutomaticBuildSnapshot(fixture.target, options));
      expect(removed.stages[1].pending_tasks).toContain(committed[0]);
      expect(removed.stages[1].descriptor_plan_digest).toBe(digest);
      expect(meter.planDigests).toEqual([]);
      expect(comparable(removed)).toEqual(comparable(buildAutomaticBuildSnapshot(fixture.target, options)));

      // Another child commits after directory observation but before this refresh is saved.
      const later = sidecar.pending_work_units!.find(unit => unit.kind === "profile_sidecar_discourse"
        && !committed.includes(unit.work_unit_id))!;
      commit(committed[0]);
      meter.afterRead = file => {
        if (file !== artifact.replaceAll("\\", "/")) return;
        meter.afterRead = undefined;
        commit(later.work_unit_id);
      };
      ready(routeIncrementalAutomaticBuildSnapshot(fixture.target, options));
      const following = ready(routeIncrementalAutomaticBuildSnapshot(fixture.target, options));
      expect(following.stages[1].pending_tasks).not.toContain(later.work_unit_id);
      expect(comparable(following)).toEqual(comparable(buildAutomaticBuildSnapshot(fixture.target, options)));

      fs.unlinkSync(path.join(fixture.target.workspace_dir, "base.json"));
      const invalidated = observed(() => ready(routeIncrementalAutomaticBuildSnapshot(fixture.target, options)));
      expect(invalidated.value.stages).toHaveLength(1);
      expect(invalidated.value.stages[0].closed).toBe(false);
      expect(invalidated.reads.some(file => file.includes("/artifacts/pass1/"))).toBe(true);
    } finally {
      if (previousRegistry === undefined) delete process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
      else process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = previousRegistry;
    }
  }, 120_000);

  it("fills an older catalog's missing digest once and rebuilds a missing derived catalog", () => {
    const root = fs.mkdtempSync(path.join(tmpdir(), "incremental-refill-digest-"));
    const fixture = smallFixture(root);
    const first = ready(routeIncrementalAutomaticBuildSnapshot(fixture.target, {}));
    const expected = workUnitPlanDigest(first.stages[1].work_units!);
    const catalog = path.join(fixture.target.workspace_dir, ".build", "automatic-build", "v3",
      "incremental-profile", "inspection", "catalog.jsonl");
    const lines = fs.readFileSync(catalog, "utf8").split("\n");
    const header = JSON.parse(lines[0]);
    expect(header.descriptor_plan_digest).toBe(expected);
    expect(header.snapshot.stages[1]).not.toHaveProperty("descriptor_plan_digest");
    delete header.descriptor_plan_digest;
    lines[0] = JSON.stringify(header);
    fs.writeFileSync(catalog, lines.join("\n"));
    meter.planDigests = [];
    const filled = observed(() => ready(routeIncrementalAutomaticBuildSnapshot(fixture.target, {})));
    expect(filled.value.stages[1].descriptor_plan_digest).toBe(expected);
    expect(meter.planDigests).toEqual(["profile_sidecar"]);
    expect(filled.reads.some(file => file.includes("/artifacts/pass1/"))).toBe(false);
    meter.planDigests = [];
    expect(ready(routeIncrementalAutomaticBuildSnapshot(fixture.target, {})).stages[1].descriptor_plan_digest).toBe(expected);
    expect(meter.planDigests).toEqual([]);
    fs.unlinkSync(catalog);
    const rebuilt = ready(routeIncrementalAutomaticBuildSnapshot(fixture.target, {}));
    expect(rebuilt.stages[1].descriptor_plan_digest).toBe(expected);
    expect(meter.planDigests).toEqual(["profile_sidecar"]);
    expect(comparable(rebuilt)).toEqual(comparable(first));
  }, 60_000);

  it("refreshes a completed fragment parent and prepares its reducer without rereading Pass1", () => {
    const fixture = createSyntheticRoutabilityFixture();
    closeSyntheticPass1(fixture);
    const target = resolveAutomaticBuildTarget(fixture.source_file, fixture.root);
    expect(prepareAutomaticBuildSnapshot(target, "profile_sidecar").status).toBe("ready");
    const first = ready(routeIncrementalAutomaticBuildSnapshot(target, {}));
    expect(first.stages[1].descriptor_plan_digest).toBe(workUnitPlanDigest(first.stages[1].work_units!));
    const fragments = Object.values(first.stages[1].generation_tasks!).filter(task =>
      task.kind === "profile_sidecar_discourse" && task.task.route.role === "fragment");
    expect(fragments.length).toBeGreaterThan(1);
    for (const task of fragments) {
      if (task.kind !== "profile_sidecar_discourse" || task.task.descriptor.input_basis.kind !== "source_slices") throw new Error("fragment required");
      freezeProfileSidecarDiscourseShadowTask(target, task.task);
      const slice = task.task.descriptor.input_basis.slices[0];
      writeProfileSidecarDiscourseShadowCandidate({ target, source: fixture.source, task: task.task,
        candidate: { version: "profile_sidecar_discourse_observation.v1", parent_lid: task.task.route.parent_lid,
          source_slice_ordinal: slice.ordinal,
          mode_candidates: [{ value: "informative", confidence: 0.9 }], local_function_candidates: [],
          rhetorical_move_candidates: [], summary_fragments: ["Bounded observation."], relation_candidates: [] },
        provenance: { executor: "incremental-test", attempt: 1, generated_at: new Date().toISOString() } });
      if (task === fragments[0]) {
        meter.planDigests = [];
        const partial = ready(routeIncrementalAutomaticBuildSnapshot(target, {}));
        expect(partial.stages[1].work_units).toEqual(first.stages[1].work_units);
        expect(partial.stages[1].descriptor_plan_digest).toBe(first.stages[1].descriptor_plan_digest);
        expect(meter.planDigests).toEqual([]);
      }
    }
    const changed = observed(() => ready(routeIncrementalAutomaticBuildSnapshot(target, {})));
    expect(changed.value.stages[1].descriptor_plan_digest).toBe(workUnitPlanDigest(changed.value.stages[1].work_units!));
    expect(changed.value.stages[1].descriptor_plan_digest).not.toBe(first.stages[1].descriptor_plan_digest);
    expect(changed.reads.some(file => file.includes("/artifacts/pass1/") || file.includes("/migrations/pass1/"))).toBe(false);
    expect(changed.value.stages[1].pending_work_units!.some(unit => unit.kind === "profile_sidecar_discourse_reduce")).toBe(true);
    expect(changed.value.stages[1].preparation_required).toBe(true);
    expect(comparable(changed.value)).toEqual(comparable(buildAutomaticBuildSnapshot(target)));
    const prepared = observed(() => ready(routeIncrementalAutomaticBuildSnapshot(target, {}, "profile_sidecar")));
    expect(prepared.value.stages[1].preparation_required).toBe(false);
    expect(prepared.reads.some(file => file.includes("/artifacts/pass1/") || file.includes("/migrations/pass1/"))).toBe(false);
    expect(comparable(prepared.value)).toEqual(comparable(buildAutomaticBuildSnapshot(target)));
    expect(prepared.value.stages[1].descriptor_plan_digest).toBe(workUnitPlanDigest(prepared.value.stages[1].work_units!));
    meter.planDigests = [];
    expect(ready(routeIncrementalAutomaticBuildSnapshot(target, {})).stages[1].descriptor_plan_digest)
      .toBe(prepared.value.stages[1].descriptor_plan_digest);
    expect(meter.planDigests).toEqual([]);
  }, 60_000);
});
