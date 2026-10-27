import { createBuildExecutorMcpSession } from "../../../skills/build/build-executor-mcp";
import { fakeEmbedding } from "./fixtures/embedding-provider";
import { attachBuildPlanDigest, type BuildRetrievalSelection } from "../src/build-intent";
import { prepareAutomaticBuildSnapshot } from "../src/build-orchestrator";
import { readBuildRetrievalState } from "../src/automatic-build-retrieval";
import * as retrievalConfig from "../src/build-retrieval-config";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { onTestFinished } from "vitest";
import { DSH_BUILD_EXECUTION_PROFILE_V1, type BuildExecutionProfileV1 } from "../src/build-execution-profile";
import { expect, it, vi, afterEach } from "vitest";
import path from "node:path";
import { structureProductionFixture, structureProductionResponse } from "./helpers/book-structure-production";
import { confirmedStandardBuildPlan } from "./helpers/confirmed-build-plan";
import { automaticBuildPlan, automaticBuildNext } from "../../../skills/build/automatic-build";
import { readStructureThemeSession, checkpointStructureThemeAction } from "../src/book-structure-theme-session";
import { renderBookStructureGenerationTaskInput, type BookStructureGenerationTaskV1 } from "../src/book-structure-generation";
import { openAutomaticBuildExecutorSessionV3, nextAutomaticBuildExecutorInput, startAutomaticBuildExecutorGeneration,
  submitAutomaticBuildExecutorCandidateV3, issueAutomaticBuildOpaqueHandoff, type AutomaticBuildExecutorSessionResponse } from "../src/automatic-build-executor-session";
import { inspectAutomaticBuildDispatchRecoveryGeneration } from "../src/automatic-build-dispatch-runtime";

afterEach(() => { vi.unstubAllEnvs(); vi.restoreAllMocks(); });

function themesReady(count = 3, execution_profile?: BuildExecutionProfileV1) {
  const f = structureProductionFixture(2, execution_profile);
  for (let round = 0; round < 30; round++) {
    const state = f.get();
    const tasks = state.pending_tasks.map(id => {
      const generation = state.generation_tasks![id];
      if (generation.kind !== "book_structure") throw Error("expected structure task");
      return generation.task;
    });
    if (tasks.some(t => t.output_role === "organization_theme")) return { ...f, tasks };
    for (const task of tasks) {
      const candidate = structureProductionResponse(task) as { themes?: Array<Record<string, unknown>> };
      f.submit(task, candidate.themes?.length ? { themes: Array.from({ length: count }, (_, i) => ({
        ...candidate.themes![0], ref: `theme:${i}`, question: `How does ownership affect transfer under condition ${i}?`,
      })) } : candidate);
    }
  }
  throw Error("theme plan not reached");
}

function generate(response: AutomaticBuildExecutorSessionResponse, now: string, execution_profile?: BuildExecutionProfileV1) {
  if (response.action.kind !== "DELIVER_INPUT") throw Error(`expected delivery, got ${response.action.kind}`);
  const delivery = response.action;
  let request = delivery.next_request;
  for (;;) {
    const response = nextAutomaticBuildExecutorInput(request, { now, execution_profile });
    if (response.action.kind !== "INPUT_BATCH") throw Error("expected batch");
    const batch = response.action.batch;
    if (batch.final_for_generation) {
      const started = startAutomaticBuildExecutorGeneration({ version: "automatic_build_executor_generation_start_request.v3",
        opaque_session_ref: delivery.input_manifest.opaque_session_ref, generation_input_ref: delivery.input_manifest.generation_input_ref,
        confirmed_through_ordinal: batch.last_ordinal }, { now, execution_profile });
      if (started.action.kind !== "GENERATE") throw Error("expected generation");
      return started.action;
    }
    request = { ...request, ack_through_ordinal: batch.last_ordinal };
  }
}

