<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import type { BoardVisualTarget } from '../generated/BoardVisualTarget';
import { MIND_MAP_FONT_FAMILY, topicAppearance, type MindMapNode, type MindMapEdge } from '../mind-map-layout';

const props = defineProps<{
  nodes: MindMapNode[]; edges: MindMapEdge[]; selectedIds: string[]; foldedIds: string[];
  editingId?: string | null; draftTitle?: string; disabled?: boolean; readonly?: boolean;
  zoom?: number;
  dragIds?: string[]; dragDelta?: { x: number; y: number };
  dropCue?: { itemId: string; placement: 'child' | 'before' | 'after' } | null;
  highlightTargets?: BoardVisualTarget[];
}>();
const emit = defineEmits<{
  (event: 'start', pointer: PointerEvent, id: string): void;
  (event: 'select', id: string, additive: boolean): void;
  (event: 'edit', id: string): void;
  (event: 'draft', text: string): void;
  (event: 'commit'): void;
  (event: 'cancel'): void;
  (event: 'keydown', eventValue: KeyboardEvent, id: string): void;
  (event: 'fold', id: string): void;
  (event: 'edge', id: string): void;
}>();
const layer = ref<HTMLElement>();
const composing = ref(false);
const selected = computed(() => new Set(props.selectedIds));
const folded = computed(() => new Set(props.foldedIds));
const dragged = computed(() => new Set(props.dragIds ?? []));
const edgeTargetSize = computed(() => 44 / (props.zoom ?? 1));
const appearance = computed(() => new Map(props.nodes.map(node => [node.item_id, topicAppearance(node)])));
function nodeStyle(node: MindMapNode) {
  const look = appearance.value.get(node.item_id)!;
  const delta = dragged.value.has(node.item_id) ? props.dragDelta : undefined;
  return {
    left: `${node.x + (delta?.x ?? 0)}px`, top: `${node.y + (delta?.y ?? 0)}px`,
    width: `${node.width}px`, height: `${node.height}px`,
    '--topic-fill': look.fill, '--topic-stroke': look.stroke, '--topic-text': look.textColor,
    '--topic-radius': `${look.radius}px`, '--topic-font-size': `${look.fontSize}px`,
    '--topic-line-height': `${node.lineHeight}px`, '--topic-weight': look.fontWeight,
  };
}
function edgeTransform(edge: MindMapEdge) {
  return dragged.value.has(edge.parent_id) && dragged.value.has(edge.child_id)
    ? `translate(${props.dragDelta?.x ?? 0} ${props.dragDelta?.y ?? 0})` : undefined;
}
function highlighted(node: MindMapNode) {
  return props.highlightTargets?.some(target => target.kind === 'item' && target.item_id === node.item_id);
}
function highlightedEdge(id: string) {
  return props.highlightTargets?.some(target => target.kind === 'topic_edge' && target.child_item_id === id);
}
function editKeydown(event: KeyboardEvent) {
  event.stopPropagation();
  if (composing.value || event.isComposing || event.keyCode === 229) return;
  if (event.key === 'Escape') { event.preventDefault(); emit('cancel'); }
  if (event.key === 'Enter' && !event.shiftKey) { event.preventDefault(); emit('commit'); }
}
function resizeInput(input: HTMLTextAreaElement) {
  input.style.height = '0px';
  input.style.height = `${Math.max(54, Math.min(280, input.scrollHeight))}px`;
}
function draft(event: Event) {
  const input = event.target as HTMLTextAreaElement;
  emit('draft', input.value); resizeInput(input);
}
function focusTopic(id: string) {
  const node = Array.from(layer.value?.querySelectorAll<HTMLElement>('[data-topic-id]') ?? [])
    .find(element => element.dataset.topicId === id);
  node?.focus({ preventScroll: true });
}
watch(() => props.editingId, async (id, previousId) => {
  composing.value = false;
  await nextTick();
  if (id) {
    const input = layer.value?.querySelector<HTMLTextAreaElement>('.mind-map-title-input');
    if (input) { resizeInput(input); input.focus({ preventScroll: true }); input.select(); }
  } else if (previousId) focusTopic(previousId);
}, { immediate: true });
watch(() => props.selectedIds, async ids => {
  if (!ids.length || props.editingId || !layer.value?.contains(document.activeElement)) return;
  await nextTick();
  if (!props.editingId) focusTopic(ids.at(-1)!);
});
defineExpose({ focusTopic });
</script>

