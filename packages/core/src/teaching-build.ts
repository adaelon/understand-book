import { createHash, randomUUID } from "node:crypto";
import { isDeepStrictEqual } from "node:util";
import { relocateGenerationTask } from "./semantic-artifact";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import type { AutomaticBuildTarget, AutomaticBuildStageState } from "./build-orchestrator";
import type { BookStructureSidecar, BookStructureUnitSource, BookStructureKeyStop } from "./book-structure";
import { acceptFormalObjects, checkTeachingBindings, formalObjectInput, type FormalObjects, type TeachingSource } from "./teaching-map";
import { advanceCognitiveWork, newCognitiveWork, collectCognitiveMaterials, cognitiveTaskInput, CognitiveBudgetZ, type CognitiveWork, type CognitiveMaterials, type CognitiveBudget } from "./cognitive-materials";
import { canonicalBuildJson, retrievalScopeIncludes, type BuildRetrievalSelection } from "./build-intent";
import { readBuildRetrievalState, retrievalRemainingWork } from "./automatic-build-retrieval";
import { retrievalDependencies, retrievalPreparationMatches, type PreparedRetrieval } from "./semantic-retrieval-preparation";
import { retrievalCatalog, projectRetrievalCatalog } from "./semantic-retrieval";
import { resolveContentProfile } from "./content-profile";
import { evaluateModelInputBudget } from "./model-input-budget";
import { AUTOMATIC_BUILD_MODEL_INPUT_BUDGET_V1 } from "./automatic-build-protocol";
import { createWorkUnitDescriptorV3, buildWorkUnitCost, taskPolicyBindingForWorkUnit, type WorkUnitDescriptorV3 } from "./stage-work-unit";
import { createAutomaticBuildStagePolicySet } from "./automatic-build-policy-generation";
import { automaticBuildExtractionPolicy, automaticBuildGenerationArtifactPath, buildSemanticArtifactEnvelopeV3, inspectSemanticArtifact,
  writeAutomaticBuildGenerationArtifact, semanticContractFromExtractionPolicy, type SemanticArtifactEnvelopeV3, type SemanticArtifactProvenanceV2 } from "./semantic-artifact";
import { publishAutomaticBuildArtifactSet, buildAutomaticBuildStageBatchResult } from "./automatic-build-publication";
import { TEACHING_EXTRACTORS, TEACHING_GENERATIONS, type TeachingBuildStage } from "./teaching-policy";
import { createAutomaticBuildFailureDiagnosticV3 } from "./extractor-contract";
import { routeFormalObjectFragments, acceptFormalObjectFragment, type FormalObjectFragment, type FormalObjectFragmentResult } from "./teaching-object-fragments";
import { newObjectAlignment, objectAlignmentInput, advanceObjectAlignment, alignmentFocus, AlignmentActionZ, alignmentRetrievalRequest, type ObjectAlignmentWork } from "./teaching-object-alignment";
import { newTeachingReviewWork, reviewSource, reviewRanges, teachingReviewInput, advanceTeachingReview, acceptTeachingReview,
  type TeachingSample, type TeachingReviewWork, type TeachingReview } from "./teaching-source-review";
export type { TeachingReview } from "./teaching-source-review";

const read = <T>(file: string): T | undefined => existsSync(file) ? JSON.parse(readFileSync(file, "utf8")) as T : undefined;
const json = (value: unknown) => `${JSON.stringify(value, null, 2)}\n`;
const digest = (value: string) => createHash("sha256").update(value).digest("hex");
const BUDGET = { searches: 12, preview_chars: 4000, read_chars: 12000, context_chars: 12000 };
const KINDS = { formal_objects: "formal_object_set", cognitive_materials: "cognitive_step", teaching_publish: "teaching_source_review" } as const;
export interface TeachingBuildInput { target: AutomaticBuildTarget; source: TeachingSource; units: BookStructureUnitSource[]; structure: BookStructureSidecar;
  retrieval?: BuildRetrievalSelection }
export interface TeachingGenerationTask {
  descriptor: WorkUnitDescriptorV3; policy_generation_id: string; rendered_input: string;
  source: TeachingSource; structure: BookStructureSidecar;
  previous?: FormalObjects; objects?: FormalObjects; target?: BookStructureKeyStop; work?: CognitiveWork;
  discourse?: BookStructureUnitSource["discourse_items"]; samples?: TeachingSample[];
  budget?: CognitiveBudget;
  fragment?: FormalObjectFragment;
  alignment?: ObjectAlignmentWork;
  retrieval?: PreparedRetrieval;
  reviewWork?: TeachingReviewWork;
}
class TeachingPreparationBlocked extends Error {
  constructor(readonly stage: TeachingBuildStage, message: string) { super(message); }
}
type Payload = FormalObjects | CognitiveWork | TeachingReview | FormalObjectFragmentResult | ObjectAlignmentWork | TeachingReviewWork;
interface AlignmentAcceptance extends ObjectAlignmentWork {
  accepted_action: unknown;
  finish_dependencies?: Omit<AlignmentMaterialization, "result">;
}
interface AlignmentMaterialization {
  source: TeachingSource; previous: FormalObjects | null; proposal: ObjectAlignmentWork["proposal"]; result: FormalObjects;
}
const finishDependencies = (task: TeachingGenerationTask) => ({ source: task.source, previous: task.previous ?? null, proposal: task.alignment!.proposal });
const materializationsPath = (target: AutomaticBuildTarget, task: TeachingGenerationTask) =>
  teachingTaskPath(target, task.policy_generation_id, task.descriptor.work_unit_id) + ".materialized.json";