it("offers every theme together and keeps sibling descriptors stable through interleaved checkpoints and commits", () => {
  const f = themesReady();
  expect(f.tasks).toHaveLength(3);
  const seeds = f.tasks.map(renderBookStructureGenerationTaskInput);
  const [a, b, c] = f.tasks;
  f.submit(a); f.submit(b);
  expect(readStructureThemeSession(f.target, a).ordinal).toBe(1);
  expect(readStructureThemeSession(f.target, c).ordinal).toBe(0);
  f.submit(b); f.submit(b);
  const during = f.get();
  expect(during.book_structure_progress!.organization!.themes).toMatchObject({ done: 1, total: 3 });
  for (const [i, task] of f.tasks.entries()) {
    const current = during.generation_tasks![task.descriptor.work_unit_id];
    expect(current.kind).toBe("book_structure");
    if (current.kind !== "book_structure") throw Error("missing theme");
    expect(current.task.descriptor).toEqual(task.descriptor);
    expect(renderBookStructureGenerationTaskInput(current.task)).toBe(seeds[i]);
  }
  f.submit(c); f.submit(a); f.submit(c); f.submit(a); f.submit(c);
  const final = f.get();
  expect(final.pending_tasks).toHaveLength(1);
  expect(final.work_units!.find(t => t.work_unit_id === final.pending_tasks[0])!.kind).toBe("structure_theme_reconcile");
}, 60000);

it.each([false, true])("keeps one handoff and lease per theme through search, inspect and resolve, including repeated submit (DSH=%s)", dsh => {
  const execution_profile = dsh ? DSH_BUILD_EXECUTION_PROFILE_V1 : undefined;
  const f = themesReady(2, execution_profile);
  vi.stubEnv("UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT", path.join(f.root, "registry"));
  const buildPlan = confirmedStandardBuildPlan(f.source, f.root);
  const plan = automaticBuildPlan(f.source, f.root, { requested_workers: 8, available_agent_slots: 8, build_plan: buildPlan, execution_profile });
  const now = "2026-10-26T04:00:00.000Z";
  const next = automaticBuildNext(f.source, f.root, 8, { now, accepted_plan_digest: plan.preflight!.descriptor_plan_digest,
    available_agent_slots: 8, executor_dispatches: true, build_plan: buildPlan, execution_profile });
  if (!("dispatches" in next.action) || !next.action.dispatches) throw Error("missing dispatches");
  expect(next.action.dispatches).toHaveLength(2);
  const handoffs = next.action.dispatches;
  expect(handoffs.map(h => h.manifest.ordered_work_unit_ids.length)).toEqual([1, 1]);
  const sessions = handoffs.map(h => openAutomaticBuildExecutorSessionV3(h.opaque_handoff_ref, { now, execution_profile }));
  const sinks = new Set<string>();
  for (let round = 0; round < 3; round++) {
    for (const i of [1, 0]) {
      const started = generate(sessions[i], now, execution_profile);
      expect(sinks.has(started.candidate_sink_ref)).toBe(false); sinks.add(started.candidate_sink_ref);
      const task = f.tasks.find(t => t.descriptor.work_unit_id === handoffs[i].manifest.ordered_work_unit_ids[0])!;
      const session = readStructureThemeSession(f.target, task);
      const candidate = JSON.parse(JSON.stringify(structureProductionResponse({ ...task, input: session.packet })));
      const request = { version: "automatic_build_executor_candidate_submit.v3" as const,
        opaque_session_ref: started.opaque_session_ref, candidate_sink_ref: started.candidate_sink_ref, candidate };
      sessions[i] = submitAutomaticBuildExecutorCandidateV3(request as Parameters<typeof submitAutomaticBuildExecutorCandidateV3>[0], { now, execution_profile });
      expect(sessions[i].action.kind).toBe(round === 2 ? "DONE" : "DELIVER_INPUT");
      if (round === 0) {
        expect(submitAutomaticBuildExecutorCandidateV3(request as Parameters<typeof submitAutomaticBuildExecutorCandidateV3>[0], { now, execution_profile })).toEqual(sessions[i]);
        expect(readStructureThemeSession(f.target, task).ordinal).toBe(1);
        expect(openAutomaticBuildExecutorSessionV3(handoffs[i].opaque_handoff_ref, { now, execution_profile })).toEqual(sessions[i]);
      }
      if (round === 2) expect(sessions[i].action).toMatchObject({ status: "committed" });
    }
  }
  expect(f.get().book_structure_progress!.organization!.themes.done).toBe(2);
}, 120000);

