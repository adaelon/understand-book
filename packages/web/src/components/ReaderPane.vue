<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { FormulaSemantics, ImageAssetManifestEntry, MemoryRecord } from "../api";
import type { Manifest } from "../api";
import AnnotationPreview from "./AnnotationPreview.vue";
import { isDisplayFormulaSource } from "../md";
import { defaultTypography, typographyFont, typographyHtml, typographyStyle, type ReaderTypographyPreferences } from '../reader-typography';
import { captureTextPosition, textPositionTop, type ScrollAnchor } from '../reader-text-anchor';
import { createReadingContinuity } from '../useReadingContinuity';
import { resolveMarkdownNotePlacementTarget } from "../markdown-note-placement";
import { useReaderSelection, type ReaderSelectionSnapshot } from "../useReaderSelection";

type NodeKind = Manifest["tree"][number]["kind"];
export interface Segment {
  lid: string;
  text: string;
  kind: NodeKind;
  formula: FormulaSemantics | null;
  imageAsset: ImageAssetManifestEntry | null;
}

const props = defineProps<{
  typography?: ReaderTypographyPreferences;
  contextKey?: string;
  segments: Segment[];
  viewportAnchor: string | null;
  selectedLid: string | null;
  notePlacementActive?: boolean;
  renderSeg: (seg: Segment) => string;
  renderMarkdown: (source: string) => string;
  markdownHeadingLevel: (seg: Segment) => number | null;
  isAsset: (seg: Segment) => boolean;
  isHighlighted: (lid: string) => boolean;
  highlightsOf: (lid: string) => MemoryRecord[];
  highlightCardsOf: (lid: string) => MemoryRecord[];
  visibleNotes: MemoryRecord[];
  hlExcerpt: (rec: MemoryRecord) => string;
  imageMeta: (text: string) => { alt: string; src: string } | null;
  imageAsset: (lid: string) => ImageAssetManifestEntry | null;
}>();

type ReaderItem =
  | { type: "flow"; segments: Segment[] }
  | { type: "single"; segment: Segment };

function renderBody(seg: Segment) {
  const html = props.renderSeg(seg);
  return seg.kind === 'paragraph' || seg.kind === 'chapter' || seg.kind === 'section' ? typographyHtml(html) : html;
}

function imageRenderSrc(asset: ImageAssetManifestEntry | null | undefined): string | null {
  if (!asset) return null;
  return asset.url_path ?? (asset.status === "external" ? asset.original_src : null);
}
const emit = defineEmits<{
  (e: "select", lid: string): void;
  (e: "prose-selection-change", snapshot: ReaderSelectionSnapshot | null): void;
  (e: "modify-highlight", rec: MemoryRecord): void;
  (e: "delete-highlight", rec: MemoryRecord): void;
  (e: "edit-note", rec: MemoryRecord): void;
  (e: "delete-note", rec: MemoryRecord): void;
  (e: "show-notes", rec: MemoryRecord): void;
  (e: "place-note", rec: MemoryRecord): void;
  (e: "goto", lid: string): void;
  (e: "focus-source-local", source: { lid: string; quote: string | null }): void;
  (e: "open-formula", seg: Segment): void;
  (e: "scroll-edge", direction: "up" | "down"): void;
  (e: "current-lid", lid: string): void;
  (e: "viewport-interaction"): void;
  (e: "note-placement-pointer", event: PointerEvent): void;
  (e: "note-placement-target", target: { lid: string }): void;
  (e: "note-placement-invalid"): void;
}>();

