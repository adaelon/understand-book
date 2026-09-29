import type { AutomaticBuildExecutorMcpErrorV2 } from "./automatic-build-executor-session";

/** Host control facts only; no candidate, transcript, arbitrary message or inferred usage. */
export interface DshExecutorObservationV1 {
  version: "dsh_executor_observation.v1";
  launch_id: string;
  opaque_handoff_ref: string;
  dispatch_slot_ref: string;
  outcome: "receipt" | "bootstrap" | "open" | "connection" | "runtime" | "cancelled" | "cleanup";
  calls: number | null;
  last_operation?: "executor.open" | "executor.input.next" | "executor.generation.start" | "executor.submit_candidate";
  done?: "committed" | "retryable_failure" | "interrupted";
  engine_error?: AutomaticBuildExecutorMcpErrorV2;
  code?: "connection_terminal" | "handoff_ref_mismatch" | "model_context_limit" | "model_configuration_changed" | "executor_connection_closed" | "executor_ended_without_receipt" | "executor_runtime_failed" | "executor_launch_failed" | "executor_cleanup_failed" | "build_cancelled";
}

export function readDshExecutorObservation(value: unknown): DshExecutorObservationV1 {
  const v = value as DshExecutorObservationV1;
  const required = ["version", "launch_id", "opaque_handoff_ref", "dispatch_slot_ref", "outcome", "calls"];
  if (!v || typeof v !== "object" || required.some(k => !(k in v))
    || Object.keys(v).some(k => ![...required, "last_operation", "done", "code", "engine_error"].includes(k))
    || v.version !== "dsh_executor_observation.v1" || typeof v.launch_id !== "string" || !/^[a-zA-Z0-9_-]{1,80}$/u.test(v.launch_id)
    || !/^abhandoff1_[a-f0-9]{64}$/u.test(v.opaque_handoff_ref)
    || !/^abdispatchslot1_[a-f0-9]{64}$/u.test(v.dispatch_slot_ref)
    || !["receipt", "bootstrap", "open", "connection", "runtime", "cancelled", "cleanup"].includes(v.outcome)
    || (v.calls !== null && (!Number.isSafeInteger(v.calls) || v.calls < 0))
    || (v.last_operation !== undefined && !["executor.open", "executor.input.next", "executor.generation.start", "executor.submit_candidate"].includes(v.last_operation))
    || (v.done !== undefined && !["committed", "retryable_failure", "interrupted"].includes(v.done))
    || (v.code !== undefined && !["connection_terminal", "handoff_ref_mismatch", "model_context_limit", "model_configuration_changed", "executor_connection_closed", "executor_ended_without_receipt", "executor_runtime_failed", "executor_launch_failed", "executor_cleanup_failed", "build_cancelled"].includes(v.code))) {
    throw new Error("DSH host observation is invalid");
  }
  if (v.engine_error !== undefined) {
    const e = v.engine_error;
    if (!e || typeof e !== "object" || Object.keys(e).some(k => !["version", "status", "category", "diagnostic_code", "phase", "field"].includes(k))
      || e.version !== "automatic_build_executor_mcp_error.v2"
      || !["interrupted", "retryable_failure"].includes(e.status)
      || !["bootstrap", "session", "transport", "candidate_sink", "writer", "internal"].includes(e.category)
      || !["invalid_arguments", "protocol_incompatible", "connection_terminal", "handoff_ref_mismatch", "bootstrap_unavailable", "stale_generation_session", "lease_expired", "candidate_request_too_large", "candidate_sink_unavailable", "writer_failed", "executor_internal"].includes(e.diagnostic_code)
      || !["open", "input_delivery", "generation_start", "candidate_submit"].includes(e.phase)
      || (e.field !== undefined && !["arguments", "version", "opaque_handoff_ref"].includes(e.field))) throw new Error("DSH Engine observation is invalid");
  }
  return { ...v };
}
