import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { confirmedStandardBuildPlan } from "./helpers/confirmed-build-plan";
import { createAutomaticBuildInvocation, automaticBuildStep, readDshBuildInvocation, runAutomaticBuildDriverCommand } from "../../../skills/build/automatic-build-driver";
import { createBuildExecutorMcpSession } from "../../../skills/build/build-executor-mcp";
import { DSH_BUILD_EXECUTOR_CONTRACT_V1 as contract } from "../src/dsh-build-executor-contract";
import { DSH_BUILD_EXECUTION_PROFILE_V1 as profile } from "../src/build-execution-profile";
import { openAutomaticBuildExecutorSessionV3, resolveAutomaticBuildExecutorRegistryRoot, type AutomaticBuildExecutorSessionResponse } from "../src/automatic-build-executor-session";
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 } from "../src/build-executor-connection-capability";
import { spawnSync } from "node:child_process";

const runtime = { version: "build_executor_model_runtime.v1" as const, provider: "fixture", model: "fixture",
  reasoning_effort: null, max_output_tokens: 4096, context_window_tokens: 131072, safety_margin_tokens: 4096,
  context_source: "provider_model_metadata" as const };

function fixture() {
  const root = mkdtempSync(path.join(tmpdir(), "ub-dsh-session-"));
  const source = path.join(root, "guide.md");
  writeFileSync(source, "# Guide\n\nA transaction groups operations into one atomic change.\n");
  const plan = confirmedStandardBuildPlan(source, root);
  const planPath = path.join(root, "plan.json"); writeFileSync(planPath, JSON.stringify(plan));
  const input = { version: "automatic_build_invocation_create.v2" as const, target_input: source, root_dir: root,
    build_plan_path: planPath, quality_profile: "full" as const, max_parallel: 1 as const, created_at: new Date().toISOString(),
    execution_profile: { profile_id: profile.profile_id, profile_revision: profile.profile_revision }, model_runtime: runtime,
    confirmation: { version: "dsh_build_confirmation.v1" as const, plan_id: plan.plan_id, plan_revision: plan.revision,
      plan_digest: plan.plan_digest, root_session_id: "root-session", question_id: "plan-question", selected: "批准" as const,
      answered_at: new Date().toISOString() } };
  return { root, plan, input };
}

export function dshMcp() {
  const server = createBuildExecutorMcpSession({ bootstrap_version: contract.bootstrap_version,
    protocol_generation: contract.session_protocol, session_private_root: resolveAutomaticBuildExecutorRegistryRoot() });
  let id = 0;
  const rpc = (method: string, params: unknown) => server.handle_message({ jsonrpc: "2.0", id: ++id, method, params }) as any;
  const initialize = () => rpc("initialize", { protocolVersion: "2025-06-18", capabilities: { experimental: { understand_book_executor: contract } } });
  const call = (name: string, args: unknown): AutomaticBuildExecutorSessionResponse => {
    const result = rpc("tools/call", { name, arguments: args });
    expect(result.error).toBeUndefined(); expect(result.result.isError, JSON.stringify(result)).toBe(false);
    const response = JSON.parse(result.result.content[0].text);
    expect(response.version).toBe(contract.session_protocol);
    return response;
  };
  return { rpc, initialize, call };
}

