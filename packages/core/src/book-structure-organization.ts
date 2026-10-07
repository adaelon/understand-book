import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { z } from "zod";
import type { BookStructureCandidate, BookStructureExecutionContractV2 } from "./book-structure";
import { evaluateBookStructureExecution, proofBoundBookStructureDescriptor } from "./book-structure";
import type { BookStructureReferenceScope } from "./book-structure-evidence";
import type { AutomaticBuildTarget } from "./build-orchestrator";
import type { BuildExecutionProfileV1 } from "./build-execution-profile";
import type { BuildRetrievalSelection } from "./build-intent";
import { retrievalScopeIncludes } from "./build-intent";
import { automaticBuildGenerationArtifactPath } from "./semantic-artifact";
import { createBookStructureGenerationTask, readBookStructureGenerationArtifact, type BookStructureGenerationTaskV1 } from "./book-structure-generation";
import { acceptStructureOutline, newStructureChapterWork, structureChapterInput, applyStructureChapterAction,
  reviseStructureChapterWork, structureChapterRevisionRequestSchema, type StructureChapterRevisionRequest,
  continueStructureChapterSelection, structureChapterSelectionContinuationSchema, type StructureChapterSelectionContinuation,
  type StructureOutlineInput, type StructureOutline, type StructureChapterWork, type StructureChapterContext } from "./book-structure-planning";
import { planStructureThemes, structureThemePlanningInput, structureThemeInput, applyStructureThemeAction,
  reconcileStructureThemes, materializeStructureThemes, reopenStructureTheme, type StructureThemeDirectory, type StructureThemeContext } from "./book-structure-themes";
import { materializeStructureChapterSelections } from "./book-structure-materialization";
import { projectStructureRetrieval, STRUCTURE_RETRIEVAL, STRUCTURE_RETRIEVAL_POLICY } from "./book-structure-retrieval";
import { retrievalDependencies, retrievalPreparationMatches } from "./semantic-retrieval-preparation";
import { readBuildRetrievalState, retrievalRemainingWork, type TeachingRetrievalRequest } from "./automatic-build-retrieval";
import { createCandidateTransportContract } from "./executor-transport";
import { STRUCTURE_ORGANIZATION_PROMPTS } from "./book-structure-organization-prompts";

export type StructureOrganizationKind = "structure_outline" | "structure_chapter" | "structure_chapter_selection" | "structure_theme_plan" | "structure_theme" | "structure_theme_reconcile";
export type StructureOrganizationState = StructureOutline | StructureChapterWork | StructureThemeDirectory;
/** Only body is delivered. context/work are the current Core ledger, never a model-authored snapshot. */
export type StructureOrganizationInput = {
  version: "book_structure_organization_input.v1";
  body: unknown;
  retrieval_selection?: BuildRetrievalSelection;
  reference_scope: BookStructureReferenceScope;
} & (
  | { phase: "outline"; context: StructureOutlineInput }
  | { phase: "chapter"; context: StructureChapterContext; work: StructureChapterWork }
  | { phase: "plan"; context: StructureThemeContext }
  | { phase: "theme"; context: StructureThemeContext; directory: StructureThemeDirectory; ref: string; revision_request?: StructureThemeRevisionRequest }
  | { phase: "reconcile"; context: StructureThemeContext; directory: StructureThemeDirectory }
);
export interface StructureOrganizationAcceptance { version: "book_structure_action.v1"; accepted_action: unknown }
export function renderStructureOrganizationInput(input: StructureOrganizationInput): string {
  const indent = input.phase === "plan" && input.context.planning_candidate_refs ? undefined : 2;
  return JSON.stringify({ phase: input.phase, input: input.body, reference_scope: input.reference_scope }, null, indent) + "\n";
}
export function applyStructureOrganizationAction(input: StructureOrganizationInput, action: unknown): StructureOrganizationState {
  switch (input.phase) {
    case "outline": return acceptStructureOutline(action, input.context);
    case "chapter": return applyStructureChapterAction(input.work, action, input.context);
    case "plan": return planStructureThemes(action, input.context);
    case "theme": return applyStructureThemeAction(input.directory, input.ref, action, input.context);
    case "reconcile": return reconcileStructureThemes(input.directory, action, input.context);
  }
}
export function acceptStructureOrganizationAction(input: StructureOrganizationInput, action: unknown): StructureOrganizationAcceptance {
  applyStructureOrganizationAction(input, action);
  return { version: "book_structure_action.v1", accepted_action: structuredClone(action) };
}
export function structureOrganizationContracts(base: BookStructureExecutionContractV2) {
  return Object.fromEntries(Object.entries(STRUCTURE_ORGANIZATION_PROMPTS).map(([kind, semantic_prompt]) => [kind, {
    semantic_prompt, policy_fingerprint: { ...base.policy_fingerprint,
      stage_policy_version: `${kind}.v1`, schema_version: `${kind}.v1`,
      prompt_sha256: createHash("sha256").update(semantic_prompt).digest("hex") },
  }])) as Record<StructureOrganizationKind, Pick<BookStructureExecutionContractV2, "semantic_prompt" | "policy_fingerprint">>;
}
export const structureOrganizationGeneration = (kind: StructureOrganizationKind, quality: string) => `${kind.replaceAll("_", "-")}.${quality}.${kind === "structure_chapter" || kind.startsWith("structure_theme") ? "v2" : "v1"}`;
export class StructureOrganizationBlocked extends Error {}

