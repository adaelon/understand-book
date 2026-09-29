import { runAutomaticBuildExecutorSessionCommand, type AutomaticBuildExecutorSessionResponse } from "./automatic-build-executor-session";
import { BUILD_EXECUTOR_TOOL_NAMES_V1, BUILD_EXECUTOR_TOOL_CONTRACTS_V3, validateClosedToolRequest, type BuildExecutorToolNameV1, type BuildExecutorToolContractV1 } from "./build-executor-tool-contract";
export { BUILD_EXECUTOR_TOOL_NAMES_V1 } from "./build-executor-tool-contract";
export type { BuildExecutorToolNameV1, BuildExecutorToolContractV1 } from "./build-executor-tool-contract";
export * from "./codex-build-executor-contract";
type BuildExecutorResponse = AutomaticBuildExecutorSessionResponse;

export function createBuildExecutorToolAdapter(input: {
  session_protocol?: BuildExecutorResponse["version"];
  authorize_connection: (
    capability: unknown,
    call: { tool_name: BuildExecutorToolNameV1; request: unknown },
  ) => boolean;
  execute_request?: (request: unknown) => BuildExecutorResponse;
}): {
  list_tools: () => readonly BuildExecutorToolContractV1[];
  call_tool: (
    toolName: BuildExecutorToolNameV1,
    request: unknown,
    connectionCapability?: unknown,
  ) => BuildExecutorResponse;
} {
  if (typeof input.authorize_connection !== "function") {
    throw new Error("Build Executor adapter requires a child connection authorizer");
  }
  const executeRequest = input.execute_request ?? runAutomaticBuildExecutorSessionCommand;
  return Object.freeze({
    list_tools: () => BUILD_EXECUTOR_TOOL_CONTRACTS_V3,
    call_tool: (
      toolName: BuildExecutorToolNameV1,
      request: unknown,
      connectionCapability?: unknown,
    ): BuildExecutorResponse => {
      if (!(BUILD_EXECUTOR_TOOL_NAMES_V1 as readonly string[]).includes(toolName)) {
        throw new Error("Build Executor tool is unsupported");
      }
      validateClosedToolRequest(toolName, request);
      // Precise connection-open errors pass through to MCP without becoming a protocol error.
      if (!input.authorize_connection(connectionCapability, { tool_name: toolName, request })) {
        throw new Error("Build Executor child connection capability is missing or invalid");
      }
      const response = executeRequest(request);
      if (response.version !== (input.session_protocol ?? "automatic_build_executor_session.v3")) {
        throw new Error("Build Executor MCP accepts only V3 session responses");
      }
      return response;
    },
  });
}
