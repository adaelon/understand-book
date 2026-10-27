import * as fs from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { setImmediate } from "node:timers/promises";
import { describe, expect, it, vi } from "vitest";
import { prepareAutomaticBuildSnapshot, resolveAutomaticBuildTarget } from "../src/build-orchestrator";
import { automaticBuildNext, automaticBuildPlan } from "../../../skills/build/automatic-build";
import { automaticBuildStep, createAutomaticBuildInvocation } from "../../../skills/build/automatic-build-driver";
import { confirmedStandardBuildPlan } from "./helpers/confirmed-build-plan";

interface Observation {
  source_paths: Set<string>;
  target_resolutions: number;
  source_reads: number;
  source_read_bytes: number;
  all_reads: number;
  all_read_bytes: number;
}
const meter = vi.hoisted(() => ({ active: undefined as Observation | undefined }));

// All operations remain real; the same wrappers run for baseline and candidate.
vi.mock("node:fs", async (importOriginal) => {
  const actual = await importOriginal<typeof fs>();
  return { ...actual, readFileSync: (...args: Parameters<typeof fs.readFileSync>) => {
    const value = actual.readFileSync(...args);
    const active = meter.active;
    if (active) {
      const bytes = typeof value === "string" ? Buffer.byteLength(value) : value.byteLength;
      active.all_reads += 1;
      active.all_read_bytes += bytes;
      if (active.source_paths.has(path.resolve(String(args[0])))) {
        active.source_reads += 1;
        active.source_read_bytes += bytes;
      }
    }
    return value;
  } };
});
vi.mock("../src/build-orchestrator", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/build-orchestrator")>();
  return { ...actual, resolveAutomaticBuildTarget: (...args: Parameters<typeof actual.resolveAutomaticBuildTarget>) => {
    if (meter.active) meter.active.target_resolutions += 1;
    return actual.resolveAutomaticBuildTarget(...args);
  } };
});

function diskImage(root: string): Map<string, Buffer> {
  const result = new Map<string, Buffer>();
  for (const entry of fs.readdirSync(root, { recursive: true, withFileTypes: true })) {
    if (!entry.isFile()) continue;
    const absolute = path.join(entry.parentPath, entry.name);
    result.set(path.relative(root, absolute), fs.readFileSync(absolute));
  }
  return result;
}

function restore(root: string, image: Map<string, Buffer>) {
  for (const relative of diskImage(root).keys()) {
    if (!image.has(relative)) fs.unlinkSync(path.join(root, relative));
  }
  for (const [relative, bytes] of image) {
    const file = path.join(root, relative);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, bytes);
  }
}

const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

