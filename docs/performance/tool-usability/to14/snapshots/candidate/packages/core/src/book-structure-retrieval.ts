import { createHash } from "node:crypto";
import type { StructureCandidateCatalog } from "./book-structure-candidates";
import type { StructureOutlineInput } from "./book-structure-planning";
import { prepareSemanticRetrieval, retrievalDependencies, retrievalPreparationMatches, type PreparedRetrieval } from "./semantic-retrieval-preparation";
import { missingRetrievalTexts } from "./semantic-retrieval-cache";
import { compareCatalogKeys, retrievalPage, type SemanticRetrievalRecord } from "./semantic-retrieval";

export const STRUCTURE_RETRIEVAL = { projection_version: "book_structure_retrieval_projection.v1", cache_namespace: "book-structure" };
export const STRUCTURE_RETRIEVAL_POLICY = { version: "book_structure_hybrid.v1", page_size: 6, semantic_top_k: 12 };
export type StructureRetrievalChapter = StructureOutlineInput["chapters"][number] & { question?: string };
export interface StructureRetrievalIndex {
  records: SemanticRetrievalRecord[];
  locations: Record<string, { kind: "chapter" | "candidate"; unit_lid: string; candidate_ref?: string; evidence_lids: string[]; embedding_truncated: boolean }>;
}
const prefix = (text: string, length: number) => Array.from(text).slice(0, length).join("");

/** Versioned, short semantic views; full fields remain searchable and inspectable. */
export function projectStructureRetrieval(catalog: StructureCandidateCatalog, chapters: StructureRetrievalChapter[]): StructureRetrievalIndex {
  const records: SemanticRetrievalRecord[] = [], locations: StructureRetrievalIndex["locations"] = {};
  const add = (key: string, meaning: string, aliases: string[], fields: string[], short: string[], location: Omit<StructureRetrievalIndex["locations"][string], "embedding_truncated">) => {
    const embedding_text = short.filter(Boolean).join("\n");
    records.push({ key, meaning, aliases, lexical_fields: fields, embedding_text,
      content_digest: createHash("sha256").update(embedding_text).digest("hex") });
    locations[key] = { ...location, embedding_truncated: fields.join("\n") !== embedding_text };
  };
  for (const c of chapters) add(`chapter:${c.unit_lid}`, c.question ?? c.overview.text, [],
    c.question ? [c.question, c.overview.text] : [c.overview.text],
    c.question ? [prefix(c.question, 48), prefix(c.overview.text, 40)] : [prefix(c.overview.text, 88)],
    { kind: "chapter", unit_lid: c.unit_lid, evidence_lids: c.overview.evidence_lids });
  for (const c of catalog.candidates) {
    const aliases = c.aliases ?? [], conditions = c.conditions.join("; ");
    add(c.ref, c.meaning, aliases, [c.meaning, ...c.conditions, ...aliases, c.reason.text],
      [prefix(c.meaning, 48), prefix(conditions, 24), prefix(aliases.join("; "), 12)],
      { kind: "candidate", unit_lid: c.unit_lid, candidate_ref: c.ref, evidence_lids: c.evidence_lids });
  }
  records.sort((a, b) => compareCatalogKeys(a.key, b.key));
  return { records, locations };
}
type PreparationInput = Omit<Parameters<typeof prepareSemanticRetrieval>[0], "records" | "projection_version" | "cache_namespace" | "policy"> & { index: StructureRetrievalIndex };
export async function prepareStructureRetrieval(input: PreparationInput): Promise<PreparedRetrieval> {
  // Validate the complete projection before any calls, including a later oversized batch.
  if (input.mode === "semantic_required" && input.provider && !(input.request.kind === "search" && !input.request.query.trim())) {
    for (const record of input.index.records) {
      const count = input.provider.count_tokens(record.embedding_text, "document");
      if (!Number.isInteger(count) || count < 1 || count > input.provider.limits.max_input_tokens)
        throw new Error(`structure projection exceeds tokenizer limit: ${record.key}; inspect full material or revise the projection`);
    }
  }
  return prepareSemanticRetrieval({ ...input, ...STRUCTURE_RETRIEVAL, records: input.index.records, policy: STRUCTURE_RETRIEVAL_POLICY });
}
export function missingStructureRetrievalTexts(workspace: string, index: StructureRetrievalIndex, identity: Parameters<typeof missingRetrievalTexts>[2]) {
  return missingRetrievalTexts(workspace, index.records, identity, STRUCTURE_RETRIEVAL);
}
export function structureRetrievalPage(index: StructureRetrievalIndex, prepared: PreparedRetrieval, request: Parameters<typeof retrievalDependencies>[1], offset = 0) {
  const deps = prepared.dependencies;
  if (!retrievalPreparationMatches(prepared, retrievalDependencies(index.records, request, deps.mode, deps.provider,
    STRUCTURE_RETRIEVAL_POLICY, STRUCTURE_RETRIEVAL.projection_version))) throw new Error("structure retrieval preparation unavailable or stale");
  const page = retrievalPage(prepared.sequence, offset, STRUCTURE_RETRIEVAL_POLICY);
  const byKey = new Map(index.records.map(r => [r.key, r]));
  return { ...page, items: page.items.map(hit => ({ ...hit, ...index.locations[hit.key],
    meaning: prefix(byKey.get(hit.key)!.meaning, 160) })),
    evidence_notice: "Search previews are locations, not delivered evidence. Inspect/read before making claims.",
    semantic_notice: "Semantic Top-K may omit relevant material; empty query browses the complete catalog. Short embeddings retain full lexical fields and material locations." };
}
