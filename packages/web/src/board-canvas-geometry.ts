import type { BoardItem } from './generated/BoardItem';
import type { BoardLink } from './generated/BoardLink';
import type { BoardPoint } from './generated/BoardPoint';
import type { BoardVisualTarget } from './generated/BoardVisualTarget';
import type { BoardView } from './generated/BoardView';

export interface BoardBounds { x: number; y: number; width: number; height: number }
export interface BoardLinkGeometry { path: string; bounds: BoardBounds; label: BoardPoint }
export type BoardItemPosition = (item: BoardItem) => BoardPoint;

export function itemBounds(item: BoardItem, position: BoardItemPosition = item => item.position): BoardBounds {
  return { ...position(item), ...item.size };
}

export function unionBounds(bounds: BoardBounds[]): BoardBounds | null {
  if (!bounds.length) return null;
  const x = Math.min(...bounds.map(item => item.x)), y = Math.min(...bounds.map(item => item.y));
  return { x, y, width: Math.max(...bounds.map(item => item.x + item.width)) - x, height: Math.max(...bounds.map(item => item.y + item.height)) - y };
}

export function presentationTargetMatches(target: Extract<BoardVisualTarget, { kind: 'presentation_anchor' }>, item: BoardItem): boolean {
  return item.target.kind === 'presentation' && item.target.receipt.reference.presentation_id === target.reference.presentation_id
    && item.target.receipt.reference.revision === target.reference.revision;
}

export function targetBounds(target: BoardVisualTarget, items: BoardItem[], links: BoardLink[], position: BoardItemPosition = item => item.position, visited = new Set<string>()): BoardBounds | null {
  if (target.kind === 'topic_edge') {
    const child = items.find(item => item.item_id === target.child_item_id);
    const parent = items.find(item => item.target.kind === 'topic' && item.target.children.includes(target.child_item_id));
    return child && parent ? connectBounds(itemBounds(parent, position), itemBounds(child, position)).bounds : null;
  }
  if (target.kind !== 'link') {
    const item = items.find(item => item.item_id === target.item_id);
    return item && (target.kind !== 'presentation_anchor' || presentationTargetMatches(target, item)) ? itemBounds(item, position) : null;
  }
  if (visited.has(target.link_id)) return null;
  const link = links.find(link => link.link_id === target.link_id);
  if (!link) return null;
  const next = new Set(visited).add(link.link_id);
  const from = targetBounds(link.from, items, links, position, next), to = targetBounds(link.to, items, links, position, next);
  return from && to ? connectBounds(from, to).bounds : null;
}

export function linkGeometry(link: BoardLink, items: BoardItem[], links: BoardLink[], position: BoardItemPosition = item => item.position): BoardLinkGeometry | null {
  const visited = new Set([link.link_id]);
  const from = targetBounds(link.from, items, links, position, visited), to = targetBounds(link.to, items, links, position, visited);
  return from && to ? connectBounds(from, to) : null;
}

function connectionIsHorizontal(from: BoardBounds, to: BoardBounds): boolean {
  const xOverlap = Math.min(from.x + from.width, to.x + to.width) - Math.max(from.x, to.x);
  const yOverlap = Math.min(from.y + from.height, to.y + to.height) - Math.max(from.y, to.y);
  if (xOverlap > 0 && yOverlap <= 0) return false;
  if (yOverlap > 0 && xOverlap <= 0) return true;
  return Math.abs(to.x + to.width / 2 - from.x - from.width / 2) >= Math.abs(to.y + to.height / 2 - from.y - from.height / 2);
}

