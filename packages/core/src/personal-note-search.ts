import { mkdirSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { readLocalEmbeddingConfig } from './build-retrieval-config';
import { callEmbedding, sameEmbeddingIdentity, validEmbeddingVector, type EmbeddingProvider, type EmbeddingProviderIdentity } from './embedding-provider';
import { localEmbeddingIdentity, openLocalEmbeddingProvider } from './local-embedding-provider';
import { cosineSimilarity } from './semantic-retrieval-cache';
import { splitEmbeddingText } from './workspace-semantic-search';

export interface NoteIndexDocument { note_id: string; revision: number; expression_count?: number; text: string }
export interface PersonalNoteSearchRequest {
  kind: 'personal_notes'; cache_dir: string; snapshot_revision: number; notes: NoteIndexDocument[]; query?: string; top_k?: number;
}
interface VectorChunk { start: number; end: number; vector: number[] }
interface NoteVectors { version: 'personal_note_vectors.v1'; note_id: string; revision: number; expression_count?: number; provider: EmbeddingProviderIdentity; chunks: VectorChunk[] }
interface UserIndex {
  revision: number; heads: Map<string, NoteIndexDocument>; vectors: Map<string, NoteVectors>;
  queued: Map<string, NoteIndexDocument>; running: boolean; task?: Promise<void>; error?: string;
  embedded_chunks: number; document_tokens: number; cache_hits: number;
}
type ProviderSession = { provider: EmbeddingProvider; dispose(): Promise<void> };
const fileFor = (root: string, id: string) => path.join(root, `note-${encodeURIComponent(id)}.json`);
const sameHead=(a:{revision:number;expression_count?:number}|undefined,b:{revision:number;expression_count?:number})=>!!a && a.revision===b.revision && (a.expression_count??0)===(b.expression_count??0);

/** One local model, private files per user/note, and revision checks after every batch.
 * Sync acknowledges quickly; document embeddings never block a saved expression.
 * Queries enter the priority queue between the bounded document batches. */
export class PersonalNoteSearch {
  private users = new Map<string, UserIndex>();
  private session?: ProviderSession;
  private opening?: Promise<void>;
  private openError?: string;
  private jobs: { query: boolean; run: () => Promise<void> }[] = [];
  private busy = false;
  constructor(private readonly openProvider: () => Promise<ProviderSession> = async () => {
    const config = readLocalEmbeddingConfig(); return openLocalEmbeddingProvider(config, localEmbeddingIdentity(config));
  }) {}

  private warm() {
    if (this.session) return Promise.resolve();
    if (!this.opening) this.opening = this.openProvider().then(session => { this.session = session; this.openError = undefined; }, error => { this.openError = String(error instanceof Error ? error.message : error); }).finally(() => { this.opening = undefined; });
    return this.opening;
  }
  private execute<T>(query: boolean, run: () => Promise<T>): Promise<T> {
    return new Promise((resolve, reject) => {
      const job = { query, run: async () => { try { resolve(await run()); } catch (error) { reject(error); } } };
      if (query) { const index = this.jobs.findIndex(job => !job.query); this.jobs.splice(index < 0 ? this.jobs.length : index, 0, job); }
      else this.jobs.push(job);
      void this.drain();
    });
  }
  private async drain() {
    if (this.busy) return; this.busy = true;
    try { while (this.jobs.length) await this.jobs.shift()!.run(); } finally { this.busy = false; }
  }

  private sync(request: PersonalNoteSearchRequest): UserIndex {
    let user = this.users.get(request.cache_dir);
    if (!user) {
      user = { revision: -1, heads: new Map(), vectors: new Map(), queued: new Map(), running: false, embedded_chunks: 0, document_tokens: 0, cache_hits: 0 };
      this.users.set(request.cache_dir, user);
    }
    if (request.snapshot_revision < user.revision) return user;
    if (request.snapshot_revision === user.revision && !user.queued.size && !user.error) return user;
    user.revision = request.snapshot_revision;
    const previous = user.heads;
    user.heads = new Map(request.notes.map(note => [note.note_id, note]));
    mkdirSync(request.cache_dir, { recursive: true });
    const files = new Set(request.notes.map(note => path.basename(fileFor(request.cache_dir, note.note_id))));
    for (const file of readdirSync(request.cache_dir)) {
      if (file.startsWith('note-') && file.endsWith('.json') && !files.has(file)) rmSync(path.join(request.cache_dir, file), { force: true });
    }
    for (const id of previous.keys()) if (!user.heads.has(id)) { user.vectors.delete(id); user.queued.delete(id); }
    for (const note of request.notes) {
      if (sameHead(user.vectors.get(note.note_id),note) || sameHead(user.queued.get(note.note_id),note)) continue;
      user.vectors.delete(note.note_id); user.queued.set(note.note_id, note);
    }
    if (!user.running && user.queued.size) {
      user.running = true;
      user.task = this.build(request.cache_dir, user).finally(() => { user!.running = false; });
    }
    return user;
  }

  private async build(root: string, user: UserIndex) {
    await this.warm();
    if (!this.session) return;
    const provider = this.session.provider;
    try {
      while (user.queued.size) {
        const [id, note] = user.queued.entries().next().value!;
        const current = () => sameHead(user.heads.get(id),note);
        let cached: NoteVectors | undefined;
        try { cached = JSON.parse(readFileSync(fileFor(root, id), 'utf8')); }
        catch (error) { if (!(error instanceof SyntaxError) && (error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; }
        if (cached?.version === 'personal_note_vectors.v1' && cached.note_id === id && sameHead(cached,note)
          && sameEmbeddingIdentity(cached.provider, provider.identity) && Array.isArray(cached.chunks) && cached.chunks.every(chunk => validEmbeddingVector(chunk.vector, provider.identity.dimensions))) {
          if (current()) { user.vectors.set(id, cached); user.cache_hits++; user.queued.delete(id); }
          continue;
        }
        const spans = splitEmbeddingText(note.text, provider, 'document');
        const chunks: VectorChunk[] = [];
        for (let offset = 0; offset < spans.length && current(); offset += provider.limits.max_batch_size) {
          const batch = spans.slice(offset, offset + provider.limits.max_batch_size);
          const result = await this.execute(false, () => callEmbedding(provider, provider.identity, 'document', batch.map(span => span.text), {
            before_call: ({ input_tokens }) => { user.document_tokens += input_tokens; },
          }));
          user.embedded_chunks += batch.length;
          if (!current()) break;
          chunks.push(...batch.map((span, index) => ({ start: span.start, end: span.end, vector: result[index] })));
        }
        if (!current()) { if (sameHead(user.queued.get(id),note)) user.queued.delete(id); continue; }
        const vectors: NoteVectors = { version: 'personal_note_vectors.v1', note_id: id, revision: note.revision, expression_count:note.expression_count, provider: provider.identity, chunks };
        const file = fileFor(root, id), pending = `${file}.tmp`;
        try { writeFileSync(pending, JSON.stringify(vectors)); renameSync(pending, file); }
        finally { rmSync(pending, { force: true }); }
        user.vectors.set(id, vectors); user.queued.delete(id); user.error = undefined;
      }
    } catch (error) { user.error = error instanceof Error ? error.message : String(error); }
  }

  async search(request: PersonalNoteSearchRequest) {
    const user = this.sync(request);
    const result = { status: 'partial' as 'ready' | 'partial' | 'unavailable', reason: undefined as string | undefined,
      hits: [] as { note_id: string; revision: number; start_utf16: number; end_utf16: number; score: number }[],
      indexed_notes: user.vectors.size, pending_notes: user.queued.size, embedded_chunks: user.embedded_chunks,
      document_tokens: user.document_tokens, query_tokens: 0, cache_hits: user.cache_hits };
    if (this.openError || user.error) { result.status = 'unavailable'; result.reason = this.openError ?? user.error; return result; }
    result.status = user.queued.size ? 'partial' : 'ready';
    if (!request.query?.trim() || !this.session || !user.vectors.size) return result;
    const provider = this.session.provider;
    const vectors: number[][] = [];
    try {
      for (const span of splitEmbeddingText(request.query, provider, 'query')) {
        const embedded = await this.execute(true, () => callEmbedding(provider, provider.identity, 'query', [span.text], { before_call: ({ input_tokens }) => { result.query_tokens += input_tokens; } }));
        vectors.push(embedded[0]);
      }
      for (const [id, index] of user.vectors) {
        if (!sameHead(user.heads.get(id),index)) continue;
        let best: typeof result.hits[number] | undefined;
        for (const chunk of index.chunks) {
          const score = Math.max(...vectors.map(vector => cosineSimilarity(vector, chunk.vector)));
          if (!best || score > best.score) best = { note_id: id, revision: index.revision, start_utf16: chunk.start, end_utf16: chunk.end, score };
        }
        if (best) result.hits.push(best);
      }
      result.hits.sort((a, b) => b.score - a.score || a.note_id.localeCompare(b.note_id)); result.hits.length = Math.min(result.hits.length, request.top_k ?? 20);
    } catch (error) { result.status = 'unavailable'; result.reason = error instanceof Error ? error.message : String(error); }
    return result;
  }
  async settled() { await Promise.all([...this.users.values()].map(user => user.task)); }
  async dispose() { await this.settled(); await this.session?.dispose(); this.session = undefined; }
}