<template>
  <div ref="layer" class="mind-map-layer" :style="{ fontFamily: MIND_MAP_FONT_FAMILY }" role="tree" aria-label="思维导图" aria-multiselectable="true">
    <svg class="mind-map-edges" width="1" height="1">
      <g v-for="edge in edges" :key="edge.child_id" :transform="edgeTransform(edge)"
        :class="{ 'mind-map-edge-detached': dragged.has(edge.child_id) && !dragged.has(edge.parent_id), 'mind-map-edge-highlighted': highlightedEdge(edge.child_id) }"
        :data-topic-edge="edge.child_id" class="mind-map-edge" role="button" tabindex="0" aria-label="查看主题关系"
        @click.stop="!disabled && emit('edge', edge.child_id)"
        @keydown.enter.prevent.stop="!disabled && emit('edge', edge.child_id)" @keydown.space.prevent.stop="!disabled && emit('edge', edge.child_id)"
        @pointerdown.stop>
        <rect class="mind-map-edge-target" :x="edge.bounds.x + edge.bounds.width / 2 - edgeTargetSize / 2" :y="edge.bounds.y + edge.bounds.height / 2 - edgeTargetSize / 2"
          :width="edgeTargetSize" :height="edgeTargetSize" fill="transparent" pointer-events="all" />
        <path class="mind-map-edge-hit" :d="edge.path" stroke="transparent" stroke-width="16" fill="none" />
        <path class="mind-map-edge-line" :d="edge.path" fill="none" :stroke="appearance.get(edge.child_id)?.stroke" stroke-width="2" />
      </g>
    </svg>
    <div v-for="node in nodes" :key="node.item_id" class="mind-map-topic"
      :class="{ selected: selected.has(node.item_id), root: !node.parent_id, dragging: dragged.has(node.item_id), editing: editingId === node.item_id,
        highlighted: highlighted(node), 'drop-child': dropCue?.itemId === node.item_id && dropCue.placement === 'child',
        'drop-before': dropCue?.itemId === node.item_id && dropCue.placement === 'before', 'drop-after': dropCue?.itemId === node.item_id && dropCue.placement === 'after' }"
      :style="nodeStyle(node)" :data-topic-id="node.item_id" :data-item-id="node.item_id" :data-mind-map-id="node.map_id"
      role="treeitem" :tabindex="selected.has(node.item_id) || (!selectedIds.length && !node.parent_id) ? 0 : -1" :aria-level="node.depth + 1" :aria-selected="selected.has(node.item_id)"
      :aria-expanded="node.item.target.kind === 'topic' && node.item.target.children.length ? !folded.has(node.item_id) : undefined"
      :aria-label="node.item.title" @pointerdown.stop="emit('start', $event, node.item_id)"
      @dblclick.stop="!disabled && !readonly && emit('edit', node.item_id)"
      @keydown="!disabled && emit('keydown', $event, node.item_id)">
      <svg class="mind-map-topic-label" data-board-topic-text :width="node.width" :height="node.height" aria-hidden="true">
        <text v-for="(line, index) in node.lines" :key="index" :x="line.x" :y="line.y" :font-size="node.fontSize" :font-weight="appearance.get(node.item_id)?.fontWeight" fill="currentColor">{{ line.text }}</text>
      </svg>
      <textarea v-if="editingId === node.item_id" class="mind-map-title-input" :data-topic-editor="node.item_id" :value="draftTitle ?? node.item.title" aria-label="编辑主题标题"
        rows="1" spellcheck="false" @pointerdown.stop @click.stop @dblclick.stop @input="draft" @keydown="editKeydown"
        @compositionstart="composing = true" @compositionend="composing = false" />
      <button v-if="node.item.target.kind === 'topic' && node.item.target.children.length && editingId !== node.item_id"
        type="button" class="mind-map-fold" :class="{ left: node.side === 'left' && node.parent_id }"
        :disabled="disabled" :aria-label="folded.has(node.item_id) ? `展开 ${node.hiddenCount} 个子主题` : '收起子主题'"
        :title="folded.has(node.item_id) ? `展开 ${node.hiddenCount} 个子主题` : '收起子主题'" @pointerdown.stop @click.stop="emit('fold', node.item_id)">
        <span v-if="folded.has(node.item_id)" class="mind-map-fold-count">{{ node.hiddenCount }}</span><span v-else aria-hidden="true">−</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.mind-map-layer{position:absolute;inset:0;pointer-events:none}.mind-map-edges{position:absolute;inset:0;overflow:visible}.mind-map-edge{pointer-events:stroke;cursor:pointer;outline:none}.mind-map-edge-line{stroke-linecap:round;stroke-linejoin:round;opacity:.72;pointer-events:none}.mind-map-edge:hover .mind-map-edge-line,.mind-map-edge:focus-visible .mind-map-edge-line,.mind-map-edge-highlighted .mind-map-edge-line{opacity:1;stroke-width:3}.mind-map-edge-detached{opacity:.18;pointer-events:none}
