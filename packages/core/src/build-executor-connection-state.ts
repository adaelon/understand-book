import path from "node:path";
import { canonicalAutomaticBuildJson } from "./automatic-build-protocol";
import { BuildExecutorInvalidArgumentsError, type BuildExecutorToolNameV1 } from "./build-executor-tool-contract";
import { validateExecutorInputManifestV4 } from "./build-executor-input-manifest";

const OPAQUE_HANDOFF_REF = /^abhandoff1_[a-f0-9]{64}$/u;
const OPAQUE_SESSION_REF = /^absession1_[a-f0-9]{64}$/u;
const GENERATION_INPUT_REF = /^abinput1_[a-f0-9]{64}$/u;
const CANDIDATE_SINK_REF = /^absink1_[a-f0-9]{64}$/u;

export interface BuildExecutorToolCallV1 {
  tool_name: BuildExecutorToolNameV1;
  request: unknown;
}

export interface BuildExecutorStdioConnectionCapabilityV3 {
  readonly connection_capability: symbol;
  authorize_connection: (capability: unknown, call: BuildExecutorToolCallV1) => boolean;
  observe_response: (call: BuildExecutorToolCallV1, response: unknown) => void;
}

export class BuildExecutorConnectionOpenError extends Error {
  constructor(readonly diagnostic_code: "connection_terminal" | "handoff_ref_mismatch") {
    super(diagnostic_code);
    this.name = "BuildExecutorConnectionOpenError";
  }
}

type ConnectionPhase = "open" | "input" | "grant" | "final_batch" | "generate" | "wait" | "terminal";

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function stringField(value: unknown, field: string): string | undefined {
  if (!isRecord(value) || typeof value[field] !== "string") return undefined;
  return value[field];
}

function callKey(call: BuildExecutorToolCallV1): string {
  return canonicalAutomaticBuildJson({ tool_name: call.tool_name, request: call.request });
}


/** Ref/phase/ordinal state for one V3 connection, independent of its host bootstrap. */
export function createBuildExecutorConnectionStateV3(input: { session_private_root: string }): BuildExecutorStdioConnectionCapabilityV3 {
  return createConnectionState({ ...input, session_protocol: "automatic_build_executor_session.v3" });
}

export function createBuildExecutorConnectionStateV4(input: { session_private_root: string }): BuildExecutorStdioConnectionCapabilityV3 {
  return createConnectionState({ ...input, session_protocol: "automatic_build_executor_session.v4" });
}