const sameModelTask = (a: TeachingGenerationTask, b: TeachingGenerationTask) =>
  isDeepStrictEqual(a.descriptor, b.descriptor) && a.policy_generation_id === b.policy_generation_id && a.rendered_input === b.rendered_input;
export interface TeachingReadiness {
  version: "teaching_readiness.v1"; source_id: string; source_revision: string;
  status: "ready"; teaching_map_revision: string; map_path: string; limitations: string[];
  coverage: { source: "complete"; structure: "complete"; objects: "complete"; cognitive_materials: "complete"; source_review: "passed" };
}
interface TeachingMapPublication { source_id: string; source_revision: string; objects: FormalObjects; cognitive_materials: CognitiveMaterials; reviews: TeachingReview[] }
interface TeachingRepair {
  source_revision: string; formal_prompt_sha256: string; revision: number; formal_revision: number; previous?: FormalObjects;
  formal_feedback: TeachingReview[]; unit_feedback: Record<string, TeachingReview["samples"]>;
  unit_revisions: Record<string, number>;
  cognitive_revisions: Record<string, number>; cognitive_feedback: Record<string, TeachingReview["samples"]>;
  review_revisions: Record<string, number>;
}

export function teachingTaskPath(target: AutomaticBuildTarget, generation: string, id: string): string {
  // Same encoded work-unit naming and target-owned private root as generation artifacts.
  return path.join(target.workspace_dir, ".build", "teaching", generation, `${encodeURIComponent(id)}.task.json`);
}
export function freezeTeachingTask(target: AutomaticBuildTarget, task: TeachingGenerationTask): void {
  if (task.descriptor.stage === "formal_objects" && !task.fragment) {
    const baseline = path.join(target.workspace_dir, ".build", "teaching", `baseline-${task.source.source_revision}-${task.descriptor.policy_fingerprint.prompt_sha256}.json`);
    if (!existsSync(baseline)) { mkdirSync(path.dirname(baseline), { recursive: true }); writeFileSync(baseline, json({ previous: task.previous }), { flag: "wx" }); }
  }
  const file = teachingTaskPath(target, task.policy_generation_id, task.descriptor.work_unit_id);
  const old = read<TeachingGenerationTask>(file);
  if (old && !(task.alignment?.version === "formal_object_alignment.v3" && old.alignment?.version === "formal_object_alignment.v3"
    ? sameModelTask(relocateGenerationTask(old, target.target_ref), task)
    : canonicalBuildJson(relocateGenerationTask(old, target.target_ref)) === canonicalBuildJson(task))) throw new Error("teaching frozen task changed");
  if (!old) { mkdirSync(path.dirname(file), { recursive: true }); writeFileSync(file, json(task), { flag: "wx" }); }
  // The immutable task records what the model saw; this separate writer context owns the current ledger.
  if (task.alignment?.version === "formal_object_alignment.v3") {
    const current = `${file}.current.json`, bytes = json(task);
    if (!existsSync(current) || readFileSync(current, "utf8") !== bytes) writeFileSync(current, bytes);
  }
}
export function readTeachingTask(target: AutomaticBuildTarget, generation: string, id: string, current = false): TeachingGenerationTask {
  const file = teachingTaskPath(target, generation, id);
  const task = (current ? read<TeachingGenerationTask>(`${file}.current.json`) : undefined) ?? read<TeachingGenerationTask>(file);
  if (!task) throw new Error("teaching frozen task is unavailable or stale");
  return relocateGenerationTask(task, target.target_ref);
}
function accepted(target: AutomaticBuildTarget, task: TeachingGenerationTask): SemanticArtifactEnvelopeV3<Payload> | undefined {
  const d = task.descriptor;
  const artifact = read<SemanticArtifactEnvelopeV3<Payload>>(automaticBuildGenerationArtifactPath(target, d.stage, task.policy_generation_id, d.work_unit_id));
  if (!artifact) return;
  const inspection = inspectSemanticArtifact(artifact, { target: target.target_ref, stage: d.stage, work_unit_id: d.work_unit_id,
    input_hash: d.input_hash, policy_generation_id: task.policy_generation_id, semantic_contract: semanticContractFromExtractionPolicy(d.policy_fingerprint) });
  if (!inspection.policy_fresh || inspection.format !== "v3") return;
  if (!task.alignment) return artifact;
  const saved = artifact.payload as AlignmentAcceptance;
  if (task.alignment.version !== "formal_object_alignment.v3" || saved.version !== "formal_object_alignment.v3" || !saved.accepted_action) return;
  try {
    const action = AlignmentActionZ.parse(saved.accepted_action);
    if (action.kind !== "finish") return { ...artifact, payload: advanceObjectAlignment({ work: task.alignment,
      action: saved.accepted_action, source: task.source, previous: task.previous, retrieval: task.retrieval, operation_id: d.work_unit_id }) };
    if (alignmentFocus(task.alignment, task.previous).kind !== "finish") return;
    const dependencies = finishDependencies(task);
    const result = isDeepStrictEqual(saved.finish_dependencies, dependencies) ? saved.result
      : read<AlignmentMaterialization[]>(materializationsPath(target, task))?.find(m =>
        isDeepStrictEqual({ source: m.source, previous: m.previous, proposal: m.proposal }, dependencies))?.result;
    // Ordinary routing never allocates identities or writes a new materialization.
    const work = structuredClone(task.alignment);
    if (!result) return { ...artifact, payload: { ...work, finish_requested: true } };
    work.result = result; work.steps_since_progress = 0; work.inspected = []; work.read_ranges = [];
    delete work.search; delete work.reading; delete work.finish_requested;
    return { ...artifact, payload: work };
  } catch { return; } // A reusable model action must still pass today's Core gate.
}
function makeTask(input: TeachingBuildInput, stage: TeachingBuildStage, id: string, body: unknown, extra: Partial<TeachingGenerationTask>): TeachingGenerationTask {
  const policy = automaticBuildExtractionPolicy(stage, resolveContentProfile(input.target.profile_id), "full");
  const rendered = canonicalBuildJson(body);
  const budget = evaluateModelInputBudget({ ...AUTOMATIC_BUILD_MODEL_INPUT_BUDGET_V1, rendered_input: rendered,
    router_version: policy.router_version, prompt_sha256: policy.prompt_sha256, output_reserve_tokens: 4096, executor_context_floor_tokens: 16384 });
  if (budget.status !== "within_limit") throw new TeachingPreparationBlocked(stage, `教学输入超过当前任务预算：${stage}/${id}；材料仍未完成，需要缩小构造单元`);
  const lids = [...new Set(input.source.passages.map(p => p.lid))];
  const descriptor = createWorkUnitDescriptorV3({ target: input.target.target_ref, stage,
    work_unit_id: `${id}-${budget.proof.rendered_input_sha256.slice(0, 16)}`, kind: KINDS[stage],
    input_basis: { kind: "semantic_projection", projection_kind: "teaching_map", source_fingerprint: input.source.source_revision,
      projection_sha256: budget.proof.rendered_input_sha256, parent_lids: lids },
    input_hash: budget.proof.rendered_input_sha256, input_budget_proof: budget.proof, policy_fingerprint: policy, evidence_lids: lids,
    cost: buildWorkUnitCost({ estimated_input_tokens: budget.proof.estimated_rendered_tokens, visible_lids: lids.length, expected_output_items: 1 }) });
  return { descriptor, policy_generation_id: TEACHING_GENERATIONS[stage], rendered_input: rendered, source: input.source, structure: input.structure, ...extra };
}
function stageState(input: TeachingBuildInput, stage: TeachingBuildStage, tasks: TeachingGenerationTask[], closed: boolean): AutomaticBuildStageState {
  const policy = automaticBuildExtractionPolicy(stage, resolveContentProfile(input.target.profile_id), "full");
  const policy_set = createAutomaticBuildStagePolicySet({ target_ref: input.target.target_ref, stage, frozen_at: "2026-09-29T00:00:00.000Z",
    members: [{ kind: KINDS[stage], extractor: TEACHING_EXTRACTORS[stage], policy_generation_id: TEACHING_GENERATIONS[stage], policy_fingerprint: policy }] });
  const pending = tasks.filter(task => !accepted(input.target, task));
  return { stage, closed, policy_set, work_units: tasks.map(t => t.descriptor), pending_work_units: pending.map(t => t.descriptor),
    pending_tasks: pending.map(t => t.descriptor.work_unit_id),
    task_bindings: Object.fromEntries(tasks.map(t => [t.descriptor.work_unit_id, taskPolicyBindingForWorkUnit(t.descriptor, t.policy_generation_id)])),
    generation_tasks: Object.fromEntries(tasks.map(t => [t.descriptor.work_unit_id, { kind: "teaching" as const, task: t }])) };
}
function validateWholeSource(input: TeachingBuildInput): void {
  const { source, structure } = input;
  const units = new Set(source.passages.map(p => p.unit_lid));
  if (!units.size || structure.header.book_id !== source.source_id || structure.spine.length !== units.size
    || new Set(structure.spine.map(u => u.lid)).size !== units.size || structure.spine.some(u => !units.has(u.lid))) throw new Error("teaching structure coverage incomplete");
  const checkLids = (lids: string[]) => checkTeachingBindings(lids.map(lid => ({ source_id: source.source_id, source_revision: source.source_revision, lid })), source);
  structure.spine.forEach(u => checkLids(u.summary.evidence_lids));
  structure.throughlines.forEach(t => checkLids(t.summary.evidence_lids));
  structure.key_stops.forEach(t => checkLids([t.lid, ...t.reason.evidence_lids]));
}
function publicationMatches(input: TeachingBuildInput, objects: FormalObjects, materials: CognitiveMaterials): boolean {
  const receipt = read<TeachingReadiness>(path.join(input.target.workspace_dir, "teaching_readiness.json"));
  if (receipt?.status !== "ready" || receipt.source_revision !== input.source.source_revision) return false;
  const map = read<TeachingMapPublication>(path.join(input.target.workspace_dir, receipt.map_path));
  return !!map && canonicalBuildJson(map.objects) === canonicalBuildJson(objects) && canonicalBuildJson(map.cognitive_materials) === canonicalBuildJson(materials);
}