const notesByLid = computed(() => {
  const map = new Map<string, MemoryRecord[]>();
  for (const note of props.visibleNotes) {
    const lid = note.anchor.lid;
    if (!lid) continue;
    const arr = map.get(lid);
    if (arr) arr.push(note);
    else map.set(lid, [note]);
  }
  return map;
});
function notesOf(lid: string): MemoryRecord[] {
  return notesByLid.value.get(lid) ?? [];
}
function isMarkdownHeading(seg: Segment): boolean {
  return props.markdownHeadingLevel(seg) !== null;
}
function isFlowSegment(seg: Segment): boolean {
  if (seg.kind === "paragraph" && isMarkdownHeading(seg)) return false;
  return seg.kind === "paragraph" || (seg.kind === "formula" && !isDisplayFormulaSource(seg.text));
}
function shouldJoinFlow(prev: Segment, next: Segment): boolean {
  return prev.kind === "formula" || next.kind === "formula";
}
const readerItems = computed<ReaderItem[]>(() => {
  const items: ReaderItem[] = [];
  let flow: Segment[] = [];
  const flush = () => {
    if (!flow.length) return;
    items.push({ type: "flow", segments: flow });
    flow = [];
  };
  for (const seg of props.segments) {
    if (isFlowSegment(seg)) {
      const last = flow[flow.length - 1];
      if (last && !shouldJoinFlow(last, seg)) {
        flush();
      }
      flow.push(seg);
    }
    else {
      flush();
      items.push({ type: "single", segment: seg });
    }
  }
  flush();
  return items;
});
function itemKey(item: ReaderItem): string {
  return item.type === "flow" ? item.segments.map((seg) => seg.lid).join("|") : item.segment.lid;
}
function markdownHeadingClass(seg: Segment): Record<string, boolean> {
  const level = props.markdownHeadingLevel(seg);
  if (level === null) return {};
  return {
    "heading-markdown": true,
    [`heading-markdown-${level}`]: true,
  };
}

const pane = ref<HTMLElement | null>(null);
useReaderSelection(pane, (snapshot) => emit("prose-selection-change", snapshot));
const placementCandidateLid = ref<string | null>(null);
const wrappedAssetLids = ref<Set<string>>(new Set());
const expandedAssetLid = ref<string | null>(null);
const validPlacementLids = computed(() => new Set(props.segments.map((segment) => segment.lid)));
const edgePx = 2;
const preloadScreens = 2;
let pendingCheck = false;
let currentReadingLid: string | null = null;
let lastVisibleAnchor: ScrollAnchor | null = null;
let arrivalTimer: ReturnType<typeof setTimeout> | undefined;
let arrivalNode: HTMLElement | null = null;

const appliedTypography = ref(props.typography ?? { ...defaultTypography });
const bodyStyle = computed(() => typographyStyle(appliedTypography.value));
const reflow = createReadingContinuity();
let reflowPending = false;
let expectedScrollTop = 0;
let selecting = false;
let composing = false;
let deferredTypography = false;
let typographyRequest = 0;
function cancelTypographyRestore() { reflow.cancelRestore(); reflowPending = false; }
function interact() { cancelTypographyRestore(); emit('viewport-interaction'); }

async function applyTypography() {
  if (selecting || composing) { deferredTypography = true; return; }
  deferredTypography = false;
  const request = ++typographyRequest;
  const anchor = captureScrollAnchor(props.segments.map(s => s.lid));
  const token = reflow.beginRestore(props.contextKey ?? 'local');
  const current = () => reflow.isCurrent(token) && token.contextKey === (props.contextKey ?? 'local');
  reflowPending = true;
  expectedScrollTop = pane.value?.scrollTop ?? 0;
  const p = props.typography ?? { ...defaultTypography };
  // Load the actual text's unicode subsets, including headings, from bundled resources.
  const text = props.segments.map(s => s.text).join('');
  if (document.fonts) {
    try { await Promise.all([400, 600].map(weight => document.fonts.load(`${weight} ${p.fontSizePx}px "${typographyFont(p.font)}"`, text))); }
    catch { /* The fallback remains readable when a local font request fails. */ }
  }
  if (request !== typographyRequest || token.contextKey !== (props.contextKey ?? 'local')) return;
  if (selecting || composing) { deferredTypography = true; cancelTypographyRestore(); return; }
  // Keep the previous face until the requested local font is ready. A late face
  // must not reflow an active drag or composition, or restore a superseded position.
  appliedTypography.value = p;
  await nextTick();
  await restoreScrollAnchor(anchor, current);
  if (request !== typographyRequest) return;
  reflowPending = false;
  void scheduleScrollStateCheck();
}
function endSelection() { selecting = false; if (deferredTypography && !composing) void applyTypography(); }
function startComposition() { composing = true; cancelTypographyRestore(); }
function endComposition() { composing = false; if (deferredTypography && !selecting) void applyTypography(); }
watch(() => props.typography, () => { void applyTypography(); });
watch(() => props.contextKey, () => { typographyRequest++; cancelTypographyRestore(); deferredTypography = false; }, { flush: 'sync' });


