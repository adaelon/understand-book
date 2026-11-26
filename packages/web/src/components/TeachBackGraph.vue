<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue';
import { BookOpen, MessageSquare } from '@lucide/vue';
import { t as uiText } from '../i18n';
import type { TeachBackBookGraph } from '../generated/TeachBackBookGraph';
import type { TeachBackStudentGraph } from '../generated/TeachBackStudentGraph';
import type { TeachBackRow } from '../generated/TeachBackRow';
import { focusNumber, graphLayout, relationLabels, type GraphNode, type GraphSide } from '../teach-back-layout';

const props = defineProps<{
  graph: TeachBackBookGraph | TeachBackStudentGraph;
  side: GraphSide; rows: TeachBackRow[]; selected: string;
}>();
const emit = defineEmits<{ select: [rowId: string, kind: 'node' | 'edge', id: string] }>();
const canvas = ref<HTMLElement>();
const width = ref(280);
const layout = computed(() => graphLayout(props.graph, props.side, props.rows, width.value >= 410 ? 3 : 2));
const selectedIndex = computed(() => props.rows.findIndex(row => row.row_id === props.selected));
const title = computed(() => uiText(props.side === 'book' ? '书中推理' : '本次表达'));
const countLabel = computed(() => props.graph.edges.length
  ? uiText('{0} 条关系', [props.graph.edges.length])
  : uiText('{0} 个独立判断', [props.graph.nodes.length]));
function nodeContext(node: GraphNode) {
  if (node.condition) return node.condition;
  const incoming = props.graph.edges.filter(edge => edge.to === node.id);
  const outgoing = props.graph.edges.some(edge => edge.from === node.id);
  if (!incoming.length) return uiText(outgoing ? '推理起点' : '独立陈述');
  if (incoming.every(edge => edge.relation_type === 'requires')) return uiText('所需条件');
  return uiText(outgoing ? '中间命题' : '推理落点');
}
const marker = useId();
const paths = ref<Record<string, string>>({});
const nodeGroups = (id: string) => layout.value.nodeGroups.get(id) ?? [];
const edgeGroup = (id: string) => {
  const groups = layout.value.edgeGroups.get(id) ?? [];
  return groups.includes(selectedIndex.value) ? selectedIndex.value : groups[0];
};
const activeNode = (id: string) => nodeGroups(id).includes(selectedIndex.value);
const nodeOwner = (id: string) => activeNode(id) ? selectedIndex.value : nodeGroups(id)[0];
function selectNode(id: string) {
  const groups = nodeGroups(id);
  if (!groups.length) return;
  const next = (groups.indexOf(selectedIndex.value) + 1) % groups.length;
  emit('select', props.rows[groups[next]].row_id, 'node', id);
}
function selectEdge(id: string) {
  const row = props.rows[edgeGroup(id)];
  if (row) emit('select', row.row_id, 'edge', id);
}

type Point = { x: number; y: number };
function measure() {
  const element = canvas.value;
  if (!element) return;
  const bounds = element.getBoundingClientRect();
  if (!bounds.width) return;
  width.value = bounds.width;
  const rects = (selector: string, key: 'nodeId' | 'edgeLabel') => new Map(
    Array.from(element.querySelectorAll<HTMLElement>(selector), el => [el.dataset[key]!, el.getBoundingClientRect()]));
  const nodes = rects('[data-node-id]', 'nodeId');
  const centerX = (box: DOMRect) => box.left - bounds.left + box.width / 2;
  const captionCenters = new Map<string, number>();
  // Align captions with their links. Fan-in, fan-out and bypass arrows can
  // share a channel, so give adjacent captions enough room for either locale.
  for (const band of element.querySelectorAll<HTMLElement>('.edge-level')) {
    const limit = band.offsetLeft + band.offsetWidth;
    let left = band.offsetLeft;
    const captions = Array.from(band.querySelectorAll<HTMLElement>('[data-edge-label]'), tag => {
      const edge = props.graph.edges.find(e => e.id === tag.dataset.edgeLabel)!;
      const from = nodes.get(edge.from)!, to = nodes.get(edge.to)!;
      const x = layout.value.bypass.some(e => e.id === edge.id) ? limit : (centerX(from) + centerX(to)) / 2;
      const w = tag.getBoundingClientRect().width;
      const start = Math.max(left, x - w / 2);
      left = start + w + 3;
      return { tag, w, start };
    });
    let right = limit;
    for (const caption of captions.reverse()) {
      const start = Math.min(caption.start, right - caption.w);
      caption.tag.style.transform = `translateX(${start - caption.tag.offsetLeft}px)`;
      captionCenters.set(caption.tag.dataset.edgeLabel!, start + caption.w / 2);
      right = start - 3;
    }
  }
  const labels = rects('[data-edge-label]', 'edgeLabel');
  // Smooth bends leave node borders perpendicular, with room for the arrowhead.
  function bend(a: Point, b: Point) {
    const y = (a.y + b.y) / 2;
    return `C ${a.x} ${y}, ${b.x} ${y}, ${b.x} ${b.y}`;
  }
  const next: Record<string, string> = {};
  props.graph.edges.forEach(edge => {
    const from = nodes.get(edge.from), to = nodes.get(edge.to), label = labels.get(edge.id);
    if (!from || !to || !label) return;
    if (layout.value.inlineIds.has(edge.id)) {
      const right = from.left < to.left;
      const x1 = (right ? from.right : from.left) - bounds.left;
      const x2 = (right ? to.left - 1 : to.right + 1) - bounds.left;
      const y1 = from.top - bounds.top + from.height / 2;
      const y2 = to.top - bounds.top + to.height / 2;
      next[edge.id] = `M ${x1} ${y1} L ${x2} ${y2}`;
      return;
    }
    const start = { x: centerX(from), y: from.bottom - bounds.top };
    const via = { x: captionCenters.get(edge.id)!, y: label.top - bounds.top + label.height / 2 };
    const end = { x: centerX(to), y: to.top - bounds.top - 2 };
    let d = `M ${start.x} ${start.y} ${bend(start, via)}`;
    const lane = layout.value.bypass.findIndex(e => e.id === edge.id);
    if (lane >= 0) {
      const x = bounds.width - 3 - 8 * lane;
      // Skipped levels and return arrows travel outside the node columns.
      d += ` H ${x} V ${end.y - 8} H ${end.x} V ${end.y}`;
    } else d += ' ' + bend(via, end);
    next[edge.id] = d;
  });
  paths.value = next;
}

