/** Model routing and context metadata captured before a build is confirmed. */
export interface BuildExecutorModelRuntimeV1 {
  version: "build_executor_model_runtime.v1";
  provider: string;
  model: string;
  reasoning_effort: string | null;
  max_output_tokens: number;
  context_window_tokens: number;
  safety_margin_tokens: number;
  context_source: "provider_model_metadata";
}

/** Copy persisted fields so callers cannot change a running executor's budget. */
export function readBuildExecutorModelRuntime(value: unknown): Readonly<BuildExecutorModelRuntimeV1> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("executor_model_runtime_invalid");
  const record = value as Record<string, unknown>;
  const fields = ["version", "provider", "model", "reasoning_effort", "max_output_tokens",
    "context_window_tokens", "safety_margin_tokens", "context_source"];
  if (Object.keys(record).length !== fields.length || fields.some(key => !Object.hasOwn(record, key))
    || record.version !== "build_executor_model_runtime.v1" || record.context_source !== "provider_model_metadata"
    || typeof record.provider !== "string" || !record.provider.trim()
    || typeof record.model !== "string" || !record.model.trim()
    || !(record.reasoning_effort === null || typeof record.reasoning_effort === "string" && record.reasoning_effort.trim())
    || !Number.isSafeInteger(record.max_output_tokens) || (record.max_output_tokens as number) < 1
    || !Number.isSafeInteger(record.context_window_tokens) || (record.context_window_tokens as number) < 1
    || !Number.isSafeInteger(record.safety_margin_tokens) || (record.safety_margin_tokens as number) < 0
    || (record.max_output_tokens as number) + (record.safety_margin_tokens as number) >= (record.context_window_tokens as number)) {
    throw new Error("executor_model_runtime_invalid");
  }
  return Object.freeze({ ...record } as unknown as BuildExecutorModelRuntimeV1);
}
