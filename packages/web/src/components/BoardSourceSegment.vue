<script setup lang="ts">
import { computed } from 'vue';
import type { AssetManifest, BoardSourceReadResponse } from '../api';
import { t as uiText } from '../i18n';
import { renderSourceMarkdown } from '../md';
import { markMarkdownDomSourceRanges } from '../markdown-source-map';
import 'katex/dist/katex.min.css';

const props = defineProps<{
  segment: BoardSourceReadResponse['segments'][number];
  assets: AssetManifest | null;
  loadingImages: boolean;
}>();

function unavailableImage(image: HTMLImageElement, message: string) {
  const placeholder = document.createElement('span');
  placeholder.className = 'source-image-unavailable';
  placeholder.textContent = [image.alt, message].filter(Boolean).join(' · ');
  image.replaceWith(placeholder);
}
function imageFailed(event: Event) {
  if (event.target instanceof HTMLImageElement) unavailableImage(event.target, uiText('图片暂不可用'));
}
const rendered = computed(() => {
  const root = document.createElement('div');
  root.innerHTML = renderSourceMarkdown(props.segment.text);
  markMarkdownDomSourceRanges(props.segment.text, root,
    props.segment.highlights.map(range => ({ ...range, className: 'source-highlight' })));
  for (const image of root.querySelectorAll('img')) {
    const src = image.getAttribute('src') ?? '';
    const asset = props.assets?.images.find(entry => entry.original_src === src || encodeURI(entry.original_src) === src);
    const url = asset?.url_path ?? (asset?.status === 'external' ? asset.original_src : null);
    if (url) image.setAttribute('src', url);
    else if (!/^(https?:|data:image\/)/i.test(src)) unavailableImage(image,
      props.loadingImages ? uiText('正在加载图片…') : uiText('图片暂不可用'));
  }
  return root.innerHTML;
});
</script>

<template>
  <article class="board-source-segment" :data-source-lid="segment.lid" :data-source-kind="segment.kind"
    :class="{ 'has-citation': segment.highlights.length > 0 }" @error.capture="imageFailed" v-html="rendered"></article>
</template>

<style scoped>
.board-source-segment{position:relative;min-width:0;color:var(--ink);font-family:var(--reader-font,var(--serif,serif));font-size:18px;line-height:1.95;overflow-wrap:anywhere;white-space:normal;scroll-margin-top:38px}
.board-source-segment.has-citation{padding-left:20px;margin-left:-22px;border-left:2px solid color-mix(in srgb,var(--brand-accent) 65%,transparent)}
.board-source-segment :deep(p){margin:0 0 1.1em}
.board-source-segment :deep(h1),.board-source-segment :deep(h2),.board-source-segment :deep(h3),.board-source-segment :deep(h4){font-family:var(--sans);line-height:1.55;margin:1.35em 0 .65em;font-weight:600}
.board-source-segment :deep(h1){font-size:1.6em}.board-source-segment :deep(h2){font-size:1.35em}.board-source-segment :deep(h3),.board-source-segment :deep(h4){font-size:1.15em}
.board-source-segment :deep(ul),.board-source-segment :deep(ol){padding-left:1.5em;margin:.6em 0 1em}.board-source-segment :deep(li){margin:.25em 0}
.board-source-segment :deep(pre){overflow-x:auto;white-space:pre;padding:16px;border-radius:8px;background:var(--surface-soft);font-size:.8em;line-height:1.65}
.board-source-segment :deep(code){font-family:ui-monospace,Consolas,monospace}
.board-source-segment :deep(table){display:block;max-width:100%;overflow-x:auto;overscroll-behavior-x:contain;border-collapse:collapse;font-size:.85em;line-height:1.7;margin:18px 0 24px}
.board-source-segment :deep(td),.board-source-segment :deep(th){min-width:80px;padding:10px 14px;border:1px solid var(--hairline);text-align:left;vertical-align:top}
.board-source-segment :deep(th){background:var(--surface-soft);font-weight:600}.board-source-segment :deep(tr:nth-child(even)){background:color-mix(in srgb,var(--surface-soft) 55%,transparent)}
.board-source-segment :deep(blockquote){margin:.8em 0 1.2em;padding:4px 0 4px 18px;border-left:3px solid var(--hairline);color:var(--slate)}
.board-source-segment :deep(.katex-display){overflow-x:auto;overflow-y:hidden;padding:6px 0}
.board-source-segment :deep(mark){color:inherit;background:color-mix(in srgb,#efc35c 33%,transparent);border-radius:3px;box-decoration-break:clone;-webkit-box-decoration-break:clone}
.board-source-segment :deep(img){display:block;max-width:100%;height:auto;margin:20px auto;border-radius:8px}
.board-source-segment :deep(.source-image-unavailable){display:block;padding:22px;border:1px solid var(--hairline);border-radius:8px;color:var(--steel);font-family:var(--sans);font-size:13px;text-align:center}
@media(max-width:700px){.board-source-segment{font-size:17px;line-height:1.9}.board-source-segment.has-citation{padding-left:12px;margin-left:-14px}}
</style>
