import type { BookText, Manifest } from './api';
import type { PaginationRenderSource } from './reader-pagination-render';
import { createWindowTextReader } from './reader-window-text';

/** Read-only, source-version-scoped input. Consumed text belongs to the paginator;
 * only the unread tail of one merged request remains here. No progress commands. */
export class FocusSource {
  readonly leaves: Manifest['tree'];
  private cache = new Map<number, PaginationRenderSource>();
  private signal?: AbortSignal;
  private inFlightBytes = 0;
  private readWindow: (lids: string[]) => Promise<BookText[]>;
  constructor(tree: Manifest['tree'], read: (lid: string, end?: string, signal?: AbortSignal) => Promise<BookText>, private image: (lid: string) => PaginationRenderSource['imageAsset']) {
    this.leaves = tree.filter(node => node.children.length === 0);
    this.readWindow = createWindowTextReader(tree, (lid, end) => read(lid, end, this.signal));
  }
  bytes() { return this.inFlightBytes + [...this.cache.values()].reduce((n, s) => n + s.text.length * 2, 0); }
  parsedBytes() { return [...this.cache.values()].reduce((n, s) => n + (s.preparedFormulaHtml?.length ?? 0) * 2, 0); }
  clear() { this.cache.clear(); }
  seed(sources: PaginationRenderSource[]) {
    this.cache.clear();
    for (const source of sources) {
      const index = this.leaves.findIndex(n => n.lid === source.lid);
      if (index >= 0 && this.bytes() + source.text.length * 2 <= 32768) this.cache.set(index, source);
    }
  }
  async take(index: number, signal?: AbortSignal, reserve: (bytes: number) => void = () => {}): Promise<PaginationRenderSource | null> {
    signal?.throwIfAborted();
    if (!this.leaves[index]) return null;
    if (!this.cache.has(index)) {
      this.cache.clear();
      // Eight adjacent leaves, at most 32KiB except for one required large leaf.
      // Reserve the merged response plus its split strings while both are live.
      const first = this.leaves[index];
      let end = index;
      while (end + 1 < Math.min(index + 8, this.leaves.length) && (this.leaves[end + 1].span.end - first.span.start) * 2 <= 32768) end++;
      const bytes = (this.leaves[end].span.end - first.span.start) * 4;
      reserve(bytes);
      this.inFlightBytes = bytes;
      this.signal = signal;
      try {
        const texts = await this.readWindow(this.leaves.slice(index, end + 1).map(n => n.lid));
        signal?.throwIfAborted();
        texts.forEach((text, i) => this.cache.set(index + i, { ...text, kind: this.leaves[index + i].kind, imageAsset: this.image(text.lid) }));
      } finally { this.inFlightBytes = 0; }
    }
    const source = this.cache.get(index)!;
    this.cache.delete(index);
    return source;
  }
}
