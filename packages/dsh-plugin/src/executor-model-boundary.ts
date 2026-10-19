import { randomUUID } from "node:crypto";
import type { GenerateOptions } from "@deepseek-ai/dsh-llm";
import { estimateTokens } from "../../core/src/window.ts";
import type { ExecutorBinding } from "./executor-host.ts";

/** Include accumulated messages, role text and tool schemas, not just the latest chunk. */
export function measureExecutorContext(request: GenerateOptions): number {
  return estimateTokens(JSON.stringify({ system: request.system, messages: request.messages,
    tools: request.tools, toolHistory: request.toolHistory, stop: request.stop }));
}

/** Scoped middleware rejects before the provider call; it never truncates or rewrites input. */
export function installExecutorModelBoundary(binding: ExecutorBinding): void {
  const agent = binding.agent;
  if (!agent) throw new Error("executor_creation_binding_invalid");
  const { provider, model, reasoning_effort, max_output_tokens, context_window_tokens, safety_margin_tokens } = binding.launch.runtime;
  const inputCap = context_window_tokens - safety_margin_tokens - max_output_tokens;
  agent.ctx.on("llm/stream", async function* (request, next) {
    if (request.sessionId !== agent.id) { yield* next(); return; }
    binding.assertCaller(agent);
    if (request.provider !== provider || request.model !== model
      || (request.reasoningEffort ?? null) !== reasoning_effort || request.maxTokens !== max_output_tokens) {
      binding.failure = "model_configuration_changed";
      throw new Error(binding.failure);
    }
    if (measureExecutorContext(request) > inputCap) {
      binding.failure = "model_context_limit";
      throw new Error(binding.failure);
    }
    const cost = binding.modelCost;
    let call: string | undefined;
    if (cost) {
      try {
        call = await cost.ledger.admit(randomUUID(), `${cost.logicalId}:call:${binding.costObservation.model_calls}`, provider, model, "dsh_native_v4", {
          execution_id:cost.executionId, request:{model,max_tokens:request.maxTokens,input_tokens_estimate:measureExecutorContext(request)},
        });
      } catch (error) { cost.blocked(); throw error; }
    }
    binding.costObservation.model_calls++;
    let usage: Record<string,unknown> | undefined, succeeded = false;
    try {
      for await (const chunk of next()) {
        if (chunk.type === "usage") { usage = {...chunk.usage}; binding.costObservation.usages.push(usage); }
        if (chunk.type === "finish") succeeded = !["error","aborted"].includes(chunk.reason.kind);
        yield chunk;
      }
    } finally {
      if (cost && call) {
        try { await cost.ledger.settleCall(call, usage, succeeded); }
        catch (error) { cost.blocked(); throw error; }
      }
    }
  });
}
