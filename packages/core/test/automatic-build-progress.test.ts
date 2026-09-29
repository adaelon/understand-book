import { describe, expect, it } from "vitest";
import { projectAutomaticBuildProgress } from "../src/automatic-build-progress";
import type { AutomaticBuildPreflightV2 } from "../src/automatic-build-budget";
import type { AutomaticBuildStageState } from "../src/build-orchestrator";

const plan = { public_stage_closure: ["pass1", "profile_sidecar", "book_structure"], private_artifacts: [] };
function stage(options: { closed?: boolean; pending?: string[]; extra?: boolean } = {}): AutomaticBuildStageState {
  return {
    stage: "pass1", closed: options.closed ?? false,
    pending_tasks: options.pending ?? ["second"],
    work_units: [
      { work_unit_id: "first" }, { work_unit_id: "second" },
      { work_unit_id: "skip", deterministic_skip: true },
      ...(options.extra ? [{ work_unit_id: "reducer" }] : []),
    ] as AutomaticBuildStageState["work_units"],
  };
}
function forecast(confidence: "low" | "matched"): AutomaticBuildPreflightV2 {
  return {
    stage: "pass1", cost_scope: { remaining: { work_units: 1, dispatches: 1 } },
    wall_clock: { confidence: { level: confidence, sample_count: confidence === "matched" ? 4 : 0 },
      predicted: { remaining: { p50_ms: 300_000, p95_ms: 450_000 } } },
  } as AutomaticBuildPreflightV2;
}

describe("build progress projection", () => {
  it("shows the whole selected route without inventing zero counts for future stages", () => {
    const progress = projectAutomaticBuildProgress({ plan, stages: [stage(),
      { stage: "pass2", closed: false, pending_tasks: [] }], status: "running" });
    expect(progress.current_stage).toBe("pass1");
    expect(progress.stages).toEqual([
      { stage: "pass1", status: "pending", work: { scope: "discovered", total: 3,
        eligible: 2, skipped: 1, pending: 1, committed: 1 } },
      { stage: "profile_sidecar", status: "awaiting_dependencies" },
      { stage: "book_structure", status: "awaiting_dependencies" },
    ]);
  });

  it("does not call all known tasks committed a finished stage before its reducer and publication", () => {
    const initial = projectAutomaticBuildProgress({ plan, stages: [stage({ pending: [] })], status: "running" });
    expect(initial.stages[0]).toMatchObject({ status: "pending", work: { committed: 2, pending: 0 } });
    const expanded = projectAutomaticBuildProgress({ plan,
      stages: [stage({ extra: true, pending: ["reducer"] })], status: "running" });
    expect(expanded.stages[0]).toMatchObject({ status: "pending", work: { scope: "discovered",
      eligible: 3, committed: 2, pending: 1 } });
    const published = projectAutomaticBuildProgress({ plan,
      stages: [stage({ closed: true, pending: [] })], status: "running" });
    expect(published.stages[0].status).toBe("complete");
    expect(published.current_stage).toBe("profile_sidecar");
  });

  it("keeps private work and overall completion distinct from public stage completion", () => {
    const input = { plan: { public_stage_closure: ["pass1"], private_artifacts: [{}] } as Parameters<typeof projectAutomaticBuildProgress>[0]["plan"],
      stages: [stage({ closed: true, pending: [] })] };
    const running = projectAutomaticBuildProgress({ ...input, status: "running" });
    expect(running).toMatchObject({ status: "running", private_artifact_count: 1,
      completion_condition: "all_plan_artifacts_published", stages: [{ status: "complete" }] });
    expect(running.current_stage).toBeUndefined();
    expect(projectAutomaticBuildProgress({ ...input, status: "complete" }).status).toBe("complete");
    expect(projectAutomaticBuildProgress({ ...input, status: "needs_user" }).status).toBe("needs_user");
  });

  it("hides uncalibrated timing defaults while retaining remaining batch counts", () => {
    const progress = projectAutomaticBuildProgress({ plan, stages: [stage()],
      preflight: forecast("low"), status: "running" });
    expect(progress.current_stage_forecast).toEqual({ stage: "pass1", remaining_work_units: 1,
      remaining_dispatches: 1,
      service_time: { scope: "current_stage_service_only", confidence: "low", sample_count: 0 } });
  });

  it("labels calibrated service time with its limited scope and drops stale stage forecasts", () => {
    const preflight = forecast("matched");
    const progress = projectAutomaticBuildProgress({ plan, stages: [stage()], preflight, status: "running" });
    expect(progress.current_stage_forecast?.service_time).toEqual({ scope: "current_stage_service_only",
      confidence: "matched", sample_count: 4, p50_ms: 300_000, p95_ms: 450_000 });
    expect(projectAutomaticBuildProgress({ plan, stages: [stage({ closed: true, pending: [] })],
      preflight, status: "running" }).current_stage_forecast).toBeUndefined();
    expect(projectAutomaticBuildProgress({ plan, stages: [stage()],
      preflight, status: "complete" }).current_stage_forecast).toBeUndefined();
  });
});
