import { createInterface } from "node:readline";
import { WorkspaceSemanticSearch, type SemanticSearchRequest } from "../packages/core/src/workspace-semantic-search";
import { PersonalNoteSearch, type PersonalNoteSearchRequest } from '../packages/core/src/personal-note-search';
import { readLocalEmbeddingConfig } from '../packages/core/src/build-retrieval-config';
import { localEmbeddingIdentity, openLocalEmbeddingProvider } from '../packages/core/src/local-embedding-provider';

// Stdout is the JSONL protocol. Runtime diagnostics belong on stderr.
console.log = console.error;
let session: ReturnType<typeof openLocalEmbeddingProvider> | undefined;
const openProvider=async()=> {
  session ??= (async()=>{const config=readLocalEmbeddingConfig();return openLocalEmbeddingProvider(config,localEmbeddingIdentity(config));})();
  const opened=await session.catch(error=>{session=undefined;throw error;});
  return {provider:opened.provider,dispose:async()=>{}};
};
const engine = new WorkspaceSemanticSearch(openProvider);
const notes = new PersonalNoteSearch(openProvider);
try {
  const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
  for await (const line of lines) {
    if (!line.trim()) continue;
    try {
      const request=JSON.parse(line) as SemanticSearchRequest | PersonalNoteSearchRequest | {kind:'warmup'};
      if ('kind' in request && request.kind === 'warmup') {
        const {provider}=await openProvider();
        await provider.embed_query('想法与理解', {});
        process.stdout.write(`${JSON.stringify({status:'ready',model:provider.identity.model_id,dimensions:provider.identity.dimensions})}\n`);
        continue;
      }
      const result = 'kind' in request && request.kind === 'personal_notes' ? await notes.search(request) : await engine.search(request as SemanticSearchRequest);
      process.stdout.write(`${JSON.stringify(result)}\n`);
    } catch (error) {
      process.stdout.write(`${JSON.stringify({ status: "unavailable", reason: error instanceof Error ? error.message : String(error),
        hits: [], indexed_publications: 0, embedded_chunks: 0, cache_hits: 0, query_tokens: 0, document_tokens: 0 })}\n`);
    }
  }
} finally {
  await engine.dispose();
  await notes.dispose();
  if(session) await session.then(value=>value.dispose(),()=>undefined);
}
