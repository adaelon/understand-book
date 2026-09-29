import { describe, expect, it } from "vitest";
import { tmpdir } from "node:os";
import { validateDeliverySessionRecord, validateDeliverySessionRecordV4 } from "../src/automatic-build-executor-session";
import { createBuildExecutorConnectionStateV3, createBuildExecutorConnectionStateV4 } from "../src/build-executor-connection-state";
import { validateExecutorInputManifestV4 } from "../src/build-executor-input-manifest";
import { CODEX_EXECUTOR_TRANSPORT_PROFILE_V2, DSH_EXECUTOR_TRANSPORT_PROFILE_V3 } from "../src/executor-transport";

const sessionRef = `absession1_${"1".repeat(64)}`;
const inputRef = `abinput1_${"2".repeat(64)}`;
const selection = { profile_id: "dsh_native_v4", profile_revision: 1 };
const common = {
  opaque_session_ref: sessionRef,
  opaque_handoff_ref: `abhandoff1_${"3".repeat(64)}`,
  open_session_ref: `absession1_${"4".repeat(64)}`,
  generation_input_ref: inputRef,
  semantic_prompt_sha256: "a".repeat(64), semantic_prompt_byte_length: 100,
  semantic_input_sha256: "b".repeat(64), semantic_input_byte_length: 200,
  semantic_prompt_chunk_count: 1, semantic_input_chunk_count: 2, total_chunk_count: 3,
  output_schema_version: "semantic_candidate.v3", created_at: "2026-09-26T00:00:00.000Z",
};
const publicOwner = { version: "automatic_build_dispatch_owner_identity.v1", stage: "pass1", dispatch_id: "dispatch", dispatch_run_id: "run" };
const privateOwner = {
  version: "automatic_build_private_artifact_owner_identity.v2", task_id: "task", book_id: "book",
  source_fingerprint: "source", intent_id: "intent", intent_revision: 1, plan_id: "plan", plan_revision: 1,
  artifact_id: "artifact", artifact_type: "concept_map", blueprint_id: "blueprint", blueprint_version: "1", attempt: 1,
};

describe("executor record and response generations", () => {
  for (const kind of ["public", "private"] as const) {
    it(`keeps ${kind} V3 bytes and decodes V4 only with its explicit profile`, () => {
      const prefix = kind === "public" ? "automatic_build_executor_delivery_session_record" : "automatic_build_executor_private_delivery_session_record";
      const identity = kind === "public" ? { owner_identity: publicOwner, stage: "pass1", work_unit_id: "unit" }
        : { owner_identity: privateOwner, task_id: "task" };
      const legacy = { version: `${prefix}.v3`, ...common, ...identity, transport_profile: CODEX_EXECUTOR_TRANSPORT_PROFILE_V2 };
      const current = { ...legacy, version: `${prefix}.v4`, execution_profile: selection, transport_profile: DSH_EXECUTOR_TRANSPORT_PROFILE_V3 };
      expect(validateDeliverySessionRecord(legacy, sessionRef)).toEqual(legacy);
      expect(validateDeliverySessionRecordV4(current, sessionRef)).toEqual(current);
      expect(() => validateDeliverySessionRecord(current, sessionRef)).toThrow();
      expect(() => validateDeliverySessionRecordV4(legacy, sessionRef)).toThrow();
      expect(() => validateDeliverySessionRecordV4({ ...current, transport_profile: CODEX_EXECUTOR_TRANSPORT_PROFILE_V2 }, sessionRef)).toThrow();
      expect(() => validateDeliverySessionRecordV4({ ...current, execution_profile: { profile_id: "codex_mcp_v3", profile_revision: 1 } }, sessionRef)).toThrow();
      expect(() => validateDeliverySessionRecordV4({ ...current, total_chunk_count: 4 }, sessionRef)).toThrow();
      expect(() => validateDeliverySessionRecordV4({ ...current, extra: 1 }, sessionRef)).toThrow();
    });
  }

  it("validates complete V4 manifests and keeps old connections from consuming V4 envelopes", () => {
    const manifest = {
      version: "automatic_build_executor_input_manifest.v4", opaque_session_ref: sessionRef,
      generation_input_ref: inputRef, execution_profile: selection, transport_profile: DSH_EXECUTOR_TRANSPORT_PROFILE_V3,
      segments: [{ kind: "semantic_prompt", byte_length: 100, sha256: "a".repeat(64), chunk_count: 1 },
        { kind: "semantic_input", byte_length: 200, sha256: "b".repeat(64), chunk_count: 2 }], total_chunk_count: 3,
    };
    expect(validateExecutorInputManifestV4(manifest)).toEqual(manifest);
    expect(() => validateExecutorInputManifestV4({ ...manifest, total_chunk_count: 2 })).toThrow();
    expect(() => validateExecutorInputManifestV4({ ...manifest, transport_profile: CODEX_EXECUTOR_TRANSPORT_PROFILE_V2 })).toThrow();
    expect(() => validateExecutorInputManifestV4({ ...manifest, segments: [manifest.segments[0], manifest.segments[0]] })).toThrow();
    const response = { version: "automatic_build_executor_session.v4", action: { kind: "DELIVER_INPUT", input_manifest: manifest,
      next_request: { version: "automatic_build_executor_input_next_request.v4", opaque_session_ref: sessionRef, generation_input_ref: inputRef } } };
    const call = { tool_name: "executor.open" as const,
      request: { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: common.opaque_handoff_ref } };
    const legacy = createBuildExecutorConnectionStateV3({ session_private_root: tmpdir() });
    const current = createBuildExecutorConnectionStateV4({ session_private_root: tmpdir() });
    expect(() => legacy.observe_response(call, response)).toThrow(/incompatible/);
    expect(() => current.observe_response(call, { ...response, version: "automatic_build_executor_session.v3" })).toThrow(/incompatible/);
    expect(current.authorize_connection(current.connection_capability, call)).toBe(true);
    current.observe_response(call, response);
    expect(current.authorize_connection(current.connection_capability, { tool_name: "executor.input.next", request: response.action.next_request })).toBe(true);
  });
});
