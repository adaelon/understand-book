import type { PDFPageProxy } from 'pdfjs-dist/legacy/build/pdf.mjs';
import { pdfOcrWordSeparator, type PdfOcrLine } from '../../core/src/pdf-ocr';

/** Transparent text over OCR-only regions; it participates in the same native selection. */
export function appendPdfOcrText(container: HTMLElement, lines: PdfOcrLine[], viewport: ReturnType<PDFPageProxy['getViewport']>) {
  for (const old of container.querySelectorAll('[data-pdf-ocr]')) old.remove();
  const measure = document.createElement('canvas').getContext('2d');
  for (const line of lines) {
    for (const [index, word] of line.words.entries()) {
      const a = viewport.convertToViewportPoint(word.bbox[0], word.bbox[1]), b = viewport.convertToViewportPoint(word.bbox[2], word.bbox[3]);
      const left = Math.min(a[0], b[0]), top = Math.min(a[1], b[1]);
      const width = Math.abs(b[0] - a[0]), height = Math.abs(b[1] - a[1]);
      const span = document.createElement('span'); span.dataset.pdfOcr = 'true';
      span.textContent = word.text + (index < line.words.length - 1 ? pdfOcrWordSeparator(word.text, line.words[index + 1].text) : '');
      Object.assign(span.style, { position: 'absolute', left: `${left}px`, top: `${top}px`, fontSize: `${height}px`, fontFamily: 'sans-serif', lineHeight: '1', color: 'transparent', whiteSpace: 'pre', transformOrigin: '0 0' });
      if (measure) { measure.font = `${height}px sans-serif`; span.style.transform = `scaleX(${width / Math.max(1, measure.measureText(word.text).width)})`; }
      container.insertBefore(span, container.querySelector('.endOfContent'));
    }
    const br = document.createElement('br'); br.dataset.pdfOcr = 'true'; container.insertBefore(br, container.querySelector('.endOfContent'));
  }
}
