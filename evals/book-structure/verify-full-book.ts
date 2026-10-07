import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { readAutomaticBuildTaskStage, resolveAutomaticBuildTarget } from "../../packages/core/src/build-orchestrator";
import { freezeBookStructureGenerationTask, readStructureCandidateCatalog, renderBookStructureGenerationTaskInput,
  writeBookStructureGenerationCandidate, type BookStructureGenerationTaskV1 } from "../../packages/core/src/book-structure-generation";
import { STRUCTURE_DISCOVERY_PROMPT } from "../../packages/core/src/book-structure-discovery";
import { STRUCTURE_ORGANIZATION_PROMPTS } from "../../packages/core/src/book-structure-organization-prompts";
import { collectAutomaticBuildStageQuality } from "../../packages/core/src/automatic-build-quality";
import type { FullBookPlan, ChapterRevisionEvent, ChapterSelectionEvent, ThemeRevisionEvent } from "./full-book";
import { requestStructureChapterRevision, requestStructureChapterSelectionContinuation, requestStructureThemeRevision } from "../../packages/core/src/book-structure-organization";

const read = <T>(name: string): T => JSON.parse(readFileSync(name, "utf8"));
interface SavedStep { ordinal: number; kind: string; unit_lid: string; status: string; task_file: string;
  input_file: string; prompt_file: string; response_file?: string; finished_at?: string }
function acceptedSteps(dir: string, plan: FullBookPlan) {
  return ["session.json", ...(plan.discovery_units ?? []).map(u => `session.discovery-${u.unit_lid}.json`)]
    .flatMap(name => existsSync(path.join(dir, name)) ? read<{ steps: SavedStep[] }>(path.join(dir, name)).steps : [])
    .filter(s => s.status === "accepted").sort((a, b) => a.finished_at!.localeCompare(b.finished_at!));
}

