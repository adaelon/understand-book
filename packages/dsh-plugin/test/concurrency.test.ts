import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, writeFileSync, readdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createBuildControl } from "../src/build-control.ts";
import { harness, textResponse, toolResponse } from "./harness.ts";
import { MetadataModel, objects } from "./control-fixture.ts";

for (const workers of [2, 3] as const) test(`real host ${workers} workers refill before slow child finishes`, { timeout: 180000 }, async () => {
  const root = mkdtempSync(path.join(tmpdir(), "ub-dh5-concurrency-"));
  const source = path.join(root, "book.md");
  writeFileSync(source, ["# Transactions", ...Array.from({ length: 900 }, (_, i) =>
    `Paragraph ${i + 1} contains deterministic semantic evidence for the concurrent build protocol.`)].join("\n\n"));
  const children = new Set<string>();
  const handoffs = new Set<string>();
  let first: string | undefined;
  let release!: () => void;
  const refill = new Promise<void>(resolve => { release = resolve; });
  let timeout = false;
  const timer = setTimeout(() => { timeout = true; release(); }, 90000);
  let peak = 0;
  const h = await harness(new MetadataModel(async function* (request) {
    if (!children.has(request.sessionId!)) {
      children.add(request.sessionId!);
      const ref = JSON.stringify(request.messages).match(/abhandoff1_[a-f0-9]{64}/u)![0];
      assert.equal(handoffs.has(ref), false); handoffs.add(ref);
      first ??= request.sessionId;
      if (children.size === 4) release();
    }
    peak = Math.max(peak, h.ctx.agents.list().length - 1);
    const response = objects(request.messages).filter(v => v.version === "automatic_build_executor_session.v4").at(-1);
    let name: string; let args: any;
    if (!response) {
      if (request.sessionId === first) await refill;
      name = "ub_executor_open"; args = { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: JSON.stringify(request.messages).match(/abhandoff1_[a-f0-9]{64}/u)![0] };
    } else if (response.action.kind === "DELIVER_INPUT") {
      name = "ub_executor_input_next"; args = response.action.next_request;
    } else if (response.action.kind === "INPUT_BATCH") {
      const b = response.action.batch;
      name = b.final_for_generation ? "ub_executor_generation_start" : "ub_executor_input_next";
      args = b.final_for_generation
        ? { version: "automatic_build_executor_generation_start_request.v3", opaque_session_ref: b.opaque_session_ref, generation_input_ref: b.generation_input_ref, confirmed_through_ordinal: b.last_ordinal }
        : { version: "automatic_build_executor_input_next_request.v4", opaque_session_ref: b.opaque_session_ref, generation_input_ref: b.generation_input_ref, ack_through_ordinal: b.last_ordinal };
    } else if (response.action.kind === "GENERATE") {
      const a = response.action;
      assert.equal(a.output_contract.stage, "pass1");
      name = "ub_executor_submit_candidate";
      args = { version: "automatic_build_executor_candidate_submit.v3", opaque_session_ref: a.opaque_session_ref, candidate_sink_ref: a.candidate_sink_ref, candidate: { nodes: [], edges: [] } };
    } else { assert.equal(response.action.kind, "DONE"); yield* textResponse("DH5-PRIVATE-FINAL"); return; }
    yield* toolResponse(`call-${h.model.requests.length}`, name, args);
  }));
  const trace = path.join(root, "engine-processes.jsonl");
  const config = { executable: process.env.UNDERSTAND_BOOK_TEST_NODE!, prefixArgs: ["--import", "tsx", fileURLToPath(new URL("engine-source-cli.ts", import.meta.url)), "--dh5-trace", trace],
    driverRoot: path.join(root, "registry"), maxOutputTokens: 4096, safetyMarginTokens: 4096, handoffsPerCall: 4 };
  const control = createBuildControl(h.ctx, config, "understand-book-executor");
  const remove = h.ctx.on("user-questions/request", async r => ({ answers: [{ id: r.questions[0].id, selected: ["批准"] }] }));
  try {
    const inv = await control.prepareAndConfirm(h.parent, new AbortController().signal, { target_input: source, root_dir: root, pass2: "disabled", max_parallel: workers }) as any;
    const running = control.run(h.parent, new AbortController().signal, inv.invocation_ref);
    assert.throws(() => control.run(h.parent, new AbortController().signal, inv.invocation_ref), /already_running/);
    const result = await running as any;
    assert.equal(timeout, false, `batch barrier: ${root}`);
    assert.equal(children.size, 4, JSON.stringify(result));
    assert.equal(peak, workers);
    assert.equal(result.host_observations, undefined, JSON.stringify(result));
    assert.equal(result.yielded, true);
    assert.deepEqual(h.ctx.agents.list(), [h.parent]);
    const dir = path.join(config.driverRoot, "dsh-observations", inv.invocation_ref);
    const observations = readdirSync(dir).map(f => JSON.parse(readFileSync(path.join(dir, f), "utf8")));
    assert.equal(observations.length, 4);
    assert.ok(observations.every(o => o.done === "committed"));
    const pids = readFileSync(trace, "utf8").trim().split("\n").map(l => JSON.parse(l)).filter(v => v.pid).map(v => v.pid);
    assert.equal(pids.length, 4);
    for (const pid of pids) assert.throws(() => process.kill(pid, 0), { code: "ESRCH" });
    assert.equal(JSON.stringify(h.parent.session.snapshotEvents()).includes("DH5-PRIVATE-FINAL"), false);
    console.log(JSON.stringify({ workers, root, peak, children: children.size, outcome: "refilled" }));
  } finally { clearTimeout(timer); release(); remove(); await control.dispose(); await h.dispose(); }
});
