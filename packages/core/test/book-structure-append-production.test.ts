import { cpSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { expect, it } from "vitest";
import { structureProductionFixture, structureProductionResponse } from "./helpers/book-structure-production";
import { buildAutomaticBuildSnapshot, prepareAutomaticBuildSnapshot, resolveAutomaticBuildTarget } from "../src/build-orchestrator";
import { automaticBuildPlan, automaticBuildNext, runAutomaticBuildCloseStage, prepareExplicitLegacyBuildPlan } from "../../../skills/build/automatic-build";
import { collectAutomaticBuildStageQuality } from "../src/automatic-build-quality";
import { openAutomaticBuildExecutorSessionV3, nextAutomaticBuildExecutorInput, startAutomaticBuildExecutorGeneration, submitAutomaticBuildExecutorCandidateV3 } from "../src/automatic-build-executor-session";

it("routes chapter references and theme actions through the opaque executor, publishes once and resumes after relocation", async () => {
  const f = structureProductionFixture();
  expect(prepareAutomaticBuildSnapshot(f.target, "book_structure").status).toBe("ready");
  const plan = prepareExplicitLegacyBuildPlan(f.source, f.root, { pass2: "disabled" }).plan;
  const oldPublic = path.join(f.target.workspace_dir, "book_structure.json");
  const oldBytes = JSON.stringify({ old: "previous publication remains readable until close" });
  writeFileSync(oldPublic, oldBytes);
  const executed = new Set<string>();
  let state = f.get();
  for (let turn = 0; state.pending_tasks.length && turn < 25; turn++) {
    for (const id of state.pending_tasks) {
      const generation = state.generation_tasks![id];
      if (generation.kind !== "book_structure") throw Error("structure task expected");
      const task = generation.task, candidate = JSON.parse(JSON.stringify(structureProductionResponse(task)));
      expect(state.closed).toBe(false); expect(readFileSync(oldPublic, "utf8")).toBe(oldBytes);
      if ((task.output_role !== "organization_action" && task.output_role !== "unit_observation") || executed.has(task.descriptor.kind)) { f.submit(task, candidate); continue; }
      const p = automaticBuildPlan(f.source, f.root, { build_plan: plan, requested_workers: 1, available_agent_slots: 1 });
      const next = automaticBuildNext(f.source, f.root, 1, { build_plan: plan, accepted_plan_digest: p.preflight!.descriptor_plan_digest,
        available_agent_slots: 1, executor_dispatches: true });
      if (!("dispatches" in next.action) || !next.action.dispatches?.length) throw Error(JSON.stringify(next.action));
      const dispatch = next.action.dispatches[0];
      expect(dispatch.manifest.ordered_work_unit_ids).toEqual([id]);
      let response = openAutomaticBuildExecutorSessionV3(dispatch.opaque_handoff_ref);
      for (let n = 0; n < 100; n++) {
        const a = response.action;
        if (a.kind === "DELIVER_INPUT") response = nextAutomaticBuildExecutorInput(a.next_request);
        else if (a.kind === "INPUT_BATCH") {
          response = a.batch.final_for_generation ? startAutomaticBuildExecutorGeneration({ version: "automatic_build_executor_generation_start_request.v3",
            opaque_session_ref: a.batch.opaque_session_ref, generation_input_ref: a.batch.generation_input_ref, confirmed_through_ordinal: a.batch.last_ordinal })
            : nextAutomaticBuildExecutorInput({ version: "automatic_build_executor_input_next_request.v4", opaque_session_ref: a.batch.opaque_session_ref,
              generation_input_ref: a.batch.generation_input_ref, ack_through_ordinal: a.batch.last_ordinal });
        } else if (a.kind === "GENERATE") {
          const request = { version: "automatic_build_executor_candidate_submit.v3" as const, opaque_session_ref: a.opaque_session_ref,
            candidate_sink_ref: a.candidate_sink_ref, candidate };
          response = submitAutomaticBuildExecutorCandidateV3(request);
          expect(response.action, JSON.stringify(response)).toMatchObject({ kind: "DONE", status: "committed" });
          expect(submitAutomaticBuildExecutorCandidateV3(request)).toEqual(response);
          break;
        } else throw Error(JSON.stringify(a));
      }
      executed.add(task.descriptor.kind);
    }
    state = f.get();
  }
  expect(state.structure_blocked).toBeUndefined(); expect(state.pending_tasks).toEqual([]);
  expect([...executed].sort()).toEqual(["structure_chapter", "structure_fragment", "structure_outline", "structure_theme", "structure_theme_plan", "structure_theme_reconcile"]);
  expect(state.work_units!.some(t => t.kind.startsWith("structure_relation") || t.kind.startsWith("structure_stitch"))).toBe(false);
  expect(state.book_structure_materialized!.output.key_stops).toHaveLength(6);
  expect(state.book_structure_materialized!.output.spine!.map(u => u.key_stop_ids.length)).toEqual([1, 1, 1]);
  expect(state.book_structure_materialized!.output.throughlines).toHaveLength(1);
  const quality = collectAutomaticBuildStageQuality(f.target, state, "full");
  expect(quality, JSON.stringify(quality)).toMatchObject({ gate_status: "passed", book_structure_publication: "ready",
    book_structure_leaf_coverage: { chapters: 3, expected: 9, covered: 9, gaps: 0, overlaps: 0 },
    book_structure_organization: { candidates: 6, chapters: { done: 3, total: 3 }, themes: { done: 1, total: 1 } } });
  expect(runAutomaticBuildCloseStage(f.source, f.root, "book_structure", { quality_profile: "full" })).toMatchObject({ status: "closed" });
  const bytes = readFileSync(oldPublic, "utf8");
  expect(collectAutomaticBuildStageQuality(f.target, f.get(), "full")).toMatchObject({ digest: quality.digest, book_structure_publication: "published" });
  expect(f.get().book_structure_progress).toMatchObject({ organization: { candidates: 6, chapters: { done: 3, total: 3 }, themes: { done: 1, total: 1, planned: true }, reconciliation: "complete" }, publication: "published" });
  expect(runAutomaticBuildCloseStage(f.source, f.root, "book_structure", { quality_profile: "full" })).toMatchObject({ status: "closed" });
  expect(readFileSync(oldPublic, "utf8")).toBe(bytes);
  const movedRoot = path.join(f.root, "moved"), moved = path.join(movedRoot, ".understand-book", f.target.book_id);
  cpSync(f.target.workspace_dir, moved, { recursive: true });
  const target = resolveAutomaticBuildTarget(moved, movedRoot);
  expect(buildAutomaticBuildSnapshot(target).stages.find(s => s.stage === "book_structure")!.pending_tasks).toEqual([]);
  rmSync(path.join(moved, "book_structure.json"));
  expect(prepareAutomaticBuildSnapshot(target, "book_structure").status).toBe("ready");
  expect(runAutomaticBuildCloseStage(moved, movedRoot, "book_structure", { quality_profile: "full" })).toMatchObject({ status: "closed" });
  expect(readFileSync(path.join(moved, "book_structure.json"), "utf8")).toBe(bytes);
}, 120000);
