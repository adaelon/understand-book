<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { api as sharedApi, type AssetManifest, type SourcePopupView } from '../api';
import { bindSceneApi } from '../network-context';
import { renderSourceMarkdown } from '../md';
import MediaSourcePreview from './MediaSourcePreview.vue';
import { markMarkdownDomSourceRanges } from '../markdown-source-map';
import 'katex/dist/katex.min.css';

const props = defineProps<{ source: SourcePopupView }>();
const api = bindSceneApi(sharedApi);
const assets = ref<AssetManifest | null>(null);
const loadingImages = ref(false);
const text = computed(() => props.source.excerpt?.text ?? props.source.context_before + props.source.highlighted_quote + props.source.context_after);
const markup = computed(() => renderSourceMarkdown(text.value));
watch(() => props.source, async (source, _, onCleanup) => {
  assets.value = null;
  loadingImages.value = false;
  if (source.stale || !markup.value.includes('<img ')) return;
  let active = true;
  onCleanup(() => { active = false; });
  loadingImages.value = true;
  try {
    const query = source.source_url ? new URL(source.source_url, window.location.href).searchParams : null;
    const book_id = query?.get('book_id'), publication_id = query?.get('publication_id');
    const manifest = await api.assetManifest(book_id && publication_id ? { book_id, publication_id } : undefined);
    if (active) assets.value = manifest;
  } catch { /* The excerpt remains readable when the asset list is unavailable. */ }
  finally { if (active) loadingImages.value = false; }
}, { immediate: true });

function unavailableImage(img: HTMLImageElement, message: string) {
  const placeholder = document.createElement('span');
  placeholder.className = 'source-image-unavailable';
  placeholder.textContent = [img.alt, message].filter(Boolean).join(' · ');
  img.replaceWith(placeholder);
}
function imageFailed(event: Event) {
  if (event.target instanceof HTMLImageElement) unavailableImage(event.target, '图片暂不可用');
}
const rendered = computed(() => {
  const { source } = props;
  const highlight = source.excerpt?.highlight ?? {
    start: source.context_before.length,
    end: source.context_before.length + source.highlighted_quote.length,
  };
  const root = document.createElement('div');
  root.innerHTML = markup.value;
  markMarkdownDomSourceRanges(text.value, root, [{ ...highlight, className: 'source-highlight' }]);
  for (const img of root.querySelectorAll('img')) {
    const src = img.getAttribute('src') ?? '';
    const asset = assets.value?.images.find(image => image.original_src === src || encodeURI(image.original_src) === src);
    const url = asset?.url_path ?? (asset?.status === 'external' ? asset.original_src : null);
    if (url) img.setAttribute('src', url);
    else if (!/^(https?:|data:image\/)/i.test(src)) unavailableImage(img, loadingImages.value ? '正在加载图片…' : '图片暂不可用');
  }
  return root.innerHTML;
});
</script>

<template>
  <MediaSourcePreview v-if="source.media" :key="source.source_ref_id" :source="source" />
  <div v-else-if="source.stale" class="source-excerpt source-snapshot">{{ source.highlighted_quote }}</div>
  <div v-else class="source-excerpt" @error.capture="imageFailed" v-html="rendered"></div>
</template>

<style scoped>
.source-excerpt { color: var(--ink); font-size: 16px; line-height: 1.75; overflow-wrap: anywhere; white-space: normal; }
.source-snapshot { white-space: pre-wrap; }
.source-excerpt :deep(p) { margin: 0 0 .85em; }
.source-excerpt :deep(h1), .source-excerpt :deep(h2), .source-excerpt :deep(h3) { font-size: 1.12em; line-height: 1.4; margin: 1em 0 .5em; }
.source-excerpt :deep(ul), .source-excerpt :deep(ol) { padding-left: 1.5em; margin: .6em 0; }
.source-excerpt :deep(li) { margin: .25em 0; }
.source-excerpt :deep(pre) { overflow-x: auto; white-space: pre; padding: 12px; border-radius: 6px; background: var(--surface); font-size: .85em; line-height: 1.55; }
.source-excerpt :deep(code) { font-family: ui-monospace, Consolas, monospace; }
.source-excerpt :deep(table) { display: block; max-width: 100%; overflow-x: auto; overscroll-behavior-x: contain; border-collapse: collapse; font-size: .85em; line-height: 1.6; margin: 12px 0 18px; }
.source-excerpt :deep(td), .source-excerpt :deep(th) { min-width: 80px; padding: 9px 12px; border: 1px solid var(--line, #ddd); text-align: left; vertical-align: top; }
.source-excerpt :deep(th) { background: var(--surface-soft); font-weight: 600; }
.source-excerpt :deep(tr:nth-child(even)) { background: color-mix(in srgb, var(--surface-soft) 55%, transparent); }
.source-excerpt :deep(blockquote) { margin: .7em 0; padding-left: 12px; border-left: 3px solid var(--line); }
.source-excerpt :deep(.katex-display) { overflow-x: auto; overflow-y: hidden; padding: 4px 0; }
.source-excerpt :deep(mark) { color: inherit; background: rgba(255, 193, 7, .22); border-radius: 2px; }
.source-excerpt :deep(img) { display: block; max-width: 100%; height: auto; margin: 12px auto; border-radius: 8px; }
.source-excerpt :deep(.source-image-unavailable) { display: block; padding: 18px; border: 1px solid var(--line); border-radius: 8px; color: var(--steel); font-size: 13px; text-align: center; }
</style>
