import { createApp, h, shallowRef, nextTick } from 'vue';
import FocusReaderSurface from '../../src/components/FocusReaderSurface.vue';
import { ReaderPagination, type PaginationLayout, type PreparedScreen, type SourceCursor } from '../../src/reader-pagination';
import { renderPaginationSource, type PaginationRenderSource } from '../../src/reader-pagination-render';
import { projectMarkdownDom } from '../../src/markdown-source-map';
import { FOCUS_READING_BUDGET } from '../../src/reader-pagination-budget';
import 'katex/dist/katex.min.css';
import '../../src/style.css';
import './focus-pagination.css';

let sources: PaginationRenderSource[] = [];
let engine: ReaderPagination;
let starts: SourceCursor[] = [], position = 0;
let layout: PaginationLayout;
const version = shallowRef(0);
const longTasks: { start: number; duration: number }[] = [];
new PerformanceObserver(list => longTasks.push(...list.getEntries().map(e => ({ start: e.startTime, duration: e.duration })))).observe({ type: 'longtask', buffered: true });
const ready = async (root: HTMLElement, source: PaginationRenderSource, layout: PaginationLayout) => {
  await Promise.all([400, 600].map(weight => document.fonts.load(`${weight} ${layout.fontSize}px ${layout.fontFamily}`, source.text)));
  await Promise.all(Array.from(root.querySelectorAll('img')).map(img => img.decode().catch(error => { throw new Error(`Image ${source.lid}: ${img.src}: ${error}`); })));
};
createApp({ setup: () => () => h('main', { class: 'fp2' }, [
  h('header', 'FP2 · bounded source pagination'),
  h(FocusReaderSurface, { key: version.value, read: async (index: number) => sources[index] ?? null, render: renderPaginationSource, ready, onReady: (value: ReaderPagination) => { engine = value; } }),
]) }).mount('#app');
function geometry(fontSize = 19): PaginationLayout {
  return { width: innerWidth - (innerWidth <= 600 ? 32 : 64), height: innerHeight - 110, fontSize, mobile: innerWidth <= 600, fontFamily: '"Noto Serif SC Variable"', lineHeight: 1.85 };
}
function summary(screen: PreparedScreen) {
  return { id: screen.id, start: screen.start, end: screen.end, eof: screen.eof, pages: screen.pages.map(page => page.fragments.map(({ index, lid, start, end, semanticStart, semanticEnd }) => ({ index, lid, start, end, semanticStart, semanticEnd }))) };
}
function audit() {
  const screen = engine.visible()!;
  return screen.pages.flatMap(page => page.fragments.map(fragment => {
    const expected = ['table', 'code', 'image'].includes(sources[fragment.index].kind) || fragment.element.classList.contains('formula-display') ? null : projectMarkdownDom(renderPaginationSource(sources[fragment.index])).text.slice(fragment.semanticStart, fragment.semanticEnd);
    const actual = expected === null ? null : projectMarkdownDom(fragment.element).text;
    return { lid: fragment.lid, start: fragment.start, end: fragment.end, expected, actual };
  }));
}
async function open(material: string, chunk: string | number = 'all', font = 19) {
  const chunks = ['all', 'whitespace'].includes(String(chunk)) ? Array.from({ length: 10 }, (_, i) => i) : [chunk];
  sources = (await Promise.all(chunks.map(c => fetch(`/fp1/${material}/${c}.json`).then(r => r.json())))).flat();
  if (chunk === 'whitespace') sources = sources.filter(s => ['1.5.2', '1.5.3'].includes(s.lid));
  if (material === 'mixed' && chunk === 'stress') sources.push(
    { lid: 'stress.long', kind: 'paragraph', text: ('中文长段 **strong** and English $x^2$，保留原文。').repeat(160) },
    { lid: 'stress.list', kind: 'paragraph', text: Array.from({ length: 60 }, (_, i) => `${i + 1}. 第 ${i} 项：**保留列表**与 $y_i$ 公式。`).join('\n') },
    { lid: 'stress.code', kind: 'code', text: Array.from({ length: 100 }, (_, i) => `line ${i}: ` + 'wide_identifier_'.repeat(18)).join('\n') },
  );
  version.value++; await nextTick();
  layout = geometry(font); starts = [{ index: 0, offset: 0 }]; position = 0;
  const screen = await engine.prepare(starts[0], layout); engine.show(screen);
  return { screen: summary(screen), sources: sources.map(s => ({ lid: s.lid, length: s.text.length })), resources: engine.resources() };
}
async function turn(direction: number) {
  const current = engine.visible()!;
  let cursor: SourceCursor;
  if (direction > 0) {
    if (current.eof) return null;
    cursor = current.end; starts[++position] = { ...cursor };
  } else {
    if (!position) return null;
    cursor = starts[--position];
  }
  let target = engine.resident().find(s => s.start.index === cursor.index && s.start.offset === cursor.offset);
  const before = JSON.stringify(summary(current));
  if (!target) target = await engine.prepare(cursor, layout);
  if (before !== JSON.stringify(summary(current))) throw new Error('Preparation changed the visible boundary');
  const workBefore = { ...engine.metrics };
  engine.show(target);
  if (workBefore.alignments !== engine.metrics.alignments || workBefore.preparations !== engine.metrics.preparations || workBefore.geometryProbes !== engine.metrics.geometryProbes) throw new Error('Show performed preparation work');
  return { screen: summary(target), resources: engine.resources() };
}
async function reflow(font: number, cursor?: SourceCursor) {
  layout = geometry(font); const anchor = cursor ?? engine.anchor()!;
  const old = engine.visible()!;
  const at = performance.now();
  const work = engine.reflow(layout, anchor);
  const oldStillVisible = !old.element.hidden && old.element.isConnected;
  const result = await work;
  const readyMs = performance.now() - at;
  starts = [{ ...anchor }]; position = 0;
  return { readyMs, oldStillVisible, anchor, screen: summary(result!.screen), resources: engine.resources() };
}
Object.assign(window, { fp2: { open, turn, reflow, audit, summary: () => summary(engine.visible()!), resources: () => engine.resources(), report: () => ({ longTasks, metrics: engine.metrics, peak: engine.peak }), budget: FOCUS_READING_BUDGET,
  engine: () => engine,
  async warm() { await Promise.all([400, 600].map(w => document.fonts.load(`${w} 22px "Noto Serif SC Variable"`, sources.map(s => s.text).join('')))); },
} });
