import { mkdtempSync, readFileSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  prepareProfileDiscourseInputs, readAutomaticBuildTaskStage, resolveAutomaticBuildTarget,
} from "../src/build-orchestrator";
import { listActiveAutomaticBuildTaskLeases } from "../src/automatic-build-lease";
import { readAutomaticBuildAttemptSnapshot } from "../src/automatic-build-task-store";
import { freezeAutomaticBuildStagePolicySet } from "../src/automatic-build-policy-generation";
import {
  freezeProfileSidecarDiscourseShadowTask, freezeProfileSidecarSemanticFastPathTask,
  writeProfileSidecarDiscourseShadowCandidate,
} from "../src/profile-sidecar-reduction";
import type { ExtractionQualityProfile } from "../src/semantic-artifact";
import { renderAutomaticBuildTaskInput } from "../../../skills/build/automatic-build";
import { renderProfileSidecarModelInput } from "../src/model-input-renderer";

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

function fixture(fragment = false) {
  const root = mkdtempSync(path.join(tmpdir(), "understand-book-discourse-inputs-"));
  roots.push(root);
  const sourceFile = path.join(root, "source.md");
  writeFileSync(sourceFile, `# Source\n\n${"A source passage explains why input determines output. ".repeat(fragment ? 700 : 10)}\n`);
  const target = resolveAutomaticBuildTarget(sourceFile, root);
  const cacheFile = path.join(target.workspace_dir, ".build/automatic-build/v3/profile-discourse-inputs.json");
  const stage = (quality: ExtractionQualityProfile = "full") =>
    readAutomaticBuildTaskStage(target, { stage: "profile_sidecar", work_unit_id: "" }, quality)!;
  return { root, sourceFile, target, cacheFile, stage };
}

describe("prepared profile discourse inputs", () => {
  it("reuses identical inputs without claiming a task or spending an attempt", () => {
    const book = fixture();
    const before = book.stage();
    const attempts = readAutomaticBuildAttemptSnapshot(book.target);
    expect(prepareProfileDiscourseInputs(book.target, "full")).toEqual({
      status: "prepared", reused: false, ordinary_tasks: 1, fragment_tasks: 0,
    });
    expect(book.stage()).toEqual(before);
    expect(prepareProfileDiscourseInputs(book.target, "full")).toMatchObject({ reused: true });
    expect(readAutomaticBuildAttemptSnapshot(book.target)).toEqual(attempts);
    expect(listActiveAutomaticBuildTaskLeases(book.target, "profile_sidecar")).toEqual([]);
  });

  it("recomputes after the source or quality policy changes", () => {
    const book = fixture();
    prepareProfileDiscourseInputs(book.target, "full");
    const before = book.stage();
    writeFileSync(book.sourceFile, "# Source\n\nThe revised passage has different evidence.\n");
    const current = resolveAutomaticBuildTarget(book.sourceFile, book.root, { book_id: book.target.book_id });
    const currentStage = () => readAutomaticBuildTaskStage(current,
      { stage: "profile_sidecar", work_unit_id: "" }, "full")!;
    const changed = currentStage();
    expect(changed.generation_tasks).not.toEqual(before.generation_tasks);
    unlinkSync(book.cacheFile);
    expect(currentStage()).toEqual(changed);
    expect(prepareProfileDiscourseInputs(current, "full")).toMatchObject({ reused: false });
    expect(currentStage()).toEqual(changed);
    expect(prepareProfileDiscourseInputs(current, "balanced")).toMatchObject({ reused: false });
    const balanced = readAutomaticBuildTaskStage(current,
      { stage: "profile_sidecar", work_unit_id: "" }, "balanced");
    unlinkSync(book.cacheFile);
    expect(readAutomaticBuildTaskStage(current,
      { stage: "profile_sidecar", work_unit_id: "" }, "balanced")).toEqual(balanced);
    expect(balanced?.generation_tasks).not.toEqual(changed.generation_tasks);
  });

  it("delivers a frozen ordinary input directly and still rejects a changed source", () => {
    const book = fixture();
    const stage = book.stage();
    const generation = Object.values(stage.generation_tasks ?? {})[0];
    if (generation.kind !== "profile_sidecar_fast_path") throw new Error("expected ordinary discourse");
    freezeAutomaticBuildStagePolicySet(book.target, stage.policy_set!);
    freezeProfileSidecarSemanticFastPathTask(book.target, generation.task);
    const render = () => renderAutomaticBuildTaskInput(book.target, "profile_sidecar",
      generation.task.descriptor.work_unit_id, { policy_generation_id: generation.task.policy_generation_id });
    expect(render().stdout).toBe(renderProfileSidecarModelInput(generation.task.packet));
    writeFileSync(book.sourceFile, "# Source\n\nThis is a new source revision.\n");
    expect(render).toThrow("source fingerprint is stale");
  });

  it("routes reducers from newly committed fragments while reusing source preparation", () => {
    const book = fixture(true);
    const before = book.stage();
    expect(prepareProfileDiscourseInputs(book.target, "full")).toMatchObject({
      status: "prepared", ordinary_tasks: 0,
    });
    expect(book.stage()).toEqual(before);
    freezeAutomaticBuildStagePolicySet(book.target, before.policy_set!);
    const tasks = Object.values(before.generation_tasks ?? {});
    expect(tasks.length).toBeGreaterThan(1);
    for (const [index, generation] of tasks.entries()) {
      if (generation.kind !== "profile_sidecar_discourse" || generation.task.route.role !== "fragment"
        || generation.task.descriptor.input_basis.kind !== "source_slices") throw new Error("expected source fragment");
      const task = generation.task;
      const slice = generation.task.descriptor.input_basis.slices[0];
      freezeProfileSidecarDiscourseShadowTask(book.target, task);
      writeProfileSidecarDiscourseShadowCandidate({
        target: book.target, source: readFileSync(book.sourceFile, "utf8"), task,
        provenance: { executor: "test", attempt: 1, generated_at: "2026-10-20T03:00:00.000Z" },
        candidate: {
          version: "profile_sidecar_discourse_observation.v1", parent_lid: task.route.parent_lid,
          source_slice_ordinal: slice.ordinal, core_sha256: slice.core_sha256,
          mode_candidates: [{ value: "informative", confidence: 0.9 }],
          local_function_candidates: [], rhetorical_move_candidates: [],
          summary_fragments: [`Evidence from fragment ${index + 1}.`], relation_candidates: [],
        },
      });
      const live = book.stage();
      expect(live.pending_tasks).not.toContain(task.descriptor.work_unit_id);
      if (index < tasks.length - 1) {
        expect(live.pending_work_units?.every(unit => unit.kind === "profile_sidecar_discourse_fragment")).toBe(true);
      } else {
        expect(live.pending_work_units?.some(unit => unit.kind === "profile_sidecar_discourse_reduce")).toBe(true);
      }
    }
    expect(prepareProfileDiscourseInputs(book.target, "full")).toMatchObject({ reused: true });
    const reduced = book.stage();
    unlinkSync(book.cacheFile);
    expect(book.stage()).toEqual(reduced);
  });
});