/** Existing controller calls this after the accepted structure stage; no additional executor loop. */
export function routeTeachingBuildStages(input: TeachingBuildInput): AutomaticBuildStageState[] {
  const states: AutomaticBuildStageState[] = [];
  try { return routeTeachingStages(input, states); }
  catch (error) {
    if (!(error instanceof TeachingPreparationBlocked)) throw error;
    const state = states.find(s => s.stage === error.stage);
    if (state) state.teaching_blocked = error.message;
    else states.push({ stage: error.stage, pending_tasks: [], closed: false, teaching_blocked: error.message });
    return states;
  }
}
function routeTeachingStages(input: TeachingBuildInput, states: AutomaticBuildStageState[]): AutomaticBuildStageState[] {
  validateWholeSource(input);
  const retrievalState = readBuildRetrievalState(input.target.workspace_dir);
  const requestedSelection = input.retrieval ?? retrievalState.selection;
  const selection = requestedSelection && retrievalScopeIncludes(requestedSelection.data_scope, "formal_objects") ? requestedSelection : undefined;
  const formalPolicy = automaticBuildExtractionPolicy("formal_objects", resolveContentProfile(input.target.profile_id), "full");
  const savedRound = read<TeachingRepair>(path.join(input.target.workspace_dir, ".build", "teaching", "repair.json"));
  const round = savedRound?.source_revision === input.source.source_revision && savedRound.formal_prompt_sha256 === formalPolicy.prompt_sha256 ? savedRound : undefined;
  const baseline = read<{ previous?: FormalObjects }>(path.join(input.target.workspace_dir, ".build", "teaching", `baseline-${input.source.source_revision}-${formalPolicy.prompt_sha256}.json`));
  const oldObjects = read<FormalObjects>(path.join(input.target.workspace_dir, "formal_objects.json"));
  const previous = round?.previous ?? (baseline ? baseline.previous : oldObjects);
  let formalResult: FormalObjects | undefined;
  let formalTasks: TeachingGenerationTask[];
  try {
    const formal = makeTask(input, "formal_objects", `objects-${round?.formal_revision ?? 0}`,
      { ...formalObjectInput(input.source, input.units, input.structure, previous), feedback: round?.formal_feedback ?? [] }, { previous });
    formalTasks = [formal];
    formalResult = accepted(input.target, formal)?.payload as FormalObjects | undefined;
  } catch (error) {
    if (!(error instanceof TeachingPreparationBlocked)) throw error;
    const policy = automaticBuildExtractionPolicy("formal_objects", resolveContentProfile(input.target.profile_id), "full");
    let fragments: FormalObjectFragment[];
    try {
      fragments = routeFormalObjectFragments({ ...input, feedback_by_unit: round?.unit_feedback, feedback_revision_by_unit: round?.unit_revisions, budget: { ...AUTOMATIC_BUILD_MODEL_INPUT_BUDGET_V1,
        router_version: policy.router_version, prompt_sha256: policy.prompt_sha256, output_reserve_tokens: 4096, executor_context_floor_tokens: 16384 } });
    } catch (cause) {
      throw new TeachingPreparationBlocked("formal_objects", cause instanceof Error ? cause.message : String(cause));
    }
    const tasks = fragments.map(fragment => makeTask({ ...input, source: fragment.input.source }, "formal_objects",
      `objects-${fragment.id}`, fragment.input, { fragment }));
    const state = stageState(input, "formal_objects", tasks, false);
    if (state.pending_tasks.length) { states.push(state); return states; }
    let alignment = newObjectAlignment(tasks.map(t => accepted(input.target, t)!.payload as FormalObjectFragmentResult));
    for (let step = 0; ; step++) {
      if (alignment.steps_since_progress >= 32) throw new TeachingPreparationBlocked("formal_objects", "跨块对齐本轮检索预算耗尽；已接纳步骤保留，材料仍未完成");
      let retrieval: PreparedRetrieval | undefined;
      if (selection) {
        const dependencies = retrievalDependencies(projectRetrievalCatalog(retrievalCatalog(alignment.proposal, previous)),
          alignmentRetrievalRequest(alignment, previous), selection.retrieval_mode, selection.provider?.identity ?? null);
        const slot = `${TEACHING_GENERATIONS.formal_objects}/${input.source.source_revision}/${round?.formal_revision ?? 0}/${step}`;
        retrieval = Object.values(retrievalState.prepared).find(p => retrievalPreparationMatches(p, dependencies));
        if (!retrieval) {
          const pending = stageState(input, "formal_objects", tasks, false);
          pending.preparation_required = true;
          pending.retrieval_preparation = { slot, dependencies };
          pending.retrieval_remaining = retrievalRemainingWork(input.target.workspace_dir, pending.retrieval_preparation,
            retrievalState.prepared[slot] ?? Object.values(retrievalState.prepared).at(-1));
          states.push(pending);
          return states;
        }
      }
      const task = makeTask(input, "formal_objects", `align-v3-${round?.formal_revision ?? 0}-${step}`,
        objectAlignmentInput(alignment, input.source, previous, retrieval), { alignment, previous, ...(retrieval ? { retrieval } : {}) });
      // A formerly accepted action can fail a gate on an unshown current dependency.
      // Keep that immutable receipt and give the replacement action its own task slot.
      while (existsSync(automaticBuildGenerationArtifactPath(input.target, "formal_objects", task.policy_generation_id, task.descriptor.work_unit_id))
        && !accepted(input.target, task)) task.descriptor.work_unit_id += "-replay";
      tasks.push(task);
      const result = accepted(input.target, task)?.payload as ObjectAlignmentWork | undefined;
      if (!result) break;
      alignment = result;
      if (alignment.result) { formalResult = alignment.result; break; }
      if (alignment.finish_requested) break;
    }
    formalTasks = tasks;
  }
  const formalFile = read<FormalObjects>(path.join(input.target.workspace_dir, "formal_objects.json"));
  states.push(stageState(input, "formal_objects", formalTasks, !!formalResult && canonicalBuildJson(formalFile ?? null) === canonicalBuildJson(formalResult)));
  if (!states[0].closed || !formalResult) return states;
  const tasks: TeachingGenerationTask[] = [];
  const works: CognitiveWork[] = [];
  let blocked: string | undefined;
  for (const [targetIndex, target] of input.structure.key_stops.entries()) {
    const saved = read<CognitiveResume>(resumePath(input, targetIndex));
    const repairRevision = round?.cognitive_revisions[target.id] ?? 0;
    const resume = saved?.source_revision === input.source.source_revision && saved.objects_revision === formalResult.revision
      && (saved.repair_revision ?? 0) === repairRevision ? saved : undefined;
    let work = resume?.work ?? newCognitiveWork(input.source, target);
    const workBudget = resume?.budget ?? BUDGET;
    // Replay accepted bounded steps; only the first missing step is dispatched.
    for (let step = 0; ; step++) {
      const feedback = round?.cognitive_feedback[target.id] ?? [];
      const task = makeTask(input, "cognitive_materials", `target-${targetIndex}-repair-${repairRevision}-resume-${resume?.revision ?? 0}-${step}`,
        { ...cognitiveTaskInput(input.source, target, formalResult, work, workBudget, body => {
          const policy = automaticBuildExtractionPolicy("cognitive_materials", resolveContentProfile(input.target.profile_id), "full");
          return evaluateModelInputBudget({ ...AUTOMATIC_BUILD_MODEL_INPUT_BUDGET_V1, rendered_input: canonicalBuildJson({ ...body as object, feedback }),
            router_version: policy.router_version, prompt_sha256: policy.prompt_sha256, output_reserve_tokens: 4096, executor_context_floor_tokens: 16384 }).status === "within_limit";
        }), feedback },
        { objects: formalResult, target, work, budget: workBudget, discourse: input.units.flatMap(u => u.discourse_items) });
      tasks.push(task);
      const result = accepted(input.target, task)?.payload as CognitiveWork | undefined;
      if (!result) break;
      work = result;
      if (work.stop_reason) { blocked = `认知素材读取预算耗尽：${target.id}；保留进度，材料仍未完成`; break; }
      if (work.status === "complete") { works.push(work); break; }
      if (step >= 48) { blocked = `认知素材动作预算耗尽：${target.id}；保留进度，材料仍未完成`; break; }
    }
  }
  const materials = works.length === input.structure.key_stops.length ? collectCognitiveMaterials(input.source, input.structure.key_stops, formalResult, works) : undefined;
  const materialFile = read<CognitiveMaterials>(path.join(input.target.workspace_dir, "cognitive_materials.json"));
  states.push(stageState(input, "cognitive_materials", tasks, !!materials && canonicalBuildJson(materialFile ?? null) === canonicalBuildJson(materials)));
  if (blocked) { states.at(-1)!.teaching_blocked = blocked; return states; }
  if (!states.at(-1)!.closed || !materials) return states;
  const samples = teachingSamples(formalResult, materials);
  const reviewTasks: TeachingGenerationTask[] = [];
  for (const sample of samples) {
    const reviewRevision = round?.review_revisions[reviewIdentity(sample.sample_id)] ?? 0;
    try {
      reviewTasks.push(makeTask(input, "teaching_publish", `review-${sample.sample_id}-repair-${reviewRevision}`, {
        samples: [sample], source: reviewSource(input.source, sample),
      }, { samples: [sample] }));
    } catch (error) {
      if (!(error instanceof TeachingPreparationBlocked)) throw error;
      let work = newTeachingReviewWork(input.source, sample);
      const limit = reviewRanges(input.source, sample).reduce((n, r) => n + Math.ceil((r.end - r.start) / 2000), 0) + 64;
      for (let step = 0; ; step++) {
        if (step >= limit || work.reading.stop_reason) throw new TeachingPreparationBlocked("teaching_publish", `来源审阅读取预算耗尽：${sample.sample_id}`);
        const task = makeTask(input, "teaching_publish", `review-${sample.sample_id}-repair-${reviewRevision}-step-${step}`,
          teachingReviewInput(input.source, sample, work), { samples: [sample], reviewWork: work });
        reviewTasks.push(task);
        const result = accepted(input.target, task)?.payload as TeachingReviewWork | undefined;
        if (!result || result.review) break;
        work = result;
      }
    }
  }
  states.push(stageState(input, "teaching_publish", reviewTasks, publicationMatches(input, formalResult, materials)));
  return states;
}