/** Recompute deliveries and semantic acceptance from saved responses. No model or Provider is opened. */
export function verifyFullBook(output: string) {
  const dir = path.resolve(output), plan = read<FullBookPlan>(path.join(dir, "plan.json"));
  const expected = read<{ status: string; candidates: unknown; structure: unknown }>(path.join(dir, "report.json"));
  if (expected.status !== "ready") throw Error("Complete full-book report required for replay");
  const replay = path.join(dir, "replay"), workspace = path.join(replay, ".understand-book", path.basename(plan.workspace));
  if (existsSync(replay)) throw Error("Replay record already exists; retain it");
  mkdirSync(workspace, { recursive: true });
  for (const name of ["source.txt", "source_manifest.json", "profile_metadata.json", "base.json", "discourse_index.json", "formula_semantics.json"])
    cpSync(path.join(plan.workspace, name), path.join(workspace, name));
  // Exact historical query preparations have their own Core slots. Reuse them as recorded ranking evidence;
  // recompute current request/catalog dependencies, never call a Provider or copy generation artifacts.
  const retrieval = path.join(plan.workspace, ".build", "teaching", "retrieval.json");
  if (existsSync(retrieval)) {
    mkdirSync(path.join(workspace, ".build", "teaching"), { recursive: true });
    cpSync(retrieval, path.join(workspace, ".build", "teaching", "retrieval.json"));
  }
  const target = resolveAutomaticBuildTarget(workspace);
  const state = (unit?: string) => readAutomaticBuildTaskStage(target, { stage: "book_structure", work_unit_id: "full-book-replay",
    ...(unit ? { parent_lid: unit } : {}) }, "full", { retrieval: plan.build_plan.retrieval!.selection })!;
  let shared: SavedStep[] = [];
  if (plan.budget_parent_dir) {
    const parent = read<FullBookPlan>(path.join(plan.budget_parent_dir, "plan.json"));
    const limit = read<{ accepted_through_ordinal: number }>(path.join(dir, "shared-inputs.json")).accepted_through_ordinal;
    shared = acceptedSteps(plan.budget_parent_dir, parent).filter(s => s.kind === "structure_fragment" || s.kind === "structure_outline"
      || (path.dirname(s.task_file) === plan.budget_parent_dir && s.ordinal <= limit));
  }
  const own = acceptedSteps(dir, plan);
  const revisionEvents = [plan.budget_parent_dir, dir].filter((d): d is string => Boolean(d))
    .flatMap(d => existsSync(path.join(d, "chapter-revision-events.json")) ? read<ChapterRevisionEvent[]>(path.join(d, "chapter-revision-events.json")) : []);
  const restoredRevisions = new Set<string>();
  const selectionEvents = [plan.budget_parent_dir, dir].filter((d): d is string => Boolean(d))
    .flatMap(d => existsSync(path.join(d, "chapter-selection-events.json")) ? read<ChapterSelectionEvent[]>(path.join(d, "chapter-selection-events.json")) : []);
  const restoredSelections = new Set<string>();
  const themeRevisionEvents = [plan.budget_parent_dir, dir].filter((d): d is string => Boolean(d))
    .flatMap(d => existsSync(path.join(d, "theme-revision-events.json")) ? read<ThemeRevisionEvent[]>(path.join(d, "theme-revision-events.json")) : []);
  const restoredThemeRevisions = new Set<string>();
  for (const [index, step] of [...shared, ...own].entries()) {
    const saved = read<BookStructureGenerationTaskV1>(step.task_file);
    const revision = "phase" in saved.input && saved.input.phase === "chapter" ? saved.input.work.revision : undefined;
    if (revision && !restoredRevisions.has(revision.id)) {
      const request = revisionEvents.find(e => e.kind === "requested" && e.revision_id === revision.id)?.request;
      if (!request) throw Error(`Missing recorded chapter revision request for ${revision.id}`);
      requestStructureChapterRevision(workspace, request); restoredRevisions.add(revision.id);
    }
    const continuation = "phase" in saved.input && saved.input.phase === "chapter" ? saved.input.work.selection_draft?.continuation : undefined;
    if (continuation && !restoredSelections.has(continuation.id)) {
      const request = selectionEvents.find(event => event.kind === "requested" && event.continuation_id === continuation.id)?.request;
      if (!request) throw Error(`Missing recorded chapter selection request for ${continuation.id}`);
      requestStructureChapterSelectionContinuation(workspace, request); restoredSelections.add(continuation.id);
    }
    const themeRevision = "phase" in saved.input && saved.input.phase === "theme" ? saved.input.revision_request : undefined;
    if (themeRevision && !restoredThemeRevisions.has(themeRevision.id)) {
      const request = themeRevisionEvents.find(event => event.kind === "requested" && event.revision_id === themeRevision.id)?.request;
      if (!request) throw Error(`Missing recorded theme revision request for ${themeRevision.id}`);
      requestStructureThemeRevision(workspace, request); restoredThemeRevisions.add(themeRevision.id);
    }
    const s = state(step.kind === "structure_fragment" ? step.unit_lid : undefined);
    if (s.structure_blocked || s.retrieval_preparation) throw Error(`Unreplayable current dependencies at ${step.task_file}: ${s.structure_blocked ?? "missing recorded retrieval"}`);
    const g = s.generation_tasks?.[saved.descriptor.work_unit_id];
    if (g?.kind !== "book_structure" || !s.pending_tasks.includes(saved.descriptor.work_unit_id)) throw Error(`Saved task not pending at replay step ${index + 1}`);
    const task = g.task;
    const prompt = step.kind === "structure_fragment" ? STRUCTURE_DISCOVERY_PROMPT
      : STRUCTURE_ORGANIZATION_PROMPTS[step.kind as keyof typeof STRUCTURE_ORGANIZATION_PROMPTS];
    // Shared lexical/semantic planning deliveries contain no retrieval difference. Actual theme deliveries
    // are verified under their own mode against the preserved Core preparations.
    if (renderBookStructureGenerationTaskInput(task) !== readFileSync(step.input_file, "utf8")
      || prompt !== readFileSync(step.prompt_file, "utf8")) throw Error(`Replay delivery changed at ${step.task_file}`);
    freezeBookStructureGenerationTask(target, task);
    writeBookStructureGenerationCandidate({ target, task, candidate: read(step.response_file!),
      provenance: { executor: "bsr7-saved-response-replay", attempt: index + 1, generated_at: new Date().toISOString() } });
  }
  const s = state(), quality = collectAutomaticBuildStageQuality(target, s, "full");
  if (s.pending_tasks.length || !s.book_structure_materialized || quality.gate_status !== "passed") throw Error("Replay not fully materialized and validated");
  const tasks = Object.values(s.generation_tasks ?? {}).flatMap(g => g.kind === "book_structure"
    && (g.task.output_role === "unit_observation" || g.task.output_role === "unit_artifact") ? [g.task] : []);
  const catalog = readStructureCandidateCatalog(target, tasks);
  if (!isDeepStrictEqual(catalog, expected.candidates) || !isDeepStrictEqual(s.book_structure_materialized.output, expected.structure))
    throw Error("Replay candidates or full structure differ from saved report");
  const result = { status: "passed", workspace, accepted_steps: own.length, shared_steps: shared.length,
    same_inputs: true, same_candidates: true, same_structure: true, quality, model_calls: 0, provider_calls: 0,
    retrieval_basis: "Preserved Core query preparations; current requests and complete catalog dependencies matched" };
  writeFileSync(path.join(dir, "verification.json"), JSON.stringify(result, null, 2) + "\n");
  return result;
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) console.log(JSON.stringify(verifyFullBook(process.argv[2]), null, 2));
