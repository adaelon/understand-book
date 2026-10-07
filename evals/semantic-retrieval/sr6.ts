import { readFileSync } from "node:fs";
import { isDeepStrictEqual } from "node:util";
import { fixture, cases, policy } from "../../packages/core/testdata/semantic-retrieval/gold";
import { advanceObjectAlignment, alignmentFocus, alignmentRetrievalRequest, objectAlignmentInput, type ObjectAlignmentWork } from "../../packages/core/src/teaching-object-alignment";
import { projectRetrievalCatalog, retrievalCatalog, retrieveHybrid, type RetrievalSequence } from "../../packages/core/src/semantic-retrieval";
import { rankSemanticRecords, type RetrievalVectors } from "../../packages/core/src/semantic-retrieval-cache";
import type { EmbeddingProviderIdentity } from "../../packages/core/src/embedding-provider";
import { retrievalPreparationMatches, type PreparedRetrieval, type RetrievalDependencies } from "../../packages/core/src/semantic-retrieval-preparation";

export const GROUPS = ["A", "B", "C"] as const;
export type Group = typeof GROUPS[number];
export const decisionBudget = { max_steps: 384, max_actions_without_progress: 32 };

/** Freeze the actual prompt and data, not a second diagnostic fingerprint. */
export function experimentContract(prompt: string, options: { thinking?: "disabled"; max_steps?: number } = {}) {
  return { version: "sr6-gold.v1", groups: GROUPS, gold_policy: policy, cases,
    fixture: fixture(), prompt, decision_budget: { ...decisionBudget, max_steps: options.max_steps ?? decisionBudget.max_steps },
    ...(options.thinking ? { thinking: { type: options.thinking } } : {}),
    temperature: 0, max_output_tokens: 4096,
    read_gate: "advanceObjectAlignment/formal_object_alignment.v3",
    protocol: "Three complete, independent alignments; fixed-query retrieval scored separately on the unchanged initial catalog. No gold labels enter model input." };
}

export function legacySequence(work: ObjectAlignmentWork, previous: ReturnType<typeof fixture>["previous"], query: string): RetrievalSequence {
  const terms = query.toLocaleLowerCase().split(/\s+/u).filter(Boolean);
  const hits = retrievalCatalog(work.proposal, previous).filter(r => !terms.length
    || terms.some(t => JSON.stringify(r.object).toLocaleLowerCase().includes(t)))
    .map(r => ({ key: r.key, match_reasons: [] }));
  return { hits, candidate_count: hits.length, semantic_contributed: 0, semantic_truncated: false };
}

export function rankingReport(saved: { identity: EmbeddingProviderIdentity; documents: Array<{ key: string; content_digest: string; embedding_text: string; vector: number[] }>;
  query_vectors: Array<{ id: string; query: string; vector: number[] }> }) {
  const { work, previous } = fixture(), records = projectRetrievalCatalog(retrievalCatalog(work.proposal, previous));
  if (saved.documents.length !== records.length) throw new Error("saved document inventory changed");
  for (const r of records) {
    const old = saved.documents.find(d => d.key === r.key);
    if (!old || old.embedding_text !== r.embedding_text || old.content_digest !== r.content_digest) throw new Error("saved projection changed");
  }
  const vectors: RetrievalVectors = { version: "semantic_retrieval_vectors.v1", provider: saved.identity,
    projection_version: "formal_object_retrieval_projection.v1", records: saved.documents.map(({ key, content_digest }) => ({ key, content_digest })),
    vectors: Object.fromEntries(saved.documents.map(r => [r.content_digest, r.vector])) };
  const rows = cases.map(c => {
    const q = saved.query_vectors.find(q => q.id === c.id);
    if (!q || q.query !== c.query) throw new Error("saved query changed");
    const request = { kind: "search" as const, query: c.query, focus_key: c.focus };
    const sequences = { A: legacySequence(work, previous, c.query), B: retrieveHybrid(records, request),
      C: retrieveHybrid(records, request, rankSemanticRecords(records, vectors, { provider: saved.identity, vector: q.vector }, c.focus)) };
    const metrics = Object.fromEntries(GROUPS.map(group => [group, Object.fromEntries([6, 12].map(k => {
      const keys = sequences[group].hits.slice(0, k).map(h => h.key), hits = c.compare.filter(key => keys.includes(key)).length;
      return [k, { hits, recall: hits / c.compare.length, precision_at_k: hits / k }];
    }))]));
    const exact = sequences.B.hits.filter(h => h.match_reasons.some(r => r === "alias" || r === "exact_meaning")).map(h => h.key);
    return { id: c.id, category: c.category, sequences, metrics,
      exact_retained: isDeepStrictEqual(sequences.C.hits.slice(0, exact.length).map(h => h.key), exact) };
  });
  const aggregate = Object.fromEntries(GROUPS.map(group => [group, Object.fromEntries([6, 12].map(k => [k, {
    recall: rows.reduce((n, r) => n + r.metrics[group][k].recall, 0) / rows.length,
    precision_at_k: rows.reduce((n, r) => n + r.metrics[group][k].precision_at_k, 0) / rows.length,
  }]))]));
  return { provenance: "SR1 saved real MiniLM vectors; zero new inference", identity: saved.identity, rows, aggregate };
}

const canonical = (work: ObjectAlignmentWork, key: string) => work.redirects[key] ?? key;
function sameIdentity(work: ObjectAlignmentWork, left: string, right: string): boolean {
  const object = work.proposal.objects.find(o => o.key === canonical(work, left));
  if (!object) throw new Error(`final catalog lost ${left}`);
  if (right.startsWith("previous/")) return object.existing_ref?.object_id === decodeURIComponent(right.slice(9));
  return object.key === canonical(work, right);
}

