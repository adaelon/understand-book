// Keep large libraries from starting one PDF worker per visible card at once.
let active = 0;
const waiting: Array<() => void> = [];
async function acquire() {
  if (active >= 2) await new Promise<void>(resolve => waiting.push(resolve));
  active++;
}
function release() { active--; waiting.shift()?.(); }

export async function renderPdfCover(url: string, canvas: HTMLCanvasElement, controller: AbortController): Promise<void> {
  await acquire();
  // PDF.js aborts its range transport during successful document cleanup too.
  // Keep that transport signal separate from the card's cancellation signal.
  const transport = new AbortController();
  let destroy: (() => Promise<void>) | undefined;
  const cancel = () => { transport.abort(); void destroy?.(); };
  try {
    if (controller.signal.aborted) return;
    const [pdf, worker, { pdfRangeSource }] = await Promise.all([
      import('pdfjs-dist/legacy/build/pdf.mjs'),
      import('pdfjs-dist/legacy/build/pdf.worker.mjs?url'),
      import('./pdf-range-source'),
    ]);
    if (controller.signal.aborted) return;
    pdf.GlobalWorkerOptions.workerSrc = worker.default;
    controller.signal.addEventListener('abort', cancel, { once: true });
    const source = await pdfRangeSource(url, transport, cancel);
    if (controller.signal.aborted) return;
    const task = pdf.getDocument(source);
    let destruction: Promise<void> | undefined;
    destroy = () => destruction ??= task.destroy();
    const document = await task.promise;
    const page = await document.getPage(1);
    if (controller.signal.aborted) return;
    const base = page.getViewport({ scale: 1 });
    const viewport = page.getViewport({ scale: Math.min(360 / base.width, 500 / base.height) });
    canvas.width = Math.ceil(viewport.width);
    canvas.height = Math.ceil(viewport.height);
    const context = canvas.getContext('2d');
    if (!context) throw new Error('封面画布不可用');
    await page.render({ canvas, canvasContext: context, viewport }).promise;
  } finally {
    controller.signal.removeEventListener('abort', cancel);
    try { await destroy?.(); } finally { transport.abort(); release(); }
  }
}
