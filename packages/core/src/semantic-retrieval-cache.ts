import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { callEmbedding, sameEmbeddingIdentity, validEmbeddingVector, type EmbeddingProvider, type EmbeddingProviderIdentity, type EmbeddingCallOptions } from "./embedding-provider";
import { RETRIEVAL_PROJECTION_VERSION, compareCatalogKeys, type SemanticRetrievalRecord } from "./semantic-retrieval";

export interface RetrievalVectors {
  version: "semantic_retrieval_vectors.v1";
  provider: EmbeddingProviderIdentity;
  projection_version: string;
  records: Array<{ key: string; content_digest: string }>;
  vectors: Record<string, number[]>;
}
export interface RetrievalCacheOptions { projection_version?: string; cache_namespace?: string }
const cacheFile = (workspace: string, namespace?: string) => path.join(workspace, ".build", "semantic-retrieval", namespace ?? "", "cache.json");
function readCache(workspace: string, namespace?: string): RetrievalVectors | undefined {
  try { return JSON.parse(readFileSync(cacheFile(workspace, namespace), "utf8")); }
  catch (e) {
    if (e instanceof SyntaxError || (e as NodeJS.ErrnoException).code === "ENOENT") return;
    throw e;
  }
}
/** Read-only remaining work; never materializes or repairs the cache. */
export function missingRetrievalTexts(workspace: string, records: SemanticRetrievalRecord[], identity: EmbeddingProviderIdentity, options: RetrievalCacheOptions = {}): string[] {
  const cache = readCache(workspace, options.cache_namespace);
  const reusable = cache?.version === "semantic_retrieval_vectors.v1" && sameEmbeddingIdentity(cache.provider, identity)
    && cache.projection_version === (options.projection_version ?? RETRIEVAL_PROJECTION_VERSION);
  return [...new Map(records.map(r => [r.content_digest, r.embedding_text]))]
    .filter(([digest]) => !reusable || !validEmbeddingVector(cache!.vectors[digest], identity.dimensions)).map(([, text]) => text);
}
function saveCache(workspace: string, cache: RetrievalVectors, namespace?: string) {
  const file = cacheFile(workspace, namespace);
  mkdirSync(path.dirname(file), { recursive: true });
  // A single replace keeps provider identity, dimensions and vectors together.
  writeFileSync(`${file}.tmp`, JSON.stringify(cache));
  renameSync(`${file}.tmp`, file);
}

/** Explicit preparation only. Ordinary reads/routing must never call this helper. */
export async function prepareRetrievalVectors(input: EmbeddingCallOptions & RetrievalCacheOptions & { workspace: string; records: SemanticRetrievalRecord[];
  provider: EmbeddingProvider }) {
  const { provider, records } = input, identity = structuredClone(provider.identity);
  input.signal?.throwIfAborted();
  const projection_version = input.projection_version ?? RETRIEVAL_PROJECTION_VERSION;
  const previous = readCache(input.workspace, input.cache_namespace);
  const reusable = previous?.version === "semantic_retrieval_vectors.v1" && sameEmbeddingIdentity(previous.provider, identity)
    && previous.projection_version === projection_version;
  const cache: RetrievalVectors = { version: "semantic_retrieval_vectors.v1", provider: identity, projection_version,
    records: records.map(({ key, content_digest }) => ({ key, content_digest })), vectors: reusable ? { ...previous.vectors } : {} };
  const unique = new Map(records.map(r => [r.content_digest, r.embedding_text]));
  const missing = [...unique].filter(([digest]) => !validEmbeddingVector(cache.vectors[digest], identity.dimensions));
  const batch = provider.limits.max_batch_size;
  if (!Number.isInteger(batch) || batch < 1) throw new Error("invalid embedding batch limit");
  let added = 0;
  try {
    for (let offset = 0; offset < missing.length; offset += batch) {
      const entries = missing.slice(offset, offset + batch);
      const vectors = await callEmbedding(provider, identity, "document", entries.map(([, text]) => text), input);
      entries.forEach(([digest], i) => cache.vectors[digest] = vectors[i]);
      added += entries.length;
    }
  } finally {
    // Save completed batches on both success and failure, once per preparation.
    // Rewriting a growing single-book JSON cache after every batch would be quadratic.
    if (added || !missing.length && !isDeepStrictEqual(previous, cache)) saveCache(input.workspace, cache, input.cache_namespace);
  }
  if (!sameEmbeddingIdentity(provider.identity, identity)) throw new Error("embedding provider identity mismatch");
  return { cache, embedded_documents: missing.length, cache_hits: unique.size - missing.length };
}

export function cosineSimilarity(a: number[], b: number[]): number {
  let dot = 0, aa = 0, bb = 0;
  for (let i = 0; i < a.length; i++) { dot += a[i] * b[i]; aa += a[i] * a[i]; bb += b[i] * b[i]; }
  return dot / Math.sqrt(aa * bb);
}
/** Score only current records. Never deduplicate different keys by their text digest. */
export function rankSemanticRecords(records: SemanticRetrievalRecord[], cache: RetrievalVectors,
  query: { provider: EmbeddingProviderIdentity; vector: number[] }, focus_key?: string, projection_version = RETRIEVAL_PROJECTION_VERSION) {
  if (cache.version !== "semantic_retrieval_vectors.v1" || cache.projection_version !== projection_version
    || !sameEmbeddingIdentity(cache.provider, query.provider)) throw new Error("retrieval vector identity mismatch");
  const dimensions = cache.provider.dimensions;
  if (!validEmbeddingVector(query.vector, dimensions)) throw new Error("retrieval query dimensions invalid");
  const valid = new Map(records.filter(r => r.key !== focus_key).map(r => [r.key, r]));
  const mapped = new Map(cache.records.map(r => [r.key, r.content_digest]));
  return [...valid.values()].map(r => {
    const vector = cache.vectors[r.content_digest];
    if (mapped.get(r.key) !== r.content_digest || !validEmbeddingVector(vector, dimensions)) throw new Error("retrieval document vector unavailable or stale");
    return { key: r.key, cosine: cosineSimilarity(vector, query.vector) };
  }).sort((a, b) => b.cosine - a.cosine || compareCatalogKeys(a.key, b.key));
}
