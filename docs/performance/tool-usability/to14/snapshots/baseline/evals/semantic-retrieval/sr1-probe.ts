import { writeFileSync } from "node:fs";
import { localProvider } from "./local-provider";
import { fixture, cases, policy } from "../../packages/core/testdata/semantic-retrieval/gold";
import { projectRetrievalCatalog, retrievalCatalog } from "../../packages/core/src/semantic-retrieval";
import { callEmbedding, type EmbeddingCall } from "../../packages/core/src/embedding-provider";

const [runtime, model, output] = process.argv.slice(2);
if (!output) throw new Error("Usage: sr1-probe.ts RUNTIME MODEL OUTPUT");
const { work, previous } = fixture();
const records = projectRetrievalCatalog(retrievalCatalog(work.proposal, previous));
if (records.length > policy.max_documents || cases.length > policy.max_queries) throw new Error("fixed Gold probe budget exceeded");
const start = performance.now(), local = await localProvider(runtime, model), calls: EmbeddingCall[] = [];
try {
  const p = local.provider, vectors: number[][] = [], queries: number[][] = [];
  for (let i = 0; i < records.length; i += p.limits.max_batch_size)
    vectors.push(...await callEmbedding(p, p.identity, "document", records.slice(i, i + 8).map(r => r.embedding_text), { on_call: c => calls.push(c) }));
  for (const c of cases) queries.push(...await callEmbedding(p, p.identity, "query", [c.query], { on_call: c => calls.push(c) }));
  const result = { version: "sr1-probe.v1", recorded_at: new Date().toISOString(), identity: p.identity, limits: p.limits,
    records: records.length, queries: cases.length, max_tokens: Math.max(...records.map(r => p.count_tokens(r.embedding_text, "document"))),
    calls, total_ms: performance.now() - start, remote_calls: 0,
    // Saved vectors let SR2/SR3 compare the same inference without spending another probe budget.
    documents: records.map((r, i) => ({ ...r, vector: vectors[i] })), query_vectors: cases.map((c, i) => ({ id: c.id, query: c.query, vector: queries[i] })) };
  writeFileSync(output, JSON.stringify(result, null, 2) + "\n");
  console.log(JSON.stringify({ records: result.records, queries: result.queries, max_tokens: result.max_tokens, calls: calls.length, total_ms: result.total_ms }));
} finally { await local.dispose(); }
