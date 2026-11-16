<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { ArrowDown, ArrowUp, List, LocateFixed, LoaderCircle, X } from '@lucide/vue';
import { api as sharedApi, type AssetManifest, type BoardSourceReadResponse, type SourcePopupView, type SourceWindowLoader, type SourceWindowRequest } from '../api';
import { bindSceneApi } from '../network-context';
import { t as uiText } from '../i18n';
import { renderSourceMarkdown } from '../md';
import { isReaderFlowSegment, shouldJoinReaderFlow } from '../reader-flow';
import BoardSourceSegment from './BoardSourceSegment.vue';
import SourceExcerpt from './SourceExcerpt.vue';

const props = defineProps<{ readWindow: SourceWindowLoader; source: SourcePopupView }>();
const emit = defineEmits<{ (event: 'position', lid: string | undefined): void }>();
const api = bindSceneApi(sharedApi);
const scroller = ref<HTMLElement>();
const outlineScroller = ref<HTMLElement>();
const segments = ref<BoardSourceReadResponse['segments']>([]);
const outline = ref<NonNullable<BoardSourceReadResponse['outline']>>([]);
const previous = ref<string | null>(null), next = ref<string | null>(null);
const currentLid = ref(''), sourceLid = ref('');
const initialLoading = ref(false), pageLoading = ref<'before' | 'after' | null>(null);
const error = ref(''), failedRequest = ref<SourceWindowRequest | null>(null);
const outlineOpen = ref(typeof window !== 'undefined' && window.innerWidth > 850);
const assets = ref<AssetManifest | null>(null), loadingImages = ref(false);
let sequence = 0, assetSequence = 0, disposed = false;
let publication: BoardSourceReadResponse['published_book_ref'];
let assetsRequested = false, adjustingScroll = false, lastScrollTop = 0, scrollFrame = 0;
let touchY: number | null = null;
const maxSegments = 96;

// The source store splits inline formulas into separate LIDs. Rejoin their
// sentence as the main Reader does, keeping each LID for highlights and paging.
type SourceSegment = BoardSourceReadResponse['segments'][number];
type ReadingItem = { type: 'flow'; parts: SourceSegment[] } | { type: 'block'; segment: SourceSegment };
const readingItems = computed(() => {
  const items: ReadingItem[] = [];
  let flow: SourceSegment[] = [];
  const flush = () => {
    if (flow.length === 1 && flow[0].kind === 'paragraph') items.push({ type: 'block', segment: flow[0] });
    else if (flow.length) items.push({ type: 'flow', parts: flow });
    flow = [];
  };
  for (const segment of segments.value) {
    if (isReaderFlowSegment(segment)) {
      if (flow.length && !shouldJoinReaderFlow(flow[flow.length - 1], segment)) flush();
      flow.push(segment);
    } else {
      flush();
      items.push({ type: 'block', segment });
    }
  }
  flush();
  return items;
});

// Outline lids are canonical tree paths. A heading remains current while reading
// its descendants, including chapters outside the source's original excerpt.
const currentHeading = computed(() => {
  const lid = currentLid.value;
  let heading = outline.value.find(entry => entry.lid === lid);
  if (!heading) heading = outline.value.filter(entry => lid.startsWith(`${entry.lid}.`))
    .sort((a, b) => b.lid.length - a.lid.length)[0];
  if (!heading) {
    const index = segments.value.findIndex(segment => segment.lid === lid);
    const preceding = new Set(segments.value.slice(0, index + 1).map(segment => segment.lid));
    heading = outline.value.filter(entry => preceding.has(entry.lid)).at(-1);
  }
  return heading;
});
const headingLabel = computed(() => currentHeading.value?.title || props.source.heading_path?.at(-1) || props.source.label);

// Keep the active chapter visible in long books without scrolling the dialog or
// stealing focus from the text. Reopening the drawer also restores its location.
function revealCurrentHeading() {
  const list = outlineScroller.value;
  const active = list?.querySelector<HTMLElement>('[aria-current="location"]');
  if (!list || !active) return;
  const bounds = list.getBoundingClientRect();
  const item = active.getBoundingClientRect();
  const header = list.querySelector<HTMLElement>('.source-reader-outline-heading');
  const top = header?.getBoundingClientRect().bottom ?? bounds.top;
  if (item.top < top + 8 || item.bottom > bounds.bottom - 8) {
    list.scrollTop += item.top - top - (bounds.bottom - top - item.height) / 2;
  }
}
watch([() => currentHeading.value?.lid, outlineOpen], revealCurrentHeading, { flush: 'post' });

