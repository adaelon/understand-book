import assert from "node:assert/strict";
import { test } from "node:test";
import { createExecutorHost } from "../src/executor-host.ts";
import { harness, probeRuntime, ScriptedModel, textResponse, toolResponse } from "./harness.ts";

const names = ["ub_executor_open", "ub_executor_input_next", "ub_executor_generation_start", "ub_executor_submit_candidate"];

test("Native model requests retain complete Unicode/formula/code results and candidate parameters", async () => {
  const payload = "中文 $x=\\alpha$\r\n```ts\nconst value = {x: 1};\n```\n".repeat(900) + "END-INPUT-SENTINEL";
  const candidate = { text: payload, nested: [null, { quoted: '"\\\r\n' }] };
  let step = 0;
  let submitted = false;
  const containsPayload = (value: unknown): boolean => {
    if (typeof value === "string") {
      try { return JSON.parse(value)?.payload === payload; } catch { return false; }
    }
    return !!value && typeof value === "object" && Object.values(value).some(containsPayload);
  };
  const model = new ScriptedModel(async function* (request) {
    assert.deepEqual(request.tools?.map(t => t.name).sort(), [...names].sort());
    if (step > 1) {
      assert.ok(containsPayload(request.messages));
    }
    if (step < 4) yield* toolResponse(`step-${step}`, names[step], step++ === 3 ? { candidate } : {});
    else yield* textResponse("PRIVATE-FINAL-SENTINEL");
  });
  const h = await harness(model);
  const host = createExecutorHost(h.ctx, "understand-book-executor", async binding => {
    for (const [index, name] of names.entries()) binding.agent!.ctx.tools.register({
      name, description: "DH0 native roundtrip",
      parameters: { type: "object", properties: { candidate: { type: "object" } } },
      output: { schema: { type: "object", additionalProperties: true }, render: (_args, value) => [{ type: "text", text: JSON.stringify(value) }] },
      async execute(args, exec) {
        binding.assertCaller(exec.agent);
        if (index === 3) { assert.deepEqual(args, { candidate }); submitted = true; }
        return index === 1 ? { payload } : { operation: index, status: "ok" };
      },
    });
  });
  try {
    const run = await host.launch({ parent: h.parent, signal: new AbortController().signal,
      opaque_handoff_ref: `abhandoff1_${"d".repeat(64)}`, runtime: probeRuntime({ max_output_tokens: 8192 }) });
    assert.deepEqual(await run.terminal, { stop_reason: "completed" });
    assert.equal(submitted, true);
    assert.equal(model.requests.length, 5);
    assert.ok(Buffer.byteLength(JSON.stringify(candidate)) > 32768);
    assert.equal(JSON.stringify(h.parent.session.snapshotEvents()).includes("PRIVATE-FINAL-SENTINEL"), false);
    await run.dispose();
  } finally { await host.dispose(); await h.dispose(); }
});

test("a published child honors cancellation and reaches quiescence", async () => {
  let entered!: () => void;
  const ready = new Promise<void>(resolve => { entered = resolve; });
  const h = await harness(new ScriptedModel(async function* (request) {
    entered();
    await new Promise<void>((resolve) => {
      if (request.signal?.aborted) resolve();
      else request.signal?.addEventListener("abort", () => resolve(), { once: true });
    });
    throw new Error("cancelled");
  }));
  const host = createExecutorHost(h.ctx, "understand-book-executor", async () => {});
  const abort = new AbortController();
  try {
    const run = await host.launch({ parent: h.parent, signal: abort.signal,
      opaque_handoff_ref: `abhandoff1_${"e".repeat(64)}`, runtime: probeRuntime() });
    await ready;
    abort.abort();
    assert.equal((await run.terminal).stop_reason, "aborted");
    await run.dispose();
    assert.deepEqual(h.ctx.agents.list(), [h.parent]);
  } finally { await host.dispose(); await h.dispose(); }
});
