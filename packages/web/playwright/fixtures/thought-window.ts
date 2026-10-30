import { createApp, h } from 'vue';
import BoardSourcePreview from '../../src/components/BoardSourcePreview.vue';
import ReaderWorkspace from '../../src/components/ReaderWorkspace.vue';
import { registerThoughtContext } from '../../src/personal-notes';

// Supply a known visible context for testing the source selector; App's actual reader registration has its own component test.
export function setReadingContext(label: string, bookId = 'learning', lid = '2.1') {
  registerThoughtContext(() => ({ book_id: bookId, publication_id: 'v1', lid, label }));
}

export function openReaderWorkspace() {
  const host = document.createElement('div');
  host.style.cssText = 'position:absolute;inset:0;background:var(--canvas);';
  document.querySelector('.application-content')!.append(host);
  createApp({ render: () => h('div', { class: 'app' }, [
    h('header', { class: 'topbar' }, '参数、输入与激活'),
    h(ReaderWorkspace, { logical: { contextKey: 'capture-test', revision: 1, activePreset: 'technical_read', openSlots: ['technical.structure_map', 'technical.agent'], focusedSlot: null } }, {
      default: () => [h('aside', { class: 'left-rail' }, '阅读目录'),
        h('article', { class: 'reader-pane' }, '正在看的原文：模型参数在推理过程中保持固定。'),
        h('aside', { class: 'right-rail' }, '正在看的解释：激活依赖当前输入。')],
    }),
  ]) }).mount(host);
}

// Exercise the actual source window beneath the floating note window.
export function openReadingWindow() {
  const host = document.createElement('div');
  document.querySelector('.application-content')!.append(host);
  const quote = '理解一个概念，需要能够用自己的话说明它与其他概念的关系。';
  const text = `## 从熟悉到理解\n\n${quote}\n\n阅读时产生的疑问和联想，可以先简短记下，继续核对眼前的材料。\n\n` +
    Array.from({ length: 16 }, (_, i) => `### ${i + 1}. 检查自己的解释\n\n熟悉一段表述，并不意味着已经能解释它。合上书，尝试举出一个例子，再回到原文检查例子成立的条件。固定的参数、变化的输入和随之产生的激活，需要分别说明。\n\n`).join('');
  const app = createApp(BoardSourcePreview, {
    source: { source_ref_id: 'reading-note-demo', label: '从熟悉到理解', material_title: '学习与理解',
      highlighted_quote: quote, context_before: '', context_after: '', stale: false, can_open_in_reader: false,
      excerpt: { text, highlight: { start: text.indexOf(quote), end: text.indexOf(quote) + quote.length } } },
    onClose: () => { app.unmount(); host.remove(); },
  });
  app.mount(host);
}
