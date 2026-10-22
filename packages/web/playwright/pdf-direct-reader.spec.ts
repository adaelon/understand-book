import { test, expect, type Page } from '@playwright/test';
import { pdfFixture } from '../../core/test/pdf-fixtures';

async function install(page: Page, pdf: Uint8Array) {
  await page.route('**/test-source.pdf', route => route.fulfill({ status: 200, contentType: 'application/pdf', body: Buffer.from(pdf) }));
  // Native disk persistence is covered by the Rust store test. This bridge survives reloads.
  await page.addInitScript(() => {
    (window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
      const saved = JSON.parse(localStorage.getItem('pdf-reading') || '{"annotations":[],"pages":[],"page_index":0}');
      if (command === 'desktop_pdf_reading') return saved;
      if (command !== 'desktop_pdf_update') throw new Error(command);
      const update = args.update;
      if (update.kind === 'page') saved.pages = [...saved.pages.filter((page: any) => page.page_index !== update.page.page_index), update.page];
      if (update.kind === 'annotation') saved.annotations.push(update.annotation);
      if (update.kind === 'delete_annotation') saved.annotations = saved.annotations.filter((entry: any) => entry.id !== update.id);
      if (update.kind === 'position') saved.page_index = update.page_index;
      localStorage.setItem('pdf-reading', JSON.stringify(saved));
    }};
  });
}
async function selectFirstLine(page: Page, selector: string) {
  const text = page.locator(selector).first(); await expect(text).toBeVisible();
  const box = (await text.boundingBox())!;
  await page.mouse.move(box.x + 1, box.y + box.height / 2); await page.mouse.down();
  await page.mouse.move(box.x + box.width - 1, box.y + box.height / 2, { steps: 12 }); await page.mouse.up();
  await expect(page.getByRole('button', { name: '保存划线', exact: true })).toBeVisible();
}

test('native PDF opens without OCR, saves highlights and restores them at desktop and narrow sizes', async ({ page }, info) => {
  const requests: string[] = []; page.on('request', request => requests.push(request.url()));
  const pdf = pdfFixture([{ lines: [
    { text: 'Reading starts with the original PDF', x: 50, y: 710, size: 24 },
    { text: 'Select a passage, highlight it and add a note.', x: 50, y: 655 },
    { text: 'No Markdown file or manual proofreading is required.', x: 50, y: 625 },
  ] }, { lines: [{ text: 'A second page keeps its own reading position.', x: 50, y: 710 }] }]);
  await install(page, pdf); await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto('/playwright/fixtures/pdf-direct-reader.html');
  await expect(page.getByRole('status')).toContainText('正文已准备好', { timeout: 30000 });
  expect(requests.filter(url => /traineddata|tesseract-core/.test(url))).toEqual([]);
  await selectFirstLine(page, '.textLayer span');
  await page.getByLabel('划线笔记内容').fill('先抓住原文的关键句。');
  await page.getByRole('button', { name: '保存划线', exact: true }).click();
  await page.getByRole('button', { name: /划线笔记/ }).click();
  await expect(page.getByLabel('已保存的划线笔记')).toContainText('先抓住原文的关键句');
  await expect(page.locator('.saved-highlights i')).toHaveCount(1);
  await expect.poll(async () => page.locator('.pdf-page').evaluate(element => element.clientWidth)).toBeLessThan(950);
  await expect(page.getByLabel('PDF 缩放')).toBeEnabled();
  const mark = (await page.locator('.saved-highlights i').boundingBox())!;
  const line = (await page.locator('.textLayer span').first().boundingBox())!;
  expect(Math.abs(mark.width - line.width)).toBeLessThan(5);
  await page.screenshot({ path: info.outputPath('native-desktop.png') });
  await page.getByLabel('PDF 缩放').selectOption('1.5');
  await expect(page.locator('.saved-highlights i')).toHaveCount(1);
  await page.getByLabel('下一页', { exact: true }).click();
  await page.reload(); await expect(page.getByRole('status')).toContainText('正文已准备好');
  await expect(page.locator('.page-controls')).toContainText('2 / 2');
  await page.getByLabel('上一页', { exact: true }).click();
  await expect(page.locator('.saved-highlights i')).toHaveCount(1);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole('button', { name: /划线笔记/ }).click();
  await expect(page.getByLabel('已保存的划线笔记')).toContainText('先抓住原文的关键句');
  await expect.poll(async () => page.locator('.pdf-page').evaluate(element => element.clientWidth)).toBeLessThan(390);
  await expect(page.getByLabel('PDF 缩放')).toBeEnabled();
  await page.screenshot({ path: info.outputPath('native-narrow.png') });
  await page.getByRole('button', { name: '删除划线' }).click();
  await expect(page.locator('.saved-highlights i')).toHaveCount(0);
});

test('scan OCR is local, selectable, persisted and reused after reopening', async ({ page }, info) => {
  test.setTimeout(120000);
  const requests: string[] = []; page.on('request', request => requests.push(request.url()));
  await install(page, pdfFixture([{ lines: [{ text: 'A PDF with native and scanned text', x: 50, y: 710, size: 22 }], scan: {
    lines: ['Scanned evidence remains selectable.', 'Keep existing text without duplicates.'], box: [50, 430, 562, 584],
  } }]));
  await page.goto('/playwright/fixtures/pdf-direct-reader.html');
  await expect(page.getByRole('status')).toContainText('正文已准备好', { timeout: 100000 });
  await expect(page.locator('.textLayer [data-pdf-ocr=true]').first()).toContainText('Scanned');
  await selectFirstLine(page, '.textLayer span[data-pdf-ocr]');
  await page.getByRole('button', { name: '保存划线', exact: true }).click();
  await page.screenshot({ path: info.outputPath('scan-highlight.png') });
  expect(requests.some(url => /eng.traineddata/.test(url))).toBe(true);
  expect(requests.every(url => url.startsWith('http://127.0.0.1:4174/'))).toBe(true);
  requests.length = 0;
  await page.reload(); await expect(page.getByRole('status')).toContainText('正文已准备好');
  await expect(page.locator('.saved-highlights i')).toHaveCount(1);
  expect(requests.some(url => /traineddata|tesseract-core/.test(url))).toBe(false);
});