function readElements() {
  return [...(scroller.value?.querySelectorAll<HTMLElement>('[data-source-lid]') ?? [])];
}
function visibleAnchor() {
  const viewport = scroller.value;
  if (!viewport) return null;
  const top = viewport.getBoundingClientRect().top;
  const node = readElements().find(element => {
    const bounds = element.getBoundingClientRect();
    return bounds.height > 0 && bounds.bottom > top + 56;
  });
  return node ? { lid: node.dataset.sourceLid!, offset: node.getBoundingClientRect().top - top } : null;
}
function updatePosition() {
  const anchor = visibleAnchor();
  if (anchor && anchor.lid !== currentLid.value) {
    currentLid.value = anchor.lid;
    emit('position', anchor.lid);
  }
}
async function restoreAfterRender(anchor: ReturnType<typeof visibleAnchor>, focus?: string, token = sequence) {
  adjustingScroll = true;
  await nextTick();
  if (disposed || token !== sequence) { adjustingScroll = false; return; }
  const viewport = scroller.value;
  const node = readElements().find(element => element.dataset.sourceLid === (focus || anchor?.lid));
  if (viewport && node) {
    const target = focus ? node.querySelector<HTMLElement>('mark') ?? node : node;
    viewport.scrollTop += target.getBoundingClientRect().top - viewport.getBoundingClientRect().top - (focus ? 32 : anchor?.offset ?? 0);
  }
  lastScrollTop = viewport?.scrollTop ?? 0;
  updatePosition();
  if (focus) {
    await nextTick();
    if (disposed || token !== sequence) return;
    revealCurrentHeading();
  }
  // Ignore the scroll event caused by restoring the retained paragraph. Only a
  // later user scroll can request another window, so a short book cannot drain.
  requestAnimationFrame(() => { if (token === sequence) adjustingScroll = false; });
}
async function loadAssets() {
  if (assetsRequested || !segments.value.some(segment => renderSourceMarkdown(segment.text).includes('<img '))) return;
  assetsRequested = true;
  loadingImages.value = true;
  const token = ++assetSequence;
  try {
    const result = await api.assetManifest(publication ?? undefined);
    if (!disposed && token === assetSequence) assets.value = result;
  } catch { /* Text stays readable when an illustration is unavailable. */ }
  finally { if (!disposed && token === assetSequence) loadingImages.value = false; }
}
async function read(request: SourceWindowRequest) {
  const direction = request.direction ?? 'around';
  const token = ++sequence;
  if (direction === 'around') initialLoading.value = true;
  else pageLoading.value = direction;
  error.value = '';
  failedRequest.value = null;
  try {
    const result = await props.readWindow(request);
    if (disposed || token !== sequence) return;
    const anchor = direction === 'around' ? null : visibleAnchor();
    publication = result.published_book_ref;
    if (result.outline) outline.value = result.outline;
    sourceLid.value = result.source_lid;
    if (direction === 'around') {
      segments.value = result.segments;
      previous.value = result.previous_lid;
      next.value = result.next_lid;
      currentLid.value = result.focus_lid;
      emit('position', result.focus_lid);
    } else {
      const retained = new Set(segments.value.map(segment => segment.lid));
      const added = result.segments.filter(segment => !retained.has(segment.lid));
      if (direction === 'before') {
        segments.value = [...added, ...segments.value];
        previous.value = result.previous_lid;
        if (segments.value.length > maxSegments) {
          segments.value = segments.value.slice(0, maxSegments);
          next.value = segments.value.at(-1)!.lid;
        }
      } else {
        segments.value = [...segments.value, ...added];
        next.value = result.next_lid;
        if (segments.value.length > maxSegments) {
          segments.value = segments.value.slice(-maxSegments);
          previous.value = segments.value[0].lid;
        }
      }
    }
    initialLoading.value = false;
    pageLoading.value = null;
    void loadAssets();
    await restoreAfterRender(anchor, direction === 'around' ? result.focus_lid : undefined);
  } catch (failure) {
    if (disposed || token !== sequence) return;
    error.value = failure instanceof Error ? failure.message : uiText('原文暂时无法读取，请重试。');
    failedRequest.value = request;
  } finally {
    if (!disposed && token === sequence) {
      initialLoading.value = false;
      pageLoading.value = null;
    }
  }
}
function around(lid?: string) {
  return read({ anchor_lid: lid, direction: 'around', include_outline: outline.value.length === 0 });
}
function loadEdge(direction: 'before' | 'after') {
  if (initialLoading.value || pageLoading.value || error.value) return;
  if (!(direction === 'before' ? previous.value : next.value)) return;
  const anchor = direction === 'before' ? segments.value[0]?.lid : segments.value.at(-1)?.lid;
  if (anchor) void read({ anchor_lid: anchor, direction, include_outline: false });
}
function edgeFromMovement(delta: number) {
  const viewport = scroller.value;
  if (!viewport || adjustingScroll || !delta) return;
  if (delta < 0 && viewport.scrollTop < 180) loadEdge('before');
  else if (delta > 0 && viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight < 240) loadEdge('after');
}
function onScroll() {
  if (scrollFrame) return;
  scrollFrame = requestAnimationFrame(() => {
    scrollFrame = 0;
    const top = scroller.value?.scrollTop ?? 0;
    updatePosition();
    edgeFromMovement(top - lastScrollTop);
    lastScrollTop = top;
  });
}
function onTouchMove(event: TouchEvent) {
  const y = event.touches[0]?.clientY;
  if (y !== undefined && touchY !== null) edgeFromMovement(touchY - y);
  touchY = y ?? null;
}
function onReaderKey(event: KeyboardEvent) {
  if (event.key === 'ArrowUp' || event.key === 'PageUp' || event.key === 'Home') edgeFromMovement(-1);
  else if (event.key === 'ArrowDown' || event.key === 'PageDown' || event.key === 'End' || event.key === ' ') edgeFromMovement(1);
}
function openChapter(lid: string) {
  if (window.innerWidth <= 850) outlineOpen.value = false;
  void around(lid);
}
function backToSource() {
  const retained = segments.value.find(segment => segment.lid === sourceLid.value);
  if (retained) {
    // A page already in flight must not pull the view back to the paragraph
    // captured before the learner chose to return to the citation.
    ++sequence;
    pageLoading.value = null;
    error.value = '';
    failedRequest.value = null;
    void restoreAfterRender(null, retained.lid);
  }
  else void around();
}
watch(() => props.readWindow, () => {
  ++sequence; ++assetSequence;
  segments.value = []; outline.value = []; previous.value = null; next.value = null;
  currentLid.value = ''; sourceLid.value = ''; assets.value = null; publication = undefined;
  assetsRequested = false; loadingImages.value = false; pageLoading.value = null;
  emit('position', undefined);
  void around();
}, { immediate: true });
onBeforeUnmount(() => { disposed = true; ++sequence; ++assetSequence; if (scrollFrame) cancelAnimationFrame(scrollFrame); });
</script>

