<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef } from 'vue';
import { getDocument, GlobalWorkerOptions, TextLayer, type PDFDocumentProxy, type PDFPageProxy, type RenderTask } from 'pdfjs-dist/legacy/build/pdf.mjs';
import workerUrl from 'pdfjs-dist/legacy/build/pdf.worker.mjs?url';
import { extractPdfPageGeometry } from '../../../core/src/pdf-geometry';
import { sourceFromPdfGeometry } from '../../../core/src/pdf-native-source';
import { browserPdfOcr } from '../pdf-ocr-browser';
import { appendPdfOcrText } from '../pdf-ocr-text-layer';
import { localMaterials, type PdfAnnotation, type PdfPageText } from '../local-materials';
import { pdfRangeSource } from '../pdf-range-source';

GlobalWorkerOptions.workerSrc = workerUrl;
const props = defineProps<{ url: string; materialId: string }>();
const canvas = ref<HTMLCanvasElement | null>(null), layer = ref<HTMLDivElement | null>(null), host = ref<HTMLDivElement | null>(null);
const page = ref(1), count = ref(0), busy = ref(true), error = ref(''), zoom = ref(1), width = ref(0), height = ref(0);
const preparing = ref(false), preparation = ref(''), preparationFailed = ref(false), saving = ref(false), notice = ref(''), notesOpen = ref(false);
const annotations = ref<PdfAnnotation[]>([]), selection = shallowRef<{ quote: string; rects: PdfAnnotation['rects']; page_index: number } | null>(null);
const note = ref('');
const controller = new AbortController(), pages = new Map<number, PdfPageText>();
let pdf: PDFDocumentProxy | undefined, task: RenderTask | undefined, textLayer: TextLayer | undefined;
let viewport: ReturnType<PDFPageProxy['getViewport']> | undefined, renderedPage = -1, revision = 0;
let loading: ReturnType<typeof getDocument> | undefined, observer: ResizeObserver | undefined, resizeTimer: ReturnType<typeof setTimeout> | undefined;
const highlights = computed(() => annotations.value.filter(entry => entry.page_index === page.value - 1));
const colors = ['#f0c34e', '#72b99b', '#9da5dc'];
function message(reason: unknown) { return reason instanceof Error ? reason.message : String(reason); }
function overlay() {
  if (layer.value && viewport && renderedPage === page.value - 1) appendPdfOcrText(layer.value, pages.get(renderedPage)?.ocr_lines ?? [], viewport);
}
async function render() {
  if (!pdf || !canvas.value || !layer.value || !host.value) return;
  const ticket = ++revision, target = page.value - 1;
  busy.value = true; error.value = ''; selection.value = null;
  const previous = task; previous?.cancel(); textLayer?.cancel();
  try {
    await previous?.promise.catch(() => {});
    const source = await pdf.getPage(target + 1);
    if (controller.signal.aborted || ticket !== revision) return;
    const base = source.getViewport({ scale: 1 });
    const scale = Math.max(160, Math.min(host.value.clientWidth - 40, 1000)) / base.width * zoom.value;
    viewport = source.getViewport({ scale }); const ratio = Math.min(window.devicePixelRatio || 1, 2);
    width.value = viewport.width; height.value = viewport.height;
    canvas.value.width = Math.ceil(viewport.width * ratio); canvas.value.height = Math.ceil(viewport.height * ratio);
    layer.value.replaceChildren();
    layer.value.style.setProperty('--scale-factor', String(viewport.scale));
    layer.value.style.setProperty('--total-scale-factor', String(viewport.scale));
    task = source.render({ canvas: canvas.value, viewport, transform: [ratio, 0, 0, ratio, 0, 0] });
    await task.promise;
    if (controller.signal.aborted || ticket !== revision) return;
    textLayer = new TextLayer({ textContentSource: source.streamTextContent(), container: layer.value, viewport });
    await textLayer.render();
    if (controller.signal.aborted || ticket !== revision) return;
    renderedPage = target; overlay();
  } catch (reason) { if (!controller.signal.aborted && ticket === revision) error.value = message(reason); }
  finally { if (ticket === revision) busy.value = false; }
}
async function turn(target: number) {
  if (target < 1 || target > count.value) return;
  page.value = target;
  try { await localMaterials.pdfUpdate(props.materialId, { kind: 'position', page_index: target - 1 }); }
  catch (reason) { error.value = `阅读位置未保存：${message(reason)}`; }
  await render(); host.value?.scrollTo(0, 0);
}
function pickText() {
  const selected = window.getSelection();
  if (busy.value || !selected?.rangeCount || selected.isCollapsed || !viewport || !layer.value) return;
  const range = selected.getRangeAt(0);
  if (!layer.value.contains(range.startContainer) || !layer.value.contains(range.endContainer)) return;
  const quote = selected.toString().trim(); if (!quote) return;
  const bounds = layer.value.getBoundingClientRect(), rects: PdfAnnotation['rects'] = [];
  for (const rect of range.getClientRects()) {
    const left = Math.max(0, rect.left - bounds.left), top = Math.max(0, rect.top - bounds.top);
    const right = Math.min(bounds.width, rect.right - bounds.left), bottom = Math.min(bounds.height, rect.bottom - bounds.top);
    if (right - left < 1 || bottom - top < 1) continue;
    const a = viewport.convertToPdfPoint(left, top), b = viewport.convertToPdfPoint(right, bottom);
    const box: PdfAnnotation['rects'][number] = [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[0], b[0]), Math.max(a[1], b[1])];
    if (!rects.some(old => old.every((value, index) => Math.abs(value - box[index]) < 0.5))) rects.push(box);
  }
  if (rects.length) { selection.value = { quote, rects, page_index: page.value - 1 }; note.value = ''; notice.value = ''; }
}
async function save(color: string) {
  if (!selection.value || saving.value) return;
  saving.value = true; error.value = '';
  const annotation: PdfAnnotation = { ...selection.value, id: crypto.randomUUID(), color, note: note.value.trim() };
  try {
    await localMaterials.pdfUpdate(props.materialId, { kind: 'annotation', annotation });
    annotations.value.push(annotation); selection.value = null; window.getSelection()?.removeAllRanges(); notice.value = '划线已保存在本机';
  } catch (reason) { error.value = `划线未保存：${message(reason)}`; }
  finally { saving.value = false; }
}
async function remove(id: string) {
  try { await localMaterials.pdfUpdate(props.materialId, { kind: 'delete_annotation', id }); annotations.value = annotations.value.filter(entry => entry.id !== id); }
  catch (reason) { error.value = `删除未完成：${message(reason)}`; }
}
function rectangle(box: PdfAnnotation['rects'][number], color: string) {
  if (!width.value || !height.value || !viewport) return {};
  const a = viewport.convertToViewportPoint(box[0], box[1]), b = viewport.convertToViewportPoint(box[2], box[3]);
  return { left: `${Math.min(a[0], b[0])}px`, top: `${Math.min(a[1], b[1])}px`, width: `${Math.abs(b[0] - a[0])}px`, height: `${Math.abs(b[1] - a[1])}px`, background: color };
}
async function prepareText() {
  if (!pdf || preparing.value) return;
  preparing.value = true; preparationFailed.value = false;
  let firstFailure = '';
  const ocr = browserPdfOcr(controller.signal, value => { preparation.value = value; });
  try {
    for (let index = 0; index < pdf.numPages; index++) {
      controller.signal.throwIfAborted(); if (pages.has(index)) continue;
      preparation.value = `正在准备文字 ${index + 1} / ${pdf.numPages} 页`;
      let source: PDFPageProxy | undefined;
      try {
        source = await pdf.getPage(index + 1);
        const geometry = await extractPdfPageGeometry(source);
        await ocr.preparePage(source, geometry);
        const pageText: PdfPageText = { page_index: index, text: sourceFromPdfGeometry({ pages: [geometry] }).source, ocr_lines: ocr.results.find(result => result.pageIndex === index)?.lines ?? [] };
        await localMaterials.pdfUpdate(props.materialId, { kind: 'page', page: pageText });
        pages.set(index, pageText); if (renderedPage === index) overlay();
      } catch (reason) {
        if (controller.signal.aborted) throw reason;
        preparationFailed.value = true; firstFailure ||= `第 ${index + 1} 页：${message(reason)}`;
      } finally { if (index !== page.value - 1) source?.cleanup(); }
    }
    controller.signal.throwIfAborted();
    await localMaterials.pdfUpdate(props.materialId, { kind: 'source_complete' });
    preparation.value = preparationFailed.value ? `文字准备未完成（${firstFailure}），可继续阅读或重试。` : '正文已准备好 · 选中文字即可划线';
  } catch (reason) { if (!controller.signal.aborted) { preparationFailed.value = true; preparation.value = `文字准备未完成：${message(reason)}`; } }
  finally { await ocr.close(); preparing.value = false; }
}
onMounted(async () => {
  try {
    const [source, saved] = await Promise.all([pdfRangeSource(props.url, controller, reason => { error.value = reason.message; }), localMaterials.pdfReading(props.materialId)]);
    if (controller.signal.aborted) return;
    annotations.value = saved.annotations; for (const item of saved.pages) pages.set(item.page_index, item);
    loading = getDocument(source); pdf = await loading.promise; count.value = pdf.numPages;
    page.value = Math.min(count.value, saved.page_index + 1);
    await render(); if (controller.signal.aborted) return;
    void prepareText();
    let previousWidth = host.value?.clientWidth;
    observer = new ResizeObserver(entries => {
      const next = entries[0]?.contentRect.width;
      if (next === previousWidth) return; previousWidth = next;
      clearTimeout(resizeTimer); resizeTimer = setTimeout(() => { void render(); }, 100);
    });
    if (host.value) observer.observe(host.value);
  } catch (reason) { if (!controller.signal.aborted) error.value = message(reason); busy.value = false; }
});
onBeforeUnmount(() => { controller.abort(); observer?.disconnect(); clearTimeout(resizeTimer); task?.cancel(); textLayer?.cancel(); void loading?.destroy(); });
</script>
<template>
  <div class="local-pdf-reader">
    <nav aria-label="PDF 阅读工具">
      <div class="page-controls"><button :disabled="busy || page <= 1" @click="turn(page - 1)" aria-label="上一页">←</button><span>{{ page }} / {{ count }} 页</span><button :disabled="busy || page >= count" @click="turn(page + 1)" aria-label="下一页">→</button></div>
      <select v-model="zoom" aria-label="PDF 缩放" :disabled="busy" @change="render"><option :value="1">适合宽度</option><option :value="1.25">125%</option><option :value="1.5">150%</option></select>
      <button :aria-expanded="notesOpen" @click="notesOpen = !notesOpen">划线笔记 <span class="count">{{ annotations.length }}</span></button>
    </nav>
    <div class="reading-status" role="status"><span>{{ busy ? '正在打开 PDF…' : notice || preparation || '选中文字即可划线' }}</span><button v-if="preparationFailed && !preparing" @click="prepareText">重试文字识别</button></div>
    <p v-if="error" role="alert">{{ error }}</p>
    <div v-if="selection" class="selection-tools" aria-label="保存选中文字">
      <div><strong>划线</strong><span>{{ selection.quote }}</span></div>
      <input v-model="note" aria-label="划线笔记内容" placeholder="加一句笔记（选填）" @keydown.enter="save(colors[0])" />
      <div class="selection-actions"><button v-for="(color, index) in colors" :key="color" :disabled="saving" :style="{ '--mark-color': color }" class="mark-button" @pointerdown.prevent @click="save(color)">{{ index === 0 ? (saving ? '保存中…' : '保存划线') : index === 1 ? '绿色' : '紫色' }}</button><button @click="selection = null">取消</button></div>
    </div>
    <div class="reading-body">
      <div ref="host" class="pdf-scroll"><div class="pdf-page" :style="{ width: `${width}px`, height: `${height}px` }">
        <canvas ref="canvas" :style="{ width: `${width}px`, height: `${height}px` }" />
        <div class="saved-highlights" aria-hidden="true"><template v-for="entry in highlights" :key="entry.id"><i v-for="(rect, index) in entry.rects" :key="index" :style="rectangle(rect, entry.color)" /></template></div>
        <div ref="layer" class="textLayer" @pointerup="pickText" @keyup="pickText" />
      </div></div>
      <aside v-if="notesOpen" class="pdf-notes" aria-label="已保存的划线笔记"><header><strong>划线笔记</strong><span>保存在这台电脑</span></header>
        <p v-if="!annotations.length">选中原文后点「保存划线」。再次打开时，划线和笔记仍在。</p>
        <article v-for="entry in annotations" :key="entry.id" :style="{ borderLeftColor: entry.color }"><button class="quote" @click="turn(entry.page_index + 1)"><small>第 {{ entry.page_index + 1 }} 页</small>{{ entry.quote }}</button><p v-if="entry.note">{{ entry.note }}</p><button class="delete-note" @click="remove(entry.id)">删除划线</button></article>
      </aside>
    </div>
  </div>
