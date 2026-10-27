<script setup lang="ts">
import { t as uiText } from '../i18n';
import { getLocale } from '../i18n';

import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { videoGet, videoUrl, type PreparingVideo } from '../video-api';
import { attachVideoPlayback } from '../video-playback';
import type { TranscriptMap } from '../../../core/src/generated/TranscriptMap';
import type { VideoQuestionContext } from '../../../core/src/generated/VideoQuestionContext';
import { videoContentRect, videoRegion, type DisplayRect, type Point } from '../video-region';
import { videoTime } from '../video-question';
import { captureQuestionImage } from '../question-image';
import { ArrowLeft, Captions, Check, ChevronDown, Clock3, Crosshair, Focus, Languages, Play, Search, Scissors, X } from '@lucide/vue';
const props = defineProps<{ initial: PreparingVideo; owner: string; published?: import('../video-api').PublishedVideo; playback?: import('../video-api').VideoPlayback; detailsLoading?: boolean; detailsError?: string; embedded?: boolean; mediaUrl?: string; question?: VideoQuestionContext | null; selectionDisabled?: boolean; adoptionError?: string }>();
const emit = defineEmits<{ (event: 'close'): void; (event: 'frame-ready', ready: boolean): void; (event: 'ready', publication: NonNullable<PreparingVideo['publication']>): void; (event: 'select-region', context: VideoQuestionContext): void; (event: 'clear-region'): void; (event: 'select-range', context: VideoQuestionContext): void; (event: 'clear-range'): void }>();
const state = ref(props.initial), player = ref<HTMLVideoElement>(), error = ref(''), playbackFailure = ref('');
const buffering = ref(true);
const mediaSource = computed(() => props.mediaUrl || videoUrl(state.value.video, 'media'));
const playlist = computed(() => props.playback?.hls ? mediaSource.value.replace(/\/media$/, '/playback/index.m3u8') : null);
let releasePlayback: (() => void) | undefined;
function savedPosition(): number {
  try { const saved = JSON.parse(localStorage.getItem(key) || 'null'); return Number.isFinite(saved?.time) && saved.time >= 0 ? Math.min(saved.time, Math.max(0, duration.value / 1000 - 0.1)) : 0; } catch { return 0; }
}
function connectPlayback() {
  releasePlayback?.(); releasePlayback = undefined;
  if (!player.value) return;
  buffering.value = true; playbackFailure.value = '';
  releasePlayback = attachVideoPlayback(player.value, mediaSource.value, playlist.value, savedPosition(), message => { playbackFailure.value = message; buffering.value = false; });
}
function retryPlayback() { remember(); connectPlayback(); }
const transcript = ref<TranscriptMap | undefined>(props.published?.map);
const surface = ref<HTMLElement>(), transcriptSurface = ref<HTMLElement>();
const translations = computed(() => new Map(props.published?.translations?.map(t => [t.segment_id, t.zh_text]) ?? []));
const translationTasks = computed(() => new Map(props.published?.translation_progress?.tasks.flatMap(t => t.segment_ids.map(id => [id, t] as const)) ?? []));
function translationStatus(id: string) {
  const task = translationTasks.value.get(id);
  if (task?.status === 'failed') return uiText('翻译失败，等待构建端重试');
  if (task?.status === 'running') return task.attempts > 1 ? uiText('正在重试翻译') : uiText('正在翻译');
  if (task?.status === 'accepted') return uiText('译文已生成，等待发布');
  return uiText('待翻译');
}
const untranslated = computed(() => transcript.value?.segments.filter(s => !translations.value.has(s.segment_id)) ?? []);
const failedTranslations = computed(() => untranslated.value.filter(s => translationTasks.value.get(s.segment_id)?.status === 'failed'));
const position = ref(0);
const currentSegment = computed(() => transcript.value?.segments.find(s => s.location.start_ms <= position.value && s.location.end_ms > position.value));
const transcriptQuery = ref('');
const transcriptLanguage = ref<'bilingual' | 'en' | 'zh'>(getLocale() === 'en' ? 'en' : 'bilingual');
const followingTranscript = ref(true);
const filteredSegments = computed(() => {
  const query = transcriptQuery.value.trim().toLocaleLowerCase();
  return (transcript.value?.segments ?? []).filter(segment => !query || `${segment.text}\n${translations.value.get(segment.segment_id) ?? ''}`.toLocaleLowerCase().includes(query));
});
const duration = computed(() => props.published?.source.duration_ms ?? state.value.duration_ms ?? 0);
const playbackProgress = computed(() => duration.value > 0 ? Math.min(100, position.value / duration.value * 100) : 0);
async function scrollToCurrent() {
  await nextTick();
  const pane = transcriptSurface.value;
  const active = pane?.querySelector<HTMLElement>('[aria-current="true"]');
  if (!pane || !active || !pane.clientHeight) return;
  const row = active.getBoundingClientRect(), bounds = pane.getBoundingClientRect();
  if (row.top < bounds.top + 12 || row.bottom > bounds.bottom - 12) pane.scrollTop += row.top - bounds.top - Math.max(12, (pane.clientHeight - row.height) / 2);
}
function followCurrent() { transcriptQuery.value = ''; followingTranscript.value = true; void scrollToCurrent(); }
function browseTranscript() { followingTranscript.value = false; }
watch(() => currentSegment.value?.segment_id, () => { if (followingTranscript.value && !transcriptQuery.value.trim()) void scrollToCurrent(); });
watch(transcriptLanguage, () => { if (followingTranscript.value && !transcriptQuery.value.trim()) void scrollToCurrent(); });
watch(() => props.published, async published => {
  const anchors = [transcriptSurface.value, surface.value].flatMap(pane => {
    if (!pane) return [];
    const top = pane.getBoundingClientRect().top;
    const anchor = Array.from(pane.querySelectorAll<HTMLElement>('[data-segment]')).find(p => p.getBoundingClientRect().bottom > top);
    return anchor ? [{ pane, anchor, offset: anchor.getBoundingClientRect().top - top }] : [];
  });
  if (published) transcript.value = published.map;
  await nextTick();
  for (const { pane, anchor, offset } of anchors) pane.scrollTop += anchor.getBoundingClientRect().top - pane.getBoundingClientRect().top - offset;
});
const frameTime = ref<number | null>(null), paused = ref(true);
let frameCallback: number | undefined;
const selecting = ref(false), selectionError = ref(''), dragRegion = ref<VideoQuestionContext['region']>(null);
const contentBox = ref<DisplayRect>({ left: 0, top: 0, width: 0, height: 0 });
let selectionContext: VideoQuestionContext | null = null;
let drag: { id: number; start: Point; box: DisplayRect } | null = null;
let resizeObserver: ResizeObserver | undefined;
const rangeEditing = ref(false), rangeStart = ref<number | string>(0), rangeEnd = ref<number | string>(0), rangeError = ref('');
let rangeContext: VideoQuestionContext | null = null;
function cancelRangeEdit() { rangeEditing.value = false; rangeContext = null; rangeError.value = ''; }
function beginRange() {
  try {
    cancelSelection(); rangeError.value = '';
    rangeContext = props.question ? JSON.parse(JSON.stringify(props.question)) : captureQuestion(crypto.randomUUID());
    const start = props.question?.range?.start_ms ?? Math.floor((player.value?.currentTime ?? 0) * 1000);
    rangeStart.value = start / 1000;
    rangeEnd.value = (props.question?.range?.end_ms ?? Math.min(start + 10000, props.published!.source.duration_ms)) / 1000;
    rangeEditing.value = true;
  } catch (failure) { rangeError.value = (failure as Error).message; }
}
function markRange(edge: 'start' | 'end') {
  if (!player.value) return;
  const seconds = Math.floor(player.value.currentTime * 1000) / 1000;
  if (edge === 'start') rangeStart.value = seconds;
  else rangeEnd.value = seconds;
}
function applyRange() {
  const start = Number(rangeStart.value) * 1000, end = Number(rangeEnd.value) * 1000;
  if (rangeStart.value === '' || rangeEnd.value === '' || !Number.isFinite(start) || !Number.isFinite(end)
    || start < 0 || start >= end || end > props.published!.source.duration_ms) {
    rangeError.value = uiText('请填写视频内的起止秒数，终点须晚于起点'); return;
  }
  const context = props.question ?? rangeContext;
  if (!context || props.selectionDisabled) return;
  emit('select-range', { ...context, range: { video: { ...context.material.video }, start_ms: start, end_ms: end } });
  cancelRangeEdit();
}
const sameFrame = computed(() => !props.question || props.question.frame.actual_pts_ms === frameTime.value);
const visibleRegion = computed(() => dragRegion.value ?? (paused.value && sameFrame.value ? props.question?.region : null));
const contentStyle = computed(() => ({ left: `${contentBox.value.left}px`, top: `${contentBox.value.top}px`, width: `${contentBox.value.width}px`, height: `${contentBox.value.height}px` }));
const regionStyle = computed(() => {
  const [x, y, width, height] = visibleRegion.value?.rect_normalized ?? [0, 0, 0, 0];
  return { left: `${x * 100}%`, top: `${y * 100}%`, width: `${width * 100}%`, height: `${height * 100}%` };
});
function measure() {
  const video = player.value;
  if (!video?.videoWidth || !video.videoHeight) return;
  const box = video.getBoundingClientRect();
  contentBox.value = videoContentRect({ left: 0, top: 0, width: box.width, height: box.height }, video.videoWidth, video.videoHeight);
}
function cancelSelection() { selecting.value = false; drag = null; dragRegion.value = null; selectionContext = null; }
function beginSelection() {
  try {
    cancelRangeEdit();
    selectionError.value = '';
    if (!sameFrame.value) throw new Error(uiText('请回到问题绑定画面再框选'));
    selectionContext = captureQuestion(props.question?.draft_id ?? crypto.randomUUID());
    if (props.question) selectionContext = JSON.parse(JSON.stringify(props.question));
    paused.value = true; measure(); selecting.value = true;
  } catch (failure) { selectionError.value = (failure as Error).message; }
}
function point(event: PointerEvent): Point { return { x: event.clientX, y: event.clientY }; }
function startSelection(event: PointerEvent) {
  if (event.button !== 0 || !selectionContext || drag) return;
  const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
  drag = { id: event.pointerId, start: point(event), box };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  event.preventDefault();
}
function moveSelection(event: PointerEvent) {
  if (!drag || event.pointerId !== drag.id || !selectionContext) return;
  dragRegion.value = videoRegion(selectionContext.frame, drag.box, drag.start, point(event));
}
function finishSelection(event: PointerEvent) {
  if (!drag || event.pointerId !== drag.id || !selectionContext) return;
  moveSelection(event);
  if (dragRegion.value) {
    try {
      if (!player.value || frameTime.value !== selectionContext.frame.actual_pts_ms) throw new Error(uiText('画面已改变，请暂停后重新框选'));
      const image = captureQuestionImage(player.value, dragRegion.value);
      emit('select-region', { ...selectionContext, region: dragRegion.value, image });
    } catch (failure) { selectionError.value = uiText('截图失败：{0}', [(failure as Error).message]); }
  }
  cancelSelection();
}
function playbackStarted() { paused.value = false; cancelSelection(); }
function seeking() { frameTime.value = null; cancelSelection(); emit('frame-ready', false); }
function escapeSelection(event: KeyboardEvent) { if (event.key === 'Escape') { cancelSelection(); cancelRangeEdit(); } }
watch(() => props.question?.draft_id, () => { cancelSelection(); cancelRangeEdit(); });
watch(() => props.selectionDisabled, disabled => { if (disabled) { cancelSelection(); cancelRangeEdit(); } });
function followFrame() {
  const video = player.value;
  if (!video?.requestVideoFrameCallback) return;
  frameCallback = video.requestVideoFrameCallback((_now, metadata) => {
    const context = props.playback ?? props.published;
    if (context) {
      const [numerator, denominator] = context.time_base;
      frameTime.value = Math.round(metadata.mediaTime * denominator / numerator) * numerator / denominator * 1000;
    }
    emit('frame-ready', true);
    followFrame();
  });
}
function captureQuestion(draftId: string): import('../../../core/src/generated/VideoQuestionContext').VideoQuestionContext {
  const video = player.value, published = props.published;
  if (!video || !published || frameTime.value === null || video.seeking || video.readyState < 2) throw new Error(uiText('请等待视频画面加载后再输入'));
  video.pause();
  return { draft_id: draftId, publication: { ...published.publication }, material: structuredClone(JSON.parse(JSON.stringify(published.material))),
    frame: { video: { ...published.source.video }, actual_pts_ms: frameTime.value, width: video.videoWidth, height: video.videoHeight }, region: null, range: null };
}
const editing = computed(() => selecting.value || rangeEditing.value);
defineExpose({ captureQuestion, seek, editing });
let loadedPublication: string | undefined;
function seek(ms: number) { if (player.value) { player.value.currentTime = ms / 1000; position.value = ms; player.value.pause(); remember(); } }
function timeLabel(ms: number) { const s=Math.floor(ms/1000); return `${Math.floor(s/60)}:${String(s%60).padStart(2,'0')}`; }
let timer: ReturnType<typeof setTimeout> | undefined, disposed = false;
const key = `video-position:${props.owner}:${props.initial.video.media_id}:${props.initial.video.media_revision}`;
function remember() {
  if (!player.value || player.value.readyState === 0) return;
  try { localStorage.setItem(key, JSON.stringify({ time: player.value.currentTime, rate: player.value.playbackRate })); } catch { /* Playback works without browser storage. */ }
}
function restore() {
  followFrame();
  measure();
  if (!player.value) return;
  try {
    const saved = JSON.parse(localStorage.getItem(key) || 'null');
    if (saved && Number.isFinite(saved.time) && saved.time >= 0) player.value.currentTime = Math.min(saved.time, player.value.duration);
    if (saved && Number.isFinite(saved.rate) && saved.rate >= .25 && saved.rate <= 4) player.value.playbackRate = saved.rate;
  } catch { /* Start at the beginning if no valid local position exists. */ }
  position.value = player.value.currentTime * 1000;
}
async function refresh() {
  if (props.published || props.playback) return;
  try {
    const next = await videoGet<PreparingVideo>(videoUrl(props.initial.video, 'status'));
    if (!disposed) {
      state.value = next; error.value = '';
      if (next.english_ready && next.publication?.publication_id !== loadedPublication) {
        const value = await videoGet<{map: TranscriptMap}>(videoUrl(next.video,'transcript'));
        if (!disposed) { transcript.value=value.map; loadedPublication=next.publication?.publication_id; }
      }
    }
  } catch (failure) {
    if (!disposed) { remember(); player.value?.pause(); error.value = (failure as Error).message; }
  } finally { if (!disposed) timer = setTimeout(refresh, 3000); }
}
function playbackError() { playbackFailure.value = uiText('浏览器无法播放这份视频，请确认导入副本可读且浏览器支持 H.264/AAC MP4。'); }
watch([player, mediaSource, playlist], connectPlayback);
watch(player, video => {
  resizeObserver?.disconnect();
  if (video && typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(() => { if (drag) cancelSelection(); measure(); });
    resizeObserver.observe(video);
  }
});
onMounted(() => { window.addEventListener('pagehide', remember); window.addEventListener('keydown', escapeSelection); void refresh(); });
onBeforeUnmount(() => { disposed = true; clearTimeout(timer); resizeObserver?.disconnect(); window.removeEventListener('keydown', escapeSelection); window.removeEventListener('pagehide', remember); remember(); if (frameCallback !== undefined) player.value?.cancelVideoFrameCallback?.(frameCallback); releasePlayback?.(); });
</script>
<template>
  <main ref="surface" class="video-reader" :aria-label="uiText('视频学习')">
    <header class="video-heading">
      <div class="video-heading-copy">
        <div class="video-eyebrow"><span class="video-type"><Play :size="12" fill="currentColor" aria-hidden="true" />{{ uiText('视频学习') }}</span><span v-if="duration"><Clock3 :size="12" aria-hidden="true" />{{ timeLabel(duration) }}</span><span v-if="published"><Languages :size="13" aria-hidden="true" />{{ uiText('双语伴读') }}</span></div>
        <h1 :title="initial.title">{{ initial.title }}</h1>
      </div>
      <button v-if="!embedded" class="back-library" @click="$emit('close')"><ArrowLeft :size="15" aria-hidden="true" />{{ uiText('返回书架') }}</button>
      <div v-else class="video-watch-position" :aria-label="uiText('播放器当前位置')"><span>{{ uiText('观看至') }} <strong>{{ timeLabel(position) }}</strong></span><div class="video-watch-track" role="progressbar" :aria-label="uiText('观看位置')" :aria-valuenow="Math.round(playbackProgress)" aria-valuemin="0" aria-valuemax="100"><i :style="{ width: `${playbackProgress}%` }" /></div></div>
    </header>
    <div class="video-learning-layout">
    <section class="video-viewing" :aria-label="uiText('观看与选取')">
    <div class="video-player-card">
    <div v-if="state.playable && !error" class="video-stage">
      <video ref="player" :controls="!selecting" playsinline preload="metadata"
        :poster="playback?.cover_url || undefined" @waiting="buffering = true" @loadeddata="buffering = false" @canplay="buffering = false" @playing="buffering = false"
        @timeupdate="position = ($event.target as HTMLVideoElement).currentTime * 1000" @seeking="seeking" @loadedmetadata="restore" @resize="measure" @play="playbackStarted" @pause="paused = true; remember()" @seeked="remember" @ratechange="remember" @error="playbackError" />
      <div v-if="buffering && !playbackFailure" class="video-buffering" role="status">{{ uiText('正在加载画面…') }}</div>
      <div v-if="selecting || visibleRegion" class="video-region-layer" :class="{ selecting }" :style="contentStyle" :aria-label="uiText('画面框选区域')"
        @pointerdown="startSelection" @pointermove="moveSelection" @pointerup="finishSelection" @pointercancel="cancelSelection" @lostpointercapture="cancelSelection">
        <div v-if="visibleRegion" class="video-region-box" :style="regionStyle" />
      </div>
    </div>
    <div v-if="published" class="video-tools" :aria-label="uiText('视频提问工具')">
      <div class="region-actions">
        <button v-if="!selecting" :disabled="selectionDisabled || frameTime === null || !sameFrame" :title="uiText('暂停视频，框选想了解的代码、公式或图示')" @click="beginSelection"><Focus :size="16" aria-hidden="true" />{{ question?.region ? uiText('重新框选') : uiText('框选画面') }}</button>
        <button v-else class="is-active" @click="cancelSelection"><X :size="15" aria-hidden="true" />{{ uiText('退出框选') }}</button>
      </div>
      <div class="range-actions"><button :class="{ 'is-active': rangeEditing }" :disabled="selectionDisabled || (!question && frameTime === null)" @click="beginRange"><Scissors :size="15" aria-hidden="true" />{{ question?.range ? uiText('编辑时间范围') : uiText('选择时间范围') }}</button></div>
      <span class="video-tool-hint">{{ selecting ? uiText('拖动框选代码、公式或图示') : uiText('选取后，在伴读中提问') }}</span>
    </div>
    <div v-if="question?.region || question?.range || (question && !sameFrame)" class="video-selection-summary">
      <button v-if="question && !sameFrame" @click="seek(question.frame.actual_pts_ms)"><Crosshair :size="14" aria-hidden="true" />{{ uiText('回到问题画面') }}</button>
      <template v-if="question?.region"><span><Check :size="13" aria-hidden="true" />{{ uiText('已选区域 ·') }} {{ timeLabel(question.frame.actual_pts_ms) }}</span><button :disabled="selectionDisabled" @click="cancelSelection(); emit('clear-region')">{{ uiText('取消选区') }}</button></template>
      <template v-if="question?.range"><span :aria-label="uiText('已选时间范围')">{{ videoTime(question.range.start_ms) }}–{{ videoTime(question.range.end_ms) }}</span><button :disabled="selectionDisabled" @click="cancelRangeEdit(); emit('clear-range')">{{ uiText('取消时间范围') }}</button></template>
    </div>
      <form v-if="rangeEditing && published" class="video-range-editor" :aria-label="uiText('时间范围编辑')" @submit.prevent="applyRange">
        <div class="range-editor-heading"><strong>{{ uiText('选取想讨论的片段') }}</strong><span>{{ uiText('单位：秒') }}</span></div>
        <div class="range-field"><label>{{ uiText('起点（秒）') }}<input v-model="rangeStart" type="number" step="any" min="0" :max="published.source.duration_ms / 1000" /></label><button type="button" @click="markRange('start')">{{ uiText('当前位置设为起点') }}</button></div>
        <div class="range-field"><label>{{ uiText('终点（秒）') }}<input v-model="rangeEnd" type="number" step="any" min="0" :max="published.source.duration_ms / 1000" /></label><button type="button" @click="markRange('end')">{{ uiText('当前位置设为终点') }}</button></div>
        <div class="range-editor-buttons"><button type="button" @click="cancelRangeEdit">{{ uiText('放弃修改') }}</button><button type="submit" class="primary-button">{{ uiText('保存时间范围') }}</button></div>
      </form>
    </div>
    <p v-if="selectionError" class="video-error" role="alert">{{ selectionError }}</p>
    <p v-if="rangeError" role="alert">{{ rangeError }}</p>
    <p v-if="error || playbackFailure || state.failure" role="alert">{{ error || playbackFailure || state.failure }} <button v-if="playbackFailure" type="button" @click="retryPlayback">{{ uiText('重新加载') }}</button></p>
    <p v-if="detailsLoading" class="video-preparation" role="status">{{ uiText('字幕与学习记录正在载入，可先观看视频。') }}</p>
    <p v-if="detailsError" class="video-error" role="alert">{{ detailsError }}</p>
    <p v-if="adoptionError" class="video-preparation" role="status">{{ uiText('新成果暂未接入：') }}{{ adoptionError }}</p>
    <section v-if="currentSegment" class="current-bilingual" :aria-label="uiText('当前片段中英对照')">
      <div class="current-caption-label"><span><Captions :size="15" aria-hidden="true" />{{ uiText('此刻在讲') }}</span><button :aria-label="uiText('重听当前句 {0}', [timeLabel(currentSegment.location.start_ms)])" @click="seek(currentSegment.location.start_ms)"><Play :size="11" fill="currentColor" aria-hidden="true" />{{ timeLabel(currentSegment.location.start_ms) }}</button></div>
      <p class="current-original" lang="en">{{ currentSegment.text }}</p>
      <p class="current-translation" :class="{ 'translation-pending': !translations.has(currentSegment.segment_id) }">{{ translations.get(currentSegment.segment_id) ?? translationStatus(currentSegment.segment_id) }}</p>
    </section>
    <div v-if="!published && !playback" class="video-preparation" role="status">
      <template v-if="state.stage === 'copying'">{{ uiText('正在保留原片副本') }} <progress :value="state.copied_bytes" :max="state.total_bytes" /> {{ Math.floor(state.copied_bytes / state.total_bytes * 100) }}%</template>
      <template v-else-if="state.stage === 'probing'">{{ uiText('原片已导入，正在检查媒体与解码能力。') }}</template>
      <template v-else-if="state.stage === 'failed'">{{ uiText('准备未完成。处理失败原因后，请恢复此视频的导入任务。') }}</template>
      <template v-else-if="state.english_ready">{{ uiText('视频字幕已发布，可按当前画面提问并回看回答来源。') }}</template>
      <template v-else-if="state.transcript_progress"> {{ uiText('视频转写：') }}{{ state.transcript_progress.chunks.filter(c=>c.status==='completed').length }} / {{ state.transcript_progress.chunks.length }} {{ uiText('块完成。') }} <p v-if="state.transcript_progress.failure" role="alert">{{ state.transcript_progress.failure }}</p>
        <span v-if="state.transcript_progress.stage==='ready'">{{ uiText('正文与时间映射已完成，等待发布。') }}</span>
      </template>
      <template v-else><span v-if="state.playable">{{ uiText('原片可观看。') }}</span>{{ uiText('视频转写与首个发布尚未就绪。') }}</template>
    </div>
    <details v-if="published && transcript" class="video-readiness">
      <summary><span class="readiness-dot" :class="{ pending: untranslated.length }" /><span>{{ untranslated.length ? uiText('中文翻译 {0} / {1}', [transcript.segments.length - untranslated.length, transcript.segments.length]) : uiText('双语字幕已就绪') }}</span><ChevronDown :size="14" aria-hidden="true" /></summary>
      <div class="readiness-content"><p v-if="published.coverage" :aria-label="uiText('已接入成果')">{{ uiText('已接入') }} {{ published.translations?.length ?? 0 }} {{ uiText('段中文 · 视觉索引已覆盖') }} {{ Math.round((published.coverage.find(c => c.capability === 'visual_index')?.accepted.reduce((n, r) => n + r.end_ms - r.start_ms, 0) ?? 0) / 1000) }} {{ uiText('秒') }}</p>
      <div :aria-label="uiText('中文翻译进度')"> {{ uiText('中文') }} {{ transcript.segments.length - untranslated.length }} / {{ transcript.segments.length }} {{ uiText('段 · 待补齐') }} {{ untranslated.length }} {{ uiText('段') }}<span v-if="failedTranslations.length"> {{ uiText('· 失败') }} {{ failedTranslations.length }} {{ uiText('段，构建恢复时只重试未完成内容') }}</span>
      <button v-if="untranslated.length" @click="seek(untranslated[0].location.start_ms)">{{ uiText('回看首个待翻译片段') }} {{ timeLabel(untranslated[0].location.start_ms) }}</button>
      </div></div>
    </details>
    <button v-if="!embedded && state.publication" class="primary-button open-video-reader" @click="emit('ready', state.publication)">{{ uiText('打开视频阅读与提问') }}</button>
    <label v-if="!embedded && !state.publication" class="video-question">{{ uiText('伴读问题') }}<textarea disabled :placeholder="uiText('转写与发布就绪后开放问答')" /></label>
    <button v-if="!embedded && !state.publication" disabled>{{ state.english_ready ? uiText('视觉伴读接入中，暂不能提问') : uiText('准备中，暂不能提问') }}</button>
    </section>

    <section v-if="transcript" class="transcript-panel" :aria-label="uiText('视频字幕')">
      <header class="transcript-heading"><div><h2><Captions :size="18" aria-hidden="true" />{{ uiText('字幕') }}</h2><span>{{ transcript.segments.length }} {{ uiText('段 · 点击跳转') }}</span></div><button class="follow-transcript" :class="{ active: followingTranscript && !transcriptQuery.trim() }" :aria-pressed="followingTranscript && !transcriptQuery.trim()" @click="followCurrent"><Crosshair :size="14" aria-hidden="true" />{{ followingTranscript && !transcriptQuery.trim() ? uiText('跟随播放') : uiText('回到当前') }}</button></header>
      <div class="transcript-controls">
        <label class="transcript-search"><Search :size="15" aria-hidden="true" /><input v-model="transcriptQuery" type="search" :aria-label="uiText('搜索字幕')" :placeholder="uiText('搜索字幕中的内容…')" /><button v-if="transcriptQuery" :aria-label="uiText('清除字幕搜索')" @click="transcriptQuery = ''"><X :size="14" aria-hidden="true" /></button></label>
        <div class="transcript-languages" role="group" :aria-label="uiText('字幕语言')"><button :aria-pressed="transcriptLanguage === 'bilingual'" @click="transcriptLanguage = 'bilingual'">{{ uiText('双语') }}</button><button :aria-pressed="transcriptLanguage === 'en'" @click="transcriptLanguage = 'en'">{{ uiText('原文') }}</button><button :aria-pressed="transcriptLanguage === 'zh'" @click="transcriptLanguage = 'zh'">{{ uiText('中文') }}</button></div>
      </div>
      <p v-if="transcriptQuery.trim()" class="transcript-result-count" role="status">{{ uiText('找到') }} {{ filteredSegments.length }} {{ uiText('段字幕') }}</p>
      <div ref="transcriptSurface" class="transcript" :aria-label="uiText('视频转写')" tabindex="0" @wheel.passive="browseTranscript" @touchmove.passive="browseTranscript" @pointerdown="$event.target === transcriptSurface && browseTranscript()" @keydown="['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End'].includes($event.key) && browseTranscript()">
      <button v-for="segment in filteredSegments" :key="segment.segment_id" :data-segment="segment.segment_id" class="transcript-row" :class="{ current: currentSegment?.segment_id === segment.segment_id }" :aria-current="currentSegment?.segment_id === segment.segment_id ? 'true' : undefined" :aria-label="uiText('回看 {0}', [timeLabel(segment.location.start_ms)])" @click="seek(segment.location.start_ms)">
        <span class="segment-time"><Play v-if="currentSegment?.segment_id === segment.segment_id" :size="10" fill="currentColor" aria-hidden="true" />{{ timeLabel(segment.location.start_ms) }}</span>
        <span class="segment-copy"><span v-if="transcriptLanguage !== 'zh'" class="segment-original" lang="en">{{ segment.text }}</span>
        <template v-if="transcriptLanguage !== 'en'"><span v-if="translations.has(segment.segment_id)" class="segment-translation">{{ translations.get(segment.segment_id) }}</span>
        <span v-else-if="published" class="segment-translation translation-pending">{{ translationStatus(segment.segment_id) }}</span>
        <span v-else class="segment-translation translation-pending">{{ uiText('待翻译') }}</span></template></span>
      </button>
      <div v-if="!filteredSegments.length" class="transcript-empty"><Search :size="24" aria-hidden="true" /><p>{{ transcriptQuery.trim() ? uiText('没有找到相关字幕') : uiText('字幕正在准备中') }}</p><span>{{ transcriptQuery.trim() ? uiText('试试更短的词，中文和英文都可以') : uiText('准备完成后，可以在这里逐句回看') }}</span></div>
      </div>
      <footer class="transcript-footer">{{ uiText('视频字幕 · 技术词可能需要校正') }}</footer>
    </section>
    </div>
  </main>