<template>
  <div class="board-source-reader">
    <div class="source-reader-toolbar">
      <button type="button" class="source-reader-outline-toggle" :aria-label="uiText('目录')" :aria-expanded="outlineOpen" @click="outlineOpen = !outlineOpen"><List :size="17" aria-hidden="true" /><span>{{ uiText('目录') }}</span></button>
      <p class="source-reader-location" :title="headingLabel">{{ headingLabel }}</p>
      <button type="button" class="source-reader-return" :disabled="initialLoading" @click="backToSource"><LocateFixed :size="16" aria-hidden="true" /><span>{{ uiText('回到引用处') }}</span></button>
    </div>
    <div class="source-reader-body" :class="{ 'outline-open': outlineOpen }">
      <button v-if="outlineOpen" type="button" class="source-reader-outline-scrim" :aria-label="uiText('收起目录')" tabindex="-1" @click="outlineOpen = false"></button>
      <nav v-if="outlineOpen" ref="outlineScroller" class="source-reader-outline" :aria-label="uiText('原文目录')">
        <div class="source-reader-outline-heading"><span>{{ uiText('目录') }}</span><button type="button" :aria-label="uiText('收起目录')" @click="outlineOpen = false"><X :size="16" aria-hidden="true" /></button></div>
        <p v-if="initialLoading && !outline.length" class="source-reader-outline-empty">{{ uiText('正在加载目录…') }}</p>
        <p v-else-if="!outline.length" class="source-reader-outline-empty">{{ uiText('这份材料没有章节目录') }}</p>
        <button v-for="entry in outline" :key="entry.lid" type="button" :data-outline-lid="entry.lid" :aria-current="entry.lid === currentHeading?.lid ? 'location' : undefined" :style="{ paddingLeft: `${12 + Math.min(entry.depth, 5) * 12}px` }" :title="entry.title" @click="openChapter(entry.lid)">{{ entry.title }}</button>
      </nav>
      <div ref="scroller" class="source-reader-scroll" data-source-reader-scroll tabindex="0" :aria-label="uiText('原文阅读区')" :aria-busy="initialLoading || !!pageLoading" @scroll.passive="onScroll" @wheel.passive="edgeFromMovement($event.deltaY)" @touchstart.passive="touchY = $event.touches[0]?.clientY ?? null" @touchmove.passive="onTouchMove" @keydown="onReaderKey">
        <div class="source-reader-page">
          <div v-if="initialLoading" class="source-reader-loading" role="status"><LoaderCircle :size="18" aria-hidden="true" />{{ uiText('正在加载原文…') }}</div>
          <div v-if="error" class="source-reader-error" role="alert"><p>{{ error }}</p><button type="button" @click="failedRequest && read(failedRequest)">{{ uiText('重试') }}</button></div>
          <template v-if="segments.length">
            <div class="source-reader-edge source-reader-edge-before">
              <button v-if="previous" type="button" :disabled="initialLoading || !!pageLoading" @click="loadEdge('before')"><LoaderCircle v-if="pageLoading === 'before'" :size="14" aria-hidden="true" /><ArrowUp v-else :size="14" aria-hidden="true" />{{ pageLoading === 'before' ? uiText('正在加载前文…') : uiText('阅读前文') }}</button>
              <span v-else>{{ uiText('已到材料开头') }}</span>
            </div>
            <template v-for="item in readingItems" :key="item.type === 'flow' ? item.parts[0].lid : item.segment.lid">
              <p v-if="item.type === 'flow'" class="source-reader-flow" :class="{ 'has-citation': item.parts.some(part => part.highlights.length > 0) }"><BoardSourceSegment v-for="part in item.parts" :key="part.lid" :segment="part" :assets="assets" :loading-images="loadingImages" inline /></p>
              <BoardSourceSegment v-else :segment="item.segment" :assets="assets" :loading-images="loadingImages" />
            </template>
            <div class="source-reader-edge source-reader-edge-after">
              <button v-if="next" type="button" :disabled="initialLoading || !!pageLoading" @click="loadEdge('after')"><LoaderCircle v-if="pageLoading === 'after'" :size="14" aria-hidden="true" /><ArrowDown v-else :size="14" aria-hidden="true" />{{ pageLoading === 'after' ? uiText('正在加载后文…') : uiText('继续阅读') }}</button>
              <span v-else>{{ uiText('已到材料末尾') }}</span>
            </div>
          </template>
          <SourceExcerpt v-else :source="source" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.board-source-reader{display:flex;flex:1;flex-direction:column;min-height:0;min-width:0;height:100%}
