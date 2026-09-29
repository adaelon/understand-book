import assert from "node:assert/strict";
import { test } from "node:test";
import { existsSync, mkdtempSync, writeFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { createBuildControl } from "../src/build-control.ts";
import { harness, ScriptedModel, textResponse } from "./harness.ts";

class MetadataModel extends ScriptedModel {
  async resolveModel(provider: string, model: string) { return { provider, id: model, name: model, context: { contextWindow: 131072 } }; }
}

for (const mode of ["mixed", "decline", "empty", "no-answerer", "cancel", "dispose", "source-drift"] as const) test(`confirmation ${mode} never creates an invocation`, { timeout: 30000 }, async () => {
  const root = mkdtempSync(path.join(tmpdir(), "ub-dsh-confirm-"));
  const source = path.join(root, "book.md"); writeFileSync(source, "# Transactions\n\nAtomic operations.\n");
  const h = await harness(new MetadataModel(async function* () { yield* textResponse("must not run"); }));
  const control = createBuildControl(h.ctx, { executable: process.env.UNDERSTAND_BOOK_TEST_NODE!, prefixArgs: ["--import", "tsx", fileURLToPath(new URL("engine-source-cli.ts", import.meta.url))],
    driverRoot: path.join(root, "registry"), maxOutputTokens: 4096, safetyMarginTokens: 4096, handoffsPerCall: 1 }, "understand-book-executor");
  const abort = new AbortController();
  const remove = mode === "no-answerer" ? () => {} : h.ctx.on("user-questions/request", async request => {
    if (mode === "source-drift") writeFileSync(source, "# Changed book\n\nNew material after review.\n");
    if (mode === "cancel" || mode === "dispose") {
      if (mode === "cancel") abort.abort(); else void control.dispose();
      return new Promise<never>((_resolve, reject) => {
        if (request.signal?.aborted) reject(new Error("cancelled"));
        else request.signal?.addEventListener("abort", () => reject(new Error("cancelled")), { once: true });
      });
    }
    return { answers: [{ id: request.questions[0].id, selected: mode === "empty" ? [] : mode === "decline" ? ["拒绝"] : ["批准"],
      ...(mode === "mixed" ? { custom: "但请先修改预算" } : {}) }] };
  });
  try {
    await assert.rejects(control.prepareAndConfirm(h.parent, abort.signal, { target_input: source, root_dir: root, pass2: "disabled" }));
    assert.equal(existsSync(path.join(root, "registry", "invocations")), false);
    assert.equal(h.model.requests.length, 0);
  } finally { remove(); await control.dispose(); await h.dispose(); }
});
