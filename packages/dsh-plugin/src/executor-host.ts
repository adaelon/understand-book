import { AsyncLocalStorage } from "node:async_hooks";
import type { Context } from "@deepseek-ai/cordis";
import type { Agent } from "@deepseek-ai/dsh-agent";
import type { SubagentRun } from "@deepseek-ai/dsh-subagent";
import type {} from "@deepseek-ai/dsh-agent-preset-registry";
import type {} from "@deepseek-ai/dsh-tools";
import { readBuildExecutorModelRuntime, type BuildExecutorModelRuntimeV1 } from "../../core/src/build-executor-model-runtime.ts";
import { executorAgentOptions } from "./executor-runtime.ts";
import { installExecutorModelBoundary } from "./executor-model-boundary.ts";
import { EXECUTOR_ROLE } from "./executor-role.ts";

export interface ExecutorLaunch {
  launch_id?: string;
  dispatch_slot_ref?: string;
  parent: Agent;
  signal: AbortSignal;
  opaque_handoff_ref: string;
  runtime: Readonly<BuildExecutorModelRuntimeV1>;
}

export interface ExecutorBinding {
  readonly launch: ExecutorLaunch;
  agent?: Agent;
  active: boolean;
  failure?: "model_context_limit" | "model_configuration_changed";
  assertCaller(caller: Agent | undefined): void;
}

/** Associate native spawn creation with one launch before the child's first prompt. */
export function createExecutorHost(
  ctx: Context,
  presetId: string,
  prepare: (binding: ExecutorBinding) => Promise<void | (() => Promise<void>)>,
) {
  const creating = new AsyncLocalStorage<ExecutorBinding>();
  const runs = new Set<SubagentRun>();
  const pending = new Set<Promise<unknown>>();
  const cancellations = new Set<AbortController>();
  const resources = new Map<ExecutorBinding, () => Promise<void>>();
  async function disposeResource(binding: ExecutorBinding) {
    const cleanup = resources.get(binding);
    if (cleanup) { try { await cleanup(); } finally { resources.delete(binding); } }
  }
  let closed = false;
  let hostDisposal: Promise<void> | undefined;
  const removeListener = ctx.on("agent/created", async ({ agent }) => {
    const binding = creating.getStore();
    if (!binding) return;
    if (closed || !binding.active || binding.agent || ctx.agents.get(agent.id) !== agent
      || !ctx.agents.isOwnedBy(agent.id, binding.launch.parent)) {
      throw new Error("executor_creation_binding_invalid");
    }
    binding.agent = agent;
    await ctx.agentPresets.select(agent, presetId);
    agent.ctx.tools.restrict({ allow: [] });
    agent.ctx.tools.presentAs("native");
    installExecutorModelBoundary(binding);
    const cleanup = await prepare(binding);
    if (cleanup) resources.set(binding, cleanup);
  });

  async function launchOwned(input: ExecutorLaunch, signal: AbortSignal) {
    if (closed || input.signal.aborted) throw new Error("executor_host_closed");
    if (!/^abhandoff1_[a-f0-9]{64}$/u.test(input.opaque_handoff_ref)) {
      throw new Error("executor_handoff_invalid");
    }
    const binding: ExecutorBinding = {
      launch: input, active: true,
      assertCaller(caller) {
        if (!binding.active || !caller || caller !== binding.agent
          || ctx.agents.get(caller.id) !== caller || !ctx.agents.isOwnedBy(caller.id, input.parent)) {
          throw new Error("executor_caller_not_owned");
        }
      },
    };
    let run: SubagentRun;
    try {
      run = await creating.run(binding, () => ctx.subagents.start("spawn", {
        parent: input.parent,
        signal,
        agentOptions: executorAgentOptions(input.runtime),
        persona: EXECUTOR_ROLE,
        prompt: [{ type: "text", text: input.opaque_handoff_ref }],
      }));
    } catch (error) {
      binding.active = false;
      await disposeResource(binding);
      throw error;
    }
    if (!binding.agent || run.localAgent !== binding.agent || closed) {
      binding.active = false;
      void run.result.catch(() => undefined);
      try { await run.dispose(); } finally { await disposeResource(binding); }
      throw new Error("executor_published_binding_invalid");
    }
    runs.add(run);
    // Never return the model's final text as a parent-facing result.
    const terminal = run.result.then(
      (result) => ({ stop_reason: result.stopReason, ...(binding.failure ? { diagnostic_code: binding.failure } : {}) }),
      () => ({ stop_reason: "host_error" as const, ...(binding.failure ? { diagnostic_code: binding.failure } : {}) }),
    ).finally(() => { binding.active = false; });
    let disposal: Promise<void> | undefined;
    return {
      child_id: run.id,
      binding,
      terminal,
      dispose: () => disposal ??= (async () => {
        binding.active = false;
        try { await run.dispose(); } finally { runs.delete(run); await disposeResource(binding); }
      })(),
    };
  }

  function launch(input: ExecutorLaunch) {
    const launch = Object.freeze({ ...input, runtime: readBuildExecutorModelRuntime(input.runtime) });
    const cancellation = new AbortController();
    cancellations.add(cancellation);
    const signal = AbortSignal.any([launch.signal, cancellation.signal]);
    const task = launchOwned({ ...launch, signal }, signal);
    pending.add(task);
    const settled = () => { pending.delete(task); cancellations.delete(cancellation); };
    void task.then(settled, settled);
    return task;
  }

  return {
    launch,
    dispose() {
      return hostDisposal ??= (async () => {
        closed = true;
        for (const cancellation of cancellations) cancellation.abort();
        // A rejected launch already owns unpublished rollback. Await that boundary as well
        // as all published handles before declaring the host disposed.
        const results = await Promise.allSettled([...runs].map(run => run.dispose()));
        await Promise.allSettled([...pending]);
        const resourceResults = await Promise.allSettled([...resources.keys()].map(disposeResource));
        removeListener();
        runs.clear();
        const failed = [...results, ...resourceResults].find(r => r.status === "rejected");
        if (failed?.status === "rejected") throw failed.reason;
      })();
    },
  };
}
