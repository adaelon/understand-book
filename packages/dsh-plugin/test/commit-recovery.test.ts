import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, writeFileSync, readdirSync, readFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createBuildControl } from "../src/build-control.ts";
import { harness, textResponse, toolResponse } from "./harness.ts";
import { MetadataModel, objects, candidateFor } from "./control-fixture.ts";

for (const mode of ["lost-response", "crash-after-commit"] as const) test(`${mode}: preserve facts, recreate controller, reuse accepted work and finish`, { timeout: 180000 }, async () => {
  const root = mkdtempSync(path.join(tmpdir(), "ub-dh5-commit-"));
  const source = path.join(root, "book.md");
  writeFileSync(source, "# Transactions\n\nTransactions group operations into one atomic change.\n");
  const marker = path.join(root, "dropped-response");
  const inputs = new Map<string, string>();
  let pass1Generations = 0;
  let crashOnce = mode === "crash-after-commit";
  if (crashOnce) writeFileSync(marker, "response loss disabled for crash test");
  const h = await harness(new MetadataModel(async function* (request) {
    if (JSON.stringify(request.messages).includes("executor_connection_closed")) { yield* textResponse("PRIVATE-UNCERTAIN-FINAL"); return; }
    const r = objects(request.messages).filter(v => v.version === "automatic_build_executor_session.v4").at(-1);
    let name: string; let args: any;
    if (!r) {
      name = "ub_executor_open"; args = { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: JSON.stringify(request.messages).match(/abhandoff1_[a-f0-9]{64}/u)![0] };
    } else if (r.action.kind === "DELIVER_INPUT") { name = "ub_executor_input_next"; args = r.action.next_request; }
    else if (r.action.kind === "INPUT_BATCH") {
      const b = r.action.batch;
      for (const chunk of b.chunks) if (chunk.segment === "semantic_input") inputs.set(request.sessionId!, (inputs.get(request.sessionId!) ?? "") + chunk.payload_utf8);
      name = b.final_for_generation ? "ub_executor_generation_start" : "ub_executor_input_next";
      args = b.final_for_generation ? { version: "automatic_build_executor_generation_start_request.v3", opaque_session_ref: b.opaque_session_ref,
        generation_input_ref: b.generation_input_ref, confirmed_through_ordinal: b.last_ordinal }
        : { version: "automatic_build_executor_input_next_request.v4", opaque_session_ref: b.opaque_session_ref, generation_input_ref: b.generation_input_ref, ack_through_ordinal: b.last_ordinal };
    } else if (r.action.kind === "GENERATE") {
      if (r.action.output_contract.stage === "pass1") pass1Generations++;
      name = "ub_executor_submit_candidate"; args = { version: "automatic_build_executor_candidate_submit.v3", opaque_session_ref: r.action.opaque_session_ref,
        candidate_sink_ref: r.action.candidate_sink_ref, candidate: candidateFor(r.action.output_contract, inputs.get(request.sessionId!) ?? "") };
    } else { assert.equal(r.action.kind, "DONE");
      if (crashOnce) { crashOnce = false; throw new Error("PRIVATE-AFTER-COMMIT-CRASH"); }
      yield* textResponse("PRIVATE-FINAL"); return; }
    yield* toolResponse(`call-${h.model.requests.length}`, name, args);
  }));
  const config = { executable: process.env.UNDERSTAND_BOOK_TEST_NODE!, prefixArgs: ["--import", "tsx", fileURLToPath(new URL("engine-source-cli.ts", import.meta.url)), "--dh5-drop-commit", marker],
    driverRoot: path.join(root, "registry"), maxOutputTokens: 4096, safetyMarginTokens: 4096, handoffsPerCall: 8 };
  let control = createBuildControl(h.ctx, config, "understand-book-executor");
  const remove = h.ctx.on("user-questions/request", async r => ({ answers: [{ id: r.questions[0].id, selected: ["批准"] }] }));
  try {
    const inv = await control.prepareAndConfirm(h.parent, new AbortController().signal, { target_input: source, root_dir: root, pass2: "disabled", max_parallel: 3 }) as any;
    const interrupted = await control.run(h.parent, new AbortController().signal, inv.invocation_ref) as any;
    assert.ok(existsSync(marker));
    assert.equal(interrupted.host_observations.length, 1, JSON.stringify(interrupted));
    assert.equal(interrupted.host_observations[0].outcome, mode === "lost-response" ? "connection" : "runtime");
    assert.equal(interrupted.host_observations[0].last_operation, "executor.submit_candidate");
    assert.equal(interrupted.host_observations[0].done, mode === "lost-response" ? undefined : "committed");
    await control.dispose();
    control = createBuildControl(h.ctx, config, "understand-book-executor");
    const completed = await control.run(h.parent, new AbortController().signal, inv.invocation_ref) as any;
    assert.equal(completed.step.action.kind, "DONE", JSON.stringify(completed));
    assert.equal(pass1Generations, 1, "accepted work must not be regenerated");
    const dir = path.join(config.driverRoot, "dsh-observations", inv.invocation_ref);
    const observations = readdirSync(dir).map(f => JSON.parse(readFileSync(path.join(dir, f), "utf8")));
    assert.equal(observations.length, 4);
    assert.deepEqual(h.ctx.agents.list(), [h.parent]);
    assert.equal(JSON.stringify([interrupted, completed]).includes("PRIVATE-"), false);
    console.log(JSON.stringify({ mode, root, pass1Generations, observations: observations.length, outcome: "DONE" }));
  } finally { remove(); await control.dispose(); await h.dispose(); }
});