</template>
<style scoped>
.video-reader { min-width: 0; min-height: 0; width: 100%; margin: 0 auto; padding: 28px 30px 20px; box-sizing: border-box; overflow: auto; background: #f8f7f4; container: video-reader / inline-size; color: var(--ink); }
.video-heading { display: flex; align-items: center; justify-content: space-between; gap: 24px; margin: 0 0 24px; }
.video-heading-copy { min-width: 0; }
.video-eyebrow { display: flex; align-items: center; gap: 15px; color: var(--steel); font-size: 11px; letter-spacing: .02em; }
.video-eyebrow > span { display: inline-flex; align-items: center; gap: 5px; }
.video-eyebrow .video-type { color: var(--brand-accent-deep); font-weight: 600; }
h1 { font-size: clamp(19px, 2.2cqi, 27px); font-weight: 600; letter-spacing: -.035em; line-height: 1.4; margin: 9px 0 0; overflow-wrap: anywhere; }
.video-watch-position { flex: 0 0 112px; color: var(--steel); font-size: 11px; text-align: right; font-variant-numeric: tabular-nums; }
.video-watch-position strong { font-size: 15px; color: var(--ink); margin-left: 4px; font-weight: 500; }
.video-watch-track { height: 3px; background: var(--hairline); border-radius: 3px; margin-top: 10px; overflow: hidden; }
.video-watch-track i { display: block; height: 100%; background: var(--brand-accent); }
.video-learning-layout { display: grid; gap: 22px; align-items: start; }
.video-viewing { min-width: 0; }
.video-player-card { overflow: hidden; border: 1px solid #dedbd5; border-radius: 14px; background: #fff; box-shadow: 0 4px 20px #28251f06; }
.video-stage { position: relative; }
.video-buffering { position: absolute; top: 1rem; left: 50%; transform: translateX(-50%); border-radius: 2rem; padding: .55rem 1rem; color: white; background: #16252bcc; pointer-events: none; font-size: .85rem; }
video { display: block; width: 100%; aspect-ratio: 16 / 9; max-height: 48vh; background: #171817; object-fit: contain; object-position: center; }
.video-region-layer { position: absolute; pointer-events: none; }
.video-region-layer.selecting { pointer-events: auto; touch-action: none; cursor: crosshair; }
.video-region-box { position: absolute; border: 2px solid #38bdf8; box-sizing: border-box; background: rgb(56 189 248 / 12%); pointer-events: none; }
.video-tools { display: flex; align-items: center; flex-wrap: wrap; gap: 5px; padding: 9px 12px; }
.video-tools button { min-height: 34px; border-color: transparent; padding: 6px 9px; }
.video-tools .is-active { color: var(--brand-accent-deep); background: var(--brand-accent-soft); }
.video-tool-hint { margin-left: auto; padding: 0 6px; color: var(--stone); font-size: 11px; }
.video-selection-summary { display: flex; align-items: center; flex-wrap: wrap; gap: 7px; padding: 10px 16px; border-top: 1px solid var(--hairline-soft); background: var(--brand-accent-soft); font-size: 12px; }
.video-selection-summary span { display: inline-flex; align-items: center; gap: 4px; }
.video-selection-summary button { min-height: 28px; padding: 3px 6px; border: 0; background: transparent; color: var(--brand-accent-deep); font-size: 11px; }
.video-range-editor { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; padding: 18px; border-top: 1px solid var(--hairline); background: #faf9f6; }
.range-editor-heading { grid-column: 1 / -1; display: flex; justify-content: space-between; gap: 12px; font-size: 13px; }
.range-editor-heading > span { color: var(--steel); font-size: 11px; }
.range-field { min-width: 0; }
.range-field label { display: grid; gap: 5px; font-size: 12px; color: var(--steel); }
.range-field input { width: 100%; min-width: 0; padding: 8px 10px; font: inherit; color: var(--ink); border: 1px solid var(--line); border-radius: 7px; background: #fff; }
.range-field button { border: 0; background: transparent; padding-inline: 0; color: var(--brand-accent-deep); font-size: 11px; }
.range-editor-buttons { grid-column: 1 / -1; display: flex; justify-content: flex-end; gap: 8px; }
.current-bilingual { margin-top: 20px; padding: 0 2px 0 16px; border-left: 2px solid var(--brand-accent-line); }
.current-caption-label { display: flex; justify-content: space-between; align-items: center; gap: 12px; color: var(--brand-accent-deep); font-size: 11px; }
.current-caption-label > span { display: inline-flex; align-items: center; gap: 7px; font-weight: 600; }
.current-caption-label button { min-height: 24px; padding: 2px 6px; font-size: 11px; color: var(--steel); border: 0; background: transparent; }
.current-original { font-size: 15px; line-height: 1.65; color: var(--ink); margin: 8px 0 5px; }
.current-translation { font-size: 13px; line-height: 1.75; color: var(--steel); margin: 0; }
.video-readiness { margin: 20px 0 0; font-size: 11px; color: var(--steel); }
.video-readiness summary { display: flex; align-items: center; gap: 7px; cursor: pointer; list-style: none; width: fit-content; min-height: 30px; }
.video-readiness summary::-webkit-details-marker { display: none; }
.video-readiness[open] summary > svg { transform: rotate(180deg); }
.readiness-dot { width: 5px; height: 5px; border-radius: 50%; background: #50856b; }
.readiness-dot.pending { background: #b98832; }
.readiness-content { padding: 10px 14px; margin-top: 5px; border: 1px solid var(--line); border-radius: 8px; line-height: 1.7; }
.readiness-content p { margin: 0 0 6px; }
.readiness-content button { display: block; margin-top: 8px; font-size: 11px; }
.video-preparation { padding: 16px 0; font-size: 13px; color: var(--steel); }
.transcript-panel { display: flex; flex-direction: column; min-width: 0; overflow: hidden; background: #fffefa; border: 1px solid #e8e5df; border-radius: 12px; }
.transcript-heading { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 17px 18px 13px; }
.transcript-heading > div { display: flex; align-items: baseline; flex-wrap: wrap; gap: 8px; }
h2 { display: inline-flex; align-items: center; gap: 7px; margin: 0; font-size: 14px; font-weight: 600; }
.transcript-heading > div > span { color: var(--stone); font-size: 10px; }
.follow-transcript { border: 0; min-height: 28px; padding: 4px 6px; color: var(--steel); white-space: nowrap; font-size: 11px; }
.follow-transcript.active { color: var(--brand-accent-deep); background: var(--brand-accent-soft); }
.transcript-controls { display: flex; align-items: center; gap: 12px; padding: 0 16px 12px; border-bottom: 1px solid var(--hairline-soft); }
.transcript-search { display: flex; flex: 1; align-items: center; gap: 7px; min-width: 0; padding: 0 10px; border: 1px solid #eae7e1; border-radius: 7px; background: #f8f7f4; color: var(--stone); }
.transcript-search:focus-within { outline: 2px solid var(--brand-accent-line); outline-offset: 1px; }
.transcript-search input { flex: 1; min-width: 0; width: 100%; height: 33px; padding: 0; border: 0; border-radius: 0; background: transparent; box-shadow: none; outline: 0; font: inherit; font-size: 12px; color: var(--ink); }
.transcript-search input::-webkit-search-cancel-button { display: none; }
.transcript-search > button { padding: 0; min-height: 24px; border: 0; background: transparent; }
.transcript-languages { display: flex; flex: 0 0 auto; gap: 2px; padding: 2px; border-radius: 7px; background: #f1efea; }
.transcript-languages button { min-height: 27px; padding: 3px 9px; border: 0; border-radius: 5px; background: transparent; color: var(--steel); font-size: 11px; }
.transcript-languages button[aria-pressed="true"] { background: #fff; color: var(--ink); box-shadow: 0 1px 3px #25232012; }
.transcript { max-height: 330px; overflow: auto; overscroll-behavior: contain; padding: 8px; scrollbar-width: thin; scrollbar-color: #d4cfc5 transparent; }
.transcript .transcript-row { display: grid; grid-template-columns: 52px minmax(0, 1fr); gap: 9px; align-items: start; width: 100%; min-height: 46px; padding: 13px 12px; border: 0; border-radius: 7px; background: transparent; text-align: left; font-weight: 400; line-height: 1.65; }
.transcript .transcript-row + .transcript-row { margin-top: 3px; }
.transcript .transcript-row:hover { background: #f4f2ec; }
.transcript .transcript-row.current { background: #f6eadf; box-shadow: inset 2px 0 var(--brand-accent); }
.segment-time { display: flex; align-items: center; justify-content: flex-end; gap: 4px; padding-top: 2px; color: var(--stone); font-size: 11px; font-variant-numeric: tabular-nums; }
.current .segment-time { color: var(--brand-accent-deep); }
.segment-copy { min-width: 0; overflow-wrap: anywhere; }
.segment-original { display: block; color: var(--slate); font-size: 14px; }
.segment-translation { display: block; color: var(--steel); margin-top: 5px; font-size: 13px; }
.translation-pending { color: var(--stone); font-size: 11px; }
.transcript-footer { padding: 9px 18px; border-top: 1px solid var(--hairline-soft); color: var(--stone); font-size: 10px; }
.transcript-result-count { margin: 8px 18px 0; color: var(--steel); font-size: 11px; }
.transcript-empty { display: grid; justify-items: center; gap: 8px; padding: 38px 14px; color: var(--stone); text-align: center; font-size: 11px; }
.transcript-empty p { color: var(--steel); margin: 4px 0 0; font-size: 13px; }
.video-question { display: grid; gap: 8px; margin-top: 24px; }
textarea { min-height: 70px; resize: vertical; font: inherit; padding: 12px; }
button { display: inline-flex; justify-content: center; align-items: center; gap: 6px; padding: 7px 11px; min-height: 32px; border: 1px solid var(--hairline); border-radius: 7px; font: inherit; font-size: 12px; background: transparent; cursor: pointer; }
button:hover:not(:disabled) { background: #f3f0e9; }
button:disabled { opacity: .4; cursor: default; }
button:focus-visible, .transcript:focus-visible { outline: 2px solid var(--brand-accent); outline-offset: 2px; }
.primary-button { border-color: var(--brand-accent); background: var(--brand-accent); color: #fff; }
.primary-button:hover:not(:disabled) { background: var(--brand-accent-deep); }
.open-video-reader { margin-top: 18px; }
[role=alert] { color: #b42318; overflow-wrap: anywhere; font-size: 12px; }
@container video-reader (min-width: 780px) {
  .video-learning-layout { grid-template-columns: minmax(0, 1.75fr) minmax(280px, 1fr); gap: 22px; }
  .transcript-panel { position: sticky; top: 0; max-height: calc(100dvh - 190px); }
  .transcript-controls { flex-wrap: wrap; }
  .transcript-search { flex-basis: 100%; }
  .transcript { max-height: none; min-height: 0; flex: 1; }
  .transcript-heading > div { gap: 4px; }
}
@container video-reader (max-width: 570px) {
  .video-watch-position { flex-basis: 78px; }
  .video-heading { gap: 16px; margin-bottom: 18px; }
  .video-eyebrow { gap: 10px; font-size: 10px; }
  .video-eyebrow > span:last-child:not(.video-type) { display: none; }
  .video-tools { padding: 7px; }
  .video-tool-hint { width: 100%; margin: 2px 0 3px; }
  .transcript-controls { flex-wrap: wrap; gap: 9px; }
  .transcript-search { flex-basis: 100%; }
  .current-original { font-size: 14px; }
  .current-translation { font-size: 12px; }
  .transcript-heading { padding-inline: 14px; }
  .transcript-heading > div { gap: 4px; }
}
@media (max-width: 1023px) { .video-reader { padding: 20px; } }
@media (max-width: 480px) { .video-reader { padding: 18px 14px; } .video-player-card { border-radius: 10px; } .video-heading h1 { font-size: 19px; } }
</style>
