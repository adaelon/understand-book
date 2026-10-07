<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { BookCoverSource } from '../api';
import { renderPdfCover } from '../pdf-cover';

const props = defineProps<{ title: string; cover?: BookCoverSource | null }>();
const root = ref<HTMLElement | null>(null), canvas = ref<HTMLCanvasElement | null>(null);
const visible = ref(false), loaded = ref(false), failed = ref(false);
let observer: IntersectionObserver | undefined;
onMounted(() => {
  if (typeof IntersectionObserver === 'undefined') { visible.value = true; return; }
  observer = new IntersectionObserver(entries => {
    if (entries.some(entry => entry.isIntersecting)) { visible.value = true; observer?.disconnect(); }
  }, { rootMargin: '120px' });
  if (root.value) observer.observe(root.value);
});
onBeforeUnmount(() => observer?.disconnect());
watch([visible, () => props.cover?.url, () => props.cover?.kind], async (_, __, cleanup) => {
  loaded.value = false; failed.value = false;
  if (!visible.value || props.cover?.kind !== 'pdf' || !canvas.value) return;
  const controller = new AbortController();
  cleanup(() => controller.abort());
  try {
    await renderPdfCover(props.cover.url, canvas.value, controller);
    if (!controller.signal.aborted) loaded.value = true;
  } catch {
    if (!controller.signal.aborted) failed.value = true;
  }
}, { flush: 'post' });
</script>

<template>
  <div ref="root" class="book-cover" :class="{ 'book-cover-loaded': loaded && !failed }" aria-hidden="true">
    <div v-if="!loaded || failed" class="book-cover-fallback">
      <div class="book-cover-rule"></div>
      <div class="book-cover-name">{{ title }}</div>
      <div class="book-cover-mark">UNDERSTAND BOOK</div>
    </div>
    <img v-if="visible && cover?.kind === 'image' && !failed" :key="cover.url" :src="cover.url" alt="" decoding="async"
      @load="loaded = true" @error="failed = true; loaded = false" />
    <canvas v-if="visible && cover?.kind === 'pdf'" ref="canvas" v-show="loaded && !failed"></canvas>
  </div>
</template>

<style scoped>
.book-cover { position: relative; display: grid; place-items: center; width: 100%; aspect-ratio: 2 / 3; overflow: hidden; border-radius: 3px 7px 7px 3px; background: #eee9de; box-shadow: 0 3px 10px #1d302314; }
.book-cover-fallback { position: absolute; inset: 0; display: flex; flex-direction: column; gap: 14px; padding: 20px 16px 16px 20px; background: var(--cover-color, #e2e9e1); border-left: 5px solid #0000000c; color: #2f483b; }
.book-cover-rule { width: 24px; height: 3px; background: currentColor; opacity: .55; flex-shrink: 0; }
.book-cover-name { font-family: var(--serif, Georgia, serif); font-size: clamp(15px, 1.6vw, 21px); line-height: 1.45; font-weight: 650; overflow: hidden; overflow-wrap: anywhere; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 5; }
.book-cover-mark { margin-top: auto; font: 8px/1.4 system-ui, sans-serif; letter-spacing: .1em; opacity: .65; }
.book-cover img, .book-cover canvas { display: block; width: 100%; height: 100%; object-fit: contain; }
.book-cover img { position: absolute; inset: 0; }
.book-cover-loaded { background: #fff; }
</style>
