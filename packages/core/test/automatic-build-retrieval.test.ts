import { existsSync, mkdtempSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { compileBuildMode } from "../src/build-capability";
import { attachBuildPlanDigest, computeBuildPlanDigest, transitionBuildPlan, validateBuildPlanV1, type BuildRetrievalSelection } from "../src/build-intent";
import { prepareAutomaticBuildSnapshot, resolveAutomaticBuildTarget } from "../src/build-orchestrator";
import { prepareBuildRetrieval, readBuildRetrievalState, retrievalConfigurationMatches, retrievalRemainingWork } from "../src/automatic-build-retrieval";
import { summarizeRetrievalUsage } from "../src/automatic-build-budget";
import { projectRetrievalCatalog, retrievalCatalog } from "../src/semantic-retrieval";
import { STRUCTURE_RETRIEVAL } from "../src/book-structure-retrieval";
import { retrievalDependencies } from "../src/semantic-retrieval-preparation";
import { fakeEmbedding } from "./fixtures/embedding-provider";
import { fixture as gold } from "../testdata/semantic-retrieval/gold";
import { automaticBuildNextWithPreparation, automaticBuildPlan } from "../../../skills/build/automatic-build";
import { DSH_BUILD_EXECUTION_PROFILE_V1 } from "../src/build-execution-profile";

function fixture(mode: "lexical_only" | "semantic_required" = "semantic_required") {
  const root = mkdtempSync(path.join(tmpdir(), "sr4a-")), source = path.join(root, "guide.md");
  writeFileSync(source, "# Guide\n\nOne source paragraph.\n");
  const target = resolveAutomaticBuildTarget(source, root), provider = fakeEmbedding();
  provider.limits.max_input_tokens = 10000;
  const selection: BuildRetrievalSelection = { retrieval_mode: mode,
    provider: { identity: structuredClone(provider.identity), location: "local" },
    data_scope: "current_and_previous_formal_object_projections_and_queries" };
  const draft = compileBuildMode({ mode: "standard_deep", book_id: target.book_id,
    source_fingerprint: target.target_ref.input_fingerprint,
    content_profile: { id: "technical_learning", version: "technical_learning_v0" },
    plan_id: "sr4a-plan", revision: 1, created_at: "2026-10-01T08:00:00.000Z", budget: { on_exceed: "needs_user" },
    public_freshness: [], retrieval: { selection, estimate: { records: null, queries: null, basis: "unknown_until_fragments" },
      budget: { max_documents: 100, max_queries: 20, max_calls: 40 } },
  }).plan!;
  const plan = transitionBuildPlan(draft, "confirmed", { at: draft.created_at, confirmation_source: "codex_conversation" });
  const { work, previous } = gold();
  const request = { slot: "test/0", dependencies: retrievalDependencies(projectRetrievalCatalog(retrievalCatalog(work.proposal, previous)),
    { kind: "search", query: "速度" }, mode, selection.provider!.identity) };
  return { root, source, target, provider, draft, plan, runtime: { selection, provider }, request, workspace: target.workspace_dir };
}

describe("SR4a confirmed retrieval preparation", () => {
  it("prepares both authorized consumers without overwriting their document caches", async () => {
    const f = fixture();
    const selection = { ...f.runtime.selection, data_scope: "book_structure_and_formal_object_projections_and_queries" as const };
    const plan = attachBuildPlanDigest({ ...f.plan, retrieval: { ...f.plan.retrieval!, selection } });
    const common = { ...f, plan, runtime: { selection, provider: f.provider } };
    const structure = { slot: "book-structure/test/0", dependencies: { ...f.request.dependencies,
      projection_version: STRUCTURE_RETRIEVAL.projection_version } };
    expect(await prepareBuildRetrieval({ ...common, request: structure })).toBeUndefined();
    const structureCalls = f.provider.calls.length;
    expect(await prepareBuildRetrieval(common)).toBeUndefined();
    expect(f.provider.calls.length).toBeGreaterThan(structureCalls);
    const state = readBuildRetrievalState(f.workspace);
    expect(retrievalRemainingWork(f.workspace, structure).documents).toBe(0);
    expect(retrievalRemainingWork(f.workspace, f.request).documents).toBe(0);
    expect(state.prepared[structure.slot].dependencies.projection_version).toBe(STRUCTURE_RETRIEVAL.projection_version);
    expect(state.prepared[f.request.slot].dependencies.projection_version).toBe(f.request.dependencies.projection_version);
    const calls = f.provider.calls.length;
    await prepareBuildRetrieval({ ...common, request: structure }); await prepareBuildRetrieval(common);
    expect(f.provider.calls).toHaveLength(calls);
  });
  it("opens the local session only for missing preparation and disposes it on failure and success", async () => {
    const f = fixture(); let opens = 0, closes = 0;
    const runtime = { selection: f.runtime.selection, open_provider: async () => {
      opens++; return { provider: f.provider, dispose: async () => { closes++; } };
    } };
    automaticBuildPlan(f.source, f.root, { build_plan: f.plan });
    expect(opens).toBe(0);
    const embed = f.provider.embed_query;
    f.provider.embed_query = async () => { throw new Error("local failure"); };
    expect(await prepareBuildRetrieval({ ...f, runtime })).toMatchObject({ reason: "provider_failed" });
    expect([opens, closes]).toEqual([1, 1]);
    f.provider.embed_query = embed;
    expect(await prepareBuildRetrieval({ ...f, runtime })).toBeUndefined();
    expect([opens, closes]).toEqual([2, 2]);
    expect(await prepareBuildRetrieval({ ...f, runtime })).toBeUndefined();
    expect([opens, closes]).toEqual([2, 2]);
    const changed = fixture(); changed.provider.identity.model_revision = "drift";
    expect(await prepareBuildRetrieval({ ...changed, runtime: { selection: changed.runtime.selection,
      open_provider: async () => ({ provider: changed.provider, dispose: async () => { closes++; } }) } }))
      .toMatchObject({ reason: "build_plan_retrieval_drift" });
    expect(changed.provider.calls).toEqual([]); expect(closes).toBe(3);
  });
  it("stops a transient retry at the confirmed quota instead of hiding an extra attempt", async () => {
    const f = fixture(); f.request.dependencies.records = f.request.dependencies.records.slice(0, 1);
    f.provider.max_retries = 1;
    let attempts = 0;
    f.provider.embed_documents = async () => { attempts++; throw Object.assign(new Error("busy"), { code: "EBUSY" }); };
    f.plan.retrieval!.budget.max_documents = 1; f.plan = attachBuildPlanDigest(f.plan);
    expect(await prepareBuildRetrieval(f)).toMatchObject({ reason: "build_plan_budget_changed" });
    expect(attempts).toBe(1);
    expect(readBuildRetrievalState(f.workspace).calls).toHaveLength(1);
  });
  it("binds selection, location, data scope and quotas in the existing plan identity", () => {
    const { plan } = fixture();
    for (const mutate of [
      (p: typeof plan) => { p.retrieval!.selection.retrieval_mode = "lexical_only"; },
      (p: typeof plan) => { p.retrieval!.selection.provider!.location = "remote"; },
      (p: typeof plan) => { p.retrieval!.selection.provider!.identity.model_revision = "2"; },
      (p: typeof plan) => { p.retrieval!.budget.max_calls++; },
    ]) {
      const changed = structuredClone(plan); mutate(changed);
      expect(computeBuildPlanDigest(changed)).not.toBe(plan.plan_digest);
      expect(() => validateBuildPlanV1(changed)).toThrow("plan_digest");
    }
    expect(() => validateBuildPlanV1({ ...plan, retrieval: { ...plan.retrieval, selection: { ...plan.retrieval!.selection, data_scope: "entire_book" } } })).toThrow();
  });
  it("keeps unconfirmed, configuration-drift and ordinary plan reads at zero calls", async () => {
    const f = fixture();
    const prepare = (plan = f.plan, runtime = f.runtime) => prepareAutomaticBuildSnapshot(f.target, "formal_objects", { authorization: { plan, runtime } });
    expect(await prepare(f.draft)).toMatchObject({ status: "needs_user", action: { reason: "build_plan_unconfirmed" } });
    const drift = structuredClone(f.runtime.selection); drift.provider!.location = "remote";
    expect(await prepare(f.plan, { ...f.runtime, selection: drift })).toMatchObject({ status: "needs_user", action: { reason: "build_plan_retrieval_drift" } });
    f.provider.identity.model_id = "changed";
    expect(await prepare()).toMatchObject({ status: "needs_user", action: { reason: "build_plan_retrieval_drift" } });
    expect(retrievalConfigurationMatches(f.plan, f.runtime)).toBe(false);
    automaticBuildPlan(f.source, f.root, { build_plan: f.plan });
    expect(await automaticBuildNextWithPreparation(f.source, f.root, 1, { build_plan: f.draft, retrieval_runtime: f.runtime }))
      .toMatchObject({ action: { reason: "build_plan_unconfirmed" } });
    expect(f.provider.calls).toEqual([]);
    expect(existsSync(path.join(f.workspace, ".build/teaching/retrieval.json"))).toBe(false);
  });
  it.each(["documents", "queries", "calls", "input_tokens", "total_tokens", "wall_clock"] as const)("rejects insufficient %s before the first provider call", async limit => {
    const f = fixture();
    if (limit === "total_tokens") f.plan.budget.max_total_tokens = 0;
    else if (limit === "wall_clock") f.plan.budget.max_wall_clock_minutes = 0;
    else f.plan.retrieval!.budget[`max_${limit}`] = 0;
    f.plan = attachBuildPlanDigest(f.plan);
    expect(await prepareBuildRetrieval(f)).toMatchObject({ reason: "build_plan_budget_changed" });
    expect(f.provider.calls).toEqual([]);
    expect(readBuildRetrievalState(f.workspace).calls).toEqual([]);
    expect(readBuildRetrievalState(f.workspace).selection).toEqual(f.runtime.selection);
    expect(existsSync(path.join(f.workspace, ".build/semantic-retrieval/cache.json"))).toBe(false);
  });
  it("records successful and failed attempts, respects remaining quota on retry, retains unknown usage", async () => {
    const f = fixture();
    const original = f.provider.embed_documents;
    let batches = 0;
    f.provider.embed_documents = async (texts, options) => {
      if (++batches === 2) throw new Error("rate limited");
      return { ...await original(texts, options), usage: { input_tokens: 50, cost: { amount: 0.001, currency: "USD" } } };
    };
    expect(await prepareBuildRetrieval(f)).toMatchObject({ reason: "provider_failed" });
    expect(summarizeRetrievalUsage(readBuildRetrievalState(f.workspace).calls)).toMatchObject({ documents: 16, queries: 0, calls: 2,
      known_input_tokens: 50, unknown_usage_calls: 1 });
    expect(retrievalRemainingWork(f.workspace, f.request)).toMatchObject({ records: 28, documents: 20, queries: 1 });
    f.provider.embed_documents = original;
    f.plan.retrieval!.budget.max_documents = 28;
    f.plan = attachBuildPlanDigest(f.plan);
    const calls = f.provider.calls.length;
    expect(await prepareBuildRetrieval(f)).toMatchObject({ reason: "build_plan_budget_changed" });
    expect(f.provider.calls).toHaveLength(calls);
    f.plan.revision++; f.plan.retrieval!.budget.max_documents = 40; f.plan = attachBuildPlanDigest(f.plan);
    expect(await prepareBuildRetrieval(f)).toBeUndefined();
    const state = readBuildRetrievalState(f.workspace);
    expect(summarizeRetrievalUsage(state.calls)).toMatchObject({ documents: 36, queries: 1, calls: 6 });
    expect(state.calls[0].usage?.cost).toEqual({ amount: 0.001, currency: "USD" });
    const before = readFileSync(path.join(f.workspace, ".build/teaching/retrieval.json"), "utf8");
    unlinkSync(path.join(f.workspace, ".build/semantic-retrieval/cache.json"));
    expect(await prepareBuildRetrieval({ ...f, runtime: { selection: f.runtime.selection } })).toBeUndefined();
    expect(readFileSync(path.join(f.workspace, ".build/teaching/retrieval.json"), "utf8")).toBe(before);
  });
  it("keeps lexical and empty browse provider-free, and shares a prepared page across Harness choices", async () => {
    const f = fixture("lexical_only");
    expect(await prepareBuildRetrieval(f)).toBeUndefined();
    expect(readBuildRetrievalState(f.workspace).prepared[f.request.slot].sequence.hits.length).toBeGreaterThan(0);
    const semantic = fixture();
    semantic.request.dependencies.request = { kind: "search", query: "" };
    expect(await prepareBuildRetrieval(semantic)).toBeUndefined();
    expect(readBuildRetrievalState(semantic.workspace).prepared[semantic.request.slot].sequence.hits).toHaveLength(28);
    const first = automaticBuildPlan(f.source, f.root, { build_plan: f.plan });
    const second = automaticBuildPlan(f.source, f.root, { build_plan: f.plan, execution_profile: DSH_BUILD_EXECUTION_PROFILE_V1 });
    expect(second.snapshot.target).toEqual(first.snapshot.target);
    expect(f.provider.calls).toEqual([]); expect(semantic.provider.calls).toEqual([]);
    expect(existsSync(path.join(f.workspace, ".build/semantic-retrieval/cache.json"))).toBe(false);
  });
});
