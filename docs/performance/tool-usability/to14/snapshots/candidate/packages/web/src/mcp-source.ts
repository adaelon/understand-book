import type { Manifest } from './api';
import type { PublishedBookRef } from './network-context';

export interface McpSource {
  published_book_ref: PublishedBookRef;
  start_lid: string; end_lid: string;
  start_utf16: number; end_utf16: number;
}
export const sourceParams = ['book_id', 'publication_id', 'start_lid', 'end_lid', 'start_utf16', 'end_utf16'] as const;
export function readMcpSource(search: string): McpSource | null {
  const params = new URLSearchParams(search);
  if (!sourceParams.some(key => params.has(key))) return null;
  const values = sourceParams.map(key => params.get(key));
  if (values.some(value => !value?.trim())) throw new Error('来源链接不完整，请重新打开 Agent 返回的完整链接。');
  const [book, publication, start, end, from, to] = values as string[];
  if (!/^\d+$/.test(from) || !/^\d+$/.test(to)
    || !Number.isSafeInteger(Number(from)) || !Number.isSafeInteger(Number(to)) || Number(to) <= Number(from)) {
    throw new Error('来源范围无效，请重新打开 Agent 返回的完整链接。');
  }
  return { published_book_ref: { book_id: book, publication_id: publication },
    start_lid: start, end_lid: end, start_utf16: Number(from), end_utf16: Number(to) };
}

/** Coordinates are absolute UTF-16; project once against the exact publication's leaves. */
export function resolveMcpSource(source: McpSource, manifest: Manifest) {
  const first = manifest.tree.find(node => node.lid === source.start_lid);
  const last = manifest.tree.find(node => node.lid === source.end_lid);
  if (!first || !last || first.span.start > last.span.start
    || source.start_utf16 < first.span.start || source.end_utf16 > last.span.end) {
    throw new Error('来源位置与该材料版本不符，请核对链接。');
  }
  const ranges = new Map<string, [number, number]>();
  for (const node of manifest.tree) {
    if (node.children.length) continue;
    const start = Math.max(source.start_utf16, node.span.start), end = Math.min(source.end_utf16, node.span.end);
    if (end > start) ranges.set(node.lid, [start - node.span.start, end - node.span.start]);
  }
  const lid = ranges.keys().next().value;
  if (!lid) throw new Error('来源范围内没有可显示的原文。');
  return { lid, ranges };
}

export function displayedSourceRange(range: [number, number] | undefined, offset: number, length: number): [number, number] | null {
  if (!range) return null;
  const start = Math.max(0, range[0] - offset), end = Math.min(length, range[1] - offset);
  return end > start ? [start, end] : null;
}
