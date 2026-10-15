<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { FormulaSemantics, ImageAssetManifestEntry, MemoryRecord } from "../api";
import type { Manifest } from "../api";
import AnnotationPreview from "./AnnotationPreview.vue";
import FocusReaderPane from './FocusReaderPane.vue';
import type { FocusSource } from '../reader-focus-source';
import type { SourceCursor, PaginationOptions, PaginationSource } from '../reader-pagination';
import { abortable } from '../reader-focus';
import { isReaderFlowSegment, shouldJoinReaderFlow } from '../reader-flow';
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
  focusSource?: FocusSource;
  readerActive?: boolean;
  contextKey?: string;
  segments: Segment[];
  viewportAnchor: string | null;
  selectedLid: string | null;
  sourceFocus?: { lid: string; quote: string | null; ranges?: Map<string, [number, number]> } | null;
  annotationRevision?: unknown;
  focusReturnLabel?: string;
  focusSurfaceLabel?: string;
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

const renderedBodies = computed(() => new Map(props.segments.map(seg => {
  const html = props.renderSeg(seg);
  return [seg.lid, seg.kind === 'paragraph' || seg.kind === 'chapter' || seg.kind === 'section' ? typographyHtml(html) : html];
})));
function renderBody(seg: Segment) { return renderedBodies.value.get(seg.lid) ?? ''; }