const annotationGroups = computed(() => props.segments.flatMap(seg => {
  const records = [...notesOf(seg.lid), ...props.highlightCardsOf(seg.lid)];
  return records.length ? [{ lid: seg.lid, records }] : [];
}));
const activeAnnotation = ref<{ lid: string; id: string } | null>(null);
const activeRecords = computed(() => annotationGroups.value.find(g => g.lid === activeAnnotation.value?.lid)?.records ?? []);
const activeRecord = computed(() => activeRecords.value.find(r => r.mem_id === activeAnnotation.value?.id));
const annotationSourceText = computed(() => props.segments.find(s => s.lid === activeAnnotation.value?.lid)?.text.slice(0, 220) ?? '');
const markerStyles = ref<Record<string, Record<string, string>>>({});
const previewStyle = ref<Record<string, string>>({});
let annotationTrigger: HTMLElement | null = null;
let annotationResize: ResizeObserver | null = null;
function closeAnnotation(restoreFocus = false) {
  activeAnnotation.value = null;
  pane.value?.querySelectorAll('[data-annotation-active]').forEach(el => el.removeAttribute('data-annotation-active'));
  if (restoreFocus && annotationTrigger?.isConnected) annotationTrigger.focus({ preventScroll: true });
  annotationTrigger = null;
}
async function toggleAnnotation(lid: string, event: MouseEvent) {
  if (activeAnnotation.value?.lid === lid) { closeAnnotation(true); return; }
  const group = annotationGroups.value.find(g => g.lid === lid);
  if (!group) return;
  closeAnnotation();
  annotationTrigger = event.currentTarget as HTMLElement;
  activeAnnotation.value = { lid, id: group.records[0].mem_id };
  updateAnnotationGeometry();
  await nextTick();
  if (!selecting && !composing && window.getSelection()?.isCollapsed !== false) {
    document.getElementById('reader-annotation-preview')?.focus({ preventScroll: true });
  }
}
function selectAnnotation(id: string) {
  if (activeAnnotation.value) activeAnnotation.value = { ...activeAnnotation.value, id };
  updateAnnotationGeometry();
}
function editAnnotation(record: MemoryRecord) {
  if (record.type === 'highlight') emit('modify-highlight', record);
  else emit('edit-note', record);
  closeAnnotation();
}
function deleteAnnotation(record: MemoryRecord) {
  if (record.type === 'highlight') emit('delete-highlight', record);
  else emit('delete-note', record);
}
function placeAnnotation(record: MemoryRecord) { closeAnnotation(); emit('place-note', record); }
function openAnnotationSource(source: { lid: string; quote: string | null }) { closeAnnotation(true); emit('focus-source-local', source); }
function showAnnotationInNotes(record: MemoryRecord) { closeAnnotation(); emit('show-notes', record); }
function updateAnnotationGeometry() {
  const root = pane.value;
  if (!root) return;
  const rect = root.getBoundingClientRect();
  const styles: Record<string, Record<string, string>> = {};
  for (const group of annotationGroups.value) {
    const node = lidElement(group.lid);
    if (!node) continue;
    const box = node.getBoundingClientRect();
    // A group keeps its trigger position while readers inspect its individual records.
    const record = group.records[0];
    const source = props.segments.find(s => s.lid === group.lid)?.text ?? '';
    const range = record?.selection_context?.ranges.find(r => r.lid === group.lid)?.range ?? record?.range;
    const sourceTop = range ? textPositionTop(node, source, { ...range, text: source.slice(range.start, range.end), top: 0 }) : null;
    const anchorTop = sourceTop ?? box.top;
    const visible = anchorTop + 36 > rect.top && anchorTop < rect.bottom && rect.width > 0;
    styles[group.lid] = {
      top: `${anchorTop - rect.top + root.scrollTop}px`,
      left: `${Math.max(0, box.left - rect.left - 36)}px`,
    };
    if (activeAnnotation.value?.lid === group.lid) {
      if (!visible && (rect.height > 0 || getComputedStyle(root).display === 'none')) { closeAnnotation(); continue; }
      node.setAttribute('data-annotation-active', 'true');
      const sideSpace = window.innerWidth - box.right;
      previewStyle.value = window.innerWidth >= 1024 && sideSpace >= 340
        ? { left: `${box.right + 12}px`, top: `${Math.max(12, Math.min(anchorTop, window.innerHeight - 360))}px`, bottom: 'auto', width: '320px', maxHeight: 'min(520px, calc(100dvh - 24px))' }
        : { left: `${Math.max(8, rect.left + 8)}px`, bottom: `${Math.max(8, window.innerHeight - rect.bottom + 8)}px`, top: 'auto', width: `${Math.max(0, Math.min(rect.width - 16, 480))}px`, maxHeight: 'min(520px, calc(100dvh - 96px))' };
    }
  }
  markerStyles.value = styles;
}
watch([annotationGroups, appliedTypography], async () => {
  if (activeAnnotation.value && !activeRecord.value) closeAnnotation();
  await nextTick(); updateAnnotationGeometry();
}, { flush: 'post' });
watch(() => props.contextKey, () => { lastVisibleAnchor = null; closeAnnotation(); }, { flush: 'sync' });

