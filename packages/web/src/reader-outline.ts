import type { Manifest as SourceManifest } from './generated/Manifest';
import type { NodeKind } from '../../core/src/generated/NodeKind';

export interface OutlineItem {
  lid: string;
  kind: NodeKind;
  depth: number;
  title: string;
  span?: { start: number; end: number };
}
export type ReaderManifest = SourceManifest & { reader_outline?: OutlineItem[] };

export function outlineTitle(text: string, fallback: string): string {
  const first = text.split('\n').map(line => line.trim()).find(Boolean);
  if (!first || /^!\[[^\]]*\]\(/.test(first)) return fallback;
  return first.replace(/^#{1,6}\s+/, '').replace(/\s+#+\s*$/, '').slice(0, 80);
}

export function readerOutline(manifest: ReaderManifest, fallback: string): OutlineItem[] {
  if (manifest.reader_outline?.length) return manifest.reader_outline;
  return manifest.tree.filter(node => node.children.length > 0 || node.kind === 'chapter' || node.kind === 'section')
    .map(node => ({ lid: node.lid, kind: node.kind, depth: Math.min(node.lid.split('.').length - 1, 4),
      title: outlineTitle(node.title ?? '', fallback) }));
}

export function activeOutline(items: OutlineItem[], anchor: string | null, offset?: number): OutlineItem | null {
  if (!anchor) return null;
  return items.filter(item => item.span
    ? offset !== undefined && offset >= item.span.start && offset < item.span.end
    : anchor === item.lid || anchor.startsWith(`${item.lid}.`))
    .sort((a, b) => b.depth - a.depth)[0] ?? null;
}
