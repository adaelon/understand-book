import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { resolveAutomaticBuildTarget } from "../../packages/core/src/build-orchestrator";
import { readBookStructureGenerationTask, readBookStructureGenerationArtifact, readStructureCandidateCatalog,
  type BookStructureGenerationTaskV1 } from "../../packages/core/src/book-structure-generation";
import { bookStructureCanonicalTitle, type BookStructureUnitArtifact, type BookStructureUnitCard } from "../../packages/core/src/book-structure";
import type { LidNode } from "../../packages/core/src/generated/LidNode";
import type { StructureCandidateCatalog } from "../../packages/core/src/book-structure-candidates";
import { acceptStructureOutline, applyStructureChapterAction, newStructureChapterWork, structureChapterInput,
  type StructureOutlineInput, type StructureOutline, type StructureChapterWork, type StructureSourceExcerpt } from "../../packages/core/src/book-structure-planning";
import { materializeStructureChapterSelections } from "../../packages/core/src/book-structure-materialization";
import { renderStructurePlanningInput } from "../../packages/core/src/model-input-renderer";
import { estimateTokens } from "../../packages/core/src/window";
import { DEFAULT_BOOK, assertSourceExcerpt, summarizeUsage } from "./extract";

const root = fileURLToPath(new URL("../../", import.meta.url));
const fixtures = path.join(root, "packages/core/testdata/book-structure");
const json = <T>(file: string): T => JSON.parse(readFileSync(file, "utf8"));
interface SampleChapter {
  chapter: { unit_lid: string; contribution_ref: string; task_path: string; card: BookStructureUnitCard };
  recovered_candidates: Array<{ ref: string; task_path: string; evidence: Array<{ lids: string[]; text: string }> }>;
  evidence: Array<{ lids: string[]; text: string; span: { start: number; end: number } }>;
  published: unknown;
}
export interface ChapterExperimentInput {
  version: "book_structure_chapter_experiment.v1";
  source_workspace: string;
  outline_input: StructureOutlineInput;
  catalog: StructureCandidateCatalog;
  excerpts: StructureSourceExcerpt[];
  old_published: unknown;
  historical_estimates: unknown;
}

/** Select inputs explicitly. acceptance.json is read only by the separate comparison command. */
export function prepareChapterExperiment(book = DEFAULT_BOOK): ChapterExperimentInput {
  const sample = json<SampleChapter>(path.join(fixtures, "chapter-8.input.json"));
  const baseline = json<{ chapter_cards: Array<{ unit_lid: string; task_path: string }>; estimates: unknown }>(path.join(fixtures, "baseline.json"));
  const target = resolveAutomaticBuildTarget(book);
  const source = readFileSync(path.join(book, "source.txt"), "utf8");
  const nodes = json<{ lid_nodes: LidNode[] }>(path.join(book, "base.json")).lid_nodes;
  const byLid = new Map(nodes.map(node => [node.lid, node]));
  const load = (relative: string) => {
    const identity = json<BookStructureGenerationTaskV1>(path.join(book, relative));
    const task = readBookStructureGenerationTask(target, identity.policy_generation_id, identity.descriptor.work_unit_id);
    if (!task) throw new Error(`missing current task: ${relative}`);
    if ("excerpts" in task.input) for (const excerpt of task.input.excerpts) assertSourceExcerpt(nodes, source, excerpt);
    return task;
  };
  const chapters = baseline.chapter_cards.map(item => {
    const task = load(item.task_path);
    const artifact = readBookStructureGenerationArtifact(target, task);
    if (!artifact || task.output_role !== "unit_artifact") throw new Error("missing accepted chapter overview");
    const card = (artifact.payload as BookStructureUnitArtifact).output.unit_card;
    const unit = byLid.get(card.unit_lid);
    if (!unit) throw new Error("chapter no longer exists");
    return { unit_lid: card.unit_lid, title: bookStructureCanonicalTitle(unit, byLid, source), overview: card.summary };
  });
  const tasks = [...new Set([sample.chapter.task_path, ...sample.recovered_candidates.map(c => c.task_path)])].map(load);
  const catalog = readStructureCandidateCatalog(target, tasks, nodes.filter(n => n.kind === "section").map(n => n.lid));
  const excerpts = sample.evidence.map(e => {
    if (source.slice(e.span.start, e.span.end) !== e.text) throw new Error("chapter sample source changed; refresh BSR0 before running");
    return { unit_lid: sample.chapter.unit_lid, lids: e.lids, text: e.text };
  });
  const prefaceUnits = new Set(chapters.filter(c => /前言|preface/iu.test(c.title)).map(c => c.unit_lid));
  const preface = nodes.filter(n => !n.children.length).flatMap(n => {
    const unit = [...prefaceUnits].find(lid => n.lid.startsWith(lid + "."));
    return unit ? [{ unit_lid: unit, lids: [n.lid], text: source.slice(n.span.start, n.span.end) }] : [];
  });
  return { version: "book_structure_chapter_experiment.v1", source_workspace: path.resolve(book),
    outline_input: { version: "book_structure_outline_input.v1", chapters, preface }, catalog, excerpts,
    old_published: sample.published, historical_estimates: baseline.estimates };
}

