import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { strToU8, zipSync } from "fflate";
import { afterEach, describe, expect, it } from "vitest";
import { loadBookSource } from "../src/book-source";
import { readAutomaticBuildTaskStage, resolveAutomaticBuildTarget } from "../src/build-orchestrator";
import { freezeAutomaticBuildStagePolicySet } from "../src/automatic-build-policy-generation";
import { freezeProfileSidecarSemanticFastPathTask } from "../src/profile-sidecar-reduction";
import { renderProfileSidecarModelInput } from "../src/model-input-renderer";
import { renderAutomaticBuildTaskInput, runAutomaticBuildStageWriter } from "../../../skills/build/automatic-build";
import { epubToSource } from "../src/epub-adapter";

const roots: string[] = [];
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });

function importedEpub(snapshotPath = "source.epub") {
  const root = mkdtempSync(path.join(tmpdir(), "book-source-resume-"));
  roots.push(root);
  const workspace = path.join(root, ".understand-book", "guide");
  mkdirSync(workspace, { recursive: true });
  const bytes = zipSync({
    "META-INF/container.xml": strToU8('<container><rootfiles><rootfile full-path="content.opf"/></rootfiles></container>'),
    "content.opf": strToU8('<package><manifest><item id="chapter" href="chapter.xhtml"/></manifest><spine><itemref idref="chapter"/></spine></package>'),
    "chapter.xhtml": strToU8('<html><body><h1>Guide</h1><p>Before <span><img src="figure.png"/>after.</span></p></body></html>'),
    "figure.png": strToU8("image bytes"),
  });
  // A real prior import: inline image extraction changed after this canonical
  // text and these citation anchors had already been published.
  const source = "Guide\n\nBefore after.\n\n";
  const lidNodes = [
    { lid: "1", path: [1], kind: "chapter", span: { start: 0, end: 20 }, children: ["1.1", "1.2"] },
    { lid: "1.1", path: [1, 1], kind: "paragraph", span: { start: 0, end: 5 }, children: [] },
    { lid: "1.2", path: [1, 2], kind: "paragraph", span: { start: 7, end: 20 }, children: [] },
  ];
  const epub = path.join(workspace, snapshotPath);
  writeFileSync(epub, bytes);
  writeFileSync(path.join(workspace, "source.txt"), source);
  writeFileSync(path.join(workspace, "base.json"), JSON.stringify({ book_id: "guide", lid_nodes: lidNodes, graph_nodes: [], graph_edges: [] }));
  writeFileSync(path.join(workspace, "source_manifest.json"), JSON.stringify({ book_id: "guide",
    canonical_source: { kind: "epub", path: "original.epub", snapshot_path: snapshotPath, truth_file: "source.txt" } }));
  writeFileSync(path.join(workspace, "profile_metadata.json"), JSON.stringify({ header: { book_id: "guide", profile_id: "technical_learning" } }));
  return { root, workspace, epub, source, lidNodes, bytes };
}

describe("imported EPUB source identity across parser upgrades", () => {
  it.each(["source.epub", "imported.epub"])("resumes %s using the published canonical text and citation anchors", (snapshotPath) => {
    const f = importedEpub(snapshotPath);
    expect(epubToSource(f.bytes).source).not.toBe(f.source);
    const before = readFileSync(path.join(f.workspace, "base.json"));
    const loaded = loadBookSource(f.epub);
    expect(loaded).toEqual({ source: f.source, blocks: [], lidNodes: f.lidNodes });
    expect(loadBookSource(path.join(f.workspace, "source.txt"))).toEqual(loaded);
    expect(readFileSync(path.join(f.workspace, "base.json"))).toEqual(before);
  });

  it("resolves the same build target and package fingerprint after a parser upgrade", () => {
    const f = importedEpub();
    const target = resolveAutomaticBuildTarget("guide", f.root);
    expect(target.source_path).toBe(f.epub);
    expect(target.target_ref.input_fingerprint).toBe(createHash("sha256").update(f.bytes).digest("hex"));
    expect(resolveAutomaticBuildTarget(f.epub, f.root)).toEqual(target);
    expect(resolveAutomaticBuildTarget(path.join(f.workspace, "source.txt"), f.root)).toEqual(target);
  });

  it.each(["input", "writer"])("keeps frozen profile %s on the published source after a parser upgrade", (phase) => {
    const f = importedEpub();
    const target = resolveAutomaticBuildTarget("guide", f.root);
    const stage = readAutomaticBuildTaskStage(target, { stage: "profile_sidecar", work_unit_id: "" }, "full")!;
    const generation = Object.values(stage.generation_tasks ?? {})[0];
    if (generation?.kind !== "profile_sidecar_fast_path") throw new Error("expected frozen discourse task");
    freezeAutomaticBuildStagePolicySet(target, stage.policy_set!);
    freezeProfileSidecarSemanticFastPathTask(target, generation.task);
    const task = generation.task;
    if (phase === "input") {
      expect(renderAutomaticBuildTaskInput(target, "profile_sidecar", task.descriptor.work_unit_id,
        { policy_generation_id: task.policy_generation_id }).stdout).toBe(renderProfileSidecarModelInput(task.packet));
    } else {
      const candidatePath = path.join(f.root, "candidate.json");
      writeFileSync(candidatePath, JSON.stringify({
        discourse_items: task.packet.discourse_lids.map(lid => ({ lid, mode: "informative", relations: [] })),
      }));
      const result = runAutomaticBuildStageWriter(target, "profile_sidecar", task.descriptor.work_unit_id,
        candidatePath, { policy_generation_id: task.policy_generation_id, attempt: 1,
          executor: "test", generated_at: "2026-10-29T00:00:00.000Z" });
      expect(JSON.parse(readFileSync(result.artifact_path, "utf8")).input_hash).toBe(task.descriptor.input_hash);
    }
    expect(readFileSync(path.join(f.workspace, "source.txt"), "utf8")).toBe(f.source);
  });

  it("uses the current parser for new EPUB inputs, including another package beside a workspace snapshot", () => {
    const f = importedEpub();
    for (const file of [path.join(f.root, "new.epub"), path.join(f.workspace, "other.epub")]) {
      writeFileSync(file, f.bytes);
      expect(loadBookSource(file).source).toBe(epubToSource(f.bytes).source);
      expect(loadBookSource(file).blocks.some(block => block.assetKind === "image")).toBe(true);
    }
  });

  it("rejects an inconsistent canonical base instead of reparsing and silently replacing its anchors", () => {
    const f = importedEpub();
    const file = path.join(f.workspace, "base.json");
    const base = JSON.parse(readFileSync(file, "utf8"));
    writeFileSync(file, JSON.stringify({ ...base, book_id: "another-book" }));
    expect(() => loadBookSource(f.epub)).toThrow("EPUB snapshot base identity mismatch");
  });
});
