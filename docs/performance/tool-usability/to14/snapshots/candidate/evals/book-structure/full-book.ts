import { cpSync, existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { readAutomaticBuildTaskStage, resolveAutomaticBuildTarget } from "../../packages/core/src/build-orchestrator";
import { freezeBookStructureGenerationTask, readBookStructureGenerationArtifact, readStructureCandidateCatalog,
  renderBookStructureGenerationTaskInput, writeBookStructureGenerationCandidate, type BookStructureGenerationTaskV1 } from "../../packages/core/src/book-structure-generation";
import { STRUCTURE_DISCOVERY_PROMPT } from "../../packages/core/src/book-structure-discovery";
import { STRUCTURE_ORGANIZATION_PROMPTS } from "../../packages/core/src/book-structure-organization-prompts";
import { configureBuildRetrieval, configuredBuildRetrievalRuntime, reviseBuildRetrieval } from "../../packages/core/src/build-retrieval-config";
import { prepareBuildRetrieval, readBuildRetrievalState } from "../../packages/core/src/automatic-build-retrieval";
import { summarizeRetrievalUsage } from "../../packages/core/src/automatic-build-budget";
import { transitionBuildPlan, type BuildPlanV1 } from "../../packages/core/src/build-intent";
import { collectAutomaticBuildStageQuality } from "../../packages/core/src/automatic-build-quality";
import { prepareExplicitLegacyBuildPlan, runAutomaticBuildCloseStage } from "../../skills/build/automatic-build";
import { estimateTokens } from "../../packages/core/src/window";
import { DEFAULT_BOOK } from "./extract";
import { requestStructureChapterRevision, requestStructureChapterSelectionContinuation, requestStructureThemeRevision,
  type StructureThemeRevisionRequest } from "../../packages/core/src/book-structure-organization";
import type { StructureChapterRevisionRequest, StructureChapterSelectionContinuation } from "../../packages/core/src/book-structure-planning";

const SOURCE_FILES = ["source.txt", "source_manifest.json", "profile_metadata.json", "base.json", "discourse_index.json", "formula_semantics.json"];
const read = <T>(file: string): T => JSON.parse(readFileSync(file, "utf8"));
const save = (file: string, value: unknown) => { writeFileSync(file + ".tmp", JSON.stringify(value, null, 2) + "\n"); renameSync(file + ".tmp", file); };
export interface FullBookPlan {
  version: "book_structure_full_book_experiment.v1"; status: "draft" | "confirmed";
  source_workspace: string; workspace: string; execution: "codex_subagent";
  max_submissions: number; max_minutes: number; embedding_config?: string;
  retrieval_mode: "lexical_only" | "semantic_required"; prepared_at: string; confirmed_at?: string;
  authorization?: string; build_plan: BuildPlanV1;
  budget_parent_dir?: string; comparison_dir?: string;
  discovery_units?: { unit_lid: string; max_submissions: number }[];
}
interface Step {
  ordinal: number; kind: string; unit_lid: string; task_file: string; input_file: string; prompt_file: string;
  status: "pending" | "accepted" | "rejected"; opened_at: string; finished_at?: string;
  response_file?: string; error?: string; input_estimated_tokens: number; output_estimated_tokens?: number;
  max_candidate_estimated_tokens?: number;
}
interface Session {
  steps: Step[]; started_at?: string;
  failure_recoveries?: { after_ordinal: number; reason: string; at: string }[];
}
export interface ChapterRevisionEvent {
  kind: "requested" | "opened" | "accepted" | "rejected";
  revision_id: string; at: string; ordinal?: number; request?: StructureChapterRevisionRequest;
}
export interface ChapterSelectionEvent {
  kind: "requested" | "opened" | "accepted" | "rejected";
  continuation_id: string; at: string; ordinal?: number; request?: StructureChapterSelectionContinuation;
}
export interface ThemeRevisionEvent {
  kind: "requested" | "opened" | "accepted" | "rejected";
  revision_id: string; at: string; ordinal?: number; request?: StructureThemeRevisionRequest;
}

/** Real generation is performed outside this adapter. All semantic acceptance and publication remain Core-owned. */
export async function fullBookExperiment(command: string, output: string, argument?: string,
  options: { book?: string; config?: string; mode?: FullBookPlan["retrieval_mode"]; submissions?: number; minutes?: number; unit?: string } = {}) {
  const dir = path.resolve(output), file = (name: string) => path.join(dir, name);
  if (command === "prepare") {
    if (existsSync(file("plan.json"))) throw Error("Existing experiment and quota must not be overwritten");
    const book = options.book ?? DEFAULT_BOOK;
    const workspace = path.resolve("tmp", `bsr7-${path.basename(dir)}`, ".understand-book", path.basename(book));
    if (existsSync(workspace)) throw Error("Experiment workspace already exists");
    mkdirSync(dir, { recursive: true }); mkdirSync(workspace, { recursive: true });
    for (const name of SOURCE_FILES) cpSync(path.join(book, name), path.join(workspace, name));
    cpSync(path.join(book, "book_structure.json"), file("old-book_structure.json"));
    const target = resolveAutomaticBuildTarget(workspace);
    const mode = options.mode ?? "semantic_required";
    const config = options.config ? path.resolve(options.config) : undefined;
    const legacy = prepareExplicitLegacyBuildPlan(workspace, target.root_dir, { pass2: "disabled" }).plan;
    const retrieval = configureBuildRetrieval(mode, { max_documents: 10000, max_queries: 800, max_calls: 2400 }, config,
      "book_structure_projections_and_queries");
    const plan: FullBookPlan = { version: "book_structure_full_book_experiment.v1", status: "draft", source_workspace: book,
      workspace, execution: "codex_subagent", max_submissions: options.submissions ?? 800, max_minutes: options.minutes ?? 720,
      retrieval_mode: mode, ...(config ? { embedding_config: config } : {}), prepared_at: new Date().toISOString(),
      build_plan: reviseBuildRetrieval(legacy, retrieval) };
    save(file("plan.json"), plan); save(file("session.json"), { steps: [] });
    save(file("review.json"), { ...plan, material: SOURCE_FILES, old_structure: file("old-book_structure.json"),
      generation: "Independent Codex agent; only next prompt/input delivered; no external generation API",
      usage: "Actual Codex model calls and tokens unknown; serialized estimates and submissions recorded separately",
      embedding: mode === "semantic_required" ? "Installed local MiniLM; actual tokenizer usage in Core retrieval ledger" : "none" });
    return { status: "prepared", workspace, retrieval_mode: mode };
  }
  const plan = read<FullBookPlan>(file("plan.json"));
  const sessionName = options.unit ? `session.discovery-${options.unit}.json` : "session.json";
  if (options.unit && !plan.discovery_units?.some(u => u.unit_lid === options.unit)) throw Error("Unregistered discovery unit");
  const session = existsSync(file(sessionName)) ? read<Session>(file(sessionName)) : { steps: [] } as Session;
  if (command === "confirm") {
    if (plan.status !== "draft" || !argument?.trim()) throw Error("Draft plan and current authorization text required");
    plan.status = "confirmed"; plan.confirmed_at = new Date().toISOString(); plan.authorization = argument;
    plan.build_plan = transitionBuildPlan(plan.build_plan, "confirmed", { at: plan.confirmed_at, confirmation_source: "codex_conversation" });
    save(file("plan.json"), plan); return { status: "confirmed" };
  }
  const target = resolveAutomaticBuildTarget(plan.workspace);
  const state = () => readAutomaticBuildTaskStage(target, { stage: "book_structure", work_unit_id: "full-book-experiment", ...(options.unit ? { parent_lid: options.unit } : {}) }, "full",
    { retrieval: plan.build_plan.retrieval!.selection })!;
  const budgetPlan = plan.budget_parent_dir ? read<FullBookPlan>(path.join(plan.budget_parent_dir, "plan.json")) : plan;
  const budgetDir = plan.budget_parent_dir ?? dir;
  const budgetSession = read<Session>(path.join(budgetDir, "session.json"));
  const discoverySteps = (p: FullBookPlan, d: string) => (p.discovery_units ?? []).flatMap(u => {
    const name = path.join(d, `session.discovery-${u.unit_lid}.json`);
    return existsSync(name) ? read<Session>(name).steps : [];
  });
  const sharedSubmissions = () => read<Session>(path.join(budgetDir, "session.json")).steps.length + discoverySteps(budgetPlan, budgetDir).length + (budgetPlan.comparison_dir && existsSync(path.join(budgetPlan.comparison_dir, "session.json"))
    ? read<Session>(path.join(budgetPlan.comparison_dir, "session.json")).steps.length : 0);
  const assertTime = () => { if (budgetSession.started_at && Date.now() - Date.parse(budgetSession.started_at) > budgetPlan.max_minutes * 60000)
    throw Error("Experiment time quota exhausted; accepted results retained"); };
  const assertConfirmed = () => { if (plan.status !== "confirmed") throw Error("Confirm this experiment's execution method and quota first"); };
  const revisionEvent = (event: ChapterRevisionEvent) => {
    const name = file("chapter-revision-events.json");
    const events = existsSync(name) ? read<ChapterRevisionEvent[]>(name) : [];
    save(name, [...events, event]);
  };
  const selectionEvent = (event: ChapterSelectionEvent) => {
    const name = file("chapter-selection-events.json");
    const events = existsSync(name) ? read<ChapterSelectionEvent[]>(name) : [];
    save(name, [...events, event]);
  };
  const themeRevisionEvent = (event: ThemeRevisionEvent) => {
    const name = file("theme-revision-events.json");
    const events = existsSync(name) ? read<ThemeRevisionEvent[]>(name) : [];
    save(name, [...events, event]);
  };
  const unresolvedFailures = () => session.steps.filter(step =>
    step.ordinal > (session.failure_recoveries?.at(-1)?.after_ordinal ?? 0));
  const stoppedAfterFailures = () => unresolvedFailures().slice(-3).length === 3
    && unresolvedFailures().slice(-3).every(step => step.status === "rejected");
  if (command === "resume-after-fix") {
    assertConfirmed(); assertTime();
    if (!argument?.trim()) throw Error("A concrete repair note is required before resuming");
    if (session.steps.some(step => step.status === "pending") || !stoppedAfterFailures())
      throw Error("Resume only an idle session stopped after three consecutive rejections");
    const recovery = { after_ordinal: session.steps.length, reason: argument.trim(), at: new Date().toISOString() };
    session.failure_recoveries = [...(session.failure_recoveries ?? []), recovery];
    save(file(sessionName), session);
    return { status: "resumed", ...recovery };
  }
  if (command === "request-chapter-revision") {
    assertConfirmed(); assertTime();
    if (options.unit || plan.budget_parent_dir || !argument) throw Error("Request a chapter revision from the primary session with a request JSON file");
    if (session.steps.some(s => s.kind.startsWith("structure_theme"))) throw Error("Request chapter revisions before shared theme planning");
    const value = read<StructureChapterRevisionRequest>(argument), s = state();
    const chapterTasks = Object.values(s.generation_tasks ?? {}).flatMap(g => g.kind === "book_structure"
      && "phase" in g.task.input && g.task.input.phase === "chapter" && g.task.input.work.unit_lid === value.unit_lid ? [g.task] : []);
    const accepted = chapterTasks.find(task => {
      const artifact = readBookStructureGenerationArtifact(target, task);
      return artifact && (artifact.payload as { accepted_action?: { kind?: string } }).accepted_action?.kind === "select";
    });
    if (!accepted || !("phase" in accepted.input) || accepted.input.phase !== "chapter") throw Error("Chapter revision requires an accepted initial chapter selection");
    const chapterContext = accepted.input.context;
    if (value.candidate_refs.some(ref => !chapterContext.catalog.candidates.some(c => c.ref === ref && c.unit_lid === value.unit_lid)))
      throw Error("Chapter revision locator must belong to its chapter");
    const request = requestStructureChapterRevision(plan.workspace, value);
    revisionEvent({ kind: "requested", revision_id: request.id, request, at: new Date().toISOString(), ordinal: session.steps.length });
    return { status: "requested", request, pending_ordinal: session.steps.find(s => s.status === "pending")?.ordinal ?? null };
  }
  if (command === "request-chapter-selection") {
    assertConfirmed(); assertTime();
    if (options.unit || plan.budget_parent_dir || !argument) throw Error("Request chapter selection delivery from the primary session with a request JSON file");
    if (session.steps.some(step => step.kind.startsWith("structure_theme"))) throw Error("Request chapter selection delivery before shared theme planning");
    const value = read<StructureChapterSelectionContinuation>(argument), s = state();
    const task = s.pending_tasks.flatMap(id => {
      const generation = s.generation_tasks?.[id];
      return generation?.kind === "book_structure" && "phase" in generation.task.input
        && generation.task.input.phase === "chapter" && !generation.task.input.work.revision
        && generation.task.input.work.unit_lid === value.unit_lid ? [generation.task] : [];
    })[0];
    const prefix = `structure:chapter:${value.unit_lid}:`;
    if (!task || !task.descriptor.work_unit_id.startsWith(prefix)) throw Error("Selection continuation requires the current unfinished initial chapter");
    const actionOrdinal = Number(task.descriptor.work_unit_id.slice(prefix.length).split("-")[0]);
    const pending = session.steps.find(step => step.status === "pending");
    const expectedOrdinal = actionOrdinal + (pending?.task_file && read<BookStructureGenerationTaskV1>(pending.task_file).descriptor.work_unit_id === task.descriptor.work_unit_id ? 1 : 0);
    if (value.from_action_ordinal !== expectedOrdinal) throw Error("Selection continuation must start after the accepted actions and any existing pending delivery");
    const request = requestStructureChapterSelectionContinuation(plan.workspace, value);
    selectionEvent({ kind: "requested", continuation_id: request.id, request, at: new Date().toISOString(), ordinal: session.steps.length });
    return { status: "requested", request, pending_ordinal: pending?.ordinal ?? null };
  }
  if (command === "request-theme-revision") {
    assertConfirmed(); assertTime();
    if (options.unit || !argument) throw Error("Request a theme revision with a request JSON file from its own experiment");
    const value = read<StructureThemeRevisionRequest>(argument);
    const accepted = session.steps.some(step => step.status === "accepted" && step.kind === "structure_theme"
      && step.response_file && (() => {
        const action = read<{ kind?: string; result?: { ref?: string } }>(step.response_file!);
        return action.kind === "resolve" && action.result?.ref === value.theme_ref;
      })());
    if (!accepted) throw Error("Theme revision requires an accepted theme result in this experiment");
    const request = requestStructureThemeRevision(plan.workspace, value);
    themeRevisionEvent({ kind: "requested", revision_id: request.id, request, at: new Date().toISOString(), ordinal: session.steps.length });
    return { status: "requested", request, pending_ordinal: session.steps.find(step => step.status === "pending")?.ordinal ?? null };
  }
  if (command === "status") {
    const own = [...session.steps, ...discoverySteps(plan, dir)];
    const count = (steps: Step[]) => ({ submissions: steps.length, accepted: steps.filter(s => s.status === "accepted").length,
      rejected: steps.filter(s => s.status === "rejected").length, pending: steps.filter(s => s.status === "pending").length });
    return { status: plan.status, mode: plan.retrieval_mode, ...count(own), shared_submissions: sharedSubmissions(),
      max_submissions: budgetPlan.max_submissions, started_at: budgetSession.started_at,
      deadline: budgetSession.started_at ? new Date(Date.parse(budgetSession.started_at) + budgetPlan.max_minutes * 60000).toISOString() : null,
      discovery_units: (plan.discovery_units ?? []).map(u => {
        const name = file(`session.discovery-${u.unit_lid}.json`);
        return { unit_lid: u.unit_lid, ...count(existsSync(name) ? read<Session>(name).steps : []), expected_accepted: u.max_submissions - 3 };
      }) };
  }
  if (command === "discovery-units") {
    assertConfirmed(); assertTime();
    if (options.unit || plan.budget_parent_dir || session.steps.some(s => s.status === "pending")) throw Error("Register units from the idle primary session");
    const s = state();
    const pending = s.pending_tasks.map(id => s.generation_tasks?.[id]).filter(g => g?.kind === "book_structure");
    if (pending.some(g => g!.kind === "book_structure" && g!.task.descriptor.kind !== "structure_fragment")) throw Error("Parallel discovery requires accepted global outline and remaining fragments only");
    const counts = new Map<string, number>();
    for (const g of pending) if (g?.kind === "book_structure") counts.set(g.task.parent_unit_lid, (counts.get(g.task.parent_unit_lid) ?? 0) + 1);
    if (!plan.discovery_units) {
      plan.discovery_units = [...counts].map(([unit_lid, count]) => ({ unit_lid, max_submissions: count + 3 }));
      if (session.steps.length + plan.discovery_units.reduce((n, u) => n + u.max_submissions, 0) >= plan.max_submissions)
        throw Error("Discovery lane allowances leave no organization quota");
      save(file("plan.json"), plan);
    }
    return { status: "registered", units: plan.discovery_units.map(u => ({ ...u, pending: counts.get(u.unit_lid) ?? 0 })),
      shared_started_at: budgetSession.started_at, shared_max_submissions: plan.max_submissions, shared_max_minutes: plan.max_minutes };
  }
  if (command === "fork-lexical") {
    assertConfirmed(); assertTime();
    if (plan.retrieval_mode !== "semantic_required" || plan.budget_parent_dir || plan.comparison_dir
      || session.steps.at(-1)?.kind !== "structure_theme_plan" || session.steps.at(-1)?.status !== "accepted")
      throw Error("Fork once after shared theme planning, before theme generation");
    const comparisonDir = file("lexical"), workspace = path.join(path.dirname(path.dirname(plan.workspace)), "lexical-workspace", ".understand-book", path.basename(plan.workspace));
    if (existsSync(comparisonDir) || existsSync(workspace)) throw Error("Comparison already exists; retain it");
    mkdirSync(comparisonDir, { recursive: true }); mkdirSync(workspace, { recursive: true });
    for (const name of SOURCE_FILES) cpSync(path.join(plan.workspace, name), path.join(workspace, name));
    cpSync(path.join(plan.workspace, ".build"), path.join(workspace, ".build"), { recursive: true });
    const now = new Date().toISOString();
    const build_plan = transitionBuildPlan(reviseBuildRetrieval(plan.build_plan, configureBuildRetrieval("lexical_only",
      { max_documents: 0, max_queries: 0, max_calls: 0 }, undefined, "book_structure_projections_and_queries")), "confirmed",
      { at: now, confirmation_source: "codex_conversation" });
    const comparison: FullBookPlan = { ...plan, workspace, retrieval_mode: "lexical_only", build_plan,
      prepared_at: now, confirmed_at: now, budget_parent_dir: dir };
    delete comparison.embedding_config;
    delete comparison.discovery_units;
    save(path.join(comparisonDir, "plan.json"), comparison); save(path.join(comparisonDir, "session.json"), { steps: [] });
    save(path.join(comparisonDir, "shared-inputs.json"), { source: dir, accepted_through_ordinal: session.steps.at(-1)!.ordinal,
      constraint: "Same source, accepted candidates, chapter selections, theme plan, prompts and generation budgets; only retrieval changes. All submissions/time share parent quota." });
    plan.comparison_dir = comparisonDir; save(file("plan.json"), plan);
    return { status: "forked", comparison_dir: comparisonDir, workspace };
  }
  if (command === "next") {
    assertConfirmed(); assertTime();
    const pending = session.steps.find(s => s.status === "pending"); if (pending) return pending;
    let s = state();
    if (s.structure_blocked) throw Error(s.structure_blocked);
    const pendingFragments = s.pending_tasks.filter(id => {
      const g = s.generation_tasks?.[id];
      return g?.kind === "book_structure" && g.task.descriptor.kind === "structure_fragment";
    });
    if (options.unit && !pendingFragments.length) return { status: "complete_unit", unit_lid: options.unit };
    if (!options.unit && plan.discovery_units && pendingFragments.length)
      throw Error("Finish active discovery lanes before advancing the primary session");
    // Observe completion before the quota gate: the last allowed response can finish the book.
    if (!s.pending_tasks.length && !s.retrieval_preparation) {
      if (options.unit) return { status: "complete_unit", unit_lid: options.unit };
      if (!s.book_structure_materialized) throw Error("No pending work but no complete materialization");
      return { status: "complete", command: `full-book.ts report ${dir}` };
    }
    if (sharedSubmissions() >= budgetPlan.max_submissions) throw Error("Experiment submission quota exhausted; accepted results retained");
    if (options.unit && session.steps.length >= plan.discovery_units!.find(u => u.unit_lid === options.unit)!.max_submissions)
      throw Error("Discovery unit submission allowance exhausted; shared quota retained");
    if (!options.unit && plan.discovery_units && discoverySteps(plan, dir).some(s => s.status === "pending"))
      throw Error("Finish active discovery lanes before advancing the primary session");
    if (stoppedAfterFailures())
      throw Error("Three consecutive rejections; inspect failure before continuing");
    session.started_at ??= budgetSession.started_at ?? new Date().toISOString(); save(file(sessionName), session);
    if (s.retrieval_preparation) {
      const failure = await prepareBuildRetrieval({ workspace: plan.workspace, plan: plan.build_plan,
        runtime: configuredBuildRetrievalRuntime(plan.build_plan, plan.embedding_config), request: s.retrieval_preparation });
      if (failure) throw Error(`${failure.reason}: ${failure.message}`);
      assertTime(); s = state();
    }
    if (s.structure_blocked) throw Error(s.structure_blocked);
    const id = s.pending_tasks[0], generation = s.generation_tasks?.[id];
    if (generation?.kind !== "book_structure") throw Error("No reachable BookStructure task after preparation");
    const task = generation.task, input = renderBookStructureGenerationTaskInput(task);
    const kind = task.descriptor.kind;
    const prompt = kind === "structure_fragment" ? STRUCTURE_DISCOVERY_PROMPT
      : STRUCTURE_ORGANIZATION_PROMPTS[kind as keyof typeof STRUCTURE_ORGANIZATION_PROMPTS];
    if (!prompt) throw Error(`Unsupported full-book task: ${kind}`);
    const ordinal = session.steps.length + 1, stem = `${options.unit ? `discovery-${options.unit}-` : ""}${String(ordinal).padStart(4, "0")}`;
    const step: Step = { ordinal, kind, unit_lid: task.parent_unit_lid,
      task_file: file(`${stem}.task.json`), input_file: file(`${stem}.input.json`), prompt_file: file(`${stem}.prompt.md`),
      status: "pending", opened_at: new Date().toISOString(), input_estimated_tokens: estimateTokens(prompt + input) };
    step.max_candidate_estimated_tokens = task.descriptor.execution_budget_proof.max_candidate_tokens;
    freezeBookStructureGenerationTask(target, task); save(step.task_file, task);
    writeFileSync(step.input_file, input); writeFileSync(step.prompt_file, prompt);
    session.steps.push(step); save(file(sessionName), session);
    if ("phase" in task.input && task.input.phase === "chapter" && task.input.work.revision)
      revisionEvent({ kind: "opened", revision_id: task.input.work.revision.id, ordinal, at: step.opened_at });
    if ("phase" in task.input && task.input.phase === "chapter" && task.input.work.selection_draft)
      selectionEvent({ kind: "opened", continuation_id: task.input.work.selection_draft.continuation.id, ordinal, at: step.opened_at });
    if ("phase" in task.input && task.input.phase === "theme" && task.input.revision_request)
      themeRevisionEvent({ kind: "opened", revision_id: task.input.revision_request.id, ordinal, at: step.opened_at });
    return step;
  }
  if (command === "submit") {
    assertConfirmed();
    const step = session.steps.find(s => s.status === "pending"); if (!step || !argument) throw Error("No pending input or response file");
    const raw = readFileSync(argument, "utf8"), task = read<BookStructureGenerationTaskV1>(step.task_file);
    step.response_file = file(`${options.unit ? `discovery-${options.unit}-` : ""}${String(step.ordinal).padStart(4, "0")}.response.json`); writeFileSync(step.response_file, raw);
    step.output_estimated_tokens = estimateTokens(raw);
    try {
      assertTime();
      if (step.output_estimated_tokens > task.descriptor.execution_budget_proof.max_candidate_tokens) throw Error("Candidate exceeds formal output budget");
      writeBookStructureGenerationCandidate({ target, task, candidate: JSON.parse(raw),
        provenance: { executor: "bsr7-codex-subagent", attempt: step.ordinal, generated_at: new Date().toISOString() } });
      step.status = "accepted";
    } catch (error) { step.status = "rejected"; step.error = String(error); }
    step.finished_at = new Date().toISOString(); save(file(sessionName), session);
    if ("phase" in task.input && task.input.phase === "chapter" && task.input.work.revision)
      revisionEvent({ kind: step.status, revision_id: task.input.work.revision.id, ordinal: step.ordinal, at: step.finished_at });
    if ("phase" in task.input && task.input.phase === "chapter" && task.input.work.selection_draft)
      selectionEvent({ kind: step.status, continuation_id: task.input.work.selection_draft.continuation.id, ordinal: step.ordinal, at: step.finished_at });
    if ("phase" in task.input && task.input.phase === "theme" && task.input.revision_request)
      themeRevisionEvent({ kind: step.status, revision_id: task.input.revision_request.id, ordinal: step.ordinal, at: step.finished_at });
    return { ordinal: step.ordinal, status: step.status, error: step.error };
  }
  if (command === "report" || command === "close") {
    const s = state(), quality = collectAutomaticBuildStageQuality(target, s, "full");
    const tasks = Object.values(s.generation_tasks ?? {}).flatMap(g => g.kind === "book_structure" ? [g.task] : []);
    const accepted = tasks.filter(t => readBookStructureGenerationArtifact(target, t));
    const catalog = readStructureCandidateCatalog(target, accepted.filter(t => t.output_role === "unit_observation" || t.output_role === "unit_artifact"));
    const allSteps = [...session.steps, ...(plan.budget_parent_dir ? [] : discoverySteps(plan, dir))];
    const phaseCost = (filter: (s: Step) => boolean) => {
      const steps = allSteps.filter(filter);
      const finished = steps.flatMap(s => s.finished_at ? [Date.parse(s.finished_at)] : []);
      return { submissions: steps.length, accepted: steps.filter(s => s.status === "accepted").length,
        rejected: steps.filter(s => s.status === "rejected").length, pending: steps.filter(s => s.status === "pending").length,
        input_estimated_tokens: steps.reduce((n, s) => n + s.input_estimated_tokens, 0),
        output_estimated_tokens: steps.reduce((n, s) => n + (s.output_estimated_tokens ?? 0), 0),
        executor_ms: steps.reduce((n, s) => n + (s.finished_at ? Date.parse(s.finished_at) - Date.parse(s.opened_at) : 0), 0),
        wall_ms: finished.length ? Math.max(...finished) - Math.min(...steps.map(s => Date.parse(s.opened_at))) : null };
    };
    const report = { status: s.book_structure_materialized && !s.pending_tasks.length && !s.structure_blocked ? "ready" : "incomplete",
      workspace: plan.workspace, mode: plan.retrieval_mode, quality, candidates: catalog,
      structure: s.book_structure_materialized?.output ?? null, progress: s.book_structure_progress,
      submissions: phaseCost(() => true), discovery: phaseCost(s => s.kind === "structure_outline" || s.kind === "structure_fragment"),
      organization: phaseCost(s => s.kind !== "structure_outline" && s.kind !== "structure_fragment"),
      actual_model_calls: null, actual_input_tokens: null, actual_output_tokens: null, external_generation_api_calls: 0,
      embedding: summarizeRetrievalUsage(readBuildRetrievalState(plan.workspace).calls),
      wall_ms: phaseCost(() => true).wall_ms,
      shared_quota_elapsed_ms: budgetSession.started_at ? Date.now() - Date.parse(budgetSession.started_at) : null };
    save(file("report.json"), report);
    if (command === "report") return { status: report.status, candidates: catalog.candidates.length, submissions: report.submissions, progress: report.progress };
    assertConfirmed();
    if (report.status !== "ready" || quality.gate_status !== "passed") throw Error("Complete and validate all structure work before Engine publication");
    const receipt = runAutomaticBuildCloseStage(plan.workspace, target.root_dir, "book_structure", { quality_profile: "full" });
    save(file("publication.json"), receipt);
    if (!("status" in receipt) || receipt.status !== "closed" || !state().closed) throw Error("Engine did not confirm the BookStructure publication");
    return { status: "published", workspace: plan.workspace, receipt };
  }
  throw Error("Use prepare|confirm|status|discovery-units|request-chapter-revision|request-chapter-selection|request-theme-revision|resume-after-fix|next|submit|report|fork-lexical|close OUTPUT_DIR [RESPONSE_FILE|AUTHORIZATION|REPAIR_NOTE] [DISCOVERY_UNIT]");
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [command, output, argument, unit] = process.argv.slice(2);
  console.log(JSON.stringify(await fullBookExperiment(command, output, argument,
    { config: process.env.UNDERSTAND_BOOK_EMBEDDING_CONFIG, unit }), null, 2));
}
