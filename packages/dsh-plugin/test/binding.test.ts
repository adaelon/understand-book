import assert from "node:assert/strict";
import { test } from "node:test";
import { ToolCallId, createUserMessage } from "@deepseek-ai/dsh-llm";
import type { Agent } from "@deepseek-ai/dsh-agent";
import { createExecutorHost, type ExecutorBinding } from "../src/executor-host.ts";
import { harness, probeRuntime } from "./harness.ts";

const names = ["ub_executor_open", "ub_executor_input_next", "ub_executor_generation_start", "ub_executor_submit_candidate"];
const handoff = (char: string) => `abhandoff1_${char.repeat(64)}`;

test("real spawn binds before publication, isolates overlapping launches and rejects foreign identities", async () => {
  const h = await harness();
  const bindings: ExecutorBinding[] = [];
  const host = createExecutorHost(h.ctx, "understand-book-executor", async binding => {
    await Promise.resolve();
    bindings.push(binding);
    const agent = binding.agent!;
    for (const name of names) agent.ctx.tools.register({
      name, description: "DH0 synthetic operation",
      parameters: { type: "object", properties: {}, additionalProperties: false },
      output: { schema: { type: "object", additionalProperties: true }, render: (_args, value) => [{ type: "text", text: JSON.stringify(value) }] },
      async execute(_args, exec) { binding.assertCaller(exec.agent); return { input: "semantic-sentinel" }; },
    });
    // Actual registry dispatch before spawn returns a handle.
    const early = await agent.ctx.tools.execute({
      callId: ToolCallId("early"), name: names[0], arguments: {}, agent,
      signal: new AbortController().signal,
    });
    assert.equal(early.isError, false);
    assert.throws(() => binding.assertCaller(h.parent), /not_owned/);
    assert.throws(() => binding.assertCaller({ id: agent.id } as Agent), /not_owned/);
  });
  try {
    h.parent.followup(createUserMessage({ content: [{ type: "text", text: "parent-private-sentinel" }], source: { kind: "user" } }));
    await h.parent.whenIdle();
    const parentHistory = JSON.stringify(h.parent.session.snapshotEvents());
    const runs = await Promise.all(["a", "b"].map(char => host.launch({
      parent: h.parent, signal: new AbortController().signal,
      opaque_handoff_ref: handoff(char), runtime: probeRuntime(),
    })));
    assert.equal(bindings.length, 2);
    assert.notEqual(bindings[0].agent, bindings[1].agent);
    assert.throws(() => bindings[0].assertCaller(bindings[1].agent), /not_owned/);
    for (const run of runs) {
      assert.deepEqual(await run.terminal, { stop_reason: "completed" });
      const request = h.model.requests.find(r => JSON.stringify(r.messages).includes(run.binding.launch.opaque_handoff_ref))!;
      assert.ok(request);
      assert.deepEqual(request.tools?.map(t => t.name).sort(), [...names].sort());
      assert.equal(JSON.stringify(request.messages).includes("parent-private-sentinel"), false);
      assert.throws(() => run.binding.assertCaller(run.binding.agent), /not_owned/);
      await run.dispose();
      assert.equal(h.ctx.agents.get(run.child_id), undefined);
    }
    assert.equal(JSON.stringify(h.parent.session.snapshotEvents()).includes("semantic-sentinel"), false);
    assert.equal(parentHistory.includes("semantic-sentinel"), false);
    assert.equal(h.ctx.agentPresets.composedPreset(h.parent.ctx), "standard");
    assert.equal(h.ctx.tools.get(names[0], h.parent), undefined);
  } finally { await host.dispose(); await h.dispose(); }
});

test("cancelled and failed creation release real child resources", async () => {
  const h = await harness();
  const controller = new AbortController();
  const host = createExecutorHost(h.ctx, "understand-book-executor", async () => {
    controller.abort();
    throw new Error("probe_creation_failure");
  });
  try {
    await assert.rejects(host.launch({ parent: h.parent, signal: controller.signal,
      opaque_handoff_ref: handoff("c"), runtime: probeRuntime() }));
    assert.deepEqual(h.ctx.agents.list(), [h.parent]);
    assert.equal(h.model.requests.length, 0);
  } finally { await host.dispose(); await h.dispose(); }
});

