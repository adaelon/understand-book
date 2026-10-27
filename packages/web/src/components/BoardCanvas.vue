<script setup lang="ts">
import { computed, onBeforeUnmount, ref, useId } from 'vue';
import { BookOpen, CornerDownRight, Grip, Maximize, MessageSquareText, Minus, Plus, Shapes, Sparkles, UserRound } from '@lucide/vue';
import { t as uiText } from '../i18n';
import type { BoardItem } from '../generated/BoardItem';
import type { BoardView } from '../generated/BoardView';
import type { BoardLink } from '../generated/BoardLink';
import type { BoardVisualTarget } from '../generated/BoardVisualTarget';
import { boardItemPreview, boardKindLabels, fitBoard, zoomBoardAt } from '../board-workspace';
import { connectBounds, focusBoardBounds, graphicPath, graphicTextLines, itemBounds, linkGeometry, presentationTargetMatches, targetBounds, unionBounds, visibleGraphicText } from '../board-canvas-geometry';

const props = defineProps<{
  items: BoardItem[]; view: BoardView; disabled?: boolean; readonly?: boolean; links?: BoardLink[];
  visibleItemIds?: string[]; visibleLinkIds?: string[]; highlightTargets?: BoardVisualTarget[]; progress?: Record<string, number>;
  transition?: 'instant' | 'draw' | 'reveal_text';
}>();
const emit = defineEmits<{
  (event: 'select', id: string): void;
  (event: 'move', id: string, position: { x: number; y: number }): void;
  (event: 'view', view: BoardView): void;
  (event: 'add'): void;
  (event: 'ask'): void;
  (event: 'activate-target', target: BoardVisualTarget): void;
  (event: 'user-control', kind: 'pan' | 'zoom' | 'drag' | 'keyboard'): void;
}>();
const surface = ref<HTMLElement>();
const dragging = ref<string | null>(null);
const dragPosition = ref<{ x: number; y: number } | null>(null);
const panning = ref(false);
const arrowId = `board-arrow-${useId()}`;
const icons = { prebuild: BookOpen, artifact: BookOpen, answer: MessageSquareText, presentation: Shapes, understanding: UserRound };
const worldStyle = computed(() => ({ transform: `translate(${props.view.x}px, ${props.view.y}px) scale(${props.view.zoom})` }));
const targetSize = computed(() => 44 / props.view.zoom);
const shownItems = computed(() => props.visibleItemIds === undefined ? props.items : props.items.filter(item => props.visibleItemIds!.includes(item.item_id)));
const links = computed(() => props.links ?? []);
const connections = computed(() => shownItems.value.flatMap(item => {
  const parent = shownItems.value.find(candidate => candidate.item_id === item.reply_to_item_id);
  if (!parent) return [];
  return [{ id: item.item_id, ...connectBounds(itemBounds(parent, point), itemBounds(item, point)) }];
}));
const explicitConnections = computed(() => links.value.flatMap(link => {
  if (props.visibleLinkIds !== undefined && !props.visibleLinkIds.includes(link.link_id)) return [];
  const geometry = linkGeometry(link, shownItems.value, links.value, point);
  return geometry ? [{ ...link, labelText: link.label, ...geometry }] : [];
}));
let pointer: { id: number; startX: number; startY: number; x: number; y: number; itemId: string | null; moved: boolean } | null = null;

