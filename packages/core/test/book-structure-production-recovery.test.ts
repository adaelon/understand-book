import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { structureProductionFixture, structureProductionResponse } from "./helpers/book-structure-production";
import { attachBuildPlanDigest, type BuildRetrievalSelection } from "../src/build-intent";
import { prepareExplicitLegacyBuildPlan, automaticBuildNextWithPreparation } from "../../../skills/build/automatic-build";
import { nextAutomaticBuildAction, buildAutomaticBuildSnapshot, prepareAutomaticBuildSnapshot } from "../src/build-orchestrator";
import { prepareBuildRetrieval, readBuildRetrievalState } from "../src/automatic-build-retrieval";
import { fakeEmbedding } from "./fixtures/embedding-provider";
import { automaticBuildGenerationArtifactPath, buildSemanticArtifactEnvelopeV3 } from "../src/semantic-artifact";
import { readBookStructureGenerationArtifact, type BookStructureGenerationTaskV1 } from "../src/book-structure-generation";
import { DSH_BUILD_EXECUTION_PROFILE_V1 } from "../src/build-execution-profile";
import { buildRetrievalReview, buildRetrievalScope } from "../src/build-retrieval-config";

const tasks = (state: ReturnType<ReturnType<typeof structureProductionFixture>["get"]>) => state.pending_tasks.map(id => {
  const g = state.generation_tasks![id]; if (g.kind !== "book_structure") throw Error("structure task expected"); return g.task;
});
function semanticFixture(dsh = false) {
  const f = structureProductionFixture(2, dsh ? DSH_BUILD_EXECUTION_PROFILE_V1 : undefined), provider = fakeEmbedding();
  provider.limits.max_input_tokens = 10000;
  const selection: BuildRetrievalSelection = { retrieval_mode: "semantic_required", provider: { location: "local", identity: provider.identity }, data_scope: "book_structure_projections_and_queries" };
  const plan = attachBuildPlanDigest({ ...prepareExplicitLegacyBuildPlan(f.source, f.root, { pass2: "disabled" }).plan,
    retrieval: { selection, estimate: { records: null, queries: null, basis: "unknown_until_fragments" }, budget: { max_documents: 100, max_queries: 20, max_calls: 100 } } });
  const runtime = { selection, provider };
  return { ...f, selection, plan, runtime, provider };
}
describe("BSR5 current-input preparation and action recovery", () => {
  it.each([false, true])("ordinary observation/writer never embed; both transports resume cancellation and provider failure (DSH=%s)", async dsh => {
    const f = semanticFixture(dsh);
    let state = f.get(f.selection);
    for (let i = 0; i < 12 && !state.retrieval_preparation; i++) {
      tasks(state).forEach(t => f.submit(t)); state = f.get(f.selection);
    }
    expect(state.retrieval_preparation).toBeDefined(); expect(f.provider.calls).toEqual([]);
    const accepted = state.work_units!.map(t => t.work_unit_id);
    const preparation = { target: f.target, stage: "book_structure" as const,
      options: { execution_profile: dsh ? DSH_BUILD_EXECUTION_PROFILE_V1 : undefined,
        authorization: { plan: f.plan, runtime: f.runtime } } };
    const aborted = new AbortController(); aborted.abort();
    const cancelled = await prepareAutomaticBuildSnapshot(preparation.target, preparation.stage,
      { ...preparation.options, authorization: { ...preparation.options.authorization, signal: aborted.signal } });
    expect(cancelled).toMatchObject({ status: "needs_user", action: { reason: "retrieval_cancelled", stage: "book_structure" } });
    expect(f.get(f.selection).work_units!.map(t => t.work_unit_id)).toEqual(accepted);
    expect(await prepareAutomaticBuildSnapshot(preparation.target, preparation.stage, preparation.options)).toMatchObject({ status: "ready" });
    expect(f.provider.calls).toEqual([]); // Empty-query browse needs no embedding.
    state = f.get(f.selection); const search = tasks(state)[0]; f.submit(search);
    state = f.get(f.selection); expect(state.retrieval_preparation!.dependencies.request).toMatchObject({ query: "ownership" });
    const embed = f.provider.embed_query; f.provider.embed_query = async () => { throw Error("offline"); };
    expect(await prepareAutomaticBuildSnapshot(preparation.target, preparation.stage, preparation.options))
      .toMatchObject({ status: "needs_user", action: { reason: "retrieval_provider_failed" } });
    expect(f.get(f.selection).work_units!.map(t => t.work_unit_id)).toContain(search.descriptor.work_unit_id);
    f.provider.embed_query = embed;
    const documents = f.provider.calls.filter(c => c.role === "document").length;
    // The older host must prepare whichever stage is reachable, not hard-code T5A.
    const next = await automaticBuildNextWithPreparation(f.source, f.root, 1, { build_plan: f.plan, retrieval_runtime: f.runtime,
      execution_profile: dsh ? DSH_BUILD_EXECUTION_PROFILE_V1 : undefined });
    expect(next.action).not.toMatchObject({ reason: "preparation_required" });
    expect(f.provider.calls.filter(c => c.role === "document")).toHaveLength(documents);
    const calls = f.provider.calls.length;
    for (let i = 0; i < 8; i++) {
      state = f.get(f.selection); if (!state.pending_tasks.length) break;
      tasks(state).forEach(t => f.submit(t));
    }
    state = f.get(f.selection);
    expect(state.book_structure_materialized?.output.throughlines).toHaveLength(1);
    expect(f.provider.calls).toHaveLength(calls);
    expect(readBuildRetrievalState(f.target.workspace_dir).calls.some(c => c.status === "failed")).toBe(true);
  }, 120000);

  it("keeps an unresolved directory incomplete and preserves the previous publication", () => {
    const f = structureProductionFixture(2);
    f.write("book_structure.json", { previous: "readable" });
    let state = f.get();
    for (let i = 0; i < 20 && !state.structure_blocked; i++) {
      for (const task of tasks(state)) f.submit(task, "phase" in task.input && task.input.phase === "reconcile"
        ? { edits: [], merges: [], unresolved: ["The transfer prerequisite needs source review."] } : structureProductionResponse(task));
      state = f.get();
    }
    expect(state).toMatchObject({ closed: false, structure_blocked: "The transfer prerequisite needs source review.",
      book_structure_progress: { publication: "pending", organization: { reconciliation: "unresolved" } } });
    expect(nextAutomaticBuildAction({ ...buildAutomaticBuildSnapshot(f.target), stages: [state] }))
      .toMatchObject({ kind: "needs_user", reason: "structure_preparation_incomplete" });
    expect(JSON.parse(readFileSync(path.join(f.target.workspace_dir, "book_structure.json"), "utf8"))).toEqual({ previous: "readable" });
  }, 120000);

  it("can switch a prepared semantic run to lexical and formal-only plans without stale writer context", async () => {
    const f = semanticFixture();
    let state = f.get(f.selection);
    for (let i = 0; i < 12 && !state.retrieval_preparation; i++) { tasks(state).forEach(t => f.submit(t)); state = f.get(f.selection); }
    await prepareAutomaticBuildSnapshot(f.target, "book_structure", { authorization: { plan: f.plan, runtime: f.runtime } });
    expect(readBuildRetrievalState(f.target.workspace_dir).selection).toEqual(f.selection);
    const formal = { ...f.selection, data_scope: "current_and_previous_formal_object_projections_and_queries" as const };
    const lexical = { retrieval_mode: "lexical_only" as const, data_scope: "book_structure_projections_and_queries" as const };
    for (const selection of [lexical, formal]) {
      state = f.get(selection);
      const task = tasks(state)[0];
      expect(task).toBeDefined();
      expect(() => f.submit(task)).not.toThrow();
    }
    expect(f.provider.calls).toEqual([]);
  }, 120000);

  it("does not expand a formal-only confirmation, and rejects a structure request outside its scope", async () => {
    const f = semanticFixture();
    let state = f.get(f.selection);
    for (let i = 0; i < 12 && !state.retrieval_preparation; i++) { tasks(state).forEach(t => f.submit(t)); state = f.get(f.selection); }
    const formal = { ...f.selection, data_scope: "current_and_previous_formal_object_projections_and_queries" as const };
    const plan = attachBuildPlanDigest({ ...f.plan, retrieval: { ...f.plan.retrieval!, selection: formal } });
    await expect(prepareBuildRetrieval({ workspace: f.target.workspace_dir, plan, runtime: { selection: formal, provider: f.provider }, request: state.retrieval_preparation! }))
      .rejects.toThrow(/consumer scope/);
    expect(f.provider.calls).toEqual([]);
    expect(f.get(formal).retrieval_preparation).toBeUndefined();
    expect(buildRetrievalReview(f.plan)).toContain("结构投影");
    expect(buildRetrievalReview(f.plan)).not.toContain("正式对象投影：");
    expect(buildRetrievalScope(f.plan)).toBe("book_structure_and_formal_object_projections_and_queries");
  }, 120000);

  it("replays accepted actions against an unshown changed candidate, keeping other chapter selections and old receipts", () => {
    const f = structureProductionFixture(2);
    f.submit(tasks(f.get())[0]); // Source outline precedes discovery.
    const unitTasks = tasks(f.get());
    const first = unitTasks[0], ordinary = structureProductionResponse(first) as any;
    ordinary.candidate_key_stops = Array.from({ length: 26 }, (_, i) => ({ ...ordinary.candidate_key_stops[0], id: `c${String(i).padStart(2, "0")}`,
      meaning: `Distinct mechanism ${i}`, reason: { ...ordinary.candidate_key_stops[0].reason, text: `Distinct mechanism ${i}` } }));
    f.submit(first, ordinary); f.submit(unitTasks[1]);
    let state = f.get(), firstChapter = tasks(state)[0];
    if (!("phase" in firstChapter.input) || firstChapter.input.phase !== "chapter") throw Error("chapter expected");
    const firstInput = firstChapter.input;
    const firstRef = firstInput.context.catalog.candidates.find(c => c.unit_lid === firstInput.work.unit_lid)!.ref;
    f.submit(firstChapter, { kind: "inspect", refs: [firstRef] });
    // Finish the independent second chapter while the first is still browsing.
    const second = tasks(state)[1]; f.submit(second);
    state = f.get();
    const secondSelection = tasks(state).find(t => "phase" in t.input && t.input.phase === "chapter" && t.input.work.unit_lid !== ordinary.parent_unit_lid)!;
    f.submit(secondSelection);
    const receiptFile = automaticBuildGenerationArtifactPath(f.target, "book_structure", firstChapter.policy_generation_id, firstChapter.descriptor.work_unit_id);
    const receiptBytes = readFileSync(receiptFile, "utf8");
    const unitArtifact = readBookStructureGenerationArtifact(f.target, first)!;
    (unitArtifact.payload as any).candidate_key_stops[25].meaning = "Changed unshown mechanism";
    writeFileSync(automaticBuildGenerationArtifactPath(f.target, "book_structure", first.policy_generation_id, first.descriptor.work_unit_id), JSON.stringify(buildSemanticArtifactEnvelopeV3(unitArtifact)));
    state = f.get();
    expect(state.book_structure_progress!.organization!.chapters.done).toBe(1);
    expect(state.work_units!.map(t => t.work_unit_id)).toContain(firstChapter.descriptor.work_unit_id);
    const current = tasks(state)[0];
    if (!("phase" in current.input) || current.input.phase !== "chapter") throw Error("chapter expected");
    expect(current.input.context.catalog.candidates.find(c => c.local_id === "c25")!.meaning).toBe("Changed unshown mechanism");
    expect(current.input.work.inspecting).toEqual([firstRef]);
    expect(readFileSync(receiptFile, "utf8")).toBe(receiptBytes);
    expect(() => f.submit(current, { kind: "select", selection: { unit_lid: ordinary.parent_unit_lid, role: "foundation",
      summary: ordinary.summary_fragments[0], accepted_stop_refs: [firstRef], macro_stop_refs: [firstRef] } })).toThrow(/complete chapter index/);
    // Source mutation invalidates the current path; no stale writer acceptance.
    writeFileSync(f.target.source_path, readFileSync(f.target.source_path, "utf8") + "\nNew source paragraph.\n");
    expect(() => f.submit(current, { kind: "browse", offset: 24 })).toThrow(/changed/);
    expect(readFileSync(receiptFile, "utf8")).toBe(receiptBytes);
  }, 120000);
});
