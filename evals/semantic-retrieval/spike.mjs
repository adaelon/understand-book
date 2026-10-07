import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { cpus, platform, arch } from 'node:os';
import { lexicalLanes, fuse, metrics } from './ranking.mjs';

const [runtime, modelDir, inputFile, outputFile] = process.argv.slice(2);
if (!outputFile) throw new Error('Usage: node spike.mjs RUNTIME_DIR MODEL_DIR INPUT.json OUTPUT.json');
const { pipeline, env } = await import(pathToFileURL(resolve(runtime, 'node_modules/@huggingface/transformers/dist/transformers.node.mjs')).href);
const runtimeVersion = JSON.parse(readFileSync(resolve(runtime, 'node_modules/@huggingface/transformers/package.json'), 'utf8')).version;
const onnxVersion = JSON.parse(readFileSync(resolve(runtime, 'node_modules/onnxruntime-node/package.json'), 'utf8')).version;
env.allowRemoteModels = false;
const input = JSON.parse(readFileSync(inputFile, 'utf8'));
const metadata = JSON.parse(readFileSync(resolve(modelDir, 'model-metadata.json'), 'utf8'));
const started = performance.now();
const extractor = await pipeline('feature-extraction', resolve(modelDir), { dtype: 'q8', device: 'cpu',
  session_options: { intraOpNumThreads: 4, interOpNumThreads: 1 } });
const loadMs = performance.now() - started;
const documents = input.records.map(r => r.embedding_text), queries = input.queries.map(q => q.query);
if (documents.length > input.policy.max_documents || queries.length > input.policy.max_queries) throw new Error('SR0 input budget exceeded');
const calls = [];
async function embed(texts, role) {
  const result = [];
  for (let offset = 0; offset < texts.length; offset += input.policy.batch_size) {
    const batch = texts.slice(offset, offset + input.policy.batch_size);
    const lengths = batch.map(text => extractor.tokenizer(text, { truncation: false }).input_ids.size);
    if (lengths.some(n => n > input.policy.max_sequence_tokens)) throw new Error(`SR0 input exceeds 128 tokens: ${role}/${offset}: ${lengths}`);
    const before = performance.now();
    const output = await extractor(batch, { pooling: 'mean', normalize: true, truncation: false });
    const vectors = output.tolist();
    if (vectors.length !== batch.length || vectors.some(v => v.length !== 384 || v.some(n => !Number.isFinite(n)))) throw new Error('Invalid embedding response');
    calls.push({ role, offset, records: batch.length, token_lengths: lengths, tokenizer_tokens: lengths.reduce((a, b) => a + b, 0), latency_ms: performance.now() - before });
    result.push(...vectors);
  }
  return result;
}
try {
  const docVectors = await embed(documents, 'document');
  const queryVectors = await embed(queries, 'query');
  const rows = input.queries.map((q, qi) => {
    const semantic = input.records.map((r, i) => ({ key: r.key, cosine: docVectors[i].reduce((n, v, j) => n + v * queryVectors[qi][j], 0) }))
      .filter(r => r.key !== q.focus).sort((a, b) => b.cosine - a.cosine || (a.key < b.key ? -1 : a.key > b.key ? 1 : 0));
    const lanes = lexicalLanes(input.records, q.query, q.focus);
    const rankings = { A: q.substring, B: [...lanes.exact, ...lanes.lexical], C: fuse(lanes, semantic.map(r => r.key)), semantic: semantic.map(r => r.key) };
    return { ...q, rankings, semantic_scores: semantic,
      metrics: Object.fromEntries(Object.entries(rankings).map(([name, rank]) => [name, Object.fromEntries(input.policy.ks.map(k => [k, metrics(rank, q.compare, k)]))])),
      identity_decision: { false_merge: null, missed_reuse: null, model_steps: null, inspect_calls: null, read_calls: null, reason: 'retrieval-only spike; no model decision loop executed' } };
  });
  const aggregate = Object.fromEntries(['A', 'B', 'C', 'semantic'].map(group => [group, Object.fromEntries(input.policy.ks.map(k => [k, {
    macro_recall: rows.reduce((n, r) => n + r.metrics[group][k].recall, 0) / rows.length,
    macro_precision_at_k: rows.reduce((n, r) => n + r.metrics[group][k].precision_at_k, 0) / rows.length,
    hits: rows.reduce((n, r) => n + r.metrics[group][k].hits, 0), relevant: rows.reduce((n, r) => n + r.compare.length, 0),
  }]))]));
  const continuation = rows.some(r => r.metrics.A[6].recall < 1 && r.compare.some(key => !r.rankings.A.includes(key) && r.rankings.semantic.slice(0, 6).includes(key)));
  const result = { version: 'sr0-spike.v1', recorded_at: new Date().toISOString(), policy: input.policy,
    provider: { provider_id: 'transformers.js-local-cpu', ...metadata, runtime_version: runtimeVersion, onnx_runtime_version: onnxVersion, dtype: 'q8', dimensions: 384,
      pooling: 'mean', normalize: true, document_prefix: '', query_prefix: '', truncation: false, max_sequence_tokens: 128,
      batch_size: 8, batch_limit_basis: 'experiment cap, not a measured provider maximum', intra_op_threads: 4, inter_op_threads: 1 },
    host: { platform: platform(), arch: arch(), cpu: cpus()[0]?.model, node: process.version },
    usage: { document_records: documents.length, query_records: queries.length, calls, retries: 0, remote_inference_calls: 0,
      provider_billed_tokens: null, inference_api_cost: 0, electricity_cost: null, load_ms: loadMs, total_ms: performance.now() - started },
    aggregate, continuation, rows };
  writeFileSync(outputFile, JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify({ aggregate, continuation, calls: calls.length, total_ms: result.usage.total_ms }, null, 2));
} finally { await extractor.dispose(); }
