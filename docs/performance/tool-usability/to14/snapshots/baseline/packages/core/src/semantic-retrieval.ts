import { createHash } from "node:crypto";
import type { FormalObjectProposal, FormalObjects, LearningObjectRevision, LearningObjectRef } from "./teaching-map";

export const RETRIEVAL_PROJECTION_VERSION = "formal_object_retrieval_projection.v1";
export interface SemanticRetrievalRecord {
  key: string;
  meaning: string;
  aliases: string[];
  lexical_fields: string[];
  embedding_text: string;
  content_digest: string;
}
export type RetrievalCatalogEntry = { key: string; object: FormalObjectProposal["objects"][number] | LearningObjectRevision };
export const compareCatalogKeys = (a: string, b: string) => a < b ? -1 : a > b ? 1 : 0;
const refKey = (ref: LearningObjectRef) => JSON.stringify([ref.source_id, ref.object_id]);
export const previousCatalogKey = (id: string) => `previous/${encodeURIComponent(id)}`;

export function retrievalCatalog(proposal: FormalObjectProposal, previous?: FormalObjects): RetrievalCatalogEntry[] {
  const revisions = new Map<string, LearningObjectRevision>();
  for (const object of previous?.objects ?? []) {
    const key = refKey(object.ref), old = revisions.get(key);
    if (!old || object.object_revision > old.object_revision) revisions.set(key, object);
  }
  return [...proposal.objects.map(object => ({ key: object.key, object })), ...(previous?.active_refs ?? []).map(ref => {
    const object = revisions.get(refKey(ref));
    if (!object) throw new Error("retrieval prior reference unavailable");
    return { key: previousCatalogKey(ref.object_id), object };
  })];
}

// v1: preserve field/role order; one-hop summaries cap at 256 Unicode code points,
// then cap the entire embedding prefix at 1024. Full lexical fields remain intact.
// The adapter's real tokenizer rejects any remaining >128-token MiniLM input;
// there is no SDK truncation, estimated token count, or provider-dependent projection.
const prefix = (text: string, limit: number) => Array.from(text).slice(0, limit).join("");
export function projectRetrievalCatalog(catalog: RetrievalCatalogEntry[]): SemanticRetrievalRecord[] {
  const current = new Map(catalog.filter(r => "key" in r.object).map(r => [r.key, r.object]));
  const previous = new Map(catalog.flatMap(r => "ref" in r.object ? [[refKey(r.object.ref), r.object] as const] : []));
  const fields = (o: RetrievalCatalogEntry["object"]) => [o.kind, o.meaning, ...o.aliases, ...o.conditions];
  return catalog.map(({ key, object }) => {
    const summary = (target: string | LearningObjectRef) => {
      const o = typeof target === "string" ? current.get(target) : previous.get(refKey(target));
      if (!o) throw new Error("retrieval participant or component unavailable");
      return fields(o).join("; ");
    };
    const roles = object.participants.map(p => `${p.role}: ${summary("object_key" in p ? p.object_key : p.object_ref)}`);
    const components = "component_keys" in object ? object.component_keys.map(summary) : object.component_refs.map(summary);
    const lexical_fields = [...fields(object), ...roles, ...components];
    const embedding_text = prefix([...fields(object), ...roles.map(s => prefix(s, 256)), ...components.map(s => prefix(s, 256))].join("\n"), 1024);
    return { key, meaning: object.meaning, aliases: [...object.aliases], lexical_fields, embedding_text,
      content_digest: createHash("sha256").update(embedding_text).digest("hex") };
  }).sort((a, b) => compareCatalogKeys(a.key, b.key));
}

export type MatchReason = "exact_meaning" | "alias" | "lexical" | "semantic";
export interface RetrievalHit { key: string; match_reasons: MatchReason[] }
export interface RetrievalPolicy { version: string; page_size: number; semantic_top_k: number }
export const RETRIEVAL_POLICY: RetrievalPolicy = { version: "formal_object_hybrid.v1", page_size: 6, semantic_top_k: 12 };
export type RetrievalRequest = { kind: "focus"; focus_key: string } | { kind: "search"; query: string; focus_key?: string };
export interface RetrievalSequence {
  hits: RetrievalHit[];
  candidate_count: number;
  semantic_contributed: number;
  semantic_truncated: boolean;
}
export function retrievalQuery(records: SemanticRetrievalRecord[], request: RetrievalRequest) {
  if (request.kind === "search") return { embedding: request.query, exact: [request.query], lexical: request.query, browse: !request.query.trim() };
  const focus = records.find(r => r.key === request.focus_key);
  if (!focus) throw new Error("retrieval focus unavailable");
  return { embedding: focus.embedding_text, exact: [focus.meaning, ...focus.aliases], lexical: [focus.meaning, ...focus.aliases].join(" "), browse: false };
}