.mind-map-topic{position:absolute;box-sizing:border-box;border:1.5px solid var(--topic-stroke);border-radius:var(--topic-radius);background:var(--topic-fill);color:var(--topic-text);box-shadow:0 2px 7px #20293007;cursor:grab;touch-action:none;pointer-events:auto;user-select:none;outline:none;z-index:2}.mind-map-topic.root{box-shadow:0 4px 15px #2029300c}.mind-map-topic:hover{box-shadow:0 4px 14px #20293014}.mind-map-topic.selected{outline:2px solid var(--brand-accent);outline-offset:4px}.mind-map-topic:focus-visible{outline:2px solid var(--brand-accent);outline-offset:4px}.mind-map-topic.highlighted{box-shadow:0 0 0 7px color-mix(in srgb,var(--brand-accent) 15%,transparent)}.mind-map-topic.dragging{cursor:grabbing;z-index:6;opacity:.88;box-shadow:0 10px 24px #20293022}.mind-map-topic.editing{z-index:8}.mind-map-topic-label{position:absolute;left:-1.5px;top:-1.5px;overflow:visible;pointer-events:none;font-family:inherit}
.mind-map-title-input{position:absolute;left:-2px;top:-2px;box-sizing:border-box;width:max(100%,220px);min-height:100%;max-height:280px;resize:none;overflow:auto;margin:0;padding:14px 19px;border:2px solid var(--brand-accent);border-radius:var(--topic-radius);background:var(--reader-card,#fff);color:var(--ink,#26303a);font-family:inherit;font-size:var(--topic-font-size);font-weight:var(--topic-weight);line-height:var(--topic-line-height);outline:none;box-shadow:0 7px 24px #20293022;user-select:text;touch-action:auto;cursor:text;z-index:2}
.mind-map-fold{position:absolute;right:-14px;top:calc(50% - 13px);display:grid;place-items:center;box-sizing:border-box;min-width:26px;min-height:0;height:26px;padding:0 5px;border:1px solid var(--topic-stroke);border-radius:13px;background:var(--reader-card,#fff);color:var(--topic-stroke);font-family:inherit;font-size:14px;font-weight:600;line-height:1;cursor:pointer;z-index:1}.mind-map-fold.left{left:-14px;right:auto}.mind-map-fold:hover{background:var(--surface,#f3f5f7)}.mind-map-fold:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}.mind-map-fold-count{font-size:10px;font-weight:650}.mind-map-fold:disabled{cursor:default;opacity:.5}
.mind-map-topic.drop-child{outline:3px solid var(--brand-accent);outline-offset:6px;background:color-mix(in srgb,var(--brand-accent) 12%,var(--topic-fill))}.mind-map-topic.drop-before::before,.mind-map-topic.drop-after::after{content:'';position:absolute;left:-10px;right:-10px;height:3px;border-radius:2px;background:var(--brand-accent);box-shadow:0 0 0 3px color-mix(in srgb,var(--brand-accent) 12%,transparent)}.mind-map-topic.drop-before::before{top:-12px}.mind-map-topic.drop-after::after{bottom:-12px}
@media(pointer:coarse){.mind-map-fold::before{content:'';position:absolute;inset:-9px}.mind-map-title-input{font-size:max(16px,var(--topic-font-size))}}
</style>
