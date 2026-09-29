import assert from "node:assert/strict";
import { test } from "node:test";
import { ToolCallId } from "@deepseek-ai/dsh-llm";
import { createExecutorHost } from "../src/executor-host.ts";
import { DSH_EXECUTOR_TOOL_NAMES, installExecutorTools } from "../src/executor-tools.ts";
import { BUILD_EXECUTOR_TOOL_CONTRACTS_V3 } from "../../core/src/build-executor-tool-contract.ts";
import { harness, probeRuntime, ScriptedModel, textResponse, toolResponse } from "./harness.ts";

test("the shipped four schemas run through Native and reject extra fields before forwarding", async () => {
  const handoff = `abhandoff1_${"a".repeat(64)}`;
  const session = `absession1_${"b".repeat(64)}`;
  const input = `abinput1_${"c".repeat(64)}`;
  const sink = `absink1_${"d".repeat(64)}`;
  const requests = [
    { version: "automatic_build_executor_open_request.v3", opaque_handoff_ref: handoff },
    { version: "automatic_build_executor_input_next_request.v4", opaque_session_ref: session, generation_input_ref: input },
    { version: "automatic_build_executor_generation_start_request.v3", opaque_session_ref: session, generation_input_ref: input, confirmed_through_ordinal: 0 },
    { version: "automatic_build_executor_candidate_submit.v3", opaque_session_ref: session, candidate_sink_ref: sink, candidate: { nested: [null, true, "中文"] } },
  ];
  let step = 0;
  const calls: unknown[] = [];
  const model = new ScriptedModel(async function* (request) {
    assert.deepEqual(request.tools?.map(t => t.name).sort(), Object.values(DSH_EXECUTOR_TOOL_NAMES).sort());
    if (step < 4) {
      const current = step++;
      yield* toolResponse(`contract-${current}`, DSH_EXECUTOR_TOOL_NAMES[BUILD_EXECUTOR_TOOL_CONTRACTS_V3[current].name], requests[current]);
    } else yield* textResponse("done");
  });
  const h = await harness(model);
  const host = createExecutorHost(h.ctx, "understand-book-executor", async binding => {
    installExecutorTools(binding, async (_name, request) => { calls.push(request); return { accepted: true }; });
    const invalid = await binding.agent!.ctx.tools.execute({
      callId: ToolCallId("extra"), name: DSH_EXECUTOR_TOOL_NAMES["executor.open"],
      arguments: { ...requests[0], profile: "model-chosen" }, agent: binding.agent!, signal: new AbortController().signal,
    });
    assert.equal(invalid.isError, true);
    assert.equal(calls.length, 0);
  });
  try {
    const run = await host.launch({ parent: h.parent, signal: new AbortController().signal,
      opaque_handoff_ref: handoff, runtime: probeRuntime() });
    assert.equal((await run.terminal).stop_reason, "completed");
    assert.deepEqual(calls, requests);
    await run.dispose();
  } finally { await host.dispose(); await h.dispose(); }
});
