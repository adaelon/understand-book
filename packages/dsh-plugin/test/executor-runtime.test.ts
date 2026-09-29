import assert from "node:assert/strict";
import { test } from "node:test";
import { ReasoningEffortId } from "@deepseek-ai/dsh-llm";
import { resolveExecutorRuntime } from "../src/executor-runtime.ts";
import { createExecutorHost } from "../src/executor-host.ts";
import { harness, probeRuntime, ScriptedModel, textResponse } from "./harness.ts";

class MetadataModel extends ScriptedModel {
  async resolveModel(provider: string, model: string) {
    return { provider, id: model, name: model, context: { contextWindow: 16000 }, defaultMaxTokens: 12000,
      reasoning: { efforts: [{ id: ReasoningEffortId("off"), name: "Off" }], defaultEffort: ReasoningEffortId("off") } };
  }
}

test("host metadata freezes defaults and output cap for subsequent executor launches", async () => {
  const h = await harness(new MetadataModel(async function* () { yield* textResponse("done"); }));
  const host = createExecutorHost(h.ctx, "understand-book-executor", async () => {});
  try {
    const selected = { provider: "probe", model: "selected", maxTokens: 1000 };
    const runtime = await resolveExecutorRuntime(h.ctx, selected, 100);
    assert.equal(runtime.reasoning_effort, "off");
    assert.equal(runtime.context_window_tokens, 16000);
    assert.equal(runtime.max_output_tokens, 1000);
    selected.model = "later-selection";
    selected.maxTokens = 12000;
    for (const char of ["a", "b"]) {
      const run = await host.launch({ parent: h.parent, signal: AbortSignal.timeout(10000),
        opaque_handoff_ref: `abhandoff1_${char.repeat(64)}`, runtime: JSON.parse(JSON.stringify(runtime)) });
      assert.deepEqual(await run.terminal, { stop_reason: "completed" });
      await run.dispose();
    }
    assert.equal(h.model.requests.length, 2);
    assert.ok(h.model.requests.every(request => request.model === "selected" && request.maxTokens === 1000
      && request.reasoningEffort === "off"));
    assert.equal(h.parent.options.model, "scripted");
    assert.deepEqual(h.ctx.agents.list(), [h.parent]);
  } finally { await host.dispose(); await h.dispose(); }
});

test("missing context metadata and implicit output budgets cannot become a runtime", async () => {
  const h = await harness();
  try {
    await assert.rejects(resolveExecutorRuntime(h.ctx, { provider: "probe", model: "scripted" }, 100), /budget_invalid/);
    await assert.rejects(resolveExecutorRuntime(h.ctx, { provider: "probe", model: "scripted", maxTokens: 1000 }, 100), /context_unknown/);
    assert.equal(h.model.requests.length, 0);
  } finally { await h.dispose(); }
});

test("launch detaches runtime before asynchronous child preparation", async () => {
  const h = await harness();
  const runtime = probeRuntime();
  const host = createExecutorHost(h.ctx, "understand-book-executor", async binding => {
    runtime.model = "changed-during-create";
    runtime.max_output_tokens = 5;
    assert.equal(binding.launch.runtime.model, "scripted");
    assert.equal(Object.isFrozen(binding.launch.runtime), true);
  });
  try {
    const run = await host.launch({ parent: h.parent, signal: AbortSignal.timeout(10000),
      opaque_handoff_ref: `abhandoff1_${"c".repeat(64)}`, runtime });
    assert.deepEqual(await run.terminal, { stop_reason: "completed" });
    assert.equal(h.model.requests[0].maxTokens, 1000);
    assert.equal(h.model.requests[0].model, "scripted");
    await run.dispose();
  } finally { await host.dispose(); await h.dispose(); }
});
