import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import type { EmbeddingProvider } from "../../packages/core/src/embedding-provider";

/** SR1 sample adapter only: local existing model, no production injection or retries. */
export async function localProvider(runtime: string, model: string) {
  const { pipeline, env } = await import(pathToFileURL(resolve(runtime, "node_modules/@huggingface/transformers/dist/transformers.node.mjs")).href);
  env.allowRemoteModels = false;
  const metadata = JSON.parse(readFileSync(resolve(model, "model-metadata.json"), "utf8"));
  const version = (name: string) => JSON.parse(readFileSync(resolve(runtime, `node_modules/${name}/package.json`), "utf8")).version;
  const extractor = await pipeline("feature-extraction", resolve(model), { dtype: "q8", device: "cpu",
    session_options: { intraOpNumThreads: 4, interOpNumThreads: 1 } });
  const tokens = (text: string): number => extractor.tokenizer(text, { truncation: false }).input_ids.size;
  const embed = async (texts: string[], signal?: AbortSignal) => {
    signal?.throwIfAborted();
    const lengths = texts.map(tokens);
    if (lengths.some(n => n > 128) || texts.length > 8) throw new Error("local probe input limit exceeded");
    const output = await extractor(texts, { pooling: "mean", normalize: true, truncation: false });
    signal?.throwIfAborted();
    return { vectors: output.tolist() as number[][], usage: { input_tokens: lengths.reduce((a, b) => a + b, 0) } };
  };
  const provider: EmbeddingProvider = {
    identity: { provider_id: "transformers.js-local-cpu", model_id: metadata.model, model_revision: metadata.revision,
      dimensions: 384, embedding_config: { dtype: "q8", pooling: "mean", normalize: true, document_prefix: "", query_prefix: "",
        transformers_version: version("@huggingface/transformers"), onnx_version: version("onnxruntime-node"),
        intra_op_threads: 4, inter_op_threads: 1, truncation: false } },
    limits: { max_input_tokens: 128, max_batch_size: 8 }, count_tokens: tokens,
    embed_documents: (texts, { signal }) => embed(texts, signal),
    embed_query: async (text, { signal }) => { const r = await embed([text], signal); return { vector: r.vectors[0], usage: r.usage }; },
  };
  return { provider, dispose: () => extractor.dispose() };
}
