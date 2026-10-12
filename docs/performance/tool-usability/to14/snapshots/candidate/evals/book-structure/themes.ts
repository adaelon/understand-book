import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { prepareChapterExperiment } from "./chapter";
import { DEFAULT_BOOK, assertSourceExcerpt } from "./extract";
import { resolveAutomaticBuildTarget } from "../../packages/core/src/build-orchestrator";
import { readBookStructureGenerationTask, readStructureCandidateCatalog, type BookStructureGenerationTaskV1 } from "../../packages/core/src/book-structure-generation";
import type { LidNode } from "../../packages/core/src/generated/LidNode";
import { planStructureThemes, structureThemePlanningInput, structureThemeInput, applyStructureThemeAction, reconcileStructureThemes,
  materializeStructureThemes, reopenStructureTheme, type StructureThemeContext, type StructureThemeDirectory } from "../../packages/core/src/book-structure-themes";
import { projectStructureRetrieval, prepareStructureRetrieval, STRUCTURE_RETRIEVAL, STRUCTURE_RETRIEVAL_POLICY } from "../../packages/core/src/book-structure-retrieval";
import { retrievalDependencies, type PreparedRetrieval } from "../../packages/core/src/semantic-retrieval-preparation";
import { retrieveHybrid } from "../../packages/core/src/semantic-retrieval";
import { readLocalEmbeddingConfig } from "../../packages/core/src/build-retrieval-config";
import { localEmbeddingIdentity, openLocalEmbeddingProvider } from "../../packages/core/src/local-embedding-provider";
import type { EmbeddingCall, EmbeddingProviderIdentity } from "../../packages/core/src/embedding-provider";
import { estimateTokens } from "../../packages/core/src/window";
import type { BookStructureCandidate } from "../../packages/core/src/book-structure";

const root = fileURLToPath(new URL("../../", import.meta.url));
const fixtureDir = path.join(root, "packages/core/testdata/book-structure");
const read = <T = any>(file: string): T => JSON.parse(readFileSync(file, "utf8"));
const save = (file: string, value: unknown) => { mkdirSync(path.dirname(file), { recursive: true }); writeFileSync(file + ".tmp", JSON.stringify(value, null, 2) + "\n"); renameSync(file + ".tmp", file); };
export interface ThemeExperimentPlan {
  version: "book_structure_theme_experiment.v1"; status: "confirmed"; authorization: string;
  context: StructureThemeContext; fixed_question: string; prompt: string;
  embedding: { config_file: string; identity: EmbeddingProviderIdentity; max_documents: number; max_queries: number; max_calls: number };
  max_steps_per_group: number; max_input_tokens: number; max_output_tokens: number;
  directory?: StructureThemeDirectory; planning_response?: unknown;
}
interface ThemeStep {
  ordinal: number; phase: "theme" | "reconcile"; theme_ref?: string; opened_at: string; finished_at?: string;
  prompt: string; input: string; input_tokens_estimate: number; output_tokens_estimate?: number;
  preparation?: Pick<PreparedRetrieval, "sequence" | "diagnostics" | "query_embedding">;
  response?: unknown; status: "pending" | "accepted" | "rejected"; error?: string;
}
export interface ThemeExperimentRun {
  group: "B" | "C"; directory: StructureThemeDirectory; steps: ThemeStep[]; prepared?: PreparedRetrieval;
  embedding_attempts: Array<{ role: "document" | "query"; records: number; input_tokens: number; result?: EmbeddingCall }>;
  started_at?: string; completed_at?: string;
  revisions?: Array<{ before_step: number; ref: string; issue: string }>;
}

