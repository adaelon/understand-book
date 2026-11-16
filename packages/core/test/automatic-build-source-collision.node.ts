import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, it } from "node:test";
import assert from "node:assert/strict";
import { deriveBookId } from "../src/book-id";
import { resolveAutomaticBuildTarget } from "../src/build-orchestrator";
import { prepareExplicitLegacyBuildPlan } from "../../../skills/build/automatic-build";

const roots: string[] = [];
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });

function fixture() {
  const root = mkdtempSync(path.join(tmpdir(), "book-source-collision-"));
  roots.push(root);
  const oldSource = path.join(root, "《矛盾论、实践论》 (毛泽东) (z-library.sk, 1lib.sk, z-lib.sk).epub");
  const source = path.join(root, "美的历程 (李泽厚) (z-library.sk, 1lib.sk, z-lib.sk).epub");
  const bookId = deriveBookId(oldSource);
  const workspace = path.join(root, ".understand-book", bookId);
  mkdirSync(workspace, { recursive: true });
  // Source selection must finish before EPUB parsing. Package parsing and
  // canonical anchor preservation are covered by book-source.test.ts.
  writeFileSync(oldSource, "old package bytes");
  writeFileSync(source, "new package bytes");
  writeFileSync(path.join(workspace, "source.txt"), "# Old book\n\nAn already imported book.\n");
  writeFileSync(path.join(workspace, "base.json"), JSON.stringify({ book_id: bookId, lid_nodes: [], graph_nodes: [], graph_edges: [] }));
  writeFileSync(path.join(workspace, "profile_metadata.json"), JSON.stringify({ header: { book_id: bookId, profile_id: "technical_learning" } }));
  writeFileSync(path.join(workspace, "source_manifest.json"), JSON.stringify({ book_id: bookId,
    canonical_source: { kind: "epub", path: oldSource, truth_file: "source.txt" } }));
  return { root, oldSource, source, bookId, workspace };
}

describe("explicit build source and workspace ownership", () => {
  for (const explicit of [false, true]) it(`rejects a colliding book before creating a plan (explicit ID: ${explicit})`, () => {
    const f = fixture();
    assert.equal(deriveBookId(f.source), f.bookId);
    const before = readFileSync(path.join(f.workspace, "source.txt"));
    const options = explicit ? { book_id: f.bookId } : {};
    assert.throws(() => prepareExplicitLegacyBuildPlan(f.source, f.root, options), /source_workspace_conflict.*--book-id/);
    assert.equal(existsSync(path.join(f.workspace, ".build")), false);
    assert.deepEqual(readFileSync(path.join(f.workspace, "source.txt")), before);
  });

  it("resolves the requested book under an independent explicit ID", () => {
    const f = fixture();
    const target = resolveAutomaticBuildTarget(f.source, f.root, { book_id: "mei-de-li-cheng" });
    assert.equal(target.book_id, "mei-de-li-cheng");
    assert.equal(target.source_path, f.source);
    assert.equal(target.workspace_dir, path.join(f.root, ".understand-book", "mei-de-li-cheng"));
    assert.equal(existsSync(path.join(f.workspace, ".build")), false);
  });

  it("keeps the original import and explicit workspace resumable without an EPUB snapshot", () => {
    const f = fixture();
    const target = resolveAutomaticBuildTarget(f.workspace, f.root);
    assert.deepEqual(resolveAutomaticBuildTarget(f.oldSource, f.root), target);
    assert.deepEqual(resolveAutomaticBuildTarget(f.oldSource, f.root, { book_id: f.bookId }), target);
    assert.deepEqual(resolveAutomaticBuildTarget(f.bookId, f.root), target);
    rmSync(f.oldSource);
    assert.deepEqual(resolveAutomaticBuildTarget(f.oldSource, f.root, { book_id: f.bookId }), target);
  });

  it("recognizes an identical relocated EPUB by its imported package without reparsing", () => {
    const f = fixture();
    copyFileSync(f.oldSource, path.join(f.workspace, "source.epub"));
    const moved = path.join(f.root, "moved");
    mkdirSync(moved);
    const relocated = path.join(moved, path.basename(f.oldSource));
    copyFileSync(f.oldSource, relocated);
    rmSync(f.oldSource);
    assert.deepEqual(resolveAutomaticBuildTarget(relocated, f.root), resolveAutomaticBuildTarget(f.workspace, f.root));
  });
});
