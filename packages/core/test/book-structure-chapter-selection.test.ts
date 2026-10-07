import { it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { structureProductionFixture, structureProductionResponse } from "./helpers/book-structure-production";
import { freezeBookStructureGenerationTask, renderBookStructureGenerationTaskInput, type BookStructureGenerationTaskV1 } from "../src/book-structure-generation";
import { applyStructureOrganizationAction, requestStructureChapterSelectionContinuation, requestStructureChapterRevision } from "../src/book-structure-organization";
import { STRUCTURE_ORGANIZATION_PROMPTS } from "../src/book-structure-organization-prompts";

it("continues at the accepted chapter action boundary, preserves old frozen tasks and commits all staged stops through the writer", () => {
  const f = structureProductionFixture(1);
  const historical: BookStructureGenerationTaskV1[] = [];
  let boundary: BookStructureGenerationTaskV1 | undefined;
  for (let i = 0; i < 12; i++) {
    const s = f.get();
    const generation = s.generation_tasks![s.pending_tasks[0]];
    if (generation.kind !== "book_structure") throw Error("unexpected task");
    const task = generation.task;
    const action = structureProductionResponse(task);
    if (task.descriptor.kind === "structure_chapter" && (action as { kind?: string }).kind === "select") { boundary = task; break; }
    f.submit(task, action); historical.push(task);
  }
  expect(boundary).toBeDefined();
  if (!boundary || !("phase" in boundary.input) || boundary.input.phase !== "chapter") throw Error("missing chapter boundary");
  const oldPendingFile = freezeBookStructureGenerationTask(f.target, boundary);
  const oldPendingBytes = readFileSync(oldPendingFile, "utf8");
  const ordinal = Number(boundary.descriptor.work_unit_id.match(/:([0-9]+)-[a-f0-9]+$/)![1]);
  const request = { id: "output-capacity", unit_lid: boundary.input.work.unit_lid, from_action_ordinal: ordinal,
    issue: "Keep the complete teaching selection within each candidate request" };
  requestStructureChapterSelectionContinuation(f.target.workspace_dir, request);
  const s = f.get(), generation = s.generation_tasks![s.pending_tasks[0]];
  if (generation.kind !== "book_structure" || !("phase" in generation.task.input) || generation.task.input.phase !== "chapter") throw Error("missing continuation");
  const partTask = generation.task, p = generation.task.input;
  expect(partTask.descriptor.kind).toBe("structure_chapter_selection");
  expect(partTask.policy_generation_id).toBe("structure-chapter-selection.full.v1");
  expect(partTask.descriptor.execution_budget_proof.max_candidate_tokens).toBe(boundary.descriptor.execution_budget_proof.max_candidate_tokens);
  expect(p.work.seen_refs).toEqual(boundary.input.work.seen_refs);
  expect(p.work.evidence_lids).toEqual(boundary.input.work.evidence_lids);
  expect(p.work.inspecting).toEqual(boundary.input.work.inspecting);
  expect(p.work.reading).toEqual(boundary.input.work.reading);
  expect(p.work.selection_draft!.continuation).toEqual(request);
  expect(STRUCTURE_ORGANIZATION_PROMPTS.structure_chapter_selection).toContain('"kind":"select_stops"');
  for (const old of historical) {
    const current = s.generation_tasks![old.descriptor.work_unit_id];
    if (current.kind !== "book_structure") throw Error("old task disappeared");
    expect(current.task.descriptor).toEqual(old.descriptor);
    expect(renderBookStructureGenerationTaskInput(current.task)).toBe(renderBookStructureGenerationTaskInput(old));
    expect(() => freezeBookStructureGenerationTask(f.target, current.task)).not.toThrow();
  }
  const refs = p.context.catalog.candidates.filter(c => c.unit_lid === p.work.unit_lid).map(c => c.ref);
  f.submit(partTask, { kind: "select_stops", refs: refs.slice(0, 1) });
  const afterPart = f.get(), finalGeneration = afterPart.generation_tasks![afterPart.pending_tasks[0]];
  if (finalGeneration.kind !== "book_structure" || !("phase" in finalGeneration.task.input) || finalGeneration.task.input.phase !== "chapter") throw Error("missing final task");
  const finalTask = finalGeneration.task, work = finalGeneration.task.input.work;
  expect(work.selection_draft!.accepted_stop_refs).toEqual(refs.slice(0, 1));
  expect(work.evidence_lids).toEqual(expect.arrayContaining(p.context.catalog.candidates.find(c => c.ref === p.work.inspecting[0])!.evidence_lids));
  const selection = { unit_lid: p.work.unit_lid, role: "foundation", summary: p.context.catalog.candidates.find(c => c.ref === refs[0])!.reason,
    macro_stop_refs: refs.slice(0, 1) };
  const action = { kind: "select", selection };
  const result = applyStructureOrganizationAction(finalTask.input as typeof p, action);
  expect("result" in result && result.result?.selection.accepted_stop_refs).toEqual(refs.slice(0, 1));
  f.submit(finalTask, action);
  requestStructureChapterRevision(f.target.workspace_dir, { id: "source-omission", unit_lid: p.work.unit_lid,
    issue: "Include the second source mechanism", candidate_refs: refs.slice(1, 2) });
  const pending = () => { const state = f.get(), g = state.generation_tasks![state.pending_tasks[0]];
    if (g.kind !== "book_structure" || !("phase" in g.task.input) || g.task.input.phase !== "chapter") throw Error("missing chapter revision");
    return { task: g.task, input: g.task.input }; };
  let revision = pending();
  expect(revision.task.descriptor.kind).toBe("structure_chapter_selection");
  expect(revision.input.work.revision!.previous_selection.accepted_stop_refs).toEqual(refs.slice(0, 1));
  expect(revision.input.work.selection_draft!.accepted_stop_refs).toEqual(refs.slice(0, 1));
  f.submit(revision.task, { kind: "browse", offset: 0 });
  revision = pending();
  expect((revision.input.body as { index: { items: unknown[] } }).index.items).toHaveLength(refs.length);
  f.submit(revision.task, { kind: "inspect", refs: refs.slice(1, 2) });
  revision = pending(); f.submit(revision.task, { kind: "select_stops", refs: refs.slice(1, 2) });
  revision = pending();
  const revisedAction = { kind: "select", selection: { ...selection, summary: p.context.catalog.candidates.find(c => c.ref === refs[1])!.reason, macro_stop_refs: refs } };
  const revisedResult = applyStructureOrganizationAction(revision.input, revisedAction);
  expect("result" in revisedResult && revisedResult.result?.selection.accepted_stop_refs).toEqual(refs);
  f.submit(revision.task, revisedAction);
  for (let i = 0; i < 4 && !f.get().book_structure_materialized; i++) {
    const state = f.get(), g = state.generation_tasks![state.pending_tasks[0]];
    if (g.kind !== "book_structure") throw Error("unexpected remaining task");
    f.submit(g.task);
  }
  expect(f.get().book_structure_materialized).toBeDefined();
  expect(readFileSync(oldPendingFile, "utf8")).toBe(oldPendingBytes);
}, 60000);
