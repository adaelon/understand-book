import { writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { fixture } from "../../packages/core/testdata/semantic-retrieval/gold";
import { projectRetrievalCatalog, retrievalCatalog, RETRIEVAL_PROJECTION_VERSION } from "../../packages/core/src/semantic-retrieval";
import { rankSemanticRecords, type RetrievalVectors } from "../../packages/core/src/semantic-retrieval-cache";
const results = [];
for (const size of [3000, 10000]) {
  const { work } = fixture();
  work.proposal.objects = Array.from({ length: size }, (_, i) => ({ ...work.proposal.objects[0], key: `object/${i.toString().padStart(5, "0")}`,
    meaning: `物理量 ${i} 的独立定义`, participants: [], component_keys: [] }));
  global.gc?.();
  const before = process.memoryUsage(), started = performance.now();
  const records = projectRetrievalCatalog(retrievalCatalog(work.proposal));
  const projection_ms = performance.now() - started;
  const cache: RetrievalVectors = { version: "semantic_retrieval_vectors.v1", projection_version: RETRIEVAL_PROJECTION_VERSION,
    provider: { provider_id: "scale-synthetic", model_id: "384d", model_revision: null, embedding_config: {}, dimensions: 384 },
    records: records.map(({ key, content_digest }) => ({ key, content_digest })),
    vectors: Object.fromEntries(records.map((r, i) => [r.content_digest, Array.from({ length: 384 }, (_, j) => ((i + j * 13) % 71 + 1) / 71)])) };
  const cosineStart = performance.now();
  const ranked = rankSemanticRecords(records, cache, { provider: cache.provider, vector: Array(384).fill(0.1) });
  const cosine_ms = performance.now() - cosineStart, after = process.memoryUsage();
  if (ranked.length !== size) throw new Error("scale rank lost records");
  results.push({ size, dimensions: 384, projection_ms, cosine_ms, heap_delta_bytes: after.heapUsed - before.heapUsed,
    rss_delta_bytes: after.rss - before.rss, cache_json_bytes: Buffer.byteLength(JSON.stringify(cache)) });
}
const result = { recorded_at: new Date().toISOString(), cpu: cpus()[0]?.model, node: process.version,
  scope: "synthetic vectors; one measurement per size; excludes embedding inference and persistence", results };
writeFileSync(process.argv[2], JSON.stringify(result, null, 2) + "\n");
console.log(JSON.stringify(result, null, 2));
