import type { ExecutorBinding } from "./executor-host.ts";
import { connectExecutor, type EngineConfig } from "./engine-client.ts";
import { installExecutorTools } from "./executor-tools.ts";
import type { DshExecutorObservationV1 } from "../../core/src/dsh-executor-observation.ts";

export interface ExecutorObservation {
  done?: "committed" | "retryable_failure" | "interrupted";
  error_code?: string;
  calls?: number;
  last_operation?: DshExecutorObservationV1["last_operation"];
  bootstrap_failed?: boolean;
  engine_error?: DshExecutorObservationV1["engine_error"];
}

export async function prepareExecutorBridge(binding: ExecutorBinding, config: EngineConfig, observation: ExecutorObservation) {
  observation.calls = 0;
  const connection = await connectExecutor(config, binding.launch.signal).catch(error => {
    observation.bootstrap_failed = true;
    throw error;
  });
  installExecutorTools(binding, async (name, request, signal) => {
    if (name === "executor.open" && (request as { opaque_handoff_ref: string }).opaque_handoff_ref !== binding.launch.opaque_handoff_ref) {
      observation.error_code = "handoff_ref_mismatch";
      observation.last_operation = name;
      throw new Error("executor_handoff_mismatch");
    }
    observation.calls!++;
    observation.last_operation = name;
    try {
      const response = await connection.call(name, request, signal) as any;
      if (response.version === "automatic_build_executor_mcp_error.v2") {
        observation.error_code = response.diagnostic_code;
        observation.engine_error = response;
      }
      if (response.action?.kind === "DONE") observation.done = response.action.status;
      return response;
    } catch { observation.error_code = "executor_connection_closed"; throw new Error("executor_connection_closed"); }
  });
  return () => connection.dispose();
}
