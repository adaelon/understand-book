import { randomUUID } from "node:crypto";
import type { Context } from "@deepseek-ai/cordis";
import type { Agent } from "@deepseek-ai/dsh-agent";
import type { AutomaticBuildStepResponseV1 } from "../../../skills/build/automatic-build-driver.ts";
import type { BuildPlanV1, BuildRetrievalPlan } from "../../core/src/build-intent.ts";
import type { DshExecutorObservationV1 } from "../../core/src/dsh-executor-observation.ts";
import { readBuildExecutorModelRuntime } from "../../core/src/build-executor-model-runtime.ts";
import { checkHarnessCapabilities } from "./capability-check.ts";
import { createExecutorHost } from "./executor-host.ts";
import { prepareExecutorBridge, type ExecutorObservation } from "./executor-bridge.ts";
import { resolveExecutorRuntime } from "./executor-runtime.ts";
import { engineCommand, type EngineConfig } from "./engine-client.ts";

export interface BuildControlConfig extends EngineConfig {
  maxOutputTokens: number;
  safetyMarginTokens: number;
  handoffsPerCall: number;
}

const reservations = new WeakMap<object, Set<{ agent?: Agent }>>();

function exactlySelected(answer: any, id: string, label: string): boolean {
  return Array.isArray(answer?.answers) && answer.answers.length === 1 && answer.answers[0].id === id
    && Array.isArray(answer.answers[0].selected) && answer.answers[0].selected.length === 1
    && answer.answers[0].selected[0] === label && !answer.answers[0].custom;
}

