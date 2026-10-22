import { describe, expect, it, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { buildPdfNativeFoundation, extractPdfWithOcr } from '../src/pdf-native-foundation';
import { extractPdfTextGeometry } from '../src/pdf-geometry';
import { pdfOcrSession } from '../src/pdf-ocr';
import { pdfFixture } from './pdf-fixtures';

describe('PDF text-first source', () => {
  it('binds repeated text directly to its own page and preserves literal Markdown symbols', async () => {
    const pdf = pdfFixture([0, 1].map(() => ({ lines: [
      { text: 'Repeated paragraph on different pages.', x: 50, y: 700 },
      { text: '1. f(x) = x^2 + 1 | [original] * literal', x: 50, y: 650 },
    ] })));
    const createWorker = vi.fn(); const geometry = await extractPdfTextGeometry(pdf);
    const ocr = pdfOcrSession({ createWorker, render: vi.fn() });
    for (const page of geometry.pages) await ocr.preparePage({} as any, page);
    await ocr.close(); expect(createWorker).not.toHaveBeenCalled();
    const { source, artifacts } = buildPdfNativeFoundation({ pdf, geometry, book_id: 'direct', original_pdf_path: 'original.pdf' });
    for (const page of artifacts.pdf_selection_map_pages) {
      expect(page.chars.filter(char => char.text.trim()).length).toBeGreaterThan(50);
      for (const char of page.chars) {
        expect(char.rect.pageIndex).toBe(page.pageIndex);
        expect(source.slice(char.source_span!.start, char.source_span!.end).replace(/\\(.)/gu, '$1')).toBe(char.text);
      }
    }
    const firstEnd = Math.max(...artifacts.pdf_selection_map_pages[0].chars.map(char => char.source_span!.end));
    expect(artifacts.pdf_selection_map_pages[1].chars.every(char => char.source_span!.start > firstEnd)).toBe(true);
    expect(artifacts.source_manifest.canonical_source.kind).toBe('pdf_text');
  });

  it('reads two columns in column order', async () => {
    const pdf = pdfFixture([{ lines: [
      { text: 'Left first.', x: 50, y: 700 }, { text: 'Right first.', x: 340, y: 700 },
      { text: 'Left second.', x: 50, y: 670 }, { text: 'Right second.', x: 340, y: 670 },
    ] }]);
    const { source, artifacts } = buildPdfNativeFoundation({ pdf, geometry: await extractPdfTextGeometry(pdf), book_id: 'columns', original_pdf_path: 'original.pdf' });
    expect(source.indexOf('Left second')).toBeLessThan(source.indexOf('Right first'));
    const bySelectionOrder = [...artifacts.pdf_selection_map_pages[0].chars].sort((a, b) => a.char_index - b.char_index);
    expect(bySelectionOrder.map(char => char.text).join('')).toBe('Left first.Left second.Right first.Right second.');
  });

  it('uses actual OCR on a scan and a missing region while keeping the native text once', async () => {
    const pdf = pdfFixture([
      { scan: { lines: ['Scanned evidence remains selectable.', 'Read the original page and take notes.'], box: [50, 430, 562, 584] } },
      { lines: [{ text: 'Native heading', x: 50, y: 710, size: 20 }], scan: { lines: ['Missing image text is recovered.', 'Keep existing text without duplicates.'], box: [50, 430, 562, 584] } },
      { lines: [{ text: 'Scanned evidence remains selectable.', x: 62.8, y: 551.9, size: 19.63 }], scan: { lines: ['Scanned evidence remains selectable.', 'Only missing words are added.'], box: [50, 430, 562, 584] } },
      { scan: { lines: [], jpeg: readFileSync(new URL('./fixtures/pdf-native-chinese.jpg', import.meta.url)), box: [50, 430, 562, 584] } },
    ]);
    const extracted = await extractPdfWithOcr(pdf);
    const { source, artifacts } = buildPdfNativeFoundation({ pdf, ...extracted, book_id: 'scan', original_pdf_path: 'original.pdf' });
    expect(source).toContain('Scanned evidence remains selectable');
    expect(source).toContain('Missing image text is recovered');
    expect(source.match(/Native heading/g)).toHaveLength(1);
    expect(source.match(/Scanned evidence remains selectable/g)).toHaveLength(2);
    expect(source).toContain('中文文字识别');
    expect(source).toContain('阅读原文并保存笔记');
    expect(artifacts.pdf_source_map.pages.every(page => page.ocr_lines!.length > 0)).toBe(true);
    expect(artifacts.pdf_selection_map_pages.every(page => page.chars.length > 10)).toBe(true);
  }, 90_000);

  it('settles cancellation even when a terminated OCR worker leaves recognition pending', async () => {
    const pdf = pdfFixture([{}]), geometry = await extractPdfTextGeometry(pdf), controller = new AbortController();
    let started!: () => void; const recognizing = new Promise<void>(resolve => { started = resolve; });
    const terminate = vi.fn().mockResolvedValue(undefined), dispose = vi.fn();
    const ocr = pdfOcrSession({ signal: controller.signal,
      createWorker: async () => ({ recognize: () => { started(); return new Promise(() => {}); }, terminate }) as any,
      render: async () => ({ image: Buffer.alloc(0), toPdf: value => value, dispose }),
    });
    const pending = ocr.preparePage({} as any, geometry.pages[0]);
    await recognizing; controller.abort(); await expect(pending).rejects.toThrow(); await ocr.close();
    expect(terminate).toHaveBeenCalled(); expect(dispose).toHaveBeenCalled();
  });
});
