import type { BoardItem } from './generated/BoardItem';
import type { BoardLink } from './generated/BoardLink';
import type { BoardVisualTarget } from './generated/BoardVisualTarget';
import { connectionLabelOffsetY, linkGeometry, targetBounds } from './board-canvas-geometry';
import { layoutMindMaps, MIND_MAP_BACKGROUND, MIND_MAP_FONT_FAMILY, mindMapBranchBounds, topicAppearance, topicDescendants, type MindMapLayout } from './mind-map-layout';
import { serializeMindMapOutline } from './mind-map-outline';

export interface MindMapPngOptions {
  rootItemId?: string;
  filename?: string;
  pixelRatio?: number;
  links?: readonly BoardLink[];
  background?: string;
}

function roundedRect(context: CanvasRenderingContext2D, x: number, y: number, width: number, height: number, radius: number) {
  context.beginPath(); context.roundRect(x, y, width, height, radius);
}

/** Full semantic content is exported, including branches currently folded in the Reader. */
export async function exportMindMapPng(input: MindMapLayout, options: MindMapPngOptions = {}): Promise<Blob> {
  const labels = input.sourceItems.filter(item => item.target.kind === 'topic').map(item => item.title).join('') + (options.links ?? []).map(link => link.label).join('');
  if (labels) await document.fonts.load(`500 16px ${MIND_MAP_FONT_FAMILY}`, labels);
  await document.fonts.ready;
  const layout = layoutMindMaps(input.sourceItems, input.mindMaps);
  const scope = options.rootItemId ? new Set(topicDescendants(layout.sourceItems, options.rootItemId)) : null;
  const nodes = layout.nodes.filter(node => !scope || scope.has(node.item_id));
  let bounds = mindMapBranchBounds(layout, options.rootItemId);
  if (!bounds || !nodes.length) throw new Error('没有可导出的思维导图。');
  const canvas = document.createElement('canvas'), context = canvas.getContext('2d');
  if (!context) throw new Error('当前浏览器无法生成图片。');
  const links = [...(options.links ?? [])];
  function inScope(target: BoardVisualTarget, seen = new Set<string>()): boolean {
    if (target.kind === 'item') return layout.byId.has(target.item_id) && (!scope || scope.has(target.item_id));
    if (target.kind === 'topic_edge') {
      const child = layout.byId.get(target.child_item_id);
      return Boolean(child?.parent_id && (!scope || scope.has(child.item_id) && scope.has(child.parent_id)));
    }
    if (target.kind !== 'link' || seen.has(target.link_id)) return false;
    const link = links.find(link => link.link_id === target.link_id);
    return Boolean(link && inScope(link.from, new Set(seen).add(target.link_id)) && inScope(link.to, new Set(seen).add(target.link_id)));
  }
  const connections = links.flatMap(link => {
    if (!inScope(link.from) || !inScope(link.to)) return [];
    const geometry = linkGeometry(link, layout.items, links);
    if (!geometry) return [];
    const fromBounds = targetBounds(link.from, layout.items, links), toBounds = targetBounds(link.to, layout.items, links);
    const labelOffsetY = fromBounds && toBounds ? connectionLabelOffsetY(fromBounds, toBounds) : -9;
    context!.font = `500 14px ${MIND_MAP_FONT_FAMILY}`;
    const labelWidth = context!.measureText(link.label).width;
    if (link.label) {
      const left = Math.min(bounds!.x, geometry.label.x - labelWidth / 2 - 5), top = Math.min(bounds!.y, geometry.label.y + labelOffsetY - 18);
      const right = Math.max(bounds!.x + bounds!.width, geometry.label.x + labelWidth / 2 + 5), bottom = Math.max(bounds!.y + bounds!.height, geometry.label.y + labelOffsetY + 6);
      bounds = { x: left, y: top, width: right - left, height: bottom - top };
    }
    return [{ link, geometry, labelOffsetY }];
  });
  const padding = 40, width = Math.ceil(bounds.width + padding * 2), height = Math.ceil(bounds.height + padding * 2);
  // Keep at least one output pixel per world pixel; never silently shrink an unreadable giant image.
  const maximumPixels = 64_000_000, requestedRatio = Math.max(1, options.pixelRatio ?? 2);
  const ratio = Math.min(requestedRatio, 32760 / Math.max(width, height), Math.sqrt(maximumPixels / (width * height)));
  if (ratio < 1) throw new Error('这张导图太大，请选中一个分支后导出，以保留清晰可读的文字。');
  canvas.width = Math.ceil(width * ratio); canvas.height = Math.ceil(height * ratio);
  context.fillStyle = options.background ?? MIND_MAP_BACKGROUND; context.fillRect(0, 0, canvas.width, canvas.height);
  context.scale(ratio, ratio); context.translate(padding - bounds.x, padding - bounds.y);
  context.lineWidth = 2;
  for (const edge of layout.edges) {
    if (scope && (!scope.has(edge.parent_id) || !scope.has(edge.child_id))) continue;
    context.strokeStyle = topicAppearance(layout.byId.get(edge.child_id)!).stroke; context.globalAlpha = .72;
    context.stroke(new Path2D(edge.path)); context.globalAlpha = 1;
  }
  for (const { link, geometry, labelOffsetY } of connections) {
    context.strokeStyle = '#8b8984'; context.lineWidth = 1.8; context.stroke(new Path2D(geometry.path));
    // A cross-topic relation keeps its directed arrow, unlike a structural tree edge.
    const coordinates = geometry.path.match(/-?\d+(?:\.\d+)?/g)?.map(Number) ?? [];
    if (coordinates.length === 8) {
      const [,,,, cx, cy, ex, ey] = coordinates, angle = Math.atan2(ey - cy, ex - cx);
      context.beginPath(); context.moveTo(ex - 9 * Math.cos(angle - .5), ey - 9 * Math.sin(angle - .5)); context.lineTo(ex, ey);
      context.lineTo(ex - 9 * Math.cos(angle + .5), ey - 9 * Math.sin(angle + .5)); context.stroke();
    }
    if (link.label) {
      context.font = `500 14px ${MIND_MAP_FONT_FAMILY}`; context.textAlign = 'center';
      context.lineWidth = 3; context.lineJoin = 'round'; context.strokeStyle = options.background ?? MIND_MAP_BACKGROUND;
      context.strokeText(link.label, geometry.label.x, geometry.label.y + labelOffsetY);
      context.fillStyle = '#3d3d3a'; context.fillText(link.label, geometry.label.x, geometry.label.y + labelOffsetY); context.textAlign = 'start';
    }
  }
  for (const node of nodes) {
    const style = topicAppearance(node);
    roundedRect(context, node.x, node.y, node.width, node.height, style.radius);
    context.fillStyle = style.fill;
    context.shadowColor = node.depth === 0 ? '#2029300c' : '#20293007'; context.shadowBlur = node.depth === 0 ? 15 : 7; context.shadowOffsetY = node.depth === 0 ? 4 : 2;
    context.fill(); context.shadowColor = 'transparent'; context.shadowBlur = 0; context.shadowOffsetY = 0;
    context.lineWidth = 1.5; context.strokeStyle = style.stroke; context.stroke();
    context.font = `${style.fontWeight} ${node.fontSize}px ${MIND_MAP_FONT_FAMILY}`; context.fillStyle = style.textColor;
    for (const line of node.lines) context.fillText(line.text, node.x + line.x, node.y + line.y);
  }
  return new Promise((resolve, reject) => canvas.toBlob(blob => blob ? resolve(blob) : reject(new Error('图片导出失败，请选择较小的分支重试。')), 'image/png'));
}

export function downloadMindMapBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob), link = document.createElement('a');
  link.href = url; link.download = filename; document.body.append(link); link.click(); link.remove();
  // Keep the URL alive until the browser has begun its download.
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export async function downloadMindMapPng(layout: MindMapLayout, options: MindMapPngOptions = {}): Promise<void> {
  downloadMindMapBlob(await exportMindMapPng(layout, options), options.filename ?? '思维导图.png');
}

export function downloadMindMapMarkdown(items: readonly BoardItem[], rootItemId: string, filename = '思维导图.md'): void {
  downloadMindMapBlob(new Blob([serializeMindMapOutline(items, rootItemId)], { type: 'text/markdown;charset=utf-8' }), filename);
}
