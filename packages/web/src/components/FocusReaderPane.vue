<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { FocusReader } from '../reader-focus';
import { FocusPageTurn } from '../reader-focus-turn';
import type { FocusSource } from '../reader-focus-source';
import type { SourceCursor, PaginationOptions } from '../reader-pagination';
import { readReaderSelection } from '../useReaderSelection';
import { typographyFont, type ReaderTypographyPreferences } from '../reader-typography';
import '../reader-pagination.css';

const props = defineProps<{ source: FocusSource; anchor: SourceCursor; typography: ReaderTypographyPreferences; active: boolean;
  decorations?: PaginationOptions['decorations']; decorationRevision?: unknown; placementActive?: boolean; returnLabel?: string; surfaceLabel?: string }>();
const emit = defineEmits<{ (event: 'current', cursor: SourceCursor): void; (event: 'confirmed'): void; (event: 'tool', tool: string): void;
  (event: 'annotation', lid: string, id: string, trigger: HTMLElement): void;
  (event: 'select', lid: string): void; (event: 'formula', lid: string): void }>();
const host = ref<HTMLElement>();
const state = ref({ phase: 'idle', error: null as string | null });
const toolsOpen = ref(false);
const atStart = ref(true), atEnd = ref(false);
let reader: FocusReader | undefined;
let turn: FocusPageTurn | undefined;
let deferredResize = false;
let observer: ResizeObserver | undefined;
let lastGeometry = '';
let lastId = 0;
let confirmedId = 0;
let selecting = false;
let composing = false;
let suspended = false;
function confirm() {
  const current = reader?.pagination.visible();
  if (!current || current.element.inert || !props.active || turn?.busy() || confirmedId === current.id) return;
  confirmedId = current.id; emit('confirmed');
}
function layout() {
  const p = props.typography;
  return { width: host.value!.clientWidth, height: host.value!.clientHeight, fontSize: p.fontSizePx,
    fontFamily: `"${typographyFont(p.font)}"`, lineHeight: p.lineHeight, mobile: innerWidth <= 600,
    letterSpacingEm: p.languageMode === 'latin' ? 0 : p.cjkLetterSpacingEm, paragraphGapEm: p.languageMode === 'latin' ? 1.15 : 1.2,
    maxPageWidthEm: p.languageMode === 'latin' ? { narrow: 30, standard: 34, wide: 37.5 }[p.measure] : { narrow: 32, standard: 36, wide: 38 }[p.measure] };
}
function changed() {
  if (!reader) return;
  state.value = { ...reader.state };
  const current = reader.pagination.visible();
  atStart.value = !current || current.start.index === 0 && current.start.offset === 0;
  atEnd.value = !!current?.eof;
  if (current && !current.element.inert && lastId !== current.id) { lastId = current.id; emit('current', reader.pagination.anchor()!); }
  if (!turn?.busy()) confirm();
}
function resize(force = false) {
  if (!reader || !host.value || !props.active || suspended || document.hidden) return;
  if (turn?.busy() || props.placementActive || selecting || composing || host.value && readReaderSelection(host.value)) { deferredResize = true; return; }
  const next = layout();
  if (next.width <= 0 || next.height <= 0) { reader.setVisible(false); return; }
  reader.setVisible(true);
  turn?.bounds(host.value.getBoundingClientRect().left, next.width);
  const key = JSON.stringify(next);
  if (force || key !== lastGeometry) {
    lastGeometry = key;
    reader.open(reader.pagination.anchor() ?? props.anchor, next);
  }
}
function rebuild() {
  turn?.dispose();
  reader?.dispose(); lastGeometry = ''; lastId = 0; confirmedId = 0;
  reader = new FocusReader(host.value!, props.source, changed, (lid, source) => props.decorations?.(lid, source) ?? { marks: [], markers: [] });
  turn = new FocusPageTurn(reader, host.value!, {
    reducedMotion: () => matchMedia('(prefers-reduced-motion: reduce)').matches,
    tools: () => { toolsOpen.value = !toolsOpen.value; },
    settled: () => { confirm(); if (deferredResize) { deferredResize = false; resize(); } },
  });
  turn.setEnabled(props.active && !document.hidden && !props.placementActive);
  reader.setSelectionPaused(!!props.placementActive);
  resize(true);
}
function visibility() { turn?.setEnabled(props.active && !suspended && !document.hidden && !props.placementActive); reader?.setVisible(props.active && !suspended && !document.hidden); resize(); }
function tool(name: string) { toolsOpen.value = false; emit('tool', name); }
function selectionChanged() {
  const selected = !!host.value && !!readReaderSelection(host.value);
  reader?.setSelectionPaused(!!props.placementActive || selecting || composing || selected);
  if (!props.placementActive && !selecting && !composing && !selected && deferredResize) { deferredResize = false; resize(); }
}
function pointerDown(event: PointerEvent) {
  if (event.pointerType === 'mouse' && event.button === 0 && (event.target as Element)?.closest('[data-lid]')) { selecting = true; reader?.setSelectionPaused(true); }
}
function pointerUp() { selecting = false; selectionChanged(); }
function compositionStart() { composing = true; selectionChanged(); }
function compositionEnd() { composing = false; selectionChanged(); }
function click(event: MouseEvent) {
  const target = event.target as Element;
  const screen = target.closest<HTMLElement>('.focus-screen');
  if (!screen || screen.inert || screen.hidden || screen !== reader?.pagination.visible()?.element || props.placementActive) return;
  const marker = target.closest<HTMLElement>('[data-annotation-lid]');
  if (marker) { emit('annotation', marker.dataset.annotationLid!, marker.dataset.annotationId!, marker); return; }
  if (target.closest('a, button') || readReaderSelection(host.value!)) return;
  const lid = target.closest<HTMLElement>('[data-lid]')?.dataset.lid;
  if (lid) { if (reader?.pagination.source(lid)?.kind === 'formula') emit('formula', lid); else emit('select', lid); }
}
function key(event: KeyboardEvent) {
  if (!['Enter', ' '].includes(event.key)) return;
  const target = event.target as HTMLElement, lid = target.dataset.lid;
  if (lid && target.getAttribute('role') === 'button' && reader?.pagination.source(lid)?.kind === 'formula') { event.preventDefault(); emit('formula', lid); }
}
onMounted(() => {
  rebuild(); observer = new ResizeObserver(() => resize()); observer.observe(host.value!);
  document.addEventListener('visibilitychange', visibility);
  document.addEventListener('selectionchange', selectionChanged);
  document.addEventListener('pointerup', pointerUp); document.addEventListener('pointercancel', pointerUp);
  document.addEventListener('compositionstart', compositionStart); document.addEventListener('compositionend', compositionEnd);
});
watch(() => props.source, rebuild);
watch(() => props.typography, () => resize(true));
watch(() => props.active, visibility, { flush: 'post' });
watch(() => props.decorationRevision, () => reader?.refreshDecorations());
watch(() => props.placementActive, active => { turn?.setEnabled(props.active && !document.hidden && !active); selectionChanged(); });
onBeforeUnmount(() => {
  turn?.dispose(); reader?.dispose(); observer?.disconnect();
  document.removeEventListener('visibilitychange', visibility);
  document.removeEventListener('selectionchange', selectionChanged);
  document.removeEventListener('pointerup', pointerUp); document.removeEventListener('pointercancel', pointerUp);
  document.removeEventListener('compositionstart', compositionStart); document.removeEventListener('compositionend', compositionEnd);
});
defineExpose({ move: (direction: number) => turn?.turn(direction), snapshot: () => ({ ...reader?.snapshot(), turn: turn?.snapshot() }),
  work: () => ({ ...reader?.pagination.metrics }),
  anchor: () => reader?.pagination.anchor(), open: (cursor: SourceCursor) => {
    suspended = false; turn?.cancel(); lastId = 0; confirmedId = 0;
    const next = layout(); lastGeometry = JSON.stringify(next);
    turn?.bounds(host.value!.getBoundingClientRect().left, next.width);
    turn?.setEnabled(props.active && !document.hidden && !props.placementActive);
    reader?.setVisible(props.active); reader?.open(cursor, next);
  },
  retainCurrent: () => { suspended = true; turn?.setEnabled(false); reader?.retainCurrent(); },
  readingPosition: () => reader?.pagination.readingPosition(),
  selection: (range: Range) => reader?.pagination.selection(range), source: (lid: string) => reader?.pagination.source(lid),
  ranges: () => reader?.pagination.visibleRanges() ?? [],
  rangeRects: (lid: string, start: number, end: number) => reader?.pagination.rangeRects(lid, start, end) ?? [],
  setPaused: (value: boolean) => reader?.setPaused(value) });
