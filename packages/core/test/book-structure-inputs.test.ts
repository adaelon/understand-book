import { existsSync, readFileSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { prepareBookStructureInputs, readAutomaticBuildTaskStage } from "../src/build-orchestrator";
import { readAutomaticBuildAttemptSnapshot } from "../src/automatic-build-task-store";
import { DSH_BUILD_EXECUTION_PROFILE_V1 } from "../src/build-execution-profile";
import { automaticBuildGenerationArtifactPath } from "../src/semantic-artifact";
import { freezeBookStructureGenerationTask, renderBookStructureGenerationTaskInput } from "../src/book-structure-generation";
import { renderAutomaticBuildTaskInput } from "../../../skills/build/automatic-build";
import { structureProductionFixture, structureProductionResponse } from "./helpers/book-structure-production";

const cachePath = (workspace: string) => path.join(workspace, ".build/automatic-build/v4/book-structure-inputs.json");
const tasks = (stage: ReturnType<ReturnType<typeof structureProductionFixture>["get"]>) =>
  Object.values(stage.generation_tasks!).flatMap(g => g.kind === "book_structure" ? [g.task] : []);

function withoutPreparation<T>(workspace: string, read: () => T): T {
  const directory = cachePath(workspace), parked = `${directory}.test-disabled`;
  renameSync(directory, parked);
  try { return read(); } finally { renameSync(parked, directory); }
}

function acceptOutline(f: ReturnType<typeof structureProductionFixture>) {
  const outline = tasks(f.get()).find(t => t.descriptor.kind === "structure_outline")!;
  f.submit(outline);
  return outline;
}

describe("BookStructure input preparation", () => {
  it("waits for the outline and preserves identical delivery, live receipts and chapter readiness", () => {
    const f = structureProductionFixture(2);
    expect(prepareBookStructureInputs(f.target, "full")).toEqual({ status: "skipped", reason: "outline_pending" });
    expect(existsSync(cachePath(f.target.workspace_dir))).toBe(false);
    acceptOutline(f);
    const before = f.get(), attempts = readAutomaticBuildAttemptSnapshot(f.target);
    expect(prepareBookStructureInputs(f.target, "full")).toMatchObject({
      status: "prepared", reused: false, ordinary_tasks: 0, fragment_tasks: 2,
    });
    expect(readAutomaticBuildAttemptSnapshot(f.target)).toEqual(attempts);
    expect(f.get()).toEqual(before);
    expect(prepareBookStructureInputs(f.target, "full")).toMatchObject({ reused: true });
    const fragments = tasks(before).filter(t => t.descriptor.kind === "structure_fragment");
    for (const task of tasks(before)) {
      freezeBookStructureGenerationTask(f.target, task);
      expect(renderAutomaticBuildTaskInput(f.target, "book_structure", task.descriptor.work_unit_id,
        { policy_generation_id: task.policy_generation_id }).stdout).toBe(renderBookStructureGenerationTaskInput(task));
    }
    const focused = readAutomaticBuildTaskStage(f.target, { stage: "book_structure",
      work_unit_id: fragments[0].descriptor.work_unit_id, parent_lid: fragments[0].parent_unit_lid }, "full")!;
    expect(tasks(focused).find(t => t.descriptor.kind === "structure_fragment")).toEqual(fragments[0]);
    f.submit(fragments[0]);
    const partial = f.get();
    expect(partial.pending_tasks).not.toContain(fragments[0].descriptor.work_unit_id);
    expect(partial.pending_tasks).toContain(fragments[1].descriptor.work_unit_id);
    expect(tasks(partial).some(t => t.descriptor.kind === "structure_chapter")).toBe(false);
    f.submit(fragments[1]);
    const ready = f.get();
    expect(tasks(ready).some(t => t.descriptor.kind === "structure_chapter")).toBe(true);
    expect(ready).toEqual(withoutPreparation(f.target.workspace_dir, () => f.get()));
  }, 120_000);

  it("invalidates changed upstream projections and rejects the old frozen candidate", () => {
    const f = structureProductionFixture(2);
    acceptOutline(f);
    const original = tasks(f.get()).find(t => t.descriptor.kind === "structure_fragment")!;
    prepareBookStructureInputs(f.target, "full");
    const file = path.join(f.target.workspace_dir, "discourse_index.json");
    const discourse = JSON.parse(readFileSync(file, "utf8"));
    discourse.items.find((item: { lid: string }) => original.allowed_evidence_lids.includes(item.lid)).mode = "argumentative";
    writeFileSync(file, JSON.stringify(discourse));
    const current = f.get();
    expect(current).toEqual(withoutPreparation(f.target.workspace_dir, () => f.get()));
    expect(tasks(current).find(t => t.descriptor.work_unit_id === original.descriptor.work_unit_id)?.descriptor.input_hash)
      .not.toBe(original.descriptor.input_hash);
    expect(() => f.submit(original)).toThrow(/current input changed/);
    expect(prepareBookStructureInputs(f.target, "full")).toMatchObject({ reused: false });
    expect(prepareBookStructureInputs(f.target, "full")).toMatchObject({ reused: true });
  }, 120_000);

  it("replays a replaced outline and invalidates changed quality, source and transport", () => {
    const f = structureProductionFixture(2);
    const outline = acceptOutline(f);
    prepareBookStructureInputs(f.target, "full");
    const fragment = tasks(f.get()).find(t => t.descriptor.kind === "structure_fragment")!;
    unlinkSync(automaticBuildGenerationArtifactPath(f.target, "book_structure", outline.policy_generation_id, outline.descriptor.work_unit_id));
    const replacement = structuredClone(structureProductionResponse(outline)) as { chapters: Array<{ question: { text: string } }> };
    replacement.chapters[0].question.text = "Under which conditions does ownership permit transfer?";
    f.submit(outline, replacement);
    expect(f.get()).toEqual(withoutPreparation(f.target.workspace_dir, () => f.get()));
    expect(() => f.submit(fragment)).toThrow(/current input changed/);
    expect(prepareBookStructureInputs(f.target, "full")).toMatchObject({ reused: false });
    for (const options of [{ quality_profile: "balanced" as const }, { execution_profile: DSH_BUILD_EXECUTION_PROFILE_V1 }]) {
      const read = () => readAutomaticBuildTaskStage(f.target, { stage: "book_structure", work_unit_id: "" },
        options.quality_profile ?? "full", options);
      expect(read()).toEqual(withoutPreparation(f.target.workspace_dir, read));
    }
    // Canonical source changes invalidate preparation even with the old caller target ref.
    writeFileSync(f.source, readFileSync(f.source, "utf8").replace("explicit conditions", "revised conditions"));
    const read = () => readAutomaticBuildTaskStage(f.target, { stage: "book_structure", work_unit_id: "" }, "full");
    expect(read()).toEqual(withoutPreparation(f.target.workspace_dir, read));
  }, 120_000);

  it("also reuses ordinary unit inputs for the paper profile", () => {
    const f = structureProductionFixture(2);
    const target = { ...f.target, profile_id: "paper" as const, target_ref: { ...f.target.target_ref, profile_id: "paper" as const } };
    const read = () => readAutomaticBuildTaskStage(target, { stage: "book_structure", work_unit_id: "" }, "full")!;
    const before = read();
    expect(tasks(before).every(t => t.descriptor.kind === "structure_unit")).toBe(true);
    expect(prepareBookStructureInputs(target, "full")).toMatchObject({ status: "prepared", ordinary_tasks: 2, fragment_tasks: 0 });
    expect(read()).toEqual(before);
    expect(prepareBookStructureInputs(target, "full")).toMatchObject({ reused: true });
  }, 120_000);
});
