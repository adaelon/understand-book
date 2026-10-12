import { isDeepStrictEqual } from "node:util";
import { callEmbedding, sameEmbeddingIdentity, type EmbeddingProvider, type EmbeddingProviderIdentity, type EmbeddingCallOptions } from "./embedding-provider";
import { prepareRetrievalVectors, rankSemanticRecords, type RetrievalCacheOptions } from "./semantic-retrieval-cache";
import { RETRIEVAL_POLICY, RETRIEVAL_PROJECTION_VERSION, retrieveHybrid, retrievalQuery, compareCatalogKeys,
  type SemanticRetrievalRecord, type RetrievalRequest, type RetrievalPolicy, type RetrievalSequence } from "./semantic-retrieval";

export type RetrievalMode = "lexical_only" | "semantic_required";
export interface RetrievalDependencies {
  mode: RetrievalMode;
  provider: EmbeddingProviderIdentity | null;
  projection_version: string;
  policy: RetrievalPolicy;
  request: RetrievalRequest;
  records: SemanticRetrievalRecord[];
}
export interface PreparedRetrieval {
  version: "prepared_formal_object_retrieval.v1";
  dependencies: RetrievalDependencies;
  sequence: RetrievalSequence;
  query_embedding?: { provider: EmbeddingProviderIdentity; text: string; vector: number[] };
  diagnostics: { semantic_scores: Array<{ key: string; cosine: number }> };
}
export function retrievalDependencies(records: SemanticRetrievalRecord[], request: RetrievalRequest, mode: RetrievalMode,
  provider: EmbeddingProviderIdentity | null, policy = RETRIEVAL_POLICY, projection_version = RETRIEVAL_PROJECTION_VERSION): RetrievalDependencies {
  return structuredClone({ records: [...records].sort((a, b) => compareCatalogKeys(a.key, b.key)), request, mode,
    provider: mode === "lexical_only" ? null : provider, policy, projection_version });
}
export function retrievalPreparationMatches(prepared: PreparedRetrieval | undefined, dependencies: RetrievalDependencies): boolean {
  return prepared?.version === "prepared_formal_object_retrieval.v1" && isDeepStrictEqual(prepared.dependencies, dependencies);
}

/** Caller owns authorization. No routing or candidate-writer code invokes this operation. */
export async function prepareSemanticRetrieval(input: EmbeddingCallOptions & RetrievalCacheOptions & { workspace: string; records: SemanticRetrievalRecord[]; request: RetrievalRequest;
  mode: RetrievalMode; provider_identity?: EmbeddingProviderIdentity; provider?: EmbeddingProvider; policy?: RetrievalPolicy;
  previous?: PreparedRetrieval }): Promise<PreparedRetrieval> {
  const identity = input.provider_identity ?? input.provider?.identity ?? null;
  if (input.mode === "semantic_required" && !identity) throw new Error("semantic retrieval provider identity required");
  const dependencies = retrievalDependencies(input.records, input.request, input.mode, identity, input.policy, input.projection_version);
  if (input.previous && retrievalPreparationMatches(input.previous, dependencies)) return input.previous;
  input.signal?.throwIfAborted();
  const { records, request, policy } = dependencies, query = retrievalQuery(records, request);
  let semantic_scores: Array<{ key: string; cosine: number }> = [], query_embedding: PreparedRetrieval["query_embedding"];
  if (input.mode === "semantic_required" && !query.browse) {
    const provider = input.provider;
    if (!provider) throw new Error("semantic retrieval preparation requires provider");
    if (!sameEmbeddingIdentity(provider.identity, identity!)) throw new Error("embedding provider identity mismatch");
    // Reject an oversized query before any document calls; do not truncate the Agent's request.
    const count = provider.count_tokens(query.embedding, "query");
    if (!Number.isInteger(count) || count < 1 || count > provider.limits.max_input_tokens) throw new Error("embedding query token limit exceeded");
    const { cache } = await prepareRetrievalVectors({ ...input, records, provider });
    const oldQuery = input.previous?.query_embedding;
    query_embedding = oldQuery && sameEmbeddingIdentity(oldQuery.provider, identity!) && oldQuery.text === query.embedding
      ? structuredClone(oldQuery) : { provider: structuredClone(identity!), text: query.embedding,
        vector: (await callEmbedding(provider, identity!, "query", [query.embedding], input))[0] };
    semantic_scores = rankSemanticRecords(records, cache, query_embedding, request.focus_key, dependencies.projection_version);
  }
  return { version: "prepared_formal_object_retrieval.v1", dependencies,
    sequence: retrieveHybrid(records, request, semantic_scores, policy), ...(query_embedding ? { query_embedding } : {}),
    diagnostics: { semantic_scores } };
}