function atTopEdge(el: HTMLElement): boolean {
  return el.scrollTop <= edgePx;
}

function atBottomEdge(el: HTMLElement): boolean {
  return el.scrollHeight - el.scrollTop - el.clientHeight <= edgePx;
}

function preloadPx(el: HTMLElement): number {
  return Math.max(320, el.clientHeight * preloadScreens);
}

function nearTop(el: HTMLElement): boolean {
  return el.scrollTop <= preloadPx(el);
}

function nearBottom(el: HTMLElement): boolean {
  return el.scrollHeight - el.scrollTop - el.clientHeight <= preloadPx(el);
}

function requestBuffer(direction: "up" | "down") {
  emit("scroll-edge", direction);
}

function checkBufferNeed() {
  const el = pane.value;
  if (!el || props.segments.length === 0) return;
  if (nearBottom(el)) requestBuffer("down");
  if (nearTop(el)) requestBuffer("up");
}

function currentLidAtProbe(): string | null {
  const el = pane.value;
  if (!el) return null;
  const paneRect = el.getBoundingClientRect();
  const probeOffset = Math.min(Math.max(el.clientHeight * 0.28, 96), Math.max(el.clientHeight - 24, 0));
  const probeY = paneRect.top + probeOffset;
  const nodes = Array.from(el.querySelectorAll<HTMLElement>("[data-lid]"));
  let fallback: string | null = null;
  for (const node of nodes) {
    const lid = node.dataset.lid ?? null;
    if (!lid) continue;
    const rect = node.getBoundingClientRect();
    if (rect.bottom < paneRect.top || rect.top > paneRect.bottom) continue;
    if (rect.top <= probeY && rect.bottom >= probeY) return lid;
    if (rect.top <= probeY) fallback = lid;
    else return fallback ?? lid;
  }
  return fallback;
}

function updateCurrentLid() {
  const lid = currentLidAtProbe();
  if (!lid || lid === currentReadingLid) return;
  currentReadingLid = lid;
  emit("current-lid", lid);
}

function checkScrollState() {
  pendingCheck = false;
  if (reflowPending) return;
  updateCurrentLid();
  checkBufferNeed();
}

async function scheduleScrollStateCheck() {
  if (pendingCheck) return;
  pendingCheck = true;
  await nextTick();
  checkScrollState();
}

