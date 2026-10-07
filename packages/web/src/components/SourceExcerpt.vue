<script setup lang="ts">
import { computed } from 'vue';
import type { SourcePopupView } from '../api';
import { renderMarkdown } from '../md';
import { markMarkdownDomSourceRanges } from '../markdown-source-map';
import 'katex/dist/katex.min.css';

const props = defineProps<{ source: SourcePopupView }>();
const rendered = computed(() => {
  const { source } = props;
  const text = source.excerpt?.text ?? source.context_before + source.highlighted_quote + source.context_after;
  const highlight = source.excerpt?.highlight ?? {
    start: source.context_before.length,
    end: source.context_before.length + source.highlighted_quote.length,
  };
  const root = document.createElement('div');
  root.innerHTML = renderMarkdown(text);
  markMarkdownDomSourceRanges(text, root, [{ ...highlight, className: 'source-highlight' }]);
  return root.innerHTML;
});
</script>

<template>
  <div v-if="source.stale" class="source-excerpt source-snapshot">{{ source.highlighted_quote }}</div>
  <div v-else class="source-excerpt" v-html="rendered"></div>
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
.source-excerpt :deep(table) { display: block; max-width: 100%; overflow-x: auto; border-collapse: collapse; font-size: .9em; }
.source-excerpt :deep(td), .source-excerpt :deep(th) { padding: 6px 10px; border: 1px solid var(--line, #ddd); }
.source-excerpt :deep(blockquote) { margin: .7em 0; padding-left: 12px; border-left: 3px solid var(--line); }
.source-excerpt :deep(.katex-display) { overflow-x: auto; overflow-y: hidden; padding: 4px 0; }
.source-excerpt :deep(mark) { color: inherit; background: rgba(255, 193, 7, .22); border-radius: 2px; }
.source-excerpt :deep(img) { max-width: 100%; }
</style>
