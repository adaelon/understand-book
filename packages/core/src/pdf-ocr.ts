import type { Worker, Page as OcrPage } from "tesseract.js";
import type { PDFPageProxy } from "pdfjs-dist/legacy/build/pdf.mjs";
import type { PdfGeometryChar, PdfGeometryPage } from "./pdf-geometry";

export type PdfBox = [number, number, number, number];
export interface PdfOcrWord { text: string; bbox: PdfBox }
export interface PdfOcrLine { text: string; bbox: PdfBox; words: PdfOcrWord[] }
export interface PdfOcrResult { pageIndex: number; regions: PdfBox[]; lines: PdfOcrLine[]; error?: string }
export const pdfOcrWordSeparator = (before: string, after: string) => /\p{Script=Han}$/u.test(before) && /^\p{Script=Han}/u.test(after) ? "" : " ";

const area = (box: PdfBox) => Math.max(0, box[2] - box[0]) * Math.max(0, box[3] - box[1]);
const intersection = (a: PdfBox, b: PdfBox): PdfBox => [Math.max(a[0], b[0]), Math.max(a[1], b[1]), Math.min(a[2], b[2]), Math.min(a[3], b[3])];
const overlap = (a: PdfBox, b: PdfBox) => area(intersection(a, b));
const union = (boxes: PdfBox[]): PdfBox => [Math.min(...boxes.map(b => b[0])), Math.min(...boxes.map(b => b[1])), Math.max(...boxes.map(b => b[2])), Math.max(...boxes.map(b => b[3]))];

/** Only raster content without a usable text overlay needs recognition. */
export function missingPdfTextRegions(page: PdfGeometryPage): PdfBox[] {
  const text = page.chars.filter(char => char.text.trim());
  if (!text.length) return [[...page.view]];
  const candidates = (page.objects ?? []).filter(object => object.kind !== "form_xobject")
    .map(object => intersection(object.bbox, page.view))
    .filter(box => box[2] - box[0] >= 24 && box[3] - box[1] >= 12)
    .filter(box => {
      const covered = text.filter(char => overlap(box, char.bbox) > area(char.bbox) * 0.5);
      // A scan with a small page number is still a scan. A real overlay spans the image.
      return covered.length < 20 || area(union(covered.map(char => char.bbox))) < area(box) * 0.35;
    });
  const regions: PdfBox[] = [];
  for (const box of candidates.sort((a, b) => area(b) - area(a))) {
    if (!regions.some(kept => overlap(box, kept) >= area(box) * 0.95)) regions.push(box);
  }
  return regions;
}

export interface PdfOcrOptions {
  createWorker: () => Promise<Worker>;
  render: (page: PDFPageProxy, region: PdfBox, scale: number) => Promise<{
    image: Parameters<Worker["recognize"]>[0];
    toPdf: (bbox: PdfBox) => PdfBox;
    dispose: () => void;
  }>;
  signal?: AbortSignal;
  onProgress?: (message: string) => void;
}

/** One lazy worker per document, released on completion or cancellation. */
export function pdfOcrSession(options: PdfOcrOptions) {
  let worker: Promise<Worker> | undefined;
  let cancelled = false;
  let rejectAbort: (reason: unknown) => void;
  const aborted = new Promise<never>((_, reject) => { rejectAbort = reject; });
  // A terminated Tesseract worker does not settle its outstanding recognition call.
  void aborted.catch(() => {});
  const abort = () => {
    cancelled = true; rejectAbort(options.signal?.reason ?? new Error("PDF recognition cancelled"));
    void worker?.then(value => value.terminate()).catch(() => {});
  };
  options.signal?.addEventListener("abort", abort, { once: true });
  const results: PdfOcrResult[] = [];
  return {
    results,
    async preparePage(pdfPage: PDFPageProxy, page: PdfGeometryPage) {
      options.signal?.throwIfAborted();
      const regions = missingPdfTextRegions(page);
      if (!regions.length) return;
      const result: PdfOcrResult = { pageIndex: page.pageIndex, regions, lines: [] };
      results.push(result);
      for (const region of regions) {
        options.signal?.throwIfAborted();
        options.onProgress?.(`正在识别第 ${page.pageIndex + 1} 页的图片文字…`);
        const rendered = await options.render(pdfPage, region, Math.min(3, 2400 / Math.max(region[2] - region[0], region[3] - region[1])));
        try {
          worker ??= options.createWorker();
          const engine = await Promise.race([worker, aborted]);
          if (cancelled) { await engine.terminate(); options.signal?.throwIfAborted(); }
          const { data } = await Promise.race([engine.recognize(rendered.image, {}, { text: true, blocks: true }), aborted]);
          appendOcrText(page, data, rendered.toPdf, result.lines);
        } finally { rendered.dispose(); }
      }
    },
    async close() {
      options.signal?.removeEventListener("abort", abort);
      if (!cancelled) await worker?.then(value => value.terminate()).catch(() => {});
    },
  };
}

/** Native text owns its existing area. OCR only fills uncovered words. */
export function appendOcrText(page: PdfGeometryPage, result: OcrPage, toPdf: (box: PdfBox) => PdfBox, overlay: PdfOcrLine[]) {
  const existing = page.chars.filter(char => char.text.trim()).map(char => char.bbox);
  for (const block of result.blocks ?? []) for (const paragraph of block.paragraphs) for (const line of paragraph.lines) {
    let run: PdfOcrWord[] = [];
    const flush = () => {
      if (!run.length) return;
      const chars: PdfGeometryChar[] = [], words = run;
      for (const [index, word] of words.entries()) {
        if (index && pdfOcrWordSeparator(words[index - 1].text, word.text)) {
          const before = words[index - 1];
          chars.push({ pageIndex: page.pageIndex, charIndex: page.chars.length + chars.length, text: " ", origin: "ocr", bbox: [before.bbox[2], word.bbox[1], Math.max(before.bbox[2], word.bbox[0]), word.bbox[3]] });
        }
        const letters = Array.from(word.text), width = (word.bbox[2] - word.bbox[0]) / letters.length;
        for (const [offset, text] of letters.entries()) chars.push({ pageIndex: page.pageIndex, charIndex: page.chars.length + chars.length, text, origin: "ocr", bbox: [word.bbox[0] + offset * width, word.bbox[1], word.bbox[0] + (offset + 1) * width, word.bbox[3]] });
      }
      const text = chars.map(char => char.text).join(""), bbox = union(words.map(word => word.bbox));
      page.lines.push({ pageIndex: page.pageIndex, lineIndex: page.lines.length, text, bbox, char_start: page.chars.length, char_end: page.chars.length + chars.length });
      page.chars.push(...chars);
      overlay.push({ text, bbox, words });
      existing.push(...words.map(word => word.bbox));
      run = [];
    };
    for (const word of line.words) {
      const bbox = intersection(toPdf([word.bbox.x0, word.bbox.y0, word.bbox.x1, word.bbox.y1]), page.view);
      if (!word.text.trim() || area(bbox) <= 0 || existing.some(box => overlap(box, bbox) > Math.min(area(box), area(bbox)) * 0.5)) { flush(); continue; }
      run.push({ text: word.text.trim(), bbox });
    }
    flush();
  }
}
