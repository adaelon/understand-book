import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { retrievalScopeIncludes, type BuildPlanV1, type BuildRetrievalSelection } from "./build-intent";
import { sameEmbeddingIdentity, type EmbeddingProvider } from "./embedding-provider";
import { STRUCTURE_RETRIEVAL } from "./book-structure-retrieval";
import { RETRIEVAL_PROJECTION_VERSION } from "./semantic-retrieval";
import { missingRetrievalTexts } from "./semantic-retrieval-cache";
import { retrievalQuery } from "./semantic-retrieval";
import { prepareSemanticRetrieval, retrievalPreparationMatches, type PreparedRetrieval, type RetrievalDependencies } from "./semantic-retrieval-preparation";
import { retrievalBudgetViolation, summarizeRetrievalUsage, type AutomaticBuildRetrievalCall } from "./automatic-build-budget";

function cacheOptions(d: RetrievalDependencies) {
  return { projection_version: d.projection_version, ...(d.projection_version === STRUCTURE_RETRIEVAL.projection_version ? { cache_namespace: STRUCTURE_RETRIEVAL.cache_namespace } : {}) };
}
export interface TeachingRetrievalRequest { slot: string; dependencies: RetrievalDependencies }
export interface BuildRetrievalRuntime {
  selection: BuildRetrievalSelection;
  provider?: EmbeddingProvider;
  open_provider?: (signal?: AbortSignal) => Promise<{ provider: EmbeddingProvider; dispose: () => Promise<void> }>;
}
interface RetrievalState {
  selection?: BuildRetrievalSelection;
  prepared: Record<string, PreparedRetrieval>;
  calls: Array<AutomaticBuildRetrievalCall & { plan_id: string; plan_revision: number; plan_digest: string }>;
  failure?: { reason: "provider_failed" | "cancelled" | "build_plan_budget_changed" | "build_plan_retrieval_drift"; message: string };
}
const file = (workspace: string) => path.join(workspace, ".build", "teaching", "retrieval.json");
export function readBuildRetrievalState(workspace: string): RetrievalState {
  return existsSync(file(workspace)) ? JSON.parse(readFileSync(file(workspace), "utf8")) : { prepared: {}, calls: [] };
}
function save(workspace: string, state: RetrievalState) {
  const target = file(workspace);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(`${target}.tmp`, JSON.stringify(state));
  renameSync(`${target}.tmp`, target);
}
export function retrievalRemainingWork(workspace: string, request: TeachingRetrievalRequest, previous?: PreparedRetrieval) {
  const d = request.dependencies, query = retrievalQuery(d.records, d.request);
  if (retrievalPreparationMatches(previous, d) || d.mode === "lexical_only" || query.browse)
    return { records: d.records.length, documents: 0, queries: 0, calls: 0 };
  const documents = missingRetrievalTexts(workspace, d.records, d.provider!, cacheOptions(d)).length;
  const queries = previous?.query_embedding && sameEmbeddingIdentity(previous.query_embedding.provider, d.provider!)
    && previous.query_embedding.text === query.embedding ? 0 : 1;
  // Batch size belongs to the adapter; ordinary reads cannot promise a precise call count.
  return { records: d.records.length, documents, queries, calls: null };
}
export function retrievalConfigurationMatches(plan: BuildPlanV1, runtime: BuildRetrievalRuntime): boolean {
  return isDeepStrictEqual(plan.retrieval?.selection, runtime.selection)
    && (!runtime.provider || runtime.selection.retrieval_mode === "lexical_only"
      || sameEmbeddingIdentity(runtime.provider.identity, runtime.selection.provider!.identity));
}