function createConnectionState(input: { session_private_root: string;
  session_protocol: "automatic_build_executor_session.v3" | "automatic_build_executor_session.v4" }): BuildExecutorStdioConnectionCapabilityV3 {
  if (typeof input.session_private_root !== "string"
    || !path.isAbsolute(input.session_private_root)
    || input.session_private_root.includes("\0")) {
    throw new Error("Build Executor session-private root is invalid");
  }

  // The symbol binds calls to this one thread-owned stdio connection. It is intentionally not a
  // caller-role credential: any separately created connection receives its own unrelated symbol.
  const connectionCapability = Symbol("build-executor-stdio-connection");
  const sessionPrivateRoot = path.resolve(input.session_private_root);
  void sessionPrivateRoot;

  let phase: ConnectionPhase = "open";
  let handoffRef: string | undefined;
  let sessionRef: string | undefined;
  let generationInputRef: string | undefined;
  let candidateSinkRef: string | undefined;
  let expectedAckThroughOrdinal: number | undefined;
  let finalBatchOrdinal: number | undefined;
  let lastObservedCallKey: string | undefined;

  const optionalOrdinal = (record: Record<string, unknown>, field: string): number | undefined => {
    const value = record[field];
    return Number.isSafeInteger(value) && (value as number) >= 0 ? value as number : undefined;
  };

  const authorizeConnection = (capability: unknown, call: BuildExecutorToolCallV1): boolean => {
    if (capability !== connectionCapability || !isRecord(call.request)) return false;
    const key = callKey(call);
    if (key === lastObservedCallKey) return true;

    if (call.tool_name === "executor.open") {
      const requestedHandoffRef = stringField(call.request, "opaque_handoff_ref");
      if (!requestedHandoffRef || !OPAQUE_HANDOFF_REF.test(requestedHandoffRef)) {
        throw new BuildExecutorInvalidArgumentsError("opaque_handoff_ref");
      }
      if (phase === "terminal") throw new BuildExecutorConnectionOpenError("connection_terminal");
      if (handoffRef && requestedHandoffRef !== handoffRef) {
        throw new BuildExecutorConnectionOpenError("handoff_ref_mismatch");
      }
      if (phase !== "open" && phase !== "wait") return false;
      handoffRef ??= requestedHandoffRef;
      return true;
    }

    if (call.tool_name === "executor.input.next") {
      if (Object.hasOwn(call.request, "previous_chunk_receipt")) return false;
      const requestedSessionRef = stringField(call.request, "opaque_session_ref");
      const requestedInputRef = stringField(call.request, "generation_input_ref");
      const ordinalValue = call.request.ack_through_ordinal;
      const requestedOrdinal = optionalOrdinal(call.request, "ack_through_ordinal");
      if (ordinalValue !== undefined && requestedOrdinal === undefined) return false;
      return phase === "input"
        && !!requestedSessionRef && OPAQUE_SESSION_REF.test(requestedSessionRef)
        && requestedSessionRef === sessionRef
        && !!requestedInputRef && GENERATION_INPUT_REF.test(requestedInputRef)
        && requestedInputRef === generationInputRef
        && requestedOrdinal === expectedAckThroughOrdinal;
    }

    if (call.tool_name === "executor.generation.start") {
      const requestedSessionRef = stringField(call.request, "opaque_session_ref");
      const requestedInputRef = stringField(call.request, "generation_input_ref");
      const requestedOrdinal = optionalOrdinal(call.request, "confirmed_through_ordinal");
      return phase === "final_batch"
        && !!requestedSessionRef && requestedSessionRef === sessionRef
        && OPAQUE_SESSION_REF.test(requestedSessionRef)
        && !!requestedInputRef && requestedInputRef === generationInputRef
        && GENERATION_INPUT_REF.test(requestedInputRef)
        && requestedOrdinal === finalBatchOrdinal;
    }

    if (call.tool_name === "executor.submit_candidate") {
      const requestedSessionRef = stringField(call.request, "opaque_session_ref");
      const requestedSinkRef = stringField(call.request, "candidate_sink_ref");
      return phase === "generate"
        && !!requestedSessionRef && requestedSessionRef === sessionRef
        && OPAQUE_SESSION_REF.test(requestedSessionRef)
        && !!requestedSinkRef && requestedSinkRef === candidateSinkRef
        && CANDIDATE_SINK_REF.test(requestedSinkRef);
    }

    return false;
  };

  const observeResponse = (call: BuildExecutorToolCallV1, response: unknown): void => {
    if (!isRecord(response)
      || response.version !== input.session_protocol
      || !isRecord(response.action)
      || typeof response.action.kind !== "string") {
      throw new Error("Build Executor stdio connection received an incompatible response");
    }
    const action = response.action;
    if (action.kind === "DELIVER_INPUT") {
      if (!isRecord(action.input_manifest) || !isRecord(action.next_request)) {
        throw new Error("Build Executor delivery binding is invalid");
      }
      const manifest = action.input_manifest;
      if (input.session_protocol === "automatic_build_executor_session.v4") validateExecutorInputManifestV4(manifest);
      const next = action.next_request;
      const manifestSessionRef = stringField(manifest, "opaque_session_ref");
      const manifestInputRef = stringField(manifest, "generation_input_ref");
      const nextSessionRef = stringField(next, "opaque_session_ref");
      const nextInputRef = stringField(next, "generation_input_ref");
      const nextOrdinalValue = next.ack_through_ordinal;
      const nextOrdinal = optionalOrdinal(next, "ack_through_ordinal");
      if (!manifestSessionRef || !OPAQUE_SESSION_REF.test(manifestSessionRef)
        || !manifestInputRef || !GENERATION_INPUT_REF.test(manifestInputRef)
        || nextSessionRef !== manifestSessionRef
        || nextInputRef !== manifestInputRef
        || Object.hasOwn(next, "previous_chunk_receipt")
        || nextOrdinalValue !== undefined && nextOrdinal === undefined
        || Object.hasOwn(manifest, "transport_profile_digest")) {
        throw new Error("Build Executor delivery binding is invalid");
      }
      sessionRef = manifestSessionRef;
      generationInputRef = manifestInputRef;
      candidateSinkRef = undefined;
      expectedAckThroughOrdinal = nextOrdinal;
      finalBatchOrdinal = undefined;
      phase = "input";
    } else if (action.kind === "INPUT_BATCH") {
      if (call.tool_name !== "executor.input.next" || !isRecord(call.request) || !isRecord(action.batch)) {
        throw new Error("Build Executor input batch binding is invalid");
      }
      const batch = action.batch;
      const batchSessionRef = stringField(batch, "opaque_session_ref");
      const batchInputRef = stringField(batch, "generation_input_ref");
      const firstOrdinal = optionalOrdinal(batch, "first_ordinal");
      const lastOrdinal = optionalOrdinal(batch, "last_ordinal");
      const requestedAck = optionalOrdinal(call.request, "ack_through_ordinal");
      const expectedFirstOrdinal = requestedAck === undefined ? 0 : requestedAck + 1;
      if (!batchSessionRef || batchSessionRef !== sessionRef
        || !batchInputRef || batchInputRef !== generationInputRef
        || firstOrdinal !== expectedFirstOrdinal
        || lastOrdinal === undefined || lastOrdinal < firstOrdinal
        || !Array.isArray(batch.chunks)
        || batch.chunks.length !== lastOrdinal - firstOrdinal + 1
        || typeof batch.final_for_generation !== "boolean") {
        throw new Error("Build Executor input batch binding is invalid");
      }
      for (let index = 0; index < batch.chunks.length; index += 1) {
        const chunk = batch.chunks[index];
        if (!isRecord(chunk)
          || stringField(chunk, "opaque_session_ref") !== sessionRef
          || stringField(chunk, "generation_input_ref") !== generationInputRef
          || optionalOrdinal(chunk, "ordinal") !== firstOrdinal + index
          || typeof chunk.payload_utf8 !== "string"
          || !isRecord(chunk.byte_range)
          || !Number.isSafeInteger(chunk.byte_range.start)
          || !Number.isSafeInteger(chunk.byte_range.end)
          || (chunk.byte_range.end as number) - (chunk.byte_range.start as number)
            !== Buffer.byteLength(chunk.payload_utf8, "utf8")
          || Object.hasOwn(chunk, "payload_sha256")) {
          throw new Error("Build Executor input batch chunk binding is invalid");
        }
      }
      expectedAckThroughOrdinal = lastOrdinal;
      if (batch.final_for_generation) {
        finalBatchOrdinal = lastOrdinal;
        phase = "final_batch";
      } else {
        phase = "input";
      }
    } else if (action.kind === "GENERATE") {
      const generateSessionRef = stringField(action, "opaque_session_ref");
      const sinkRef = stringField(action, "candidate_sink_ref");
      if (!generateSessionRef || !OPAQUE_SESSION_REF.test(generateSessionRef)
        || !sinkRef || !CANDIDATE_SINK_REF.test(sinkRef)) {
        throw new Error("Build Executor candidate sink binding is invalid");
      }
      sessionRef = generateSessionRef;
      candidateSinkRef = sinkRef;
      phase = "generate";
    } else if (action.kind === "WAIT") {
      phase = "wait";
    } else if (action.kind === "DONE") {
      phase = "terminal";
    } else {
      throw new Error("Build Executor stdio connection received an unsupported action");
    }
    lastObservedCallKey = callKey(call);
  };

  return Object.freeze({
    connection_capability: connectionCapability,
    authorize_connection: authorizeConnection,
    observe_response: observeResponse,
  });
}
