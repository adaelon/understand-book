<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from 'vue';
import { BookOpen, CornerDownRight, Grip, Maximize, Minimize, MessageSquareText, Minus, NotebookPen, Plus, Scan, Shapes, Sparkles, UserRound } from '@lucide/vue';
import { t as uiText } from '../i18n';
import type { BoardItem } from '../generated/BoardItem';
import type { BoardView } from '../generated/BoardView';
import type { BoardLink } from '../generated/BoardLink';
import type { BoardVisualTarget } from '../generated/BoardVisualTarget';
import type { BoardMindMap } from '../generated/BoardMindMap';
import { boardItemPreview, boardKindLabels, fitBoard, zoomBoardAt } from '../board-workspace';
import { connectBounds, connectionLabelOffsetY, focusBoardBounds, graphicPath, itemBounds, linkGeometry, presentationTargetMatches, targetBounds, unionBounds, visibleGraphicText } from '../board-canvas-geometry';
import { drawingAppearance } from '../board-canvas-appearance';
import BoardDrawingText from './BoardDrawingText.vue';
import MindMapLayer from './MindMapLayer.vue';
import { layoutMindMaps, topicDescendants } from '../mind-map-layout';

const props = withDefaults(defineProps<{
  items: BoardItem[]; view: BoardView; disabled?: boolean; readonly?: boolean; links?: BoardLink[];
  visibleItemIds?: string[]; visibleLinkIds?: string[]; highlightTargets?: BoardVisualTarget[]; progress?: Record<string, number>;
  transition?: 'instant' | 'draw' | 'reveal_text'; fullscreen?: boolean;
  mindMaps?: BoardMindMap[]; selectedIds?: string[]; foldedIds?: string[]; editingId?: string | null; draftTitle?: string;
}>(), { fullscreen: undefined });
const emit = defineEmits<{
  (event: 'select', id: string): void;
  (event: 'move', id: string, position: { x: number; y: number }): void;
  (event: 'view', view: BoardView): void;
  (event: 'add'): void;
  (event: 'ask'): void;
  (event: 'toggle-fullscreen'): void;
  (event: 'activate-target', target: BoardVisualTarget): void;
  (event: 'user-control', kind: 'pan' | 'zoom' | 'drag' | 'keyboard'): void;
  (event: 'topic-select', id: string, additive: boolean): void;
  (event: 'topic-edit', id: string): void;
  (event: 'topic-draft', title: string): void;
  (event: 'topic-commit'): void;
  (event: 'topic-cancel'): void;
  (event: 'topic-keydown', eventValue: KeyboardEvent, id: string): void;
  (event: 'topic-drop', drop: { itemId: string; parentId: string; index: number }): void;
  (event: 'topic-root-move', move: { mapId: string; anchor: { x: number; y: number } }): void;
  (event: 'topic-fold', id: string): void;
  (event: 'topic-box-select', ids: string[]): void;
}>();
const surface = ref<HTMLElement>();
const mindMapLayer = ref<InstanceType<typeof MindMapLayer>>();
const dragging = ref<string | null>(null);
const dragPosition = ref<{ x: number; y: number } | null>(null);
const panning = ref(false);
const fontRevision = ref(0);
const mindMapLayout = computed(() => {
  void fontRevision.value;
  return layoutMindMaps(props.items, props.mindMaps ?? [], { collapsedIds: props.foldedIds ?? [] });
});
const topicDrag = ref<{ itemId: string; mapId: string; isRoot: boolean; x: number; y: number; ids: string[] } | null>(null);
const topicDrop = ref<{ itemId: string; parentId: string; index: number; placement: 'child' | 'before' | 'after' } | null>(null);
const topicDelta = ref({ x: 0, y: 0 });
const box = ref<{ startX: number; startY: number; x: number; y: number } | null>(null);
const boxStyle = computed(() => !box.value ? {} : { left: `${Math.min(box.value.startX, box.value.x)}px`, top: `${Math.min(box.value.startY, box.value.y)}px`, width: `${Math.abs(box.value.x - box.value.startX)}px`, height: `${Math.abs(box.value.y - box.value.startY)}px` });
const geometryItems = computed(() => {
  if (!topicDrag.value) return mindMapLayout.value.items;
  const moved = new Set(topicDrag.value.ids);
  return mindMapLayout.value.items.map(item => moved.has(item.item_id) ? { ...item, position: { x: item.position.x + topicDelta.value.x, y: item.position.y + topicDelta.value.y } } : item);
});
const arrowId = `board-arrow-${useId()}`;
const icons = { personal_note: UserRound, note: NotebookPen, prebuild: BookOpen, artifact: BookOpen, answer: MessageSquareText, presentation: Shapes, understanding: UserRound };
const worldStyle = computed(() => ({ transform: `translate(${props.view.x}px, ${props.view.y}px) scale(${props.view.zoom})` }));
const targetSize = computed(() => 44 / props.view.zoom);
const shownItems = computed(() => props.visibleItemIds === undefined ? geometryItems.value : geometryItems.value.filter(item => props.visibleItemIds!.includes(item.item_id)));
const freeItems = computed(() => shownItems.value.filter((item): item is BoardItem & { target: Exclude<BoardItem['target'], { kind: 'topic' }> } => item.target.kind !== 'topic'));
const shownTopicNodes = computed(() => props.visibleItemIds === undefined ? mindMapLayout.value.nodes : mindMapLayout.value.nodes.filter(node => props.visibleItemIds!.includes(node.item_id)));
const shownTopicEdges = computed(() => {
  const ids = new Set(shownTopicNodes.value.map(node => node.item_id));
  return mindMapLayout.value.edges.filter(edge => ids.has(edge.parent_id) && ids.has(edge.child_id));
});
const drawingAppearances = computed(() => Object.fromEntries(props.items.filter(item => item.target.kind === 'drawing').map(item => [item.item_id, drawingAppearance(item)] as const)));
const links = computed(() => props.links ?? []);
const topicIds = computed(() => new Set(props.items.filter(item => item.target.kind === 'topic').map(item => item.item_id)));
const connections = computed(() => shownItems.value.flatMap(item => {
  // Drawings retain the reply origin for context, but their visual relations
  // are authored explicitly through BoardLink rather than card reply arrows.
  if (item.target.kind === 'drawing' || item.target.kind === 'topic') return [];
  const parent = shownItems.value.find(candidate => candidate.item_id === item.reply_to_item_id);
  if (!parent || parent.target.kind === 'drawing' || parent.target.kind === 'topic') return [];
  return [{ id: item.item_id, ...connectBounds(itemBounds(parent, point), itemBounds(item, point)) }];
}));
const explicitConnections = computed(() => links.value.flatMap(link => {
  if (props.visibleLinkIds !== undefined && !props.visibleLinkIds.includes(link.link_id)) return [];
  const geometry = linkGeometry(link, shownItems.value, links.value, point);
  if (!geometry) return [];
  const from = targetBounds(link.from, shownItems.value, links.value, point), to = targetBounds(link.to, shownItems.value, links.value, point);
  if (!from || !to) return [];
  if (!link.note_relation_id) return [{ ...link, fullLabel: link.label, labelText: link.label, labelWidth: 0, labelOffsetY: connectionLabelOffsetY(from, to),
    topicLabel: [link.from, link.to].some(target => target.kind === 'topic_edge' || target.kind === 'item' && topicIds.value.has(target.item_id)), ...geometry }];
  const label = noteLinkLabel(link.label);
  const labelBounds = { x: geometry.label.x - label.width / 2, y: geometry.label.y - 13, width: label.width, height: 26 };
  const overlaps = [from, to].some(bounds => labelBounds.x < bounds.x + bounds.width && labelBounds.x + labelBounds.width > bounds.x
    && labelBounds.y < bounds.y + bounds.height && labelBounds.y + labelBounds.height > bounds.y);
  // The default card gap is narrower than a relationship label. Keep that
  // shared relationship readable above the cards instead of behind their text.
  if (overlaps) {
    const startX = from.x + from.width / 2, endX = to.x + to.width / 2;
    const startY = from.y + from.height / 2, endY = to.y + to.height / 2;
    if (Math.abs(endX - startX) >= Math.abs(endY - startY)) {
      const y = Math.min(from.y, to.y) - 40;
      geometry.path = `M ${startX} ${from.y - 3} L ${startX} ${y} L ${endX} ${y} L ${endX} ${to.y - 3}`;
      geometry.label = { x: (startX + endX) / 2, y };
    } else {
      const x = Math.max(from.x + from.width, to.x + to.width) + 36;
      geometry.path = `M ${from.x + from.width + 3} ${startY} L ${x} ${startY} L ${x} ${endY} L ${to.x + to.width + 3} ${endY}`;
      geometry.label = { x: x + label.width / 2 + 8, y: (startY + endY) / 2 };
    }
  }
  return [{ ...link, fullLabel: link.label, labelText: label.text, labelWidth: label.width, labelOffsetY: 4, topicLabel: false, ...geometry }];
}));
function noteLinkLabel(value: string) {
  let text = '', width = 0;
  for (const character of Array.from(value)) {
    const next = /[\u0000-\u00ff]/.test(character) ? 7 : 12;
    if (width + next > 204) { text += '…'; width += 12; break; }
    text += character; width += next;
  }
  return { text, width: width + 24 };
}
let pointer: { id: number; startX: number; startY: number; x: number; y: number; itemId: string | null; moved: boolean; mode: 'free' | 'topic' | 'pan' | 'box'; additive: boolean; captureTarget: HTMLElement } | null = null;
const touchPointers = new Map<number, { x: number; y: number }>();
let pinch: { distance: number; center: { x: number; y: number }; view: BoardView } | null = null;
function touchPair() {
  const [a, b] = [...touchPointers.values()];
  return a && b ? { distance: Math.max(1, Math.hypot(b.x - a.x, b.y - a.y)), center: { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 } } : null;
}
function resetPointer() {
  if (pointer?.captureTarget.hasPointerCapture(pointer.id)) pointer.captureTarget.releasePointerCapture(pointer.id);
  pointer = null; dragging.value = null; dragPosition.value = null; panning.value = false;
  topicDrag.value = null; topicDrop.value = null; topicDelta.value = { x: 0, y: 0 }; box.value = null;
}
function trackPointerDown(event: PointerEvent) {
  if (props.disabled || event.pointerType !== 'touch' || (event.target as Element).closest('textarea,input')) return;
  touchPointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
  const pair = touchPair();
  if (pair) {
    resetPointer();
    pinch = { ...pair, view: { ...props.view } };
    surface.value?.setPointerCapture(event.pointerId);
    emit('user-control', 'zoom'); event.preventDefault();
  }
}
function surfacePoint(clientX: number, clientY: number) {
  const rect = surface.value?.getBoundingClientRect();
  return { x: clientX - (rect?.left ?? 0), y: clientY - (rect?.top ?? 0) };
}
function worldPoint(clientX: number, clientY: number) {
  const point = surfacePoint(clientX, clientY);
  return { x: (point.x - props.view.x) / props.view.zoom, y: (point.y - props.view.y) / props.view.zoom };
}