interface Call {
  phase: "outline" | "chapter";
  status: "reserved" | "received" | "accepted" | "rejected" | "failed";
  input: string; request_input?: string; response?: string; error?: string; elapsed_ms?: number;
  reported_model?: string; finish_reason?: string;
  usage: { input_tokens?: number; output_tokens?: number; cached_input_tokens?: number };
}
interface Run { calls: Call[]; outline?: StructureOutline; chapter: StructureChapterWork; elapsed_ms: number }
interface Plan {
  version: "book_structure_bsr2_plan.v1";
  plan_id: string; revision: 1; status: "draft" | "confirmed"; confirmed_at?: string;
  generation: { model: string; endpoint: string };
  max_calls: number; max_wall_minutes: number; max_input_tokens: number;
  output_tokens: { outline: number; chapter: number };
  prompts: { outline: string; chapter: string };
  input: ChapterExperimentInput;
}

export async function chapterExperimentCli(args: string[]) {
  const [command, output, book = DEFAULT_BOOK] = args;
  if (!output || !["prepare", "confirm", "run", "report"].includes(command)) throw new Error("Usage: chapter.ts prepare|confirm|run|report OUTPUT_DIR [BOOK_DIR]");
  const dir = path.resolve(output);
  const file = (name: string) => path.join(dir, name);
  const save = (name: string, value: unknown) => {
    mkdirSync(dir, { recursive: true }); writeFileSync(file(name + ".tmp"), JSON.stringify(value, null, 2) + "\n");
    renameSync(file(name + ".tmp"), file(name));
  };
  if (existsSync(path.join(root, ".env"))) process.loadEnvFile(path.join(root, ".env"));
  const provider = () => {
    const model = process.env.FLUID_LLM_MODEL, base = process.env.OPENCODE_BASE_URL;
    if (!model || !base || !process.env.OPENCODE_API_KEY) throw new Error("Existing generation provider is not configured");
    return { model, endpoint: new URL("chat/completions", base.replace(/\/?$/, "/")).href };
  };
  const prompts = { outline: readFileSync(path.join(root, "agents/book-structure-outline.md"), "utf8"),
    chapter: readFileSync(path.join(root, "agents/book-structure-chapter.md"), "utf8") };
  if (command === "prepare") {
    if (existsSync(file("plan.json"))) throw new Error("Plan already exists; do not reset the experiment quota");
    const input = prepareChapterExperiment(book);
    const plan: Plan = { version: "book_structure_bsr2_plan.v1", plan_id: "bsr2-chapter-8-" + path.basename(dir), revision: 1,
      status: "draft", generation: provider(), max_calls: 20, max_wall_minutes: 30, max_input_tokens: 12000,
      output_tokens: { outline: 5000, chapter: 3500 }, prompts, input };
    const rendered = renderStructurePlanningInput(input.outline_input, plan.max_input_tokens);
    save("plan.json", plan);
    save("review.json", { plan_id: plan.plan_id, revision: plan.revision, status: plan.status, generation: plan.generation,
      scope: "ai-infra 第 8 章 + 14 张已有章节概述、真实目录和前言；仅局部评估，不发布", candidates: input.catalog.candidates.length,
      max_calls: plan.max_calls, max_wall_minutes: plan.max_wall_minutes, max_input_tokens_per_call: plan.max_input_tokens,
      output_tokens: plan.output_tokens, max_total_input_tokens: plan.max_calls * plan.max_input_tokens,
      max_total_output_tokens: plan.max_calls * Math.max(...Object.values(plan.output_tokens)),
      initial_outline_input_estimate: estimateTokens(rendered), embedding_calls: 0,
      note: "输入估算包括提示词时在每次调用前另行检查；未知实际 usage 保持未知，拒绝与失败也消耗调用额度。" });
    console.log(readFileSync(file("review.json"), "utf8")); return;
  }
  const plan = json<Plan>(file("plan.json"));
  if (plan.version !== "book_structure_bsr2_plan.v1" || !isDeepStrictEqual(plan.prompts, prompts)) throw new Error("Planning contract changed; prepare a new reviewable plan");
  if (command === "confirm") {
    // Invoked by the operator only after explicit approval of this exact plan/revision.
    if (plan.status !== "confirmed") { plan.status = "confirmed"; plan.confirmed_at = new Date().toISOString(); save("plan.json", plan); }
    console.log(JSON.stringify({ plan_id: plan.plan_id, revision: plan.revision, status: plan.status })); return;
  }
  const run: Run = existsSync(file("run.json")) ? json<Run>(file("run.json"))
    : { calls: [], chapter: newStructureChapterWork("10"), elapsed_ms: 0 };
  const titles = Object.fromEntries(plan.input.outline_input.chapters.map(c => [c.unit_lid, c.title]));
  const context = () => ({ catalog: plan.input.catalog, outline: run.outline!, titles, excerpts: plan.input.excerpts });
  const report = () => {
    const materialized = run.chapter.result ? materializeStructureChapterSelections(plan.input.catalog, [run.chapter.result], ["10"], titles) : undefined;
    const result = { version: "book_structure_bsr2_report.v1", plan_id: plan.plan_id, revision: plan.revision,
      complete: !!materialized, calls: run.calls.length, rejected: run.calls.filter(c => c.status === "rejected").length,
      failed: run.calls.filter(c => c.status === "failed").length, elapsed_ms: run.elapsed_ms,
      tokens: summarizeUsage(run.calls), generation: plan.generation, embedding_calls: 0,
      reads: run.calls.filter(c => c.status === "accepted" && c.phase === "chapter").flatMap(c => {
        const a = JSON.parse(c.response!); return ["read", "inspect", "browse"].includes(a.kind) ? [a] : [];
      }), old_published: plan.input.old_published, new_structure: materialized ?? null,
      historical_estimates: plan.input.historical_estimates, historical_actual_tokens: null };
    save("report.json", result); return result;
  };
  if (command === "run") {
    if (plan.status !== "confirmed") throw new Error("Approve the exact BSR2 plan/revision and invoke confirm before real calls");
    if (!isDeepStrictEqual(plan.generation, provider())) throw new Error("Generation provider changed since plan preparation");
    if (!isDeepStrictEqual(plan.input, prepareChapterExperiment(plan.input.source_workspace))) throw new Error("Source or accepted contributions changed; prepare a new plan");
    const began = performance.now(), previousElapsed = run.elapsed_ms;
    const persist = () => { run.elapsed_ms = previousElapsed + performance.now() - began; save("run.json", run); report(); };
    try {
      while (!run.chapter.result) {
        const phase = run.outline ? "chapter" : "outline";
        const input = renderStructurePlanningInput(phase === "outline" ? plan.input.outline_input : structureChapterInput(run.chapter, context()), plan.max_input_tokens);
        let call = run.calls.at(-1);
        if (call?.status === "reserved") throw new Error("Previous call outcome unknown; preserve its quota and inspect before resuming");
        if (call?.status === "received") {
          if (call.input !== input || call.phase !== phase) throw new Error("Stored response input changed");
        } else {
          const remaining = plan.max_wall_minutes * 60000 - previousElapsed - (performance.now() - began);
          if (run.calls.length >= plan.max_calls || remaining <= 0) throw new Error("Confirmed experiment budget exhausted");
          const feedback = call?.status === "rejected" ? `\nCore rejected: ${call.error}\nPrevious action: ${call.response}` : "";
          if (estimateTokens(prompts[phase] + input + feedback) > plan.max_input_tokens) throw new Error("Complete prompt exceeds input budget");
          call = { phase, status: "reserved", input, request_input: input + feedback, usage: {} }; run.calls.push(call); persist();
          const start = performance.now();
          try {
            const response = await fetch(plan.generation.endpoint, { method: "POST", signal: AbortSignal.timeout(Math.ceil(Math.min(300000, remaining))),
              headers: { authorization: `Bearer ${process.env.OPENCODE_API_KEY}`, "content-type": "application/json" },
              body: JSON.stringify({ model: plan.generation.model, temperature: 0, max_tokens: plan.output_tokens[phase], thinking: { type: "disabled" },
                response_format: { type: "json_object" }, messages: [{ role: "system", content: prompts[phase] }, { role: "user", content: call.request_input }] }) });
            if (!response.ok) throw new Error(`generation HTTP ${response.status}`);
            const body = await response.json() as { model?: string; choices?: Array<{ finish_reason?: string; message?: { content?: string } }>;
              usage?: { prompt_tokens?: number; completion_tokens?: number; prompt_tokens_details?: { cached_tokens?: number } } };
            call.response = body.choices?.[0]?.message?.content ?? ""; call.reported_model = body.model; call.finish_reason = body.choices?.[0]?.finish_reason;
            call.usage = { input_tokens: body.usage?.prompt_tokens, output_tokens: body.usage?.completion_tokens,
              cached_input_tokens: body.usage?.prompt_tokens_details?.cached_tokens };
            call.elapsed_ms = performance.now() - start; call.status = "received"; persist();
          } catch (error) {
            call.elapsed_ms = performance.now() - start; call.status = "failed"; call.error = String(error); persist(); throw error;
          }
        }
        try {
          const value = JSON.parse(call.response!);
          if (phase === "outline") run.outline = acceptStructureOutline(value, plan.input.outline_input);
          else run.chapter = applyStructureChapterAction(run.chapter, value, context());
          call.status = "accepted";
        } catch (error) { call.status = "rejected"; call.error = String(error); }
        persist();
        console.log(JSON.stringify({ phase, call: run.calls.length, status: call.status, complete: !!run.chapter.result }));
        if (run.calls.slice(-3).length === 3 && run.calls.slice(-3).every(c => c.status === "rejected")) throw new Error("Three consecutive rejected actions; inspect before spending more");
      }
    } finally { persist(); }
  }
  const result = report();
  // Answer anchors are only an audit aid. A source/content review must still assess the chosen mechanisms.
  if (run.chapter.result) {
    const answers = json<{ chapter: { requirements: Array<{ mechanism: string; evidence: string[]; expected: string }> } }>(path.join(fixtures, "acceptance.json"));
    save("comparison.json", answers.chapter.requirements.map(r => ({ ...r,
      selected_refs: plan.input.catalog.candidates.filter(c => run.chapter.result!.selection.accepted_stop_refs.includes(c.ref)
        && r.evidence.some(lid => c.evidence_lids.includes(lid))).map(c => c.ref) })));
  }
  console.log(JSON.stringify({ complete: result.complete, calls: result.calls, tokens: result.tokens, elapsed_ms: result.elapsed_ms }));
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await chapterExperimentCli(process.argv.slice(2));