.source-reader-toolbar{display:flex;align-items:center;gap:14px;flex-shrink:0;min-height:42px;padding:4px 16px;border-bottom:1px solid var(--hairline);background:var(--reader-card)}
.source-reader-toolbar button{display:inline-flex;align-items:center;justify-content:center;gap:7px;flex-shrink:0;padding:7px 9px;border:0;border-radius:6px;background:transparent;color:var(--steel);font:500 12px var(--sans);cursor:pointer}
.source-reader-toolbar button:hover,.source-reader-toolbar button[aria-expanded=true]{background:var(--surface-soft);color:var(--brand-accent)}.source-reader-toolbar button:disabled{opacity:.5;cursor:wait}
.source-reader-location{flex:1;min-width:0;margin:0;color:var(--stone);font-size:12px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.source-reader-body{display:flex;position:relative;flex:1;min-height:0;min-width:0;overflow:hidden}
.source-reader-outline{flex:0 0 232px;min-width:0;overflow-y:auto;overscroll-behavior:contain;padding:0 8px 20px;background:color-mix(in srgb,var(--surface-soft) 46%,var(--reader-card));border-right:1px solid var(--hairline);scrollbar-width:thin;scrollbar-color:var(--hairline) transparent}
.source-reader-outline-heading{display:flex;position:sticky;top:0;z-index:1;align-items:center;justify-content:space-between;padding:8px 10px;background:var(--reader-card);color:var(--stone);font-size:11px;letter-spacing:.08em}.source-reader-outline-heading button{display:grid;place-items:center;width:26px;height:26px;padding:0;border:0;border-radius:5px;background:transparent;color:var(--steel);cursor:pointer}
.source-reader-outline>button{display:block;width:100%;padding:7px 12px;border:0;border-left:2px solid transparent;border-radius:5px;background:transparent;color:var(--slate);font:13px/1.55 var(--sans);text-align:left;overflow-wrap:anywhere;cursor:pointer}
.source-reader-outline>button:hover{background:var(--surface-soft)}.source-reader-outline>button[aria-current=location]{border-left-color:var(--brand-accent);background:color-mix(in srgb,var(--brand-accent) 8%,var(--reader-card));color:var(--brand-accent);font-weight:550}
.source-reader-outline-empty{padding:10px 14px;color:var(--stone);font-size:12px;line-height:1.8}
.source-reader-outline-scrim{display:none;padding:0;border-radius:0}
.source-reader-scroll{flex:1;min-width:0;min-height:0;overflow:auto;overscroll-behavior:contain;scrollbar-gutter:stable;scrollbar-width:thin;scrollbar-color:var(--hairline) transparent;outline:none}
.source-reader-page{max-width:1240px;margin:0 auto;padding:16px clamp(24px,3vw,40px) 28px}
.source-reader-flow{margin:0 0 1.1em;color:var(--ink);font-family:var(--reader-font,var(--serif,serif));font-size:18px;line-height:1.95;overflow-wrap:anywhere}
.source-reader-flow.has-citation{padding-left:20px;margin-left:-22px;border-left:2px solid color-mix(in srgb,var(--brand-accent) 65%,transparent)}
@media(max-width:700px){.source-reader-flow{font-size:17px;line-height:1.9}.source-reader-flow.has-citation{padding-left:12px;margin-left:-14px}}
.source-reader-loading{display:flex;align-items:center;justify-content:center;gap:10px;min-height:60px;color:var(--stone);font:13px var(--sans)}
.source-reader-loading svg,.source-reader-edge button svg.lucide-loader-circle{animation:source-reader-spin 1s linear infinite}
.source-reader-error{display:flex;align-items:center;gap:15px;padding:12px 15px;margin-bottom:20px;border:1px solid color-mix(in srgb,var(--brand-error) 25%,var(--hairline));border-radius:8px;background:var(--surface-soft);color:var(--brand-error);font:12px/1.8 var(--sans)}.source-reader-error p{flex:1;margin:0}.source-reader-error button{padding:5px 10px;flex-shrink:0;border:1px solid currentColor;border-radius:5px;background:transparent;color:inherit;cursor:pointer}
.source-reader-edge{display:flex;align-items:center;justify-content:center;min-height:50px;margin:0 0 20px;color:var(--stone);font:11px var(--sans)}.source-reader-edge-after{margin:30px 0 0;border-top:1px solid var(--hairline);padding-top:22px}.source-reader-edge button{display:inline-flex;align-items:center;gap:7px;padding:8px 16px;border:1px solid var(--hairline);border-radius:20px;background:var(--reader-card);color:var(--steel);font:12px var(--sans);cursor:pointer}.source-reader-edge button:hover{border-color:var(--brand-accent);color:var(--brand-accent)}.source-reader-edge button:disabled{opacity:.5;cursor:wait}
.board-source-reader button:focus-visible,.source-reader-scroll:focus-visible{outline:2px solid var(--brand-accent);outline-offset:-2px}
@keyframes source-reader-spin{to{transform:rotate(360deg)}}
@media(max-width:1100px){.source-reader-outline{flex-basis:218px}.source-reader-page{padding-left:28px;padding-right:28px}}
@media(max-width:850px){.source-reader-outline{position:absolute;z-index:2;inset:0 auto 0 0;width:min(290px,82%);box-shadow:10px 0 30px #25232010;background:var(--reader-card)}.source-reader-outline-scrim{display:block;position:absolute;z-index:1;inset:0;border:0;background:#25232035;cursor:pointer}.source-reader-toolbar{padding:4px 12px;gap:9px}}
@media(max-width:600px){.source-reader-toolbar{min-height:44px;padding:4px 10px;gap:6px}.source-reader-toolbar button{min-height:34px;padding:7px 6px;font-size:11px}.source-reader-outline-toggle span{display:none}.source-reader-location{font-size:11px}.source-reader-page{padding:12px 20px 24px}.source-reader-return span{max-width:110px;white-space:normal}.source-reader-edge{margin-bottom:16px}}
@media(prefers-reduced-motion:reduce){.source-reader-loading svg,.source-reader-edge button svg.lucide-loader-circle{animation:none}}
</style>