export function teachingSamples(objects: FormalObjects, materials: CognitiveMaterials): TeachingSample[] {
  // One object per source unit plus every conditional relation and gap-bearing material;
  // one material of each form catches systematic shape errors without rereviewing a book.
  const ids = new Set(objects.coverage.flatMap(c => c.object_refs.slice(0, 1).map(r => r.object_id)));
  const current = objects.active_refs.map(ref => objects.objects.filter(o => o.ref.object_id === ref.object_id).at(-1)!);
  current.filter(o => o.kind === "relation" || o.kind === "composite").forEach(o => ids.add(o.ref.object_id));
  const samples: TeachingSample[] = current.filter(o => ids.has(o.ref.object_id)).map(o => {
    const bindings = [...o.source_bindings, ...objects.coverage.filter(c => c.object_refs[0]?.object_id === o.ref.object_id).flatMap(c => c.source_bindings)];
    return { sample_id: `object:${o.ref.object_id}:${o.object_revision}`, content: o,
      evidence_lids: [...new Set(bindings.map(b => b.lid))], evidence_bindings: bindings };
  });
  const kinds = new Set<string>();
  for (const material of materials.materials) {
    if (!kinds.has(material.kind) || material.gaps.length) {
      kinds.add(material.kind);
      const bindings = [...material.steps, ...material.connections, ...material.gaps, ...material.patterns].flatMap(s => s.source_bindings);
      for (const gap of material.gaps) for (const lid of gap.inspected_lids) if (!bindings.some(b => b.lid === lid)) bindings.push({ source_id: objects.source_id, source_revision: objects.source_revision, lid });
      samples.push({ sample_id: `material:${material.target_id}`, content: material,
        evidence_lids: [...new Set(bindings.map(b => b.lid))], evidence_bindings: bindings });
    }
  }
  // Every empty unit still requires independent structural coverage review.
  for (const coverage of objects.coverage.filter(c => !c.object_refs.length)) samples.push({ sample_id: `coverage:${coverage.unit_lid}`, content: coverage,
    evidence_lids: coverage.source_bindings.map(b => b.lid), evidence_bindings: coverage.source_bindings });
  return samples;
}
export function acceptTeachingCandidate(target: AutomaticBuildTarget, task: TeachingGenerationTask, candidate: unknown, provenance: SemanticArtifactProvenanceV2): string {
  const existing = accepted(target, task);
  if (existing) return automaticBuildGenerationArtifactPath(target, task.descriptor.stage, task.policy_generation_id, task.descriptor.work_unit_id);
  let payload: Payload;
  try {
  if (task.fragment) payload = acceptFormalObjectFragment(task.fragment, candidate);
  else if (task.alignment) {
    const next = advanceObjectAlignment({ work: task.alignment, action: candidate, retrieval: task.retrieval,
      source: task.source, previous: task.previous, operation_id: task.descriptor.work_unit_id });
    payload = { ...next, accepted_action: structuredClone(candidate),
      ...(next.result ? { finish_dependencies: finishDependencies(task) } : {}) } as AlignmentAcceptance;
  }
  else if (task.descriptor.stage === "formal_objects") payload = acceptFormalObjects({ source: task.source, proposal: candidate,
    operation_id: task.descriptor.work_unit_id, previous: task.previous });
  else if (task.descriptor.stage === "cognitive_materials") payload = advanceCognitiveWork({ work: task.work!, action: candidate,
    source: task.source, target: task.target!, objects: task.objects!, discourse: task.discourse!, budget: task.budget ?? BUDGET });
  else if (task.reviewWork) payload = advanceTeachingReview(task.source, task.samples![0], task.reviewWork, candidate);
  else payload = acceptTeachingReview(task.source, task.samples!, candidate);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const invalid = new Error(message);
    Object.assign(invalid, { failure_diagnostic: createAutomaticBuildFailureDiagnosticV3({
      category: "schema", code: "semantic_output_invalid", phase: "artifact_writer",
      expected: message.slice(0, 120),
    }) });
    throw invalid;
  }
  return writeAutomaticBuildGenerationArtifact(target, buildSemanticArtifactEnvelopeV3({ target: target.target_ref, stage: task.descriptor.stage,
    work_unit_id: task.descriptor.work_unit_id, input_hash: task.descriptor.input_hash, policy_generation_id: task.policy_generation_id,
    semantic_contract: semanticContractFromExtractionPolicy(task.descriptor.policy_fingerprint), provenance, payload }));
}
export function teachingReviewFailures(target: AutomaticBuildTarget, state: AutomaticBuildStageState): string[] {
  if (state.stage !== "teaching_publish") return [];
  return Object.values(state.generation_tasks ?? {}).flatMap(t => {
    if (t.kind !== "teaching") return [];
    const payload = accepted(target, t.task)?.payload;
    const review = payload && reviewPayload(payload);
    return review?.samples.filter(s => s.verdict === "fail").map(s => `${s.sample_id}: ${s.reason}`) ?? [];
  });
}
export function closeTeachingStage(input: TeachingBuildInput, stage: TeachingBuildStage, routedState?: AutomaticBuildStageState) {
  validateWholeSource(input);
  const state = routedState ?? routeTeachingBuildStages(input).find(s => s.stage === stage);
  if (!state || state.pending_tasks.length || state.teaching_blocked || state.preparation_required) throw new Error("teaching required work incomplete");
  const tasks = Object.values(state.generation_tasks ?? {}).map(t => (t as { kind: "teaching"; task: TeachingGenerationTask }).task);
  const payloads = tasks.map(t => accepted(input.target, t)!.payload);
  let artifacts: Record<string, string>;
  if (stage === "formal_objects") {
    const final = payloads.at(-1)!;
    const lastTask = tasks.at(-1)!;
    let objects = "version" in final && final.version === "formal_object_alignment.v3" ? final.result : final as FormalObjects;
    if (lastTask.alignment && "finish_requested" in final && final.finish_requested) {
      objects = advanceObjectAlignment({ work: lastTask.alignment, action: { kind: "finish" }, source: lastTask.source,
        previous: lastTask.previous, retrieval: lastTask.retrieval, operation_id: lastTask.descriptor.work_unit_id }).result!;
      const file = materializationsPath(input.target, lastTask);
      const saved = read<AlignmentMaterialization[]>(file) ?? [];
      saved.push({ ...finishDependencies(lastTask), result: objects });
      writeFileSync(file, json(saved));
    }
    if (!objects || objects.version !== "formal_objects.v1") throw new Error("formal object alignment incomplete");
    artifacts = { "formal_objects.json": json(objects) };
  }
  else if (stage === "cognitive_materials") {
    const objects = read<FormalObjects>(path.join(input.target.workspace_dir, "formal_objects.json"))!;
    artifacts = { "cognitive_materials.json": json(collectCognitiveMaterials(input.source, input.structure.key_stops, objects,
      (payloads as CognitiveWork[]).filter(w => w.status === "complete"))) };
  } else {
    const reviews = payloads.flatMap(p => { const review = reviewPayload(p); return review ? [review] : []; });
    const expected = new Set(tasks.flatMap(t => t.samples!.map(s => s.sample_id)));
    if (reviews.flatMap(r => r.samples).length !== expected.size) throw new Error("teaching review work incomplete");
    if (reviews.some(r => r.samples.some(s => s.verdict !== "pass"))) throw new Error("teaching source review failed; repair required");
    const objects = read<FormalObjects>(path.join(input.target.workspace_dir, "formal_objects.json"))!;
    const materials = read<CognitiveMaterials>(path.join(input.target.workspace_dir, "cognitive_materials.json"))!;
    if (publicationMatches(input, objects, materials)) return read<TeachingReadiness>(path.join(input.target.workspace_dir, "teaching_readiness.json"))!;
    const revision = randomUUID();
    const mapPath = `teaching/versions/${revision}/map.json`;
    const map: TeachingMapPublication = { source_id: input.source.source_id, source_revision: input.source.source_revision,
      objects, cognitive_materials: materials, reviews };
    const receipt: TeachingReadiness = { version: "teaching_readiness.v1", source_id: input.source.source_id, source_revision: input.source.source_revision,
      status: "ready", teaching_map_revision: revision, map_path: mapPath,
      limitations: materials.materials.flatMap(m => m.gaps.map(g => g.description)),
      coverage: { source: "complete", structure: "complete", objects: "complete", cognitive_materials: "complete", source_review: "passed" } };
    artifacts = { [mapPath]: json(map), "teaching_readiness.json": json(receipt) };
  }
  return buildAutomaticBuildStageBatchResult(publishAutomaticBuildArtifactSet({ workspace_dir: input.target.workspace_dir, stage, artifacts }));
}