it("rejects undelivered evidence and altered replay without losing accepted theme progress", () => {
  const f = themesReady(1), task = f.tasks[0];
  const initial = readStructureThemeSession(f.target, task);
  const search = { kind: "search", query: "ownership", notes: "searched" };
  expect(checkpointStructureThemeAction(f.target, task, "input-0", initial.rendered, search)).toBe("continue");
  expect(checkpointStructureThemeAction(f.target, task, "input-0", initial.rendered, search)).toBe("continue");
  expect(() => checkpointStructureThemeAction(f.target, task, "input-0", initial.rendered, { ...search, query: "changed" })).toThrow(/changed after acceptance/);
  const current = readStructureThemeSession(f.target, task);
  expect(current.ordinal).toBe(1);
  const forged: BookStructureGenerationTaskV1 = structuredClone(task);
  if (!("phase" in forged.input) || forged.input.phase !== "theme") throw Error("theme expected");
  forged.input.directory.works[0].inspecting = forged.input.directory.works[0].plan.member_refs;
  forged.input.directory.works[0].notes = "searched";
  const candidate = structureProductionResponse(forged);
  expect(() => checkpointStructureThemeAction(f.target, task, "input-1", current.rendered, candidate)).toThrow(/undelivered|inspected/);
  expect(readStructureThemeSession(f.target, task).ordinal).toBe(1);
}, 60000);

it("resumes accepted theme actions under a replacement lease epoch after interruption", () => {
  const f = themesReady(1);
  vi.stubEnv("UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT", path.join(f.root, "recovery-registry"));
  const buildPlan = confirmedStandardBuildPlan(f.source, f.root);
  const plan = automaticBuildPlan(f.source, f.root, { requested_workers: 1, available_agent_slots: 1, build_plan: buildPlan });
  const now = "2026-10-26T04:00:00.000Z", later = "2026-10-26T06:00:00.000Z";
  const next = automaticBuildNext(f.source, f.root, 1, { now, accepted_plan_digest: plan.preflight!.descriptor_plan_digest,
    available_agent_slots: 1, executor_dispatches: true, build_plan: buildPlan });
  if (!("dispatches" in next.action) || !next.action.dispatches) throw Error("missing dispatch");
  const handoff = next.action.dispatches[0], task = f.tasks[0];
  const first = generate(openAutomaticBuildExecutorSessionV3(handoff.opaque_handoff_ref, { now }), now);
  submitAutomaticBuildExecutorCandidateV3({ version: "automatic_build_executor_candidate_submit.v3",
    opaque_session_ref: first.opaque_session_ref, candidate_sink_ref: first.candidate_sink_ref,
    candidate: { kind: "search", query: "ownership", notes: "searched" } }, { now });
  const recovery = inspectAutomaticBuildDispatchRecoveryGeneration(f.target, "book_structure", handoff.manifest.dispatch_id,
    { now: later, dispatch_run_id: handoff.dispatch_run_id }).recovery_identity;
  expect(recovery.lease_epoch).toBe(2);
  const replacement = issueAutomaticBuildOpaqueHandoff({ target: f.target, kind: "public_dispatch",
    owner_identity: { version: "automatic_build_dispatch_owner_identity.v1", stage: "book_structure",
      dispatch_id: handoff.manifest.dispatch_id, dispatch_run_id: handoff.dispatch_run_id },
    recovery_identity: recovery, executor_handoff: handoff.executor_handoff, issued_at: later });
  let response = openAutomaticBuildExecutorSessionV3(replacement.opaque_handoff_ref, { now: later });
  expect(readStructureThemeSession(f.target, task).ordinal).toBe(1);
  for (let round = 0; round < 2; round++) {
    const started = generate(response, later);
    const candidate = JSON.parse(JSON.stringify(structureProductionResponse({ ...task, input: readStructureThemeSession(f.target, task).packet })));
    response = submitAutomaticBuildExecutorCandidateV3({ version: "automatic_build_executor_candidate_submit.v3",
      opaque_session_ref: started.opaque_session_ref, candidate_sink_ref: started.candidate_sink_ref, candidate }, { now: later });
  }
  expect(response.action).toMatchObject({ kind: "DONE", status: "committed" });
}, 60000);