const structureThemeRevisionRequestSchema = z.object({
  id: z.string().min(1).max(120).regex(/^[A-Za-z0-9_-]+$/),
  theme_ref: z.string().min(1), issue: z.string().trim().min(1).max(4000),
}).strict();
export type StructureThemeRevisionRequest = z.infer<typeof structureThemeRevisionRequestSchema>;
const themeRevisionPath = (workspace: string) => path.join(workspace, ".build", "automatic-build", "book-structure-theme-revisions.json");
export function readStructureThemeRevisionRequests(workspace: string): StructureThemeRevisionRequest[] {
  const file = themeRevisionPath(workspace);
  return existsSync(file) ? (JSON.parse(readFileSync(file, "utf8")) as unknown[]).map(value => structureThemeRevisionRequestSchema.parse(value)) : [];
}
/** Reopen one accepted theme after the original directory pass; its writer remains the acceptance gate. */
export function requestStructureThemeRevision(workspace: string, value: unknown): StructureThemeRevisionRequest {
  const request = structureThemeRevisionRequestSchema.parse(value), requests = readStructureThemeRevisionRequests(workspace);
  if (requests.some(r => r.id === request.id)) throw new Error("theme revision id already requested");
  const file = themeRevisionPath(workspace);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file + ".tmp", JSON.stringify([...requests, request], null, 2) + "\n"); renameSync(file + ".tmp", file);
  return request;
}

const chapterRevisionPath = (workspace: string) => path.join(workspace, ".build", "automatic-build", "book-structure-chapter-revisions.json");
export function readStructureChapterRevisionRequests(workspace: string): StructureChapterRevisionRequest[] {
  const file = chapterRevisionPath(workspace);
  return existsSync(file) ? (JSON.parse(readFileSync(file, "utf8")) as unknown[]).map(value => structureChapterRevisionRequestSchema.parse(value)) : [];
}
/** Requests are editorial inputs, not semantic answers; acceptance still goes through the chapter writer. */
export function requestStructureChapterRevision(workspace: string, value: unknown): StructureChapterRevisionRequest {
  const request = structureChapterRevisionRequestSchema.parse(value), requests = readStructureChapterRevisionRequests(workspace);
  if (requests.some(r => r.id === request.id)) throw new Error("chapter revision id already requested");
  const file = chapterRevisionPath(workspace);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file + ".tmp", JSON.stringify([...requests, request], null, 2) + "\n"); renameSync(file + ".tmp", file);
  return request;
}
const chapterSelectionContinuationPath = (workspace: string) => path.join(workspace, ".build", "automatic-build", "book-structure-chapter-selection-continuations.json");
export function readStructureChapterSelectionContinuations(workspace: string): StructureChapterSelectionContinuation[] {
  const file = chapterSelectionContinuationPath(workspace);
  return existsSync(file) ? (JSON.parse(readFileSync(file, "utf8")) as unknown[]).map(value => structureChapterSelectionContinuationSchema.parse(value)) : [];
}
export function requestStructureChapterSelectionContinuation(workspace: string, value: unknown): StructureChapterSelectionContinuation {
  const request = structureChapterSelectionContinuationSchema.parse(value), requests = readStructureChapterSelectionContinuations(workspace);
  if (requests.some(r => r.id === request.id || r.unit_lid === request.unit_lid)) throw new Error("chapter selection continuation already requested");
  const file = chapterSelectionContinuationPath(workspace);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file + ".tmp", JSON.stringify([...requests, request], null, 2) + "\n"); renameSync(file + ".tmp", file);
  return request;
}

