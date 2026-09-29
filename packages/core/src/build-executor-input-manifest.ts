import type { AutomaticBuildExecutorInputManifestV3 } from "./automatic-build-executor-session";
import { canonicalAutomaticBuildJson } from "./automatic-build-protocol";
import { resolveBuildExecutionProfile, type BuildExecutionProfileSelectionV1 } from "./build-execution-profile";
import { validateExecutorTransportProfileV3, type ExecutorTransportProfileV3 } from "./executor-transport";

export interface AutomaticBuildExecutorInputManifestV4 extends Omit<AutomaticBuildExecutorInputManifestV3,
  "version" | "transport_profile"> {
  version: "automatic_build_executor_input_manifest.v4";
  execution_profile: BuildExecutionProfileSelectionV1;
  transport_profile: ExecutorTransportProfileV3;
}

function exactObject(value: unknown, keys: string[]): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("executor manifest object is invalid");
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) {
    throw new Error("executor manifest has missing or unsupported fields");
  }
  return value as Record<string, unknown>;
}

export function validateExecutorInputManifestV4(value: unknown): AutomaticBuildExecutorInputManifestV4 {
  const record = exactObject(value, ["version", "opaque_session_ref", "generation_input_ref", "execution_profile",
    "transport_profile", "segments", "total_chunk_count"]);
  if (record.version !== "automatic_build_executor_input_manifest.v4"
    || typeof record.opaque_session_ref !== "string" || !/^absession1_[a-f0-9]{64}$/u.test(record.opaque_session_ref)
    || typeof record.generation_input_ref !== "string" || !/^abinput1_[a-f0-9]{64}$/u.test(record.generation_input_ref)) {
    throw new Error("executor manifest identity is invalid");
  }
  const profile = resolveBuildExecutionProfile(record.execution_profile);
  const transport = validateExecutorTransportProfileV3(record.transport_profile);
  if (profile.input_manifest_version !== record.version
    || canonicalAutomaticBuildJson(profile.transport_profile) !== canonicalAutomaticBuildJson(transport)) {
    throw new Error("executor manifest profile does not match its published configuration");
  }
  if (!Array.isArray(record.segments) || record.segments.length !== 2) throw new Error("executor manifest segments are invalid");
  let total = 0;
  for (const [index, value] of record.segments.entries()) {
    const segment = exactObject(value, ["kind", "byte_length", "sha256", "chunk_count"]);
    if (segment.kind !== ["semantic_prompt", "semantic_input"][index]
      || !Number.isSafeInteger(segment.byte_length) || (segment.byte_length as number) < 1
      || !Number.isSafeInteger(segment.chunk_count) || (segment.chunk_count as number) < 1
      || typeof segment.sha256 !== "string" || !/^[a-f0-9]{64}$/u.test(segment.sha256)) {
      throw new Error("executor manifest segment is invalid");
    }
    total += segment.chunk_count as number;
  }
  if (record.total_chunk_count !== total || total > transport.max_input_chunks) throw new Error("executor manifest chunk count is invalid");
  return record as unknown as AutomaticBuildExecutorInputManifestV4;
}
