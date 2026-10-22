import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { setImmediate as yieldToRunner } from "node:timers/promises";
import { spawnSync } from "node:child_process";
import { describe, expect, it, vi } from "vitest";
import {
  buildAutomaticBuildSnapshot, prepareAutomaticBuildSnapshot, resolveAutomaticBuildTarget,
} from "../src/build-orchestrator";
import { freezePass1ShadowTask } from "../src/pass1-reduction";
import { readAutomaticBuildAttemptSnapshot } from "../src/automatic-build-task-store";
import { runAutomaticBuildCloseStage } from "../../../skills/build/automatic-build";
import { createAutomaticBuildInvocation, runAutomaticBuildDriverCommand } from "../../../skills/build/automatic-build-driver";
import { createBuildExecutorMcpSession } from "../../../skills/build/build-executor-mcp";
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 } from "../src/build-executor-connection-capability";
import { readProfileSidecarProductionTask } from "../src/profile-sidecar-reduction";
import { writePass1ProductionTaskArtifact } from "./helpers/model-input-routability-fixture";
import { confirmedStandardBuildPlan } from "./helpers/confirmed-build-plan";
import type { AutomaticBuildExecutorSessionResponseV3 } from "../src/automatic-build-executor-session";
import type { AutomaticBuildRefillResponseV1 } from "../../../skills/build/automatic-build-refill";
import { executorMcpInvocation, JsonLineMcpClient } from "../../../apps/desktop/scripts/smoke-t7-executor-release";

const repo = path.resolve(import.meta.dirname, "../../..");
const evidenceRoot = path.join(repo, "tmp/profile-discourse-preparation-20261020");
const reportPath = process.env.DISCOURSE_HANDOFF_REPORT;
const mode = process.env.DISCOURSE_HANDOFF_MODE ?? "prepared";
const compiled = process.env.DISCOURSE_HANDOFF_EXE;
const samples = reportPath ? 4 : 1;
const clock = new Date("2026-10-20T03:00:00.000Z");

function connection(root: string, index: number) {
  const client = compiled
    ? new JsonLineMcpClient(executorMcpInvocation(compiled, root), path.join(root, "registry"), compiled) : undefined;
  const initialized = client?.request("initialize", { protocolVersion: "2025-06-18", capabilities: {},
    clientInfo: { name: "discourse-handoff-benchmark", version: "1" } });
  const session = client ? undefined : createBuildExecutorMcpSession({
    bootstrap_version: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.version,
    protocol_generation: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.session_protocol,
    session_private_root: path.join(root, `connection-${index}`),
  });
  let sequence = 0;
  const call = async (name: string, args: unknown) => {
    await initialized;
    const params = { name, arguments: args };
    const rpc = (client ? await client.request("tools/call", params)
      : session!.handle_message({ jsonrpc: "2.0", id: ++sequence, method: "tools/call", params })) as {
        result: { isError: boolean; content: { text: string }[] };
      };
    expect(rpc.result?.isError, rpc.result?.content?.[0]?.text).toBe(false);
    return (JSON.parse(rpc.result.content[0].text) as AutomaticBuildExecutorSessionResponseV3).action;
  };
  const start = async (ref: string) => {
    const opened = await call("executor.open", { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: ref });
    if (opened.kind !== "DELIVER_INPUT") throw new Error(`open: ${opened.kind}`);
    const batch = await call("executor.input.next", opened.next_request);
    if (batch.kind !== "INPUT_BATCH" || !batch.batch.final_for_generation) throw new Error("expected complete input batch");
    const started = await call("executor.generation.start", {
      version: "automatic_build_executor_generation_start_request.v3", opaque_session_ref: batch.batch.opaque_session_ref,
      generation_input_ref: batch.batch.generation_input_ref, confirmed_through_ordinal: batch.batch.last_ordinal,
    });
    if (started.kind !== "GENERATE") throw new Error(`start: ${started.kind}`);
    return started;
  };
  return { call, start, close: async () => { await client?.close(); } };
}

function driverCommand(request: { version: string; [key: string]: unknown }) {
  if (!compiled) return runAutomaticBuildDriverCommand(request);
  const result = spawnSync(compiled, [request.version === "automatic_build_refill_request.v1" ? "build.refill" : "build.step"],
    { cwd: repo, encoding: "utf8", windowsHide: true, input: JSON.stringify(request), timeout: 60_000 });
  expect(result.status, result.stderr).toBe(0);
  return JSON.parse(result.stdout);
}

