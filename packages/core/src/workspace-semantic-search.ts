import { mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { readLocalEmbeddingConfig } from "./build-retrieval-config";
import { callEmbedding, sameEmbeddingIdentity, validEmbeddingVector, type EmbeddingProvider, type EmbeddingProviderIdentity } from "./embedding-provider";
import { localEmbeddingIdentity, openLocalEmbeddingProvider } from "./local-embedding-provider";
import { cosineSimilarity } from "./semantic-retrieval-cache";
import type { LidNode } from "./generated/LidNode";

export interface SemanticPublication {
  book_id: string;
  publication_id: string;
  directory: string;
  source_revision: string;
}
export interface SemanticSearchRequest {
  query: string;
  top_k: number;
  cache_dir: string;
  /** Already authorized, frozen publications. This service never discovers additional material. */
  publications: SemanticPublication[];
}
export interface SemanticSearchHit {
  book_id: string;
  publication_id: string;
  lid: string;
  start_utf16: number;
  end_utf16: number;
  score: number;
}
export interface SemanticSearchResult {
  status: "ready" | "unavailable";
  reason?: string;
  hits: SemanticSearchHit[];
  indexed_publications: number;
  embedded_chunks: number;
  cache_hits: number;
  /** Actual tokenizer inputs for this request, including special tokens and repeated attempts. */
  query_tokens: number;
  document_tokens: number;
}
interface Chunk { lid: string; start_utf16: number; end_utf16: number; vector: number[] }
interface Index {
  version: "workspace_source_vectors.v1";
  chunking: "source_leaf_tokens.v1";
  book_id: string;
  publication_id: string;
  source_revision: string;
  provider: EmbeddingProviderIdentity;
  chunks: Chunk[];
}
interface ProviderSession { provider: EmbeddingProvider; dispose(): Promise<void> }
const emptyResult = (): SemanticSearchResult => ({ status: "ready", hits: [], indexed_publications: 0,
  embedded_chunks: 0, cache_hits: 0, query_tokens: 0, document_tokens: 0 });
const compare = (a: string, b: string) => a < b ? -1 : a > b ? 1 : 0;

/** No truncation: every UTF-16 interval is covered, with boundaries at Unicode code points.
 * Token counts come from the actual model tokenizer. Exponential windows avoid tokenizing
 * the remaining whole chapter repeatedly when a source leaf is unusually long. */
export function splitEmbeddingText(text: string, provider: EmbeddingProvider, role: "query" | "document") {
  const bounds = [0];
  for (const char of text) bounds.push(bounds.at(-1)! + char.length);
  const spans: Array<{ start: number; end: number; text: string }> = [];
  const limit = Math.min(128, provider.limits.max_input_tokens);
  let from = 0;
  const fits = (end: number) => provider.count_tokens(text.slice(bounds[from], bounds[end]), role) <= limit;
  while (from < bounds.length - 1) {
    const last = bounds.length - 1;
    let low = from, high = Math.min(last, from + limit);
    while (fits(high)) {
      low = high;
      if (high === last) break;
      high = Math.min(last, from + (high - from) * 2);
    }
    while (low + 1 < high) {
      const middle = Math.floor((low + high) / 2);
      if (fits(middle)) low = middle; else high = middle;
    }
    if (low === from) throw new Error("embedding tokenizer cannot fit a source character");
    // Prefer a sentence boundary, then a word boundary, without making tiny fragments.
    if (low < last) {
      const earliest = from + Math.floor((low - from) * 0.65);
      let boundary: number | undefined;
      for (const separator of [/[\n。！？.!?;；]/u, /\s/u]) {
        for (let at = low; at > earliest; at--) {
          if (separator.test(text.slice(bounds[at - 1], bounds[at])) && fits(at)) { boundary = at; break; }
        }
        if (boundary !== undefined) break;
      }
      if (boundary !== undefined) low = boundary;
    }
    spans.push({ start: bounds[from], end: bounds[low], text: text.slice(bounds[from], bounds[low]) });
    from = low;
  }
  return spans;
}

function indexFile(cacheDir: string, publication: SemanticPublication) {
  // The exact publication identity is already available; no additional content digest is needed.
  return path.join(cacheDir, `book-${encodeURIComponent(publication.book_id)}`,
    `publication-${encodeURIComponent(publication.publication_id)}.json`);
}
function readIndex(file: string, publication: SemanticPublication, identity: EmbeddingProviderIdentity): Index | undefined {
  let value: Index;
  try { value = JSON.parse(readFileSync(file, "utf8")); }
  catch (error) {
    if (error instanceof SyntaxError || (error as NodeJS.ErrnoException).code === "ENOENT") return;
    throw error;
  }
  if (value?.version !== "workspace_source_vectors.v1" || value.chunking !== "source_leaf_tokens.v1"
    || value.book_id !== publication.book_id || value.publication_id !== publication.publication_id
    || value.source_revision !== publication.source_revision || !sameEmbeddingIdentity(value.provider, identity)
    || !Array.isArray(value.chunks) || !value.chunks.every(c => typeof c.lid === "string"
      && Number.isSafeInteger(c.start_utf16) && c.start_utf16 >= 0
      && Number.isSafeInteger(c.end_utf16) && c.end_utf16 > c.start_utf16
      && validEmbeddingVector(c.vector, identity.dimensions))) return;
  return value;
}
function saveIndex(file: string, index: Index) {
  mkdirSync(path.dirname(file), { recursive: true });
  const pending = `${file}.tmp`;
  try {
    writeFileSync(pending, JSON.stringify(index));
    renameSync(pending, file);
  } finally {
    rmSync(pending, { force: true });
  }
}

/** One process owns one model session. Disk indices are derived, publication-scoped caches. */
export class WorkspaceSemanticSearch {
  private session?: ProviderSession;
  constructor(private readonly openProvider: () => Promise<ProviderSession> = async () => {
    const config = readLocalEmbeddingConfig();
    return openLocalEmbeddingProvider(config, localEmbeddingIdentity(config));
  }) {}

  async dispose() { const session = this.session; this.session = undefined; await session?.dispose(); }

  async search(request: SemanticSearchRequest): Promise<SemanticSearchResult> {
    const result = emptyResult();
    try {
      if (!request.query.trim() || !request.publications.length) return result;
      if (!Number.isInteger(request.top_k) || request.top_k < 1 || request.top_k > 100)
        throw new Error("semantic top_k must be between 1 and 100");
      this.session ??= await this.openProvider();
      const provider = this.session.provider, identity = structuredClone(provider.identity);
      const countCall = ({ role, input_tokens }: { role: "query" | "document"; input_tokens: number }) => {
        if (role === "query") result.query_tokens += input_tokens; else result.document_tokens += input_tokens;
      };
      const queryVectors: number[][] = [];
      for (const part of splitEmbeddingText(request.query, provider, "query")) {
        queryVectors.push(...await callEmbedding(provider, identity, "query", [part.text], { before_call: countCall }));
      }
      const hits: SemanticSearchHit[] = [];
      const visited = new Set<string>();
      for (const publication of request.publications) {
        const key = JSON.stringify([publication.book_id, publication.publication_id]);
        if (visited.has(key)) continue;
        visited.add(key);
        const file = indexFile(request.cache_dir, publication);
        let index = readIndex(file, publication, identity);
        if (index) result.cache_hits += index.chunks.length;
        else {
          const source = readFileSync(path.join(publication.directory, "source.txt"), "utf8");
          const base = JSON.parse(readFileSync(path.join(publication.directory, "base.json"), "utf8")) as { book_id: string; lid_nodes: LidNode[] };
          if (base.book_id !== publication.book_id) throw new Error("semantic source book identity mismatch");
          const leaves = base.lid_nodes.filter(node => !node.children.length).sort((a, b) => a.span.start - b.span.start || compare(a.lid, b.lid));
          const pieces: Array<{ lid: string; start_utf16: number; end_utf16: number; text: string }> = [];
          for (const leaf of leaves) {
            if (!Number.isSafeInteger(leaf.span.start) || !Number.isSafeInteger(leaf.span.end)
              || leaf.span.start < 0 || leaf.span.end < leaf.span.start || leaf.span.end > source.length)
              throw new Error(`invalid semantic source span: ${leaf.lid}`);
            for (const part of splitEmbeddingText(source.slice(leaf.span.start, leaf.span.end), provider, "document")) {
              if (part.text.trim()) pieces.push({ lid: leaf.lid, start_utf16: leaf.span.start + part.start,
                end_utf16: leaf.span.start + part.end, text: part.text });
            }
          }
          index = { version: "workspace_source_vectors.v1", chunking: "source_leaf_tokens.v1", book_id: publication.book_id,
            publication_id: publication.publication_id, source_revision: publication.source_revision, provider: identity, chunks: [] };
          const batchSize = Math.min(8, provider.limits.max_batch_size);
          if (!Number.isInteger(batchSize) || batchSize < 1) throw new Error("invalid embedding batch size");
          for (let offset = 0; offset < pieces.length; offset += batchSize) {
            const batch = pieces.slice(offset, offset + batchSize);
            const vectors = await callEmbedding(provider, identity, "document", batch.map(p => p.text), { before_call: countCall });
            index.chunks.push(...batch.map(({ text: _text, ...chunk }, i) => ({ ...chunk, vector: vectors[i] })));
            result.embedded_chunks += batch.length;
          }
          // Incomplete work never replaces a complete cache or appears ready on the next request.
          saveIndex(file, index);
          result.indexed_publications++;
        }
        for (const chunk of index.chunks) {
          const score = Math.max(...queryVectors.map(vector => cosineSimilarity(chunk.vector, vector)));
          hits.push({ book_id: publication.book_id, publication_id: publication.publication_id, lid: chunk.lid,
            start_utf16: chunk.start_utf16, end_utf16: chunk.end_utf16, score });
        }
      }
      result.hits = hits.sort((a, b) => b.score - a.score || compare(a.book_id, b.book_id)
        || compare(a.publication_id, b.publication_id) || compare(a.lid, b.lid) || a.start_utf16 - b.start_utf16).slice(0, request.top_k);
      return result;
    } catch (error) {
      return { ...result, status: "unavailable", reason: error instanceof Error ? error.message : String(error), hits: [] };
    }
  }
}
