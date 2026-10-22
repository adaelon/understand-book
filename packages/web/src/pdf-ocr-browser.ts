import { pdfOcrSession, type PdfBox } from '../../core/src/pdf-ocr';

export function browserPdfOcr(signal: AbortSignal, onProgress: (message: string) => void) {
  return pdfOcrSession({
    signal, onProgress,
    async createWorker() { return (await import('./pdf-ocr-worker')).createPdfOcrWorker(); },
    async render(page, region, scale) {
      const viewport = page.getViewport({ scale });
      const a = viewport.convertToViewportPoint(region[0], region[1]), b = viewport.convertToViewportPoint(region[2], region[3]);
      const left = Math.min(a[0], b[0]), top = Math.min(a[1], b[1]);
      const canvas = document.createElement('canvas');
      canvas.width = Math.max(1, Math.ceil(Math.abs(a[0] - b[0]))); canvas.height = Math.max(1, Math.ceil(Math.abs(a[1] - b[1])));
      const task = page.render({ canvas, viewport, transform: [1, 0, 0, 1, -left, -top] });
      const abort = () => task.cancel(); signal.addEventListener('abort', abort, { once: true });
      try { await task.promise; } finally { signal.removeEventListener('abort', abort); }
      return { image: canvas,
        toPdf(box: PdfBox): PdfBox {
          const a = viewport.convertToPdfPoint(box[0] + left, box[1] + top), b = viewport.convertToPdfPoint(box[2] + left, box[3] + top);
          return [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[0], b[0]), Math.max(a[1], b[1])];
        },
        dispose() { canvas.width = canvas.height = 1; },
      };
    },
  });
}