async function fixture(root: string, kind: "ordinary" | "fragment") {
  mkdirSync(root, { recursive: true });
  const sourceFile = path.join(root, "source.md");
  const paragraph = kind === "ordinary"
    ? "Independent explanation: increasing the input increases the output. ".repeat(12)
    : "This independent passage explains a bounded source fragment and its evidence. ".repeat(540);
  writeFileSync(sourceFile, Array.from({ length: kind === "ordinary" ? (reportPath ? 96 : 16) : (reportPath ? 12 : 2) },
    (_, i) => `# Section ${i + 1}\n\n${paragraph}\n\n`).join(""));
  const target = resolveAutomaticBuildTarget(sourceFile, root);
  for (let round = 0; round < 8; round++) {
    const prepared = prepareAutomaticBuildSnapshot(target, "pass1");
    if (prepared.status !== "ready") throw new Error("Pass1 fixture preparation failed");
    const stage = prepared.value.stages[0];
    if (!stage.pending_work_units?.length) break;
    for (const unit of stage.pending_work_units) {
      const task = stage.generation_tasks?.[unit.work_unit_id];
      if (task?.kind !== "pass1") throw new Error("Pass1 fixture task missing");
      freezePass1ShadowTask(target, task.task);
      writePass1ProductionTaskArtifact({ target, policy_generation_id: task.task.policy_generation_id,
        work_unit_id: unit.work_unit_id, marker: `Synthetic ${unit.work_unit_id}` });
      await yieldToRunner();
    }
  }
  runAutomaticBuildCloseStage(sourceFile, root, "pass1");
  if (prepareAutomaticBuildSnapshot(target, "profile_sidecar").status !== "ready") throw new Error("sidecar preparation failed");
  const plan = confirmedStandardBuildPlan(sourceFile, root);
  const planFile = path.join(root, "plan.json");
  writeFileSync(planFile, JSON.stringify(plan));
  const invocation = createAutomaticBuildInvocation({ version: "automatic_build_invocation_create.v1",
    target_input: sourceFile, root_dir: root, build_plan_path: planFile, quality_profile: "full", max_parallel: 3,
    created_at: clock.toISOString() });
  writeFileSync(path.join(root, "fixture.json"), JSON.stringify({ sourceFile, invocation }));
}

function candidateForTask(target: ReturnType<typeof resolveAutomaticBuildTarget>, unit: {
  work_unit_id: string; policy_generation_id: string;
}) {
  const task = readProfileSidecarProductionTask(target, unit.policy_generation_id, unit.work_unit_id);
  if (task.version === "profile_sidecar_semantic_fast_path_task.v1") {
    return { discourse_items: task.packet.visible_lids.map(lid => ({ lid, mode: "informative", relations: [] })) };
  }
  if (task.route.role !== "fragment" || task.descriptor.input_basis.kind !== "source_slices") throw new Error("expected source fragment");
  const slice = task.descriptor.input_basis.slices[0];
  return {
    version: "profile_sidecar_discourse_observation.v1", parent_lid: task.route.parent_lid,
    source_slice_ordinal: slice.ordinal, core_sha256: slice.core_sha256,
    mode_candidates: [{ value: "informative", confidence: 0.9 }],
    local_function_candidates: [{ value: "explanation", confidence: 0.8 }],
    rhetorical_move_candidates: [{ value: "concept_elaboration", confidence: 0.8 }],
    summary_fragments: ["The input determines the output."], relation_candidates: [],
  };
}

function frozenTaskForSession(root: string, sessionRef: string) {
  // Existing private session records bind a generation to its exact task.
  const directory = path.join(root, "registry", "executor-task-sessions");
  const records = readdirSync(directory).filter(file => file.endsWith(".json"))
    .map(file => JSON.parse(readFileSync(path.join(directory, file), "utf8")));
  const session = records.find(record => record.opaque_session_ref === sessionRef);
  if (!session) throw new Error("task session not found");
  return { work_unit_id: session.work_unit_id, policy_generation_id: session.policy_generation_id };
}