/** Canonical catalog records are the authority; retired/redirect source keys cannot occupy a lane. */
export function retrieveHybrid(records: SemanticRetrievalRecord[], request: RetrievalRequest,
  semantic: Array<{ key: string; cosine: number }> = [], policy: RetrievalPolicy = RETRIEVAL_POLICY): RetrievalSequence {
  if (!Number.isInteger(policy.page_size) || policy.page_size < 1 || !Number.isInteger(policy.semantic_top_k) || policy.semantic_top_k < 0)
    throw new Error("invalid retrieval policy");
  const query = retrievalQuery(records, request);
  const current = [...new Map(records.map(r => [r.key, r])).values()].sort((a, b) => compareCatalogKeys(a.key, b.key));
  if (query.browse) return { hits: current.map(r => ({ key: r.key, match_reasons: [] })), candidate_count: current.length,
    semantic_contributed: 0, semantic_truncated: false };
  const valid = current.filter(r => r.key !== request.focus_key), keys = new Set(valid.map(r => r.key));
  const normalize = (s: string) => s.trim().toLowerCase();
  const exactTerms = new Set(query.exact.map(normalize).filter(Boolean));
  const terms = query.lexical.toLowerCase().split(/\s+/u).filter(Boolean);
  const reasons = new Map<string, Set<MatchReason>>();
  for (const r of valid) {
    const matches = new Set<MatchReason>();
    if (exactTerms.has(normalize(r.meaning))) matches.add("exact_meaning");
    if (r.aliases.some(a => exactTerms.has(normalize(a)))) matches.add("alias");
    if (terms.some(t => r.lexical_fields.some(f => f.toLowerCase().includes(t)))) matches.add("lexical");
    reasons.set(r.key, matches);
  }
  const exact = valid.filter(r => reasons.get(r.key)!.has("exact_meaning") || reasons.get(r.key)!.has("alias")).map(r => r.key);
  const lexical = valid.filter(r => reasons.get(r.key)!.has("lexical")).map(r => r.key);
  // Filter and deduplicate before quota, ordered by score and canonical key even for supplied vectors.
  const ranked = [...new Map(semantic.filter(r => keys.has(r.key)).sort((a, b) => b.cosine - a.cosine || compareCatalogKeys(a.key, b.key))
    .map(r => [r.key, r])).values()].map(r => r.key);
  const order = [...exact], seen = new Set(exact);
  let li = 0, si = 0, contributed = 0;
  while (li < lexical.length || si < ranked.length && contributed < policy.semantic_top_k) {
    while (li < lexical.length && seen.has(lexical[li])) li++;
    if (li < lexical.length) { const key = lexical[li++]; order.push(key); seen.add(key); }
    while (si < ranked.length && contributed < policy.semantic_top_k) {
      const key = ranked[si++];
      reasons.get(key)!.add("semantic");
      if (seen.has(key)) continue;
      order.push(key); seen.add(key); contributed++; break;
    }
  }
  const reasonOrder: MatchReason[] = ["exact_meaning", "alias", "lexical", "semantic"];
  return { hits: order.map(key => ({ key, match_reasons: reasonOrder.filter(r => reasons.get(key)!.has(r)) })),
    candidate_count: order.length, semantic_contributed: contributed, semantic_truncated: ranked.some(key => !seen.has(key)) };
}
export function retrievalPage(sequence: RetrievalSequence, offset: number, policy: RetrievalPolicy = RETRIEVAL_POLICY) {
  if (!Number.isInteger(offset) || offset < 0 || offset > sequence.hits.length) throw new Error("retrieval offset out of range");
  const items = sequence.hits.slice(offset, offset + policy.page_size);
  return { items, offset, next_offset: offset + items.length < sequence.hits.length ? offset + items.length : null,
    candidate_count: sequence.candidate_count, semantic_truncated: sequence.semantic_truncated };
}
