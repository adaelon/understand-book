import { Context } from "@deepseek-ai/cordis";
import Loader from "@deepseek-ai/cordis-plugin-loader";
import AgentRegistry from "@deepseek-ai/dsh-agent";
import AgentLoop from "@deepseek-ai/dsh-agent-loop";
import Presets from "@deepseek-ai/dsh-agent-preset-registry";
import LlmRuntime, { LlmAdapter, ToolCallId, type GenerateOptions, type StreamChunk } from "@deepseek-ai/dsh-llm";
import Sessions, { SessionId } from "@deepseek-ai/dsh-session";
import Projections from "@deepseek-ai/dsh-session-projection";
import SystemPrompt from "@deepseek-ai/dsh-system-prompt";
import Tools from "@deepseek-ai/dsh-tools";
import Subagents from "@deepseek-ai/dsh-subagent";
import * as Spawn from "@deepseek-ai/dsh-subagent-spawn-in-process";
import UserQuestions from "@deepseek-ai/dsh-user-questions";
import type { BuildExecutorModelRuntimeV1 } from "../../core/src/build-executor-model-runtime.ts";

/** Synthetic capacity for deterministic host tests; not a provider capacity claim. */
export function probeRuntime(overrides: Partial<BuildExecutorModelRuntimeV1> = {}): BuildExecutorModelRuntimeV1 {
  return { version: "build_executor_model_runtime.v1", provider: "probe", model: "scripted", reasoning_effort: null,
    max_output_tokens: 1000, context_window_tokens: 1000000, safety_margin_tokens: 100,
    context_source: "provider_model_metadata", ...overrides };
}

export function textResponse(text: string): StreamChunk[] {
  return [
    { type: "block-start", index: 0, blockType: "text" },
    { type: "text-delta", index: 0, text },
    { type: "block-end", index: 0, block: { type: "text", text } },
    { type: "finish", reason: { kind: "stop" } },
  ];
}

export function toolResponse(id: string, name: string, args: object): StreamChunk[] {
  const callId = ToolCallId(id);
  const argumentsJson = JSON.stringify(args);
  return [
    { type: "block-start", index: 0, blockType: "tool-call" },
    { type: "tool-call-delta", index: 0, id: callId, name, argumentsDelta: argumentsJson },
    { type: "block-end", index: 0, block: { type: "tool-call", id: callId, name, arguments: argumentsJson } },
    { type: "finish", reason: { kind: "tool-calls" } },
  ];
}

export class ScriptedModel extends LlmAdapter {
  requests: GenerateOptions[] = [];
  constructor(readonly script: (request: GenerateOptions) => AsyncIterable<StreamChunk>) { super(); }
  async resolveModel(provider: string, model: string) { return { provider, id: model, name: model }; }
  async *stream(request: GenerateOptions) {
    this.requests.push(request);
    yield* this.script(request);
  }
}

export async function harness(model = new ScriptedModel(async function* () { yield* textResponse("final-sentinel"); })) {
  const ctx = new Context();
  await ctx.plugin(Loader);
  await ctx.plugin(LlmRuntime);
  await ctx.plugin(Sessions);
  await ctx.plugin(Projections);
  await ctx.plugin(SystemPrompt, { personaPrefix: "" });
  await ctx.plugin(Tools, { mode: "both" });
  await ctx.plugin(AgentRegistry);
  await ctx.plugin(AgentLoop, { agents: [] });
  await ctx.plugin(Presets, { default: "standard" });
  await ctx.agentPresets.register({ id: "standard", plugins: [] });
  await ctx.agentPresets.register({ id: "understand-book-executor", plugins: [] });
  await ctx.plugin(Subagents);
  await ctx.plugin(Spawn, { providerName: "spawn" });
  await ctx.plugin(UserQuestions);
  ctx.llm.registerAdapter(["probe"], model);
  const parentHandle = await ctx.agents.create({
    sessionId: SessionId("dh0-parent"),
    agentOptions: { provider: "probe", model: "scripted" },
    setup: async agentCtx => { await ctx.agentPresets.mount(agentCtx, "standard"); },
  });
  return { ctx, parent: parentHandle.agent, model, dispose: () => ctx.fiber.dispose() };
}
