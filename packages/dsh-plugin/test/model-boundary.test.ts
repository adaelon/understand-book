import assert from "node:assert/strict";
import { test } from "node:test";
import { createExecutorHost } from "../src/executor-host.ts";
import { measureExecutorContext } from "../src/executor-model-boundary.ts";
import { harness, probeRuntime, ScriptedModel, textResponse, toolResponse } from "./harness.ts";

test("accumulated child history exceeds context before the provider sees it", async () => {
  let step = 0;
  const h = await harness(new ScriptedModel(async function* () {
    assert.ok(step < 8, "context middleware must stop the growing history");
    yield* toolResponse(`history-${step++}`, "ub_executor_input_next", {});
  }));
  const host = createExecutorHost(h.ctx, "understand-book-executor", async binding => {
    binding.agent!.ctx.tools.register({ name: "ub_executor_input_next", description: "Synthetic small input chunk",
      parameters: { type: "object", properties: {} },
      output: { schema: {}, render: (_args, value) => [{ type: "text", text: JSON.stringify(value) }] },
      execute: async () => ({ text: "x".repeat(3000) }),
    });
  });
  try {
    const run = await host.launch({ parent: h.parent, signal: AbortSignal.timeout(10000),
      opaque_handoff_ref: `abhandoff1_${"1".repeat(64)}`, runtime: probeRuntime({ max_output_tokens: 100, context_window_tokens: 2000 }) });
    const terminal = await run.terminal;
    assert.equal(terminal.diagnostic_code, "model_context_limit");
    assert.ok(step >= 2);
    assert.ok(h.model.requests.every(request => measureExecutorContext(request) <= 1800));
    await run.dispose();
    assert.deepEqual(h.ctx.agents.list(), [h.parent]);
  } finally { await host.dispose(); await h.dispose(); }
});

test("a child request route change fails before any provider work", async () => {
  const h = await harness(new ScriptedModel(async function* () { yield* textResponse("must-not-run"); }));
  const host = createExecutorHost(h.ctx, "understand-book-executor", async binding => {
    binding.agent!.ctx.on("agent/request", async (_request, next) => ({ ...await next(), model: "changed" }));
  });
  try {
    const run = await host.launch({ parent: h.parent, signal: AbortSignal.timeout(10000),
      opaque_handoff_ref: `abhandoff1_${"2".repeat(64)}`, runtime: probeRuntime({ max_output_tokens: 100, context_window_tokens: 2000 }) });
    assert.equal((await run.terminal).diagnostic_code, "model_configuration_changed");
    assert.equal(h.model.requests.length, 0);
    await run.dispose();
  } finally { await host.dispose(); await h.dispose(); }
});
