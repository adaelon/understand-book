<script setup lang="ts">
import { t as uiText } from '../i18n';

import { ref, watch } from 'vue';
import { Play, Music2 } from '@lucide/vue';
import BookCover from './BookCover.vue';
import type { BookCoverSource } from '../api';
const props = defineProps<{ title: string; cover?: BookCoverSource | null; video?: boolean; music?: boolean; compact?: boolean; paused?: boolean }>();
const failed = ref(false);
watch(() => props.cover?.url, () => { failed.value = false; });
</script>

<template>
  <div v-if="video" class="material-video-cover" :class="{ compact }" aria-hidden="true">
    <div class="video-cover-art"><span class="video-cover-eyebrow">{{ uiText('VIDEO / 课程录像') }}</span><strong>{{ title }}</strong><span class="video-cover-line"></span></div>
    <img v-if="cover?.kind === 'image' && !failed" :src="cover.url" alt="" loading="lazy" decoding="async" @error="failed = true" />
    <span class="video-cover-play"><Play :size="compact ? 14 : 22" fill="currentColor" :stroke-width="1.5" /></span>
    <span v-if="!compact" class="video-cover-label">{{ uiText('课程视频') }}</span>
  </div>
  <div v-else-if="music" class="material-music-cover" :class="{ compact }" aria-hidden="true"><Music2 :size="compact ? 22 : 38" /><span v-if="!compact">{{ uiText('MUSIC / 精选伴听') }}</span><strong v-if="!compact">{{ title }}</strong></div>
  <BookCover v-else :title="title" :cover="cover" :compact="compact" :paused="paused" />
</template>

<style scoped>
.material-music-cover{width:100%;aspect-ratio:16/10;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:13px;background:linear-gradient(145deg,#e8e8d7,#d1dcc2);color:#55714f;border-radius:10px;padding:20px;box-sizing:border-box}.material-music-cover span{font-size:9px;letter-spacing:.13em}.material-music-cover strong{font:500 19px/1.5 var(--serif);text-align:center;overflow-wrap:anywhere}.material-music-cover.compact{height:100%;padding:6px;border-radius:5px;aspect-ratio:auto}
.material-video-cover { position: relative; width: 100%; aspect-ratio: 16 / 10; overflow: hidden; border-radius: 10px; background: #263d37; color: #f9f4e8; }
.video-cover-art { position: absolute; inset: 0; display: flex; flex-direction: column; justify-content: center; gap: 14px; padding: 24px 26px 52px; background: radial-gradient(ellipse at 100% 0, #65857970, transparent 65%); }
.video-cover-eyebrow { font: 9px/1.4 var(--sans); letter-spacing: .16em; color: #bdcfc3; }
.video-cover-art strong { font-size: clamp(16px, 1.4vw, 22px); font-weight: 500; line-height: 1.5; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 3; overflow: hidden; overflow-wrap: anywhere; }
.video-cover-line { width: 36px; height: 2px; background: #d5b78b; }
.material-video-cover img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
.video-cover-play { position: absolute; bottom: 14px; left: 16px; display: grid; place-items: center; width: 36px; height: 36px; border: 1px solid #ffffff70; border-radius: 50%; background: #1d302bba; color: white; }
.video-cover-label { position: absolute; bottom: 21px; right: 14px; background: #1d302bc9; padding: 3px 7px; border-radius: 4px; font-size: 10px; }
.compact { border-radius: 5px; height: 100%; aspect-ratio: auto; }
.compact .video-cover-art { padding: 8px; }.compact .video-cover-art strong,.compact .video-cover-eyebrow,.compact .video-cover-line { display: none; }
.compact .video-cover-play { width: 26px; height: 26px; top: 50%; left: 50%; transform: translate(-50%,-50%); }
</style>
