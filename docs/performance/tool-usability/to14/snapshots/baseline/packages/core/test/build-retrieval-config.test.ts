import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { configureBuildRetrieval, configuredBuildRetrievalRuntime, reviseBuildRetrieval, buildRetrievalReview } from "../src/build-retrieval-config";
import { localEmbeddingIdentity, openLocalEmbeddingProvider, LOCAL_EMBEDDING_MODEL } from "../src/local-embedding-provider";
import { confirmedStandardBuildPlan } from "./helpers/confirmed-build-plan";
import { runAutomaticBuildDriverCommand } from "../../../skills/build/automatic-build-driver";

function fixture() {
  const root = mkdtempSync(path.join(tmpdir(), "sr5-config-"));
  const json = (file: string, data: unknown) => { mkdirSync(path.dirname(file), { recursive: true }); writeFileSync(file, JSON.stringify(data)); };
  json(path.join(root, "model/model-metadata.json"), { model: LOCAL_EMBEDDING_MODEL, revision: "test-revision" });
  for (const name of ["@huggingface/transformers", "onnxruntime-node"]) json(path.join(root, "runtime/node_modules", name, "package.json"), { version: "test" });
  const config = path.join(root, "embedding.json");
  json(config, { version: "local_embedding_config.v1", runtime_dir: "runtime", model_dir: "model" });
  const source = path.join(root, "guide.md"); writeFileSync(source, "# Guide\n\nA transaction groups operations.\n");
  const plan = confirmedStandardBuildPlan(source, root), planPath = path.join(root, "plan.json"); json(planPath, plan);
  return { root, config, plan, planPath };
}
const budget = { max_documents: 12, max_queries: 4, max_calls: 12 };
describe("SR5 retrieval configuration and confirmation", () => {
  it("previews installed facts without importing a runtime, and keeps locations out of persisted selection", () => {
    const f = fixture(), retrieval = configureBuildRetrieval("semantic_required", budget, f.config);
    const plan = reviseBuildRetrieval(f.plan, retrieval);
    expect(plan.status).toBe("draft"); expect(plan.confirmed_at).toBeUndefined();
    expect(plan.revision).toBe(f.plan.revision + 1); expect(plan.plan_digest).not.toBe(f.plan.plan_digest);
    expect(JSON.stringify(plan)).not.toContain(f.root);
    expect(plan.retrieval!.selection.provider).toMatchObject({ location: "local", identity: { model_id: LOCAL_EMBEDDING_MODEL, dimensions: 384 } });
    const review = buildRetrievalReview(plan);
    for (const text of ["semantic_required", "local", LOCAL_EMBEDDING_MODEL, "未知", "一跳", "queries=4", "0 USD"]) expect(review).toContain(text);
  });
  it("uses a separate draft and requires confirmation of that exact plan", () => {
    const f = fixture(), original = readFileSync(f.planPath, "utf8");
    const result = runAutomaticBuildDriverCommand({ version: "build_retrieval_configure.v1", build_plan_path: f.planPath,
      retrieval_mode: "semantic_required", budget, config_file: f.config }) as { build_plan_path: string; plan: typeof f.plan };
    expect(readFileSync(f.planPath, "utf8")).toBe(original);
    expect(result.plan.status).toBe("draft");
    const request = { version: "build_retrieval_confirm.v1", build_plan_path: result.build_plan_path,
      plan_digest: result.plan.plan_digest, confirmation_source: "codex_conversation" };
    expect(() => runAutomaticBuildDriverCommand({ ...request, plan_digest: f.plan.plan_digest })).toThrow("changed");
    expect(runAutomaticBuildDriverCommand(request)).toMatchObject({ plan: { status: "confirmed", retrieval: result.plan.retrieval } });
  });
  it("keeps lexical provider-free and detects unavailable or drifted installations before inference", async () => {
    const f = fixture();
    const lexical = reviseBuildRetrieval(f.plan, configureBuildRetrieval("lexical_only", { max_documents: 0, max_queries: 0, max_calls: 0 }, "missing"));
    expect(configuredBuildRetrievalRuntime(lexical).open_provider).toBeUndefined();
    const semantic = reviseBuildRetrieval(f.plan, configureBuildRetrieval("semantic_required", budget, f.config));
    await expect(configuredBuildRetrievalRuntime(semantic, "missing").open_provider!()).rejects.toThrow();
    const config = { runtime_dir: path.join(f.root, "runtime"), model_dir: path.join(f.root, "model") };
    const identity = localEmbeddingIdentity(config);
    await expect(openLocalEmbeddingProvider(config, { ...identity, model_revision: "changed" })).rejects.toMatchObject({ code: "build_plan_retrieval_drift" });
    const controller = new AbortController(); controller.abort();
    await expect(openLocalEmbeddingProvider(config, identity, controller.signal)).rejects.toThrow();
  });
});
