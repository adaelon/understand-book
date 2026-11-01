import type { BoardItem } from './generated/BoardItem';
import type { BoardMindMap } from './generated/BoardMindMap';
import type { BoardBounds } from './board-canvas-geometry';

export const MIND_MAP_FONT_FAMILY = '"Noto Sans SC Variable", "Noto Sans SC", system-ui, sans-serif';
export const MIND_MAP_BACKGROUND = '#faf9f6';
const HORIZONTAL_GAP = 56;
const VERTICAL_GAP = 28;
const BRANCH_GAP = 28;
const TEXT_WIDTH = 256;

export interface MindMapTextLine { text: string; x: number; y: number }
export interface MindMapNode extends BoardBounds {
  item: BoardItem;
  item_id: string;
  map_id: string;
  parent_id: string | null;
  depth: number;
  side: 'left' | 'right';
  lines: MindMapTextLine[];
  fontSize: number;
  lineHeight: number;
  hiddenCount: number;
}
export interface MindMapEdge {
  map_id: string;
  parent_id: string;
  child_id: string;
  side: 'left' | 'right';
  path: string;
  bounds: BoardBounds;
}
export interface MindMapLayout {
  /** Topic positions are derived; retained free cards keep their stored geometry. */
  items: BoardItem[];
  nodes: MindMapNode[];
  edges: MindMapEdge[];
  byId: Map<string, MindMapNode>;
  bounds: BoardBounds | null;
  sourceItems: readonly BoardItem[];
  mindMaps: readonly BoardMindMap[];
}
export type MindMapMeasureText = (text: string, fontSize: number, fontWeight: number) => number;
export interface MindMapLayoutOptions {
  collapsedIds?: readonly string[];
  measureText?: MindMapMeasureText;
}
export interface TopicTreeIndex {
  byId: Map<string, BoardItem>;
  parentById: Map<string, string>;
  childrenById: Map<string, string[]>;
}

export function topicTreeIndex(items: readonly BoardItem[]): TopicTreeIndex {
  const byId = new Map(items.filter(item => item.target.kind === 'topic').map(item => [item.item_id, item]));
  const parentById = new Map<string, string>(), childrenById = new Map<string, string[]>();
  for (const item of byId.values()) {
    if (item.target.kind !== 'topic') continue;
    const children = item.target.children.filter(id => byId.has(id));
    childrenById.set(item.item_id, children);
    for (const id of children) parentById.set(id, item.item_id);
  }
  return { byId, parentById, childrenById };
}

export function topicDescendants(items: readonly BoardItem[], itemId: string, includeSelf = true): string[] {
  const index = topicTreeIndex(items), result: string[] = [], seen = new Set<string>();
  const pending = includeSelf ? [itemId] : [...(index.childrenById.get(itemId) ?? [])].reverse();
  while (pending.length) {
    const id = pending.pop()!;
    if (!index.byId.has(id) || seen.has(id)) continue;
    seen.add(id); result.push(id);
    pending.push(...[...(index.childrenById.get(id) ?? [])].reverse());
  }
  return result;
}

export function topicAncestors(items: readonly BoardItem[], itemId: string): string[] {
  const { parentById } = topicTreeIndex(items), result: string[] = [], seen = new Set([itemId]);
  let id = parentById.get(itemId);
  while (id && !seen.has(id)) { result.push(id); seen.add(id); id = parentById.get(id); }
  return result;
}

/** Browser measurement uses the same loaded font as the topic layer and PNG. */
export function createMindMapTextMeasurer(): MindMapMeasureText {
  const context = typeof document === 'undefined' ? null : document.createElement('canvas').getContext('2d');
  const widths = new Map<string, number>();
  return (text, fontSize, fontWeight) => {
    const key = `${fontSize}/${fontWeight}/${text}`, cached = widths.get(key);
    if (cached !== undefined) return cached;
    let width: number;
    if (context) {
      context.font = `${fontWeight} ${fontSize}px ${MIND_MAP_FONT_FAMILY}`;
      width = context.measureText(text).width;
    } else {
      // Non-browser tests and static inspection have a predictable CJK-aware fallback.
      width = Array.from(text).reduce((sum, char) => sum + fontSize * (/^[\x00-\xff]$/.test(char) ? .57 : 1), 0);
    }
    widths.set(key, width);
    return width;
  };
}