function onScroll() {
  if (reflowPending && Math.abs((pane.value?.scrollTop ?? 0) - expectedScrollTop) > 1) cancelTypographyRestore();
  updateAnnotationGeometry();
  void scheduleScrollStateCheck();
}

function onWheel(event: WheelEvent) {
  const el = pane.value;
  if (!el) return;
  if (event.deltaY !== 0) interact();
  if (event.deltaY > 0 && atBottomEdge(el)) {
    event.preventDefault();
    requestBuffer("down");
  }
  else if (event.deltaY < 0 && atTopEdge(el)) {
    event.preventDefault();
    requestBuffer("up");
  }
  else {
    void scheduleScrollStateCheck();
  }
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName.toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select" || target.isContentEditable;
}

function onKeydown(event: KeyboardEvent) {
  const el = pane.value;
  if (!el || isEditableTarget(event.target)) return;
  if (event.key === 'Escape') { closeAnnotation(true); return; }
  if ((event.target as Element)?.closest('button, [role="dialog"]')) return;
  const line = 72;
  const page = Math.max(line, Math.floor(el.clientHeight * 0.82));
  let delta = 0;
  if (event.key === "ArrowDown") delta = line;
  else if (event.key === "ArrowUp") delta = -line;
  else if (event.key === "PageDown") delta = page;
  else if (event.key === "PageUp") delta = -page;
  else return;
  interact();
  event.preventDefault();
  if (delta > 0 && atBottomEdge(el)) {
    requestBuffer("down");
    return;
  }
  if (delta < 0 && atTopEdge(el)) {
    requestBuffer("up");
    return;
  }
  el.scrollBy({ top: delta, behavior: "instant" });
  void scheduleScrollStateCheck();
}

function markdownPlacementTarget(event: PointerEvent): { lid: string } | null {
  const root = pane.value;
  return root
    ? resolveMarkdownNotePlacementTarget(event, root, validPlacementLids.value)
    : null;
}

function onPointerMove(event: PointerEvent) {
  if (!props.notePlacementActive) return;
  placementCandidateLid.value = markdownPlacementTarget(event)?.lid ?? null;
}

function onPointerLeave() {
  placementCandidateLid.value = null;
}

function onPointerUp(event: PointerEvent) {
  if (!props.notePlacementActive) return;
  emit("note-placement-pointer", event);
  const target = markdownPlacementTarget(event);
  placementCandidateLid.value = target?.lid ?? null;
  if (target) emit("note-placement-target", target);
  else emit("note-placement-invalid");
}

function isTextAsset(segment: Segment): boolean {
  return segment.kind === 'code' || segment.kind === 'table';
}

function toggleAssetWrap(lid: string) {
  const next = new Set(wrappedAssetLids.value);
  if (next.has(lid)) next.delete(lid);
  else next.add(lid);
  wrappedAssetLids.value = next;
}

function toggleAssetExpanded(lid: string) {
  expandedAssetLid.value = expandedAssetLid.value === lid ? null : lid;
}

function lidElement(lid: string): HTMLElement | null {
  const el = pane.value;
  if (!el) return null;
  return Array.from(el.querySelectorAll<HTMLElement>("[data-lid]")).find((node) => node.dataset.lid === lid) ?? null;
}

function captureScrollAnchor(candidateLids: string[]): ScrollAnchor | null {
  const el = pane.value;
  if (!el) return null;
  if (getComputedStyle(el).display === 'none') return lastVisibleAnchor;
  const paneRect = el.getBoundingClientRect();
  let best: { lid: string; top: number; score: number } | null = null;
  for (const lid of candidateLids) {
    const node = lidElement(lid);
    if (!node) continue;
    const rect = node.getBoundingClientRect();
    const top = rect.top - paneRect.top;
    const visible = rect.bottom >= paneRect.top && rect.top <= paneRect.bottom;
    const score = (visible ? 0 : 100_000) + Math.abs(top);
    if (!best || score < best.score) best = { lid, top, score };
  }
  const probeY = paneRect.top + Math.min(Math.max(el.clientHeight * 0.28, 96), Math.max(el.clientHeight - 24, 0));
  const probeLid = currentLidAtProbe();
  const lid = probeLid && candidateLids.includes(probeLid) ? probeLid : best?.lid;
  if (!lid) return null;
  const node = lidElement(lid)!;
  const segment = props.segments.find(s => s.lid === lid);
  const textPosition = segment && ['paragraph', 'chapter', 'section'].includes(segment.kind)
    ? captureTextPosition(node, segment.text, probeY, paneRect.top) : undefined;
  lastVisibleAnchor = { lid, top: node.getBoundingClientRect().top - paneRect.top, ...(textPosition ? { textPosition } : {}) };
  return lastVisibleAnchor;
}