export function connectBounds(from: BoardBounds, to: BoardBounds): BoardLinkGeometry {
  const dx = to.x + to.width / 2 - from.x - from.width / 2, dy = to.y + to.height / 2 - from.y - from.height / 2;
  const horizontal = connectionIsHorizontal(from, to);
  const sign = (horizontal ? dx : dy) >= 0 ? 1 : -1;
  const start = horizontal ? { x: from.x + (sign > 0 ? from.width : 0), y: from.y + from.height / 2 } : { x: from.x + from.width / 2, y: from.y + (sign > 0 ? from.height : 0) };
  const end = horizontal ? { x: to.x + (sign > 0 ? 0 : to.width), y: to.y + to.height / 2 } : { x: to.x + to.width / 2, y: to.y + (sign > 0 ? 0 : to.height) };
  const bend = Math.max(36, Math.abs(horizontal ? end.x - start.x : end.y - start.y) / 2) * sign;
  const c1 = horizontal ? { x: start.x + bend, y: start.y } : { x: start.x, y: start.y + bend };
  const c2 = horizontal ? { x: end.x - bend, y: end.y } : { x: end.x, y: end.y - bend };
  const points = [start, c1, c2, end];
  const x = Math.min(...points.map(point => point.x)), y = Math.min(...points.map(point => point.y));
  return { path: `M ${start.x} ${start.y} C ${c1.x} ${c1.y}, ${c2.x} ${c2.y}, ${end.x} ${end.y}`,
    bounds: { x, y, width: Math.max(...points.map(point => point.x)) - x, height: Math.max(...points.map(point => point.y)) - y },
    label: { x: (start.x + 3 * c1.x + 3 * c2.x + end.x) / 8, y: (start.y + 3 * c1.y + 3 * c2.y + end.y) / 8 } };
}

/** Put a vertical relationship's text inside the gap, rather than over its upper topic. */
export function connectionLabelOffsetY(from: BoardBounds, to: BoardBounds): number {
  return connectionIsHorizontal(from, to) ? -9 : 4;
}

export function focusBoardBounds(bounds: BoardBounds, width: number, height: number, current: BoardView): BoardView {
  if (width <= 0 || height <= 0) return current;
  const zoom = Math.min(1, Math.max(1, width - 96) / Math.max(1, bounds.width), Math.max(1, height - 120) / Math.max(1, bounds.height));
  return { ...current, zoom, x: width / 2 - (bounds.x + bounds.width / 2) * zoom, y: height / 2 - (bounds.y + bounds.height / 2) * zoom };
}

export function graphicPath(points: BoardPoint[]): string {
  return points.map((point, index) => `${index ? 'L' : 'M'} ${point.x} ${point.y}`).join(' ');
}

export function visibleGraphicText(text: string, progress: number): string {
  const characters = Array.from(text);
  return characters.slice(0, Math.ceil(characters.length * Math.max(0, Math.min(1, progress)))).join('');
}

export function graphicTextLines(text: string, width: number, fontSize: number): string[] {
  const lines: string[] = [];
  const characterWidth = (character: string) => /[\u0000-\u00ff]/.test(character) ? fontSize * .58 : fontSize;
  for (const paragraph of text.split('\n')) {
    let line = '', measured = 0;
    const nextLine = () => { lines.push(line.trimEnd()); line = ''; measured = 0; };
    // Keep ordinary words, numbers and units such as "1.5" or "m/s" together.
    // Chinese still wraps character by character; an oversized token can split.
    for (const token of paragraph.match(/[A-Za-z0-9][A-Za-z0-9._/%°+-]*|[ \t]+|./gu) ?? []) {
      const characters = Array.from(token), tokenWidth = characters.reduce((sum, character) => sum + characterWidth(character), 0);
      if (/^[ \t]+$/.test(token)) { if (line) { line += token; measured += tokenWidth; } continue; }
      if (line && measured + tokenWidth > width) nextLine();
      if (tokenWidth <= width) { line += token; measured += tokenWidth; continue; }
      for (const character of characters) {
        const next = characterWidth(character);
        if (line && measured + next > width) nextLine();
        line += character; measured += next;
      }
    }
    lines.push(line.trimEnd());
  }
  return lines;
}
