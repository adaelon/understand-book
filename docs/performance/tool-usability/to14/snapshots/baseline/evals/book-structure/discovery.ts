import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { resolveAutomaticBuildTarget, readAutomaticBuildTaskStage } from "../../packages/core/src/build-orchestrator";
import { freezeBookStructureGenerationTask, writeBookStructureGenerationCandidate, readBookStructureGenerationArtifact,
  readStructureCandidateCatalog, renderBookStructureGenerationTaskInput, type BookStructureGenerationTaskV1 } from "../../packages/core/src/book-structure-generation";
import { STRUCTURE_DISCOVERY_PROMPT } from "../../packages/core/src/book-structure-discovery";
import { STRUCTURE_ORGANIZATION_PROMPTS } from "../../packages/core/src/book-structure-organization-prompts";
import { estimateTokens } from "../../packages/core/src/window";
import { DEFAULT_BOOK } from "./extract";

const json = <T>(file: string): T => JSON.parse(readFileSync(file, "utf8"));
const save = (file: string, value: unknown) => writeFileSync(file, JSON.stringify(value, null, 2) + "\n");
const SOURCE_FILES = ["source.txt", "source_manifest.json", "profile_metadata.json", "base.json", "discourse_index.json", "formula_semantics.json"];
interface Plan { version: "book_structure_discovery_experiment.v1"; status: "draft" | "confirmed";
  source_workspace: string; workspace: string; unit_lid: string; execution: "codex_subagent";
  max_submissions: number; max_minutes: number; prepared_at: string; confirmed_at?: string }
interface Step { ordinal: number; task_file: string; input_file: string; prompt_file: string; opened_at: string;
  status: "pending" | "accepted" | "rejected"; response_file?: string; error?: string; elapsed_ms?: number;
  input_estimated_tokens: number; output_estimated_tokens?: number }
interface Session { steps: Step[]; started_at?: string }