async function restoreScrollAnchor(anchor: ScrollAnchor | null, current: () => boolean = () => true) {
  if (!anchor) return;
  await nextTick();
  if (!current()) return;
  const el = pane.value;
  const node = lidElement(anchor.lid);
  if (!el || !node) return;
  if (getComputedStyle(el).display === 'none') return;
  const paneRect = el.getBoundingClientRect();
  const source = props.segments.find(s => s.lid === anchor.lid)?.text;
  const textTop = anchor.textPosition && source !== undefined ? textPositionTop(node, source, anchor.textPosition) : null;
  const currentTop = (textTop ?? node.getBoundingClientRect().top) - paneRect.top;
  el.scrollTop += currentTop - (textTop !== null ? anchor.textPosition!.top : anchor.top);
  expectedScrollTop = el.scrollTop;
  void scheduleScrollStateCheck();
}

async function scrollLidIntoView(lid: string): Promise<boolean> {
  cancelTypographyRestore();
  await nextTick();
  const el = pane.value;
  const node = lidElement(lid);
  if (!el || !node) return false;
  const paneRect = el.getBoundingClientRect();
  const nodeRect = node.getBoundingClientRect();
  el.scrollTop += nodeRect.top - paneRect.top;
  currentReadingLid = lid;
  if (arrivalTimer) clearTimeout(arrivalTimer);
  arrivalNode?.removeAttribute('data-reader-arrival');
  arrivalNode = node;
  node.setAttribute('data-reader-arrival', 'true');
  arrivalTimer = setTimeout(() => { node.removeAttribute('data-reader-arrival'); arrivalNode = null; }, 900);
  emit("current-lid", lid);
  updateAnnotationGeometry();
  return true;
}

defineExpose({ captureScrollAnchor, restoreScrollAnchor, scrollLidIntoView, cancelTypographyRestore });

onMounted(() => {
  annotationResize = new ResizeObserver(updateAnnotationGeometry);
  if (pane.value) { annotationResize.observe(pane.value); const prose = pane.value.querySelector('.prose'); if (prose) annotationResize.observe(prose); }
  window.addEventListener('resize', updateAnnotationGeometry);
  void nextTick(updateAnnotationGeometry);
  document.addEventListener('pointerup', endSelection);
  document.addEventListener('pointercancel', endSelection);
  document.addEventListener('compositionstart', startComposition);
  document.addEventListener('compositionend', endComposition);
  void applyTypography();
  void scheduleScrollStateCheck();
});
onBeforeUnmount(() => {
  if (arrivalTimer) clearTimeout(arrivalTimer);
  arrivalNode?.removeAttribute('data-reader-arrival');
  annotationResize?.disconnect();
  window.removeEventListener('resize', updateAnnotationGeometry);
  closeAnnotation();
  typographyRequest++;
  cancelTypographyRestore();
  document.removeEventListener('pointerup', endSelection);
  document.removeEventListener('pointercancel', endSelection);
  document.removeEventListener('compositionstart', startComposition);
  document.removeEventListener('compositionend', endComposition);
});
watch(() => props.notePlacementActive, (active) => {
  if (!active) placementCandidateLid.value = null;
});