describe("profile discourse preparation and refill", () => {
  it.each(["ordinary", "fragment"] as const)("measures %s submit → refill → next generation with three live slots", async kind => {
    const previousRegistry = process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
    if (!compiled) {
      vi.useFakeTimers({ toFake: ["Date"] });
      vi.setSystemTime(clock);
    }
    const root = reportPath ? path.join(evidenceRoot, `${kind}-work`)
      : mkdtempSync(path.join(tmpdir(), `understand-book-discourse-${kind}-`));
    const saved = reportPath ? path.join(evidenceRoot, `${kind}-initial`) : `${root}-initial`;
    const connections: ReturnType<typeof connection>[] = [];
    process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = path.join(root, "registry");
    try {
      // Both roots are test-owned; performance runs restore the same absolute paths.
      if (!existsSync(saved)) {
        await fixture(root, kind);
        cpSync(root, saved, { recursive: true });
      }
      await yieldToRunner();
      const results = [];
      for (let sample = 0; sample < samples; sample++) {
        rmSync(root, { recursive: true, force: true });
        cpSync(saved, root, { recursive: true });
        await yieldToRunner();
        const { sourceFile, invocation } = JSON.parse(readFileSync(path.join(root, "fixture.json"), "utf8"));
        const target = resolveAutomaticBuildTarget(sourceFile, root);
        const snapshot = buildAutomaticBuildSnapshot(target);
        const sidecar = snapshot.stages.find(s => s.stage === "profile_sidecar")!;
        const before = sidecar.generation_tasks;
        const step = driverCommand({ version: "automatic_build_step_request.v1",
          invocation_ref: invocation.invocation_ref, available_agent_slots: 3 }) as {
            action: { kind: string; executors: Array<{ dispatch_slot_ref: string; opaque_handoff_ref: string }> };
          };
        expect(step.action.kind, JSON.stringify(step)).toBe("SPAWN_EXECUTORS");
        expect(step.action.executors).toHaveLength(3);
        const live: Record<string, { child: string; opaque_handoff_ref: string }> = {};
        const children = [];
        for (const [index, launch] of step.action.executors.entries()) {
          const child = connection(root, index);
          connections.push(child);
          live[launch.dispatch_slot_ref] = { child: `child-${index}`, opaque_handoff_ref: launch.opaque_handoff_ref };
          children.push({ ...child, started: await child.start(launch.opaque_handoff_ref) });
        }
        let prepareMs = 0;
        if (mode === "prepared") {
          const attemptsBefore = readAutomaticBuildAttemptSnapshot(target);
          const start = performance.now();
          const processResult = spawnSync(compiled ?? process.execPath, compiled ? ["build.prepare"] : ["--import", "tsx",
            path.join(repo, "skills/build/automatic-build-driver.ts")], {
            cwd: repo, encoding: "utf8", windowsHide: true, input: JSON.stringify({ version: "automatic_build_prepare_request.v1",
              invocation_ref: invocation.invocation_ref }), timeout: 60_000,
          });
          prepareMs = performance.now() - start;
          expect(processResult.status, processResult.stderr).toBe(0);
          const prepared = JSON.parse(processResult.stdout);
          expect(prepared).toMatchObject({ version: "automatic_build_prepare.v1", status: "prepared" });
          expect(readAutomaticBuildAttemptSnapshot(target)).toEqual(attemptsBefore);
        }
        const current = children[2];
        const unit = frozenTaskForSession(root, current.started.opaque_session_ref);
        unit.policy_generation_id = before![unit.work_unit_id].task.policy_generation_id;
        const candidate = candidateForTask(target, unit);
        const start = performance.now();
        const committed = await current.call("executor.submit_candidate", {
          version: "automatic_build_executor_candidate_submit.v3", opaque_session_ref: current.started.opaque_session_ref,
          candidate_sink_ref: current.started.candidate_sink_ref, candidate,
        });
        const submitted = performance.now();
        expect(committed).toMatchObject({ kind: "DONE", status: "committed" });
        const refill = driverCommand({ version: "automatic_build_refill_request.v1",
          invocation_ref: invocation.invocation_ref, capacity_limit: 3, live_by_slot: live,
          completed_refs: [], terminal_children: ["child-2"] }) as AutomaticBuildRefillResponseV1;
        const refilled = performance.now();
        expect(refill.available_agent_slots).toBe(1);
        expect(Object.values(refill.live_by_slot).map(v => v.child).sort()).toEqual(["child-0", "child-1"]);
        expect(refill.ready_executors).toHaveLength(1);
        const next = connection(root, 3);
        connections.push(next);
        await next.start(refill.ready_executors[0].opaque_handoff_ref);
        const started = performance.now();
        const receipt = readAutomaticBuildAttemptSnapshot(target).stages.profile_sidecar?.[unit.work_unit_id];
        expect(receipt).toMatchObject({ semantic_attempt: 1, failures: 0, submit_revision: 1 });
        const after = buildAutomaticBuildSnapshot(target).stages.find(s => s.stage === "profile_sidecar")!;
        expect(after.pending_tasks).not.toContain(unit.work_unit_id);
        expect(after.generation_tasks).toEqual(before);
        results.push({ sample: sample ? `warm-${sample}` : "initial", prepare_ms: prepareMs,
          submit_ms: submitted - start, refill_ms: refilled - submitted, next_start_ms: started - refilled,
          handoff_ms: started - start, units: sidecar.work_units?.length, semantic_attempts: 1, durable_commits: 1 });
        for (const child of connections.splice(0)) await child.close();
        await yieldToRunner();
      }
      if (reportPath) {
        mkdirSync(path.dirname(reportPath), { recursive: true });
        const report = existsSync(reportPath) ? JSON.parse(readFileSync(reportPath, "utf8")) : {};
        report[kind] = { mode, runtime: compiled ? "compiled Bun sidecar + real stdio MCP" : process.version,
          ...(compiled ? { executable: compiled } : {}), samples: results,
          scope: "Real stores, writer, driver refill and MCP handlers; deterministic candidates; excludes native spawn and model generation." };
        writeFileSync(reportPath, JSON.stringify(report, null, 2) + "\n");
      }
    } finally {
      for (const child of connections) await child.close();
      vi.useRealTimers();
      if (previousRegistry === undefined) delete process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
      else process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = previousRegistry;
      if (!reportPath) {
        rmSync(root, { recursive: true, force: true });
        rmSync(saved, { recursive: true, force: true });
      }
    }
  }, 180_000);
});
