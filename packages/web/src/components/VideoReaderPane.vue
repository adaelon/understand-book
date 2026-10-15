<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { videoGet, videoUrl, type PreparingVideo } from '../video-api';
import type { TranscriptMap } from '../../../core/src/generated/TranscriptMap';
import type { VideoQuestionContext } from '../../../core/src/generated/VideoQuestionContext';
import { videoContentRect, videoRegion, type DisplayRect, type Point } from '../video-region';
import { videoTime } from '../video-question';
const props = defineProps<{ initial: PreparingVideo; owner: string; published?: import('../video-api').PublishedVideo; embedded?: boolean; mediaUrl?: string; question?: VideoQuestionContext | null; selectionDisabled?: boolean }>();
const emit = defineEmits<{ (event: 'close'): void; (event: 'frame-ready', ready: boolean): void; (event: 'ready', publication: NonNullable<PreparingVideo['publication']>): void; (event: 'select-region', context: VideoQuestionContext): void; (event: 'clear-region'): void; (event: 'select-range', context: VideoQuestionContext): void; (event: 'clear-range'): void }>();
const state = ref(props.initial), player = ref<HTMLVideoElement>(), error = ref(''), playbackFailure = ref('');
const transcript = ref<TranscriptMap | undefined>(props.published?.map);
const surface = ref<HTMLElement>(), transcriptSurface = ref<HTMLElement>();
const translations = computed(() => new Map(props.published?.translations?.map(t => [t.segment_id, t.zh_text]) ?? []));
const translationTasks = computed(() => new Map(props.published?.translation_progress?.tasks.flatMap(t => t.segment_ids.map(id => [id, t] as const)) ?? []));
function translationStatus(id: string) {
  const task = translationTasks.value.get(id);
  if (task?.status === 'failed') return '翻译失败，等待构建端重试';
  if (task?.status === 'running') return task.attempts > 1 ? '正在重试翻译' : '正在翻译';
  if (task?.status === 'accepted') return '译文已生成，等待发布';
  return '待翻译';
}
const untranslated = computed(() => transcript.value?.segments.filter(s => !translations.value.has(s.segment_id)) ?? []);
const failedTranslations = computed(() => untranslated.value.filter(s => translationTasks.value.get(s.segment_id)?.status === 'failed'));
const currentSegment = computed(() => transcript.value?.segments.find(s => s.location.start_ms <= position.value && s.location.end_ms > position.value));
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
const position = ref(0);
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
    rangeError.value = '请填写视频内的起止秒数，终点须晚于起点'; return;
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
    if (!sameFrame.value) throw new Error('请回到问题绑定画面再框选');
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
  if (dragRegion.value) emit('select-region', { ...selectionContext, region: dragRegion.value });
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
    if (props.published) {
      const [numerator, denominator] = props.published.time_base;
      frameTime.value = Math.round(metadata.mediaTime * denominator / numerator) * numerator / denominator * 1000;
    }
    emit('frame-ready', true);
    followFrame();
  });
}
function captureQuestion(draftId: string): import('../../../core/src/generated/VideoQuestionContext').VideoQuestionContext {
  const video = player.value, published = props.published;
  if (!video || !published || frameTime.value === null || video.seeking || video.readyState < 2) throw new Error('请等待视频画面加载后再输入');
  video.pause();
  return { draft_id: draftId, publication: { ...published.publication }, material: structuredClone(JSON.parse(JSON.stringify(published.material))),
    frame: { video: { ...published.source.video }, actual_pts_ms: frameTime.value, width: video.videoWidth, height: video.videoHeight }, region: null, range: null };
}
const editing = computed(() => selecting.value || rangeEditing.value);
defineExpose({ captureQuestion, seek, editing });
let loadedPublication: string | undefined;
function seek(ms: number) { if (player.value) { player.value.currentTime = ms / 1000; player.value.pause(); remember(); } }
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
}
async function refresh() {
  if (props.published) return;
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
function playbackError() { playbackFailure.value = '浏览器无法播放这份视频，请确认导入副本可读且浏览器支持 H.264/AAC MP4。'; }
watch(player, video => {
  resizeObserver?.disconnect();
  if (video && typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(() => { if (drag) cancelSelection(); measure(); });
    resizeObserver.observe(video);
  }
});
onMounted(() => { window.addEventListener('pagehide', remember); window.addEventListener('keydown', escapeSelection); void refresh(); });
onBeforeUnmount(() => { disposed = true; clearTimeout(timer); resizeObserver?.disconnect(); window.removeEventListener('keydown', escapeSelection); window.removeEventListener('pagehide', remember); remember(); if (frameCallback !== undefined) player.value?.cancelVideoFrameCallback?.(frameCallback); player.value?.pause(); });
</script>
<template>
  <main ref="surface" class="video-reader">
    <header v-if="!embedded"><h1>{{ state.title }}</h1><button @click="$emit('close')">返回书架</button></header>
    <div v-if="state.playable && !error" class="video-stage">
      <video ref="player" :controls="!selecting" playsinline preload="metadata"
        :src="mediaUrl || videoUrl(state.video, 'media')" @timeupdate="position = ($event.target as HTMLVideoElement).currentTime * 1000" @seeking="seeking" @loadedmetadata="restore" @resize="measure" @play="playbackStarted" @pause="paused = true; remember()" @seeked="remember" @ratechange="remember" @error="playbackError" />
      <div v-if="selecting || visibleRegion" class="video-region-layer" :class="{ selecting }" :style="contentStyle" aria-label="画面框选区域"
        @pointerdown="startSelection" @pointermove="moveSelection" @pointerup="finishSelection" @pointercancel="cancelSelection" @lostpointercapture="cancelSelection">
        <div v-if="visibleRegion" class="video-region-box" :style="regionStyle" />
      </div>
    </div>
    <div v-if="published" class="region-actions">
      <button v-if="!selecting" :disabled="selectionDisabled || frameTime === null || !sameFrame" @click="beginSelection">{{ question?.region ? '重新框选' : '框选画面' }}</button>
      <template v-else><span>拖动框选代码、公式或图示</span><button @click="cancelSelection">退出框选</button></template>
      <button v-if="question && !sameFrame" @click="seek(question.frame.actual_pts_ms)">回到问题画面</button>
      <template v-if="question?.region"><span>已选区域 · {{ timeLabel(question.frame.actual_pts_ms) }}</span><button :disabled="selectionDisabled" @click="cancelSelection(); emit('clear-region')">取消选区</button></template>
    </div>
    <p v-if="selectionError" role="alert">{{ selectionError }}</p>
    <div v-if="published" class="range-actions">
      <button :disabled="selectionDisabled || (!question && frameTime === null)" @click="beginRange">{{ question?.range ? '编辑时间范围' : '选择时间范围' }}</button>
      <template v-if="question?.range"><span aria-label="已选时间范围">{{ videoTime(question.range.start_ms) }}–{{ videoTime(question.range.end_ms) }}</span><button :disabled="selectionDisabled" @click="cancelRangeEdit(); emit('clear-range')">取消时间范围</button></template>
      <form v-if="rangeEditing" aria-label="时间范围编辑" @submit.prevent="applyRange">
        <label>起点（秒）<input v-model="rangeStart" type="number" step="any" min="0" :max="published.source.duration_ms / 1000" /></label>
        <button type="button" @click="markRange('start')">当前位置设为起点</button>
        <label>终点（秒）<input v-model="rangeEnd" type="number" step="any" min="0" :max="published.source.duration_ms / 1000" /></label>
        <button type="button" @click="markRange('end')">当前位置设为终点</button>
        <button type="submit">保存时间范围</button><button type="button" @click="cancelRangeEdit">放弃修改</button>
      </form>
    </div>
    <p v-if="rangeError" role="alert">{{ rangeError }}</p>
    <p v-if="error || playbackFailure || state.failure" role="alert">{{ error || playbackFailure || state.failure }}</p>
    <p v-if="published" aria-label="播放器当前位置">当前播放 {{ timeLabel(position) }}</p>
    <section v-if="currentSegment" class="current-bilingual" aria-label="当前片段中英对照">
      <p>{{ currentSegment.text }}</p>
      <p>{{ translations.get(currentSegment.segment_id) ?? translationStatus(currentSegment.segment_id) }}</p>
    </section>
    <div role="status">
      <template v-if="state.stage === 'copying'">正在保留原片副本 <progress :value="state.copied_bytes" :max="state.total_bytes" /> {{ Math.floor(state.copied_bytes / state.total_bytes * 100) }}%</template>
      <template v-else-if="state.stage === 'probing'">原片已导入，正在检查媒体与解码能力。</template>
      <template v-else-if="state.stage === 'failed'">准备未完成。处理失败原因后，请恢复此视频的导入任务。</template>
      <template v-else-if="state.english_ready">全片英文已发布，可按当前画面提问并回看回答来源。</template>
      <template v-else-if="state.transcript_progress">
        英文转写：{{ state.transcript_progress.chunks.filter(c=>c.status==='completed').length }} / {{ state.transcript_progress.chunks.length }} 块完成。
        <p v-if="state.transcript_progress.failure" role="alert">{{ state.transcript_progress.failure }}</p>
        <span v-if="state.transcript_progress.stage==='ready'">正文与时间映射已完成，等待发布。</span>
      </template>
      <template v-else><span v-if="state.playable">原片可观看。</span>全片英文转写与首个发布尚未就绪。</template>
    </div>
    <p v-if="published?.coverage" aria-label="已接入成果">已接入 {{ published.translations?.length ?? 0 }} 段中文 · 视觉索引已覆盖 {{ Math.round((published.coverage.find(c => c.capability === 'visual_index')?.accepted.reduce((n, r) => n + r.end_ms - r.start_ms, 0) ?? 0) / 1000) }} 秒</p>
    <div v-if="published && transcript" aria-label="中文翻译进度">
      中文 {{ transcript.segments.length - untranslated.length }} / {{ transcript.segments.length }} 段 · 待补齐 {{ untranslated.length }} 段<span v-if="failedTranslations.length"> · 失败 {{ failedTranslations.length }} 段，构建恢复时只重试未完成内容</span>
      <button v-if="untranslated.length" @click="seek(untranslated[0].location.start_ms)">回看首个待翻译片段 {{ timeLabel(untranslated[0].location.start_ms) }}</button>
    </div>
    <section v-if="transcript" ref="transcriptSurface" class="transcript" aria-label="英文转写">
      <p>机器转写 · 句段时间定位 · 技术词可能需要校正</p>
      <p v-for="segment in transcript.segments" :key="segment.segment_id" :data-segment="segment.segment_id">
        <button :aria-label="`回看 ${timeLabel(segment.location.start_ms)}`" @click="seek(segment.location.start_ms)">{{ timeLabel(segment.location.start_ms) }}</button>
        {{ segment.text }}
        <span v-if="translations.has(segment.segment_id)" class="segment-translation">{{ translations.get(segment.segment_id) }}</span>
        <span v-else-if="published" class="segment-translation translation-pending">{{ translationStatus(segment.segment_id) }}</span>
      </p>
    </section>
    <button v-if="!embedded && state.publication" @click="emit('ready', state.publication)">打开视频阅读与提问</button>
    <label v-if="!embedded && !state.publication" class="video-question">伴读问题<textarea disabled placeholder="转写与发布就绪后开放问答" /></label>
    <button v-if="!embedded && !state.publication" disabled>{{ state.english_ready ? '视觉伴读接入中，暂不能提问' : '准备中，暂不能提问' }}</button>
  </main>
</template>
<style scoped>
.video-reader { width: min(1120px, 100%); margin: 0 auto; padding: 24px; box-sizing: border-box; overflow: auto; }
.segment-translation { display: block; color: var(--text-muted, #666); margin-top: 4px; }
.current-bilingual { min-height: 5em; padding: 8px 16px; background: var(--bg-subtle, #f5f5f5); border-radius: 8px; }
.current-bilingual p { margin: 6px 0; }
.translation-pending { font-size: .9em; }
header { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
h1 { font-size: 20px; overflow-wrap: anywhere; }
.video-stage { position: relative; margin: 20px 0; }
video { display: block; width: 100%; max-height: 68vh; background: #111; object-fit: contain; object-position: center; }
.video-region-layer { position: absolute; pointer-events: none; }
.video-region-layer.selecting { pointer-events: auto; touch-action: none; cursor: crosshair; }
.video-region-box { position: absolute; border: 2px solid #38bdf8; box-sizing: border-box; background: rgb(56 189 248 / 12%); pointer-events: none; }
.region-actions { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
.range-actions, .range-actions form { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; margin-top: 8px; }
.range-actions form { flex-basis: 100%; }
.range-actions label { display: flex; align-items: center; gap: 6px; }
.range-actions input { width: 100px; padding: 8px; font: inherit; }
.video-question { display: grid; gap: 8px; margin-top: 24px; }
textarea { min-height: 70px; resize: vertical; font: inherit; padding: 12px; }
.transcript { max-height: 38vh; overflow: auto; line-height: 1.6; }
.transcript button { margin-right: 8px; }
button { padding: 8px 12px; }
[role=alert] { color: #b42318; overflow-wrap: anywhere; }
</style>
