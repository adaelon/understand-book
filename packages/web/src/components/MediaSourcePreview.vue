<script setup lang="ts">
import { computed, ref, onBeforeUnmount } from 'vue';
import type { SourcePopupView } from '../api';
import type { EvidenceTarget } from '../../../core/src/generated/EvidenceTarget';
import { videoTime } from '../video-question';
const props = defineProps<{ source: SourcePopupView }>();
const failure = ref('');
const root = ref<HTMLElement>();
const orderedFrames = computed(() => [...(props.source.frames ?? [])].sort((a, b) => a.frame.actual_pts_ms - b.frame.actual_pts_ms));
const visualTimes = computed(() => {
  const target = props.source.media?.target;
  const targets = target?.kind === 'joint' ? target.members.map(m => m.target) : target ? [target] : [];
  return [...new Set(targets.flatMap(t => t.kind === 'visual'
    ? [...t.frames, ...t.regions.map(r => r.frame)].map(f => f.actual_pts_ms) : []))].sort((a, b) => a - b);
});
onBeforeUnmount(() => root.value?.querySelectorAll('video, audio').forEach(player => (player as HTMLMediaElement).pause()));
const clips = computed(() => {
  const media = props.source.media;
  if (!media) return [];
  const targets = media.target.kind === 'joint' ? media.target.members : [{ target: media.target, support_role: '' }];
  return targets.flatMap(({ target: t, support_role }) => {
    const clip = (start: number, end: number, audio = false) => ({ start, end, audio, role: support_role });
    switch ((t as EvidenceTarget).kind) {
      case 'audio': { const a = t as Extract<EvidenceTarget, {kind:'audio'}>; return [clip(a.location.start_ms, a.location.end_ms, true)]; }
      case 'visual': { const v = t as Extract<EvidenceTarget, {kind:'visual'}>; return [...v.frames, ...v.regions.map(r => r.frame)].sort((a, b) => a.actual_pts_ms - b.actual_pts_ms).map(f => clip(f.actual_pts_ms, Math.min(f.actual_pts_ms + 5000, media.duration_ms))); }
      case 'text': { const x = t as Extract<EvidenceTarget, {kind:'text'}>; return media.transcript.filter(s => s.text_range.lid === x.text_range.lid).map(s => clip(s.location.start_ms, s.location.end_ms)); }
      default: return [];
    }
  });
});
function initialize(event: Event, start: number) { (event.target as HTMLMediaElement).currentTime = start / 1000; }
function bound(event: Event, start: number, end: number) {
  const player = event.target as HTMLMediaElement;
  if (player.currentTime >= end / 1000) { player.pause(); player.currentTime = start / 1000; }
  else if (player.currentTime < start / 1000) player.currentTime = start / 1000;
}
</script>
<template>
  <section ref="root" class="media-source-preview" aria-label="媒体来源预览">
    <p v-if="source.unavailable_reason || failure" role="alert">{{ source.unavailable_reason || failure }}</p>
    <section v-if="visualTimes.length" aria-label="画面引用覆盖">
      <p>画面引用仅支持所列帧或区域；原片上下文供回看。</p>
      <ul v-if="visualTimes.length > 1" aria-label="未连续观察的间隙">
        <li v-for="(time, i) in visualTimes.slice(1)" :key="time">{{ videoTime(visualTimes[i]) }}–{{ videoTime(time) }}：帧间过程未连续观察</li>
      </ul>
    </section>
    <figure v-for="(image, i) in orderedFrames" :key="i">
      <img :src="image.url" :alt="`实际读取${image.region ? '区域' : '画面'} ${videoTime(image.frame.actual_pts_ms)}`" />
      <figcaption>{{ image.region ? '已读取区域' : '已读取画面' }} · {{ videoTime(image.frame.actual_pts_ms) }}</figcaption>
    </figure>
    <template v-if="!source.stale && source.media_url">
      <div v-for="(clip, i) in clips" :key="`${source.source_ref_id}-${i}`" class="media-clip">
        <p>{{ clip.role }} {{ clip.audio ? '原音频' : '原片上下文' }} · {{ videoTime(clip.start) }}–{{ videoTime(clip.end) }}</p>
        <component :is="clip.audio ? 'audio' : 'video'" controls playsinline preload="metadata" :src="source.media_url"
          @loadedmetadata="initialize($event, clip.start)" @timeupdate="bound($event, clip.start, clip.end)"
          @error="failure = '原发布媒体当前不可用；已保留引用位置和文字快照。'" />
      </div>
    </template>
    <section v-if="source.media?.audio_reviews?.length" aria-label="原音频复核结果">
      <div v-for="review in source.media.audio_reviews" :key="review.review_result_ref">
        <p>原音频复核 · 机器识别候选 · {{ videoTime(review.location.start_ms) }}–{{ videoTime(review.location.end_ms) }}</p>
        <p>{{ review.reviewed_text || '未识别出措辞' }}</p>
        <ul><li v-for="uncertainty in review.uncertainties" :key="uncertainty">{{ uncertainty }}</li></ul>
      </div>
    </section>
    <section v-if="source.media?.transcript.length" aria-label="相关英文与译文">
      <p class="transcript-note">相关英文 · 机器转写 · 句段定位</p>
      <div v-for="segment in source.media.transcript" :key="segment.segment_id">
        <p>{{ videoTime(segment.location.start_ms) }} {{ segment.text }}</p>
        <p v-for="translation in source.media.translations.filter(t => t.segment_id === segment.segment_id)" :key="translation.segment_id" lang="zh">{{ translation.zh_text }}</p>
      </div>
    </section>
  </section>
</template>
<style scoped>
video, audio, img { display: block; width: 100%; max-height: 300px; }
figure { margin: 12px 0; } figcaption, .transcript-note { font-size: 12px; color: var(--ink-muted); }
.media-clip { margin: 12px 0; } p { line-height: 1.6; overflow-wrap: anywhere; }
[role=alert] { color: #b42318; }
</style>
