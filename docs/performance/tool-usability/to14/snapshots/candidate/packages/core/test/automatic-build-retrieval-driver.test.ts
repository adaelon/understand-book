import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as orchestrator from "../src/build-orchestrator";
import { confirmedStandardBuildPlan } from "./helpers/confirmed-build-plan";
import { attachBuildPlanDigest } from "../src/build-intent";
import { fakeEmbedding } from "./fixtures/embedding-provider";
import { DSH_BUILD_EXECUTION_PROFILE_V1 as profile } from "../src/build-execution-profile";
import { automaticBuildStepWithPreparation, createAutomaticBuildInvocation, runAutomaticBuildDriverCommandWithPreparation } from "../../../skills/build/automatic-build-driver";

afterEach(() => vi.restoreAllMocks());
function fixture(dsh: boolean) {
  const root = mkdtempSync(path.join(tmpdir(), "sr5-driver-")), source = path.join(root, "guide.md"), file = path.join(root, "plan.json");
  writeFileSync(source, "# Guide\n\nA transaction groups operations.\n");
  const provider = fakeEmbedding();
  const selection = { retrieval_mode: "semantic_required" as const, provider: { location: "local" as const, identity: provider.identity },
    data_scope: "current_and_previous_formal_object_projections_and_queries" as const };
  const plan = attachBuildPlanDigest({ ...confirmedStandardBuildPlan(source, root), retrieval: { selection,
    estimate: { records: null, queries: null, basis: "unknown_until_fragments" }, budget: { max_documents: 12, max_queries: 4, max_calls: 12 } } });
  writeFileSync(file, JSON.stringify(plan));
  const common = { target_input: source, root_dir: root, build_plan_path: file, quality_profile: "full" as const,
    max_parallel: 1 as const, created_at: new Date().toISOString() };
  const invocation = createAutomaticBuildInvocation(dsh ? { ...common, version: "automatic_build_invocation_create.v2",
    execution_profile: { profile_id: profile.profile_id, profile_revision: profile.profile_revision },
    model_runtime: { version: "build_executor_model_runtime.v1", provider: "fixture", model: "fixture", reasoning_effort: null,
      max_output_tokens: 4096, context_window_tokens: 131072, safety_margin_tokens: 4096, context_source: "provider_model_metadata" },
    confirmation: { version: "dsh_build_confirmation.v1", plan_id: plan.plan_id, plan_revision: plan.revision, plan_digest: plan.plan_digest,
      root_session_id: "sr5", question_id: "sr5-plan", selected: "批准", answered_at: common.created_at },
  } : { ...common, version: "automatic_build_invocation_create.v1" });
  const request = { version: "automatic_build_step_request.v1" as const, invocation_ref: invocation.invocation_ref, available_agent_slots: 1 as const };
  return { plan, request, runtime: { selection, provider }, file };
}
describe("SR5 asynchronous host preparation", () => {
  it.each([{ dsh: false, stage: "formal_objects" }, { dsh: true, stage: "formal_objects" }, { dsh: false, stage: "book_structure" }, { dsh: true, stage: "book_structure" }] as const)("awaits preparation for $stage (DSH=$dsh)", async ({ dsh, stage: expectedStage }) => {
    const f = fixture(dsh);
    const original = orchestrator.nextPlannedAutomaticBuildAction;
    const prepareOriginal = orchestrator.prepareAutomaticBuildSnapshot;
    let ready = false;
    vi.spyOn(orchestrator, "nextPlannedAutomaticBuildAction").mockImplementation((...args) => ready ? original(...args)
      : { kind: "needs_user", reason: "preparation_required", stage: expectedStage, message: "prepare" });
    const prep = vi.spyOn(orchestrator, "prepareAutomaticBuildSnapshot").mockImplementation(((target: orchestrator.AutomaticBuildTarget, stage: Parameters<typeof prepareOriginal>[1], options: any) => {
      if (!options?.authorization) return prepareOriginal(target, stage, options);
      expect(stage).toBe(expectedStage);
      expect(options.authorization.plan.plan_digest).toBe(f.plan.plan_digest);
      expect(options.authorization.runtime).toBe(f.runtime);
      return Promise.resolve().then(() => { ready = true; return orchestrator.routeAutomaticBuildSnapshot(target, options); });
    }) as typeof prepareOriginal);
    expect((await automaticBuildStepWithPreparation(f.request, [], { runtime: f.runtime })).action.kind).toBe("SPAWN_EXECUTORS");
    expect(prep.mock.calls.filter(args => (args[2] as any)?.authorization)).toHaveLength(1); expect(f.runtime.provider.calls).toEqual([]);
  });
  it("maps provider failure without a semantic attempt and awaits refill and DSH control responses", async () => {
    const f = fixture(false);
    vi.spyOn(orchestrator, "nextPlannedAutomaticBuildAction").mockReturnValue({ kind: "needs_user", reason: "preparation_required", stage: "formal_objects", message: "prepare" });
    vi.spyOn(orchestrator, "prepareAutomaticBuildSnapshot").mockResolvedValue({ status: "needs_user", action: {
      kind: "needs_user", reason: "retrieval_provider_failed", stage: "formal_objects", message: "offline" } } as never);
    const refill = await runAutomaticBuildDriverCommandWithPreparation({ version: "automatic_build_refill_request.v1", invocation_ref: f.request.invocation_ref,
      capacity_limit: 1, live_by_slot: {}, completed_refs: [], terminal_children: [] });
    expect(refill).toMatchObject({ step: { action: { kind: "NEEDS_USER", reason: "retrieval_unavailable" } }, ready_executors: [] });
    vi.restoreAllMocks();
    const d = fixture(true), owner = { version: "dsh_build_controller.v1", invocation_ref: d.request.invocation_ref,
      root_session_id: "sr5", controller_id: "sr5", owner_pid: process.pid };
    await runAutomaticBuildDriverCommandWithPreparation({ ...owner, action: "acquire" });
    vi.spyOn(orchestrator, "nextPlannedAutomaticBuildAction").mockReturnValue({ kind: "needs_user", reason: "preparation_required", stage: "formal_objects", message: "prepare" });
    vi.spyOn(orchestrator, "prepareAutomaticBuildSnapshot").mockResolvedValue({ status: "needs_user", action: {
      kind: "needs_user", reason: "retrieval_cancelled", stage: "formal_objects", message: "cancelled" } } as never);
    expect(await runAutomaticBuildDriverCommandWithPreparation({ ...owner, action: "step", available_agent_slots: 1 }))
      .toMatchObject({ action: { kind: "NEEDS_USER", reason: "retrieval_cancelled" } });
    await runAutomaticBuildDriverCommandWithPreparation({ ...owner, action: "release" });
  });
});
