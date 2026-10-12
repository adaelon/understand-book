import type { BookText, Manifest } from './api';

/** Read adjacent leaves once, retaining each paragraph's canonical UTF-16 span. */
export async function readWindowText(
  lids: string[],
  tree: Manifest['tree'],
  read: (lid: string, end?: string) => Promise<BookText>,
): Promise<BookText[]> {
  const nodes = new Map(tree.map(node => [node.lid, node]));
  const leafIndex = new Map(tree.filter(node => node.children.length === 0).map((node, index) => [node.lid, index]));
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
}