watch([canvas, layout], ([element], _, onCleanup) => {
  if (!element) return;
  measure();
  const observer = new ResizeObserver(measure);
  observer.observe(element);
  for (const el of element.querySelectorAll('[data-node-id], [data-edge-label]')) observer.observe(el);
  onCleanup(() => observer.disconnect());
}, { flush: 'post' });
</script>

<template>
  <section class="teach-back-graph" :data-side="side" :aria-label="title">
    <header class="graph-header">
      <h4><BookOpen v-if="side === 'book'" aria-hidden="true" /><MessageSquare v-else aria-hidden="true" />{{ title }}</h4>
      <span>{{ countLabel }}</span>
    </header>
    <p v-if="!graph.nodes.length" class="graph-empty">{{ uiText(side === 'book' ? '未找到对应的书中关系' : '本次表达没有提到这条关系') }}</p>
    <div v-else ref="canvas" class="graph-canvas" :style="{ paddingRight: layout.bypass.length ? (8 + 9 * layout.bypass.length) + 'px' : undefined }">
      <svg class="graph-wires" aria-hidden="true">
        <defs>
          <marker v-for="(edge, i) in graph.edges" :id="marker + '-' + i" :key="edge.id"
            :data-active="edgeGroup(edge.id) === selectedIndex" markerUnits="userSpaceOnUse"
            markerWidth="8" markerHeight="8" viewBox="0 0 9 9" refX="8" refY="4.5" orient="auto">
            <path d="M 0 0 L 8 4.5 L 0 9 Z" fill="var(--graph-ink)" />
          </marker>
        </defs>
        <g v-for="(edge, i) in graph.edges" :key="edge.id" :data-edge-id="edge.id"
          :data-status="rows[edgeGroup(edge.id)]?.status" :data-active="edgeGroup(edge.id) === selectedIndex"
          :data-from="edge.from" :data-to="edge.to" @click="selectEdge(edge.id)">
          <path class="wire-hit" :d="paths[edge.id]" />
          <path class="wire" :d="paths[edge.id]" :marker-end="`url(#${marker}-${i})`" />
        </g>
      </svg>
      <div v-for="(layer, i) in layout.layers" :key="i" class="layer">
        <div class="node-level" :class="{ 'inline-step': layer.inlineEdge }">
          <button v-for="(node, n) in layer.nodes" :key="node.id" type="button" class="graph-node" :data-node-id="node.id"
            :style="layer.inlineEdge ? { gridColumn: (layer.reversed ? 1 - n : n) * 2 + 1, gridRow: 1 } : undefined"
            :data-status="rows[nodeOwner(node.id)]?.status" :data-active="activeNode(node.id)"
            :aria-pressed="activeNode(node.id)" @click="selectNode(node.id)">
            <span class="node-label"><span class="node-badges">{{ nodeGroups(node.id).map(focusNumber).join('') }}</span>{{ node.label }}</span>
            <small>{{ nodeContext(node) }}</small>
          </button>
          <button v-if="layer.inlineEdge" type="button" class="edge-tag inline-tag" :data-edge-label="layer.inlineEdge.id"
            :data-status="rows[edgeGroup(layer.inlineEdge.id)]?.status" :data-active="edgeGroup(layer.inlineEdge.id) === selectedIndex"
            :title="layer.inlineEdge.condition || undefined" :aria-pressed="edgeGroup(layer.inlineEdge.id) === selectedIndex"
            @click="selectEdge(layer.inlineEdge.id)">{{ uiText(relationLabels[layer.inlineEdge.relation_type]) }}</button>
        </div>
        <div v-if="layer.outgoing.length" class="edge-level">
          <button v-for="edge in layer.outgoing" :key="edge.id" :data-edge-label="edge.id" type="button" class="edge-tag"
            :data-status="rows[edgeGroup(edge.id)]?.status" :data-active="edgeGroup(edge.id) === selectedIndex"
            :title="edge.condition || undefined" :aria-pressed="edgeGroup(edge.id) === selectedIndex"
            @click="selectEdge(edge.id)">
            {{ uiText(relationLabels[edge.relation_type]) }}
          </button>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.teach-back-graph { --panel: #f8f5ef; --graph-ink: #928a7f; min-width: 0; box-sizing: border-box; border: 1px solid #e0d8cd; border-radius: 10px; background: var(--panel); color: var(--reader-ink); padding: 0 12px 2px; }
.graph-header { display: flex; justify-content: space-between; flex-wrap: wrap; align-items: center; gap: 3px 8px; padding: 12px 0 10px; border-bottom: 1px solid #dfd8ce; }
.graph-header h4 { display: inline-flex; gap: 7px; align-items: center; margin: 0; color: #383731; font: 650 14px/1.4 var(--sans); }
.graph-header h4 svg { width: 16px; height: 16px; color: currentColor; stroke-width: 1.7; }
.graph-header > span { color: var(--reader-muted); font: 500 12px/1.3 var(--sans); }
.graph-empty { padding: 0 2px; color: var(--reader-muted); font-size: 12px; }
.graph-canvas { position: relative; box-sizing: border-box; padding: 17px 0 18px; }
.graph-wires { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }
.wire, .wire-hit { fill: none; }
.wire { stroke: var(--graph-ink); stroke-width: 1.8px; pointer-events: none; stroke-linecap: round; stroke-linejoin: round; }
.wire-hit { stroke: transparent; stroke-width: 13px; pointer-events: stroke; cursor: pointer; }
.node-level { display: grid; grid-auto-columns: minmax(0, 1fr); grid-auto-flow: column; align-items: center; gap: 8px; }
.node-level.inline-step { grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr); column-gap: clamp(7px, 2cqi, 12px); }
.layer:has(.node-level:last-child) + .layer { margin-top: 12px; }
.graph-node, .edge-tag { position: relative; min-width: 0; max-width: 100%; box-sizing: border-box; overflow-wrap: anywhere; font-family: var(--sans); cursor: pointer; }
.graph-node { display: flex; flex-direction: column; justify-content: center; gap: 3px; justify-self: center; width: 100%; min-height: 55px; padding: 8px 10px; border: 1.5px solid #d4cbbf; border-radius: 8px; background: var(--reader-card); color: #514e47; font-size: 13px; line-height: 1.35; font-weight: 650; text-align: start; }
.graph-node:only-child { width: min(100%, 235px); }
.node-label { display: block; }
.graph-node small { color: var(--reader-muted); font-size: 11px; line-height: 1.32; font-weight: 440; }
.node-badges { margin-inline-end: .35em; color: #71695f; font: 650 .99em/1 system-ui, sans-serif; }
.edge-level { display: flex; justify-content: space-evenly; align-items: center; gap: 3px; padding: 10px 1px 12px; }
.edge-tag { min-height: 21px; padding: 1px 5px; border: 1px solid #d9d1c6; border-radius: 4px; color: #655e55; font-size: 11px; line-height: 1.35; font-weight: 500; background: var(--panel); transition: none; }
.inline-tag { grid-column: 2; grid-row: 1; max-width: 75px; }
.edge-tag::after { content: ''; position: absolute; inset: -6px -1px; }
g[data-active="true"], marker[data-active="true"] { --graph-ink: var(--brand-accent); }
.graph-node[data-active="true"] { background: #f8ede2; border-color: #c49a78; color: var(--reader-ink); }
.graph-node[data-active="true"] small { color: #655f56; }
.graph-node[data-active="true"] .node-badges { color: var(--brand-accent-deep); }
.edge-tag[data-active="true"] { color: var(--brand-accent-deep); background: #fff7ed; border-color: var(--brand-accent-line); }
button:hover { border-color: var(--brand-accent); }
button:focus-visible { outline: 2px solid var(--brand-accent-deep); outline-offset: 2px; }
.graph-node:is([data-status="omitted"], [data-status="unverified_against_passage"], [data-status="insufficient_book_basis"]) { border-style: dashed; }
:is([data-status="omitted"], [data-status="unverified_against_passage"], [data-status="insufficient_book_basis"]) .wire { stroke-dasharray: 4 3; }
</style>