describe("DSH Engine records and real session", () => {
  it("accepts three workers and fences live controllers until their matching release", () => {
    const f = fixture();
    const invocation = createAutomaticBuildInvocation({ ...f.input, max_parallel: 3 });
    expect(readDshBuildInvocation(invocation.invocation_ref, "root-session").max_parallel).toBe(3);
    const request = { version: "dsh_build_controller.v1", invocation_ref: invocation.invocation_ref,
      root_session_id: "root-session", controller_id: "controller-a", owner_pid: process.pid, action: "acquire" };
    expect(runAutomaticBuildDriverCommand(request)).toMatchObject({ acquired: true });
    expect(runAutomaticBuildDriverCommand({ ...request, controller_id: "controller-b" })).toMatchObject({ acquired: false });
    expect(() => runAutomaticBuildDriverCommand({ ...request, controller_id: "controller-b", action: "release" })).toThrow(/owner/);
    runAutomaticBuildDriverCommand({ ...request, action: "release" });
    expect(runAutomaticBuildDriverCommand({ ...request, controller_id: "controller-b" })).toMatchObject({ acquired: true });
    runAutomaticBuildDriverCommand({ ...request, controller_id: "controller-b", action: "release" });
  });
  it("reclaims a proven dead controller but an old release cannot remove its replacement", () => {
    const f = fixture();
    const invocation = createAutomaticBuildInvocation(f.input);
    const dead = spawnSync(process.execPath, ["-e", ""], { windowsHide: true });
    expect(dead.status).toBe(0);
    const request = { version: "dsh_build_controller.v1", invocation_ref: invocation.invocation_ref,
      root_session_id: "root-session", controller_id: "old", owner_pid: dead.pid, action: "acquire" };
    expect(runAutomaticBuildDriverCommand(request)).toEqual({ acquired: true });
    const replacement = { ...request, controller_id: "replacement", owner_pid: process.pid };
    expect(runAutomaticBuildDriverCommand(replacement)).toEqual({ acquired: true });
    expect(() => runAutomaticBuildDriverCommand({ ...request, action: "release" })).toThrow(/owner/);
    expect(runAutomaticBuildDriverCommand({ ...replacement, controller_id: "third" })).toEqual({ acquired: false });
    runAutomaticBuildDriverCommand({ ...replacement, action: "release" });
  });

  it("records bounded host uncertainty once, rejects foreign slots and leaves V1 closed", () => {
    const f = fixture(); const inv = createAutomaticBuildInvocation(f.input);
    const owner = { version: "dsh_build_controller.v1", invocation_ref: inv.invocation_ref, root_session_id: "root-session",
      controller_id: "observer", owner_pid: process.pid };
    runAutomaticBuildDriverCommand({ ...owner, action: "acquire" });
    const step = runAutomaticBuildDriverCommand({ ...owner, action: "step", available_agent_slots: 1 }) as any;
    const observation = { version: "dsh_executor_observation.v1", launch_id: "launch-1", ...step.action.executors[0],
      outcome: "connection", calls: null, last_operation: "executor.submit_candidate", code: "executor_connection_closed" };
    expect(runAutomaticBuildDriverCommand({ ...owner, action: "observe", observation })).toEqual({ recorded: true });
    expect(runAutomaticBuildDriverCommand({ ...owner, action: "observe", observation })).toEqual({ recorded: true });
    expect(() => runAutomaticBuildDriverCommand({ ...owner, action: "observe", observation: { ...observation, dispatch_slot_ref: `abdispatchslot1_${"a".repeat(64)}` } })).toThrow(/owner/);
    expect(() => runAutomaticBuildDriverCommand({ ...owner, action: "observe", observation: { ...observation, candidate: "PRIVATE" } })).toThrow(/invalid/);
    expect(() => runAutomaticBuildDriverCommand({ version: "automatic_build_step_request.v1", invocation_ref: inv.invocation_ref, available_agent_slots: 0, observation })).toThrow();
    runAutomaticBuildDriverCommand({ ...owner, action: "release" });
  }, 30000);

  it("zero-call DSH bootstrap failure uses a new recovery handoff in the same slot", () => {
    const f = fixture(); const inv = createAutomaticBuildInvocation(f.input);
    const owner = { version: "dsh_build_controller.v1", invocation_ref: inv.invocation_ref, root_session_id: "root-session",
      controller_id: "bootstrap", owner_pid: process.pid };
    runAutomaticBuildDriverCommand({ ...owner, action: "acquire" });
    const first = runAutomaticBuildDriverCommand({ ...owner, action: "step", available_agent_slots: 1 }) as any;
    const launch = first.action.executors[0];
    runAutomaticBuildDriverCommand({ ...owner, action: "observe", observation: { version: "dsh_executor_observation.v1",
      launch_id: "zero-call", ...launch, outcome: "bootstrap", calls: 0, code: "executor_launch_failed" } });
    const next = runAutomaticBuildDriverCommand({ ...owner, action: "step", available_agent_slots: 1 }) as any;
    expect(next.action.kind, JSON.stringify(next)).toBe("SPAWN_EXECUTORS");
    expect(next.action.executors[0].dispatch_slot_ref).toBe(launch.dispatch_slot_ref);
    expect(next.action.executors[0].opaque_handoff_ref).not.toBe(launch.opaque_handoff_ref);
    runAutomaticBuildDriverCommand({ ...owner, action: "release" });
  }, 30000);

  it("controlled Codex to DSH takeover retains accepted Pass1 and starts at the next stage", () => {
    const f = fixture();
    const { execution_profile, model_runtime, confirmation, ...common } = f.input;
    const old = createAutomaticBuildInvocation({ ...common, version: "automatic_build_invocation_create.v1" });
    const step = automaticBuildStep({ version: "automatic_build_step_request.v1", invocation_ref: old.invocation_ref, available_agent_slots: 1 });
    if (step.action.kind !== "SPAWN_EXECUTORS") throw new Error("expected Codex work");
    const ref = step.action.executors[0].opaque_handoff_ref;
    const server = createBuildExecutorMcpSession({ bootstrap_version: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.version,
      protocol_generation: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.session_protocol, session_private_root: resolveAutomaticBuildExecutorRegistryRoot() });
    let id = 0;
    const call = (name: string, args: unknown) => {
      const r = server.handle_message({ jsonrpc: "2.0", id: ++id, method: "tools/call", params: { name, arguments: args } }) as any;
      expect(r.result.isError).toBe(false); return JSON.parse(r.result.content[0].text);
    };
    let r = call("executor.open", { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: ref });
    let request = r.action.next_request;
    for (;;) {
      r = call("executor.input.next", request);
      const b = r.action.batch;
      if (b.final_for_generation) {
        r = call("executor.generation.start", { version: "automatic_build_executor_generation_start_request.v3", opaque_session_ref: b.opaque_session_ref,
          generation_input_ref: b.generation_input_ref, confirmed_through_ordinal: b.last_ordinal }); break;
      }
      request = { ...request, ack_through_ordinal: b.last_ordinal };
    }
    const contender = createAutomaticBuildInvocation({ ...f.input, created_at: "2026-09-26T00:00:00.000Z" });
    const blocked = automaticBuildStep({ version: "automatic_build_step_request.v1", invocation_ref: contender.invocation_ref, available_agent_slots: 3 });
    expect(blocked.action.kind, JSON.stringify(blocked)).not.toBe("SPAWN_EXECUTORS");
    expect(blocked.action.kind).not.toBe("DONE");
    expect(call("executor.submit_candidate", { version: "automatic_build_executor_candidate_submit.v3", opaque_session_ref: r.action.opaque_session_ref,
      candidate_sink_ref: r.action.candidate_sink_ref, candidate: { nodes: [{ id: "claim:1.2:atomic", type: "claim", name: "A transaction groups operations into one atomic change.", occurrences: [], source_lid: "1.2" }], edges: [] } }).action.status).toBe("committed");
    const oldBytes = readFileSync(path.join(resolveAutomaticBuildExecutorRegistryRoot(), "opaque-handoffs", `${ref}.json`));
    const next = createAutomaticBuildInvocation(f.input);
    expect(next.invocation_ref).not.toBe(old.invocation_ref);
    const resumed = automaticBuildStep({ version: "automatic_build_step_request.v1", invocation_ref: next.invocation_ref, available_agent_slots: 1 });
    expect(resumed.action.kind, JSON.stringify(resumed)).toBe("SPAWN_EXECUTORS");
    if (resumed.action.kind !== "SPAWN_EXECUTORS") throw new Error("expected DSH work");
    const issued = JSON.parse(readFileSync(path.join(resolveAutomaticBuildExecutorRegistryRoot(), "opaque-handoffs", `${resumed.action.executors[0].opaque_handoff_ref}.json`), "utf8"));
    expect(issued.owner_identity.stage).toBe("profile_sidecar");
    expect(readFileSync(path.join(resolveAutomaticBuildExecutorRegistryRoot(), "opaque-handoffs", `${ref}.json`))).toEqual(oldBytes);
  }, 30000);
  it("requires explicit initialization without claiming a handoff", () => {
    const m = dshMcp();
    expect(m.rpc("tools/list", {}).error.code).toBe(-32002);
    expect(m.rpc("initialize", { protocolVersion: "2025-06-18", capabilities: {} }).error.code).toBe(-32602);
    expect(m.initialize().result.capabilities.experimental.understand_book_executor).toEqual(contract);
    expect(m.rpc("tools/list", {}).result.tools).toHaveLength(4);
  });

  it("persists confirmation/runtime, rejects drift and cross-host access, replays then commits a public handoff", () => {
    const f = fixture();
    expect(() => createAutomaticBuildInvocation({ ...f.input, model_runtime: undefined })).toThrow(/runtime/);
    expect(() => createAutomaticBuildInvocation({ ...f.input, confirmation: { ...f.input.confirmation, plan_revision: 2 } })).toThrow(/changed/);
    const invocation = createAutomaticBuildInvocation(f.input);
    expect(readDshBuildInvocation(invocation.invocation_ref, "root-session").model_runtime).toEqual(runtime);
    expect(() => readDshBuildInvocation(invocation.invocation_ref, "sibling")).toThrow(/owner/);
    const step = automaticBuildStep({ version: "automatic_build_step_request.v1", invocation_ref: invocation.invocation_ref, available_agent_slots: 1 });
    expect(step.action.kind, JSON.stringify(step)).toBe("SPAWN_EXECUTORS");
    if (step.action.kind !== "SPAWN_EXECUTORS") throw new Error("expected dispatch");
    const handoff = step.action.executors[0].opaque_handoff_ref;
    expect(() => openAutomaticBuildExecutorSessionV3(handoff)).toThrow(/profile/);
    const stored = JSON.parse(readFileSync(path.join(resolveAutomaticBuildExecutorRegistryRoot(), "opaque-handoffs", `${handoff}.json`), "utf8"));
    expect(stored.version).toBe("automatic_build_opaque_handoff_record.v5");
    const m = dshMcp(); m.initialize();
    const open = m.call("executor.open", { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: handoff });
    if (open.action.kind !== "DELIVER_INPUT") throw new Error("expected input");
    expect(open.action.input_manifest.version).toBe("automatic_build_executor_input_manifest.v4");
    let request = open.action.next_request;
    let last = 0;
    for (;;) {
      const batch = m.call("executor.input.next", request);
      expect(m.call("executor.input.next", request)).toEqual(batch);
      if (batch.action.kind !== "INPUT_BATCH") throw new Error("expected batch");
      expect(batch.action.batch.chunks).toHaveLength(1);
      last = batch.action.batch.last_ordinal;
      if (batch.action.batch.final_for_generation) break;
      request = { ...request, ack_through_ordinal: last };
    }
    const started = m.call("executor.generation.start", { version: "automatic_build_executor_generation_start_request.v3",
      opaque_session_ref: request.opaque_session_ref, generation_input_ref: request.generation_input_ref, confirmed_through_ordinal: last });
    if (started.action.kind !== "GENERATE") throw new Error("expected generation");
    const done = m.call("executor.submit_candidate", { version: "automatic_build_executor_candidate_submit.v3",
      opaque_session_ref: started.action.opaque_session_ref, candidate_sink_ref: started.action.candidate_sink_ref, candidate: { nodes: [], edges: [] } });
    expect(done.action).toEqual({ kind: "DONE", status: "committed" });
    const resumed = dshMcp(); resumed.initialize();
    expect(resumed.call("executor.open", { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: handoff })).toEqual(done);
  }, 30000);
});
