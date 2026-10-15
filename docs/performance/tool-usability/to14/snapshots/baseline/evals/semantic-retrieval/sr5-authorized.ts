import assert from "node:assert/strict";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { configureBuildRetrieval, configuredBuildRetrievalRuntime } from "../../packages/core/src/build-retrieval-config";
import { prepareBuildRetrieval, readBuildRetrievalState } from "../../packages/core/src/automatic-build-retrieval";
import { summarizeRetrievalUsage } from "../../packages/core/src/automatic-build-budget";
import { compileBuildMode } from "../../packages/core/src/build-capability";
import { attachBuildPlanDigest, transitionBuildPlan } from "../../packages/core/src/build-intent";
import { fixture } from "../../packages/core/testdata/semantic-retrieval/gold";
import { projectRetrievalCatalog, retrievalCatalog } from "../../packages/core/src/semantic-retrieval";
import { retrievalDependencies } from "../../packages/core/src/semantic-retrieval-preparation";

// Explicitly authorized SR5 synthetic local sample only: 12 documents / 4 queries / 12 calls.
const [configFile, workspace, output] = process.argv.slice(2);
if (!output) throw new Error("Usage: sr5-authorized.ts CONFIG WORKSPACE OUTPUT (requires a fresh explicit sample authorization)");
mkdirSync(workspace, { recursive: true });
if (readBuildRetrievalState(workspace).calls.length) throw new Error("Use a fresh workspace; do not silently reset the sample budget");
const retrieval = configureBuildRetrieval("semantic_required", { max_documents: 12, max_queries: 4, max_calls: 12 }, configFile);
const draft = compileBuildMode({ mode: "standard_deep", book_id: "sr5-synthetic", source_fingerprint: "sr5-synthetic-source",
  content_profile: { id: "technical_learning", version: "technical_learning_v0" }, plan_id: "sr5-authorized-local", revision: 1,
  created_at: new Date().toISOString(), budget: { on_exceed: "needs_user" }, public_freshness: [], retrieval }).plan!;
const plan = transitionBuildPlan(draft, "confirmed", { at: new Date().toISOString(), confirmation_source: "codex_conversation" });
writeFileSync(path.join(workspace, "confirmed-plan.json"), JSON.stringify(plan, null, 2));
const { work, previous } = fixture();
const records = projectRetrievalCatalog(retrievalCatalog(work.proposal, previous)).slice(0, 9);
const request = { slot: "sr5/initial", dependencies: retrievalDependencies(records, { kind: "search", query: "速度" },
  "semantic_required", retrieval.selection.provider!.identity) };
const runtime = configuredBuildRetrievalRuntime(plan, configFile);
const start = performance.now();
assert.equal(await prepareBuildRetrieval({ workspace, plan, runtime, request }), undefined);
const first = readBuildRetrievalState(workspace);
const firstCalls = first.calls.length;
const bytes = readFileSync(path.join(workspace, ".build/teaching/retrieval.json"), "utf8");
assert.equal(await prepareBuildRetrieval({ workspace, plan, runtime: { selection: runtime.selection }, request }), undefined);
assert.equal(readFileSync(path.join(workspace, ".build/teaching/retrieval.json"), "utf8"), bytes);

const controller = new AbortController();
request.slot = "sr5/cancel-resume";
request.dependencies.request = { kind: "search", query: "单位时间内位置的变化" };
const cancelledRuntime = { selection: runtime.selection, open_provider: async (signal?: AbortSignal) => {
  const session = await runtime.open_provider!(signal), query = session.provider.embed_query;
  session.provider.embed_query = (text, options) => {
    queueMicrotask(() => controller.abort(new Error("SR5 authorized cancellation probe")));
    return query(text, options);
  };
  return session;
} };
assert.equal((await prepareBuildRetrieval({ workspace, plan, runtime: cancelledRuntime, request, signal: controller.signal }))?.reason, "cancelled");
const cancelled = readBuildRetrievalState(workspace);
assert.equal(cancelled.calls.at(-1)!.status, "cancelled");
assert.deepEqual(cancelled.prepared["sr5/initial"], first.prepared["sr5/initial"]);
assert.equal(await prepareBuildRetrieval({ workspace, plan, runtime, request }), undefined);
const restored = readBuildRetrievalState(workspace);
assert.ok(restored.prepared[request.slot].sequence.hits.length);
assert.equal(restored.calls.filter(c => c.role === "document").reduce((n, c) => n + c.records, 0), 9);

const driftPlan = structuredClone(plan); driftPlan.revision++;
driftPlan.retrieval!.selection.provider!.identity.model_revision = "deliberately-unconfirmed-installation";
const changed = attachBuildPlanDigest(driftPlan);
const driftRequest = structuredClone(request); driftRequest.slot = "sr5/drift";
driftRequest.dependencies.provider = changed.retrieval!.selection.provider!.identity;
assert.equal((await prepareBuildRetrieval({ workspace, plan: changed, runtime: configuredBuildRetrievalRuntime(changed, configFile), request: driftRequest }))?.reason,
  "build_plan_retrieval_drift");
assert.equal(readBuildRetrievalState(workspace).calls.length, restored.calls.length);
const usage = summarizeRetrievalUsage(restored.calls);
assert.ok(usage.documents <= 12 && usage.queries <= 4 && usage.calls <= 12);
const report = { version: "sr5-authorized-local.v1", recorded_at: new Date().toISOString(),
  authorization: { max_documents: 12, max_queries: 4, max_calls: 12, remote_inference: false, model_download: false },
  identity: retrieval.selection.provider!.identity, records: records.length, initial_calls: firstCalls, usage,
  calls: restored.calls, cancellation: cancelled.failure, config_drift: readBuildRetrievalState(workspace).failure,
  checks: { confirmed_plan: true, batch_8_plus_1: true, valid_page_without_provider: true, cancelled_query_not_accepted: true,
    resumed_without_document_reembedding: true, prior_page_preserved: true, drift_zero_calls: true }, total_ms: performance.now() - start };
writeFileSync(output, JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify({ usage, total_ms: report.total_ms, checks: report.checks }));
