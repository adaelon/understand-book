import { describe, expect, it } from "vitest";
import {
  CODEX_BUILD_EXECUTION_PROFILE_V1, DSH_BUILD_EXECUTION_PROFILE_V1, resolveBuildExecutionProfile,
} from "../src/build-execution-profile";
import {
  CODEX_EXECUTOR_TRANSPORT_PROFILE_V2, DSH_EXECUTOR_TRANSPORT_PROFILE_V3,
  packExecutorTransportPayload, packExecutorTransportBatches,
  validateExecutorTransportProfile, validateExecutorTransportProfileV3,
} from "../src/executor-transport";

describe("published execution profile boundary", () => {
  it("preserves the legacy profile and refuses new identities in the old decoder", () => {
    expect(CODEX_BUILD_EXECUTION_PROFILE_V1.transport_profile).toEqual(CODEX_EXECUTOR_TRANSPORT_PROFILE_V2);
    expect(() => validateExecutorTransportProfile(DSH_EXECUTOR_TRANSPORT_PROFILE_V3 as never)).toThrow(/unsupported/);
    expect(() => validateExecutorTransportProfileV3(CODEX_EXECUTOR_TRANSPORT_PROFILE_V2)).toThrow(/unsupported/);
    expect(() => validateExecutorTransportProfileV3({ ...DSH_EXECUTOR_TRANSPORT_PROFILE_V3, max_input_chunks: -1 })).toThrow();
    expect(() => validateExecutorTransportProfileV3({ ...DSH_EXECUTOR_TRANSPORT_PROFILE_V3, private_override: 1 })).toThrow();
    expect(() => validateExecutorTransportProfileV3({ ...DSH_EXECUTOR_TRANSPORT_PROFILE_V3, session_protocol: "automatic_build_executor_session.v3" })).toThrow();
  });

  it("selects only published revisions and does not accept caller capacity overrides", () => {
    expect(resolveBuildExecutionProfile({ profile_id: "dsh_native_v4", profile_revision: 1 })).toBe(DSH_BUILD_EXECUTION_PROFILE_V1);
    for (const value of [null, {}, { profile_id: "dsh_native_v4", profile_revision: 2 },
      { profile_id: "unknown", profile_revision: 1 },
      { profile_id: "dsh_native_v4", profile_revision: 1, max_tool_result_bytes: 1000000 }]) {
      expect(() => resolveBuildExecutionProfile(value)).toThrow();
    }
  });

  it("reconstructs Unicode at the same chunk boundary but presents one chunk per DSH batch", () => {
    const payload = '中文\r\n$x=\\alpha$ ```ts const x = "😀";```\n'.repeat(450);
    const pack = packExecutorTransportPayload({ profile: DSH_EXECUTOR_TRANSPORT_PROFILE_V3,
      payload_utf8: payload, envelope_for_chunk: frame => frame });
    expect(pack.status).toBe("within_limit");
    if (pack.status !== "within_limit") throw new Error(pack.code);
    expect(pack.chunks.map(chunk => chunk.payload_utf8).join("")).toBe(payload);
    expect(pack.chunks.length).toBeGreaterThan(1);
    const batches = packExecutorTransportBatches({ chunks: pack.chunks.map(({ordinal,payload_utf8}) => ({ordinal,payload_utf8})),
      limit: DSH_BUILD_EXECUTION_PROFILE_V1.delivery_batch_limit, envelope_for_chunks: chunks => ({ chunks }) });
    expect(batches.status).toBe("within_limit");
    if (batches.status !== "within_limit") throw new Error(batches.code);
    expect(batches.batches.every(batch => batch.chunks.length === 1)).toBe(true);
    expect(batches.batches.every(batch => batch.serialized_mcp_result_bytes <= 16384)).toBe(true);
  });
});
