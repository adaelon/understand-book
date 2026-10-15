import { describe, expect, it } from "vitest";
import { readBuildExecutorModelRuntime } from "../src/build-executor-model-runtime";

const runtime = { version: "build_executor_model_runtime.v1", provider: "probe", model: "selected",
  reasoning_effort: "off", max_output_tokens: 100, context_window_tokens: 2000,
  safety_margin_tokens: 100, context_source: "provider_model_metadata" };

describe("persistable executor model runtime", () => {
  it("roundtrips exact fields and detaches the selected configuration", () => {
    const source = { ...runtime };
    const frozen = readBuildExecutorModelRuntime(source);
    source.model = "later-model";
    expect(frozen).toEqual(runtime);
    expect(Object.isFrozen(frozen)).toBe(true);
    expect(readBuildExecutorModelRuntime(JSON.parse(JSON.stringify(frozen)))).toEqual(frozen);
  });

  it("refuses unknown fields, missing model facts and an exhausted input budget", () => {
    for (const changed of [{ version: "build_executor_model_runtime.v2" }, { provider: "" }, { model: null },
      { reasoning_effort: "" }, { max_output_tokens: 0 }, { max_output_tokens: 0.5 },
      { context_window_tokens: 200 }, { safety_margin_tokens: -1 }, { context_source: "guessed" }, { api_key: "x" }]) {
      expect(() => readBuildExecutorModelRuntime({ ...runtime, ...changed })).toThrow(/runtime_invalid/);
    }
    const { reasoning_effort, ...missing } = runtime;
    expect(() => readBuildExecutorModelRuntime(missing)).toThrow(/runtime_invalid/);
    expect(readBuildExecutorModelRuntime({ ...runtime, reasoning_effort: null }).reasoning_effort).toBeNull();
    expect(readBuildExecutorModelRuntime({ ...runtime, context_window_tokens: 201 }).context_window_tokens).toBe(201);
  });
});
