<script setup lang="ts">
import { computed, ref } from 'vue';
import ReaderPane, { type Segment } from './ReaderPane.vue';
import ReaderTypographyPanel from './ReaderTypographyPanel.vue';
import { useReaderTypography } from '../useReaderTypography';
import { renderInlineMarkdown, renderMarkdown, renderFormulaSource } from '../md';
import { network, installIdentity, readerPreferenceOwner, sceneKey } from '../network-context';
import type { MemoryRecord } from '../api';
import { textPositionTop, type ScrollAnchor } from '../reader-text-anchor';
installIdentity({ user_id: 'A', csrf_token: 'fixture' });
const typography = useReaderTypography(computed(readerPreferenceOwner));
const reader = ref<InstanceType<typeof ReaderPane> | null>(null);
const text = '阅读不是简单地收集结论，而是让问题、证据和推理发生联系。The reader keeps a precise position within this paragraph. 当字号、字体或版心改变时，同一句话仍应留在视线附近；中英混排、标点、**强调**、`code` 和 emoji 😀 都保持原文。';
const segments = ref<Segment[]>([
  { lid: '1', kind: 'chapter', text: '阅读、证据与理解', formula: null, imageAsset: null },
  { lid: '1.1', kind: 'paragraph', text: text.repeat(55), formula: null, imageAsset: null },
  { lid: '1.2', kind: 'formula', text: '$$E = mc^2 + \\sum_{i=1}^{n} x_i$$', formula: null, imageAsset: null },
  { lid: '1.3', kind: 'code', text: 'const original = "中文 English";\n' + 'long_identifier_'.repeat(30), formula: null, imageAsset: null },
  { lid: '1.4', kind: 'table', text: '| 方法 Method | 证据 Evidence |\n| --- | --- |\n| 阅读 | 原文与来源 |', formula: null, imageAsset: null },
  { lid: '1.5', kind: 'image', text: '![示例](data:image/svg+xml,example)', formula: null, imageAsset: null },
  { lid: '1.6', kind: 'paragraph', text: text.repeat(10), formula: null, imageAsset: null },
]);
const notes = ref<MemoryRecord[]>(location.search.includes('annotations') ? [{ mem_id: 'fixture-note', type: 'note', layer: 'long_term', book_id: 'fixture', anchor: { lid: '1.1' }, content: '长段的批注内容' }, { mem_id: 'fixture-later-note', type: 'note', layer: 'long_term', book_id: 'fixture', anchor: { lid: '1.1' }, content: '同段后面的引用', selection_context: { status: 'resolved', raw_quote: '阅读不是', resolved_quote: '阅读不是', ranges: [{ lid: '1.1', range: { start: text.length * 25, end: text.length * 25 + 4 } }] } }] : []);
function render(seg: Segment) { return seg.kind === 'formula' ? renderFormulaSource(seg.text) : seg.kind === 'code' || seg.kind === 'table' ? seg.text.replace(/&/g, '&amp;').replace(/</g, '&lt;') : renderInlineMarkdown(seg.text); }
const image = { status: 'external', original_src: 'data:image/svg+xml,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="600" height="160"><rect width="600" height="160" fill="#f4e4d4"/><circle cx="200" cy="80" r="55" fill="#cc785c"/><path d="M310 125L400 30L490 125Z" fill="#5db8a6"/></svg>'), alt: '几何示意', url_path: null } as unknown as NonNullable<Segment['imageAsset']>;
let saved: ScrollAnchor | null = null;
Object.assign(window, { reFixture: {
  capture: () => saved = reader.value!.captureScrollAnchor(segments.value.map(s => s.lid)),
  saved: () => saved,
  savedTop: () => {
    const root = document.querySelector<HTMLElement>(`[data-lid="${saved?.lid}"]`)!;
    return saved?.textPosition ? textPositionTop(root, segments.value.find(s => s.lid === saved!.lid)!.text, saved.textPosition)! - document.querySelector('.reader-pane')!.getBoundingClientRect().top : null;
  },
  update: typography.update,
  switchUser: (user: string | null) => installIdentity(user ? { user_id: user, csrf_token: 'fixture' } : null),
  changeScene: () => { network.value = { ...network.value, epoch: network.value.epoch + 1 }; },
  recycle: () => { segments.value = segments.value.filter(s => s.lid !== '1.1'); },
  navigate: () => reader.value!.scrollLidIntoView('1.6'),
} });
</script>
<template>
  <div class="re-fixture" :style="typography.style.value">
    <header><button @click="typography.open.value = true">阅读设置</button><span>连续阅读 · Reader 实际组件</span></header>
    <ReaderTypographyPanel v-if="typography.open.value" :preferences="typography.preferences.value" :save-error="typography.saveError.value" @change="typography.update" @close="typography.open.value = false" />
    <ReaderPane ref="reader" :context-key="sceneKey()" :typography="typography.preferences.value" :segments="segments" :viewport-anchor="null" :selected-lid="null" :render-seg="render" :render-markdown="renderMarkdown" :markdown-heading-level="() => null" :is-asset="s => ['code', 'table', 'image'].includes(s.kind)" :is-highlighted="() => false" :highlights-of="() => []" :highlight-cards-of="() => []" :visible-notes="notes" :hl-excerpt="() => ''" :image-meta="() => null" :image-asset="() => image" />
  </div>
</template>
<style scoped>
.re-fixture { height: 100%; display: flex; flex-direction: column; }
header { display: flex; padding: 12px 20px; align-items: center; gap: 20px; font: 14px var(--sans); }
.reader-pane { flex: 1; }
</style>