describe("invocation source reuse performance", () => {
  it("measures real build.step with identical source, accepted work and dispatch identity", async () => {
    const priorRegistry = process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
    const root = fs.mkdtempSync(path.join(tmpdir(), "understand-book-source-reuse-"));
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2026-10-23T09:00:00.000Z"));
    try {
      process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = path.join(root, "registry");
      const sourceFile = path.join(root, "source-reuse.md");
      const chapters = Number(process.env.UNDERSTAND_BOOK_SOURCE_REUSE_CHAPTERS ?? 16);
      const paragraph = "The method follows from the stated condition. A larger input produces a larger output. ".repeat(280);
      const source = Array.from({ length: chapters }, (_, index) => `# Chapter ${index + 1}\n\n${paragraph}\n\n`).join("");
      fs.writeFileSync(sourceFile, source);
      const target = resolveAutomaticBuildTarget(sourceFile, root);
      const prepared = prepareAutomaticBuildSnapshot(target, "pass1");
      expect(prepared.status).toBe("ready");
      const buildPlan = confirmedStandardBuildPlan(sourceFile, root);
      const planPath = path.join(root, "plan.json");
      fs.writeFileSync(planPath, JSON.stringify(buildPlan));
      const invocation = createAutomaticBuildInvocation({
        version: "automatic_build_invocation_create.v1", target_input: sourceFile, root_dir: root,
        build_plan_path: planPath, quality_profile: "full", max_parallel: 3,
        created_at: new Date().toISOString(),
      });
      const plan = automaticBuildPlan(sourceFile, root, { requested_workers: 3, available_agent_slots: 3, build_plan: buildPlan });
      const dispatched = automaticBuildNext(sourceFile, root, 3, { available_agent_slots: 3,
        accepted_plan_digest: plan.preflight?.descriptor_plan_digest, executor_dispatches: true, build_plan: buildPlan });
      if (!("dispatches" in dispatched.action) || !dispatched.action.dispatches?.length) {
        throw new Error(`expected prepared dispatches: ${dispatched.action.kind}`);
      }
      const expectedRefs = dispatched.action.dispatches.map(dispatch => dispatch.opaque_handoff_ref).sort();
      const request = { version: "automatic_build_step_request.v1" as const,
        invocation_ref: invocation.invocation_ref, available_agent_slots: 3 as const };
      const saved = diskImage(root);
      const samples: Array<{ sample: string; elapsed_ms: number; target_resolutions: number;
        source_reads: number; source_read_bytes: number; all_reads: number; all_read_bytes: number }> = [];
      let expectedResponse: ReturnType<typeof automaticBuildStep> | undefined;
      for (let index = 0; index < 4; index += 1) {
        restore(root, saved);
        await setImmediate();
        const active: Observation = { source_paths: new Set([path.resolve(sourceFile), path.resolve(target.source_path!),
          path.join(target.workspace_dir, "source.txt")]), target_resolutions: 0,
          source_reads: 0, source_read_bytes: 0, all_reads: 0, all_read_bytes: 0 };
        const started = performance.now();
        meter.active = active;
        let response: ReturnType<typeof automaticBuildStep>;
        try { response = automaticBuildStep(request); } finally { meter.active = undefined; }
        const elapsed_ms = performance.now() - started;
        expect(response.action.kind).toBe("SPAWN_EXECUTORS");
        if (response.action.kind === "SPAWN_EXECUTORS") {
          expect(response.action.executors.map(executor => executor.opaque_handoff_ref).sort()).toEqual(expectedRefs);
        }
        if (expectedResponse) expect(response).toEqual(expectedResponse);
        else expectedResponse = response;
        const { source_paths: _, ...counts } = active;
        samples.push({ sample: index === 0 ? "initial" : `warm-${index}`, elapsed_ms, ...counts });
      }
      const report = { version: "automatic_build_source_reuse_performance.v1",
        runtime: { node: process.version, platform: process.platform, architecture: process.arch },
        fixture: { chapters, source_bytes: Buffer.byteLength(source), dispatches: expectedRefs.length,
          pending_work_units: plan.snapshot.stages[0]?.pending_work_units?.length,
          model: "none; real local routing and persistent dispatch replay" },
        samples,
        warm_median_ms: median(samples.slice(1).map(sample => sample.elapsed_ms)),
        behavior: { same_action_and_handoff_identity: true, same_response_for_all_samples: true },
        limitations: ["Initial means the first measured step after fixture setup; OS file cache is not flushed.",
          "All samples restore the same on-disk files and fixed Date; process caches may remain warm.",
          "Measures current TypeScript source in-process; excludes model generation and host transport."] };
      const output = process.env.UNDERSTAND_BOOK_SOURCE_REUSE_REPORT;
      if (output) fs.writeFileSync(path.resolve(output), `${JSON.stringify(report, null, 2)}\n`);
      console.info("source reuse benchmark", JSON.stringify(report));
    } finally {
      meter.active = undefined;
      vi.useRealTimers();
      if (priorRegistry === undefined) delete process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
      else process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = priorRegistry;
      fs.rmSync(root, { recursive: true, force: true });
    }
  }, 300_000);
});
