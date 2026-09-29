import { BUILD_EXECUTOR_SERVER_NAME_V1, BUILD_EXECUTOR_TOOL_NAMES_V1, BUILD_EXECUTOR_TOOL_CONTRACTS_V3, type BuildExecutorToolNameV1 } from "./build-executor-tool-contract";

const SHARED_EXECUTOR_LAUNCHER_ARGS = Object.freeze([
  "/d",
  "/s",
  "/c",
  "scripts\\start-build-executor-mcp.cmd",
] as const);
const SHARED_EXECUTOR_ENV_VARS = Object.freeze([
  "UNDERSTAND_BOOK_BUILD_EXE",
  "UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT",
  "USERPROFILE",
] as const);

export const BUILD_EXECUTOR_MCP_CONTRACT_V3 = Object.freeze({
  version: "build_executor_mcp_contract.v3" as const,
  server_name: BUILD_EXECUTOR_SERVER_NAME_V1,
  registration_scope: "root_shared" as const,
  session_protocol: "automatic_build_executor_session.v3" as const,
  capability_binding: "stdio_connection" as const,
  caller_role_authenticated: false as const,
  child_connection_ownership: "thread_owned_stdio_connection" as const,
  parent_child_connection_shared: false as const,
  tools: BUILD_EXECUTOR_TOOL_CONTRACTS_V3,
});

export interface BuildExecutorSharedMcpConfigValidationV3 {
  status: "compatible";
  server_name: typeof BUILD_EXECUTOR_SERVER_NAME_V1;
  registration_scope: "root_shared";
  required: false;
  default_tools_approval_mode: "approve";
  tool_names: BuildExecutorToolNameV1[];
}

export interface BuildExecutorRegistrationPlacementValidationV3 {
  status: "compatible";
  registration_scope: "root_shared";
  plugin_parent_server_registered: true;
  child_effective_config_inherits_registration: true;
  agent_local_server_registered: false;
  user_config_server_registered: false;
  project_config_server_registered: false;
  child_connection_ownership: "thread_owned_stdio_connection";
  parent_child_connection_shared: false;
  caller_role_authenticated: false;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function parseJsonObject(value: unknown): Record<string, unknown> {
  let parsed = value;
  if (typeof value === "string") {
    try {
      parsed = JSON.parse(value) as unknown;
    } catch {
      throw new Error("Build Executor shared MCP config is not valid JSON");
    }
  }
  if (!isRecord(parsed)) throw new Error("Build Executor shared MCP config must be an object");
  return parsed;
}

function exactArray(value: unknown, expected: readonly string[]): boolean {
  return Array.isArray(value)
    && value.length === expected.length
    && value.every((entry, index) => entry === expected[index]);
}

export function validateBuildExecutorSharedMcpConfigV3(
  pluginMcpJson: unknown,
): BuildExecutorSharedMcpConfigValidationV3 {
  const config = parseJsonObject(pluginMcpJson);
  const servers = config.mcpServers;
  if (!isRecord(servers) || !Object.hasOwn(servers, BUILD_EXECUTOR_SERVER_NAME_V1)) {
    throw new Error("Build Executor shared MCP server is missing from the plugin parent surface");
  }
  const server = servers[BUILD_EXECUTOR_SERVER_NAME_V1];
  if (!isRecord(server)) throw new Error("Build Executor shared MCP server config is invalid");
  if (server.type !== "stdio") throw new Error("Build Executor shared MCP type must be stdio");
  if (server.command !== "cmd.exe"
    || !exactArray(server.args, SHARED_EXECUTOR_LAUNCHER_ARGS)
    || server.cwd !== ".") {
    throw new Error("Build Executor shared MCP launcher must be plugin-root relative");
  }
  if (server.required !== false) throw new Error("Build Executor shared MCP required must be false");
  if (!exactArray(server.enabled_tools, BUILD_EXECUTOR_TOOL_NAMES_V1)) {
    throw new Error("Build Executor shared MCP enabled_tools must be the exact four-tool inventory");
  }
  if (server.default_tools_approval_mode !== "approve") {
    throw new Error("Build Executor shared MCP approval mode must be approve");
  }
  if (server.startup_timeout_sec !== 10 || server.tool_timeout_sec !== 120) {
    throw new Error("Build Executor shared MCP timeout surface is incompatible");
  }
  if (!exactArray(server.env_vars, SHARED_EXECUTOR_ENV_VARS)) {
    throw new Error("Build Executor shared MCP environment surface is incompatible");
  }
  return {
    status: "compatible",
    server_name: BUILD_EXECUTOR_SERVER_NAME_V1,
    registration_scope: "root_shared",
    required: false,
    default_tools_approval_mode: "approve",
    tool_names: [...BUILD_EXECUTOR_TOOL_NAMES_V1],
  };
}

function containsExecutorMcpRegistration(toml: string): boolean {
  return /^\[mcp_servers\.understand_book_build_executor\]\s*$/mu.test(
    toml.replace(/\r\n?/gu, "\n"),
  );
}

export function validateBuildExecutorRegistrationPlacementV3(input: {
  plugin_mcp_json: unknown;
  agent_toml: string;
  user_config_toml?: string;
  project_config_toml?: string;
}): BuildExecutorRegistrationPlacementValidationV3 {
  validateBuildExecutorSharedMcpConfigV3(input.plugin_mcp_json);
  if (containsExecutorMcpRegistration(input.agent_toml)) {
    throw new Error("Build Executor role placement must not contain a duplicate MCP transport");
  }
  if (containsExecutorMcpRegistration(input.user_config_toml ?? "")) {
    throw new Error("Build Executor user config contains a duplicate plugin transport");
  }
  if (containsExecutorMcpRegistration(input.project_config_toml ?? "")) {
    throw new Error("Build Executor project config contains a duplicate plugin transport");
  }
  return {
    status: "compatible",
    registration_scope: "root_shared",
    plugin_parent_server_registered: true,
    child_effective_config_inherits_registration: true,
    agent_local_server_registered: false,
    user_config_server_registered: false,
    project_config_server_registered: false,
    child_connection_ownership: "thread_owned_stdio_connection",
    parent_child_connection_shared: false,
    caller_role_authenticated: false,
  };
}

export function validateBuildExecutorRegistrationScope(input: {
  surface: "agent" | "root" | "project";
  server_names: readonly string[];
}): void {
  const registered = input.server_names.includes(BUILD_EXECUTOR_SERVER_NAME_V1);
  if (input.surface === "agent") {
    if (!registered) throw new Error("agent-only Build Executor server registration is missing");
    return;
  }
  if (registered) {
    throw new Error(`agent-only Build Executor server must not be registered on ${input.surface}`);
  }
}

