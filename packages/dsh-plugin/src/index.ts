import type { Context } from "@deepseek-ai/cordis";
import z from "@deepseek-ai/schemastery";
import { createBuildControl } from "./build-control.ts";
import { resolvePluginConfig, type PluginConfig } from "./config.ts";
import { connectExecutor, engineCommand } from "./engine-client.ts";
import { DSH_BUILD_CONTROL_CONTRACT_V1 } from "../../core/src/dsh-build-executor-contract.ts";

export const name = "understand-book-build";
export const inject = ["agents", "subagents", "tools", "agentPresets", "userQuestions", "llm"];
export type Config = PluginConfig;
export const Config = z.object({
  executable: z.string().description("Build Engine executable (absolute path); empty uses the Reader installation").default(""),
  driverRoot: z.string().description("Shared Engine driver registry (absolute path); empty uses the Engine default").default(""),
  maxOutputTokens: z.number().default(4096),
  safetyMarginTokens: z.number().default(4096),
  handoffsPerCall: z.number().default(8),
});

/** Dedicated empty child composition excludes compaction, pruning and inherited skills. */
export async function apply(ctx: Context, input: Config = {}) {
  const config = resolvePluginConfig(input);
  const startup = new AbortController();
  ctx.effect(() => () => startup.abort());
  // Initialization negotiates the packaged contract without claiming a handoff.
  try {
    const signal = AbortSignal.any([startup.signal, AbortSignal.timeout(10000)]);
    const capabilities = await engineCommand(config, ["build.step"], { version: "dsh_build_capabilities.v1" }, signal);
    for (const [key, value] of Object.entries(DSH_BUILD_CONTROL_CONTRACT_V1)) {
      if ((capabilities as Record<string, unknown>)?.[key] !== value) throw new Error("build_engine_incompatible");
    }
    const connection = await connectExecutor(config, signal);
    await connection.dispose();
  } catch { throw new Error("build_engine_incompatible"); }
  const presetId = "understand-book-build-executor";
  const unregisterPreset = await ctx.agentPresets.register({ id: presetId, plugins: [] });
  const control = createBuildControl(ctx, config, presetId);
  ctx.effect(() => async () => {
    try { await control.dispose(); } finally { await unregisterPreset(); }
  });
  ctx.tools.register({ name: "ub_build_prepare_and_confirm", description: "Prepare an exact standard book build plan and ask the human to approve it. Returns a durable invocation reference; does not start executors.",
    parameters: { type: "object", properties: { target_input: { type: "string" }, root_dir: { type: "string" }, pass2: { type: "string", enum: ["enabled", "disabled"] }, max_parallel: { type: "integer", minimum: 1, maximum: 3 },
      budget: { type: "object", properties: { on_exceed: { type: "string", enum: ["needs_user"] }, max_total_tokens: { type: "integer", minimum: 0 }, max_wall_clock_minutes: { type: "number", minimum: 0 } }, required: ["on_exceed"], additionalProperties: false } },
      required: ["target_input", "root_dir", "pass2"], additionalProperties: false },
    output: { schema: {}, render: (_args, value) => [{ type: "text", text: JSON.stringify(value) }] },
    async execute(args: any, exec) {
      if (!exec.agent) throw new Error("build_root_required");
      return control.prepareAndConfirm(exec.agent, exec.signal, args);
    },
  });
  ctx.tools.register({ name: "ub_build_run", description: "Continue an approved Understand Book invocation in the foreground. Resume using the same reference after a bounded yield or interruption.",
    parameters: { type: "object", properties: { invocation_ref: { type: "string", pattern: "^abinv1_[a-f0-9]{64}$" } }, required: ["invocation_ref"], additionalProperties: false },
    output: { schema: {}, render: (_args, value) => [{ type: "text", text: JSON.stringify(value) }] },
    async execute(args: any, exec) {
      if (!exec.agent || Object.keys(args).length !== 1) throw new Error("build_root_required");
      return control.run(exec.agent, exec.signal, args.invocation_ref);
    },
  });
}
