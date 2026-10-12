import { createMarkdownDomSourceMap } from './markdown-source-map';

export interface TextReadingPosition { start: number; end: number; text: string; top: number }
export interface ScrollAnchor { lid: string; top: number; textPosition?: TextReadingPosition }

/** Pick a real rendered character near the reading probe, without moving the native selection. */
export function captureTextPosition(root: HTMLElement, source: string, probeY: number, paneTop: number): TextReadingPosition | undefined {
  const map = createMarkdownDomSourceMap(source, root);
  let best: { range: Range; top: number; distance: number } | undefined;
  for (const piece of map.domPieces) {
    if (piece.kind !== 'text' || piece.end === piece.start) continue;
    const node = piece.node as Text;
    const range = document.createRange();
    const base = piece.domStart ?? 0;
    let low = 0, high = piece.end - piece.start - 1;
    while (low < high) {
      const mid = Math.floor((low + high) / 2);
      range.setStart(node, base + mid); range.setEnd(node, base + mid + 1);
      if (range.getBoundingClientRect().bottom < probeY) low = mid + 1;
      else high = mid;
    }
    // Keep a surrogate pair as one verifiable source character.
    let offset = base + low;
    if (offset > 0 && /[\uDC00-\uDFFF]/.test(node.data[offset])) offset--;
    const length = (node.data.codePointAt(offset) ?? 0) > 0xffff ? 2 : 1;
    range.setStart(node, offset); range.setEnd(node, offset + length);
    const rect = range.getBoundingClientRect();
    if (!rect.height) continue;
    const distance = Math.abs(rect.top - probeY);
    if (!best || distance < best.distance) best = { range: range.cloneRange(), top: rect.top - paneTop, distance };
  }
  if (!best) return;
  const ranges = map.sourceRangesForRange(best.range);
  if (ranges.length !== 1) return;
  const { start, end } = ranges[0];
  return { start, end, text: source.slice(start, end), top: best.top };
}

export function textPositionTop(root: HTMLElement, source: string, position: TextReadingPosition): number | null {
  if (source.slice(position.start, position.end) !== position.text) return null;
  const map = createMarkdownDomSourceMap(source, root);
  const semantic = map.semanticRangesForSourceRange(position.start, position.end)[0];
  if (!semantic) return null;
  const piece = map.domPieces.find(p => p.kind === 'text' && p.start <= semantic.start && p.end > semantic.start);
  if (!piece) return null;
  const range = document.createRange();
  range.setStart(piece.node, (piece.domStart ?? 0) + semantic.start - piece.start);
  range.setEnd(piece.node, (piece.domStart ?? 0) + Math.min(semantic.end, piece.end) - piece.start);
  const rect = range.getBoundingClientRect();
  return rect.height ? rect.top : null;
}
