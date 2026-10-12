import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { sameEmbeddingIdentity, type EmbeddingProvider, type EmbeddingProviderIdentity } from "./embedding-provider";

export interface LocalEmbeddingConfig { runtime_dir: string; model_dir: string }
export const LOCAL_EMBEDDING_MODEL = "Xenova/paraphrase-multilingual-MiniLM-L12-v2";
/** Reads installed facts only; preview never imports the runtime or loads weights. */
export function localEmbeddingIdentity(config: LocalEmbeddingConfig): EmbeddingProviderIdentity {
  const metadata = JSON.parse(readFileSync(resolve(config.model_dir, "model-metadata.json"), "utf8"));
  if (metadata.model !== LOCAL_EMBEDDING_MODEL || typeof metadata.revision !== "string" || !metadata.revision)
    throw new Error("local embedding model metadata is unsupported");
  const version = (name: string) => JSON.parse(readFileSync(resolve(config.runtime_dir, `node_modules/${name}/package.json`), "utf8")).version as string;
  return { provider_id: "transformers.js-local-cpu", model_id: metadata.model, model_revision: metadata.revision,
    dimensions: 384, embedding_config: { dtype: "q8", pooling: "mean", normalize: true, document_prefix: "", query_prefix: "",
      transformers_version: version("@huggingface/transformers"), onnx_version: version("onnxruntime-node"),
      intra_op_threads: 4, inter_op_threads: 1, truncation: false } };
}

/** Existing local model only. The owning preparation disposes this session when it ends. */
export async function openLocalEmbeddingProvider(config: LocalEmbeddingConfig, expected: EmbeddingProviderIdentity, signal?: AbortSignal) {
  signal?.throwIfAborted();
  const identity = localEmbeddingIdentity(config);
  if (!sameEmbeddingIdentity(identity, expected)) throw Object.assign(new Error("local embedding configuration differs from confirmed plan"), { code: "build_plan_retrieval_drift" });
  const { pipeline, env } = await import(pathToFileURL(resolve(config.runtime_dir,
    "node_modules/@huggingface/transformers/dist/transformers.node.mjs")).href);
  env.allowRemoteModels = false;
  env.allowLocalModels = true;
  signal?.throwIfAborted();
  const extractor = await pipeline("feature-extraction", resolve(config.model_dir), { dtype: "q8", device: "cpu",
    local_files_only: true, session_options: { intraOpNumThreads: 4, interOpNumThreads: 1 } });
  if (signal?.aborted) { await extractor.dispose(); signal.throwIfAborted(); }
  const tokens = (text: string): number => extractor.tokenizer(text, { truncation: false }).input_ids.size;
  const embed = async (texts: string[], signal?: AbortSignal) => {
    signal?.throwIfAborted();
    const lengths = texts.map(tokens);
    if (!texts.length || texts.length > 8 || lengths.some(n => n > 128)) throw new Error("local embedding input limit exceeded");
    const output = await extractor(texts, { pooling: "mean", normalize: true, truncation: false });
    // An in-flight CPU inference cannot be preempted; no result is accepted after cancellation.
    signal?.throwIfAborted();
    return { vectors: output.tolist() as number[][], usage: { input_tokens: lengths.reduce((a, b) => a + b, 0),
      cost: { amount: 0, currency: "USD" } } };
  };
  const provider: EmbeddingProvider = { identity, limits: { max_input_tokens: 128, max_batch_size: 8 }, count_tokens: tokens,
    max_retries: 1,
    embed_documents: (texts, { signal }) => embed(texts, signal),
    embed_query: async (text, { signal }) => { const r = await embed([text], signal); return { vector: r.vectors[0], usage: r.usage }; },
  };
  return { provider, dispose: async () => { await extractor.dispose(); } };
}