export function discoveryExperiment(command: string, output: string, response?: string, book = DEFAULT_BOOK) {
  const dir = path.resolve(output), file = (name: string) => path.join(dir, name);
  if (command === "prepare") {
    if (existsSync(file("plan.json"))) throw Error("Existing plan and quota must not be overwritten");
    mkdirSync(dir, { recursive: true });
    const workspace = path.resolve("tmp", `bsr6-${path.basename(dir)}`, ".understand-book", path.basename(book));
    if (existsSync(workspace)) throw Error("Experiment workspace already exists");
    mkdirSync(workspace, { recursive: true });
    // Reuse the canonical source and accepted public prerequisites, never a structure intermediate or publication.
    for (const name of SOURCE_FILES)
      cpSync(path.join(book, name), path.join(workspace, name));
    const plan: Plan = { version: "book_structure_discovery_experiment.v1", status: "draft", source_workspace: book,
      workspace, unit_lid: "10", execution: "codex_subagent", max_submissions: 80, max_minutes: 60, prepared_at: new Date().toISOString() };
    save(file("plan.json"), plan); save(file("session.json"), { steps: [] });
    const source = readFileSync(path.join(workspace, "source.txt"), "utf8");
    const base = json<{ lid_nodes: Array<{ lid: string; children: string[]; span: { start: number; end: number } }> }>(path.join(workspace, "base.json"));
    const chapter = base.lid_nodes.find(n => n.lid === plan.unit_lid)!;
    writeFileSync(file("chapter-source.txt"), source.slice(chapter.span.start, chapter.span.end));
    const review = { ...plan, chapter: "第 8 章 推理优化", leaf_count: base.lid_nodes.filter(n => !n.children.length && n.lid.startsWith(plan.unit_lid + ".")).length,
      material: "Canonical full chapter; source-only global outline; current graph/discourse/formula. No old BookStructure tasks, cards or candidates.",
      model_usage: "Actual Codex model calls/tokens unavailable; submissions and serialized estimates recorded separately.",
      embedding: "none", external_generation_api: "none", publication: "none", budget: "80 submissions including rejections/retries; 60 minutes from first next; formal per-task input/output/transport bounds" };
    save(file("review.json"), review); return review;
  }
  const plan = json<Plan>(file("plan.json")), session = json<Session>(file("session.json"));
  if (command === "confirm") {
    if (plan.status !== "draft") throw Error("Plan already confirmed");
    plan.status = "confirmed"; plan.confirmed_at = new Date().toISOString(); save(file("plan.json"), plan); return { status: plan.status };
  }
  const target = resolveAutomaticBuildTarget(plan.workspace);
  if (command === "replay") {
    const original = json<{ candidates: unknown; tasks: Array<{ id: string; range: unknown; payload: unknown }> }>(file("report.json"));
    const replayDir = file("replay"), workspace = path.join(path.dirname(path.dirname(plan.workspace)), "replay-workspace", ".understand-book", path.basename(plan.workspace));
    if (existsSync(replayDir) || existsSync(workspace)) throw Error("Replay already exists; keep its verification record");
    mkdirSync(replayDir, { recursive: true }); mkdirSync(workspace, { recursive: true });
    for (const name of SOURCE_FILES) cpSync(path.join(plan.workspace, name), path.join(workspace, name));
    save(path.join(replayDir, "plan.json"), { ...plan, workspace });
    save(path.join(replayDir, "session.json"), { steps: [] });
    for (const step of session.steps.filter(s => s.status === "accepted")) {
      const next = discoveryExperiment("next", replayDir) as Step;
      if (readFileSync(next.input_file, "utf8") !== readFileSync(step.input_file, "utf8")
        || readFileSync(next.prompt_file, "utf8") !== readFileSync(step.prompt_file, "utf8")) throw Error(`Replay delivery changed at ${step.ordinal}`);
      const result = discoveryExperiment("submit", replayDir, step.response_file) as { status: string; error?: string };
      if (result.status !== "accepted") throw Error(`Replay rejected ${step.ordinal}: ${result.error}`);
    }
    if ((discoveryExperiment("next", replayDir) as { status?: string }).status !== "complete") throw Error("Replay still has pending chapter work");
    discoveryExperiment("report", replayDir);
    const replayed = json<typeof original>(path.join(replayDir, "report.json"));
    if (!isDeepStrictEqual(original.candidates, replayed.candidates) || !isDeepStrictEqual(original.tasks, replayed.tasks)) throw Error("Replay candidates or grounded payloads changed");
    const verification = { status: "passed", accepted_steps: session.steps.filter(s => s.status === "accepted").length,
      same_inputs: true, same_candidates: true, same_payloads: true, model_calls: 0, provider_calls: 0 };
    save(file("verification.json"), verification); return verification;
  }
  const state = () => readAutomaticBuildTaskStage(target, { stage: "book_structure", work_unit_id: "discovery-experiment", parent_lid: plan.unit_lid }, "full")!;
  const current = (stage = state()) => Object.values(stage.generation_tasks ?? {}).flatMap(g => g.kind === "book_structure"
    && (g.task.descriptor.kind === "structure_outline" || g.task.parent_unit_lid === plan.unit_lid) ? [g.task] : []);
  if (command === "next") {
    if (plan.status !== "confirmed") throw Error("Confirm this experiment's execution method and quota first");
    if (session.started_at && Date.now() - Date.parse(session.started_at) > plan.max_minutes * 60000) throw Error("Experiment time quota exhausted; accepted results retained");
    const pending = session.steps.find(s => s.status === "pending"); if (pending) return pending;
    if (session.steps.length >= plan.max_submissions) throw Error("Experiment submission quota exhausted; accepted results retained");
    if (session.steps.slice(-3).length === 3 && session.steps.slice(-3).every(s => s.status === "rejected")) throw Error("Three consecutive rejections; inspect failure before continuing");
    const s = state(); if (s.structure_blocked) throw Error(s.structure_blocked);
    const task = current(s).find(t => s.pending_tasks.includes(t.descriptor.work_unit_id));
    if (!task) return { status: "complete", command: `discovery.ts report ${dir}` };
    session.started_at ??= new Date().toISOString();
    const ordinal = session.steps.length + 1, stem = String(ordinal).padStart(3, "0");
    const input = renderBookStructureGenerationTaskInput(task), prompt = task.descriptor.kind === "structure_outline" ? STRUCTURE_ORGANIZATION_PROMPTS.structure_outline : STRUCTURE_DISCOVERY_PROMPT;
    const step: Step = { ordinal, task_file: file(`${stem}.task.json`), input_file: file(`${stem}.input.json`), prompt_file: file(`${stem}.prompt.md`),
      opened_at: new Date().toISOString(), status: "pending", input_estimated_tokens: estimateTokens(prompt + input) };
    freezeBookStructureGenerationTask(target, task); save(step.task_file, task);
    writeFileSync(step.input_file, input); writeFileSync(step.prompt_file, prompt);
    session.steps.push(step); save(file("session.json"), session); return step;
  }
  if (command === "submit") {
    if (plan.status !== "confirmed") throw Error("Unconfirmed experiment");
    const step = session.steps.find(s => s.status === "pending"); if (!step || !response) throw Error("No pending input or response file");
    const task = json<BookStructureGenerationTaskV1>(step.task_file), raw = readFileSync(response, "utf8");
    step.response_file = file(`${String(step.ordinal).padStart(3, "0")}.response.json`); writeFileSync(step.response_file, raw);
    step.output_estimated_tokens = estimateTokens(raw);
    try {
      const value = JSON.parse(raw);
      if (Date.now() - Date.parse(session.started_at!) > plan.max_minutes * 60000) throw Error("Experiment time quota exhausted; response retained but not accepted");
      if (step.output_estimated_tokens > task.descriptor.execution_budget_proof.max_candidate_tokens) throw Error("Candidate exceeds formal output budget");
      writeBookStructureGenerationCandidate({ target, task, candidate: value,
        provenance: { executor: "bsr6-codex-subagent", attempt: step.ordinal, generated_at: new Date().toISOString() } });
      step.status = "accepted";
    } catch (e) { step.status = "rejected"; step.error = String(e); }
    step.elapsed_ms = Date.now() - Date.parse(step.opened_at);
    save(file("session.json"), session); return { ordinal: step.ordinal, status: step.status, error: step.error };
  }
  if (command === "report") {
    const tasks = current().filter(t => t.parent_unit_lid === plan.unit_lid);
    if (!tasks.length || tasks.some(t => !readBookStructureGenerationArtifact(target, t))) throw Error("Complete chapter discovery before reporting");
    const base = json<{ lid_nodes: Array<{ lid: string; kind: string }> }>(path.join(plan.workspace, "base.json"));
    const catalog = readStructureCandidateCatalog(target, tasks, base.lid_nodes.filter(n => n.kind === "section").map(n => n.lid));
    for (const step of session.steps.filter(s => s.status === "accepted")) {
      const task = json<BookStructureGenerationTaskV1>(step.task_file);
      if (!isDeepStrictEqual(renderBookStructureGenerationTaskInput(task), readFileSync(step.input_file, "utf8"))) throw Error("Saved input changed");
      const artifact = readBookStructureGenerationArtifact(target, task);
      if (!artifact) throw Error("Saved accepted result missing");
    }
    const s = state();
    const report = { status: "complete", source_workspace: plan.source_workspace, unit_lid: plan.unit_lid,
      coverage: s.quality_routing?.book_structure_coverage?.find(c => c.parent_unit_lid === plan.unit_lid),
      candidates: catalog, tasks: tasks.map(t => ({ id: t.descriptor.work_unit_id, range: t.source_range, payload: readBookStructureGenerationArtifact(target, t)!.payload })),
      submissions: session.steps.length, accepted: session.steps.filter(s => s.status === "accepted").length, rejected: session.steps.filter(s => s.status === "rejected").length,
      input_estimated_tokens: session.steps.reduce((n, s) => n + s.input_estimated_tokens, 0), output_estimated_tokens: session.steps.reduce((n, s) => n + (s.output_estimated_tokens ?? 0), 0),
      actual_model_calls: null, actual_model_tokens: null,
      wall_ms: Math.max(...session.steps.map(s => Date.parse(s.opened_at) + (s.elapsed_ms ?? 0))) - Date.parse(session.started_at!),
      embedding_calls: 0, external_generation_api_calls: 0 };
    save(file("report.json"), report); return { status: report.status, candidates: catalog.candidates.length, submissions: report.submissions, coverage: report.coverage };
  }
  throw Error("Use prepare|confirm|next|submit|report|replay OUTPUT_DIR [RESPONSE_FILE]");
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  console.log(JSON.stringify(discoveryExperiment(process.argv[2], process.argv[3], process.argv[4]), null, 2));
}
