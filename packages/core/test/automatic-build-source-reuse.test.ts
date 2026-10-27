import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { resolveAutomaticBuildTarget, prepareAutomaticBuildSnapshot } from "../src/build-orchestrator";
import { automaticBuildStep, createAutomaticBuildInvocation } from "../../../skills/build/automatic-build-driver";
import { confirmedStandardBuildPlan } from "./helpers/confirmed-build-plan";

describe("call-scoped automatic build source validation", () => {
  it("detects source changes between steps and resumes the same handoff after source restoration", () => {
    const root = mkdtempSync(path.join(tmpdir(), "understand-book-source-boundary-"));
    const previousRegistry = process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
    process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = path.join(root, "registry");
    try {
      const source = path.join(root, "guide.md");
      writeFileSync(source, "# Guide\n\nThe conclusion requires the original condition.\n");
      const target = resolveAutomaticBuildTarget(source, root);
      expect(prepareAutomaticBuildSnapshot(target, "pass1").status).toBe("ready");
      const plan = confirmedStandardBuildPlan(source, root);
      const planFile = path.join(root, "plan.json");
      writeFileSync(planFile, JSON.stringify(plan));
      const invocation = createAutomaticBuildInvocation({ version: "automatic_build_invocation_create.v1",
        target_input: source, root_dir: root, build_plan_path: planFile, quality_profile: "full",
        max_parallel: 1, created_at: new Date().toISOString() });
      const request = { version: "automatic_build_step_request.v1" as const,
        invocation_ref: invocation.invocation_ref, available_agent_slots: 1 as const };
      const first = automaticBuildStep(request);
      expect(first.action.kind).toBe("SPAWN_EXECUTORS");
      const activeTarget = resolveAutomaticBuildTarget(source, root, { book_id: plan.book_id });
      const original = readFileSync(activeTarget.source_path);
      writeFileSync(activeTarget.source_path, "# Guide\n\nThe changed source has a different condition.\n");
      expect(resolveAutomaticBuildTarget(source, root, { book_id: plan.book_id }).target_ref.input_fingerprint)
        .not.toBe(activeTarget.target_ref.input_fingerprint);
      const changed = automaticBuildStep(request);
      // The prepared Pass1 foundation is invalidated before the plan gate can
      // compare its source fingerprint. No old dispatch may be replayed here.
      expect(changed.action).toMatchObject({ kind: "NEEDS_USER", reason: "foundation_required" });
      writeFileSync(activeTarget.source_path, original);
      const resumed = automaticBuildStep(request);
      expect(resumed.action).toEqual(first.action);
    } finally {
      if (previousRegistry === undefined) delete process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
      else process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = previousRegistry;
      rmSync(root, { recursive: true, force: true });
    }
  }, 60_000);
});