test("host disposal waits for an in-flight creation to finish cleanup", async () => {
  const h = await harness();
  let entered!: () => void;
  const ready = new Promise<void>(resolve => { entered = resolve; });
  let release!: () => void;
  const finishPreparation = new Promise<void>(resolve => { release = resolve; });
  const host = createExecutorHost(h.ctx, "understand-book-executor", async () => {
    entered();
    await finishPreparation;
  });
  try {
    const launching = host.launch({ parent: h.parent, signal: new AbortController().signal,
      opaque_handoff_ref: handoff("f"), runtime: probeRuntime() });
    const rejected = assert.rejects(launching);
    await ready;
    let disposed = false;
    const disposal = host.dispose().then(() => { disposed = true; });
    await new Promise<void>(resolve => setImmediate(resolve));
    const returnedBeforeCleanup = disposed;
    release();
    await rejected;
    await disposal;
    assert.equal(returnedBeforeCleanup, false);
    assert.deepEqual(h.ctx.agents.list(), [h.parent]);
    assert.equal(h.model.requests.length, 0);
  } finally { release?.(); await host.dispose(); await h.dispose(); }
});

test("one overlapping creation failure does not retire its successful sibling", async () => {
  const h = await harness();
  let release!: () => void;
  const both = new Promise<void>(resolve => { release = resolve; });
  let entered = 0;
  const host = createExecutorHost(h.ctx, "understand-book-executor", async binding => {
    if (++entered === 2) release();
    await both;
    if (binding.launch.opaque_handoff_ref === handoff("a")) throw new Error("one_creation_failed");
  });
  try {
    const [failed, succeeded] = await Promise.allSettled(["a", "b"].map(char => host.launch({
      parent: h.parent, signal: new AbortController().signal, opaque_handoff_ref: handoff(char),
      runtime: probeRuntime(),
    })));
    assert.equal(failed.status, "rejected");
    if (succeeded.status !== "fulfilled") throw succeeded.reason;
    assert.deepEqual(await succeeded.value.terminal, { stop_reason: "completed" });
    assert.equal(h.model.requests.length, 1);
    await succeeded.value.dispose();
    assert.deepEqual(h.ctx.agents.list(), [h.parent]);
  } finally { release(); await host.dispose(); await h.dispose(); }
});

test("one resource cleanup rejection still cleans every sibling and unpublishes every child", async () => {
  const h = await harness();
  const cleaned: string[] = [];
  const host = createExecutorHost(h.ctx, "understand-book-executor", async binding => async () => {
    cleaned.push(binding.launch.opaque_handoff_ref);
    if (binding.launch.opaque_handoff_ref === handoff("a")) throw new Error("cleanup-failed");
  });
  try {
    const runs = await Promise.all(["a", "b", "c"].map(c => host.launch({ parent: h.parent,
      signal: new AbortController().signal, opaque_handoff_ref: handoff(c), runtime: probeRuntime() })));
    await Promise.all(runs.map(r => r.terminal));
    await assert.rejects(host.dispose(), /cleanup-failed/);
    assert.deepEqual(cleaned.sort(), ["a", "b", "c"].map(handoff));
    assert.deepEqual(h.ctx.agents.list(), [h.parent]);
    for (const run of runs) assert.throws(() => run.binding.assertCaller(run.binding.agent), /not_owned/);
  } finally { await host.dispose().catch(() => {}); await h.dispose(); }
});

test("a retired launch cannot call or dispose a new child in the same dispatch slot", async () => {
  const h = await harness();
  const host = createExecutorHost(h.ctx, "understand-book-executor", async () => {});
  try {
    const input = { parent: h.parent, signal: new AbortController().signal, runtime: probeRuntime(),
      dispatch_slot_ref: `abdispatchslot1_${"a".repeat(64)}` };
    const old = await host.launch({ ...input, launch_id: "old", opaque_handoff_ref: handoff("a") });
    await old.terminal; await old.dispose();
    const next = await host.launch({ ...input, launch_id: "next", opaque_handoff_ref: handoff("b") });
    assert.notEqual(old.child_id, next.child_id);
    await old.dispose();
    assert.equal(h.ctx.agents.get(next.child_id), next.binding.agent);
    assert.throws(() => old.binding.assertCaller(next.binding.agent), /not_owned/);
    await next.terminal; await next.dispose();
    assert.deepEqual(h.ctx.agents.list(), [h.parent]);
  } finally { await host.dispose(); await h.dispose(); }
});
