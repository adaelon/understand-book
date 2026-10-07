import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { prepareChapterExperiment, type ChapterExperimentInput } from "./chapter";
import { acceptStructureOutline, applyStructureChapterAction, newStructureChapterWork, structureChapterInput,
  type StructureChapterWork, type StructureOutline } from "../../packages/core/src/book-structure-planning";
import { materializeStructureChapterSelections } from "../../packages/core/src/book-structure-materialization";
import { renderStructurePlanningInput } from "../../packages/core/src/model-input-renderer";
import { estimateTokens } from "../../packages/core/src/window";

const root = fileURLToPath(new URL("../../", import.meta.url));
interface Step {
  ordinal: number; phase: "outline" | "chapter"; opened_at: string; finished_at?: string;
  prompt: string; input: string; input_tokens_estimate: number;
  response?: unknown; output_tokens_estimate?: number; status: "pending" | "accepted" | "rejected"; error?: string;
}
export interface SubagentExperiment {
  version: "book_structure_subagent_experiment.v1";
  authorization: string; executor: "codex_subagent"; started_at?: string; completed_at?: string;
  input: ChapterExperimentInput; prompts: { outline: string; chapter: string };
  outline?: StructureOutline; chapter: StructureChapterWork; steps: Step[];
}
const titles = (s: SubagentExperiment) => Object.fromEntries(s.input.outline_input.chapters.map(c => [c.unit_lid, c.title]));
const context = (s: SubagentExperiment) => ({ catalog: s.input.catalog, outline: s.outline!, titles: titles(s), excerpts: s.input.excerpts });
function delivery(s: SubagentExperiment) {
  const phase = s.outline ? "chapter" : "outline";
  const input = renderStructurePlanningInput(phase === "outline" ? s.input.outline_input : structureChapterInput(s.chapter, context(s)));
  const prompt = s.prompts[phase];
  const tokens = estimateTokens(prompt + input);
  if (tokens > 12000) throw new Error("Planning input exceeds 12000 estimated tokens; request a smaller evidence page or shorten the outline");
  return { phase: phase as Step["phase"], prompt, input, input_tokens_estimate: tokens };
}
export function openSubagentStep(s: SubagentExperiment, now = new Date().toISOString()): Step | undefined {
  if (s.chapter.result) return undefined;
  if (s.steps.at(-1)?.status === "pending") return s.steps.at(-1)!;
  if (s.steps.length >= 20) throw new Error("Local experiment reached 20 actions; inspect results before continuing");
  const step: Step = { ordinal: s.steps.length, opened_at: now, ...delivery(s), status: "pending" };
  s.started_at ??= now; s.steps.push(step); return step;
}
export function submitSubagentStep(s: SubagentExperiment, ordinal: number, response: unknown, now = new Date().toISOString()): Step {
  const step = s.steps.at(-1);
  if (!step || step.ordinal !== ordinal || step.status !== "pending") throw new Error("No matching pending step");
  step.response = response; step.finished_at = now; step.output_tokens_estimate = estimateTokens(JSON.stringify(response));
  try {
    if (step.output_tokens_estimate > (step.phase === "outline" ? 5000 : 3500)) throw new Error("Response exceeds the planning output reserve; shorten this response");
    const prospective = structuredClone(s);
    if (step.phase === "outline") prospective.outline = acceptStructureOutline(response, s.input.outline_input);
    else prospective.chapter = applyStructureChapterAction(s.chapter, response, context(s));
    // Catch oversize requests before committing them, so the next action can recover.
    if (!prospective.chapter.result) delivery(prospective);
    s.outline = prospective.outline; s.chapter = prospective.chapter;
    step.status = "accepted";
    if (s.chapter.result) s.completed_at = now;
  } catch (error) { step.status = "rejected"; step.error = String(error); }
  return step;
}
export function subagentReport(s: SubagentExperiment) {
  return { version: "book_structure_subagent_report.v1", executor: s.executor, authorization: s.authorization,
    complete: !!s.chapter.result, started_at: s.started_at, completed_at: s.completed_at,
    wall_ms: s.completed_at && s.started_at ? Date.parse(s.completed_at) - Date.parse(s.started_at) : null,
    model_decisions: s.steps.length, accepted: s.steps.filter(c => c.status === "accepted").length,
    rejected: s.steps.filter(c => c.status === "rejected").length,
    actual_model_calls: null, actual_input_tokens: null, actual_output_tokens: null,
    input_tokens_estimate_sum: s.steps.reduce((sum, step) => sum + step.input_tokens_estimate, 0),
    output_tokens_estimate_sum: s.steps.reduce((sum, step) => sum + (step.output_tokens_estimate ?? 0), 0),
    usage_note: "Decision counts and serialized-payload estimates exclude Codex tool turns, reasoning and retained context; actual usage unavailable from this session adapter.",
    embedding_calls: 0, external_generation_api_calls: 0,
    reads: s.steps.filter(step => step.phase === "chapter" && step.status === "accepted").map(step => step.response),
    candidate_count: s.input.catalog.candidates.length, outline: s.outline ?? null,
    old_published: s.input.old_published,
    new_structure: s.chapter.result ? materializeStructureChapterSelections(s.input.catalog, [s.chapter.result], ["10"], titles(s)) : null };
}

