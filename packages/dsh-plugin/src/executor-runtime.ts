import type { Context } from "@deepseek-ai/cordis";
import type { AgentOptions } from "@deepseek-ai/dsh-agent";
import { ReasoningEffortId } from "@deepseek-ai/dsh-llm";
import { readBuildExecutorModelRuntime, type BuildExecutorModelRuntimeV1 } from "../../core/src/build-executor-model-runtime.ts";

/** Resolve host defaults once, before confirmation; never use the parent's later route. */
export async function resolveExecutorRuntime(ctx: Context, options: AgentOptions,
  safetyMarginTokens: number, signal?: AbortSignal): Promise<Readonly<BuildExecutorModelRuntimeV1>> {
  const { provider, model, maxTokens, reasoningEffort } = options;
  if (!provider || !model || !Number.isSafeInteger(maxTokens) || maxTokens! < 1) {
    throw new Error("executor_model_budget_invalid");
  }
  const selected = await ctx.llm.resolveCallConfig({ provider, model, maxTokens, reasoningEffort }, signal);
  const info = await ctx.llm.resolveModelInfo(provider, model, signal);
  if (info.provider !== provider || info.id !== model || !info.context) {
    throw new Error("executor_model_context_unknown");
  }
  return readBuildExecutorModelRuntime({
    version: "build_executor_model_runtime.v1", provider, model,
    reasoning_effort: selected.reasoningEffort ?? null,
    max_output_tokens: maxTokens,
    context_window_tokens: info.context.contextWindow,
    safety_margin_tokens: safetyMarginTokens,
    context_source: "provider_model_metadata",
  });
}

export function executorAgentOptions(runtime: Readonly<BuildExecutorModelRuntimeV1>): AgentOptions {
  return { provider: runtime.provider, model: runtime.model, maxTokens: runtime.max_output_tokens,
    ...(runtime.reasoning_effort === null ? {} : { reasoningEffort: ReasoningEffortId(runtime.reasoning_effort) }) };
}