it.skipIf(!process.env.UNDERSTAND_BOOK_THEME_SIDECAR)("runs two complete theme sessions over separate compiled MCP connections", async () => {
  const f = themesReady(2), registry = path.join(f.root, "compiled-registry");
  vi.stubEnv("UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT", registry);
  const buildPlan = confirmedStandardBuildPlan(f.source, f.root);
  const plan = automaticBuildPlan(f.source, f.root, { requested_workers: 8, available_agent_slots: 8, build_plan: buildPlan });
  const next = automaticBuildNext(f.source, f.root, 8, { accepted_plan_digest: plan.preflight!.descriptor_plan_digest,
    available_agent_slots: 8, executor_dispatches: true, build_plan: buildPlan });
  if (!("dispatches" in next.action) || !next.action.dispatches) throw Error("missing dispatches");
  expect(next.action.dispatches).toHaveLength(2);
  await Promise.all(next.action.dispatches.map(async handoff => {
    const child = spawn(process.env.UNDERSTAND_BOOK_THEME_SIDECAR!, ["executor.mcp", "--bootstrap-version",
      "automatic_build_executor_bootstrap.v3", "--protocol-generation", "automatic_build_executor_session.v3"],
      { cwd: f.root, env: process.env, stdio: ["pipe", "pipe", "pipe"], windowsHide: true });
    onTestFinished(() => { child.kill(); });
    const lines = createInterface({ input: child.stdout });
    let stderr = "", id = 0;
    child.stderr.on("data", data => { stderr += data; });
    const pending = new Map<number, { resolve: (value: any) => void; reject: (error: Error) => void }>();
    lines.on("line", line => { const value = JSON.parse(line); const waiter = pending.get(value.id);
      pending.delete(value.id); waiter?.resolve(value); });
    child.on("close", code => { for (const p of pending.values()) p.reject(new Error(`MCP closed ${code}: ${stderr}`)); });
    const rpc = (method: string, params: unknown): Promise<any> => new Promise((resolve, reject) => {
      const requestId = ++id; pending.set(requestId, { resolve, reject });
      child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: requestId, method, params }) + "\n");
    });
    const call = async (name: string, args: unknown): Promise<AutomaticBuildExecutorSessionResponse> => {
      const reply = await rpc("tools/call", { name, arguments: args });
      if (reply.error || reply.result.isError) throw new Error(JSON.stringify(reply));
      return JSON.parse(reply.result.content.find((v: any) => v.type === "text").text);
    };
    await rpc("initialize", { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "theme-smoke", version: "1" } });
    const listed = await rpc("tools/list", {});
    expect(listed.result.tools.map((t: any) => t.name)).toEqual(["executor.open", "executor.input.next", "executor.generation.start", "executor.submit_candidate"]);
    let response = await call("executor.open", { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: handoff.opaque_handoff_ref });
    const task = f.tasks.find(t => t.descriptor.work_unit_id === handoff.manifest.ordered_work_unit_ids[0])!;
    for (let round = 0; round < 3; round++) {
      if (response.action.kind !== "DELIVER_INPUT") throw Error(`expected delivery ${response.action.kind}`);
      let request = response.action.next_request;
      for (;;) {
        const delivered = await call("executor.input.next", request);
        if (delivered.action.kind !== "INPUT_BATCH") throw Error("expected input batch");
        const batch = delivered.action.batch;
        if (batch.final_for_generation) {
          response = await call("executor.generation.start", { version: "automatic_build_executor_generation_start_request.v3",
            opaque_session_ref: request.opaque_session_ref, generation_input_ref: request.generation_input_ref,
            confirmed_through_ordinal: batch.last_ordinal });
          break;
        }
        request = { ...request, ack_through_ordinal: batch.last_ordinal };
      }
      if (response.action.kind !== "GENERATE") throw Error("expected generation");
      response = await call("executor.submit_candidate", { version: "automatic_build_executor_candidate_submit.v3",
        opaque_session_ref: response.action.opaque_session_ref, candidate_sink_ref: response.action.candidate_sink_ref,
        candidate: structureProductionResponse({ ...task, input: readStructureThemeSession(f.target, task).packet }) });
      expect(response.action.kind).toBe(round === 2 ? "DONE" : "DELIVER_INPUT");
    }
    expect(response.action).toMatchObject({ kind: "DONE", status: "committed" });
  }));
  expect(f.get().book_structure_progress!.organization!.themes.done).toBe(2);
}, 120000);


