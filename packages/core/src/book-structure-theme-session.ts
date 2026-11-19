import { readAutomaticBuildStageUsage } from "./automatic-build-metrics";
import { configuredBuildRetrievalRuntime } from "./build-retrieval-config";
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import type { AutomaticBuildTarget, AutomaticBuildStage } from "./build-orchestrator";
import type { BookStructureGenerationTaskV1 } from "./book-structure-generation";
import { acceptStructureOrganizationAction, applyStructureOrganizationAction, fitStructureOrganizationExecution,
  renderStructureOrganizationInput, type StructureOrganizationInput } from "./book-structure-organization";
import { structureThemeInput, type StructureThemeDirectory } from "./book-structure-themes";
import { prepareBuildRetrieval, readBuildRetrievalState, type TeachingRetrievalRequest } from "./automatic-build-retrieval";
import { projectStructureRetrieval, STRUCTURE_RETRIEVAL, STRUCTURE_RETRIEVAL_POLICY } from "./book-structure-retrieval";
import { retrievalDependencies, retrievalPreparationMatches } from "./semantic-retrieval-preparation";
import type { BookStructureExecutionContractV2 } from "./book-structure";
import { createCandidateTransportContract, type ExecutorTransportProfile } from "./executor-transport";

type ThemeInput = Extract<StructureOrganizationInput, { phase: "theme" }>;
interface ThemeAction { input_ref: string; action: unknown }
export interface StructureThemeAcceptance {
  version: "book_structure_theme_work.v1";
  actions: unknown[];
}
export class StructureThemeActionRejected extends Error {
  constructor(error: unknown) { super(error instanceof Error ? error.message : String(error), { cause: error }); }
}

/** Retrieval stays inside the confirmed build's provider and budget authority. */
export class StructureThemeRetrievalRequired extends Error {
  constructor(readonly target: AutomaticBuildTarget, readonly request: TeachingRetrievalRequest) {
    super("theme retrieval requires authorized preparation");
  }
}

export async function prepareStructureThemeRetrieval(error: StructureThemeRetrievalRequired) {
  const plan = readBuildRetrievalState(error.target.workspace_dir).authorization;
  if (!plan) throw new Error("theme retrieval has no confirmed build authorization");
  const failure = await prepareBuildRetrieval({ workspace: error.target.workspace_dir, plan,
    runtime: configuredBuildRetrievalRuntime(plan), request: error.request,
    model_tokens: plan.public_stage_closure.reduce((sum, stage) => {
      const usage = readAutomaticBuildStageUsage(error.target, stage as AutomaticBuildStage);
      return sum + usage.input_tokens + usage.output_tokens;
    }, 0) });
  if (failure) throw new Error(failure.message);
}

function journalPath(target: AutomaticBuildTarget, task: BookStructureGenerationTaskV1) {
  return path.join(target.workspace_dir, ".build", "automatic-build", "theme-sessions",
    task.policy_generation_id, encodeURIComponent(task.descriptor.work_unit_id) + ".json");
}
function actionsFor(target: AutomaticBuildTarget, task: BookStructureGenerationTaskV1): ThemeAction[] {
  const file = journalPath(target, task);
  return existsSync(file) ? JSON.parse(readFileSync(file, "utf8")) : [];
}
function packetFor(target: AutomaticBuildTarget, task: BookStructureGenerationTaskV1,
  directory: StructureThemeDirectory, ordinal: number): ThemeInput {
  const initial = task.input as ThemeInput;
  const context = { ...initial.context };
  if (context.mode === "semantic_required") {
    const work = directory.works.find(w => w.plan.ref === initial.ref)!;
    const dependencies = retrievalDependencies(projectStructureRetrieval(context.catalog, context.chapters).records,
      { kind: "search", query: work.query }, context.mode, initial.retrieval_selection!.provider!.identity,
      STRUCTURE_RETRIEVAL_POLICY, STRUCTURE_RETRIEVAL.projection_version);
    const state = readBuildRetrievalState(target.workspace_dir);
    context.prepared = Object.values(state.prepared).find(p => retrievalPreparationMatches(p, dependencies));
    if (!context.prepared) throw new StructureThemeRetrievalRequired(target, {
      slot: `book-structure/session/${task.descriptor.work_unit_id}/${ordinal}`, dependencies,
    });
  }
  return { ...initial, context, directory, body: { ...structureThemeInput(directory, initial.ref, context), action_ordinal: ordinal } };
}
function replay(target: AutomaticBuildTarget, task: BookStructureGenerationTaskV1, actions: unknown[]) {
  if (task.output_role !== "organization_theme" || !("phase" in task.input) || task.input.phase !== "theme")
    throw new Error("whole-theme task required");
  let directory = task.input.directory;
  for (const [ordinal, action] of actions.entries()) {
    if (ordinal >= 128) throw new Error("theme action budget exhausted; accepted work retained");
    directory = applyStructureOrganizationAction(packetFor(target, task, directory, ordinal), action) as StructureThemeDirectory;
  }
  return directory;
}

