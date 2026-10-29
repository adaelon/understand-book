import { mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { setImmediate as yieldToRunner } from "node:timers/promises";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { automaticBuildWorkerStep, createAutomaticBuildInvocation } from "../../../skills/build/automatic-build-driver";
import { createBuildExecutorMcpSession } from "../../../skills/build/build-executor-mcp";
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 } from "../src/build-executor-connection-capability";
import { readAutomaticBuildWorker } from "../src/automatic-build-worker";
import { readAutomaticBuildAttemptSnapshot } from "../src/automatic-build-task-store";
import { resolveAutomaticBuildTarget } from "../src/build-orchestrator";
import type { AutomaticBuildExecutorSessionResponseV3 } from "../src/automatic-build-executor-session";
import { confirmedStandardBuildPlan } from "./helpers/confirmed-build-plan";
import { canonicalAutomaticBuildJson } from "../src/automatic-build-protocol";

const priorRegistry = process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
const roots: string[] = [];
afterEach(() => {
  if (priorRegistry === undefined) delete process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT;
  else process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = priorRegistry;
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

function connection(registry: string) {
  const mcp = createBuildExecutorMcpSession({
    bootstrap_version: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.version,
    protocol_generation: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.session_protocol,
    session_private_root: registry,
  });
  let ordinal = 0;
  return {
    mcp,
    async call(name: string, args: unknown): Promise<AutomaticBuildExecutorSessionResponseV3> {
      const rpc = await mcp.handle_message({ jsonrpc: "2.0", id: ++ordinal, method: "tools/call",
        params: { name, arguments: args } }) as { result: { isError?: boolean; content: { text: string }[] } };
      expect(rpc.result.isError, rpc.result.content[0]?.text).toBe(false);
      return JSON.parse(rpc.result.content[0]!.text);
    },
  };
}

describe("persistent worker executor connection", () => {
  it("crosses dispatches on two unchanged MCP connections and does not charge submit replay twice", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "understand-book-worker-executor-"));
    roots.push(root);
    const registry = path.join(root, "registry");
    process.env.UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = registry;
    const source = path.join(root, "book.md");
    writeFileSync(source, `# Worker continuity\n\n${Array.from({ length: 700 }, (_, index) =>
      `Paragraph ${index + 1} explains a stable independent observation about how shared work continues after each completed assignment.`).join("\n\n")}\n`);
    const buildPlan = confirmedStandardBuildPlan(source, root);
    const planPath = path.join(root, "plan.json");
    writeFileSync(planPath, JSON.stringify(buildPlan));
    const invocation = createAutomaticBuildInvocation({ version: "automatic_build_invocation_create.v1",
      target_input: source, root_dir: root, build_plan_path: planPath, quality_profile: "full", max_parallel: 8,
      created_at: new Date().toISOString() });
    const step = await automaticBuildWorkerStep({ version: "automatic_build_step_request.v1",
      invocation_ref: invocation.invocation_ref, available_agent_slots: 2 });
    expect(step.action.kind).toBe("SPAWN_EXECUTORS");
    if (step.action.kind !== "SPAWN_EXECUTORS") throw new Error(JSON.stringify(step));
    expect(step.action.executors).toHaveLength(2);
    const workers = await Promise.all(step.action.executors.map(async item => {
      const transport = connection(registry);
      return { ...transport, rootRef: item.opaque_handoff_ref, generated: 0, waits: 0,
        response: await transport.call("executor.open", { version: "automatic_build_executor_open_request.v3",
          opaque_handoff_ref: item.opaque_handoff_ref }) };
    }));
    const taskIds = new Set<string>();
    // Run each connection independently: a waiting tool must not prevent its
    // sibling from generating the result that will wake it.
    await Promise.all(workers.map(async worker => {
      for (let tick = 0; tick < 400 && worker.response.action.kind !== "DONE"; tick++) {
        const action = worker.response.action;
        if (action.kind === "DELIVER_INPUT") {
          worker.response = await worker.call("executor.input.next", action.next_request);
        } else if (action.kind === "INPUT_BATCH") {
          worker.response = await worker.call(action.batch.final_for_generation ? "executor.generation.start" : "executor.input.next", {
            version: action.batch.final_for_generation ? "automatic_build_executor_generation_start_request.v3"
              : "automatic_build_executor_input_next_request.v4",
            opaque_session_ref: action.batch.opaque_session_ref, generation_input_ref: action.batch.generation_input_ref,
            ...(action.batch.final_for_generation ? { confirmed_through_ordinal: action.batch.last_ordinal }
              : { ack_through_ordinal: action.batch.last_ordinal }),
          });
        } else if (action.kind === "GENERATE") {
          worker.generated++;
          if (!("work_unit_id" in action.output_contract)) throw new Error("expected book candidate contract");
          expect(taskIds.has(action.output_contract.work_unit_id)).toBe(false);
          taskIds.add(action.output_contract.work_unit_id);
          const submit = { version: "automatic_build_executor_candidate_submit.v3",
            opaque_session_ref: action.opaque_session_ref, candidate_sink_ref: action.candidate_sink_ref,
            candidate: { nodes: [], edges: [] } };
          worker.response = await worker.call("executor.submit_candidate", submit);
          const before = readAutomaticBuildWorker(worker.rootRef)!;
          if (worker.response.action.kind !== "WAIT") {
            const replay = await worker.call("executor.submit_candidate", submit);
            expect(replay).toEqual(worker.response);
            const after = readAutomaticBuildWorker(worker.rootRef)!;
            expect(after.assignments).toEqual(before.assignments);
            expect(after.submitted_output_bytes).toBe(before.submitted_output_bytes);
          }
        } else {
          worker.waits++;
          expect(worker.mcp.is_terminal()).toBe(false);
          worker.response = await worker.call("executor.open", { version: "automatic_build_executor_open_request.v3",
            opaque_handoff_ref: worker.rootRef });
        }
        await yieldToRunner();
      }
    }));
    expect(taskIds.size).toBeGreaterThan(8);
    expect(workers.some(worker => readAutomaticBuildWorker(worker.rootRef)!.assignments.length > 1)).toBe(true);
    for (const worker of workers) {
      expect(worker.response.action).toEqual({ kind: "DONE", status: "committed" });
      const record = readAutomaticBuildWorker(worker.rootRef)!;
      // Assignment sizes and ready work differ between workers. Fair division
      // is not required; both original connections must finish normally.
      expect(["stage_complete", "context_rotation"]).toContain(record.terminal_reason);
      expect(record.delivered_input_bytes).toBeGreaterThan(0);
      expect(record.submitted_output_bytes).toBe(worker.generated * Buffer.byteLength(canonicalAutomaticBuildJson({ edges: [], nodes: [] })));
    }
    const attempts = readAutomaticBuildAttemptSnapshot(resolveAutomaticBuildTarget(source, root)).stages.pass1!;
    for (const id of taskIds) expect(attempts[id]).toMatchObject({ semantic_attempt: 1, lease_epoch: 1, failures: 0, submit_revision: 2 });
    expect(JSON.parse(readFileSync(planPath, "utf8"))).toEqual(buildPlan);
  }, 180_000);
});