it("prepares concurrent semantic searches inside their existing MCP sessions with one shared budget ledger", async () => {
  const f = themesReady(2), registry = path.join(f.root, "semantic-registry"), provider = fakeEmbedding();
  provider.limits.max_input_tokens = 10000;
  vi.stubEnv("UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT", registry);
  const selection: BuildRetrievalSelection = { retrieval_mode: "semantic_required", provider: { location: "local", identity: provider.identity },
    data_scope: "book_structure_projections_and_queries" };
  const runtime = { selection, provider };
  const buildPlan = attachBuildPlanDigest({ ...confirmedStandardBuildPlan(f.source, f.root), retrieval: {
    selection, estimate: { records: null, queries: null, basis: "unknown_until_fragments" },
    budget: { max_documents: 100, max_queries: 10, max_calls: 100 } } });
  vi.spyOn(retrievalConfig, "configuredBuildRetrievalRuntime").mockReturnValue(runtime);
  expect(await prepareAutomaticBuildSnapshot(f.target, "book_structure", { authorization: { plan: buildPlan, runtime } })).toMatchObject({ status: "ready" });
  const plan = automaticBuildPlan(f.source, f.root, { requested_workers: 2, available_agent_slots: 2, build_plan: buildPlan });
  const next = automaticBuildNext(f.source, f.root, 2, { accepted_plan_digest: plan.preflight!.descriptor_plan_digest,
    available_agent_slots: 2, executor_dispatches: true, build_plan: buildPlan });
  if (!("dispatches" in next.action) || !next.action.dispatches) throw Error("missing dispatches");
  expect(next.action.dispatches).toHaveLength(2);
  const tasks = f.get(selection).generation_tasks!;
  await Promise.all(next.action.dispatches.map(async (handoff, i) => {
    const session = createBuildExecutorMcpSession({ bootstrap_version: "automatic_build_executor_bootstrap.v3",
      protocol_generation: "automatic_build_executor_session.v3", session_private_root: registry });
    let id = 0;
    const call = async (name: string, args: unknown): Promise<AutomaticBuildExecutorSessionResponse> => {
      const response: any = await session.handle_message({ jsonrpc: "2.0", id: ++id, method: "tools/call", params: { name, arguments: args } });
      if (response.result.isError) throw Error(JSON.stringify(response));
      return JSON.parse(response.result.content[0].text);
    };
    let response = await call("executor.open", { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: handoff.opaque_handoff_ref });
    const generation = tasks[handoff.manifest.ordered_work_unit_ids[0]];
    if (generation.kind !== "book_structure") throw Error("missing theme");
    for (let round = 0; round < 3; round++) {
      if (response.action.kind !== "DELIVER_INPUT") throw Error("expected delivery");
      let request = response.action.next_request;
      for (;;) {
        const delivered = await call("executor.input.next", request);
        if (delivered.action.kind !== "INPUT_BATCH") throw Error("expected batch");
        const batch = delivered.action.batch;
        if (batch.final_for_generation) {
          response = await call("executor.generation.start", { version: "automatic_build_executor_generation_start_request.v3",
            opaque_session_ref: request.opaque_session_ref, generation_input_ref: request.generation_input_ref,
            confirmed_through_ordinal: batch.last_ordinal });
          break;
        }
        request = { ...request, ack_through_ordinal: batch.last_ordinal };
      }
      if (response.action.kind !== "GENERATE") throw Error("expected generation");
      const candidate = round === 0 ? { kind: "search", query: i ? "transfer" : "ownership", notes: "searched" }
        : structureProductionResponse({ ...generation.task, input: readStructureThemeSession(f.target, generation.task).packet });
      response = await call("executor.submit_candidate", { version: "automatic_build_executor_candidate_submit.v3",
        opaque_session_ref: response.action.opaque_session_ref, candidate_sink_ref: response.action.candidate_sink_ref,
        candidate: JSON.parse(JSON.stringify(candidate)) });
    }
    expect(response.action).toMatchObject({ kind: "DONE", status: "committed" });
  }));
  expect(provider.calls.filter(c => c.role === "query")).toHaveLength(2);
  expect(readBuildRetrievalState(f.target.workspace_dir).calls.filter(c => c.role === "query" && c.status === "ok")).toHaveLength(2);
  expect(f.get(selection).book_structure_progress!.organization!.themes.done).toBe(2);
}, 120000);
