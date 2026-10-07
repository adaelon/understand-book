import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { buildAutomaticBuildSnapshot, inspectAutomaticBuildStageFreshness } from "../src/build-orchestrator";
import { collectAutomaticBuildStageQuality } from "../src/automatic-build-quality";
import { closeAutomaticBuildStage } from "../src/automatic-build-close";
import { runAutomaticBuildCloseStage } from "../../../skills/build/automatic-build";
import { structureProductionFixture } from "./helpers/book-structure-production";

function publicPrerequisiteFixture() {
  const fixture = structureProductionFixture(2);
  writeFileSync(path.join(fixture.target.workspace_dir, "source.txt"), readFileSync(fixture.source, "utf8"));
  fixture.write("source_manifest.json", { book_id: fixture.target.book_id,
    canonical_source: { kind: "markdown", truth_file: "source.txt" } });
  // The supported reuse boundary consists of public source/profile/base artifacts.
  // Remove all extraction history before creating any current structure work.
  rmSync(path.join(fixture.target.workspace_dir, ".build"), { recursive: true, force: true });
  expect(existsSync(path.join(fixture.target.workspace_dir, ".build"))).toBe(false);
  const scoped = () => buildAutomaticBuildSnapshot(fixture.target, { stage: "book_structure", quality_profile: "full" });
  const get = () => {
    const state = scoped().stages.find(candidate => candidate.stage === "book_structure");
    if (!state) throw Error("expected stage-scoped BookStructure state with public prerequisites");
    return state;
  };
  return { ...fixture, scoped, get };
}

describe("BookStructure stage close with reusable public prerequisites", () => {
  it("closes full current structure without upstream extraction history and leaves whole-build routing upstream", () => {
    const f = publicPrerequisiteFixture();
    let state = f.get();
    for (let round = 0; state.pending_tasks.length && round < 30; round++) {
      for (const id of state.pending_tasks) {
        const generation = state.generation_tasks![id];
        if (generation.kind !== "book_structure") throw Error("expected structure generation");
        f.submit(generation.task);
      }
      state = f.get();
    }
    expect(state.pending_tasks).toEqual([]);
    expect(state.closed).toBe(false);
    expect(collectAutomaticBuildStageQuality(f.target, state, "full")).toMatchObject({
      gate_status: "passed", book_structure_publication: "ready",
      book_structure_leaf_coverage: { chapters: 2, expected: 6, covered: 6, gaps: 0, overlaps: 0 },
    });
    const cliEntry = fileURLToPath(new URL("../../../skills/build/automatic-build.ts", import.meta.url));
    const cliQuality = spawnSync(process.execPath, ["--import", "tsx", cliEntry, "quality", f.source,
      "book_structure", "--root", f.root], { encoding: "utf8", cwd: fileURLToPath(new URL("../../../", import.meta.url)) });
    expect(cliQuality.status, cliQuality.stderr).toBe(0);
    expect(JSON.parse(cliQuality.stdout)).toMatchObject({ gate_status: "passed", book_structure_publication: "ready" });
    expect(runAutomaticBuildCloseStage(f.source, f.root, "book_structure", { quality_profile: "full" }))
      .toMatchObject({ status: "closed", stage: "book_structure", postcondition: { stage_closed: true }, next: "replan" });
    const scoped = f.scoped();
    expect(scoped.stages.map(candidate => candidate.stage)).toEqual(["book_structure"]);
    expect(scoped.stages[0]).toMatchObject({ closed: true, pending_tasks: [] });
    expect(inspectAutomaticBuildStageFreshness(scoped)).toEqual([
      expect.objectContaining({ stage: "book_structure", fresh: true, freshness_digest: expect.any(String) }),
    ]);
    expect(JSON.parse(readFileSync(path.join(f.target.workspace_dir, "book_structure.json"), "utf8")))
      .toMatchObject({ spine: expect.any(Array), key_stops: expect.any(Array) });
    const cli = spawnSync(process.execPath, ["--import", "tsx",
      cliEntry, "close", f.source,
      "book_structure", "--root", f.root], { encoding: "utf8", cwd: fileURLToPath(new URL("../../../", import.meta.url)) });
    expect(cli.status, cli.stderr).toBe(0);
    expect(JSON.parse(cli.stdout)).toMatchObject({ status: "closed", stage: "book_structure" });
    const ordinary = buildAutomaticBuildSnapshot(f.target);
    expect(ordinary.stages.map(candidate => candidate.stage)).toEqual(["pass1"]);
    expect(ordinary.stages[0].pending_tasks.length).toBeGreaterThan(0);
    expect(ordinary.stages[0].closed).toBe(false);
  }, 120000);

  it("rejects incomplete structure before running publication", () => {
    const f = publicPrerequisiteFixture();
    expect(f.get().pending_tasks.length).toBeGreaterThan(0);
    let published = false;
    expect(() => closeAutomaticBuildStage({ target: f.target, stage: "book_structure", quality_profile: "full",
      run_batch: () => { published = true; return { stdout: "" }; },
    })).toThrow(/quality_gate_failed:book_structure/);
    expect(published).toBe(false);
    expect(existsSync(path.join(f.target.workspace_dir, "book_structure.json"))).toBe(false);
  }, 120000);
});
