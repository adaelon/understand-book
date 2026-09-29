import type { Context } from "@deepseek-ai/cordis";
import type {} from "@deepseek-ai/dsh-agent";
import type {} from "@deepseek-ai/dsh-agent-preset-registry";
import type {} from "@deepseek-ai/dsh-subagent";
import type {} from "@deepseek-ai/dsh-tools";
import type {} from "@deepseek-ai/dsh-user-questions";

/** Runtime prerequisites only. Installation and model-visible probes are separate evidence. */
export function checkHarnessCapabilities(ctx: Context) {
  const missing: string[] = [];
  for (const service of ["agents", "subagents", "tools", "agentPresets", "userQuestions"] as const) {
    if (!ctx.get(service)) missing.push(service);
  }
  const provider = ctx.get("subagents")?.getProvider("spawn");
  if (!provider) missing.push("spawn");
  else {
    if (provider.inheritsParentContext) missing.push("fresh_context");
    for (const feature of ["agentOptions", "depthLimit", "toolFilter", "persona"] as const) {
      if (!provider.capabilities[feature]) missing.push(`spawn.${feature}`);
    }
  }
  return { status: missing.length ? "unsupported" as const : "available" as const, missing };
}