/** Reload accepted sources, use all stops in named contributions, keep answers out of the input. */
export function prepareThemeExperiment(configFile: string, book = DEFAULT_BOOK): ThemeExperimentPlan {
  const chapter = prepareChapterExperiment(book), baseline = read(path.join(fixtureDir, "baseline.json")), sample = read(path.join(fixtureDir, "state-reuse.input.json"));
  const target = resolveAutomaticBuildTarget(book), source = readFileSync(path.join(book, "source.txt"), "utf8");
  const nodes: LidNode[] = read(path.join(book, "base.json")).lid_nodes;
  const paths: string[] = [...new Set<string>([...baseline.chapter_cards.map((c: any) => c.task_path),
    ...sample.candidates.map((c: any) => baseline.tasks.find((t: any) => t.work_unit_id === c.contribution_ref).task_path)])];
  const tasks = paths.map(relative => {
    const stored = read<BookStructureGenerationTaskV1>(path.join(book, relative));
    const task = readBookStructureGenerationTask(target, stored.policy_generation_id, stored.descriptor.work_unit_id);
    if (!task) throw new Error("missing accepted theme contribution");
    if ("excerpts" in task.input) task.input.excerpts.forEach(e => assertSourceExcerpt(nodes, source, e));
    return task;
  });
  const catalog = readStructureCandidateCatalog(target, tasks, nodes.filter(n => n.kind === "section").map(n => n.lid));
  const excerpts: StructureThemeContext["excerpts"] = sample.evidence.map((e: any) => {
    if (source.slice(e.span.start, e.span.end) !== e.text) throw new Error("theme source sample changed");
    return { unit_lid: e.lids[0].split(".")[0], lids: e.lids, text: e.text };
  });
  // Canonical spans for all candidate evidence support themes beyond the fixed sample.
  const byLid = new Map(nodes.map(n => [n.lid, n]));
  for (const c of catalog.candidates) {
    const anchors = c.evidence_lids.map(lid => byLid.get(lid)!);
    if (anchors.some(n => !n)) throw new Error("candidate evidence missing from canonical source");
    const text = anchors.map(n => source.slice(n.span.start, n.span.end)).join("\n");
    if (text.trim() && !excerpts.some(e => isDeepStrictEqual(e.lids, c.evidence_lids))) excerpts.push({ unit_lid: c.unit_lid, lids: c.evidence_lids, text });
  }
  return { version: "book_structure_theme_experiment.v1", status: "confirmed",
    authorization: "User selected Codex subagent + installed local MiniLM for BSR3–BSR4 (2026-10-01); independent experiment quotas, no inherited SR allowance.",
    context: { catalog, chapters: chapter.outline_input.chapters, excerpts, mode: "lexical_only" }, fixed_question: sample.question,
    prompt: readFileSync(path.join(root, "agents/book-structure-themes.md"), "utf8"),
    embedding: { config_file: path.resolve(configFile), identity: localEmbeddingIdentity(readLocalEmbeddingConfig(configFile)), max_documents: 600, max_queries: 80, max_calls: 160 },
    max_steps_per_group: 80, max_input_tokens: 20000, max_output_tokens: 6500 };
}
const contextFor = (plan: ThemeExperimentPlan, run: ThemeExperimentRun): StructureThemeContext => ({ ...plan.context,
  mode: run.group === "B" ? "lexical_only" : "semantic_required", prepared: run.prepared });
