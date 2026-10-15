import { test } from './focus-test';
import { expect, type Page } from '@playwright/test';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { backend } from './focus-app-backend';
import { startFrameTrace, pacedReaderClick } from './focus-reading-trace';

const output = path.resolve(process.env.FOCUS_EVIDENCE_DIR ?? '../../docs/performance/focus-reading-fp6-20261015/results');
const snapshot = (page: Page) => page.evaluate(() => (window as any).fp6.snapshot());
const save = async (name: string, data: unknown) => { await mkdir(output, { recursive: true }); await writeFile(path.join(output, `${name}.json`), JSON.stringify(data, null, 2)); };
async function open(page: Page, weak = false, fixture = false) {
  const server = await backend(page, 'mixed');
  const material = path.resolve('../../tmp/focus-reading-fp1/public/fp1');
  let [map, manifest, bytes] = await Promise.all([readFile(path.join(material, 'pdf-map.json'), 'utf8').then(JSON.parse),
    readFile(path.join(material, 'pdf-manifest.json'), 'utf8').then(JSON.parse), readFile(path.join(material, 'paper.pdf'))]);
  const pdfControl = { offline: false, weak };
  const selections: any[] = [];
  if (fixture) {
    bytes = columnPdf();
    map = { ...map, pages: Array.from({ length: 7 }, (_, pageIndex) => ({ pageIndex, width: 612, height: 792, rotate: 0, view: [0, 0, 612, 792] })), entries: [] };
    await page.route('**/api/reader/pdf_selection.resolve', route => {
      const body = route.request().postDataJSON(); selections.push(body);
      return route.fulfill({ json: { status: 'resolved', quote_markdown: 'Selectable', ranges: [{ lid: server.sources[0].lid, range: { start: 0, end: 10 }, source_span: { start: 0, end: 10 }, quote_markdown: 'Selectable' }] } });
    });
    await page.route('**/api/reader/pdf_ranges.project', route => {
      const ranges = route.request().postDataJSON().ranges;
      return route.fulfill({ json: { projections: ranges.map((r: any) => ({ ...r, status: 'exact', covered_range: r.range,
        rects: [{ pageIndex: 0, bbox: [72, 696, 164, 716], source_span: r.range }], terminal_rect: { pageIndex: 0, bbox: [156, 696, 164, 716], source_span: { start: 9, end: 10 } } })) } });
    });
  }
  await page.route('**/api/book/source_manifest', route => route.fulfill({ json: manifest }));
  await page.route('**/api/book/pdf_source_map', route => route.fulfill({ json: map }));
  await page.route('**/api/book/pdf/original', async route => {
    if (pdfControl.offline) return route.abort();
    const range = route.request().headers().range?.match(/bytes=(\d+)-(\d*)/);
    const start = range ? Number(range[1]) : 0, end = range?.[2] ? Number(range[2]) : bytes.length - 1;
    const body = bytes.subarray(start, Math.min(bytes.length, end + 1));
    if (pdfControl.weak) await new Promise(r => setTimeout(r, 100 + body.length / 1250));
    await route.fulfill({ status: range ? 206 : 200, contentType: 'application/pdf', body,
      headers: { 'accept-ranges': 'bytes', ...(range ? { 'content-range': `bytes ${start}-${start + body.length - 1}/${bytes.length}` } : {}) } });
  });
  await page.goto(process.env.FOCUS_ENTRY_URL ?? '/playwright/fixtures/focus-pdf.html');
  await expect(page.locator('.pdf-page-list .pdf-text-layer span').first()).toBeAttached();
  await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().click();
  await settled(page); await page.waitForTimeout(1000);
  return { ...server, pdfControl, map, selections };
}
function columnPdf() {
  const objects = ['<< /Type /Catalog /Pages 2 0 R >>', `<< /Type /Pages /Kids [${Array.from({ length: 7 }, (_, i) => `${4 + i * 2} 0 R`).join(' ')}] /Count 7 >>`,
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'];
  for (let i = 0; i < 7; i++) {
    const stream = `BT /F1 20 Tf 72 700 Td (Selectable PDF ${i + 1}) Tj 0 -36 Td (Left Formula) Tj 0 -36 Td (Alpha beta gamma) Tj ET\nBT /F1 18 Tf 330 700 Td (Right Formula) Tj 0 -36 Td (Lambda mu) Tj ET\n0.2 0.5 0.8 rg 72 100 468 400 re f`;
    objects.push(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents ${5 + i * 2} 0 R >>`,
      `<< /Length ${Buffer.byteLength(stream)} >>\nstream\n${stream}\nendstream`);
  }
  let body = '%PDF-1.4\n'; const offsets = [0];
  for (const [i, obj] of objects.entries()) { offsets.push(Buffer.byteLength(body)); body += `${i + 1} 0 obj\n${obj}\nendobj\n`; }
  const xref = Buffer.byteLength(body);
  body += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n${offsets.slice(1).map(n => `${String(n).padStart(10, '0')} 00000 n \n`).join('')}trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(body);
}
async function settled(page: Page) {
  await expect.poll(async () => {
    const s = await snapshot(page); return !!s?.current && !s.waiting && s.turn.phase === 'idle' && !s.error;
  }, { timeout: 30_000 }).toBe(true);
}
function budget(s: any, width: number) {
  expect(s.resources.pages).toBeLessThanOrEqual(width <= 600 ? 3 : 6);
  expect(s.resources.backingPixels).toBeLessThanOrEqual(12_000_000);
  expect(s.peak.backingPixels).toBeLessThanOrEqual(12_000_000);
  expect(s.peak.preparationTasks).toBeLessThanOrEqual(1);
}
async function tool(page: Page) {
  await page.locator('.pdf-reader-pane .focus-tools-toggle').click();
  await expect(page.getByRole('navigation', { name: '专注阅读工具', exact: true })).toBeVisible();
}

test('real PDF spread ready, native cross-page selection, bounds and rapid reversal', async ({ page }) => {
  await open(page);
  const before = await snapshot(page); expect(before.current.pages).toHaveLength(2); budget(before, 1440);
  await expect(page.locator('.pdf-focus-screen:not([hidden]) .pdf-page-canvas')).toHaveCount(2);
  for (const shell of await page.locator('.pdf-focus-screen:not([hidden]) .pdf-page-shell').all()) {
    expect(await shell.locator('.pdf-text-layer span').count()).toBeGreaterThan(0);
  }
  await page.locator('.pdf-focus-screen:not([hidden])').evaluate(el => {
    const pages = el.querySelectorAll('.pdf-text-layer');
    const a = pages[0].querySelector('span')!.firstChild!, b = pages[1].querySelector('span')!.firstChild!;
    const range = document.createRange(); range.setStart(a, 0); range.setEnd(b, Math.min(5, b.textContent!.length));
    const sel = window.getSelection()!; sel.removeAllRanges(); sel.addRange(range);
  });
  const quote = await page.evaluate(() => window.getSelection()!.toString()); expect(quote.length).toBeGreaterThan(30);
  await page.keyboard.press('PageDown'); expect((await snapshot(page)).current.id).toBe(before.current.id);
  await page.evaluate(() => window.getSelection()?.removeAllRanges());
  await page.getByRole('button', { name: '下一屏', exact: true }).click(); await settled(page);
  expect((await snapshot(page)).current.start.index).toBe(before.current.start.index + 2);
  for (let i = 0; i < 40; i++) {
    await page.getByRole('button', { name: i % 2 ? '下一屏' : '上一屏', exact: true }).click(); await page.waitForTimeout(250);
  }
  await settled(page); budget(await snapshot(page), 1440);
  await mkdir(output, { recursive: true }); await page.screenshot({ path: path.join(output, 'desktop.png') });
  await save('functional-desktop', { before, quote, final: await snapshot(page) });
});

test('PDF zoom, pan, tools, responsive and exit preserve original page', async ({ page }) => {
  await open(page);
  await page.getByRole('button', { name: '下一屏', exact: true }).click(); await settled(page);
  const before = await page.evaluate(() => (window as any).fp6.anchor());
  await tool(page); await page.getByLabel('PDF 缩放倍率（相对整页）').selectOption('2'); await settled(page);
  expect((await snapshot(page)).current.geometry.zoom).toBe(2);
  await page.locator('.pdf-reader-pane .focus-tools-toggle').click();
  await page.locator('.pdf-focus-screen:not([hidden])').evaluate(el => { el.scrollTop = 100; el.scrollLeft = 120; });
  await page.screenshot({ path: path.join(output, 'zoom-desktop.png') });
  const zoomed = await page.evaluate(() => (window as any).fp6.anchor());
  await tool(page); await page.getByRole('navigation', { name: '专注阅读工具', exact: true }).getByRole('button', { name: '退出专注', exact: true }).click();
  await expect(page.locator('.pdf-focus-host')).toBeHidden();
  const continuous = await page.evaluate(() => (window as any).fp6.anchor());
  expect(continuous.pageIndex).toBe(zoomed.pageIndex); expect(continuous.zoom).toBe(2);
  expect(Math.abs(continuous.pageRatio - zoomed.pageRatio)).toBeLessThan(.03);
  await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().click(); await settled(page);
  expect((await snapshot(page)).current.pages).toContain(before.pageIndex);
  await tool(page); await page.getByRole('button', { name: '适合整页', exact: true }).click(); await settled(page);
  await page.locator('.pdf-reader-pane .focus-tools-toggle').click();
  for (const [width, height] of [[1024, 900], [390, 844], [844, 390], [320, 844], [1440, 900]]) {
    await page.setViewportSize({ width, height }); await settled(page); await page.waitForTimeout(300);
    const s = await snapshot(page); expect(s.current.pages).toContain(before.pageIndex); budget(s, width);
    expect(s.current.geometry.columns).toBe(width >= 1024 ? 2 : 1);
  }
  await save('zoom-responsive', { before, zoomed, continuous, final: await snapshot(page) });
});

test('two-column selection, highlight projection, odd last page and surface return', async ({ page }) => {
  const server = await open(page, false, true);
  const current = '.pdf-focus-screen:not([hidden]):not([aria-hidden="true"])';
  const select = async () => {
    const rect = await page.locator(`${current} .pdf-text-layer span`).filter({ hasText: /^Selectable PDF 1$/ }).evaluate(el => {
      const r = document.createRange(); r.setStart(el.firstChild!, 0); r.setEnd(el.firstChild!, 10);
      const rect = r.getBoundingClientRect(); return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    });
    await page.mouse.move(rect.x + .5, rect.y + rect.height / 2); await page.mouse.down();
    await page.mouse.move(rect.x + rect.width + .5, rect.y + rect.height / 2, { steps: 12 }); await page.mouse.up();
  };
  await select(); await expect(page.locator('.pdf-selection-toolbar')).toBeVisible();
  expect(server.selections.at(-1).raw_quote).toBe('Selectable');
  expect(new Set(server.selections.at(-1).rects.map((r: any) => r.pageIndex))).toEqual(new Set([0]));
  await page.locator('.pdf-selection-toolbar').getByTitle('高亮', { exact: true }).click();
  await expect(page.locator(`${current} .pdf-user-highlight`)).toHaveCount(1);
  const geometry = () => page.locator(`${current} .pdf-page-shell`).first().evaluate(el => {
    const rect = el.getBoundingClientRect();
    const spans = [...el.querySelectorAll('.pdf-text-layer span')].filter(e => /^(Left Formula|Right Formula|Alpha beta|Lambda mu)/.test(e.textContent ?? ''));
    return { spans: spans.map(e => { const r = e.getBoundingClientRect(); return { text: e.textContent, left: (r.left - rect.left) / rect.width, top: (r.top - rect.top) / rect.height }; }),
      highlight: (el.querySelector('.pdf-user-highlight') as HTMLElement).style.cssText,
      canvas: { w: el.querySelector('canvas')!.width, h: el.querySelector('canvas')!.height }, layer: el.querySelector('.pdf-text-layer')!.getBoundingClientRect().width, width: rect.width };
  });
  const before = await geometry(); expect(before.spans).toHaveLength(4);
  await tool(page); await page.getByLabel('PDF 缩放倍率（相对整页）').selectOption('1.5'); await settled(page);
  const after = await geometry(); expect(after.highlight).toBe(before.highlight);
  for (let i = 0; i < 4; i++) { expect(after.spans[i].left).toBeCloseTo(before.spans[i].left, 2); expect(after.spans[i].top).toBeCloseTo(before.spans[i].top, 2); }
  expect(after.layer).toBeCloseTo(after.width, 0);
  await page.getByRole('button', { name: '适合整页', exact: true }).click(); await settled(page);
  await page.locator('.pdf-reader-pane .focus-tools-toggle').click();
  for (let i = 0; i < 3; i++) { await page.getByRole('button', { name: '下一屏', exact: true }).click(); await settled(page); await page.waitForTimeout(250); }
  expect((await snapshot(page)).current.pages).toEqual([6]); await expect(page.getByRole('button', { name: '下一屏', exact: true })).toBeDisabled();
  const anchor = await page.evaluate(() => (window as any).fp6.anchor());
  await tool(page); await page.getByRole('button', { name: '重排正文', exact: true }).click();
  await page.locator('.reader-pane .focus-tools-toggle').click();
  await page.getByRole('button', { name: 'PDF 原版', exact: true }).click(); await settled(page);
  expect((await snapshot(page)).current.pages).toContain(anchor.pageIndex);
  await tool(page); await page.getByRole('navigation', { name: '专注阅读工具', exact: true }).getByRole('button', { name: '问答', exact: true }).click();
  await page.getByRole('textbox', { name: '围绕当前阅读内容提问' }).fill('解释当前 PDF 页');
  await page.getByRole('button', { name: '发送', exact: true }).click();
  await expect.poll(() => server.calls.filter(c => c.path === '/agent/runs').length).toBe(1);
  const question = server.calls.find(c => c.path === '/agent/runs')!.body;
  expect(question.question_quote).toBeFalsy();
  await save('columns-projection-return', { before, after, anchor, final: await snapshot(page), selections: server.selections, question });
});

test('PDF touch follow, reverse, reduced motion and zoomed pan ownership', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 }); await open(page);
  await page.evaluate(() => { (window as any).touchEvents = []; for (const name of ['pointerdown', 'pointermove', 'pointerup', 'pointercancel', 'lostpointercapture', 'selectionchange']) document.addEventListener(name, e => { (window as any).touchEvents.push({ name, type: (e as PointerEvent).pointerType, target: (e.target as Element)?.className, selection: getSelection()?.toString(), phase: document.querySelector('.pdf-focus-host')?.getAttribute('data-turn-phase') }); }, true); });
  const cdp = await page.context().newCDPSession(page), before = await snapshot(page);
  const swipe = async (xs: number[], y = 480) => {
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: xs[0], y }] });
    for (const x of xs.slice(1)) { await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x, y }] }); await page.waitForTimeout(25); }
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  };
  await swipe([310, 290, 260, 220, 150, 80]); await settled(page);
  await save('touch-input', { events: await page.evaluate(() => (window as any).touchEvents), before, after: await snapshot(page) });
  expect((await snapshot(page)).current.start.index).toBe(before.current.start.index + 1);
  await swipe([80, 100, 140, 190, 250, 310]); await settled(page); expect((await snapshot(page)).current.start.index).toBe(before.current.start.index);
  await tool(page); await page.getByLabel('PDF 缩放倍率（相对整页）').selectOption('2'); await settled(page);
  await page.locator('.pdf-reader-pane .focus-tools-toggle').click();
  const zoomed = await snapshot(page); await swipe([310, 270, 220, 160, 90]); await page.waitForTimeout(400);
  expect((await snapshot(page)).current.id).toBe(zoomed.current.id);
  expect(await page.locator('.pdf-focus-screen:not([hidden])').evaluate(el => el.scrollLeft)).toBeGreaterThan(0);
  await page.emulateMedia({ reducedMotion: 'reduce' }); await page.getByRole('button', { name: '下一屏', exact: true }).click();
  expect((await snapshot(page)).turn.phase).toBe('idle'); budget(await snapshot(page), 390);
  await save('touch-pan', { before, zoomed, final: await snapshot(page) }); await cdp.detach();
});

for (const dpr of [2, 3]) test(`PDF original page reuse, selection and pixel cap at DPR ${dpr}`, async ({ browser }) => {
  const context = await browser.newContext({ baseURL: test.info().project.use.baseURL, viewport: { width: 1440, height: 900 }, deviceScaleFactor: dpr });
  const page = await context.newPage(); await open(page); await tool(page);
  const before = await page.locator('.pdf-focus-screen:not([hidden]) canvas').evaluateAll(nodes => nodes.map((n: any) => ({ width: n.width, height: n.height })));
  await page.getByLabel('PDF 缩放倍率（相对整页）').selectOption('2'); await settled(page);
  const after = await page.locator('.pdf-focus-screen:not([hidden]) canvas').evaluateAll(nodes => nodes.map((n: any) => ({ width: n.width, height: n.height })));
  expect(after).toEqual(before); budget(await snapshot(page), 1440);
  await page.setViewportSize({ width: 390, height: 844 }); await settled(page); await page.waitForTimeout(500);
  budget(await snapshot(page), 390);
  await page.getByRole('button', { name: '适合整页', exact: true }).click(); await settled(page);
  await page.locator('.pdf-reader-pane .focus-tools-toggle').click();
  await page.screenshot({ path: path.join(output, `mobile-dpr-${dpr}.png`) });
  await save(`dpr-${dpr}`, { dpr, before, after, final: await snapshot(page), anchor: await page.evaluate(() => (window as any).fp6.anchor()) });
  await context.close();
});

for (const width of [1440, 390]) for (const weak of [false, true]) test(`PDF normal 200 ${width} ${weak ? 'weak' : 'local'}`, async ({ page, browser }) => {
  await page.setViewportSize({ width, height: width === 390 ? 844 : 900 }); await open(page, weak);
  const stop = process.env.FP7_TRACE_NORMAL === '1' ? await startFrameTrace(page, /^fp4-\d+-input$/) : undefined;
  // Diagnose the original normal-reading workload, including its RAF style
  // observer. P1/P2 trace tests separately avoid that diagnostic overhead.
  if (stop) await page.evaluate(() => { delete (window as any).focusTraceActive; });
  const states = [], begin = Date.now(); let direction = 1;
  try { for (let i = 0; i < 200; i++) {
    const before = await snapshot(page);
    if (direction > 0 && before.current.eof || direction < 0 && before.current.start.index === 0 || i === 65 || i === 125) direction *= -1;
    await pacedReaderClick(page, 'fp6', `fp4-${i}`, direction);
    const after = await snapshot(page); states.push({ before, after }); budget(after, width);
    expect(after.current.id).not.toBe(before.current.id); expect(after.metrics.misses).toBe(0);
    if (i % 25 === 0) await save(`normal-progress-${width}-${weak ? 'weak' : 'local'}`, { turns: i + 1, state: after });
  } } finally {
    const elapsedMs = Date.now() - begin;
    const trace = stop ? await stop(path.join(output, `normal-${width}-${weak ? 'weak' : 'local'}.trace.json.gz`)) : undefined;
    await save(`normal-${width}-${weak ? 'weak' : 'local'}`, { browser: browser.version(), width, weak, elapsedMs, trace,
    environment: await page.evaluate(() => ({ userAgent: navigator.userAgent, viewport: [innerWidth, innerHeight], dpr: devicePixelRatio, longTasksSupported: PerformanceObserver.supportedEntryTypes.includes('longtask') })),
    states, samples: await page.evaluate(() => (window as any).fp6.samples()), longTasks: await page.evaluate(() => (window as any).fp6.longTasks()) }); }
});

for (const width of [1440, 390]) for (const weak of [false, true]) test(`PDF render trace ${width} ${weak ? 'weak' : 'local'}`, async ({ page }) => {
  await page.setViewportSize({ width, height: width === 390 ? 844 : 900 }); await open(page, weak);
  await mkdir(output, { recursive: true });
  const stop = await startFrameTrace(page, /^fp4-\d+-input$/);
  for (let i = 0; i < 20; i++) {
    await page.evaluate(name => (window as any).fp6.arm(name), `fp4-${i}`);
    const back = i >= 10 && i < 15;
    if (i % 5 === 4) { await page.locator('.pdf-reader-pane').evaluate(el => { (el as HTMLElement).tabIndex = -1; (el as HTMLElement).focus(); }); await page.keyboard.press(back ? 'PageUp' : 'PageDown'); }
    else await page.getByRole('button', { name: back ? '上一屏' : '下一屏', exact: true }).click();
    await page.waitForTimeout(1000);
    if (i === 9 && process.env.FP7_ENGINE === 'webview2') await stop.checkpoint();
  }
  const name = `render-${width}-${weak ? 'weak' : 'local'}`;
  const trace = await stop(path.join(output, `${name}.trace.json.gz`));
  await save(name, { trace, samples: await page.evaluate(() => (window as any).fp6.samples()), longTasks: await page.evaluate(() => (window as any).fp6.longTasks()), final: await snapshot(page) });
});

test('PDF render touch trace', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 }); await open(page);
  const cdp = await page.context().newCDPSession(page); await mkdir(output, { recursive: true });
  const stop = await startFrameTrace(page, /^fp4-\d+-input$/);
  for (let i = 0; i < 20; i++) {
    const back = i >= 10 && i < 15, xs = back ? [80, 100, 140, 190, 250, 310] : [310, 290, 260, 220, 150, 80];
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: xs[0], y: 480 }] });
    await page.evaluate(name => (window as any).fp6.arm(name), `fp4-${i}`);
    for (const x of xs.slice(1)) { await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x, y: 480 }] }); await page.waitForTimeout(25); }
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] }); await page.waitForTimeout(900);
  }
  const trace = await stop(path.join(output, 'render-touch.trace.json.gz'));
  await save('render-touch', { trace, samples: await page.evaluate(() => (window as any).fp6.samples()), longTasks: await page.evaluate(() => (window as any).fp6.longTasks()), final: await snapshot(page) });
  await cdp.detach();
});

for (const width of [1440, 390]) test(`PDF display changes ${width}`, async ({ page }) => {
  test.setTimeout(420_000);
  await page.setViewportSize({ width, height: width === 390 ? 844 : 900 }); await open(page);
  await mkdir(output, { recursive: true }); const stop = await startFrameTrace(page, /^fp6-display-.*-input$/);
  const change = async (kind: string, index: number, action: () => Promise<unknown>) => {
    await page.evaluate(name => (window as any).fp6.displayArm(name), `fp6-display-${kind}-${index}`);
    await action();
    await expect.poll(() => page.evaluate(() => (window as any).fp6.displaySamples().at(-1)?.end), { timeout: 20000 }).toBeTruthy();
    await page.waitForTimeout(400); budget(await snapshot(page), width);
  };
  await tool(page);
  for (let i = 0; i < 20; i++) await change('zoom', i, () => page.getByLabel('PDF 缩放倍率（相对整页）').selectOption(i % 2 ? '1' : '1.25'));
  if (process.env.FP7_ENGINE === 'webview2') await stop.checkpoint();
  for (let i = 0; i < 20; i++) {
    if (width === 1440) await change('columns', i, () => page.getByRole('button', { name: i % 2 ? '自动双页' : '单页阅读', exact: true }).click());
    else await change('rotation', i, () => page.setViewportSize(i % 2 ? { width: 390, height: 844 } : { width: 844, height: 390 }));
  }
  if (process.env.FP7_ENGINE === 'webview2') await stop.checkpoint();
  for (let i = 0; i < 20; i++) {
    await page.getByRole('navigation', { name: '专注阅读工具', exact: true }).getByRole('button', { name: '退出专注', exact: true }).click(); await expect(page.locator('.pdf-focus-host')).toBeHidden();
    await change('entry', i, () => page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().click());
    await tool(page);
  }
  await stop(path.join(output, `display-${width}.trace.json.gz`));
  await save(`display-${width}`, { width, samples: await page.evaluate(() => (window as any).fp6.displaySamples()), final: await snapshot(page) });
});

test('PDF long reading 20 minutes and 600 turns', async ({ page, browser }) => {
  test.setTimeout(1_350_000); await open(page); const initial = await snapshot(page), samples = [];
  const begin = Date.now(); let direction = 1;
  try { for (let i = 0; i < 601; i++) {
    if (i && i % 100 === 0) {
      await tool(page); await page.getByLabel('PDF 缩放倍率（相对整页）').selectOption(i % 200 ? '1.25' : '1');
      await settled(page); await page.locator('.pdf-reader-pane .focus-tools-toggle').click(); await page.waitForTimeout(600);
    }
    const before = await snapshot(page);
    if (direction > 0 && before.current.eof || direction < 0 && before.current.start.index === 0 || i === 75 || i === 175) direction *= -1;
    await page.evaluate(name => (window as any).fp6.arm(name), `fp4-long-${i}`);
    await page.getByRole('button', { name: direction > 0 ? '下一屏' : '上一屏', exact: true }).click();
    await page.waitForTimeout(Math.max(1, begin + (i + 1) * 2000 - Date.now()));
    const after = await snapshot(page); samples.push({ before, after }); budget(after, 1440);
    expect(after.current.id).not.toBe(before.current.id); expect(after.metrics.misses).toBe(0);
    if (i % 25 === 0) await save('long-progress', { turns: i + 1, elapsedMs: Date.now() - begin, after });
  }
  await page.emulateMedia({ reducedMotion: 'reduce' });
  while ((await snapshot(page)).current.start.index > 0) {
    await page.getByRole('button', { name: '上一屏', exact: true }).click(); await settled(page); await page.waitForTimeout(800);
  }
  await page.waitForTimeout(500);
  } finally { await save('long-reading', { browser: browser.version(), elapsedMs: Date.now() - begin, turns: samples.length, initial,
    final: await snapshot(page), samples, interactions: await page.evaluate(() => (window as any).fp6.samples()), longTasks: await page.evaluate(() => (window as any).fp6.longTasks()) }); }
});