export function subagentCli(args: string[]) {
  const [command, output, ordinal, responseFile] = args;
  if (!output || !["init", "next", "submit", "report"].includes(command)) throw new Error("Usage: chapter-subagent.ts init|next|submit|report DIR [ORDINAL RESPONSE_FILE]");
  const dir = path.resolve(output), stateFile = path.join(dir, "session.json");
  const save = (name: string, value: unknown) => {
    mkdirSync(dir, { recursive: true }); const file = path.join(dir, name);
    writeFileSync(file + ".tmp", JSON.stringify(value, null, 2) + "\n"); renameSync(file + ".tmp", file);
  };
  if (command === "init") {
    if (existsSync(stateFile)) throw new Error("Session already exists; use next to resume");
    const session: SubagentExperiment = { version: "book_structure_subagent_experiment.v1", executor: "codex_subagent",
      authorization: "User: 那你就用subagent做对照实验呗 (2026-10-01)",
      input: prepareChapterExperiment(), prompts: { outline: readFileSync(path.join(root, "agents/book-structure-outline.md"), "utf8"),
        chapter: readFileSync(path.join(root, "agents/book-structure-chapter.md"), "utf8") },
      chapter: newStructureChapterWork("10"), steps: [] };
    delivery(session); save("session.json", session);
    return { status: "ready", candidates: session.input.catalog.candidates.length };
  }
  const s: SubagentExperiment = JSON.parse(readFileSync(stateFile, "utf8"));
  if (command === "next") {
    const step = openSubagentStep(s); save("session.json", s);
    if (!step) { save("report.json", subagentReport(s)); return { status: "done" }; }
    const inputFile = path.join(dir, `input-${step.ordinal}.json`), promptFile = path.join(dir, `prompt-${step.ordinal}.md`);
    writeFileSync(inputFile, step.input); writeFileSync(promptFile, step.prompt);
    return { status: "input", ordinal: step.ordinal, phase: step.phase, input_file: inputFile, prompt_file: promptFile,
      prior_error: s.steps.at(-2)?.error ?? null, remaining_actions: 20 - step.ordinal };
  }
  if (command === "submit") {
    if (!responseFile) throw new Error("submit requires ordinal and response JSON file");
    const step = submitSubagentStep(s, Number(ordinal), JSON.parse(readFileSync(responseFile, "utf8")));
    save("session.json", s); save("report.json", subagentReport(s));
    return { ordinal: step.ordinal, status: step.status, error: step.error ?? null, complete: !!s.chapter.result };
  }
  const report = subagentReport(s); save("report.json", report);
  return { complete: report.complete, model_decisions: report.model_decisions, rejected: report.rejected, wall_ms: report.wall_ms };
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) console.log(JSON.stringify(subagentCli(process.argv.slice(2)), null, 2));
