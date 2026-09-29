import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ToolCallId } from "@deepseek-ai/dsh-llm";
import * as plugin from "../src/index.ts";
import { harness, ScriptedModel, textResponse } from "./harness.ts";

class MetadataModel extends ScriptedModel {
  async resolveModel(provider: string, model: string) { return { provider, id: model, name: model, context: { contextWindow: 131072 } }; }
}

test("Cordis entry registers root tools and zero budget reaches a real Engine boundary", { timeout: 30000 }, async () => {
  const root = mkdtempSync(path.join(tmpdir(), "ub-dsh-entry-"));
  const target = path.join(root, "book.md"); writeFileSync(target, "# Transactions\n\nAtomic operations.\n");
  const h = await harness(new MetadataModel(async function* () { yield* textResponse("must not run"); }));
  try {
    await h.ctx.plugin(plugin, { executable: process.env.UNDERSTAND_BOOK_TEST_NODE!,
      prefixArgs: ["--import", "tsx", fileURLToPath(new URL("engine-source-cli.ts", import.meta.url))],
      driverRoot: path.join(root, "registry"), maxOutputTokens: 4096, safetyMarginTokens: 4096, handoffsPerCall: 1 });
    for (const name of ["ub_build_prepare_and_confirm", "ub_build_run"]) assert.ok(h.parent.ctx.tools.get(name));
    h.ctx.on("user-questions/request", async request => ({ answers: [{ id: request.questions[0].id,
      selected: request.questions[0].intent?.kind === "plan-review" ? ["批准"] : [] }] }));
    const prepared = await h.parent.ctx.tools.execute({ callId: ToolCallId("prepare"), name: "ub_build_prepare_and_confirm",
      agent: h.parent, signal: new AbortController().signal,
      arguments: { target_input: target, root_dir: root, pass2: "disabled", budget: { on_exceed: "needs_user", max_total_tokens: 0 } } });
    assert.equal(prepared.isError, false, JSON.stringify(prepared));
    const ref = JSON.stringify(prepared).match(/abinv1_[a-f0-9]{64}/u)?.[0]; assert.ok(ref);
    const ran = await h.parent.ctx.tools.execute({ callId: ToolCallId("run"), name: "ub_build_run", agent: h.parent,
      signal: new AbortController().signal, arguments: { invocation_ref: ref } });
    assert.equal(ran.isError, false, JSON.stringify(ran));
    assert.match(JSON.stringify(ran), /budget_exceeded/u);
    assert.equal(h.model.requests.length, 0);
    assert.deepEqual(h.ctx.agents.list(), [h.parent]);
  } finally { await h.dispose(); }
});
