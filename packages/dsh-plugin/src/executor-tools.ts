import {
  BUILD_EXECUTOR_TOOL_CONTRACTS_V3,
  validateClosedToolRequest,
  type BuildExecutorToolNameV1,
} from "../../core/src/build-executor-tool-contract.ts";
import type { ExecutorBinding } from "./executor-host.ts";

export const DSH_EXECUTOR_TOOL_NAMES = {
  "executor.open": "ub_executor_open",
  "executor.input.next": "ub_executor_input_next",
  "executor.generation.start": "ub_executor_generation_start",
  "executor.submit_candidate": "ub_executor_submit_candidate",
} as const satisfies Record<BuildExecutorToolNameV1, string>;

/** The host provides the caller identity; model arguments never select a connection. */
export function installExecutorTools(
  binding: ExecutorBinding,
  call: (name: BuildExecutorToolNameV1, request: unknown, signal: AbortSignal) => Promise<unknown>,
): void {
  const agent = binding.agent;
  if (!agent) throw new Error("executor_creation_binding_invalid");
  for (const tool of BUILD_EXECUTOR_TOOL_CONTRACTS_V3) {
    agent.ctx.tools.register({
      name: DSH_EXECUTOR_TOOL_NAMES[tool.name],
      description: tool.description,
      parameters: { ...tool.input_schema },
      output: {
        schema: {},
        render: (_args, value) => [{ type: "text", text: JSON.stringify(value) }],
      },
      async execute(request, exec) {
        binding.assertCaller(exec.agent);
        validateClosedToolRequest(tool.name, request);
        return call(tool.name, request, exec.signal);
      },
    });
  }
}
