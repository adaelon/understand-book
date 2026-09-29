import type { AutomaticBuildPreflightV2 } from "./automatic-build-budget";
import type { BuildPlanV1 } from "./build-intent";
import type { AutomaticBuildStageState } from "./build-orchestrator";

export interface AutomaticBuildProgressV1 {
  version: "automatic_build_progress.v1";
  status: "running" | "needs_user" | "complete";
  completion_condition: "all_plan_artifacts_published";
  private_artifact_count: number;
  current_stage?: string;
  stages: Array<{
    stage: string;
    status: "complete" | "pending" | "awaiting_dependencies";
    work?: AutomaticBuildPreflightV2["work_units"] & { scope: "discovered" };
  }>;
  current_stage_forecast?: {
    stage: string;
    remaining_work_units: number;
    remaining_dispatches: number;
    service_time: {
      scope: "current_stage_service_only";
      confidence: "matched" | "low";
      sample_count: number;
      p50_ms?: number;
      p95_ms?: number;
    };
  };
}

/** Project counts from the existing read; never expose task identities or semantic material. */
export function projectAutomaticBuildProgress(input: {
  plan: Pick<BuildPlanV1, "public_stage_closure" | "private_artifacts">;
  stages: readonly AutomaticBuildStageState[];
  preflight?: AutomaticBuildPreflightV2 | null;
  status: AutomaticBuildProgressV1["status"];
}): AutomaticBuildProgressV1 {
  const stages = input.plan.public_stage_closure.map((stage): AutomaticBuildProgressV1["stages"][number] => {
    const state = input.stages.find(item => item.stage === stage);
    if (!state) return { stage, status: "awaiting_dependencies" };
    const result: AutomaticBuildProgressV1["stages"][number] = {
      stage, status: state.closed ? "complete" : "pending",
    };
    if (state.work_units) {
      const eligible = state.work_units.filter(unit => !unit.deterministic_skip);
      const pendingIds = new Set(state.pending_tasks);
      const pending = eligible.filter(unit => pendingIds.has(unit.work_unit_id)).length;
      result.work = {
        scope: "discovered", total: state.work_units.length, eligible: eligible.length,
        skipped: state.work_units.length - eligible.length,
        pending, committed: eligible.length - pending,
      };
    }
    return result;
  });
  // Public stages may be finished while private artifacts or publication still need work.
  const current = input.status === "complete" ? undefined : stages.find(stage => stage.status !== "complete");
  const progress: AutomaticBuildProgressV1 = {
    version: "automatic_build_progress.v1",
    status: input.status,
    completion_condition: "all_plan_artifacts_published",
    private_artifact_count: input.plan.private_artifacts.length,
    stages,
    ...(current ? { current_stage: current.stage } : {}),
  };
  const preflight = input.preflight;
  if (preflight && current?.stage === preflight.stage && current.status === "pending") {
    const confidence = preflight.wall_clock.confidence;
    const remaining = preflight.wall_clock.predicted.remaining;
    progress.current_stage_forecast = {
      stage: preflight.stage,
      remaining_work_units: preflight.cost_scope.remaining.work_units,
      remaining_dispatches: preflight.cost_scope.remaining.dispatches,
      service_time: {
        scope: "current_stage_service_only",
        confidence: confidence.level,
        sample_count: confidence.sample_count,
        // The uncalibrated five-minute fallback is a scheduling input, not a user ETA.
        ...(confidence.level === "matched" ? { p50_ms: remaining.p50_ms, p95_ms: remaining.p95_ms } : {}),
      },
    };
  }
  return progress;
}
