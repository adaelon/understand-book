<script setup lang="ts">
import { t as uiText } from './i18n';

import { onBeforeUnmount, onMounted, ref } from 'vue';
import App from './App.vue';
import LoadingAnimation from './components/LoadingAnimation.vue';
import VideoReaderPane from './components/VideoReaderPane.vue';
import MusicCalibrationPane from './components/MusicCalibrationPane.vue';
import { api } from './api';
import type { MusicAlignmentRevision, MusicCalibrationContext } from './music-calibration';
import { videoUrl, videoCatalog, type PreparingVideo } from './video-api';
const videos = ref<PreparingVideo[]>([]), selected = ref<PreparingVideo | null>(null), loading = ref(true), error = ref('');
const calibration = ref<MusicCalibrationContext | null>(null), calibrationOpen = ref(false), calibrationSaving = ref(false), calibrationError = ref('');
const calibrationOpening = ref(false);
const courseTrial = ref<{ bookId: string; revision: string } | null>(null);
const trialError = ref('');
async function openLinkedTrial(params: URLSearchParams) {
  const bookId = params.get('course_book'), revision = params.get('trial');
  if (!bookId || !revision || revision.includes(':')) throw new Error(uiText('试课链接不完整，请重新打开原链接。'));
  const current = await api.sourceFingerprint().catch(() => null);
  if (current?.book_id !== bookId) {
    const library = await api.bookLibrary();
    const book = library.books.find(entry => entry.book_id === bookId && entry.route === 'reader');
    if (!book) throw new Error(uiText('当前书库没有这份试课的原书。'));
    await api.openBook(book.dir);
  }
  courseTrial.value = { bookId, revision };
}
const libraryEntry = ['library', 'materials', 'boards', 'understanding'].includes(new URLSearchParams(location.search).get('view') ?? '');
let calibrationRequest = 0;
async function refreshCalibration(open = false) {
  const request = ++calibrationRequest;
  calibrationOpen.value = false; calibration.value = null; calibrationError.value = '';
  calibrationOpening.value = open;
  try {
    const context = await api.musicCalibrationContext();
    if (request !== calibrationRequest) return;
    calibration.value = context; calibrationOpen.value = open && !!context?.manifest.occurrences.length;
  } catch (failure) { if (request === calibrationRequest) calibrationError.value = (failure as Error).message; }
  finally { if (request === calibrationRequest) calibrationOpening.value = false; }
}
function workspaceChanged() { void refreshCalibration(); }
async function saveCalibration(revision: MusicAlignmentRevision) {
  calibrationSaving.value = true; calibrationError.value = '';
  try { calibration.value = await api.musicCalibrate(revision); }
  catch (failure) { calibrationError.value = (failure as Error).message; }
  finally { calibrationSaving.value = false; }
}
async function openVideo() {
  if (!selected.value) return;
  try {
    const response = await fetch(videoUrl(selected.value.video, 'open'), { method: 'POST' });
    if (!response.ok) throw new Error((await response.json()).message || uiText('无法打开视频发布'));
    selected.value = null;
  } catch (failure) { error.value = (failure as Error).message; }
}
onMounted(async () => {
  window.addEventListener('local-workspace-opened', workspaceChanged);
  const params = new URLSearchParams(location.search);
  if (params.has('trial') || params.has('course_book')) {
    try { await openLinkedTrial(params); }
    catch (failure) { trialError.value = (failure as Error).message; }
    loading.value = false;
    return;
  }
  await Promise.allSettled([(async () => { try {
    const catalog = await videoCatalog(); videos.value = catalog.videos;
    selected.value = libraryEntry ? null : videos.value.find(v => v.video.media_id === catalog.active?.media_id && v.video.media_revision === catalog.active?.media_revision) ?? null;
  } catch (failure) { error.value = (failure as Error).message; } })(), refreshCalibration(!libraryEntry)]);
  loading.value = false;
});
onBeforeUnmount(() => { calibrationRequest++; window.removeEventListener('local-workspace-opened', workspaceChanged); });
</script>
<template>
  <p v-if="error && selected" role="alert">{{ error }}</p>
  <LoadingAnimation v-if="loading || calibrationOpening" />
  <section v-else-if="trialError"><p role="alert">{{ trialError }}</p><a href="?">{{ uiText('返回阅读器') }}</a></section>
  <MusicCalibrationPane v-else-if="calibrationOpen && calibration" :context="calibration" :saving="calibrationSaving" :save-error="calibrationError" @save="saveCalibration" @close="calibrationOpen = false" />
  <VideoReaderPane v-else-if="selected" :key="`${selected.video.media_id}/${selected.video.media_revision}`" :initial="selected" owner="local" @close="selected = null" @ready="openVideo" />
  <template v-else>
    <p v-if="error" role="alert">{{ error }}</p>
    <p v-if="calibrationError" role="alert">{{ calibrationError }}</p>
    <nav v-if="calibration" :aria-label="uiText('本地音乐校准')"><button v-if="calibration.manifest.occurrences.length" @click="refreshCalibration(true)">{{ uiText('校准：') }}{{ calibration.manifest.work.title }}</button><p v-else>{{ uiText('这份音乐材料尚无小节候选。完成候选预构建后，可在此校准录音与谱面时间。') }}</p></nav>
    <nav v-if="videos.length" :aria-label="uiText('已导入视频')"><button v-for="video in videos" :key="video.video.media_id" @click="selected = video">{{ uiText('观看：') }}{{ video.title }}</button></nav>
    <App :course-trial="courseTrial" />
  </template>
</template>
