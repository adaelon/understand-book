import { mkdtempSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { localProvider } from "./local-provider";
import { fixture, cases } from "../../packages/core/testdata/semantic-retrieval/gold";
import { projectRetrievalCatalog, retrievalCatalog } from "../../packages/core/src/semantic-retrieval";
import { prepareSemanticRetrieval } from "../../packages/core/src/semantic-retrieval-preparation";
import type { EmbeddingCall } from "../../packages/core/src/embedding-provider";

const [runtime, model, output] = process.argv.slice(2);
if (!output) throw new Error("Usage: sr2-rebuild.ts RUNTIME MODEL OUTPUT");
const { work, previous } = fixture();
const selected = ["base/speed", "base/velocity", "base/measure", "later/speed-reworded", "previous/old-speed", "base/out"];
const records = projectRetrievalCatalog(retrievalCatalog(work.proposal, previous)).filter(r => selected.includes(r.key));
if (records.length !== 6) throw new Error("fixed rebuild sample changed");
const workspace = mkdtempSync(path.join(tmpdir(), "sr2-real-rebuild-")), calls: EmbeddingCall[] = [];
const local = await localProvider(runtime, model), started = performance.now();
try {
  const request = { kind: "search" as const, query: cases[0].query, focus_key: cases[0].focus };
  const prepare = () => prepareSemanticRetrieval({ workspace, records, request, mode: "semantic_required", provider: local.provider,
    on_call: c => calls.push(c) });
  const first = await prepare();
  unlinkSync(path.join(workspace, ".build", "semantic-retrieval", "cache.json"));
  const rebuilt = await prepare();
  const result = { recorded_at: new Date().toISOString(), provider: local.provider.identity, selected, query: request,
    documents: 12, queries: 2, remote_calls: 0, retries: 0, calls, elapsed_ms: performance.now() - started,
    first: first.sequence, rebuilt: rebuilt.sequence, identical_sequence: isDeepStrictEqual(first.sequence, rebuilt.sequence),
    max_cosine_delta: Math.max(...first.diagnostics.semantic_scores.map((r, i) => Math.abs(r.cosine - rebuilt.diagnostics.semantic_scores[i].cosine))) };
  writeFileSync(output, JSON.stringify(result, null, 2) + "\n");
  console.log(JSON.stringify({ documents: result.documents, queries: result.queries, identical_sequence: result.identical_sequence,
    max_cosine_delta: result.max_cosine_delta, elapsed_ms: result.elapsed_ms }));
  if (!result.identical_sequence) throw new Error("real cache rebuild changed the frozen sample sequence");
} finally { await local.dispose(); }