export function createBuildControl(ctx: Context, config: BuildControlConfig, presetId: string) {
  const active = new Map<string, { controller: AbortController; done: Promise<unknown> }>();
  const preparations = new Set<Promise<unknown>>();
  const shutdown = new AbortController();
  let closed = false;
  function assertRoot(agent: Agent) {
    if (closed || ctx.agents.get(agent.id) !== agent || (agent.options.subagentDepth ?? 0) > 0) throw new Error("build_root_required");
  }
  const command = (request: unknown, signal?: AbortSignal) => engineCommand(config, ["build.step"], request, signal);

  async function prepareAndConfirmOwned(agent: Agent, signal: AbortSignal,
    input: { target_input: string; root_dir: string; pass2: "enabled" | "disabled"; budget?: BuildPlanV1["budget"]; max_parallel?: 1 | 2 | 3;
      retrieval?: { retrieval_mode: "lexical_only" | "semantic_required"; budget: BuildRetrievalPlan["budget"] } }) {
    assertRoot(agent);
    if (!input || Object.keys(input).some(k => !["target_input", "root_dir", "pass2", "budget", "max_parallel", "retrieval"].includes(k))
      || typeof input.target_input !== "string" || typeof input.root_dir !== "string"
      || !["enabled", "disabled"].includes(input.pass2)) throw new Error("build_prepare_arguments_invalid");
    if (checkHarnessCapabilities(ctx).status !== "available") throw new Error("build_harness_unsupported");
    const workers = input.max_parallel ?? 1;
    if (![1, 2, 3].includes(workers)) throw new Error("build_capacity_invalid");
    const runtime = await resolveExecutorRuntime(ctx, { ...agent.options, maxTokens: config.maxOutputTokens }, config.safetyMarginTokens, signal);
    const { retrieval } = input;
    const prepared = await command({ version: "dsh_build_prepare.v2", ...input, max_parallel: workers }, signal) as {
      version: string; build_plan_path: string; plan: BuildPlanV1; review_markdown: string;
    };
    if (prepared.version !== "dsh_build_prepared.v2" || prepared.plan.private_artifacts.length) throw new Error("build_plan_unsupported");
    if (retrieval && (!prepared.plan.retrieval || prepared.plan.status !== "draft")) throw new Error("build_retrieval_configuration_failed");
    const questionId = `ub-plan-${randomUUID()}`;
    const answer = await ctx.userQuestions.ask({ agent, signal, questions: [{ id: questionId,
      question: "确认本次 Understand Book 预构建计划", detail: `${prepared.review_markdown}\n\n模型：${runtime.provider} / ${runtime.model}\n\n推理设置：${runtime.reasoning_effort ?? "默认"}；单次输出上限：${runtime.max_output_tokens} token。`,
      options: [{ label: "批准" }, { label: "拒绝" }], intent: { kind: "plan-review", approve: "批准" } }] });
    if (!exactlySelected(answer, questionId, "批准") || signal.aborted) throw new Error("build_plan_not_approved");
    assertRoot(agent);
    if (retrieval) await command({ version: "build_retrieval_confirm.v1", build_plan_path: prepared.build_plan_path,
      plan_digest: prepared.plan.plan_digest, confirmation_source: "explicit_legacy_command" }, signal);
    // Engine rereads this exact plan and compares its identity before persisting the invocation.
    return command({ version: "automatic_build_invocation_create.v2", target_input: input.target_input, root_dir: input.root_dir,
      build_plan_path: prepared.build_plan_path, quality_profile: "full", max_parallel: workers, created_at: new Date().toISOString(),
      execution_profile: { profile_id: "dsh_native_v4", profile_revision: 1 }, model_runtime: runtime,
      confirmation: { version: "dsh_build_confirmation.v1", plan_id: prepared.plan.plan_id, plan_revision: prepared.plan.revision,
        plan_digest: prepared.plan.plan_digest, root_session_id: agent.id, question_id: questionId, selected: "批准", answered_at: new Date().toISOString() } }, signal);
  }

  function prepareAndConfirm(agent: Agent, signal: AbortSignal, input: Parameters<typeof prepareAndConfirmOwned>[2]) {
    const work = prepareAndConfirmOwned(agent, AbortSignal.any([signal, shutdown.signal]), input);
    preparations.add(work);
    const settled = () => preparations.delete(work);
    void work.then(settled, settled);
    return work;
  }

  async function drive(agent: Agent, signal: AbortSignal, invocationRef: string) {
    const saved = await command({ version: "dsh_build_invocation_read.v2", invocation_ref: invocationRef, root_session_id: agent.id }, signal) as any;
    if (saved.version !== "dsh_build_invocation.v2") throw new Error("build_invocation_invalid");
    const runtime = readBuildExecutorModelRuntime(saved.model_runtime);
    const owner = { version: "dsh_build_controller.v1", invocation_ref: invocationRef, root_session_id: agent.id,
      controller_id: randomUUID(), owner_pid: process.pid };
    // Acquisition is not cancelled halfway through publication. The finally boundary owns release.
    const acquired = await command({ ...owner, action: "acquire" }) as { acquired: boolean };
    if (!acquired.acquired) return { invocation_ref: invocationRef, host_observation: { code: "build_owner_active" } };
    type Live = { id: string; ref: string; slot: string; agent?: Agent; observation: ExecutorObservation;
      work: Promise<void>; report?: DshExecutorObservationV1 };
    const live = new Map<string, Live>();
    const ready: Live[] = [];
    const reports: DshExecutorObservationV1[] = [];
    const allReservations = reservations.get(ctx.agents) ?? new Set<{ agent?: Agent }>();
    reservations.set(ctx.agents, allReservations);
    const byId = new Map<string, Live>();
    const host = createExecutorHost(ctx, presetId, binding => {
      const item = byId.get(binding.launch.launch_id!)!;
      item.agent = binding.agent;
      return prepareExecutorBridge(binding, config, item.observation);
    });
    let launched = 0;
    let stop = false;
    let drainBoundary = false;
    let decision: { request_id: string; choice_id: string } | undefined;
    let last: AutomaticBuildStepResponseV1 | undefined;
    const capacity = () => {
      const children = ctx.agents.list().filter(a => (a.options.subagentDepth ?? 0) > 0).length;
      const pending = [...allReservations].filter(r => !r.agent || ctx.agents.get(r.agent.id) !== r.agent).length;
      return checkHarnessCapabilities(ctx).status !== "available" ? 0
        : Math.max(0, Math.min(saved.max_parallel - live.size, 3 - children - pending,
          config.handoffsPerCall - launched));
    };
    const step = (slots: number) => command({ ...owner, action: "step", available_agent_slots: slots,
      ...(decision ? { decision } : {}) }, signal) as Promise<AutomaticBuildStepResponseV1>;
    const result = () => ({ invocation_ref: invocationRef, ...(last ? { step: last } : {}),
      capacity: { limit: Math.min(saved.max_parallel, 3), source: "agent_registry_conservative" },
      ...(launched >= config.handoffsPerCall && last?.action.kind !== "DONE" ? { yielded: true } : {}),
      ...(reports.some(r => r.outcome !== "receipt") ? { host_observations: reports.filter(r => r.outcome !== "receipt") } : {}) });
    async function consume() {
      while (ready.length) {
        const item = ready[0];
        // Exact launch identity prevents any old terminal from retiring a replacement slot.
        if (live.get(item.slot) !== item || live.get(item.slot)?.ref !== item.ref) { ready.shift(); continue; }
        const report = item.report!;
        await command({ ...owner, action: "observe", observation: report });
        ready.shift();
        reports.push(report);
        live.delete(item.slot); byId.delete(item.id); allReservations.delete(item);
        if (!report.done || report.outcome === "cleanup") stop = true;
      }
    }
    function launch(executor: { opaque_handoff_ref: string; dispatch_slot_ref?: string }) {
      const slot = executor.dispatch_slot_ref;
      if (!slot) throw new Error("build_dispatch_slot_missing");
      if (live.has(slot)) return;
      const item: Live = { id: randomUUID(), ref: executor.opaque_handoff_ref, slot, observation: {}, work: Promise.resolve() };
      live.set(slot, item); byId.set(item.id, item); allReservations.add(item); launched++;
      item.work = (async () => {
        let code: DshExecutorObservationV1["code"];
        let outcome: DshExecutorObservationV1["outcome"] = "runtime";
        let owned: Awaited<ReturnType<typeof host.launch>> | undefined;
        try {
          owned = await host.launch({ parent: agent, signal, opaque_handoff_ref: item.ref, runtime,
            launch_id: item.id, dispatch_slot_ref: slot });
          const terminal = await owned.terminal;
          code = terminal.diagnostic_code;
          if (item.observation.done && terminal.stop_reason === "completed") outcome = "receipt";
          else if (item.observation.done) { outcome = "runtime"; code ??= "executor_runtime_failed"; }
          else if (item.observation.error_code === "executor_connection_closed") outcome = "connection";
          else if (item.observation.last_operation === "executor.open"
            && ["handoff_ref_mismatch", "connection_terminal"].includes(item.observation.error_code ?? "")) outcome = "open";
          if (["executor_connection_closed", "connection_terminal", "handoff_ref_mismatch"].includes(item.observation.error_code ?? "")) {
            code ??= item.observation.error_code as typeof code;
          }
          if (!item.observation.done) code ??= "executor_ended_without_receipt";
        } catch {
          outcome = item.observation.bootstrap_failed ? "bootstrap" : "runtime";
          code = "executor_launch_failed";
        } finally {
          if (signal.aborted) { outcome = "cancelled"; code = "build_cancelled"; }
          try { await owned?.dispose(); } catch { outcome = "cleanup"; code = "executor_cleanup_failed"; }
          item.report = { version: "dsh_executor_observation.v1", launch_id: item.id,
            opaque_handoff_ref: item.ref, dispatch_slot_ref: slot, outcome, calls: item.observation.calls ?? null,
            ...(item.observation.last_operation ? { last_operation: item.observation.last_operation } : {}),
            ...(item.observation.engine_error ? { engine_error: item.observation.engine_error } : {}),
            ...(item.observation.done ? { done: item.observation.done } : {}), ...(code ? { code } : {}) };
          ready.push(item);
        }
      })();
    }
    try {
      for (;;) {
        await consume();
        if (signal.aborted) throw new Error("build_cancelled");
        if (drainBoundary && live.size) {
          await Promise.race([...live.values()].map(i => i.work));
          continue;
        }
        drainBoundary = false;
        // Once a boundary is reached, drain every owned executor before asking or returning.
        if (stop || launched >= config.handoffsPerCall) {
          if (live.size) { await Promise.race([...live.values()].map(i => i.work)); continue; }
          last = await step(0); decision = undefined;
          return result();
        }
        last = await step(capacity()); decision = undefined;
        if (last.version !== "automatic_build_step.v1") throw new Error("build_step_invalid");
        await consume();
        if (stop) continue;
        if (last.action.kind === "SPAWN_EXECUTORS") {
          // Engine may replay owned dispatches along with newly available work.
          for (const executor of last.action.executors) {
            if (signal.aborted || capacity() < 1) break;
            launch(executor);
          }
          if (ready.length) continue;
          if (live.size) { await Promise.race([...live.values()].map(i => i.work)); continue; }
          return result();
        }
        if (live.size) {
          drainBoundary = last.action.kind === "NEEDS_USER" || last.action.kind === "DONE";
          await Promise.race([...live.values()].map(i => i.work)); continue;
        }
        if (last.action.kind === "DONE" || last.action.kind === "WAIT") return result();
        if (last.action.kind === "NEEDS_USER") {
          // Re-read the boundary after all terminal observations were persisted.
          last = await step(capacity());
          if (last.action.kind !== "NEEDS_USER") continue;
          const action = last.action;
          if (!action.choices.length) return result();
          const answer = await ctx.userQuestions.ask({ agent, signal, questions: [{ id: action.request_id, question: action.message,
            detail: JSON.stringify(action.projection ?? {}, null, 2), options: action.choices.map(c => ({ label: c.label, description: c.consequence })) }] });
          const choice = action.choices.find(c => exactlySelected(answer, action.request_id, c.label));
          if (!choice || signal.aborted) return result();
          decision = { request_id: action.request_id, choice_id: choice.choice_id };
        }
      }
    } catch (error) {
      if (signal.aborted) return { ...result(), host_observation: { code: "build_cancelled" } };
      throw error;
    } finally {
      // allSettled is essential: one cleanup failure must not skip siblings or observation delivery.
      try {
        await host.dispose();
      } finally {
        await Promise.allSettled([...live.values()].map(i => i.work));
        try { await consume(); } finally {
          for (const item of live.values()) allReservations.delete(item);
          await command({ ...owner, action: "release" });
        }
      }
    }
  }

  function run(agent: Agent, signal: AbortSignal, invocationRef: string) {
    assertRoot(agent);
    if (active.has(invocationRef)) throw new Error("build_already_running");
    const controller = new AbortController();
    const done = drive(agent, AbortSignal.any([signal, controller.signal, shutdown.signal]), invocationRef);
    active.set(invocationRef, { controller, done });
    const settled = () => active.delete(invocationRef);
    void done.then(settled, settled);
    return done;
  }
  return { prepareAndConfirm, run, async dispose() {
    closed = true;
    shutdown.abort();
    for (const item of active.values()) item.controller.abort();
    await Promise.allSettled([...preparations, ...[...active.values()].map(item => item.done)]);
  } };
}
