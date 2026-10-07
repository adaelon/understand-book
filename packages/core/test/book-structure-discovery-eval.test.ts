import { it, expect } from "vitest";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { discoveryExperiment } from "../../../evals/book-structure/discovery";
import { structureProductionFixture, structureProductionResponse } from "./helpers/book-structure-production";
import type { BookStructureGenerationTaskV1 } from "../src/book-structure-generation";

it("records malformed submissions within the quota, resumes pending delivery and replays exact grounded discoveries", () => {
  const f = structureProductionFixture(1), dir = path.join(f.root, "discovery-eval");
  mkdirSync(dir);
  const save = (name: string, value: unknown) => writeFileSync(path.join(dir, name), JSON.stringify(value));
  writeFileSync(path.join(f.target.workspace_dir, "source.txt"), readFileSync(f.source, "utf8"));
  f.write("source_manifest.json", { book_id: f.target.book_id, canonical_source: { kind: "markdown", truth_file: "source.txt" } });
  f.write("profile_metadata.json", { header: { book_id: f.target.book_id, profile_id: "technical_learning" } });
  save("plan.json", { version: "book_structure_discovery_experiment.v1", status: "confirmed", source_workspace: f.target.workspace_dir,
    workspace: f.target.workspace_dir, unit_lid: "1", execution: "codex_subagent", max_submissions: 80, max_minutes: 60, prepared_at: new Date().toISOString() });
  save("session.json", { steps: [] });
  const first = discoveryExperiment("next", dir) as { ordinal: number; input_file: string; task_file: string };
  expect(discoveryExperiment("next", dir)).toEqual(first);
  const response = path.join(dir, "generator.json");
  writeFileSync(response, "{ invalid JSON");
  expect(discoveryExperiment("submit", dir, response)).toMatchObject({ ordinal: 1, status: "rejected" });
  expect(JSON.parse(readFileSync(path.join(dir, "session.json"), "utf8")).steps[0].output_estimated_tokens).toBeGreaterThan(0);
  const delayed = discoveryExperiment("next", dir) as { task_file: string };
  const delayedTask = JSON.parse(readFileSync(delayed.task_file, "utf8")) as BookStructureGenerationTaskV1;
  writeFileSync(response, JSON.stringify(structureProductionResponse(delayedTask)));
  const beforeExpiry = JSON.parse(readFileSync(path.join(dir, "session.json"), "utf8"));
  save("session.json", { ...beforeExpiry, started_at: "2000-01-01T00:00:00.000Z" });
  expect(() => discoveryExperiment("next", dir)).toThrow(/time quota/);
  expect(discoveryExperiment("submit", dir, response)).toMatchObject({ ordinal: 2, status: "rejected", error: expect.stringContaining("time quota") });
  const afterExpiry = JSON.parse(readFileSync(path.join(dir, "session.json"), "utf8"));
  save("session.json", { ...afterExpiry, started_at: beforeExpiry.started_at });
  for (let i = 0; i < 4; i++) {
    const next = discoveryExperiment("next", dir) as { status?: string; task_file: string };
    if (next.status === "complete") break;
    const task = JSON.parse(readFileSync(next.task_file, "utf8")) as BookStructureGenerationTaskV1;
    writeFileSync(response, JSON.stringify(structureProductionResponse(task)));
    expect(discoveryExperiment("submit", dir, response)).toMatchObject({ status: "accepted" });
  }
  expect(discoveryExperiment("next", dir)).toMatchObject({ status: "complete" });
  expect(discoveryExperiment("report", dir)).toMatchObject({ candidates: 2, submissions: 4 });
  expect(JSON.parse(readFileSync(path.join(dir, "report.json"), "utf8"))).toMatchObject({ rejected: 2, accepted: 2 });
  expect(discoveryExperiment("replay", dir)).toMatchObject({ status: "passed", same_inputs: true, same_payloads: true, model_calls: 0 });
}, 120000);
