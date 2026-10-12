import path from "node:path";
import { fileURLToPath } from "node:url";
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3, BuildExecutorConnectionOpenError, createBuildExecutorStdioConnectionCapability, validateBuildExecutorRoleConfigV3 } from "../../packages/core/src/build-executor-connection-capability";
import { BUILD_EXECUTOR_MCP_CONTRACT_V3, validateBuildExecutorSharedMcpConfigV3 } from "../../packages/core/src/codex-build-executor-contract";
import { CODEX_EXECUTOR_TRANSPORT_PROFILE_V2 } from "../../packages/core/src/executor-transport";
const PLUGIN_ROOT = process.env.UNDERSTAND_BOOK_PLUGIN_ROOT
  ? path.resolve(process.env.UNDERSTAND_BOOK_PLUGIN_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

const AUTOMATIC_BUILD_DOCTOR_POLICY_EVIDENCE_FORBIDDEN_FIELDS = [
  "current_policy_digest",
  "current_proof_digest",
  "current_route_digest",
  "evidence_digest",
  "file_sha256",
  "preflight_evaluation_digest",
  "proof_digest",
  "policy_digest",
  "policy_set_digest",
  "receipt_digest",
  "resolution_digest",
] as const;

const AUTOMATIC_BUILD_DOCTOR_TRANSPORT_FORBIDDEN_FIELDS = [
  "delivery_ledger_digest",
  "output_contract_digest",
  "pack_digest",
  "payload_sha256",
  "profile_digest",
  "serialized_response_sha256",
  "transport_profile_digest",
] as const;

interface AutomaticBuildProtocolDoctorBoundaryInputV3 {
  agent_template: string;
  plugin_mcp_projections: readonly string[];
  launcher_projections: readonly string[];
  release_contract: unknown;
}

function isDoctorRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function countOwnDoctorFields(
  value: unknown,
  fields: readonly string[],
): number {
  if (!isDoctorRecord(value)) return 0;
  return fields.filter((field) => Object.hasOwn(value, field)).length;
}

function validateDirectTextProjections(projections: readonly string[], label: string): string {
  if (projections.length === 0 || projections.some((projection) => projection.length === 0)) {
    throw new Error(`${label} projection is missing`);
  }
  const canonical = projections[0];
  if (projections.some((projection) => projection !== canonical)) {
    throw new Error(`${label} projections differ by direct text comparison`);
  }
  return canonical;
}

function validateBuildExecutorLauncherV3(text: string): void {
  const normalizedLines = text.replace(/\r\n?/gu, "\n").split("\n").map((line) => line.trim());
  const expectedCommand = `"%BUILD_EXECUTOR_BIN%" executor.mcp --bootstrap-version ${
    BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.version
  } --protocol-generation ${BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.session_protocol}`;
  if (normalizedLines.filter((line) => line === expectedCommand).length !== 1
    || normalizedLines.some((line) => line.includes("--agent-bootstrap-digest"))) {
    throw new Error("Build Executor launcher bootstrap or session protocol is incompatible");
  }
}

function releasePolicySets(releaseContract: unknown): Record<string, unknown>[] {
  if (!isDoctorRecord(releaseContract)
    || releaseContract.status !== "compatible"
    || !Array.isArray(releaseContract.policy_sets)) {
    return [];
  }
  return releaseContract.policy_sets.filter(isDoctorRecord);
}

function releasePolicyMembers(policySets: readonly Record<string, unknown>[]): Record<string, unknown>[] {
  return policySets.flatMap((policySet) => (
    Array.isArray(policySet.members) ? policySet.members.filter(isDoctorRecord) : []
  ));
}

export function validateAutomaticBuildProtocolDoctorBoundaryV3(
  input: AutomaticBuildProtocolDoctorBoundaryInputV3,
) {
  let executorRole:
    | {
      status: "compatible";
      agent_name: "understand_book_executor";
      mcp_servers_in_role: 0;
    }
    | { status: "incompatible"; diagnostic_code: "executor_role_incompatible" };
  try {
    const role = validateBuildExecutorRoleConfigV3(input.agent_template);
    executorRole = {
      status: "compatible",
      agent_name: role.agent_name,
      mcp_servers_in_role: role.mcp_servers_in_role,
    };
  } catch {
    executorRole = { status: "incompatible", diagnostic_code: "executor_role_incompatible" };
  }

  let sharedExecutorMcp:
    | {
      status: "compatible";
      registration_scope: "root_shared";
      bootstrap_version: "automatic_build_executor_bootstrap.v3";
      session_protocol: "automatic_build_executor_session.v3";
      required: false;
      default_tools_approval_mode: "approve";
      executor_tool_count: 4;
    }
    | { status: "incompatible"; diagnostic_code: "shared_executor_mcp_incompatible" };
  try {
    if (input.plugin_mcp_projections.length !== input.launcher_projections.length) {
      throw new Error("Build Executor config and launcher projection counts differ");
    }
    validateDirectTextProjections(input.plugin_mcp_projections, "Build Executor MCP config");
    const launcher = validateDirectTextProjections(
      input.launcher_projections,
      "Build Executor launcher",
    );
    const sharedConfigs = input.plugin_mcp_projections.map((projection) => (
      validateBuildExecutorSharedMcpConfigV3(projection)
    ));
    validateBuildExecutorLauncherV3(launcher);
    const shared = sharedConfigs[0];
    if (!shared
      || shared.registration_scope !== BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.registration_scope
      || shared.tool_names.length !== BUILD_EXECUTOR_MCP_CONTRACT_V3.tools.length
      || shared.tool_names.some((toolName, index) => (
        toolName !== BUILD_EXECUTOR_MCP_CONTRACT_V3.tools[index]?.name
      ))) {
      throw new Error("Build Executor shared MCP identity is incompatible");
    }
    sharedExecutorMcp = {
      status: "compatible",
      registration_scope: shared.registration_scope,
      bootstrap_version: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.version,
      session_protocol: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.session_protocol,
      required: shared.required,
      default_tools_approval_mode: shared.default_tools_approval_mode,
      executor_tool_count: 4,
    };
  } catch {
    sharedExecutorMcp = {
      status: "incompatible",
      diagnostic_code: "shared_executor_mcp_incompatible",
    };
  }

  const policySets = releasePolicySets(input.release_contract);
  const members = releasePolicyMembers(policySets);
  const policyEvidenceObjects = [input.release_contract, ...policySets, ...members];
  const toolSchemaObjects = BUILD_EXECUTOR_MCP_CONTRACT_V3.tools.flatMap((tool) => [
    tool,
    tool.input_schema,
    tool.input_schema.properties,
  ]);
  const forbiddenDigestFieldCount = countOwnDoctorFields(
    BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3,
    ["bootstrap_digest"],
  ) + countOwnDoctorFields(
    CODEX_EXECUTOR_TRANSPORT_PROFILE_V2,
    AUTOMATIC_BUILD_DOCTOR_TRANSPORT_FORBIDDEN_FIELDS,
  ) + toolSchemaObjects.reduce<number>((count, value) => count + countOwnDoctorFields(
    value,
    AUTOMATIC_BUILD_DOCTOR_TRANSPORT_FORBIDDEN_FIELDS,
  ), 0) + policyEvidenceObjects.reduce<number>((count, value) => count + countOwnDoctorFields(
    value,
    AUTOMATIC_BUILD_DOCTOR_POLICY_EVIDENCE_FORBIDDEN_FIELDS,
  ), 0);
  let connectionIntegrity:
    | {
      status: "compatible";
      model_parameter: false;
      caller_role_authenticated: false;
      cross_handoff_rejected: true;
      session_private_root_bound: true;
      forbidden_digest_field_count: 0;
    }
    | {
      status: "incompatible";
      diagnostic_code: "connection_integrity_incompatible";
      forbidden_digest_field_count: number;
    };
  try {
    const connection = createBuildExecutorStdioConnectionCapability({
      bootstrap_version: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.version,
      protocol_generation: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.session_protocol,
      session_private_root: path.resolve(PLUGIN_ROOT, ".automatic-build-executor-private"),
    });
    const firstOpen = {
      tool_name: "executor.open" as const,
      request: {
        version: "automatic_build_executor_open_request.v3",
        opaque_handoff_ref: `abhandoff1_${"a".repeat(64)}`,
      },
    };
    const crossHandoffOpen = {
      tool_name: "executor.open" as const,
      request: {
        version: "automatic_build_executor_open_request.v3",
        opaque_handoff_ref: `abhandoff1_${"b".repeat(64)}`,
      },
    };
    let relativeRootRejected = false;
    try {
      createBuildExecutorStdioConnectionCapability({
        bootstrap_version: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.version,
        protocol_generation: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.session_protocol,
        session_private_root: "relative-root-is-not-bound",
      });
    } catch {
      relativeRootRejected = true;
    }
    const serializedToolContract = JSON.stringify(BUILD_EXECUTOR_MCP_CONTRACT_V3);
    const firstOpenAccepted = connection.authorize_connection(connection.connection_capability, firstOpen);
    let crossHandoffRejected = false;
    try {
      connection.authorize_connection(connection.connection_capability, crossHandoffOpen);
    } catch (error) {
      crossHandoffRejected = error instanceof BuildExecutorConnectionOpenError
        && error.diagnostic_code === "handoff_ref_mismatch";
    }
    if (connection.authorize_connection(Symbol("root"), firstOpen)
      || !firstOpenAccepted
      || !crossHandoffRejected
      || JSON.stringify(connection.connection_capability) !== undefined
      || !relativeRootRejected
      || BUILD_EXECUTOR_MCP_CONTRACT_V3.caller_role_authenticated !== false
      || serializedToolContract.includes("connection_capability")
      || serializedToolContract.includes("session_private_root")
      || forbiddenDigestFieldCount !== 0) {
      throw new Error("Build Executor connection integrity evidence is incompatible");
    }
    connectionIntegrity = {
      status: "compatible",
      model_parameter: false,
      caller_role_authenticated: false,
      cross_handoff_rejected: true,
      session_private_root_bound: true,
      forbidden_digest_field_count: 0,
    };
  } catch {
    connectionIntegrity = {
      status: "incompatible",
      diagnostic_code: "connection_integrity_incompatible",
      forbidden_digest_field_count: forbiddenDigestFieldCount,
    };
  }

  const policyGenerationIsExplicit = members.length > 0 && members.every((member) => (
    typeof member.policy_generation_id === "string"
    && member.policy_generation_id.length > 0
    && isDoctorRecord(member.semantic_contract)
    && typeof member.semantic_contract.prompt_sha256 === "string"
    && /^[a-f0-9]{64}$/u.test(member.semantic_contract.prompt_sha256)
  ));
  const largeContentHashConsumersPresent = members.length > 0 && members.every((member) => (
    typeof member.prompt_sha256 === "string"
    && /^[a-f0-9]{64}$/u.test(member.prompt_sha256)
    && typeof member.rendered_input_sha256 === "string"
    && /^[a-f0-9]{64}$/u.test(member.rendered_input_sha256)
  ));
  const semanticIdentityForbiddenFieldCount = policyEvidenceObjects.reduce<number>(
    (count, value) => count + countOwnDoctorFields(
      value,
      AUTOMATIC_BUILD_DOCTOR_POLICY_EVIDENCE_FORBIDDEN_FIELDS,
    ),
    0,
  );
  const budgetProofIsFreshnessIdentity = semanticIdentityForbiddenFieldCount > 0;
  const semanticReuseIdentity = !budgetProofIsFreshnessIdentity
    && policyGenerationIsExplicit
    && largeContentHashConsumersPresent
    ? {
      status: "compatible" as const,
      budget_proof_is_freshness_identity: false as const,
      policy_generation_is_explicit: true as const,
      large_content_hash_consumers_present: true as const,
    }
    : {
      status: "incompatible" as const,
      diagnostic_code: "semantic_reuse_identity_incompatible" as const,
      budget_proof_is_freshness_identity: budgetProofIsFreshnessIdentity,
      policy_generation_is_explicit: policyGenerationIsExplicit,
      large_content_hash_consumers_present: largeContentHashConsumersPresent,
    };

  const checks = {
    executor_role: executorRole,
    shared_executor_mcp: sharedExecutorMcp,
    connection_integrity: connectionIntegrity,
    semantic_reuse_identity: semanticReuseIdentity,
  };
  return {
    status: Object.values(checks).every((check) => check.status === "compatible")
      ? "compatible" as const
      : "incompatible" as const,
    checks,
  };
}

