<script setup lang="ts">
import { computed, ref } from 'vue';
import type { SourcePopupView } from '../api';
import { musicTime } from '../music-playback';
const props = defineProps<{ source: SourcePopupView }>();
const audio = ref<HTMLAudioElement>(), error = ref('');
const target = computed(() => props.source.music!.target);
function locate() { if (audio.value) audio.value.currentTime = target.value.audio.start_ms / 1000; }
function bound() { if (audio.value && audio.value.currentTime * 1000 >= target.value.audio.end_ms) { audio.value.pause(); audio.value.currentTime = target.value.audio.end_ms / 1000; } }
</script>
<template>
  <div class="music-source-preview"><p class="music-source-range">原录音 {{ musicTime(target.audio.start_ms) }}–{{ musicTime(target.audio.end_ms) }} · 来源版本已固定</p><p class="music-source-copy">{{ source.music!.preview }}</p><audio v-if="source.media_url" ref="audio" :src="source.media_url" controls preload="metadata" @loadedmetadata="locate" @timeupdate="bound" @play="() => { if (audio && (audio.currentTime * 1000 < target.audio.start_ms || audio.currentTime * 1000 >= target.audio.end_ms)) locate(); }" @error="error = '该版本录音暂不可读取'" /><p v-if="error" role="alert">{{ error }}</p><details v-if="source.music_score_pages?.length"><summary>查看引用谱面</summary><img v-for="page in source.music_score_pages" :key="page.page_id" :src="page.url" :alt="`来源乐谱 ${page.page_id}`"></details><p v-if="source.stale" class="music-source-unavailable">{{ source.unavailable_reason || '原资料暂不可打开，保留引用时的内容。' }}</p></div>
</template>
<style scoped>
.music-source-preview{font-size:13px;line-height:1.8;color:var(--ink)}.music-source-range{font-size:11px;color:var(--steel)}.music-source-copy{white-space:pre-wrap}.music-source-preview audio{width:100%;height:36px;margin:8px 0}.music-source-preview details{margin-top:12px}.music-source-preview summary{cursor:pointer;color:#577346}.music-source-preview img{display:block;width:100%;height:auto;margin-top:10px;background:#fffef8}.music-source-unavailable{font-size:12px;color:var(--steel)}
</style>