/** Explicit correction reuses previous identities while retaining all accepted work and published versions. */
export function reopenTeachingBuild(input: TeachingBuildInput): void {
  const file = path.join(input.target.workspace_dir, ".build", "teaching", "repair.json");
  const previous = read<FormalObjects>(path.join(input.target.workspace_dir, "formal_objects.json"));
  const state = routeTeachingBuildStages(input).find(s => s.stage === "teaching_publish");
  const feedback = Object.values(state?.generation_tasks ?? {}).flatMap(t => {
    const artifact = accepted(input.target, (t as { task: TeachingGenerationTask }).task);
    const review = artifact && reviewPayload(artifact.payload);
    return review ? [review] : [];
  });
  const failures = feedback.flatMap(r => r.samples.filter(s => s.verdict === "fail"));
  if (!failures.length || !previous) throw new Error("teaching repair requires a failed source review");
  const saved = read<TeachingRepair>(file);
  const formalPolicy = automaticBuildExtractionPolicy("formal_objects", resolveContentProfile(input.target.profile_id), "full");
  const old = saved?.source_revision === input.source.source_revision && saved.formal_prompt_sha256 === formalPolicy.prompt_sha256 ? saved : undefined;
  const revision = (old?.revision ?? 0) + 1;
  const next: TeachingRepair = { source_revision: input.source.source_revision, formal_prompt_sha256: formalPolicy.prompt_sha256, revision, formal_revision: old?.formal_revision ?? 0,
    previous: old?.previous, formal_feedback: old?.formal_feedback ?? [], unit_feedback: { ...old?.unit_feedback }, unit_revisions: { ...old?.unit_revisions },
    cognitive_revisions: { ...old?.cognitive_revisions }, cognitive_feedback: { ...old?.cognitive_feedback }, review_revisions: { ...old?.review_revisions } };
  const objectFailures = failures.filter(s => !s.sample_id.startsWith("material:"));
  const affected = new Set(objectFailures.filter(s => s.sample_id.startsWith("object:")).map(s => s.sample_id.split(":")[1]));
  const current = previous.active_refs.map(ref => previous.objects.filter(o => o.ref.object_id === ref.object_id).at(-1)!);
  // A changed component/participant/prerequisite invalidates its consumers, not unrelated units.
  for (let changed = true; changed;) {
    changed = false;
    for (const object of current) if (!affected.has(object.ref.object_id) && [...object.component_refs, ...object.participants.map(p => p.object_ref)].some(r => affected.has(r.object_id))) {
      affected.add(object.ref.object_id); changed = true;
    }
    for (const edge of previous.prerequisites) if (affected.has(edge.prerequisite.object_ref.object_id) && !affected.has(edge.target.object_ref.object_id)) {
      affected.add(edge.target.object_ref.object_id); changed = true;
    }
  }
  const affectedLids = new Set([...objectFailures.flatMap(s => s.source_bindings.map(b => b.lid)), ...current.filter(o => affected.has(o.ref.object_id)).flatMap(o => o.source_bindings.map(b => b.lid))]);
  if (objectFailures.length) {
    next.formal_revision++; next.previous = previous; next.formal_feedback = [{ samples: objectFailures }];
    for (const unit of new Set(input.source.passages.filter(p => affectedLids.has(p.lid)).map(p => p.unit_lid))) {
      const local = objectFailures.filter(f => f.source_bindings.some(b => input.source.passages.some(p => p.lid === b.lid && p.unit_lid === unit)));
      next.unit_feedback[unit] = local.length ? local : objectFailures;
      next.unit_revisions[unit] = revision;
    }
  }
  const materials = read<CognitiveMaterials>(path.join(input.target.workspace_dir, "cognitive_materials.json"));
  for (const target of input.structure.key_stops) {
    const material = materials?.materials.find(m => m.target_id === target.id);
    const direct = failures.filter(s => s.sample_id === `material:${target.id}`);
    const depends = affectedLids.has(target.lid) || material?.object_refs.some(r => affected.has(r.object_id))
      || material && [...material.steps, ...material.connections, ...material.gaps, ...material.patterns].some(s => s.source_bindings.some(b => affectedLids.has(b.lid)));
    if (direct.length || depends) { next.cognitive_revisions[target.id] = revision; next.cognitive_feedback[target.id] = [...direct, ...objectFailures]; }
  }
  for (const sample of teachingSamples(previous, materials!)) {
    if (failures.some(s => reviewIdentity(s.sample_id) === reviewIdentity(sample.sample_id))
      || sample.evidence_lids.some(lid => affectedLids.has(lid)) || sample.sample_id.startsWith("material:") && next.cognitive_revisions[sample.sample_id.slice(9)] === revision)
      next.review_revisions[reviewIdentity(sample.sample_id)] = revision;
  }
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, json(next));
}