export function themeDelivery(plan: ThemeExperimentPlan, run: ThemeExperimentRun) {
  const work = run.directory.works.find(w => !w.result), context = contextFor(plan, run);
  const payload = work ? { phase: "theme", ...structureThemeInput(run.directory, work.plan.ref, context) }
    : { phase: "reconcile", themes: run.directory.works.map(w => w.result),
      directory: run.directory.works.map(w => w.plan) };
  const input = JSON.stringify(payload, null, 2), tokens = estimateTokens(plan.prompt + input);
  if (tokens > plan.max_input_tokens) throw new Error(`theme input exceeds ${plan.max_input_tokens} tokens; request smaller material pages`);
  return { phase: work ? "theme" as const : "reconcile" as const, ...(work ? { theme_ref: work.plan.ref } : {}),
    prompt: plan.prompt, input, input_tokens_estimate: tokens };
}
export function submitThemeStep(plan: ThemeExperimentPlan, run: ThemeExperimentRun, ordinal: number, response: unknown) {
  const step = run.steps.at(-1);
  if (!step || step.ordinal !== ordinal || step.status !== "pending") throw new Error("no matching pending theme step");
  step.response = response; step.finished_at = new Date().toISOString(); step.output_tokens_estimate = estimateTokens(JSON.stringify(response));
  try {
    if (step.output_tokens_estimate > plan.max_output_tokens) throw new Error("theme output reserve exceeded");
    const context = contextFor(plan, run);
    const next = step.phase === "theme" ? applyStructureThemeAction(run.directory, step.theme_ref!, response, context)
      : reconcileStructureThemes(run.directory, response, context);
    // Inspect/read are pure and can be budget-checked before accepting. Searches require next preparation.
    const prospective = { ...run, directory: next };
    if (!next.reconciled && (response as any).kind !== "search" && (response as any).kind !== "resolve") themeDelivery(plan, prospective);
    run.directory = next; step.status = "accepted";
    if (next.reconciled) run.completed_at = step.finished_at;
  } catch (e) { step.status = "rejected"; step.error = String(e); }
  return { status: step.status, error: step.error ?? null, complete: run.directory.reconciled && !run.directory.unresolved.length };
}
function baseFor(plan: ThemeExperimentPlan): BookStructureCandidate {
  return { spine: plan.context.chapters.map(c => ({ lid: c.unit_lid, role: "method", summary: c.overview, key_stop_ids: [], depends_on: [] })),
    key_stops: [], throughlines: [], unit_titles: Object.fromEntries(plan.context.chapters.map(c => [c.unit_lid, c.title])),
    reference_scope: { unit_lids: plan.context.chapters.map(c => c.unit_lid), dependency_target_lids: plan.context.chapters.map(c => c.unit_lid),
      evidence_by_unit: Object.fromEntries(plan.context.chapters.map(c => [c.unit_lid, c.overview.evidence_lids])) } };
}
export function themeExperimentReport(plan: ThemeExperimentPlan, run: ThemeExperimentRun) {
  const complete = run.directory.reconciled && !run.directory.unresolved.length;
  return { group: run.group, complete, candidates: plan.context.catalog.candidates.length, theme_count: run.directory.works.length,
    targeted_revisions: run.revisions?.length ?? 0,
    semantic_submissions: run.steps.filter(s => s.response !== undefined).length, rejected: run.steps.filter(s => s.status === "rejected").length,
    input_tokens_estimate: run.steps.reduce((n, s) => n + s.input_tokens_estimate, 0), output_tokens_estimate: run.steps.reduce((n, s) => n + (s.output_tokens_estimate ?? 0), 0),
    actual_model_calls: null, actual_input_tokens: null, actual_output_tokens: null, external_generation_api_calls: 0,
    wall_ms: run.started_at && run.completed_at ? Date.parse(run.completed_at) - Date.parse(run.started_at) : null,
    actions: Object.fromEntries(["search", "inspect", "read", "resolve"].map(kind => [kind, run.steps.filter(s => s.status === "accepted" && (s.response as any)?.kind === kind).length])),
    embedding: { calls: run.embedding_attempts.length,
      documents: run.embedding_attempts.filter(c => c.role === "document").reduce((n, c) => n + c.records, 0),
      queries: run.embedding_attempts.filter(c => c.role === "query").reduce((n, c) => n + c.records, 0),
      actual_input_tokens: run.embedding_attempts.reduce((n, c) => n + (c.result?.usage?.input_tokens ?? 0), 0),
      unknown_usage_calls: run.embedding_attempts.filter(c => c.result?.usage?.input_tokens === undefined).length,
      inference_ms: run.embedding_attempts.reduce((n, c) => n + (c.result?.elapsed_ms ?? 0), 0) },
    themes: run.directory.works.map(w => w.result ?? null), unresolved: run.directory.unresolved,
    structure: complete ? materializeStructureThemes(baseFor(plan), run.directory, plan.context) : null };
}
export function replayThemeExperiment(plan: ThemeExperimentPlan, run: ThemeExperimentRun) {
  const replay: ThemeExperimentRun = { group: run.group, directory: structuredClone(plan.directory!), steps: [], embedding_attempts: [] };
  const index = projectStructureRetrieval(plan.context.catalog, plan.context.chapters);
  for (const step of run.steps) {
    if (step.status === "pending") throw new Error("cannot verify a pending step");
    for (const revision of run.revisions?.filter(r => r.before_step === step.ordinal) ?? [])
      replay.directory = reopenStructureTheme(replay.directory, revision.ref, revision.issue);
    const work = replay.directory.works.find(w => !w.result);
    if (step.preparation && work) {
      const request = { kind: "search" as const, query: work.query };
      replay.prepared = { version: "prepared_formal_object_retrieval.v1", dependencies: retrievalDependencies(index.records, request,
        run.group === "C" ? "semantic_required" : "lexical_only", run.group === "C" ? plan.embedding.identity : null,
        STRUCTURE_RETRIEVAL_POLICY, STRUCTURE_RETRIEVAL.projection_version), ...step.preparation };
      if (!isDeepStrictEqual(step.preparation.sequence, retrieveHybrid(index.records, request, step.preparation.diagnostics.semantic_scores, STRUCTURE_RETRIEVAL_POLICY))) throw new Error("saved ranking differs from scores");
    }
    const delivery = themeDelivery(plan, replay);
    if (delivery.input !== step.input || delivery.prompt !== step.prompt) throw new Error(`theme replay delivery mismatch at ${step.ordinal}`);
    replay.steps.push({ ...structuredClone(step), status: "pending" });
    const outcome = submitThemeStep(plan, replay, step.ordinal, step.response);
    if (outcome.status !== step.status) throw new Error(`theme replay admission mismatch at ${step.ordinal}`);
  }
  if (!isDeepStrictEqual(replay.directory, run.directory)) throw new Error("theme replay final state mismatch");
  return { steps: run.steps.length, state_equal: true, delivery_equal: true,
    materialization_equal: isDeepStrictEqual(themeExperimentReport(plan, replay).structure, themeExperimentReport(plan, run).structure) };
}