function start(event: PointerEvent, item?: BoardItem) {
  if (pinch || props.disabled || (item && props.readonly) || event.button !== 0 || (event.target as Element).closest('button, a, input, textarea, [data-board-link]')) return;
  if (!item && (event.target as Element).closest('.board-card,.board-drawing,.mind-map-topic,[data-topic-edge]')) return;
  event.preventDefault();
  const isBox = !item && event.shiftKey && !props.readonly;
  pointer = { id: event.pointerId, startX: event.clientX, startY: event.clientY,
    x: item?.position.x ?? props.view.x, y: item?.position.y ?? props.view.y, itemId: item?.item_id ?? null, moved: false,
    mode: item ? 'free' : isBox ? 'box' : 'pan', additive: event.shiftKey || event.ctrlKey || event.metaKey, captureTarget: surface.value! };
  dragging.value = item?.item_id ?? null;
  dragPosition.value = item ? { ...item.position } : null;
  panning.value = !item && !isBox;
  if (isBox) { const point = surfacePoint(event.clientX, event.clientY); box.value = { startX: point.x, startY: point.y, ...point }; }
  surface.value?.setPointerCapture(event.pointerId);
}
function startTopic(event: PointerEvent, id: string) {
  if (pinch || props.disabled || event.button !== 0 || (event.target as Element).closest('button,textarea,input')) return;
  if (props.editingId === id) return;
  const node = mindMapLayout.value.byId.get(id);
  if (!node) return;
  if (props.readonly) { emit('topic-select', id, event.shiftKey || event.ctrlKey || event.metaKey); return; }
  event.preventDefault();
  pointer = { id: event.pointerId, startX: event.clientX, startY: event.clientY, x: node.x, y: node.y,
    itemId: id, moved: false, mode: 'topic', additive: event.shiftKey || event.ctrlKey || event.metaKey, captureTarget: event.currentTarget as HTMLElement };
  const selected = props.selectedIds ?? [], roots = new Set((props.mindMaps ?? []).map(map => map.root_item_id));
  const moving = node.parent_id && selected.includes(id) ? selected.filter(candidate => !roots.has(candidate) && props.items.some(item => item.item_id === candidate && item.target.kind === 'topic')) : [id];
  const ids = [...new Set(moving.flatMap(candidate => topicDescendants(props.items, candidate)))];
  topicDrag.value = { itemId: id, mapId: node.map_id, isRoot: !node.parent_id, x: node.x, y: node.y, ids };
  pointer.captureTarget.setPointerCapture(event.pointerId);
}
function dropAt(event: PointerEvent) {
  const drag = topicDrag.value;
  if (!drag || drag.isRoot) return null;
  const point = worldPoint(event.clientX, event.clientY), excluded = new Set(drag.ids);
  const target = shownTopicNodes.value.find(node => !excluded.has(node.item_id) && point.x >= node.x - 10 && point.x <= node.x + node.width + 10 && point.y >= node.y - 10 && point.y <= node.y + node.height + 10);
  if (!target) return null;
  const relativeY = (point.y - target.y) / target.height;
  const placement = target.parent_id && relativeY < .25 ? 'before' : target.parent_id && relativeY > .75 ? 'after' : 'child';
  const parentId = placement === 'child' ? target.item_id : target.parent_id!;
  const parent = props.items.find(item => item.item_id === parentId);
  if (!parent || parent.target.kind !== 'topic') return null;
  const children = parent.target.children.filter(id => id !== drag.itemId);
  const index = placement === 'child' ? children.length : children.indexOf(target.item_id) + (placement === 'after' ? 1 : 0);
  return { itemId: target.item_id, parentId, index, placement } as const;
}
function move(event: PointerEvent) {
  if (touchPointers.has(event.pointerId)) touchPointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
  if (pinch) {
    const pair = touchPair();
    if (pair) {
      const center = surfacePoint(pinch.center.x, pinch.center.y);
      const next = zoomBoardAt(pinch.view, pinch.view.zoom * pair.distance / pinch.distance, center);
      emit('view', { ...next, x: next.x + pair.center.x - pinch.center.x, y: next.y + pair.center.y - pinch.center.y });
    }
    return;
  }
  if (!pointer || pointer.id !== event.pointerId) return;
  const dx = event.clientX - pointer.startX, dy = event.clientY - pointer.startY;
  if (!pointer.moved && Math.abs(dx) + Math.abs(dy) > 3) { pointer.moved = true; emit('user-control', pointer.itemId ? 'drag' : 'pan'); }
  if (pointer.mode === 'topic' && pointer.moved) { topicDelta.value = { x: dx / props.view.zoom, y: dy / props.view.zoom }; topicDrop.value = dropAt(event); }
  else if (pointer.mode === 'box' && box.value) { Object.assign(box.value, surfacePoint(event.clientX, event.clientY)); }
  else if (pointer.itemId) dragPosition.value = { x: Math.round(pointer.x + dx / props.view.zoom), y: Math.round(pointer.y + dy / props.view.zoom) };
  else emit('view', { ...props.view, x: pointer.x + dx, y: pointer.y + dy });
}
function finish(event: PointerEvent) {
  touchPointers.delete(event.pointerId);
  if (pinch) { if (touchPointers.size < 2) pinch = null; if (surface.value?.hasPointerCapture(event.pointerId)) surface.value.releasePointerCapture(event.pointerId); return; }
  if (!pointer || pointer.id !== event.pointerId) return;
  if (pointer.mode === 'topic' && topicDrag.value) {
    if (pointer.moved && topicDrag.value.isRoot) emit('topic-root-move', { mapId: topicDrag.value.mapId, anchor: { x: Math.round(topicDrag.value.x + topicDelta.value.x), y: Math.round(topicDrag.value.y + topicDelta.value.y) } });
    else if (pointer.moved && topicDrop.value) emit('topic-drop', { itemId: topicDrag.value.itemId, parentId: topicDrop.value.parentId, index: topicDrop.value.index });
    else if (!pointer.moved) { emit('topic-select', topicDrag.value.itemId, pointer.additive); mindMapLayer.value?.focusTopic(topicDrag.value.itemId); }
  } else if (pointer.mode === 'box' && box.value) {
    const bounds = { x: (Math.min(box.value.startX, box.value.x) - props.view.x) / props.view.zoom, y: (Math.min(box.value.startY, box.value.y) - props.view.y) / props.view.zoom,
      width: Math.abs(box.value.x - box.value.startX) / props.view.zoom, height: Math.abs(box.value.y - box.value.startY) / props.view.zoom };
    const ids = shownTopicNodes.value.filter(node => node.x < bounds.x + bounds.width && node.x + node.width > bounds.x && node.y < bounds.y + bounds.height && node.y + node.height > bounds.y).map(node => node.item_id);
    emit('topic-box-select', ids);
  } else if (pointer.itemId && pointer.moved && dragPosition.value) emit('move', pointer.itemId, dragPosition.value);
  else if (pointer.itemId) emit('select', pointer.itemId);
  resetPointer();
}
function cancel(event: PointerEvent) {
  touchPointers.delete(event.pointerId);
  if (touchPointers.size < 2) pinch = null;
  if (!pointer || pointer.id !== event.pointerId) return;
  resetPointer();
}
function zoom(value: number, point?: { x: number; y: number }) {
  if (props.disabled) return;
  emit('user-control', 'zoom');
  const rect = surface.value?.getBoundingClientRect();
  emit('view', zoomBoardAt(props.view, value, point ?? { x: (rect?.width ?? 600) / 2, y: (rect?.height ?? 600) / 2 }));
}
function wheel(event: WheelEvent) {
  if (props.disabled) return;
  event.preventDefault();
  if (event.ctrlKey || event.metaKey) {
    const rect = surface.value!.getBoundingClientRect();
    zoom(props.view.zoom * Math.exp(-event.deltaY * 0.004), { x: event.clientX - rect.left, y: event.clientY - rect.top });
  } else { emit('user-control', 'pan'); emit('view', { ...props.view, x: props.view.x - event.deltaX, y: props.view.y - event.deltaY }); }
}
function fit() {
  if (!surface.value || surface.value.clientWidth === 0 || surface.value.clientHeight === 0) return;
  emit('view', fitBoard(shownItems.value, surface.value.clientWidth, surface.value.clientHeight, props.view));
}
function fitFromControl() { if (!props.disabled) { emit('user-control', 'zoom'); fit(); } }
function shiftInside(start: number, end: number, minimum: number, maximum: number) {
  if (end - start > maximum - minimum) return minimum - start;
  return start < minimum ? minimum - start : end > maximum ? maximum - end : 0;
}
function locate(itemId: string) {
  if (!surface.value || surface.value.clientWidth === 0 || surface.value.clientHeight === 0) return;
  const topic = mindMapLayout.value.byId.get(itemId);
  if (topic && surface.value) {
    if (props.view.zoom < .25) { focusTargets([{ kind: 'item', item_id: itemId }]); return; }
    const left = topic.x * props.view.zoom + props.view.x, top = topic.y * props.view.zoom + props.view.y;
    const dx = shiftInside(left, left + topic.width * props.view.zoom, 24, surface.value.clientWidth - 24);
    const dy = shiftInside(top, top + topic.height * props.view.zoom, 24, surface.value.clientHeight - 72);
    if (dx || dy) emit('view', { ...props.view, x: props.view.x + dx, y: props.view.y + dy });
    return;
  }
  focusTargets([{ kind: 'item', item_id: itemId }]);
}
function focusTargets(targets: BoardVisualTarget[]) {
  if (!surface.value || surface.value.clientWidth === 0 || surface.value.clientHeight === 0) return;
  const bounds = unionBounds(targets.map(target => targetBounds(target, geometryItems.value, links.value, point)).filter((bounds): bounds is NonNullable<typeof bounds> => bounds !== null));
  if (bounds) emit('view', focusBoardBounds(bounds, surface.value.clientWidth, surface.value.clientHeight, props.view));
}
function keyboard(event: KeyboardEvent, item: BoardItem) {
  if (props.disabled) return;
  if (event.key === 'Enter' || event.key === ' ') {
    emit('user-control', 'keyboard');
    if (item.target.kind === 'drawing') { event.preventDefault(); emit('select', item.item_id); }
    return;
  }
  if (!['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(event.key) || props.readonly) return;
  event.preventDefault();
  emit('user-control', 'keyboard');
  const step = event.shiftKey ? 40 : 12;
  emit('move', item.item_id, { x: item.position.x + (event.key === 'ArrowRight' ? step : event.key === 'ArrowLeft' ? -step : 0),
    y: item.position.y + (event.key === 'ArrowDown' ? step : event.key === 'ArrowUp' ? -step : 0) });
}
function linkKeydown(event: KeyboardEvent, linkId: string) {
  if (props.disabled || !['Enter', ' '].includes(event.key)) return;
  event.preventDefault(); emit('user-control', 'keyboard'); emit('activate-target', { kind: 'link', link_id: linkId });
}
function progress(id: string) { return Math.max(0, Math.min(1, props.progress?.[id] ?? 1)); }
function drawProgress(id: string) { return props.transition === 'draw' ? progress(id) : 1; }
function revealText(text: string, id: string) { return visibleGraphicText(text, props.transition === 'reveal_text' ? progress(id) : 1); }
function highlightedItem(item: BoardItem) { return props.highlightTargets?.some(target => target.kind === 'item' ? target.item_id === item.item_id : target.kind === 'presentation_anchor' && target.item_id === item.item_id && presentationTargetMatches(target, item)) ?? false; }
function highlightedLink(id: string) { return props.highlightTargets?.some(target => target.kind === 'link' && target.link_id === id) ?? false; }
function graphicLabel(item: BoardItem) { return item.target.kind === 'drawing' && item.target.graphic.kind !== 'path' ? item.target.graphic.text : item.title; }
function point(item: BoardItem) { return dragging.value === item.item_id ? dragPosition.value! : item.position; }
function position(item: BoardItem) {
  const location = point(item);
  return { left: `${location.x}px`, top: `${location.y}px`, width: `${item.size.width}px`, height: `${item.size.height}px` };
}
let disposed = false;
function remeasureFonts() { if (!disposed) fontRevision.value++; }
let resizeObserver: ResizeObserver | undefined;
let editorFrame = 0;
function keepEditorVisible() {
  editorFrame = 0;
  const editor = surface.value?.querySelector<HTMLElement>('[data-topic-editor]');
  if (!props.editingId || !surface.value || !editor || disposed) return;
  const canvas = surface.value.getBoundingClientRect(), input = editor.getBoundingClientRect();
  if (!canvas.width || !canvas.height) return;
  if (props.view.zoom < .85) {
    emit('view', zoomBoardAt(props.view, 1, { x: input.x + input.width / 2 - canvas.x, y: input.y + input.height / 2 - canvas.y }));
    void nextTick(scheduleEditorVisibility); return;
  }
  const viewport = window.visualViewport;
  const left = Math.max(canvas.left, viewport?.offsetLeft ?? 0) + 16;
  const right = Math.min(canvas.right, (viewport?.offsetLeft ?? 0) + (viewport?.width ?? window.innerWidth)) - 16;
  const top = Math.max(canvas.top, viewport?.offsetTop ?? 0) + 16;
  const bottom = Math.min(canvas.bottom, (viewport?.offsetTop ?? 0) + (viewport?.height ?? window.innerHeight)) - 84;
  const dx = shiftInside(input.left, input.right, left, right);
  const dy = shiftInside(input.top, input.bottom, top, bottom);
  if (dx || dy) emit('view', { ...props.view, x: props.view.x + dx, y: props.view.y + dy });
}
function scheduleEditorVisibility() {
  if (!disposed && props.editingId && !editorFrame) editorFrame = requestAnimationFrame(keepEditorVisible);
}
watch(() => [props.editingId, props.draftTitle], () => { void nextTick(scheduleEditorVisibility); });
onMounted(() => {
  document.fonts?.addEventListener('loadingdone', remeasureFonts); void document.fonts?.ready.then(remeasureFonts);
  resizeObserver = new ResizeObserver(scheduleEditorVisibility); if (surface.value) resizeObserver.observe(surface.value);
  window.visualViewport?.addEventListener('resize', scheduleEditorVisibility);
});
onBeforeUnmount(() => {
  disposed = true; document.fonts?.removeEventListener('loadingdone', remeasureFonts); resizeObserver?.disconnect();
  window.visualViewport?.removeEventListener('resize', scheduleEditorVisibility); if (editorFrame) cancelAnimationFrame(editorFrame);
  resetPointer(); touchPointers.clear(); pinch = null;
});
function focusTopic(id: string) { mindMapLayer.value?.focusTopic(id); }
defineExpose({ fit, locate, focusTargets, focusTopic });
</script>

<template>
  <section ref="surface" class="board-canvas" :class="{ panning }" :aria-label="uiText('知识画板')"
    @pointerdown.capture="trackPointerDown" @pointerdown="start($event)" @pointermove="move" @pointerup="finish" @pointercancel="cancel" @wheel="wheel">
    <div class="board-world" :style="worldStyle">
      <svg class="board-connections" width="1" height="1" aria-hidden="true"><defs><marker :id="arrowId" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto"><path d="M 1 1 L 7 4 L 1 7" fill="none" stroke="currentColor" stroke-width="1.3" /></marker></defs><path v-for="connection in connections" :key="connection.id" :data-board-reply-link="connection.id" :d="connection.path" fill="none" stroke="currentColor" stroke-width="1.4" :marker-end="`url(#${arrowId})`" /></svg>
      <svg class="board-connections board-explicit-links" width="1" height="1">
        <g v-for="connection in explicitConnections" :key="connection.link_id" class="board-visual-link" :class="{ 'board-highlighted-link': highlightedLink(connection.link_id) }" :data-board-link="connection.link_id" role="button" tabindex="0" :aria-label="connection.fullLabel || uiText('查看连接')" @click.stop="!disabled && emit('activate-target', { kind: 'link', link_id: connection.link_id })" @keydown="linkKeydown($event, connection.link_id)">
          <title v-if="connection.note_relation_id || connection.reason">{{ [connection.fullLabel, connection.reason].filter(Boolean).join('：') }}</title>
          <rect class="board-link-touch-target" :x="connection.label.x - targetSize / 2" :y="connection.label.y - targetSize / 2" :width="targetSize" :height="targetSize" fill="transparent" />
          <path class="board-link-hit" :d="connection.path" fill="none" stroke="transparent" stroke-width="18" />
          <path class="board-link-stroke" :d="connection.path" fill="none" stroke="currentColor" stroke-width="1.8" pathLength="1" stroke-dasharray="1" :stroke-dashoffset="1 - drawProgress(connection.link_id)" :marker-end="drawProgress(connection.link_id) >= 1 ? `url(#${arrowId})` : undefined" />
          <rect v-if="connection.note_relation_id && connection.labelText && drawProgress(connection.link_id) > 0" class="board-note-link-label-bg" :x="connection.label.x - connection.labelWidth / 2" :y="connection.label.y - 13" :width="connection.labelWidth" height="26" rx="6" />
          <text v-if="connection.labelText && drawProgress(connection.link_id) > 0" class="board-link-label" :class="{ 'board-note-link-label': connection.note_relation_id, 'board-topic-link-label': connection.topicLabel }" :x="connection.label.x" :y="connection.label.y + connection.labelOffsetY" text-anchor="middle">{{ revealText(connection.labelText, connection.link_id) }}</text>
        </g>
      </svg>
      <MindMapLayer ref="mindMapLayer" :nodes="shownTopicNodes" :edges="shownTopicEdges" :selected-ids="selectedIds ?? (view.selected_item_id ? [view.selected_item_id] : [])"
        :folded-ids="foldedIds ?? []" :editing-id="editingId" :draft-title="draftTitle" :disabled="disabled" :readonly="readonly" :zoom="view.zoom"
        :drag-ids="topicDrag?.ids" :drag-delta="topicDelta" :drop-cue="topicDrop" :highlight-targets="highlightTargets"
        @start="startTopic" @select="(id, additive) => emit('topic-select', id, additive)" @edit="id => emit('topic-edit', id)"
        @draft="title => emit('topic-draft', title)" @commit="emit('topic-commit')" @cancel="emit('topic-cancel')"
        @keydown="(event, id) => emit('topic-keydown', event, id)" @fold="id => emit('topic-fold', id)"
        @edge="id => emit('activate-target', { kind: 'topic_edge', child_item_id: id })" />
      <template v-for="item in freeItems" :key="item.item_id">
      <svg v-if="item.target.kind === 'drawing'" class="board-drawing" :class="{ selected: view.selected_item_id === item.item_id, 'board-path-drawing': item.target.graphic.kind === 'path', 'board-highlighted-item': highlightedItem(item), dragging: dragging === item.item_id }" :style="position(item)" :width="item.size.width" :height="item.size.height" :viewBox="`0 0 ${item.size.width} ${item.size.height}`" :data-item-id="item.item_id" :data-board-graphic="item.target.graphic.kind" @pointerdown.stop="start($event, item)" @click="readonly && !disabled && emit('select', item.item_id)" @keydown="keyboard($event, item)">
        <g class="board-drawing-control" role="button" tabindex="0" :aria-label="graphicLabel(item)" :aria-pressed="view.selected_item_id === item.item_id">
        <path v-if="item.target.graphic.kind === 'path'" class="board-graphic-path-hit" :d="graphicPath(item.target.graphic.points)" fill="none" stroke="transparent" :stroke-width="Math.max(targetSize, item.target.graphic.width)" stroke-linecap="round" stroke-linejoin="round" pointer-events="stroke" />
        <rect v-else class="board-graphic-touch-target" :x="(item.size.width - Math.max(targetSize, item.size.width)) / 2" :y="(item.size.height - Math.max(targetSize, item.size.height)) / 2" :width="Math.max(targetSize, item.size.width)" :height="Math.max(targetSize, item.size.height)" fill="transparent" />
        <rect class="board-graphic-outline" x="-5" y="-5" :width="item.size.width + 10" :height="item.size.height + 10" rx="8" fill="none" />
        <path v-if="item.target.graphic.kind === 'path'" class="board-native-path" :d="graphicPath(item.target.graphic.points)" :stroke="item.target.graphic.color" :stroke-width="item.target.graphic.width" fill="none" stroke-linecap="round" stroke-linejoin="round" pathLength="1" stroke-dasharray="1" :stroke-dashoffset="1 - drawProgress(item.item_id)" />
        <template v-else>
          <template v-if="item.target.graphic.kind === 'shape'">
            <path class="board-shape-outline" :data-board-shape="item.target.graphic.shape" :d="drawingAppearances[item.item_id].outline" :stroke="drawingAppearances[item.item_id].stroke" stroke-width="1.65" :fill="drawingAppearances[item.item_id].fill" :fill-opacity="drawProgress(item.item_id)" pathLength="1" stroke-dasharray="1" :stroke-dashoffset="1 - drawProgress(item.item_id)" />
            <path class="board-shape-echo" :d="drawingAppearances[item.item_id].secondOutline" :stroke="drawingAppearances[item.item_id].stroke" stroke-width=".8" stroke-opacity=".3" fill="none" pathLength="1" stroke-dasharray="1" :stroke-dashoffset="1 - drawProgress(item.item_id)" />
          </template>
          <BoardDrawingText :text="item.target.graphic.text" :width="item.size.width" :height="item.size.height" :shape="item.target.graphic.kind === 'shape' ? item.target.graphic.shape : undefined" :appearance="drawingAppearances[item.item_id]" :progress="props.transition === 'reveal_text' ? progress(item.item_id) : 1" />
        </template>
        </g>
      </svg>
      <article v-else class="board-card" :class="[item.target.kind, { selected: view.selected_item_id === item.item_id, dragging: dragging === item.item_id, 'board-highlighted-item': highlightedItem(item) }]"
        :style="position(item)" :data-item-id="item.item_id" @pointerdown.stop="start($event, item)">
        <div class="board-card-label"><component :is="icons[item.target.kind]" :size="15" aria-hidden="true" /><span>{{ uiText(boardKindLabels[item.target.kind]) }}</span><Grip :size="15" class="board-card-grip" aria-hidden="true" /></div>
        <button type="button" class="board-card-open" :aria-label="item.title" :aria-pressed="view.selected_item_id === item.item_id" @click="emit('select', item.item_id)" @keydown="keyboard($event, item)">
          <strong>{{ revealText(item.title, item.item_id) }}</strong>
          <span class="board-card-preview">{{ item.target.kind === 'note' && item.target.missing ? uiText('这条笔记已删除，可以从画板移除此卡片。') : revealText(boardItemPreview(item) || uiText('打开查看完整内容'), item.item_id) }}</span>
        </button>
        <div class="board-card-footer"><span v-if="item.target.kind === 'note'">{{ item.target.missing ? uiText('笔记已删除') : uiText('与空间笔记同步') }}</span><span v-else-if="item.reply_to_item_id" class="board-card-reply"><CornerDownRight :size="12" aria-hidden="true" />{{ uiText('追问得到的回答') }}</span><span v-else>{{ item.target.kind === 'understanding' ? uiText('原话与解释分别保留') : item.target.kind === 'presentation' ? uiText('已保留演示与现场') : uiText('完整内容已留存') }}</span><span v-if="item.source_bindings.length">{{ uiText('{0} 处原文', [item.source_bindings.length]) }}</span></div>
      </article>
      </template>
    </div>
    <div v-if="box" class="board-selection-box" :style="boxStyle" aria-hidden="true"></div>
    <div v-if="!items.length" class="board-canvas-empty">
      <span class="board-empty-symbol"><Sparkles :size="26" aria-hidden="true" /></span>
      <h3>{{ uiText('想理解什么？从这里开始') }}</h3>
      <p>{{ uiText('说出一个主题或问题。我们会一边讨论，一边把概念、关系和你的理解画在这里。') }}</p>
      <button v-if="!readonly" type="button" @click="emit('ask')"><MessageSquareText :size="17" aria-hidden="true" />{{ uiText('开始讨论') }}</button>
    </div>
    <div class="board-canvas-tools" role="group" :aria-label="uiText('画板视图')" @pointerdown.stop>
      <button type="button" :aria-label="uiText('缩小画板')" :disabled="disabled || view.zoom <= .25" @click="zoom(view.zoom - .1)"><Minus :size="17" aria-hidden="true" /></button>
      <button type="button" class="board-zoom-reset" :aria-label="uiText('恢复100%缩放')" :title="uiText('恢复100%缩放')" :disabled="disabled" @click="zoom(1)">{{ Math.round(view.zoom * 100) }}%</button>
      <button type="button" :aria-label="uiText('放大画板')" :disabled="disabled || view.zoom >= 1.75" @click="zoom(view.zoom + .1)"><Plus :size="17" aria-hidden="true" /></button>
      <span class="board-tool-divider"></span>
      <button type="button" :aria-label="uiText('显示全部内容')" :title="uiText('显示全部内容')" :disabled="disabled" @click="fitFromControl"><Scan :size="17" aria-hidden="true" /></button>
      <button v-if="fullscreen !== undefined" type="button" class="board-fullscreen-toggle" :aria-label="fullscreen ? uiText('退出全屏') : uiText('全屏画板')" :title="fullscreen ? uiText('退出全屏') : uiText('全屏画板')" :aria-pressed="fullscreen" @click="emit('toggle-fullscreen')"><Minimize v-if="fullscreen" :size="17" aria-hidden="true" /><Maximize v-else :size="17" aria-hidden="true" /><span>{{ fullscreen ? uiText('退出全屏') : uiText('全屏画板') }}</span></button>
    </div>
    <p class="board-canvas-hint">{{ readonly ? uiText('拖动空白移动画板') : mindMaps?.length ? '双击编辑 · Tab 添加子主题 · Shift 拖动框选' : uiText('拖动卡片整理 · 拖动空白移动画板') }}</p>
  </section>
</template>

<style scoped>
.board-canvas-tools .board-zoom-reset{width:auto;min-width:48px;padding:0 6px;text-align:center;font-size:11px;font-variant-numeric:tabular-nums;color:var(--steel)}
.board-link-label.board-topic-link-label{stroke-width:3}
.board-selection-box{position:absolute;pointer-events:none;border:1px solid var(--brand-accent);background:color-mix(in srgb,var(--brand-accent) 12%,transparent);z-index:9;border-radius:3px}
.board-note-link-label-bg{fill:var(--reader-card);stroke:var(--hairline);stroke-width:1}.board-link-label.board-note-link-label{stroke:none;font-size:12px}.board-visual-link:hover .board-note-link-label-bg,.board-highlighted-link .board-note-link-label-bg{stroke:var(--brand-accent)}
.board-canvas .board-canvas-tools .board-fullscreen-toggle{display:flex;align-items:center;justify-content:center;gap:5px;width:auto;padding:0 9px;font-size:11px;white-space:nowrap}
.board-drawing.board-path-drawing{pointer-events:none}.board-native-path{pointer-events:none}
button{min-height:0;padding:0;border-radius:0}
.board-drawing-control:focus-visible{outline:none}.board-drawing:focus-within .board-graphic-outline{stroke:var(--brand-accent)}
.board-drawing{position:absolute;overflow:visible;pointer-events:all;cursor:grab;touch-action:none}.board-drawing.dragging{cursor:grabbing;z-index:5}.board-graphic-outline{stroke:transparent;stroke-width:1.5;stroke-dasharray:6 5;pointer-events:none}.board-drawing:hover .board-graphic-outline{stroke:var(--hairline)}.board-drawing.selected .board-graphic-outline,.board-drawing:focus-visible .board-graphic-outline{stroke:var(--brand-accent)}.board-drawing.board-highlighted-item .board-graphic-outline{stroke:var(--brand-accent);stroke-width:2.5;stroke-dasharray:none;filter:drop-shadow(0 0 4px color-mix(in srgb,var(--brand-accent) 30%,transparent))}.board-shape-outline,.board-shape-echo{stroke-linecap:round;stroke-linejoin:round;pointer-events:none}.board-native-text{font-family:var(--sans);font-weight:400;user-select:none}.board-card.board-highlighted-item{outline:3px solid var(--brand-accent);outline-offset:5px;box-shadow:0 0 0 8px color-mix(in srgb,var(--brand-accent) 8%,transparent)}.board-visual-link{pointer-events:all;cursor:pointer;color:color-mix(in srgb,var(--steel) 78%,var(--reader-canvas))}.board-link-hit{pointer-events:stroke}.board-link-stroke{pointer-events:none;stroke-linecap:round;stroke-linejoin:round}.board-link-label{font:14px var(--sans);fill:var(--slate);stroke:var(--reader-canvas);stroke-width:6px;stroke-linejoin:round;paint-order:stroke}.board-visual-link:hover,.board-visual-link:focus-visible,.board-highlighted-link{color:var(--brand-accent);outline:none}.board-highlighted-link .board-link-stroke,.board-visual-link:focus-visible .board-link-stroke{stroke-width:3.5}.board-explicit-links{z-index:1}.board-card,.board-drawing{z-index:2}
.board-connections{position:absolute;left:0;top:0;overflow:visible;color:var(--stone);pointer-events:none}.board-card-reply{display:flex;align-items:center;gap:4px}
@media(pointer:coarse),(max-width:700px){.board-canvas .board-canvas-tools button{width:44px;height:44px}.board-canvas .board-canvas-empty>button{min-height:44px}}
.board-canvas{position:relative;min-width:0;min-height:0;overflow:hidden;background-color:var(--reader-canvas);background-image:radial-gradient(circle,color-mix(in srgb,var(--stone) 19%,transparent) .8px,transparent .8px);background-size:24px 24px;touch-action:none;cursor:grab;isolation:isolate}.board-canvas.panning{cursor:grabbing}.board-world{position:absolute;inset:0;transform-origin:0 0;pointer-events:none}.board-card{position:absolute;display:flex;flex-direction:column;padding:17px 19px 13px;border:1px solid var(--hairline);border-top:3px solid var(--card-accent);border-radius:12px;background:var(--reader-card);box-shadow:0 3px 15px #25232008;pointer-events:auto;--card-accent:#92846a;--card-tint:color-mix(in srgb,var(--card-accent) 11%,var(--reader-card))}.board-card.prebuild{--card-accent:#769780}.board-card.answer{--card-accent:#788baa}.board-card.presentation{--card-accent:#b88a63}.board-card.understanding{--card-accent:#a887ad}.board-card.selected{outline:2px solid var(--card-accent);outline-offset:3px;box-shadow:0 5px 24px #25232010}.board-card.dragging{z-index:5;box-shadow:0 12px 30px #25232020;cursor:grabbing}.board-card-label{display:flex;align-items:center;gap:6px;color:var(--card-accent);font-size:11px;font-weight:650;letter-spacing:.025em}.board-card-grip{margin-left:auto;opacity:.55}.board-card-open{display:flex;flex:1;min-height:0;flex-direction:column;gap:10px;padding:12px 0 8px;border:0;background:transparent;color:var(--ink);text-align:left;cursor:pointer;font:inherit}.board-card-open strong{display:-webkit-box;overflow:hidden;-webkit-line-clamp:2;-webkit-box-orient:vertical;font-size:16px;line-height:1.5;font-weight:620}.board-card-preview{display:-webkit-box;-webkit-line-clamp:3;-webkit-box-orient:vertical;overflow:hidden;font-size:12px;line-height:1.75;color:var(--steel)}.board-card-footer{display:flex;justify-content:space-between;gap:8px;padding-top:9px;border-top:1px solid var(--hairline-soft);font-size:10px;color:var(--stone)}.board-canvas-tools{position:absolute;bottom:23px;left:22px;display:flex;align-items:center;gap:3px;padding:5px;border:1px solid var(--hairline);border-radius:10px;background:var(--reader-card);box-shadow:0 3px 12px #25232009;cursor:default}.board-canvas-tools button{display:grid;place-items:center;width:32px;height:32px;border:0;border-radius:6px;background:transparent;color:var(--slate);cursor:pointer}.board-canvas-tools button:hover{background:var(--surface)}.board-canvas-tools button:disabled{opacity:.35;cursor:default}.board-canvas-tools>span:not(.board-tool-divider){min-width:42px;text-align:center;font-size:11px;font-variant-numeric:tabular-nums;color:var(--steel)}.board-tool-divider{height:18px;width:1px;margin:0 4px;background:var(--hairline)}.board-canvas-hint{position:absolute;bottom:23px;right:22px;margin:0;font-size:10px;color:var(--stone);pointer-events:none}.board-canvas-empty{position:absolute;left:50%;top:43%;transform:translate(-50%,-50%);width:min(360px,calc(100% - 48px));text-align:center;cursor:default}.board-empty-symbol{display:inline-grid;place-items:center;width:58px;height:58px;border:1px solid var(--hairline);border-radius:18px;background:var(--reader-card);color:var(--brand-accent)}.board-canvas-empty h3{margin:18px 0 10px;font-size:20px;font-weight:560;line-height:1.5}.board-canvas-empty p{font-size:13px;line-height:1.9;color:var(--steel)}.board-canvas-empty button{display:inline-flex;align-items:center;gap:6px;margin-top:12px;padding:10px 16px;background:var(--brand-accent);color:white;border:0;border-radius:8px;cursor:pointer;font:inherit;font-size:13px}button:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}@media(max-width:700px){.board-canvas-hint{display:none}.board-canvas-tools{bottom:16px;left:16px}}
</style>
