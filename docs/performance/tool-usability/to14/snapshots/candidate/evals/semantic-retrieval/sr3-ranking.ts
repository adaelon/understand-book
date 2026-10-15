import { readFileSync, writeFileSync } from "node:fs";
import { fixture, cases } from "../../packages/core/testdata/semantic-retrieval/gold";
import { projectRetrievalCatalog, retrievalCatalog, retrieveHybrid, RETRIEVAL_PROJECTION_VERSION } from "../../packages/core/src/semantic-retrieval";
import { rankSemanticRecords, type RetrievalVectors } from "../../packages/core/src/semantic-retrieval-cache";
import type { EmbeddingProviderIdentity } from "../../packages/core/src/embedding-provider";
const [input, output] = process.argv.slice(2);
const saved: { identity: EmbeddingProviderIdentity; documents: Array<{ key: string; content_digest: string; embedding_text: string; vector: number[] }>;
  query_vectors: Array<{ id: string; query: string; vector: number[] }> } = JSON.parse(readFileSync(input, "utf8"));
const { work, previous } = fixture(), records = projectRetrievalCatalog(retrievalCatalog(work.proposal, previous));
for (const r of records) if (saved.documents.find(d => d.key === r.key)?.embedding_text !== r.embedding_text) throw new Error("probe projection changed");
const vectors: RetrievalVectors = { version: "semantic_retrieval_vectors.v1", provider: saved.identity, projection_version: RETRIEVAL_PROJECTION_VERSION,
  records: saved.documents.map(({ key, content_digest }) => ({ key, content_digest })), vectors: Object.fromEntries(saved.documents.map(r => [r.content_digest, r.vector])) };
const metric = (keys: string[], relevant: string[], k: number) => {
  const hits = relevant.filter(key => keys.slice(0, k).includes(key)).length;
  return { hits, recall: hits / relevant.length, precision_at_k: hits / k };
};
const rows = cases.map(c => {
  const q = saved.query_vectors.find(q => q.id === c.id);
  if (!q || q.query !== c.query) throw new Error("probe query changed");
  const request = { kind: "search" as const, query: c.query, focus_key: c.focus };
  const semantic = rankSemanticRecords(records, vectors, { provider: saved.identity, vector: q.vector }, c.focus);
  const B = retrieveHybrid(records, request), C = retrieveHybrid(records, request, semantic);
  return { id: c.id, compare: c.compare, B, C, metrics: Object.fromEntries([["B", B], ["C", C]].map(([label, seq]) => [label,
    Object.fromEntries([6, 12].map(k => [k, metric((seq as typeof B).hits.map(h => h.key), c.compare, k)]))])) };
});
const aggregate = Object.fromEntries(["B", "C"].map(group => [group, Object.fromEntries([6, 12].map(k => [k, {
  recall: rows.reduce((sum, r) => sum + r.metrics[group][k].recall, 0) / rows.length,
  precision_at_k: rows.reduce((sum, r) => sum + r.metrics[group][k].precision_at_k, 0) / rows.length,
}]))]));
writeFileSync(output, JSON.stringify({ recorded_at: new Date().toISOString(), inference: "reused SR1 saved real local vectors; zero new provider calls",
  aggregate, rows, identity_quality: null }, null, 2) + "\n");
console.log(JSON.stringify(aggregate, null, 2));