let browserMeasure: MindMapMeasureText | undefined;
let fontsObserved = false;
function defaultTextMeasurer(): MindMapMeasureText {
  if (typeof document === 'undefined') return createMindMapTextMeasurer();
  if (!fontsObserved && document.fonts) {
    document.fonts.addEventListener('loadingdone', () => { browserMeasure = undefined; });
    fontsObserved = true;
  }
  // Measuring unchanged labels dominates a chapter relayout. Reuse exact widths
  // until the browser's available font faces change; semantic geometry is still recomputed.
  return browserMeasure ??= createMindMapTextMeasurer();
}

export function wrapTopicText(text: string, width: number, fontSize: number, fontWeight: number, measure: MindMapMeasureText): string[] {
  const lines: string[] = [];
  for (const paragraph of text.replace(/\r\n?/g, '\n').split('\n')) {
    let line = '', lineWidth = 0;
    const flush = () => { lines.push(line.trimEnd()); line = ''; lineWidth = 0; };
    // Keep Rust identifiers and normal words together; long tokens can wrap.
    const tokens = paragraph.match(/[A-Za-z0-9_][A-Za-z0-9_:.<>/&'%-]*|[ \t]+|./gu) ?? [];
    for (const token of tokens) {
      const tokenWidth = measure(token, fontSize, fontWeight);
      if (/^[ \t]+$/.test(token)) { if (line) { line += token; lineWidth += tokenWidth; } continue; }
      if (line && lineWidth + tokenWidth > width) flush();
      if (tokenWidth <= width) { line += token; lineWidth += tokenWidth; continue; }
      for (const char of token) {
        const charWidth = measure(char, fontSize, fontWeight);
        if (line && lineWidth + charWidth > width) flush();
        line += char; lineWidth += charWidth;
      }
    }
    lines.push(line.trimEnd());
  }
  return lines.length ? lines : [''];
}

export function topicAppearance(node: Pick<MindMapNode, 'item' | 'depth' | 'fontSize'>) {
  const style = node.item.target.kind === 'topic' ? node.item.target.style : { color: null, bold: false };
  const color = style.color || '#487e77';
  return {
    fill: node.depth === 0 ? color : '#ffffff',
    stroke: color,
    textColor: node.depth === 0 ? '#ffffff' : '#263c3a',
    fontWeight: style.bold ? 700 : node.depth === 0 ? 650 : 500,
    fontSize: node.fontSize,
    radius: node.depth === 0 ? 14 : 10,
  };
}

function boundsOf(nodes: readonly BoardBounds[]): BoardBounds | null {
  if (!nodes.length) return null;
  let left = Infinity, top = Infinity, right = -Infinity, bottom = -Infinity;
  for (const node of nodes) { left = Math.min(left, node.x); top = Math.min(top, node.y); right = Math.max(right, node.x + node.width); bottom = Math.max(bottom, node.y + node.height); }
  return { x: left, y: top, width: right - left, height: bottom - top };
}

/** Ordered semantic children and persisted branch sides are the only layout authority. */
export function layoutMindMaps(items: readonly BoardItem[], mindMaps: readonly BoardMindMap[] = [], options: MindMapLayoutOptions = {}): MindMapLayout {
  const index = topicTreeIndex(items), measure = options.measureText ?? defaultTextMeasurer();
  const collapsed = new Set(options.collapsedIds ?? []), nodes: MindMapNode[] = [], edges: MindMapEdge[] = [];
  const byId = new Map<string, MindMapNode>();
  for (const map of mindMaps) {
    if (!index.byId.has(map.root_item_id)) continue;
    const ordered: { id: string; depth: number; side: 'left' | 'right'; parent: string | null }[] = [];
    const pending: typeof ordered = [{ id: map.root_item_id, depth: 0, side: 'right', parent: null }];
    const visited = new Set<string>();
    while (pending.length) {
      const entry = pending.pop()!;
      if (visited.has(entry.id) || !index.byId.has(entry.id)) continue;
      visited.add(entry.id); ordered.push(entry);
      const children = index.childrenById.get(entry.id) ?? [];
      for (let i = children.length - 1; i >= 0; i--) {
        const id = children[i];
        const side = entry.depth === 0 ? (map.layout === 'right' ? 'right' : map.branch_sides[id] ?? (i % 2 === 0 ? 'right' : 'left')) : entry.side;
        pending.push({ id, depth: entry.depth + 1, side, parent: entry.id });
      }
    }
    const heights = new Map<string, number>(), descendants = new Map<string, number>(), visible = new Set<string>();
    for (const entry of ordered) {
      if (entry.parent && (!visible.has(entry.parent) || collapsed.has(entry.parent))) continue;
      visible.add(entry.id);
      const item = index.byId.get(entry.id)!, fontSize = entry.depth === 0 ? 20 : 16, lineHeight = Math.ceil(fontSize * 1.5);
      const weight = item.target.kind === 'topic' && item.target.style.bold ? 700 : entry.depth === 0 ? 650 : 500;
      const text = wrapTopicText(item.title || ' ', TEXT_WIDTH, fontSize, weight, measure), pad = entry.depth === 0 ? 18 : 14;
      const width = Math.ceil(Math.max(entry.depth === 0 ? 112 : 72, ...text.map(line => measure(line, fontSize, weight))) + 40);
      const height = Math.ceil(text.length * lineHeight + pad * 2);
      const node: MindMapNode = { item, item_id: entry.id, map_id: map.map_id, parent_id: entry.parent, depth: entry.depth, side: entry.side,
        x: 0, y: 0, width, height, lines: text.map((line, i) => ({ text: line, x: 20, y: pad + fontSize + i * lineHeight })), fontSize, lineHeight, hiddenCount: 0 };
      byId.set(entry.id, node); nodes.push(node);
    }
    for (let i = ordered.length - 1; i >= 0; i--) {
      const entry = ordered[i], children = index.childrenById.get(entry.id) ?? [];
      const count = children.reduce((sum, id) => sum + 1 + (descendants.get(id) ?? 0), 0); descendants.set(entry.id, count);
      const node = byId.get(entry.id);
      if (!node) continue;
      node.hiddenCount = collapsed.has(entry.id) ? count : 0;
      const shown = collapsed.has(entry.id) ? [] : children.filter(id => visible.has(id));
      const childHeight = shown.reduce((sum, id) => sum + heights.get(id)!, 0) + Math.max(0, shown.length - 1) * VERTICAL_GAP;
      heights.set(entry.id, Math.max(node.height, childHeight));
    }
    const root = byId.get(map.root_item_id)!;
    root.x = map.anchor.x; root.y = map.anchor.y;
    const placements: MindMapNode[] = [root];
    while (placements.length) {
      const parent = placements.pop()!;
      if (collapsed.has(parent.item_id)) continue;
      const children = (index.childrenById.get(parent.item_id) ?? []).map(id => byId.get(id)).filter((node): node is MindMapNode => Boolean(node));
      for (const side of ['left', 'right'] as const) {
        const group = children.filter(node => node.side === side), gap = parent.depth === 0 ? BRANCH_GAP : VERTICAL_GAP;
        const total = group.reduce((sum, node) => sum + heights.get(node.item_id)!, 0) + Math.max(0, group.length - 1) * gap;
        let y = parent.y + parent.height / 2 - total / 2;
        for (const child of group) {
          const height = heights.get(child.item_id)!;
          child.x = side === 'right' ? parent.x + parent.width + HORIZONTAL_GAP : parent.x - HORIZONTAL_GAP - child.width;
          child.y = y + (height - child.height) / 2; y += height + gap;
          placements.push(child);
          const sx = side === 'right' ? parent.x + parent.width : parent.x, sy = parent.y + parent.height / 2;
          const ex = side === 'right' ? child.x : child.x + child.width, ey = child.y + child.height / 2, middle = (sx + ex) / 2;
          edges.push({ map_id: map.map_id, parent_id: parent.item_id, child_id: child.item_id, side,
            path: `M ${sx} ${sy} C ${middle} ${sy}, ${middle} ${ey}, ${ex} ${ey}`,
            bounds: { x: Math.min(sx, ex), y: Math.min(sy, ey), width: Math.abs(ex - sx), height: Math.abs(ey - sy) } });
        }
      }
    }
  }
  // Hidden descendants disappear from the canvas, while their semantic items are untouched.
  const displayed = items.flatMap(item => {
    if (item.target.kind !== 'topic') return [item];
    const node = byId.get(item.item_id);
    return node ? [{ ...item, position: { x: node.x, y: node.y }, size: { width: node.width, height: node.height } }] : [];
  });
  return { items: displayed, nodes, edges, byId, bounds: boundsOf(nodes), sourceItems: items, mindMaps };
}

export function mindMapBranchBounds(layout: MindMapLayout, rootItemId?: string): BoardBounds | null {
  if (!rootItemId) return layout.bounds;
  const ids = new Set(topicDescendants(layout.nodes.map(node => node.item), rootItemId));
  return boundsOf(layout.nodes.filter(node => ids.has(node.item_id)));
}
