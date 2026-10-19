<script setup lang="ts">
import { onMounted, ref } from 'vue';
import App from './App.vue';
import LoadingAnimation from './components/LoadingAnimation.vue';
import VideoReaderPane from './components/VideoReaderPane.vue';
import { videoUrl, videoCatalog, type PreparingVideo } from './video-api';
const videos = ref<PreparingVideo[]>([]), selected = ref<PreparingVideo | null>(null), loading = ref(true), error = ref('');
async function openVideo() {
  if (!selected.value) return;
  try {
    const response = await fetch(videoUrl(selected.value.video, 'open'), { method: 'POST' });
    if (!response.ok) throw new Error((await response.json()).message || '无法打开视频发布');
    selected.value = null;
  } catch (failure) { error.value = (failure as Error).message; }
}
onMounted(async () => {
  try {
    const catalog = await videoCatalog(); videos.value = catalog.videos;
    selected.value = videos.value.find(v => v.video.media_id === catalog.active?.media_id && v.video.media_revision === catalog.active?.media_revision) ?? null;
  } catch (failure) { error.value = (failure as Error).message; }
  finally { loading.value = false; }
});
</script>
<template>
  <p v-if="error && selected" role="alert">{{ error }}</p>
  <LoadingAnimation v-if="loading" />
  <VideoReaderPane v-else-if="selected" :key="`${selected.video.media_id}/${selected.video.media_revision}`" :initial="selected" owner="local" @close="selected = null" @ready="openVideo" />
  <template v-else>
    <p v-if="error" role="alert">{{ error }}</p>
    <nav v-if="videos.length" aria-label="已导入视频"><button v-for="video in videos" :key="video.video.media_id" @click="selected = video">观看：{{ video.title }}</button></nav>
    <App />
  </template>
</template>
