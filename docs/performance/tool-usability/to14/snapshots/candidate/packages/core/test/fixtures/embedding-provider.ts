import type { EmbeddingProvider } from "../../src/embedding-provider";
export function fakeEmbedding() {
  const calls: Array<{ role: string; texts: string[] }> = [];
  const vector = (text: string) => [1, Array.from(text).reduce((n, c) => n + c.codePointAt(0)!, 0) % 17, text.length % 11];
  const provider: EmbeddingProvider & { calls: typeof calls } = {
    identity: { provider_id: "counting-fake", model_id: "test", model_revision: "1", dimensions: 3,
      embedding_config: { normalize: false, document_prefix: "", query_prefix: "" } },
    limits: { max_input_tokens: 128, max_batch_size: 8 }, count_tokens: text => Array.from(text).length + 2,
    calls,
    async embed_documents(texts) { calls.push({ role: "document", texts }); return { vectors: texts.map(vector) }; },
    async embed_query(text) { calls.push({ role: "query", texts: [text] }); return { vector: vector(text) }; },
  };
  return provider;
}