function start(event: PointerEvent, item?: BoardItem) {
  if (props.disabled || (item && props.readonly) || event.button !== 0 || (event.target as Element).closest('button, a, input, [data-board-link]')) return;
  if (!item && (event.target as Element).closest('.board-card,.board-drawing')) return;
  event.preventDefault();
  pointer = { id: event.pointerId, startX: event.clientX, startY: event.clientY,
    x: item?.position.x ?? props.view.x, y: item?.position.y ?? props.view.y, itemId: item?.item_id ?? null, moved: false };
  dragging.value = item?.item_id ?? null;
  dragPosition.value = item ? { ...item.position } : null;
  panning.value = !item;
  surface.value?.setPointerCapture(event.pointerId);
}
function move(event: PointerEvent) {
  if (!pointer || pointer.id !== event.pointerId) return;
  const dx = event.clientX - pointer.startX, dy = event.clientY - pointer.startY;
  if (!pointer.moved && Math.abs(dx) + Math.abs(dy) > 3) { pointer.moved = true; emit('user-control', pointer.itemId ? 'drag' : 'pan'); }
  if (pointer.itemId) dragPosition.value = { x: Math.round(pointer.x + dx / props.view.zoom), y: Math.round(pointer.y + dy / props.view.zoom) };
  else emit('view', { ...props.view, x: pointer.x + dx, y: pointer.y + dy });
}
function finish(event: PointerEvent) {
  if (!pointer || pointer.id !== event.pointerId) return;
  if (pointer.itemId && pointer.moved && dragPosition.value) emit('move', pointer.itemId, dragPosition.value);
  else if (pointer.itemId) emit('select', pointer.itemId);
  if (surface.value?.hasPointerCapture(event.pointerId)) surface.value.releasePointerCapture(event.pointerId);
  pointer = null; dragging.value = null; dragPosition.value = null; panning.value = false;
}
function cancel(event: PointerEvent) {
  if (!pointer || pointer.id !== event.pointerId) return;
  if (surface.value?.hasPointerCapture(event.pointerId)) surface.value.releasePointerCapture(event.pointerId);
  pointer = null; dragging.value = null; dragPosition.value = null; panning.value = false;
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
function fit() { if (surface.value) emit('view', fitBoard(shownItems.value, surface.value.clientWidth, surface.value.clientHeight, props.view)); }
function fitFromControl() { if (!props.disabled) { emit('user-control', 'zoom'); fit(); } }
function locate(itemId: string) {
  focusTargets([{ kind: 'item', item_id: itemId }]);
}
function focusTargets(targets: BoardVisualTarget[]) {
  if (!surface.value) return;
  const bounds = unionBounds(targets.map(target => targetBounds(target, props.items, links.value, point)).filter((bounds): bounds is NonNullable<typeof bounds> => bounds !== null));
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
function textLayout(item: BoardItem) {
  if (item.target.kind !== 'drawing' || item.target.graphic.kind === 'path') return { lines: [], fontSize: 20, lineHeight: 28, startY: 0 };
  const shape = item.target.graphic.kind === 'shape';
  const ellipse = item.target.graphic.kind === 'shape' && item.target.graphic.shape === 'ellipse';
  const width = Math.max(16, ellipse ? item.size.width * .7 : item.size.width - (shape ? 36 : 16));
  const height = Math.max(16, ellipse ? item.size.height * .7 : item.size.height - (shape ? 28 : 12));
  let fontSize = shape ? 19 : 22;
  const fullText = item.target.graphic.text;
  while (fontSize > 10 && graphicTextLines(fullText, width, fontSize).length * fontSize * 1.45 > height) fontSize--;
  const fullLines = graphicTextLines(fullText, width, fontSize), lines = graphicTextLines(revealText(fullText, item.item_id), width, fontSize);
  const lineHeight = fontSize * 1.45;
  return { lines, fontSize, lineHeight, startY: shape ? (item.size.height - fullLines.length * lineHeight) / 2 + fontSize : fontSize + 6 };
}
function point(item: BoardItem) { return dragging.value === item.item_id ? dragPosition.value! : item.position; }
function position(item: BoardItem) {
  const location = point(item);
  return { left: `${location.x}px`, top: `${location.y}px`, width: `${item.size.width}px`, height: `${item.size.height}px` };
}
onBeforeUnmount(() => { pointer = null; });
defineExpose({ fit, locate, focusTargets });
</script>

<template>
  <section ref="surface" class="board-canvas" :class="{ panning }" :aria-label="uiText('知识画板')"
    @pointerdown="start($event)" @pointermove="move" @pointerup="finish" @pointercancel="cancel" @wheel="wheel">
    <div class="board-world" :style="worldStyle">
      <svg class="board-connections" width="1" height="1" aria-hidden="true"><defs><marker :id="arrowId" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto"><path d="M 1 1 L 7 4 L 1 7" fill="none" stroke="currentColor" stroke-width="1.3" /></marker></defs><path v-for="connection in connections" :key="connection.id" :data-board-reply-link="connection.id" :d="connection.path" fill="none" stroke="currentColor" stroke-width="1.4" :marker-end="`url(#${arrowId})`" /></svg>
      <svg class="board-connections board-explicit-links" width="1" height="1">
        <g v-for="connection in explicitConnections" :key="connection.link_id" class="board-visual-link" :class="{ 'board-highlighted-link': highlightedLink(connection.link_id) }" :data-board-link="connection.link_id" role="button" tabindex="0" :aria-label="connection.labelText || uiText('查看连接')" @click.stop="!disabled && emit('activate-target', { kind: 'link', link_id: connection.link_id })" @keydown="linkKeydown($event, connection.link_id)">
          <rect class="board-link-touch-target" :x="connection.label.x - targetSize / 2" :y="connection.label.y - targetSize / 2" :width="targetSize" :height="targetSize" fill="transparent" />
          <path class="board-link-hit" :d="connection.path" fill="none" stroke="transparent" stroke-width="18" />
          <path class="board-link-stroke" :d="connection.path" fill="none" stroke="currentColor" stroke-width="1.8" pathLength="1" stroke-dasharray="1" :stroke-dashoffset="1 - drawProgress(connection.link_id)" :marker-end="drawProgress(connection.link_id) >= 1 ? `url(#${arrowId})` : undefined" />
          <text v-if="connection.labelText && drawProgress(connection.link_id) > 0" class="board-link-label" :x="connection.label.x" :y="connection.label.y - 9" text-anchor="middle">{{ revealText(connection.labelText, connection.link_id) }}</text>
        </g>
      </svg>
      <template v-for="item in shownItems" :key="item.item_id">
      <svg v-if="item.target.kind === 'drawing'" class="board-drawing" :class="{ selected: view.selected_item_id === item.item_id, 'board-path-drawing': item.target.graphic.kind === 'path', 'board-highlighted-item': highlightedItem(item), dragging: dragging === item.item_id }" :style="position(item)" :width="item.size.width" :height="item.size.height" :viewBox="`0 0 ${item.size.width} ${item.size.height}`" :data-item-id="item.item_id" :data-board-graphic="item.target.graphic.kind" @pointerdown.stop="start($event, item)" @click="readonly && !disabled && emit('select', item.item_id)" @keydown="keyboard($event, item)">
        <g class="board-drawing-control" role="button" tabindex="0" :aria-label="graphicLabel(item)" :aria-pressed="view.selected_item_id === item.item_id">
        <path v-if="item.target.graphic.kind === 'path'" class="board-graphic-path-hit" :d="graphicPath(item.target.graphic.points)" fill="none" stroke="transparent" :stroke-width="Math.max(targetSize, item.target.graphic.width)" stroke-linecap="round" stroke-linejoin="round" pointer-events="stroke" />
        <rect v-else class="board-graphic-touch-target" :x="(item.size.width - Math.max(targetSize, item.size.width)) / 2" :y="(item.size.height - Math.max(targetSize, item.size.height)) / 2" :width="Math.max(targetSize, item.size.width)" :height="Math.max(targetSize, item.size.height)" fill="transparent" />
        <rect class="board-graphic-outline" x="-5" y="-5" :width="item.size.width + 10" :height="item.size.height + 10" rx="8" fill="none" />
        <path v-if="item.target.graphic.kind === 'path'" class="board-native-path" :d="graphicPath(item.target.graphic.points)" :stroke="item.target.graphic.color" :stroke-width="item.target.graphic.width" fill="none" stroke-linecap="round" stroke-linejoin="round" pathLength="1" stroke-dasharray="1" :stroke-dashoffset="1 - drawProgress(item.item_id)" />
        <template v-else>
          <template v-if="item.target.graphic.kind === 'shape'">
            <rect v-if="item.target.graphic.shape === 'rectangle'" x="2" y="2" :width="Math.max(0, item.size.width - 4)" :height="Math.max(0, item.size.height - 4)" rx="9" :stroke="item.target.graphic.color" stroke-width="2" fill="var(--reader-canvas)" :fill-opacity="drawProgress(item.item_id)" pathLength="1" stroke-dasharray="1" :stroke-dashoffset="1 - drawProgress(item.item_id)" />
            <ellipse v-else :cx="item.size.width / 2" :cy="item.size.height / 2" :rx="Math.max(0, item.size.width / 2 - 2)" :ry="Math.max(0, item.size.height / 2 - 2)" :stroke="item.target.graphic.color" stroke-width="2" fill="var(--reader-canvas)" :fill-opacity="drawProgress(item.item_id)" pathLength="1" stroke-dasharray="1" :stroke-dashoffset="1 - drawProgress(item.item_id)" />
          </template>
          <text class="board-native-text" data-board-drawing-text :font-size="textLayout(item).fontSize" :fill="item.target.graphic.kind === 'shape' ? item.target.graphic.color : 'var(--ink)'" :text-anchor="item.target.graphic.kind === 'shape' ? 'middle' : 'start'"><tspan v-for="(line, index) in textLayout(item).lines" :key="index" :x="item.target.graphic.kind === 'shape' ? item.size.width / 2 : 8" :y="textLayout(item).startY + index * textLayout(item).lineHeight">{{ line }}</tspan></text>
        </template>
        </g>
      </svg>
      <article v-else class="board-card" :class="[item.target.kind, { selected: view.selected_item_id === item.item_id, dragging: dragging === item.item_id, 'board-highlighted-item': highlightedItem(item) }]"
        :style="position(item)" :data-item-id="item.item_id" @pointerdown.stop="start($event, item)">
        <div class="board-card-label"><component :is="icons[item.target.kind]" :size="15" aria-hidden="true" /><span>{{ uiText(boardKindLabels[item.target.kind]) }}</span><Grip :size="15" class="board-card-grip" aria-hidden="true" /></div>
        <button type="button" class="board-card-open" :aria-label="item.title" :aria-pressed="view.selected_item_id === item.item_id" @click="emit('select', item.item_id)" @keydown="keyboard($event, item)">
          <strong>{{ revealText(item.title, item.item_id) }}</strong>
          <span class="board-card-preview">{{ revealText(boardItemPreview(item) || uiText('打开查看完整内容'), item.item_id) }}</span>
        </button>
        <div class="board-card-footer"><span v-if="item.reply_to_item_id" class="board-card-reply"><CornerDownRight :size="12" aria-hidden="true" />{{ uiText('追问得到的回答') }}</span><span v-else>{{ item.target.kind === 'understanding' ? uiText('原话与解释分别保留') : item.target.kind === 'presentation' ? uiText('已保留演示与现场') : uiText('完整内容已留存') }}</span><span v-if="item.source_bindings.length">{{ uiText('{0} 处原文', [item.source_bindings.length]) }}</span></div>
      </article>
      </template>
    </div>
    <div v-if="!items.length" class="board-canvas-empty">
      <span class="board-empty-symbol"><Sparkles :size="26" aria-hidden="true" /></span>
      <h3>{{ uiText('想理解什么？从这里开始') }}</h3>
      <p>{{ uiText('在下方说出一个主题或问题。我们会一边讨论，一边把概念、关系和你的理解画在这里。') }}</p>
      <button v-if="!readonly" type="button" @click="emit('ask')"><MessageSquareText :size="17" aria-hidden="true" />{{ uiText('开始讨论') }}</button>
    </div>
    <div class="board-canvas-tools" role="group" :aria-label="uiText('画板视图')" @pointerdown.stop>
      <button type="button" :aria-label="uiText('缩小画板')" :disabled="disabled || view.zoom <= .25" @click="zoom(view.zoom - .1)"><Minus :size="17" aria-hidden="true" /></button>
      <span>{{ Math.round(view.zoom * 100) }}%</span>
      <button type="button" :aria-label="uiText('放大画板')" :disabled="disabled || view.zoom >= 1.75" @click="zoom(view.zoom + .1)"><Plus :size="17" aria-hidden="true" /></button>
      <span class="board-tool-divider"></span>
      <button type="button" :aria-label="uiText('显示全部内容')" :title="uiText('显示全部内容')" :disabled="disabled" @click="fitFromControl"><Maximize :size="17" aria-hidden="true" /></button>
    </div>
    <p class="board-canvas-hint">{{ readonly ? uiText('拖动空白移动画板') : uiText('拖动卡片整理 · 拖动空白移动画板') }}</p>
  </section>
</template>

<style scoped>
.board-drawing.board-path-drawing{pointer-events:none}.board-native-path{pointer-events:none}
button{min-height:0;padding:0;border-radius:0}
.board-drawing-control:focus-visible{outline:none}.board-drawing:focus-within .board-graphic-outline{stroke:var(--brand-accent)}
.board-drawing{position:absolute;overflow:visible;pointer-events:all;cursor:grab;touch-action:none}.board-drawing.dragging{cursor:grabbing;z-index:5}.board-graphic-outline{stroke:transparent;stroke-width:1.5;stroke-dasharray:6 5;pointer-events:none}.board-drawing:hover .board-graphic-outline{stroke:var(--hairline)}.board-drawing.selected .board-graphic-outline,.board-drawing:focus-visible .board-graphic-outline{stroke:var(--brand-accent)}.board-drawing.board-highlighted-item .board-graphic-outline{stroke:var(--brand-accent);stroke-width:2.5;stroke-dasharray:none;filter:drop-shadow(0 0 4px color-mix(in srgb,var(--brand-accent) 30%,transparent))}.board-native-text{font-family:var(--sans);font-weight:500;user-select:none}.board-card.board-highlighted-item{outline:3px solid var(--brand-accent);outline-offset:5px;box-shadow:0 0 0 8px color-mix(in srgb,var(--brand-accent) 8%,transparent)}.board-visual-link{pointer-events:all;cursor:pointer;color:var(--steel)}.board-link-hit{pointer-events:stroke}.board-link-stroke{pointer-events:none}.board-link-label{font:12px var(--sans);fill:currentColor;stroke:var(--reader-canvas);stroke-width:5px;stroke-linejoin:round;paint-order:stroke}.board-visual-link:hover,.board-visual-link:focus-visible,.board-highlighted-link{color:var(--brand-accent);outline:none}.board-highlighted-link .board-link-stroke,.board-visual-link:focus-visible .board-link-stroke{stroke-width:3.5}.board-explicit-links{z-index:1}.board-card,.board-drawing{z-index:2}
.board-connections{position:absolute;left:0;top:0;overflow:visible;color:var(--stone);pointer-events:none}.board-card-reply{display:flex;align-items:center;gap:4px}
@media(pointer:coarse),(max-width:700px){.board-canvas .board-canvas-tools button{width:44px;height:44px}.board-canvas .board-canvas-empty>button{min-height:44px}}
.board-canvas{position:relative;min-width:0;min-height:0;overflow:hidden;background-color:var(--reader-canvas);background-image:radial-gradient(circle,color-mix(in srgb,var(--stone) 35%,transparent) 1px,transparent 1px);background-size:22px 22px;touch-action:none;cursor:grab;isolation:isolate}.board-canvas.panning{cursor:grabbing}.board-world{position:absolute;inset:0;transform-origin:0 0;pointer-events:none}.board-card{position:absolute;display:flex;flex-direction:column;padding:17px 19px 13px;border:1px solid var(--hairline);border-top:3px solid var(--card-accent);border-radius:12px;background:var(--reader-card);box-shadow:0 3px 15px #25232008;pointer-events:auto;--card-accent:#92846a;--card-tint:color-mix(in srgb,var(--card-accent) 11%,var(--reader-card))}.board-card.prebuild{--card-accent:#769780}.board-card.answer{--card-accent:#788baa}.board-card.presentation{--card-accent:#b88a63}.board-card.understanding{--card-accent:#a887ad}.board-card.selected{outline:2px solid var(--card-accent);outline-offset:3px;box-shadow:0 5px 24px #25232010}.board-card.dragging{z-index:5;box-shadow:0 12px 30px #25232020;cursor:grabbing}.board-card-label{display:flex;align-items:center;gap:6px;color:var(--card-accent);font-size:11px;font-weight:650;letter-spacing:.025em}.board-card-grip{margin-left:auto;opacity:.55}.board-card-open{display:flex;flex:1;min-height:0;flex-direction:column;gap:10px;padding:12px 0 8px;border:0;background:transparent;color:var(--ink);text-align:left;cursor:pointer;font:inherit}.board-card-open strong{display:-webkit-box;overflow:hidden;-webkit-line-clamp:2;-webkit-box-orient:vertical;font-size:16px;line-height:1.5;font-weight:620}.board-card-preview{display:-webkit-box;-webkit-line-clamp:3;-webkit-box-orient:vertical;overflow:hidden;font-size:12px;line-height:1.75;color:var(--steel)}.board-card-footer{display:flex;justify-content:space-between;gap:8px;padding-top:9px;border-top:1px solid var(--hairline-soft);font-size:10px;color:var(--stone)}.board-canvas-tools{position:absolute;bottom:23px;left:22px;display:flex;align-items:center;gap:3px;padding:5px;border:1px solid var(--hairline);border-radius:10px;background:var(--reader-card);box-shadow:0 3px 12px #25232009;cursor:default}.board-canvas-tools button{display:grid;place-items:center;width:32px;height:32px;border:0;border-radius:6px;background:transparent;color:var(--slate);cursor:pointer}.board-canvas-tools button:hover{background:var(--surface)}.board-canvas-tools button:disabled{opacity:.35;cursor:default}.board-canvas-tools>span:not(.board-tool-divider){min-width:42px;text-align:center;font-size:11px;font-variant-numeric:tabular-nums;color:var(--steel)}.board-tool-divider{height:18px;width:1px;margin:0 4px;background:var(--hairline)}.board-canvas-hint{position:absolute;bottom:23px;right:22px;margin:0;font-size:10px;color:var(--stone);pointer-events:none}.board-canvas-empty{position:absolute;left:50%;top:43%;transform:translate(-50%,-50%);width:min(360px,calc(100% - 48px));text-align:center;cursor:default}.board-empty-symbol{display:inline-grid;place-items:center;width:58px;height:58px;border:1px solid var(--hairline);border-radius:18px;background:var(--reader-card);color:var(--brand-accent)}.board-canvas-empty h3{margin:18px 0 10px;font-size:20px;font-weight:560;line-height:1.5}.board-canvas-empty p{font-size:13px;line-height:1.9;color:var(--steel)}.board-canvas-empty button{display:inline-flex;align-items:center;gap:6px;margin-top:12px;padding:10px 16px;background:var(--brand-accent);color:white;border:0;border-radius:8px;cursor:pointer;font:inherit;font-size:13px}button:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}@media(max-width:700px){.board-canvas-hint{display:none}.board-canvas-tools{bottom:16px;left:16px}}
</style>