const reviewIdentity = (id: string) => id.startsWith("object:") ? id.slice(0, id.lastIndexOf(":")) : id;

function reviewPayload(payload: Payload): TeachingReview | undefined {
  if ("samples" in payload) return payload;
  if (payload.version === "teaching_review_work.v1") return payload.review;
  return undefined;
}

interface CognitiveResume { revision: number; source_revision: string; objects_revision: number; repair_revision?: number; budget: CognitiveBudget; work: CognitiveWork }
const resumePath = (input: TeachingBuildInput, index: number) => path.join(input.target.workspace_dir, ".build", "teaching", `resume-${index}.json`);
/** Explicitly approved larger reading allowance; the BuildPlan token/wall gates still apply. */
export function resumeTeachingCognitiveBudget(input: TeachingBuildInput, targetId: string, budget: CognitiveBudget): void {
  budget = CognitiveBudgetZ.parse(budget);
  const index = input.structure.key_stops.findIndex(t => t.id === targetId);
  if (index < 0) throw new Error("unknown cognitive target");
  const stage = routeTeachingBuildStages(input).find(s => s.stage === "cognitive_materials");
  const task = Object.values(stage?.generation_tasks ?? {}).filter(t => t.kind === "teaching" && t.task.target?.id === targetId).at(-1);
  if (!task || task.kind !== "teaching") throw new Error("cognitive work is unavailable");
  const work = accepted(input.target, task.task)?.payload as CognitiveWork | undefined;
  if (!work || work.status === "complete") throw new Error("cognitive work is not interrupted");
  if (Object.entries(task.task.budget ?? BUDGET).some(([key, value]) => budget[key as keyof CognitiveBudget] < value)) throw new Error("resume budget cannot reduce the prior allowance");
  const file = resumePath(input, index);
  const repair = read<TeachingRepair>(path.join(input.target.workspace_dir, ".build", "teaching", "repair.json"));
  const formalPolicy = automaticBuildExtractionPolicy("formal_objects", resolveContentProfile(input.target.profile_id), "full");
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, json({ revision: (read<CognitiveResume>(file)?.revision ?? 0) + 1, source_revision: input.source.source_revision,
    objects_revision: task.task.objects!.revision, repair_revision: repair?.source_revision === input.source.source_revision
      && repair.formal_prompt_sha256 === formalPolicy.prompt_sha256 ? repair.cognitive_revisions[targetId] ?? 0 : 0,
    budget, work } satisfies CognitiveResume));
}
