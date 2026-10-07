import assert from "node:assert/strict";
import { test } from "node:test";
import { existsSync, mkdirSync, mkdtempSync, writeFileSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { createBuildControl } from "../src/build-control.ts";
import { harness, ScriptedModel, textResponse } from "./harness.ts";

class MetadataModel extends ScriptedModel {
  async resolveModel(provider: string, model: string) { return { provider, id: model, name: model, context: { contextWindow: 131072 } }; }
}

for (const approve of [false, true]) test(`SR5 shows local retrieval identity and quota before confirmation (approve=${approve})`, { timeout: 30000 }, async () => {
  const root = mkdtempSync(path.join(tmpdir(), "ub-dsh-sr5-confirm-")), source = path.join(root, "book.md");
  writeFileSync(source, "# Transactions\n\nAtomic operations.\n");
  const json = (file: string, value: unknown) => { mkdirSync(path.dirname(file), { recursive: true }); writeFileSync(file, JSON.stringify(value)); };
  const configFile = path.join(root, "embedding.json");
  json(configFile, { version: "local_embedding_config.v1", runtime_dir: "runtime", model_dir: "model" });
  json(path.join(root, "model/model-metadata.json"), { model: "Xenova/paraphrase-multilingual-MiniLM-L12-v2", revision: "sr5-test" });
  for (const name of ["@huggingface/transformers", "onnxruntime-node"]) json(path.join(root, "runtime/node_modules", name, "package.json"), { version: "sr5-test" });
  const prior = process.env.UNDERSTAND_BOOK_EMBEDDING_CONFIG; process.env.UNDERSTAND_BOOK_EMBEDDING_CONFIG = configFile;
  const h = await harness(new MetadataModel(async function* () { yield* textResponse("must not run"); }));
  const registry = path.join(root, "registry");
  const control = createBuildControl(h.ctx, { executable: process.env.UNDERSTAND_BOOK_TEST_NODE!, prefixArgs: ["--import", "tsx", fileURLToPath(new URL("engine-source-cli.ts", import.meta.url))],
    driverRoot: registry, maxOutputTokens: 4096, safetyMarginTokens: 4096, handoffsPerCall: 1 }, "understand-book-executor");
  let questions = 0;
  const remove = h.ctx.on("user-questions/request", async request => {
    questions++;
    const detail = request.questions[0].detail!;
    for (const text of ["semantic_required", "local", "MiniLM", "queries=4", "一跳", "0 USD"]) assert.ok(detail.includes(text), text);
    assert.equal(detail.includes("候选召回：既有本地词法检索。"), false);
    assert.equal(existsSync(path.join(registry, "invocations")), false);
    return { answers: [{ id: request.questions[0].id, selected: [approve ? "批准" : "拒绝"] }] };
  });
  try {
    const result = control.prepareAndConfirm(h.parent, new AbortController().signal, { target_input: source, root_dir: root, pass2: "disabled",
      retrieval: { retrieval_mode: "semantic_required", budget: { max_documents: 12, max_queries: 4, max_calls: 12 } } });
    if (approve) {
      assert.match((await result as any).invocation_ref, /^abinv1_/u);
      const saved = JSON.parse(readFileSync(path.join(registry, "invocations", readdirSync(path.join(registry, "invocations"))[0]), "utf8"));
      const plan = JSON.parse(readFileSync(saved.input.build_plan_path, "utf8"));
      assert.equal(plan.status, "confirmed"); assert.equal(plan.retrieval.selection.retrieval_mode, "semantic_required");
    } else { await assert.rejects(result); assert.equal(existsSync(path.join(registry, "invocations")), false); }
    assert.equal(questions, 1); assert.equal(h.model.requests.length, 0);
  } finally {
    remove(); await control.dispose(); await h.dispose();
    if (prior === undefined) delete process.env.UNDERSTAND_BOOK_EMBEDDING_CONFIG; else process.env.UNDERSTAND_BOOK_EMBEDDING_CONFIG = prior;
  }
});

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