/** Only this theme's journal advances. Its dispatch descriptor and siblings stay fixed. */
export function readStructureThemeSession(target: AutomaticBuildTarget, task: BookStructureGenerationTaskV1) {
  const actions = actionsFor(target, task);
  const directory = replay(target, task, actions.map(a => a.action));
  if (actions.length >= 128) throw new Error("theme action budget exhausted; accepted work retained");
  const packet = packetFor(target, task, directory, actions.length);
  return { packet, ordinal: actions.length, rendered: renderStructureOrganizationInput(packet) };
}

export function fitStructureThemeSession(target: AutomaticBuildTarget, task: BookStructureGenerationTaskV1,
  contract: Pick<BookStructureExecutionContractV2, "semantic_prompt" | "policy_fingerprint">, transport_profile: ExecutorTransportProfile) {
  const session = readStructureThemeSession(target, task);
  const fitted = fitStructureOrganizationExecution({ packet: session.packet, contract, transport_profile,
    budget: { stage_body_limit_tokens: 20000, executor_context_floor_tokens: 32768,
      output_reserve_tokens: 6500, max_candidate_tokens: Math.min(6500,
        createCandidateTransportContract(transport_profile).candidate_value_max_estimated_tokens), safety_margin_tokens: 512 } });
  if (fitted.evaluation.status !== "within_limit") throw new Error("theme input exceeds its execution budget; accepted work retained");
  return session;
}

/** Called under the task lease; replaying an acknowledged submission never applies it twice. */
export function checkpointStructureThemeAction(target: AutomaticBuildTarget, task: BookStructureGenerationTaskV1,
  inputRef: string, renderedInput: string, action: unknown): "continue" | "resolve" {
  const actions = actionsFor(target, task);
  const prior = actions.find(a => a.input_ref === inputRef);
  if (prior) {
    if (!isDeepStrictEqual(prior.action, action)) throw new Error("theme submission changed after acceptance");
    return "continue";
  }
  const current = readStructureThemeSession(target, task);
  if (current.rendered !== renderedInput) throw new Error("theme action input is stale");
  try { acceptStructureOrganizationAction(current.packet, action); }
  catch (error) { throw new StructureThemeActionRejected(error); }
  if ((action as { kind?: string }).kind === "resolve") return "resolve";
  // Reserve the last action for resolving, as the previous action-based router did.
  if (actions.length >= 127) throw new Error("theme action budget exhausted; accepted work retained");
  const file = journalPath(target, task);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file + ".tmp", JSON.stringify([...actions, { input_ref: inputRef, action }]) + "\n");
  renameSync(file + ".tmp", file);
  return "continue";
}

export function acceptStructureThemeCandidate(target: AutomaticBuildTarget, task: BookStructureGenerationTaskV1,
  candidate: unknown): StructureThemeAcceptance {
  acceptStructureOrganizationAction(readStructureThemeSession(target, task).packet, candidate);
  const actions = [...actionsFor(target, task).map(a => a.action), candidate];
  const acceptance: StructureThemeAcceptance = { version: "book_structure_theme_work.v1", actions };
  replayStructureThemeAcceptance(target, task, acceptance);
  return acceptance;
}
export function replayStructureThemeAcceptance(target: AutomaticBuildTarget, task: BookStructureGenerationTaskV1,
  value: unknown): StructureThemeDirectory {
  const acceptance = value as StructureThemeAcceptance;
  if (acceptance?.version !== "book_structure_theme_work.v1" || !Array.isArray(acceptance.actions))
    throw new Error("invalid theme acceptance");
  const directory = replay(target, task, acceptance.actions);
  if (!directory.works.find(w => w.plan.ref === (task.input as ThemeInput).ref)?.result)
    throw new Error("theme must resolve before committing its task");
  return directory;
}