/** Only invoked after the orchestrator has validated the confirmed plan and reachable stage. */
export async function prepareBuildRetrieval(input: { workspace: string; plan: BuildPlanV1; runtime: BuildRetrievalRuntime;
  request: TeachingRetrievalRequest; signal?: AbortSignal; model_tokens?: number }) {
  const { workspace, plan, runtime, request } = input;
  if (plan.status !== "confirmed" || !retrievalConfigurationMatches(plan, runtime)) throw new Error("retrieval authorization mismatch");
  const state = readBuildRetrievalState(workspace), d = request.dependencies;
  const consumer = d.projection_version === STRUCTURE_RETRIEVAL.projection_version ? "book_structure"
    : d.projection_version === RETRIEVAL_PROJECTION_VERSION ? "formal_objects" : undefined;
  if (!consumer || !retrievalScopeIncludes(runtime.selection.data_scope, consumer)
    || !plan.public_stage_closure.includes(consumer === "book_structure" ? "book_structure" : "formal_objects")
    || d.mode !== runtime.selection.retrieval_mode
    || (d.mode === "semantic_required" && !sameEmbeddingIdentity(d.provider!, runtime.selection.provider!.identity)))
    throw new Error("retrieval request outside confirmed consumer scope");
  const previous = state.prepared[request.slot] ?? Object.values(state.prepared).filter(p => p.dependencies.projection_version === d.projection_version).at(-1);
  if (retrievalPreparationMatches(previous, d)) return;
  state.selection = structuredClone(runtime.selection);
  let provider = runtime.provider;
  let opened: Awaited<ReturnType<NonNullable<BuildRetrievalRuntime["open_provider"]>>> | undefined;
  const query = retrievalQuery(d.records, d.request);
  const usage = () => summarizeRetrievalUsage(state.calls.filter(c => c.plan_id === plan.plan_id));
  const controller = new AbortController();
  const abort = () => controller.abort(input.signal?.reason);
  if (input.signal?.aborted) abort();
  else input.signal?.addEventListener("abort", abort, { once: true });
  const started = performance.now(), priorElapsed = usage().elapsed_ms;
  const elapsed = () => priorElapsed + performance.now() - started;
  let timer: ReturnType<typeof setTimeout> | undefined, wallExceeded = false;
  let budgetFailure = false, configurationFailure = false;
  const check = (next: { documents: number; queries: number; calls: number; input_tokens: number }) => {
    controller.signal.throwIfAborted();
    const violation = retrievalBudgetViolation(plan, usage(), next, input.model_tokens, elapsed());
    if (violation) { budgetFailure = true; throw new Error(violation); }
  };
  try {
    controller.signal.throwIfAborted();
    if (d.mode === "semantic_required" && !query.browse) {
      // Loading weights is owned by this authorized call, never by preview or a writer.
      check({ documents: 0, queries: 0, calls: 0, input_tokens: 0 });
      if (plan.budget.max_wall_clock_minutes !== undefined) timer = setTimeout(() => {
        wallExceeded = true; controller.abort(new Error("build plan max_wall_clock_minutes exceeded"));
      }, Math.min(2_147_483_647, Math.max(1, (plan.budget.max_wall_clock_minutes - (plan.estimate.wall_clock_minutes.p95 ?? 0)) * 60000 - elapsed())));
      if (!provider && runtime.open_provider) {
        opened = await runtime.open_provider(controller.signal);
        provider = opened.provider;
      }
      if (!provider) throw new Error("semantic retrieval provider unavailable");
      if (!retrievalConfigurationMatches(plan, { selection: runtime.selection, provider })) {
        configurationFailure = true; throw new Error("retrieval configuration differs from confirmed plan");
      }
      const texts = missingRetrievalTexts(workspace, d.records, d.provider!, cacheOptions(d));
      const remaining = retrievalRemainingWork(workspace, request, previous);
      const activeProvider = provider;
      const count = (text: string, role: "document" | "query") => {
        const n = activeProvider.count_tokens(text, role);
        if (!Number.isSafeInteger(n) || n < 1 || n > activeProvider.limits.max_input_tokens) throw new Error("embedding input token limit exceeded");
        return n;
      };
      if (!Number.isSafeInteger(provider.limits.max_batch_size) || provider.limits.max_batch_size < 1) throw new Error("invalid embedding batch limit");
      check({ documents: texts.length, queries: remaining.queries, calls: Math.ceil(texts.length / provider.limits.max_batch_size) + remaining.queries,
        input_tokens: texts.reduce((sum, text) => sum + count(text, "document"), 0) + (remaining.queries ? count(query.embedding, "query") : 0) });
    }
    const prepared = await prepareSemanticRetrieval({ workspace, ...cacheOptions(d), records: d.records, request: d.request, mode: d.mode,
      provider_identity: d.provider ?? undefined, provider, policy: d.policy, previous, signal: controller.signal,
      before_call: call => {
        if (!retrievalConfigurationMatches(plan, { selection: runtime.selection, provider })) {
          configurationFailure = true; throw new Error("retrieval configuration changed during preparation");
        }
        check({ documents: call.role === "document" ? call.records : 0, queries: call.role === "query" ? call.records : 0,
          calls: 1, input_tokens: call.input_tokens });
        state.calls.push({ plan_id: plan.plan_id, plan_revision: plan.revision, plan_digest: plan.plan_digest,
          role: call.role, records: call.records, reserved_input_tokens: call.input_tokens, status: "failed", elapsed_ms: 0 });
        save(workspace, state);
      },
      on_call: call => { Object.assign(state.calls.at(-1)!, call); save(workspace, state); },
    });
    if (d.mode === "semantic_required" && !query.browse) check({ documents: 0, queries: 0, calls: 0, input_tokens: 0 });
    state.prepared[request.slot] = prepared;
    delete state.failure;
    save(workspace, state);
  } catch (error) {
    if (error && typeof error === "object" && "code" in error && error.code === "build_plan_retrieval_drift") configurationFailure = true;
    state.failure = { reason: configurationFailure ? "build_plan_retrieval_drift" : budgetFailure || wallExceeded ? "build_plan_budget_changed" : controller.signal.aborted ? "cancelled" : "provider_failed",
      message: error instanceof Error ? error.message : String(error) };
    save(workspace, state);
    return state.failure;
  } finally {
    if (timer) clearTimeout(timer);
    input.signal?.removeEventListener("abort", abort);
    await opened?.dispose();
  }
}