</template>
<style scoped>
.textLayer{--min-font-size:1;--text-scale-factor:calc(var(--total-scale-factor) * var(--min-font-size));--min-font-size-inv:calc(1 / var(--min-font-size))}
.textLayer :deep(> :not(.markedContent)),.textLayer :deep(.markedContent span:not(.markedContent)){--font-height:0;font-size:calc(var(--text-scale-factor) * var(--font-height));--scale-x:1;--rotate:0deg;transform:rotate(var(--rotate)) scaleX(var(--scale-x)) scale(var(--min-font-size-inv))}
.local-pdf-reader{display:flex;flex-direction:column;min-height:0;flex:1;color:#302d27}.local-pdf-reader nav{display:flex;flex-wrap:wrap;align-items:center;justify-content:center;gap:.75rem;padding:.65rem;font-size:.85rem;border-bottom:1px solid #e4ddd3}.local-pdf-reader button,.local-pdf-reader select,.local-pdf-reader input{font:inherit;border:1px solid #d8cbbd;border-radius:7px;padding:.45rem .7rem;background:#fffdf8;color:inherit}.local-pdf-reader button{cursor:pointer}.local-pdf-reader button:disabled{opacity:.5;cursor:default}.page-controls{display:flex;align-items:center;gap:.75rem}.count{margin-left:.3rem;font-variant-numeric:tabular-nums}.reading-status{font-size:.76rem;color:#786c5f;display:flex;gap:.7rem;justify-content:center;align-items:center;padding:.4rem .75rem;min-height:1.2rem}.reading-status button{padding:.15rem .4rem}.reading-body{display:flex;min-height:0;flex:1}.pdf-scroll{overflow:auto;flex:1;min-width:0;padding:20px;background:#e9e4da}.pdf-page{position:relative;margin:0 auto;background:white;box-shadow:0 2px 10px #0002}.pdf-page canvas{display:block}.textLayer{position:absolute;inset:0;overflow:clip;line-height:1;text-align:initial;transform-origin:0 0;text-size-adjust:none;forced-color-adjust:none;z-index:2}.textLayer :deep(span),.textLayer :deep(br){color:transparent;position:absolute;white-space:pre;cursor:text;transform-origin:0 0}.textLayer :deep(span::selection){background:#87b0e080}.saved-highlights{position:absolute;inset:0;pointer-events:none;z-index:1;mix-blend-mode:multiply}.saved-highlights i{position:absolute;opacity:.4;border-radius:2px}.local-pdf-reader>p{padding:0 1rem;color:#a33825}.selection-tools{display:flex;flex-wrap:wrap;align-items:center;gap:.6rem;padding:.7rem 1rem;background:#fff8e7;border-bottom:1px solid #e5d6b7;font-size:.8rem}.selection-tools>div:first-child{display:flex;align-items:center;gap:.6rem;min-width:0;flex:1 1 200px}.selection-tools strong{white-space:nowrap}.selection-tools span{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:360px}.selection-tools input{min-width:130px;flex:1 1 160px}.selection-actions{display:flex;gap:.4rem}.selection-actions .mark-button{border-bottom:4px solid var(--mark-color)}.pdf-notes{width:280px;overflow:auto;padding:1rem;background:#fcfaf6;border-left:1px solid #dfd8ce;font-size:.85rem}.pdf-notes header span{display:block;color:#786c5f;font-size:.75rem;margin:.4rem 0 1.2rem}.pdf-notes>p{color:#786c5f;line-height:1.8}.pdf-notes article{border-left:3px solid;margin:1rem 0;padding:.5rem .75rem;background:#fffdf8;border-radius:0 7px 7px 0;overflow-wrap:anywhere}.pdf-notes .quote{display:block;border:0;padding:0;text-align:left;line-height:1.7;background:transparent;width:100%}.quote small{display:block;color:#786c5f;margin-bottom:.4rem}.pdf-notes article p{line-height:1.7;white-space:pre-wrap}.pdf-notes .delete-note{border:0;padding:.5rem 0 0;font-size:.75rem;color:#786c5f;background:transparent}@media(max-width:700px){.reading-body{flex-direction:column}.pdf-notes{box-sizing:border-box;width:100%;max-height:35%;border-left:0;border-top:1px solid #dfd8ce}.pdf-notes header{display:flex;justify-content:space-between}.pdf-notes header span{margin:0}.selection-tools span{max-width:200px}.pdf-scroll{padding:12px}.local-pdf-reader nav{gap:.5rem}.selection-tools{padding:.5rem .7rem}}
</style>