export function routeStructureOrganization(input: {
  target: AutomaticBuildTarget; execution_profile: BuildExecutionProfileV1;
  contracts: ReturnType<typeof structureOrganizationContracts>;
  outline: StructureOutlineInput; context: Omit<StructureChapterContext, "outline">;
  retrieval?: BuildRetrievalSelection;
  stop_after_outline?: boolean;
  chapter_overviews?: StructureOutlineInput["chapters"];
}) {
  const state = readBuildRetrievalState(input.target.workspace_dir);
  const selection = input.retrieval ?? state.selection;
  const tasks: BookStructureGenerationTaskV1[] = [], pending: string[] = [];
  const progress = { candidates: input.context.catalog.candidates.length, outline: { done: 0, total: 1 },
    chapters: { done: 0, total: input.outline.chapters.length }, themes: { done: 0, total: 0, planned: false },
    reconciliation: "pending" as "pending" | "complete" | "unresolved" };
  let candidate: BookStructureCandidate | undefined;
  let preparation: TeachingRetrievalRequest | undefined;
  let remaining: ReturnType<typeof retrievalRemainingWork> | undefined;
  let blocked: string | undefined;
  const scope: BookStructureReferenceScope = { unit_lids: input.outline.chapters.map(c => c.unit_lid), dependency_target_lids: [], evidence_by_unit: {} };
  // Descriptors bind the actual rendered input. Core dependencies live in the current
  // context and are rechecked by applying the saved action, including unshown candidates.
  const step = (kind: StructureOrganizationKind, id: string, packet: StructureOrganizationInput) => {
    const rendered = renderStructureOrganizationInput(packet), contract = input.contracts[kind];
    const reserve = kind === "structure_outline" ? 5000 : kind === "structure_chapter" || kind === "structure_chapter_selection" ? 3500 : 6500;
    const evaluated = evaluateBookStructureExecution({ contract, rendered_input: rendered,
      transport_profile: input.execution_profile.transport_profile,
      budget: { stage_body_limit_tokens: 20000, executor_context_floor_tokens: 32768,
        output_reserve_tokens: reserve, max_candidate_tokens: Math.min(reserve, createCandidateTransportContract(input.execution_profile.transport_profile).candidate_value_max_estimated_tokens), safety_margin_tokens: 512 } });
    if (evaluated.status !== "within_limit") throw new StructureOrganizationBlocked(`BookStructure ${id} input exceeds its execution budget; reduce the requested material before resuming`);
    const descriptor = proofBoundBookStructureDescriptor({ target: input.target.target_ref,
      work_unit_id: `${id}-${evaluated.proof.rendered_input_sha256.slice(0, 16)}`, kind, rendered_input: rendered, proof: evaluated.proof,
      policy_fingerprint: contract.policy_fingerprint, input_basis: { kind: "semantic_projection", projection_kind: "book_structure",
        source_fingerprint: input.target.target_ref.input_fingerprint, projection_sha256: evaluated.proof.rendered_input_sha256, parent_lids: ["stitch"] },
      evidence_lids: ["stitch"], transport_profile: input.execution_profile.transport_profile });
    const task = createBookStructureGenerationTask({ execution_profile: input.execution_profile, target_ref: input.target.target_ref,
      descriptor, generation_input: packet, parent_unit_lid: "stitch", parent_content_hash: descriptor.input_hash,
      source_range: { start_ordinal: 0, end_ordinal_exclusive: input.outline.chapters.length }, allowed_evidence_lids: descriptor.evidence_lids,
      policy_generation_id: structureOrganizationGeneration(kind, contract.policy_fingerprint.quality_profile), output_role: "organization_action" });
    let result: StructureOrganizationState | undefined;
    for (;;) {
      try {
        const artifact = readBookStructureGenerationArtifact(input.target, task);
        if (artifact) result = applyStructureOrganizationAction(packet, (artifact.payload as StructureOrganizationAcceptance).accepted_action);
      } catch { /* The old receipt remains; a failed current gate needs a replacement action. */ }
      if (result || !existsSync(automaticBuildGenerationArtifactPath(input.target, "book_structure", task.policy_generation_id, descriptor.work_unit_id))) break;
      descriptor.work_unit_id += "-replay";
    }
    tasks.push(task);
    if (!result) pending.push(descriptor.work_unit_id);
    return result;
  };
  const packet = { version: "book_structure_organization_input.v1" as const, reference_scope: scope, retrieval_selection: selection };
  try {
    const outline = step("structure_outline", "structure:outline", { ...packet, phase: "outline", context: input.outline, body: input.outline }) as StructureOutline | undefined;
    if (!outline) return { tasks, pending, progress };
    progress.outline.done = 1;
    if (input.stop_after_outline) return { tasks, pending, progress, outline_result: outline };
    const context: StructureChapterContext = { ...input.context, outline, source_index_page_size: 64, candidate_index_page_size: 64 };
    const selected: import("./book-structure-candidates").AcceptedStructureChapterSelection[] = [];
    const acceptedWork = new Map<string, { work: StructureChapterWork; inspected_refs: string[] }>();
    const continuations = readStructureChapterSelectionContinuations(input.target.workspace_dir);
    for (const chapter of input.outline.chapters) {
      let work = newStructureChapterWork(chapter.unit_lid);
      const continuation = continuations.find(r => r.unit_lid === chapter.unit_lid);
      const inspectedRefs: string[] = [];
      for (let ordinal = 0; !work.result; ordinal++) {
        if (ordinal >= 128) throw new StructureOrganizationBlocked(`Chapter ${chapter.unit_lid} action budget exhausted; accepted work retained`);
        if (continuation && ordinal === continuation.from_action_ordinal) work = continueStructureChapterSelection(work, continuation);
        const kind = work.selection_draft ? "structure_chapter_selection" : "structure_chapter";
        const id = work.selection_draft ? `structure:chapter:${chapter.unit_lid}:selection:${continuation!.id}:${ordinal}` : `structure:chapter:${chapter.unit_lid}:${ordinal}`;
        const next = step(kind, id, {
          ...packet, phase: "chapter", context, work, body: structureChapterInput(work, context),
        }) as StructureChapterWork | undefined;
        if (!next) break;
        inspectedRefs.push(...work.inspecting);
        work = next;
      }
      if (work.result) { selected.push(work.result); progress.chapters.done++;
        acceptedWork.set(chapter.unit_lid, { work, inspected_refs: [...new Set(inspectedRefs)] }); }
    }
    const revisionPending: string[] = [];
    for (const request of readStructureChapterRevisionRequests(input.target.workspace_dir)) {
      const prior = acceptedWork.get(request.unit_lid);
      if (!prior) throw new StructureOrganizationBlocked(`Chapter ${request.unit_lid} revision requires its accepted initial selection`);
      let work = reviseStructureChapterWork(prior.work, request, prior.inspected_refs);
      for (let ordinal = 0; !work.result; ordinal++) {
        if (ordinal >= 128) throw new StructureOrganizationBlocked(`Chapter ${request.unit_lid} revision action budget exhausted; accepted work retained`);
        const next = step(work.selection_draft ? "structure_chapter_selection" : "structure_chapter", `structure:chapter:${request.unit_lid}:revision:${request.id}:${ordinal}`, {
          ...packet, phase: "chapter", context, work, body: structureChapterInput(work, context),
        }) as StructureChapterWork | undefined;
        if (!next) { revisionPending.push(pending.pop()!); break; }
        work = next;
      }
      if (!work.result) break;
      selected[selected.findIndex(s => s.selection.unit_lid === request.unit_lid)] = work.result;
      acceptedWork.set(request.unit_lid, { work, inspected_refs: work.revision!.previously_inspected_refs });
    }
    pending.unshift(...revisionPending);
    if (pending.length) return { tasks, pending, progress };
    const themeContext: StructureThemeContext = { catalog: context.catalog, excerpts: context.excerpts,
      planning_candidate_refs: selected.flatMap(s => s.selection.macro_stop_refs), source_index_page_size: 64,
      chapters: (input.chapter_overviews ?? input.outline.chapters).map(c => ({ ...c,
        overview: selected.find(s => s.selection.unit_lid === c.unit_lid)?.selection.summary ?? c.overview,
        question: outline.chapters.find(o => o.unit_lid === c.unit_lid)!.question.text })),
      mode: selection && retrievalScopeIncludes(selection.data_scope, "book_structure") ? selection.retrieval_mode : "lexical_only" };
    let directory = step("structure_theme_plan", "structure:themes:plan", { ...packet, phase: "plan", context: themeContext,
      body: structureThemePlanningInput(themeContext) }) as StructureThemeDirectory | undefined;
    if (!directory) return { tasks, pending, progress };
    progress.themes.planned = true; progress.themes.total = directory.works.length;
    for (const ref of directory.works.map(w => w.plan.ref)) {
      for (let ordinal = 0; !directory.works.find(w => w.plan.ref === ref)!.result; ordinal++) {
        if (ordinal >= 128) throw new StructureOrganizationBlocked(`Theme ${ref} action budget exhausted; accepted work retained`);
        if (themeContext.mode === "semantic_required") {
          const work = directory.works.find(w => w.plan.ref === ref)!;
          const index = projectStructureRetrieval(themeContext.catalog, themeContext.chapters);
          const dependencies = retrievalDependencies(index.records, { kind: "search", query: work.query }, themeContext.mode,
            selection!.provider!.identity, STRUCTURE_RETRIEVAL_POLICY, STRUCTURE_RETRIEVAL.projection_version);
          themeContext.prepared = Object.values(state.prepared).find(p => retrievalPreparationMatches(p, dependencies));
          if (!themeContext.prepared) {
            preparation = { slot: `book-structure/${ref}/${ordinal}`, dependencies };
            remaining = retrievalRemainingWork(input.target.workspace_dir, preparation, state.prepared[preparation.slot]);
            return { tasks, pending, progress, preparation, remaining };
          }
        }
        const next = step("structure_theme", `structure:theme:${directory.works.findIndex(w => w.plan.ref === ref)}:${ordinal}`, {
          ...packet, phase: "theme", context: structuredClone(themeContext), directory, ref,
          body: structureThemeInput(directory, ref, themeContext),
        }) as StructureThemeDirectory | undefined;
        if (!next) return { tasks, pending, progress };
        directory = next;
      }
      progress.themes.done++;
    }
    directory = step("structure_theme_reconcile", "structure:themes:reconcile", { ...packet, phase: "reconcile", context: themeContext,
      directory, body: { themes: directory.works.map(w => ({ plan: w.plan, result: w.result,
        evidence_by_unit: w.evidence_by_unit, inspected_refs: w.inspected_refs })) } }) as StructureThemeDirectory | undefined;
    if (!directory) return { tasks, pending, progress };
    progress.reconciliation = directory.unresolved.length ? "unresolved" : "complete";
    if (directory.unresolved.length) throw new StructureOrganizationBlocked(directory.unresolved.join("; "));
    for (const request of readStructureThemeRevisionRequests(input.target.workspace_dir)) {
      try { directory = reopenStructureTheme(directory, request.theme_ref, request.issue); }
      catch (error) { throw new StructureOrganizationBlocked(`Theme revision ${request.id}: ${(error as Error).message}`); }
      for (let ordinal = 0; !directory.works.find(w => w.plan.ref === request.theme_ref)!.result; ordinal++) {
        if (ordinal >= 128) throw new StructureOrganizationBlocked(`Theme revision ${request.id} action budget exhausted; accepted work retained`);
        if (themeContext.mode === "semantic_required") {
          const work = directory.works.find(w => w.plan.ref === request.theme_ref)!;
          const index = projectStructureRetrieval(themeContext.catalog, themeContext.chapters);
          const dependencies = retrievalDependencies(index.records, { kind: "search", query: work.query }, themeContext.mode,
            selection!.provider!.identity, STRUCTURE_RETRIEVAL_POLICY, STRUCTURE_RETRIEVAL.projection_version);
          themeContext.prepared = Object.values(state.prepared).find(p => retrievalPreparationMatches(p, dependencies));
          if (!themeContext.prepared) {
            preparation = { slot: `book-structure/revision/${request.id}/${ordinal}`, dependencies };
            remaining = retrievalRemainingWork(input.target.workspace_dir, preparation, state.prepared[preparation.slot]);
            return { tasks, pending, progress, preparation, remaining };
          }
        }
        const next = step("structure_theme", `structure:theme:revision:${request.id}:${ordinal}`, {
          ...packet, phase: "theme", context: structuredClone(themeContext), directory, ref: request.theme_ref,
          revision_request: request, body: structureThemeInput(directory, request.theme_ref, themeContext),
        }) as StructureThemeDirectory | undefined;
        if (!next) return { tasks, pending, progress };
        directory = next;
      }
    }
    const base = materializeStructureChapterSelections(context.catalog, selected, input.outline.chapters.map(c => c.unit_lid), context.titles);
    candidate = materializeStructureThemes(base, directory, themeContext);
  } catch (error) {
    if (!(error instanceof StructureOrganizationBlocked)) throw error;
    blocked = error.message;
  }
  return { tasks, pending, progress, candidate, preparation, remaining, blocked };
}