watch(
  () => {
    const first = props.segments[0]?.lid ?? "";
    const last = props.segments[props.segments.length - 1]?.lid ?? "";
    return `${props.segments.length}:${first}:${last}`;
  },
  () => {
    void scheduleScrollStateCheck();
  },
);
</script>

<template>
  <main
    ref="pane"
    class="reader-pane"
    tabindex="0"
    @scroll.passive="onScroll"
    @wheel="onWheel"
    @pointerdown="selecting = true; interact()"
    @pointermove="onPointerMove"
    @pointerleave="onPointerLeave"
    @pointerup="onPointerUp"
    @keydown="onKeydown"
  >
    <article class="prose" :style="bodyStyle" :lang="appliedTypography.languageMode === 'latin' ? 'en' : 'zh-Hans'">
      <div v-for="item in readerItems" :key="itemKey(item)" class="seg">
        <template v-if="item.type === 'flow'">
          <p class="flow-paragraph">
            <template v-for="seg in item.segments" :key="seg.lid">
              <button
                v-if="seg.kind === 'formula' && seg.formula"
                :data-lid="seg.lid"
                class="formula-open"
                :class="{
                  anchor: seg.lid === props.viewportAnchor,
                  selected: seg.lid === props.selectedLid,
                  hl: props.isHighlighted(seg.lid),
                  'note-placement-candidate': seg.lid === placementCandidateLid,
                }"
                title="查看公式语义剖面"
                @click.stop="emit('open-formula', seg)"
              >
                <span class="formula-open-source" v-html="renderBody(seg)"></span>
              </button>
              <span
                v-else-if="seg.kind === 'formula'"
                :data-lid="seg.lid"
                class="formula-inline-source"
                :class="{
                  anchor: seg.lid === props.viewportAnchor,
                  selected: seg.lid === props.selectedLid,
                  hl: props.isHighlighted(seg.lid),
                  'note-placement-candidate': seg.lid === placementCandidateLid,
                }"
                @click="emit('select', seg.lid)"
                v-html="renderBody(seg)"
              ></span>
              <span
                v-else
                :data-lid="seg.lid"
                class="flow-text"
                :class="{
                  anchor: seg.lid === props.viewportAnchor,
                  selected: seg.lid === props.selectedLid,
                  hl: props.isHighlighted(seg.lid),
                  'note-placement-candidate': seg.lid === placementCandidateLid,
                }"
                @click="emit('select', seg.lid)"
                v-html="renderBody(seg)"
              ></span>
            </template>
          </p>

        </template>

        <div
          v-else-if="item.segment.kind === 'formula' && isDisplayFormulaSource(item.segment.text)"
          :data-lid="item.segment.lid"
          class="formula-display"
          :class="{
            anchor: item.segment.lid === props.viewportAnchor,
            selected: item.segment.lid === props.selectedLid,
            hl: props.isHighlighted(item.segment.lid),
            'note-placement-candidate': item.segment.lid === placementCandidateLid,
          }"
          @click="!item.segment.formula && emit('select', item.segment.lid)"
        >
          <button
            v-if="item.segment.formula"
            class="formula-open formula-display-open"
            title="查看公式语义剖面"
            @click.stop="emit('open-formula', item.segment)"
          ><span class="formula-open-source" v-html="renderBody(item.segment)"></span></button>
          <span v-else class="formula-display-source" v-html="renderBody(item.segment)"></span>
        </div>

        <template v-else-if="!props.isAsset(item.segment)">
          <p
            :data-lid="item.segment.lid"
            :class="{
              anchor: item.segment.lid === props.viewportAnchor,
              selected: item.segment.lid === props.selectedLid,
              hl: props.isHighlighted(item.segment.lid),
              'note-placement-candidate': item.segment.lid === placementCandidateLid,
              ['heading-' + item.segment.kind]: item.segment.kind === 'chapter' || item.segment.kind === 'section',
              ...markdownHeadingClass(item.segment),
            }"
            @click="emit('select', item.segment.lid)"
            v-html="renderBody(item.segment)"
          ></p>

        </template>

        <section
          v-else
          :data-lid="item.segment.lid"
          class="asset-block"
          :class="[`asset-${item.segment.kind}`, {
            anchor: item.segment.lid === props.viewportAnchor,
            selected: !isTextAsset(item.segment) && item.segment.lid === props.selectedLid,
            hl: props.isHighlighted(item.segment.lid),
            'note-placement-candidate': item.segment.lid === placementCandidateLid,
            'asset-expanded': item.segment.lid === expandedAssetLid,
          }]"
          @click="!isTextAsset(item.segment) && emit('select', item.segment.lid)"
        >
          <div class="asset-head" data-reader-selection-ignore>
            <span>{{ item.segment.kind }}</span>
            <span class="asset-head-actions">
              <button
                v-if="isTextAsset(item.segment)"
                type="button"
                :aria-pressed="wrappedAssetLids.has(item.segment.lid)"
                @click.stop="toggleAssetWrap(item.segment.lid)"
              >{{ wrappedAssetLids.has(item.segment.lid) ? '不换行' : '换行' }}</button>
              <button type="button" @click.stop="toggleAssetExpanded(item.segment.lid)">
                {{ item.segment.lid === expandedAssetLid ? '收起' : '展开' }}
              </button>
              <button v-if="!isTextAsset(item.segment)" class="asset-jump" title="选中该 LID" @click.stop="emit('select', item.segment.lid)">定位</button>
            </span>
          </div>
          <pre v-if="item.segment.kind === 'code'" class="asset-source asset-code" :class="{ 'soft-wrap': wrappedAssetLids.has(item.segment.lid) }"><code v-html="renderBody(item.segment)"></code></pre>
          <div v-else-if="item.segment.kind === 'table'" class="asset-source asset-table reader-table" :class="{ 'soft-wrap': wrappedAssetLids.has(item.segment.lid) }" v-html="renderBody(item.segment)"></div>
          <figure v-else-if="item.segment.kind === 'image'" class="asset-image-figure">
            <img
              v-if="imageRenderSrc(props.imageAsset(item.segment.lid))"
              class="image-rendered"
              :src="imageRenderSrc(props.imageAsset(item.segment.lid)) || ''"
              :alt="props.imageAsset(item.segment.lid)?.alt || props.imageMeta(item.segment.text)?.alt || '图片'"
              loading="lazy"
              decoding="async"
            />
            <div v-else class="image-preview">
              <span>图片</span>
              <strong>{{ props.imageMeta(item.segment.text)?.alt || '未命名图片' }}</strong>
              <code>{{ props.imageMeta(item.segment.text)?.src || '来源不可用' }}</code>
            </div>
            <p v-if="props.imageAsset(item.segment.lid)?.warning" class="image-warning">
              {{ props.imageAsset(item.segment.lid)?.warning }}
            </p>
          </figure>

        </section>


      </div>
      <p v-if="props.segments.length === 0" class="empty">暂无正文。请确认服务端已加载书并正在监听。</p>
    </article>
    <div class="annotation-markers" data-reader-selection-ignore data-note-placement-ignore>
      <button v-for="group in annotationGroups" :key="group.lid" class="annotation-marker"
        :data-annotation-lid="group.lid" :style="markerStyles[group.lid]"
        :aria-label="`查看此处 ${group.records.length} 条批注`" :aria-expanded="activeAnnotation?.lid === group.lid"
        aria-controls="reader-annotation-preview" @pointerdown.stop @mousedown.prevent
        @click="toggleAnnotation(group.lid, $event)">✎<span>{{ group.records.length }}</span></button>
    </div>
    <Teleport to="body">
      <AnnotationPreview v-if="activeRecord" id="reader-annotation-preview" :style="previewStyle"
        :records="activeRecords" :active="activeRecord" :source-text="annotationSourceText"
        :render-markdown="props.renderMarkdown" @select="selectAnnotation" @close="closeAnnotation(true)"
        @edit="editAnnotation" @delete="deleteAnnotation" @place="placeAnnotation"
        @source="openAnnotationSource" @show-notes="showAnnotationInNotes" />
    </Teleport>

  </main>
</template>
