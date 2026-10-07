import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createBuildControl } from "../src/build-control.ts";
import { harness, textResponse, toolResponse } from "./harness.ts";

import { MetadataModel, objects, candidateFor } from "./control-fixture.ts";

for (const pass2 of ["disabled", "enabled"] as const) test(`installed DSH and real stdio complete standard plan, Pass2 ${pass2}, yielding between handoffs`, { timeout: 240000 }, async () => {
  const root = mkdtempSync(path.join(tmpdir(), "ub-dsh-control-"));
  const source = path.join(root, "book.md");
  writeFileSync(source, "# Transactions\n\nTransactions group operations into one atomic change.\n");
  const stages = new Set<string>();
  const byChild = new Map<string, { input: string }>();
  const cancellation = new AbortController();
  let cancelOnce = pass2 === "enabled";
  let rejectCandidateOnce = pass2 === "enabled";
  let sawRetry = false;
  const h = await harness(new MetadataModel(async function* (request) {
    assert.deepEqual(request.tools!.map(t => t.name).sort(), ["ub_executor_open", "ub_executor_input_next", "ub_executor_generation_start", "ub_executor_submit_candidate"].sort());
    const state = byChild.get(request.sessionId!) ?? { input: "" }; byChild.set(request.sessionId!, state);
    const response = objects(request.messages).filter(v => v.version === "automatic_build_executor_session.v4" || v.version === "automatic_build_executor_mcp_error.v2").at(-1);
    if (response?.version === "automatic_build_executor_mcp_error.v2") throw new Error(`fixture Engine error: ${response.diagnostic_code}`);
    let name: string; let args: any;
    if (!response) {
      const ref = JSON.stringify(request.messages).match(/abhandoff1_[a-f0-9]{64}/u)?.[0]; assert.ok(ref);
      name = "ub_executor_open"; args = { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: ref };
    } else if (response.action.kind === "DELIVER_INPUT") {
      if (cancelOnce) { cancelOnce = false; cancellation.abort(); yield* textResponse("cancelled"); return; }
      name = "ub_executor_input_next"; args = response.action.next_request;
    } else if (response.action.kind === "INPUT_BATCH") {
      const batch = response.action.batch;
      for (const chunk of batch.chunks) if (chunk.segment === "semantic_input") state.input += chunk.payload_utf8;
      if (batch.final_for_generation) {
        name = "ub_executor_generation_start"; args = { version: "automatic_build_executor_generation_start_request.v3", opaque_session_ref: batch.opaque_session_ref,
          generation_input_ref: batch.generation_input_ref, confirmed_through_ordinal: batch.last_ordinal };
      } else {
        name = "ub_executor_input_next"; args = { version: "automatic_build_executor_input_next_request.v4", opaque_session_ref: batch.opaque_session_ref,
          generation_input_ref: batch.generation_input_ref, ack_through_ordinal: batch.last_ordinal };
      }
    } else if (response.action.kind === "GENERATE") {
      const a = response.action; stages.add(a.output_contract.stage);
      if (a.retry_feedback) sawRetry = true;
      writeFileSync(path.join(root, "last-fixture-input.txt"), state.input);
      const candidate = candidateFor(a.output_contract, state.input) as any;
      if (rejectCandidateOnce && a.output_contract.work_unit_kind === "structure_unit") {
        candidate.unit_card.candidate_key_stops[0].type = "application";
        rejectCandidateOnce = false;
      }
      name = "ub_executor_submit_candidate"; args = { version: "automatic_build_executor_candidate_submit.v3", opaque_session_ref: a.opaque_session_ref,
        candidate_sink_ref: a.candidate_sink_ref, candidate };
    } else {
      assert.equal(response.action.kind, "DONE"); assert.ok(["committed", "retryable_failure"].includes(response.action.status));
      yield* textResponse("PRIVATE-CHILD-FINAL"); return;
    }
    yield* toolResponse(`call-${h.model.requests.length}`, name, args);
  }));
  const control = createBuildControl(h.ctx, { executable: process.env.UNDERSTAND_BOOK_TEST_NODE!, prefixArgs: ["--import", "tsx", fileURLToPath(new URL("engine-source-cli.ts", import.meta.url))],
    driverRoot: path.join(root, "registry"), maxOutputTokens: 4096, safetyMarginTokens: 4096, handoffsPerCall: 1 }, "understand-book-executor");
  const remove = h.ctx.on("user-questions/request", async request => {
    assert.ok(request.questions[0].detail!.includes(`Pass2：${pass2 === "enabled" ? "启用" : "关闭"}`));
    assert.equal(request.questions[0].detail!.includes("Transactions group operations"), false);
    return { answers: [{ id: request.questions[0].id, selected: ["批准"] }] };
  });
  try {
    const invocation = await control.prepareAndConfirm(h.parent, new AbortController().signal, { target_input: source, root_dir: root, pass2 }) as any;
    assert.match(invocation.invocation_ref, /^abinv1_/u); assert.equal(h.model.requests.length, 0);
    if (pass2 === "enabled") {
      const cancelled = await control.run(h.parent, cancellation.signal, invocation.invocation_ref) as any;
      assert.equal(cancelled.invocation_ref, invocation.invocation_ref);
      assert.equal(cancelled.host_observation.code, "build_cancelled");
      assert.deepEqual(h.ctx.agents.list(), [h.parent]);
    }
    let result: any;
    for (let n = 0; n < 18; n++) {
      result = await control.run(h.parent, new AbortController().signal, invocation.invocation_ref);
      writeFileSync(path.join(root, "last-control.json"), JSON.stringify(result, null, 2));
      assert.equal(result.invocation_ref, invocation.invocation_ref);
      assert.equal(result.host_observation, undefined, JSON.stringify(result));
      assert.deepEqual(h.ctx.agents.list(), [h.parent]);
      if (result.step.action.kind === "DONE") break;
      if (result.step.action.kind === "NEEDS_USER") {
        assert.equal(result.step.action.reason, "executor_unavailable", JSON.stringify(result));
        assert.equal(result.yielded, true);
      }
    }
    assert.equal(result.step.action.kind, "DONE", root);
    assert.ok(stages.has("pass1")); assert.ok(stages.has("profile_sidecar")); assert.ok(stages.has("book_structure"));
    for (const stage of ["formal_objects", "cognitive_materials", "teaching_publish"]) assert.ok(stages.has(stage), stage);
    // One local claim has no eligible long-range pairs; Engine closes Pass2 without a model task.
    if (pass2 === "disabled") assert.equal(stages.has("pass2"), false);
    if (pass2 === "enabled") assert.equal(sawRetry, true);
    assert.equal(JSON.stringify(h.parent.session.snapshotEvents()).includes("PRIVATE-CHILD-FINAL"), false);
    assert.equal(JSON.stringify(result).includes("Transactions group"), false);
    console.log(JSON.stringify({ pass2, root, children: byChild.size, stages: [...stages], outcome: "DONE" }));
  } finally { remove(); await control.dispose(); await h.dispose(); }
});