/** Labels judge accepted state; neither cosine nor an LLM grades these pairs. */
export function identityQuality(work: ObjectAlignmentWork) {
  if (!work.result) return null;
  const rows = cases.map(c => ({ id: c.id,
    false_merge: c.separate.filter(key => sameIdentity(work, c.focus, key)),
    missed_reuse_or_false_split: c.same.filter(key => !sameIdentity(work, c.focus, key)),
  }));
  // Report unique labeled pairs, while retaining every case row (some labels intentionally repeat).
  const pairs = (field: "false_merge" | "missed_reuse_or_false_split") => [...new Set(rows.flatMap((r, i) =>
    r[field].map(key => [cases[i].focus, key].sort().join(" | "))))];
  return { rows, false_merge: pairs("false_merge"), missed_reuse_or_false_split: pairs("missed_reuse_or_false_split") };
}

export interface GoldCall {
  input: string; request_input?: string; action?: unknown; response?: string; error?: string;
  reported_model?: string; finish_reason?: string;
  status: "reserved" | "received" | "accepted" | "rejected" | "failed";
  input_tokens?: number; output_tokens?: number; elapsed_ms?: number;
}
export interface GoldRun { group: Group; work: ObjectAlignmentWork; calls: GoldCall[] }
export function goldInput(run: GoldRun, prepared?: PreparedRetrieval) {
  const f = fixture();
  return JSON.stringify(objectAlignmentInput(run.work, f.source, f.previous, prepared));
}
export function goldDependencies(run: GoldRun) {
  const f = fixture();
  return { records: projectRetrievalCatalog(retrievalCatalog(run.work.proposal, f.previous)), request: alignmentRetrievalRequest(run.work, f.previous) };
}
export function preparedGoldPage(pages: Record<string, PreparedRetrieval>, slot: string, dependencies: RetrievalDependencies) {
  const prepared = pages[slot] ?? Object.values(pages).at(-1);
  if (!prepared || !retrievalPreparationMatches(prepared, dependencies)) throw new Error("prepared C page unavailable");
  return prepared;
}
export function acceptGoldAction(run: GoldRun, action: unknown, prepared?: PreparedRetrieval) {
  const f = fixture();
  const focus = alignmentFocus(run.work, f.previous);
  const kind = (action as { kind?: string })?.kind;
  if (run.work.steps_since_progress >= decisionBudget.max_actions_without_progress && ["search", "inspect", "read"].includes(kind ?? ""))
    throw new Error("alignment decision budget exhausted");
  const next = advanceObjectAlignment({ work: run.work, source: f.source, previous: f.previous,
    action, retrieval: prepared, operation_id: `sr6-${run.group}` });
  if (focus.kind === "finish" && !next.result) throw new Error("finish did not materialize");
  return next;
}

export function goldSummary(run: GoldRun) {
  const accepted = run.calls.filter(c => c.status === "accepted");
  return { group: run.group, complete: !!run.work.result, quality: identityQuality(run.work),
    model_calls: run.calls.length, alignment_steps: accepted.length,
    inspect: accepted.filter(c => (c.action as { kind?: string })?.kind === "inspect").length,
    read: accepted.filter(c => (c.action as { kind?: string })?.kind === "read").length,
    rejected: run.calls.filter(c => c.status === "rejected").length,
    known_input_tokens: run.calls.reduce((n, c) => n + (c.input_tokens ?? 0), 0),
    known_output_tokens: run.calls.reduce((n, c) => n + (c.output_tokens ?? 0), 0),
    unknown_usage_calls: run.calls.filter(c => c.input_tokens === undefined || c.output_tokens === undefined).length,
    reported_models: [...new Set(run.calls.flatMap(c => c.reported_model ? [c.reported_model] : []))],
    model_elapsed_ms: run.calls.reduce((n, c) => n + (c.elapsed_ms ?? 0), 0) };
}
export interface LongMaterialEvidence {
  source: string; report: string;
  source_review_passed: boolean; provider_failure_resume: boolean; cache_rebuild: boolean;
  config_change_resume: boolean; stable_identity: boolean; reader_ready: boolean;
  ordinary_read_provider_free: boolean; pass2_enabled: boolean; pass2_disabled: boolean; small_material_provider_free: boolean;
}
export function releaseDecision(ranking: ReturnType<typeof rankingReport>, runs: GoldRun[], longMaterial?: LongMaterialEvidence) {
  const reasons: string[] = [];
  if (!GROUPS.every(g => runs.filter(r => r.group === g).length === 1 && runs.find(r => r.group === g)!.work.result)) reasons.push("three_complete_agent_runs_required");
  if (ranking.aggregate.C[6].recall <= ranking.aggregate.B[6].recall) reasons.push("no_recall_gain_over_lexical");
  if (ranking.rows.some(r => !r.exact_retained)) reasons.push("exact_alias_regression");
  const b = runs.find(r => r.group === "B"), c = runs.find(r => r.group === "C");
  const bq = b && identityQuality(b.work), cq = c && identityQuality(c.work);
  if (cq?.false_merge.length) reasons.push("separated_gold_pair_merged");
  if (bq && cq && cq.missed_reuse_or_false_split.length > bq.missed_reuse_or_false_split.length) reasons.push("identity_reuse_regression");
  if (!longMaterial || !longMaterial.source || !longMaterial.report
    || Object.entries(longMaterial).some(([k, value]) => k !== "source" && k !== "report" && value !== true)) reasons.push("long_material_acceptance_incomplete");
  return { eligible: reasons.length === 0, default_mode: reasons.length ? "lexical_only" : "semantic_required", reasons };
}

export function readRanking(file: string) { return rankingReport(JSON.parse(readFileSync(file, "utf8"))); }