export async function themesCli(args: string[]) {
  const [command, output, groupOrConfig = "", ordinal, responseFile] = args;
  if (!output) throw new Error("themes.ts prepare|plan-next|plan-submit|next|submit|report|replay DIR [B|C|CONFIG] [ORDINAL RESPONSE_FILE]");
  const dir = path.resolve(output), planFile = path.join(dir, "plan.json");
  if (command === "prepare") {
    if (existsSync(planFile)) throw new Error("theme plan exists; resume it");
    const plan = prepareThemeExperiment(groupOrConfig); save(planFile, plan);
    save(path.join(dir, "review.json"), { authorization: plan.authorization, candidates: plan.context.catalog.candidates.length,
      chapters: plan.context.chapters.length, excerpts: plan.context.excerpts.length, embedding: plan.embedding,
      max_steps_per_group: plan.max_steps_per_group, max_input_tokens: plan.max_input_tokens, max_output_tokens: plan.max_output_tokens });
    return { status: "prepared", candidates: plan.context.catalog.candidates.length };
  }
  const plan = read<ThemeExperimentPlan>(planFile);
  if (command === "plan-next") {
    const input = { phase: "plan", fixed_question: plan.fixed_question, ...structureThemePlanningInput(plan.context) };
    const rendered = JSON.stringify(input, null, 2);
    if (estimateTokens(plan.prompt + rendered) > plan.max_input_tokens) throw new Error("theme planning input too large");
    writeFileSync(path.join(dir, "planning-input.json"), rendered); writeFileSync(path.join(dir, "prompt.md"), plan.prompt);
    return { input_file: path.join(dir, "planning-input.json"), prompt_file: path.join(dir, "prompt.md") };
  }
  if (command === "plan-submit") {
    if (plan.directory) throw new Error("theme plan already accepted");
    const response = read(groupOrConfig); const directory = planStructureThemes(response, plan.context);
    const fixed = directory.works.find(w => w.plan.question === plan.fixed_question);
    if (!fixed) throw new Error("include the fixed question verbatim");
    directory.works = [fixed, ...directory.works.filter(w => w !== fixed)];
    for (const work of directory.works) work.query = work.plan.question;
    plan.directory = directory; plan.planning_response = response; save(planFile, plan);
    return { status: "accepted", themes: directory.works.length };
  }
  if (!plan.directory || !["B", "C"].includes(groupOrConfig)) throw new Error("accepted plan and group B/C required");
  const group = groupOrConfig as "B" | "C", groupDir = path.join(dir, group), file = path.join(groupDir, "session.json");
  const run: ThemeExperimentRun = existsSync(file) ? read(file) : { group, directory: structuredClone(plan.directory), steps: [], embedding_attempts: [] };
  if (command === "reopen") {
    const issue = readFileSync(responseFile, "utf8").trim();
    const next = reopenStructureTheme(run.directory, ordinal, issue);
    save(path.join(groupDir, `before-revision-${run.revisions?.length ?? 0}.json`), run);
    (run.revisions ??= []).push({ before_step: run.steps.length, ref: ordinal, issue });
    run.directory = next; delete run.completed_at; save(file, run);
    return { status: "reopened", ref: ordinal, remaining_actions: plan.max_steps_per_group - run.steps.length };
  }
  if (command === "next") {
    if (run.directory.reconciled) return { status: "done", complete: !run.directory.unresolved.length };
    let step = run.steps.at(-1);
    if (!step || step.status !== "pending") {
      if (run.steps.length >= plan.max_steps_per_group) throw new Error("theme action budget exhausted");
      if (run.steps.slice(-3).length === 3 && run.steps.slice(-3).every(s => s.status === "rejected")) throw new Error("three consecutive rejections; inspect the contract before continuing");
      const work = run.directory.works.find(w => !w.result);
      run.started_at ??= new Date().toISOString();
      if (group === "C" && work) {
        const request = { kind: "search" as const, query: work.query }, index = projectStructureRetrieval(plan.context.catalog, plan.context.chapters);
        const needsProvider = request.query.trim() && (!run.prepared || !isDeepStrictEqual(run.prepared.dependencies,
          retrievalDependencies(index.records, request, "semantic_required", plan.embedding.identity, STRUCTURE_RETRIEVAL_POLICY, STRUCTURE_RETRIEVAL.projection_version)));
        const session = needsProvider ? await openLocalEmbeddingProvider(readLocalEmbeddingConfig(plan.embedding.config_file), plan.embedding.identity) : undefined;
        try {
          run.prepared = await prepareStructureRetrieval({ workspace: dir, index, request, mode: "semantic_required", provider_identity: plan.embedding.identity,
            provider: session?.provider, previous: run.prepared,
            before_call: call => {
              const used = run.embedding_attempts.filter(c => c.role === call.role).reduce((n, c) => n + c.records, 0);
              if (run.embedding_attempts.length >= plan.embedding.max_calls || used + call.records > (call.role === "document" ? plan.embedding.max_documents : plan.embedding.max_queries)) throw new Error("theme embedding quota exhausted");
              run.embedding_attempts.push(call); save(file, run);
            }, on_call: result => { run.embedding_attempts.at(-1)!.result = result; save(file, run); } });
        } finally { await session?.dispose(); save(file, run); }
      }
      step = { ordinal: run.steps.length, opened_at: new Date().toISOString(), ...themeDelivery(plan, run), status: "pending",
        ...(run.prepared && work ? { preparation: { sequence: run.prepared.sequence, diagnostics: run.prepared.diagnostics, query_embedding: run.prepared.query_embedding } } : {}) };
      run.steps.push(step); save(file, run);
    }
    mkdirSync(groupDir, { recursive: true });
    const inputFile = path.join(groupDir, `input-${step.ordinal}.json`), promptFile = path.join(groupDir, "prompt.md");
    writeFileSync(inputFile, step.input); writeFileSync(promptFile, step.prompt);
    return { status: "input", ordinal: step.ordinal, phase: step.phase, input_file: inputFile, prompt_file: promptFile,
      prior_error: run.steps.at(-2)?.error ?? null, remaining_actions: plan.max_steps_per_group - step.ordinal };
  }
  if (command === "submit") {
    const result = submitThemeStep(plan, run, Number(ordinal), read(responseFile)); save(file, run); return result;
  }
  if (command === "report") { const report = themeExperimentReport(plan, run); save(path.join(groupDir, "report.json"), report); return { complete: report.complete, themes: report.theme_count, submissions: report.semantic_submissions, embedding: report.embedding }; }
  if (command === "replay") { const result = replayThemeExperiment(plan, run); save(path.join(groupDir, "verification.json"), result); return result; }
  throw new Error("unknown theme command");
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) themesCli(process.argv.slice(2)).then(r => console.log(JSON.stringify(r, null, 2))).catch(e => { console.error(e); process.exitCode = 1; });
