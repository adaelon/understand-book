import { expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { structureProductionFixture, structureProductionResponse } from "./helpers/book-structure-production";
import { freezeBookStructureGenerationTask, renderBookStructureGenerationTaskInput, type BookStructureGenerationTaskV1 } from "../src/book-structure-generation";
import { requestStructureThemeRevision, readStructureThemeRevisionRequests } from "../src/book-structure-organization";
import type { StructureThemeResult } from "../src/book-structure-themes";

it("replays the original directory before targeted source revisions and commits only the named theme through its current writer", () => {
  const f = structureProductionFixture(2), historical: BookStructureGenerationTaskV1[] = [];
  const current = () => {
    const state = f.get(), generation = state.generation_tasks![state.pending_tasks[0]];
    if (generation?.kind !== "book_structure") throw Error("missing structure task");
    return generation.task;
  };
  let requestRegistered = false, reconcile: BookStructureGenerationTaskV1 | undefined;
  for (let turn = 0; turn < 40; turn++) {
    const task = current();
    if (task.descriptor.kind === "structure_theme_reconcile") { reconcile = task; break; }
    let action = structureProductionResponse(task);
    if ("phase" in task.input && task.input.phase === "plan") {
      const plan = action as { themes: Array<Record<string, unknown>> };
      action = { themes: [plan.themes[0], { ...plan.themes[0], ref: "theme:other", question: "What constrains transfer?" }] };
    }
    if ("phase" in task.input && task.input.phase === "theme" && !requestRegistered) {
      const file = freezeBookStructureGenerationTask(f.target, task), bytes = readFileSync(file, "utf8");
      requestStructureThemeRevision(f.target.workspace_dir, { id: "source-time", theme_ref: task.input.ref, issue: "Correct the kernel timing explanation." });
      expect(current().descriptor).toEqual(task.descriptor);
      expect(renderBookStructureGenerationTaskInput(current())).toBe(renderBookStructureGenerationTaskInput(task));
      expect(readFileSync(file, "utf8")).toBe(bytes);
      requestRegistered = true;
    }
    f.submit(task, action); historical.push(task);
  }
  expect(reconcile).toBeDefined();
  if (!reconcile || !("phase" in reconcile.input) || reconcile.input.phase !== "reconcile") throw Error("missing original reconciliation");
  const initial = structuredClone(reconcile.input.directory);
  f.submit(reconcile); historical.push(reconcile);
  let task = current();
  if (!("phase" in task.input) || task.input.phase !== "theme") throw Error("missing revision");
  const revision = task.input;
  expect(task.descriptor.kind).toBe("structure_theme");
  expect(task.descriptor.work_unit_id).toContain("structure:theme:revision:source-time:0-");
  expect(revision.revision_request).toEqual(readStructureThemeRevisionRequests(f.target.workspace_dir)[0]);
  expect(revision.directory.works.find(w => w.plan.ref === "theme:other"))
    .toEqual(initial.works.find(w => w.plan.ref === "theme:other"));
  const priorWork = initial.works.find(w => w.plan.ref === revision.ref)!;
  const reopened = revision.directory.works.find(w => w.plan.ref === revision.ref)!;
  expect(reopened.seen_refs).toEqual(priorWork.seen_refs);
  expect(reopened.inspected_refs).toEqual(priorWork.inspected_refs);
  expect(reopened.evidence_by_unit).toEqual(priorWork.evidence_by_unit);
  expect(current().descriptor).toEqual(task.descriptor);
  const result: StructureThemeResult = structuredClone(revision.directory.revision!.previous_result);
  result.stages[0].development.text = "Kernel timing measures the whole chosen path, including the additional merge work.";
  expect(() => f.submit(task, { kind: "resolve", result: { ...result, name: "Changed identity" } })).toThrow(/preserve theme name/);
  f.submit(task, { kind: "resolve", result }); historical.push(task);
  const finished = f.get();
  expect(finished.pending_tasks).toEqual([]);
  expect(finished.book_structure_materialized).toBeDefined();
  for (const old of historical) {
    const generation = finished.generation_tasks![old.descriptor.work_unit_id];
    if (generation.kind !== "book_structure") throw Error("original task disappeared");
    expect(generation.task.descriptor).toEqual(old.descriptor);
    expect(renderBookStructureGenerationTaskInput(generation.task)).toBe(renderBookStructureGenerationTaskInput(old));
  }
  requestStructureThemeRevision(f.target.workspace_dir, { id: "source-condition", theme_ref: revision.ref, issue: "Clarify the timing conditions." });
  task = current();
  if (!("phase" in task.input) || task.input.phase !== "theme") throw Error("missing second revision");
  expect(task.descriptor.work_unit_id).toContain("structure:theme:revision:source-condition:0-");
  expect(task.input.directory.revision!.previous_result).toEqual(result);
  f.submit(task, { kind: "resolve", result });
  expect(f.get().pending_tasks).toEqual([]);
  expect(() => requestStructureThemeRevision(f.target.workspace_dir, { id: "source-condition", theme_ref: revision.ref, issue: "Duplicate" })).toThrow(/already requested/);
  requestStructureThemeRevision(f.target.workspace_dir, { id: "missing-theme", theme_ref: "theme:merged-away", issue: "Repair vanished theme." });
  expect(f.get().structure_blocked).toContain("unknown accepted theme to revise");
}, 60000);
