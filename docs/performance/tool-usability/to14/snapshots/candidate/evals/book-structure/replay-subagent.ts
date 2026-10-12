import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { newStructureChapterWork } from "../../packages/core/src/book-structure-planning";
import { openSubagentStep, submitSubagentStep, subagentReport, type SubagentExperiment } from "./chapter-subagent";

const dir = path.resolve(process.argv[2]);
const saved: SubagentExperiment = JSON.parse(readFileSync(path.join(dir, "session.json"), "utf8"));
const replay: SubagentExperiment = { version: saved.version, executor: saved.executor,
  authorization: saved.authorization, input: saved.input, prompts: saved.prompts,
  chapter: newStructureChapterWork(saved.chapter.unit_lid), steps: [] };
for (const recorded of saved.steps) {
  const step = openSubagentStep(replay, recorded.opened_at)!;
  assert.equal(step.input, recorded.input, `input ${recorded.ordinal}`);
  assert.equal(step.prompt, recorded.prompt, `prompt ${recorded.ordinal}`);
  assert.notEqual(recorded.status, "pending", "cannot verify an unfinished response");
  submitSubagentStep(replay, recorded.ordinal, recorded.response, recorded.finished_at);
  assert.deepEqual(step, recorded, `decision ${recorded.ordinal}`);
}
assert.deepEqual(replay, saved, "replayed session");
const report = subagentReport(replay);
assert.deepEqual(report, JSON.parse(readFileSync(path.join(dir, "report.json"), "utf8")), "materialized report");
const verification = { replayed_decisions: replay.steps.length, inputs_and_prompts_equal: true,
  decisions_and_final_state_equal: true, materialized_report_equal: true, complete: report.complete };
writeFileSync(path.join(dir, "verification.json"), JSON.stringify(verification, null, 2) + "\n");
console.log(JSON.stringify(verification, null, 2));
