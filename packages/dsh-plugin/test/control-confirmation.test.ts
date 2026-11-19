import assert from "node:assert/strict";
import { test } from "node:test";
import { existsSync, mkdirSync, mkdtempSync, writeFileSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { createBuildControl } from "../src/build-control.ts";
import { harness, ScriptedModel, textResponse } from "./harness.ts";
import { buildSourceManifest } from "../../core/src/source-manifest.ts";
import { readTeachingBuildInput, resolveAutomaticBuildTarget } from "../../core/src/build-orchestrator.ts";

class MetadataModel extends ScriptedModel {
  async resolveModel(provider: string, model: string) { return { provider, id: model, name: model, context: { contextWindow: 131072 } }; }
}

test("installed DSH displays and freezes the accepted local course route", { timeout: 60000 }, async () => {
  const root = mkdtempSync(path.join(tmpdir(), "ub-dsh-course-route-"));
  const book = path.join(root, ".understand-book", "book");
  const source = path.join(root, "book.md");
  const text = "# First\n\nA definition.\n\n# Second\n\nAn application.\n";
  mkdirSync(book, {recursive:true});
  writeFileSync(source,text);
  writeFileSync(path.join(book,"source.txt"),text);
  const json = (name:string, value:unknown) => writeFileSync(path.join(book,name),JSON.stringify(value));
  json("source_manifest.json",buildSourceManifest({book_id:"book",source_path:source}));
  json("profile_metadata.json",{header:{book_id:"book",profile_id:"technical_learning"}});
  json("base.json",{book_id:"book",lid_nodes:[],graph_nodes:[],graph_edges:[]});
  json("discourse_index.json",{items:[]});
  json("formula_semantics.json",[]);
  json("book_structure.json",{spine:["1","2"].map(lid=>({lid,summary:{text:"Definitions",evidence_lids:[]},key_stop_ids:[],depends_on:[]})),throughlines:[],key_stops:[]});
  const target=resolveAutomaticBuildTarget(book,root);
  const route={status:"accepted",source_id:"book",source_revision:readTeachingBuildInput(target).source.source_revision,
    title:"应用定义",question:"怎样核对一个判断的成立条件？",audience:"初学者",final_task:"判断新例并说明依据",example_lessons:["需要哪些前提？"],unit_lids:["2"]};
  json("accepted-route.json",route);
  const h=await harness(new MetadataModel(async function*(){yield* textResponse("must not run");}));
  const registry=path.join(root,"registry");
  const control=createBuildControl(h.ctx,{executable:process.env.UNDERSTAND_BOOK_TEST_NODE!,prefixArgs:["--import","tsx",fileURLToPath(new URL("engine-source-cli.ts",import.meta.url))],
    driverRoot:registry,maxOutputTokens:4096,safetyMarginTokens:4096,handoffsPerCall:1},"understand-book-executor");
  const remove=h.ctx.on("user-questions/request",async request=>{
    assert.ok(request.questions[0].detail!.includes(`课程路线：${route.question}；章节：2`));
    return {answers:[{id:request.questions[0].id,selected:["批准"]}]};
  });
  try {
    const invocation=await control.prepareAndConfirm(h.parent,new AbortController().signal,{target_input:book,root_dir:root,build_course:true,course_route_path:path.join(book,"accepted-route.json")}) as any;
    assert.match(invocation.invocation_ref,/^abinv1_/u);
    const dir=path.join(registry,"invocations");
    const saved=JSON.parse(readFileSync(path.join(dir,readdirSync(dir)[0]),"utf8"));
    assert.deepEqual(JSON.parse(readFileSync(saved.input.build_plan_path,"utf8")).course_route,route);
    assert.equal(h.model.requests.length,0);
  } finally { remove(); await control.dispose(); await h.dispose(); }
});

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
    assert.ok(detail.includes("Pass2：关闭"));
    for (const text of ["semantic_required", "local", "MiniLM", "queries=4", "一跳", "0 USD"]) assert.ok(detail.includes(text), text);
    assert.equal(detail.includes("候选召回：既有本地词法检索。"), false);
    assert.equal(existsSync(path.join(registry, "invocations")), false);
    return { answers: [{ id: request.questions[0].id, selected: [approve ? "批准" : "拒绝"] }] };
  });
  try {
    const result = control.prepareAndConfirm(h.parent, new AbortController().signal, { target_input: source, root_dir: root, build_tutor: true,
      retrieval: { retrieval_mode: "semantic_required", budget: { max_documents: 12, max_queries: 4, max_calls: 12 } } });
    if (approve) {
      assert.match((await result as any).invocation_ref, /^abinv1_/u);
      const saved = JSON.parse(readFileSync(path.join(registry, "invocations", readdirSync(path.join(registry, "invocations"))[0]), "utf8"));
      const plan = JSON.parse(readFileSync(saved.input.build_plan_path, "utf8"));
      assert.equal(plan.status, "confirmed"); assert.equal(plan.retrieval.selection.retrieval_mode, "semantic_required");
      assert.equal(plan.public_stage_closure.includes("pass2"), false);
    } else { await assert.rejects(result, /build_plan_not_approved/); assert.equal(existsSync(path.join(registry, "invocations")), false); }
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
