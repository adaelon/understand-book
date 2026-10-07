import { isDeepStrictEqual } from "node:util";

export interface EmbeddingProviderIdentity {
  provider_id: string;
  model_id: string;
  model_revision: string | null;
  embedding_config: Record<string, string | number | boolean | null>;
  dimensions: number;
}
export interface EmbeddingUsage {
  input_tokens?: number;
  billed_tokens?: number;
  cost?: { amount: number; currency: string };
}
export interface EmbeddingProvider {
  identity: EmbeddingProviderIdentity;
  /** At most one extra attempt for local OS busy/temporarily unavailable errors. */
  max_retries?: 0 | 1;
  limits: { max_input_tokens: number; max_batch_size: number };
  /** Count the actual adapter input, including prefixes and special tokens. */
  count_tokens(text: string, role: "document" | "query"): number;
  embed_documents(texts: string[], options: { signal?: AbortSignal }): Promise<{ vectors: number[][]; usage?: EmbeddingUsage }>;
  embed_query(text: string, options: { signal?: AbortSignal }): Promise<{ vector: number[]; usage?: EmbeddingUsage }>;
}
export interface EmbeddingCall {
  role: "document" | "query";
  records: number;
  elapsed_ms: number;
  status: "ok" | "failed" | "cancelled";
  usage?: EmbeddingUsage;
}
export interface EmbeddingCallOptions {
  signal?: AbortSignal;
  before_call?: (call: { role: "document" | "query"; records: number; input_tokens: number }) => void;
  on_call?: (call: EmbeddingCall) => void;
}
export const sameEmbeddingIdentity = (a: EmbeddingProviderIdentity, b: EmbeddingProviderIdentity) => isDeepStrictEqual(a, b);
export function validEmbeddingVector(vector: unknown, dimensions: number): vector is number[] {
  return Array.isArray(vector) && vector.length === dimensions && vector.every(n => typeof n === "number" && Number.isFinite(n))
    && vector.some(n => n !== 0);
}
/** Each attempt crosses authorization and usage hooks, including a bounded transient retry. */
export async function callEmbedding(provider: EmbeddingProvider, expected: EmbeddingProviderIdentity,
  role: "document" | "query", texts: string[], options: EmbeddingCallOptions = {}) {
  options.signal?.throwIfAborted();
  const identity = structuredClone(expected);
  const checkIdentity = () => {
    if (!sameEmbeddingIdentity(provider.identity, identity)) throw new Error("embedding provider identity mismatch");
  };
  checkIdentity();
  if (!Number.isInteger(identity.dimensions) || identity.dimensions < 1) throw new Error("invalid embedding dimensions");
  if (!Number.isInteger(provider.limits.max_batch_size) || provider.limits.max_batch_size < 1
    || !Number.isInteger(provider.limits.max_input_tokens) || provider.limits.max_input_tokens < 1) throw new Error("invalid embedding limits");
  if (!texts.length || texts.length > provider.limits.max_batch_size || (role === "query" && texts.length !== 1)) throw new Error("embedding batch limit exceeded");
  let input_tokens = 0;
  for (const text of texts) {
    const count = provider.count_tokens(text, role);
    if (!Number.isInteger(count) || count < 1 || count > provider.limits.max_input_tokens) throw new Error("embedding input token limit exceeded");
    input_tokens += count;
  }
  for (let attempt = 0; ; attempt++) {
    options.signal?.throwIfAborted();
    checkIdentity();
    options.before_call?.({ role, records: texts.length, input_tokens });
    const started = performance.now();
    let status: EmbeddingCall["status"] = "failed", usage: EmbeddingUsage | undefined;
    try {
      const result = role === "document" ? await provider.embed_documents(texts, { signal: options.signal })
        : await provider.embed_query(texts[0], { signal: options.signal }).then(r => ({ vectors: [r.vector], usage: r.usage }));
      usage = result.usage;
      options.signal?.throwIfAborted();
      checkIdentity();
      if (result.vectors.length !== texts.length || !result.vectors.every(v => validEmbeddingVector(v, identity.dimensions))) {
        throw new Error("embedding response count or dimensions invalid");
      }
      status = "ok";
      return result.vectors;
    } catch (error) {
      const code = error && typeof error === "object" && "code" in error ? error.code : undefined;
      if (options.signal?.aborted || attempt >= (provider.max_retries === 1 ? 1 : 0)
        || (code !== "EAGAIN" && code !== "EBUSY")) throw error;
    } finally {
      if (options.signal?.aborted) status = "cancelled";
      options.on_call?.({ role, records: texts.length, elapsed_ms: performance.now() - started, status, ...(usage ? { usage } : {}) });
    }
  }
}