</script>

<template>
  <section class="focus-reader-body" aria-label="专注正文">
    <div ref="host" class="focus-reader-surface" @pointerdown="pointerDown" @click="click" @keydown="key"></div>
    <button class="focus-page-prev" aria-label="上一屏" :disabled="atStart" @click="turn?.turn(-1)">‹</button>
    <button class="focus-page-next" aria-label="下一屏" :disabled="atEnd" @click="turn?.turn(1)">›</button>
    <button class="focus-tools-toggle" aria-label="阅读工具" :aria-expanded="toolsOpen" @click="toolsOpen = !toolsOpen">···</button>
    <button v-if="props.returnLabel" class="focus-reading-return" @click="tool('return')">{{ props.returnLabel }}</button>
    <nav v-if="toolsOpen" class="focus-tools" aria-label="专注阅读工具">
      <button @click="tool('outline')">目录</button><button @click="tool('settings')">阅读设置</button>
      <button v-if="props.surfaceLabel" @click="tool('surface')">{{ props.surfaceLabel }}</button>
      <button @click="tool('agent')">问答</button><button @click="tool('notes')">笔记</button><button @click="tool('exit')">退出专注</button>
    </nav>
    <div v-if="state.phase === 'loading' || state.phase === 'waiting' || state.error" class="focus-ready-status" role="status">
      <template v-if="state.error">书页暂未准备好。<button @click="reader?.retry()">重试</button></template>
      <template v-else>正在准备书页…</template>
    </div>
  </section>
</template>
