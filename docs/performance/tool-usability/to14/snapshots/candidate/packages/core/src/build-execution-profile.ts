import {
  CODEX_EXECUTOR_DELIVERY_BATCH_LIMIT_V1,
  CODEX_EXECUTOR_TRANSPORT_PROFILE_V2,
  DSH_EXECUTOR_TRANSPORT_PROFILE_V3,
  type ExecutorTransportProfile,
  type ExecutorTransportDeliveryBatchLimitV1,
} from "./executor-transport";

export interface BuildExecutionProfileSelectionV1 {
  profile_id: "codex_mcp_v3" | "dsh_native_v4";
  profile_revision: 1;
}

export interface BuildExecutionProfileV1 extends BuildExecutionProfileSelectionV1 {
  version: "build_execution_profile.v1";
  harness_kind: "codex" | "deepseek_harness";
  transport_profile: Readonly<ExecutorTransportProfile>;
  delivery_batch_limit: Readonly<ExecutorTransportDeliveryBatchLimitV1>;
  session_protocol: "automatic_build_executor_session.v3" | "automatic_build_executor_session.v4";
  input_manifest_version: "automatic_build_executor_input_manifest.v3" | "automatic_build_executor_input_manifest.v4";
  delivery_record_version: 3 | 4;
}

/** Historical records resolve here by version, never by the machine's current host. */
export const CODEX_BUILD_EXECUTION_PROFILE_V1 = Object.freeze({
  version: "build_execution_profile.v1",
  profile_id: "codex_mcp_v3",
  profile_revision: 1,
  harness_kind: "codex",
  transport_profile: Object.freeze({ ...CODEX_EXECUTOR_TRANSPORT_PROFILE_V2 }),
  delivery_batch_limit: CODEX_EXECUTOR_DELIVERY_BATCH_LIMIT_V1,
  session_protocol: "automatic_build_executor_session.v3",
  input_manifest_version: "automatic_build_executor_input_manifest.v3",
  delivery_record_version: 3,
} satisfies BuildExecutionProfileV1);

export type CodexBuildExecutionProfileV1 = typeof CODEX_BUILD_EXECUTION_PROFILE_V1;

export const DSH_BUILD_EXECUTION_PROFILE_V1 = Object.freeze({
  version: "build_execution_profile.v1",
  profile_id: "dsh_native_v4",
  profile_revision: 1,
  harness_kind: "deepseek_harness",
  transport_profile: DSH_EXECUTOR_TRANSPORT_PROFILE_V3,
  delivery_batch_limit: Object.freeze({
    version: "executor_transport_delivery_batch_limit.v1",
    max_chunks_per_batch: 1,
    max_serialized_batch_bytes: 16_384,
    max_batches_per_work_unit: 64,
  }),
  session_protocol: "automatic_build_executor_session.v4",
  input_manifest_version: "automatic_build_executor_input_manifest.v4",
  delivery_record_version: 4,
} satisfies BuildExecutionProfileV1);

/** Select only a published identity. Callers cannot supply larger capacity numbers. */
export function resolveBuildExecutionProfile(value: unknown): typeof CODEX_BUILD_EXECUTION_PROFILE_V1 | typeof DSH_BUILD_EXECUTION_PROFILE_V1 {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("build execution profile selection must be an object");
  }
  const record = value as Record<string, unknown>;
  const keys = Object.keys(record).sort();
  if (keys.length !== 2 || keys[0] !== "profile_id" || keys[1] !== "profile_revision"
    || record.profile_revision !== 1) {
    throw new Error("build execution profile selection contains unsupported fields or revision");
  }
  if (record.profile_id === "codex_mcp_v3") return CODEX_BUILD_EXECUTION_PROFILE_V1;
  if (record.profile_id === "dsh_native_v4") return DSH_BUILD_EXECUTION_PROFILE_V1;
  throw new Error("build execution profile is unsupported");
}

/** Persist the explicit pair; all other limits are owned by this published registry. */
export function executionProfileSelection(profile: Readonly<BuildExecutionProfileV1>): BuildExecutionProfileSelectionV1 {
  return { profile_id: profile.profile_id, profile_revision: profile.profile_revision };
}
