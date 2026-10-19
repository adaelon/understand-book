import type { BookText, Manifest } from './api';

/** One mounted publication's recent windows; never retain an entire long book. */
export class ReaderWindowTextCache {
  private texts = new Map<string, BookText>();
  private bytes = 0;
  private readWindow: (lids: string[]) => Promise<BookText[]>;
  constructor(tree: Manifest['tree'], read: (lid: string, end?: string) => Promise<BookText>,
    private maxLeaves = 100, private maxBytes = 512 * 1024) {
    this.readWindow = createWindowTextReader(tree, read);
  }
  remember(texts: BookText[]) {
    for (const text of texts) {
      const previous = this.texts.get(text.lid);
      if (previous) { this.bytes -= previous.text.length * 2; this.texts.delete(text.lid); }
      if (text.text.length * 2 > this.maxBytes) continue;
      this.texts.set(text.lid, text); this.bytes += text.text.length * 2;
      while (this.texts.size > this.maxLeaves || this.bytes > this.maxBytes) {
        const first = this.texts.keys().next().value!;
        this.bytes -= this.texts.get(first)!.text.length * 2; this.texts.delete(first);
      }
    }
  }
  peek(lids: string[]): BookText[] | null {
    if (!lids.every(lid => this.texts.has(lid))) return null;
    const texts = lids.map(lid => this.texts.get(lid)!);
    this.remember(texts);
    return texts;
  }
  async read(lids: string[]): Promise<BookText[]> {
    const cached = new Map(lids.flatMap(lid => this.texts.has(lid) ? [[lid, this.texts.get(lid)!] as const] : []));
    const incoming = await this.readWindow(lids.filter(lid => !cached.has(lid)));
    this.remember(incoming);
    for (const text of incoming) cached.set(text.lid, text);
    return lids.map(lid => cached.get(lid)!);
  }
}

/** Read adjacent leaves once, retaining each paragraph's canonical UTF-16 span. */
export async function readWindowText(
  lids: string[],
  tree: Manifest['tree'],
  read: (lid: string, end?: string) => Promise<BookText>,
): Promise<BookText[]> {
  return createWindowTextReader(tree, read)(lids);
}

/** Index source metadata once for repeated, bounded focus windows. */
export function createWindowTextReader(tree: Manifest['tree'], read: (lid: string, end?: string) => Promise<BookText>) {
  const nodes = new Map(tree.map(node => [node.lid, node]));
  const leafIndex = new Map(tree.filter(node => node.children.length === 0).map((node, index) => [node.lid, index]));
  return async (lids: string[]): Promise<BookText[]> => {
  const groups: string[][] = [];
  for (const lid of lids) {
    const group = groups[groups.length - 1];
    const previous = group ? leafIndex.get(group[group.length - 1]) : undefined;
    const index = leafIndex.get(lid);
    if (previous !== undefined && index === previous + 1) group.push(lid);
    else groups.push([lid]);
  }
  const chunks = await Promise.all(groups.map(async group => {
    if (group.length === 1) return [await read(group[0])];
    const start = nodes.get(group[0])!.span.start;
    const result = await read(group[0], group[group.length - 1]);
    return group.map(lid => {
      const span = nodes.get(lid)!.span;
      return { lid, text: result.text.slice(span.start - start, span.end - start) };
    });
  }));
  return chunks.flat();
  };
}
