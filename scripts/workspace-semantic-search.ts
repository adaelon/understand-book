import { createInterface } from "node:readline";
import { WorkspaceSemanticSearch, type SemanticSearchRequest } from "../packages/core/src/workspace-semantic-search";

// Stdout is the JSONL protocol. Runtime diagnostics belong on stderr.
console.log = console.error;
const engine = new WorkspaceSemanticSearch();
try {
  const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
  for await (const line of lines) {
    if (!line.trim()) continue;
    try {
      const result = await engine.search(JSON.parse(line) as SemanticSearchRequest);
      process.stdout.write(`${JSON.stringify(result)}\n`);
    } catch (error) {
      process.stdout.write(`${JSON.stringify({ status: "unavailable", reason: error instanceof Error ? error.message : String(error),
        hits: [], indexed_publications: 0, embedded_chunks: 0, cache_hits: 0, query_tokens: 0, document_tokens: 0 })}\n`);
    }
  }
} finally {
  await engine.dispose();
}