function imageRenderSrc(asset: ImageAssetManifestEntry | null | undefined): string | null {
  if (!asset) return null;
  return asset.url_path ?? (asset.status === "external" ? asset.original_src : null);
}
const emit = defineEmits<{
  (e: 'focus-tool', tool: string): void;
  (e: 'pagination-active', active: boolean): void;
  (e: 'focus-visible', value: { position: ScrollAnchor; ranges: { lid: string; start: number; end: number }[]; sources: PaginationSource[] }): void;
  (e: 'focus-confirmed'): void;
  (e: "open-note", record: MemoryRecord, restore?: boolean): void;
  (e: "select", lid: string): void;
  (e: "prose-selection-change", snapshot: ReaderSelectionSnapshot | null): void;
  (e: "modify-highlight", rec: MemoryRecord): void;
  (e: "delete-highlight", rec: MemoryRecord): void;
  (e: "edit-note", rec: MemoryRecord): void;
  (e: "delete-note", rec: MemoryRecord): void;
  (e: "show-notes", rec: MemoryRecord): void;
  (e: "place-note", rec: MemoryRecord): void;
  (e: "goto", lid: string): void;
  (e: "focus-source-local", source: { lid: string; quote: string | null; ranges?: Map<string, [number, number]> }): void;
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
  return isReaderFlowSegment(seg, isMarkdownHeading(seg));
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
      if (last && !shouldJoinReaderFlow(last, seg)) {
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
const paginated = ref(false);
const focusMounted = ref(false);
const focusReady = ref(false);
const focusPane = ref<InstanceType<typeof FocusReaderPane>>();
const focusAnchor = ref<SourceCursor>({ index: 0, offset: 0 });
async function prepareFocus(anchor?: SourceCursor) {
  if (!props.focusSource) return;
  const position = !anchor && !paginated.value ? captureScrollAnchor(props.segments.map(s => s.lid)) : null;
  const already = focusMounted.value;
  if (!already) {
    focusReady.value = false;
    const formulas = new Map(Array.from(pane.value?.querySelectorAll<HTMLElement>('.formula-open-source') ?? []).map(el => [el.closest<HTMLElement>('[data-lid]')!.dataset.lid, el]));
    props.focusSource.seed(props.segments.map(source => {
      const formula = formulas.get(source.lid);
      if (!formula) return source;
      const copy = formula.cloneNode(true) as HTMLElement;
      copy.querySelectorAll('mark').forEach(mark => mark.replaceWith(...mark.childNodes));
      return { ...source, preparedFormulaHtml: copy.innerHTML };
    }));
  }
  typographyRequest++; typographyAbort?.abort(); cancelTypographyRestore();
  focusAnchor.value = anchor ?? { index: Math.max(0, props.focusSource.leaves.findIndex(n => n.lid === (position?.lid ?? props.viewportAnchor))), offset: position?.textPosition?.start ?? 0 };
  paginated.value = true; focusMounted.value = true;
  await nextTick();
  // The continuous branch is now unmounted. Evaluate its lazy computed value
  // once to release the previous accumulated segment array as well.
  void readerItems.value; void renderedBodies.value;
  if (already) focusPane.value?.open(focusAnchor.value);
  pane.value?.focus({ preventScroll: true });
}
async function leaveFocus(anchor: ScrollAnchor | null) {
  focusPane.value?.retainCurrent();
  reflowPending = true; paginated.value = false; focusReady.value = false; appliedTypography.value = props.typography ?? defaultTypography;
  emit('pagination-active', false); await nextTick();
  await restoreScrollAnchor(anchor); reflowPending = false;
  pane.value?.focus({ preventScroll: true });
}
useReaderSelection(pane, (snapshot) => {
  if (snapshot && paginated.value) {
    const canonical = focusPane.value?.selection(snapshot.range);
    if (!canonical?.ranges.length) return;
    emit('prose-selection-change', { ...snapshot, canonical });
  } else emit('prose-selection-change', snapshot);
});
const focusLids = ref<string[]>([]);
function focusCurrent(cursor: SourceCursor) {
  const ranges = focusPane.value?.ranges() ?? [], position = focusPane.value?.readingPosition();
  focusLids.value = [...new Set(ranges.map(r => r.lid))];
  if (!focusReady.value) {
    focusReady.value = true; emit('pagination-active', true);
    void nextTick(() => { void readerItems.value; void renderedBodies.value; });
  }
  closeAnnotation();
  if (position) {
    lastVisibleAnchor = position;
    emit('focus-visible', { position, ranges, sources: focusLids.value.flatMap(lid => { const s = focusPane.value?.source(lid); return s ? [s] : []; }) });
  }
  emit('current-lid', cursor.lid ?? props.focusSource!.leaves[cursor.index]?.lid ?? '');
}
const focusDecorationRevision = computed(() => [props.annotationRevision, props.visibleNotes, props.sourceFocus]);
const focusDecorations: NonNullable<PaginationOptions['decorations']> = (lid, source) => {
  const marks = props.highlightsOf(lid).map(record => ({ start: record.range?.start ?? 0, end: record.range?.end ?? source.text.length, className: 'hl-mark' }));
  const focus = props.sourceFocus;
  const requested = focus?.ranges?.get(lid);
  if (requested) marks.push({ start: requested[0], end: requested[1], className: 'source-focus-mark' });
  else if (focus?.lid === lid && !focus.ranges) {
    const start = focus.quote ? source.text.indexOf(focus.quote) : 0;
    if (start >= 0) marks.push({ start, end: focus.quote ? start + focus.quote.length : source.text.length, className: 'source-focus-mark' });
  }
  const records = [...notesOf(lid), ...props.highlightCardsOf(lid)];
  return { marks, markers: records.flatMap(record => {
    const ranges = record.selection_context?.ranges.filter(r => r.lid === lid).map(r => r.range) ?? (record.range ? [record.range] : []);
    return ranges.length ? ranges.map(range => ({ id: record.mem_id, ...range })) : [{ id: record.mem_id }];
  }) };
};
function focusFormula(lid: string) {
  const source = focusPane.value?.source(lid);
  if (source) emit('open-formula', { ...source, kind: source.kind as NodeKind, formula: null, imageAsset: props.imageAsset(lid) });
}
const placementCandidateLid = ref<string | null>(null);
const wrappedAssetLids = ref<Set<string>>(new Set());
const expandedAssetLid = ref<string | null>(null);
const validPlacementLids = computed(() => new Set(paginated.value ? focusLids.value : props.segments.map((segment) => segment.lid)));
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
let typographyAbort: AbortController | undefined;
function cancelTypographyRestore() { reflow.cancelRestore(); reflowPending = false; }
function interact() { cancelTypographyRestore(); emit('viewport-interaction'); }

async function applyTypography() {
  if (paginated.value) return;
  if (selecting || composing) { deferredTypography = true; return; }
  deferredTypography = false;
  const request = ++typographyRequest;
  typographyAbort?.abort();
  const controller = new AbortController(); typographyAbort = controller;
  const anchor = captureScrollAnchor(props.segments.map(s => s.lid));
  const token = reflow.beginRestore(props.contextKey ?? 'local');
  const current = () => reflow.isCurrent(token) && token.contextKey === (props.contextKey ?? 'local');
  reflowPending = true;
  expectedScrollTop = pane.value?.scrollTop ?? 0;
  const p = props.typography ?? { ...defaultTypography };
  // Load the actual text's unicode subsets, including headings, from bundled resources.
  const text = props.segments.map(s => s.text).join('');
  if (document.fonts) {
    try { await abortable(Promise.all([400, 600].map(weight => document.fonts.load(`${weight} ${p.fontSizePx}px "${typographyFont(p.font)}"`, text))), controller.signal); }
    catch { /* The fallback remains readable when a local font request fails. */ }
  }
  if (controller.signal.aborted || request !== typographyRequest || token.contextKey !== (props.contextKey ?? 'local')) return;
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


const annotationGroups = computed(() => (paginated.value ? focusLids.value : props.segments.map(s => s.lid)).flatMap(lid => {
  const records = [...notesOf(lid), ...props.highlightCardsOf(lid)];
  return records.length ? [{ lid, records }] : [];
}));
const activeAnnotation = ref<{ lid: string; id: string } | null>(null);
const activeRecords = computed(() => annotationGroups.value.find(g => g.lid === activeAnnotation.value?.lid)?.records ?? []);
const activeRecord = computed(() => activeRecords.value.find(r => r.mem_id === activeAnnotation.value?.id));
const annotationSourceText = computed(() => (paginated.value ? focusPane.value?.source(activeAnnotation.value?.lid ?? '') : props.segments.find(s => s.lid === activeAnnotation.value?.lid))?.text.slice(0, 220) ?? '');
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
async function focusAnnotation(lid: string, id: string, trigger: HTMLElement) {
  if (activeAnnotation.value?.id === id && annotationTrigger === trigger) { closeAnnotation(true); return; }
  closeAnnotation(); annotationTrigger = trigger; activeAnnotation.value = { lid, id };
  updateAnnotationGeometry(); await nextTick();
  document.getElementById('reader-annotation-preview')?.focus({ preventScroll: true });
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
function openAnnotationSource(source: { lid: string; quote: string | null; ranges?: Map<string, [number, number]> }) { closeAnnotation(true); emit('focus-source-local', source); }
function showAnnotationInNotes(record: MemoryRecord) { closeAnnotation(); emit('show-notes', record); }
function updateAnnotationGeometry() {
  const root = pane.value;
  if (!root) return;
  if (paginated.value) {
    if (!activeAnnotation.value || !annotationTrigger?.isConnected) return;
    const box = annotationTrigger.getBoundingClientRect(), rect = root.getBoundingClientRect();
    previewStyle.value = { left: `${Math.max(8, Math.min(box.left, window.innerWidth - 340))}px`,
      top: `${Math.max(8, Math.min(box.bottom + 6, window.innerHeight - 360))}px`, bottom: 'auto',
      width: `${Math.min(320, rect.width - 16)}px`, maxHeight: 'min(520px, calc(100dvh - 96px))' };
    return;
  }
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
watch(() => props.contextKey, () => { lastVisibleAnchor = null; focusMounted.value = false; closeAnnotation(); }, { flush: 'sync' });

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
  if (paginated.value) return;
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
  if (paginated.value) return;
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
  if (paginated.value) return;
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
  if (paginated.value) return;
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
  return Array.from(el.querySelectorAll<HTMLElement>(paginated.value ? '.focus-screen:not([hidden]) [data-lid]' : '[data-lid]')).find((node) => node.dataset.lid === lid) ?? null;
}

function captureScrollAnchor(candidateLids: string[]): ScrollAnchor | null {
  if (paginated.value) return focusPane.value?.readingPosition() ?? lastVisibleAnchor;
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
  if (paginated.value && props.focusSource) {
    const index = props.focusSource.leaves.findIndex(n => n.lid === anchor.lid);
    const shown = focusPane.value?.anchor();
    if (index >= 0 && (shown?.index !== index || shown.offset !== (anchor.textPosition?.start ?? 0))) focusPane.value?.open({ index, offset: anchor.textPosition?.start ?? 0 });
    return;
  }
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
  if (paginated.value && props.focusSource) {
    const index = props.focusSource.leaves.findIndex(n => n.lid === lid);
    if (index < 0) return false;
    focusPane.value?.open({ index, offset: 0 }); return true;
  }
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

async function scrollSourceIntoView(lid: string, range?: { start: number; end: number }): Promise<boolean> {
  if (paginated.value && props.focusSource) {
    const index = props.focusSource.leaves.findIndex(n => n.lid === lid);
    if (index < 0) return false;
    focusPane.value?.open({ index, lid, offset: range?.start ?? props.sourceFocus?.ranges?.get(lid)?.[0] ?? 0 }); return true;
  }
  if (!await scrollLidIntoView(lid)) return false;
  const mark = lidElement(lid)?.querySelector<HTMLElement>('.source-focus-mark');
  if (mark && pane.value) pane.value.scrollTop += mark.getBoundingClientRect().top - pane.value.getBoundingClientRect().top;
  return true;
}

defineExpose({ captureScrollAnchor, restoreScrollAnchor, scrollLidIntoView, scrollSourceIntoView, cancelTypographyRestore,
  prepareFocus, leaveFocus, suspendFocus: () => focusPane.value?.retainCurrent(), focusMove: (direction: number) => focusPane.value?.move(direction),
  focusWork: () => focusPane.value?.work(),
  focusSnapshot: () => focusPane.value?.snapshot(), focusPause: (value: boolean) => focusPane.value?.setPaused(value) });

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
  if (focusReady.value) emit('pagination-active', false);
  if (arrivalTimer) clearTimeout(arrivalTimer);
  arrivalNode?.removeAttribute('data-reader-arrival');
  annotationResize?.disconnect();
  window.removeEventListener('resize', updateAnnotationGeometry);
  closeAnnotation();
  typographyRequest++;
  typographyAbort?.abort();
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
    :class="{ 'reader-paginated': paginated }"
    tabindex="0"
    @scroll.passive="onScroll"
    @wheel="onWheel"
    @pointerdown="selecting = true; interact()"
    @pointermove="onPointerMove"
    @pointerleave="onPointerLeave"
    @pointerup="onPointerUp"
    @keydown="onKeydown"
  >
    <Teleport to="body" :disabled="paginated">
    <FocusReaderPane v-if="focusMounted && props.focusSource" v-show="paginated" ref="focusPane" :source="props.focusSource" :anchor="focusAnchor"
      :class="{ 'focus-reader-pending': !focusReady }"
      :typography="props.typography ?? defaultTypography" :active="paginated && props.readerActive !== false"
      :decorations="focusDecorations" :decoration-revision="focusDecorationRevision" :placement-active="props.notePlacementActive"
      :return-label="props.focusReturnLabel" :surface-label="props.focusSurfaceLabel"
      @tool="emit('focus-tool', $event)"
      @current="focusCurrent" @confirmed="emit('focus-confirmed')" @annotation="focusAnnotation"
      @select="emit('select', $event)" @formula="focusFormula" />
    </Teleport>
    <article v-if="!focusReady" class="prose" :style="bodyStyle" :lang="appliedTypography.languageMode === 'latin' ? 'en' : 'zh-Hans'">
      <div v-for="item in readerItems" :key="itemKey(item)" class="seg">
        <template v-if="item.type === 'flow'">
          <p class="flow-paragraph">
            <template v-for="seg in item.segments" :key="seg.lid">
              <button
                v-if="seg.kind === 'formula'"
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
            class="formula-open formula-display-open"
            title="查看公式语义剖面"
            @click.stop="emit('open-formula', item.segment)"
          ><span class="formula-open-source" v-html="renderBody(item.segment)"></span></button>
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
    <div v-if="!paginated" class="annotation-markers" data-reader-selection-ignore data-note-placement-ignore>
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
        @open-note="(record, restore) => { closeAnnotation(); emit('open-note', record, restore); }" @source="openAnnotationSource" @show-notes="showAnnotationInNotes" />
    </Teleport>

  </main>
</template>
