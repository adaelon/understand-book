import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { readAutomaticBuildTaskStage, resolveAutomaticBuildTarget } from "../src/build-orchestrator";
import { freezeAutomaticBuildStagePolicySet } from "../src/automatic-build-policy-generation";
import { freezeProfileSidecarDiscourseShadowTask, freezeProfileSidecarSemanticFastPathTask } from "../src/profile-sidecar-reduction";
import { ExtractorContractError } from "../src/extractor-contract";
import { runAutomaticBuildStageWriter } from "../../../skills/build/automatic-build";

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

function fixture(fragment: boolean) {
  const root = mkdtempSync(path.join(tmpdir(), "understand-book-profile-writer-"));
  roots.push(root);
  const sourceFile = path.join(root, "source.md");
  writeFileSync(sourceFile, `# Source\n\n${"A passage explains why input determines output. ".repeat(fragment ? 800 : 10)}\n`);
  const target = resolveAutomaticBuildTarget(sourceFile, root);
  const stage = readAutomaticBuildTaskStage(target, { stage: "profile_sidecar", work_unit_id: "" }, "full")!;
  freezeAutomaticBuildStagePolicySet(target, stage.policy_set!);
  const generation = Object.values(stage.generation_tasks ?? {}).find(g => fragment
    ? g.kind === "profile_sidecar_discourse" && g.task.route.role === "fragment"
    : g.kind === "profile_sidecar_fast_path");
  if (!generation || (generation.kind !== "profile_sidecar_discourse" && generation.kind !== "profile_sidecar_fast_path")) {
    throw new Error("expected a profile-sidecar generation task");
  }
  const task = generation.task;
  let candidate: unknown;
  if (task.version === "profile_sidecar_semantic_fast_path_task.v1") {
    freezeProfileSidecarSemanticFastPathTask(target, task);
    candidate = { discourse_items: task.packet.discourse_lids.map(lid => ({ lid, mode: "informative", relations: [] })) };
  } else {
    freezeProfileSidecarDiscourseShadowTask(target, task);
    if (task.descriptor.input_basis.kind !== "source_slices") throw new Error("expected source slices");
    const slice = task.descriptor.input_basis.slices[0];
    candidate = { version: "profile_sidecar_discourse_observation.v1", parent_lid: task.route.parent_lid,
      source_slice_ordinal: slice.ordinal,
      mode_candidates: [{ value: "informative", confidence: 0.9 }], local_function_candidates: [],
      rhetorical_move_candidates: [], summary_fragments: ["A bounded explanation."], relation_candidates: [] };
  }
  const candidatePath = path.join(root, "candidate.json");
  writeFileSync(candidatePath, JSON.stringify(candidate));
  const write = (attempt = 1) => runAutomaticBuildStageWriter(target, "profile_sidecar", task.descriptor.work_unit_id,
    candidatePath, { policy_generation_id: task.policy_generation_id, attempt,
      executor: "profile-writer-test", generated_at: "2026-10-23T06:00:00.000Z" });
  return { root, sourceFile, target, task, candidatePath, write };
}

describe("profile-sidecar generation writer", () => {
  it.each([false, true])("writes an artifact without starting another engine, fragment=%s", fragment => {
    const book = fixture(fragment);
    const previous = process.env.UNDERSTAND_BOOK_SIDECAR_SELF;
    process.env.UNDERSTAND_BOOK_SIDECAR_SELF = path.join(book.root, "unavailable-child-engine.exe");
    try {
      const result = book.write();
      const artifact = JSON.parse(readFileSync(result.artifact_path, "utf8"));
      expect(artifact).toMatchObject({
        version: "semantic_task_artifact.v3", stage: "profile_sidecar",
        work_unit_id: book.task.descriptor.work_unit_id, policy_generation_id: book.task.policy_generation_id,
      });
      if (fragment) {
        expect(JSON.parse(readFileSync(book.candidatePath, "utf8"))).not.toHaveProperty("core_sha256");
        const basis = book.task.descriptor.input_basis;
        if (basis.kind !== "source_slices") throw new Error("expected source slices");
        expect(artifact.payload.core_sha256).toBe(basis.slices[0].core_sha256);
      }
      expect(result.output_counts).toBeDefined();
      expect(existsSync(path.join(book.target.workspace_dir, ".build/profile-sidecar"))).toBe(false);
    } finally {
      if (previous === undefined) delete process.env.UNDERSTAND_BOOK_SIDECAR_SELF;
      else process.env.UNDERSTAND_BOOK_SIDECAR_SELF = previous;
    }
  });

  it("keeps candidate errors typed and allows a corrected later attempt", () => {
    const book = fixture(false);
    const valid = readFileSync(book.candidatePath, "utf8");
    writeFileSync(book.candidatePath, JSON.stringify({ discourse_items: "invalid" }));
    expect(book.write).toThrow(ExtractorContractError);
    writeFileSync(book.candidatePath, valid);
    expect(() => book.write(2)).not.toThrow();
  });

  it.each([false, true])("still rejects a changed source before publishing, fragment=%s", fragment => {
    const book = fixture(fragment);
    writeFileSync(book.sourceFile, "# Source\n\nA changed revision.\n");
    expect(book.write).toThrow("source fingerprint is stale");
  });
});
