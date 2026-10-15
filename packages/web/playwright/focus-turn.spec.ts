import { test } from './focus-test';
import { expect, type Page } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { backend } from './focus-app-backend';
import { startFrameTrace, pacedReaderClick } from './focus-reading-trace';

const output = path.resolve(process.env.FOCUS_EVIDENCE_DIR ?? '../../docs/performance/focus-reading-fp4-20261014/results');
test.beforeAll(() => mkdir(output, { recursive: true }));
const save = (name: string, data: unknown) => writeFile(path.join(output, `${name}.json`), JSON.stringify(data, null, 2));
const snapshot = (page: Page) => page.evaluate(() => (window as any).fp4.snapshot());
async function settled(page: Page) {
  await expect.poll(async () => { const s = await snapshot(page); return s?.phase === 'ready' && !s.preparing && s.turn.phase === 'idle'; }).toBe(true);
}
async function open(page: Page, material = 'complex', chunk?: string, supplied?: any[], annotated = false) {
  const server = await backend(page, material, chunk, supplied);
  if (annotated) server.control.annotations = server.sources.filter((s: any, i: number) => i % 4 === 0 && s.kind === 'paragraph').map((s: any, i: number) => ({
    mem_id: `trace-hl-${i}`, type: 'highlight', layer: 'long_term', book_id: material, anchor: { lid: s.lid },
    range: { start: 0, end: Math.min(30, s.text.length) }, content: s.text.slice(0, 30),
  }));
  await page.goto(process.env.FOCUS_ENTRY_URL ?? '/playwright/fixtures/focus-turn.html');
  await page.waitForFunction(() => (window as any).fp4?.available());
  await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().click();
  await settled(page); return server;
}
async function tool(page: Page, name: string) {
  await page.getByRole('button', { name: '阅读工具', exact: true }).click();
  await page.getByRole('navigation', { name: '专注阅读工具' }).getByRole('button', { name, exact: true }).click();
}
function noWork(before: any, after: any) {
  for (const key of ['alignments', 'preparations', 'geometryProbes', 'decorations']) expect(after.work[key], key).toBe(before.work[key]);
}
function budget(state: any, width: number) {
  const limits = { screens: 4, pages: width <= 600 ? 4 : 8, sourceBytes: 524288, parsedBytes: 2097152,
    domNodes: 12000, decodedImagePixels: 8000000, preparationTasks: 1 };
  for (const [key, limit] of Object.entries(limits)) expect(state.peak[key], key).toBeLessThanOrEqual(limit);
}
test('public entry, keyboard, overlay tools and exit continuity', async ({ page }) => {
  const { calls } = await open(page);
  const start = await snapshot(page);
  expect(await page.locator('.focus-screen:not([hidden]) .focus-page').count()).toBe(2);
  await page.getByRole('button', { name: '下一屏', exact: true }).click();
  await expect.poll(async () => (await snapshot(page)).turn.phase).toBe('idle');
  await settled(page); const next = await snapshot(page); expect(next.current.start).toEqual(start.current.end);
  await page.keyboard.press('PageUp'); await settled(page);
  expect((await snapshot(page)).current.end).toMatchObject({ index: start.current.end.index, offset: start.current.end.offset });
  const beforeTools = await snapshot(page);
  await page.getByRole('button', { name: '阅读工具', exact: true }).click();
  await page.waitForTimeout(100); noWork(beforeTools, await snapshot(page));
  await page.getByRole('navigation', { name: '专注阅读工具' }).getByRole('button', { name: '阅读设置', exact: true }).click();
  await expect(page.getByRole('button', { name: '关闭阅读设置' })).toBeVisible();
  await page.getByRole('button', { name: '关闭阅读设置' }).click();
  await page.locator('.reader-pane').focus(); await page.keyboard.press('PageDown'); await settled(page);
  const exit = (await snapshot(page)).current.start;
  await tool(page, '退出专注');
  await expect(page.locator('.focus-reader-body')).toBeHidden();
  await expect(page.locator('.reader-pane .flow-paragraph').first()).toBeVisible();
  await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().click(); await settled(page);
  const returned = await snapshot(page);
  expect(returned.current.start).toMatchObject({ index: exit.index, offset: exit.offset });
  await save('entry-exit', { start, next, exit, returned, calls });
  await page.screenshot({ path: path.join(output, 'desktop.png') });
});

test('mobile touch cover, cancellation, reversal and reduced motion', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 }); await open(page);
  await page.evaluate(() => {
    const events: unknown[] = []; Object.assign(window, { touchEvents: events });
    for (const name of ['pointerdown', 'pointermove', 'pointerup', 'pointercancel', 'lostpointercapture', 'selectionchange']) document.addEventListener(name, event => {
      const e = event as PointerEvent;
      events.push({ name, x: e.clientX, id: e.pointerId, primary: e.isPrimary, type: e.pointerType, at: e.timeStamp,
        target: (e.target as Element)?.className, selection: window.getSelection()?.toString(), state: (window as any).fp4.snapshot()?.turn });
    });
  });
  const cdp = await page.context().newCDPSession(page);
  const box = (await page.locator('.focus-reader-surface').boundingBox())!;
  const safeY = (x: number) => page.evaluate(({ x, top, height }) => {
    for (let y = top + 12; y < top + height - 12; y += 12) {
      const target = document.elementFromPoint(x, y);
      if (target?.closest('.focus-reader-surface') && !target.closest('a,button,.focus-object,.katex')) return y;
    }
    throw new Error('No prose gesture starting point');
  }, { x, top: box.y, height: box.height });
  const x = box.x + box.width * .7;
  let y = await safeY(x);
  const touch = (type: 'touchStart' | 'touchMove' | 'touchEnd' | 'touchCancel', px: number) => cdp.send('Input.dispatchTouchEvent', { type, touchPoints: type === 'touchEnd' || type === 'touchCancel' ? [] : [{ x: px, y, id: 1 }] });
  const before = await snapshot(page);
  await touch('touchStart', x); await touch('touchMove', x - 30); await page.waitForTimeout(100);
  const dragging = await snapshot(page); expect(dragging.turn.phase).toBe('dragging'); noWork(before, dragging);
  await page.screenshot({ path: path.join(output, 'mobile-drag.png') });
  await page.waitForTimeout(130); await touch('touchEnd', x - 30); await settled(page);
  expect((await snapshot(page)).current.id).toBe(before.current.id);
  await touch('touchStart', x); await touch('touchMove', x - 40); await touch('touchMove', x - box.width * .6); await touch('touchEnd', x - box.width * .6);
  await settled(page); const advanced = await snapshot(page);
  await save('touch-events', await page.evaluate(() => (window as any).touchEvents));
  expect(advanced.current.start).toEqual(before.current.end);
  y = await safeY(box.x + 60);
  await touch('touchStart', box.x + 60); await touch('touchMove', box.x + 160); await touch('touchMove', box.x + 240); await touch('touchEnd', box.x + 240);
  await settled(page); expect((await snapshot(page)).current.start).toEqual(before.current.start);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.getByRole('button', { name: '下一屏', exact: true }).click();
  expect((await snapshot(page)).turn.phase).toBe('idle');
  await save('touch', { before, dragging, advanced, final: await snapshot(page) }); await cdp.detach();
});

test('first and odd last spread, rapid input and selected text ownership', async ({ page }) => {
  const sources = Array.from({ length: 3 }, (_, i) => ({ lid: `1.${i + 1}`, kind: 'code', text: Array.from({ length: 70 }, (_, n) => `page ${i + 1} line ${n}`).join('\n') }));
  await open(page, 'bounds', undefined, sources);
  await expect(page.getByRole('button', { name: '上一屏', exact: true })).toBeDisabled();
  expect(await page.locator('.focus-screen:not([hidden]) .focus-page').count()).toBe(2);
  await page.getByRole('button', { name: '下一屏', exact: true }).click(); await settled(page);
  await expect(page.getByRole('button', { name: '下一屏', exact: true })).toBeDisabled();
  expect(await page.locator('.focus-screen:not([hidden]) .focus-page').count()).toBe(1);
  const end = await snapshot(page);
  await page.locator('.reader-pane').focus(); await page.keyboard.press('PageDown');
  expect((await snapshot(page)).current.id).toBe(end.current.id);
  const code = page.locator('.focus-screen:not([hidden]) pre');
  await code.evaluate(el => { const r = document.createRange(); r.selectNodeContents(el); const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(r); });
  await page.keyboard.press('PageUp'); expect((await snapshot(page)).current.id).toBe(end.current.id);
  await page.evaluate(() => window.getSelection()?.removeAllRanges());
  for (let i = 0; i < 40; i++) { await page.keyboard.press(i % 2 ? 'PageDown' : 'PageUp'); await page.waitForTimeout(250); }
  await settled(page); expect((await snapshot(page)).current.id).toBe(end.current.id);
  await save('bounds-fast-selection', { end, final: await snapshot(page) });
});

test('table horizontal touch scroll keeps the current page', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 }); await open(page);
  const table = page.locator('.focus-screen:not([hidden]) .focus-object.asset-table').first();
  await expect(table).toBeVisible();
  const box = (await table.boundingBox())!;
  const before = await snapshot(page), cdp = await page.context().newCDPSession(page);
  const y = Math.max(box.y + 10, Math.min(box.y + box.height / 2, 780));
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: 300, y }] });
  for (const x of [260, 200, 140, 80]) { await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x, y }] }); await page.waitForTimeout(25); }
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await page.waitForTimeout(150); expect((await snapshot(page)).current.id).toBe(before.current.id);
  expect(await table.evaluate(el => el.scrollLeft)).toBeGreaterThan(0);
  await cdp.detach();
});

test('single/double page reflow preserves source at 320, 1024 and rotated widths', async ({ page }) => {
  await open(page, 'epub');
  const anchor = (await snapshot(page)).current.start, states = [];
  for (const [width, height] of [[1024, 900], [320, 780], [844, 390], [1440, 900]]) {
    await page.setViewportSize({ width, height }); await settled(page);
    await expect(page.locator('.focus-screen:not([hidden]):not([aria-hidden="true"]) .focus-page')).toHaveCount(width >= 1024 ? 2 : 1);
    const state = await snapshot(page); states.push(state);
    expect(state.current.start).toMatchObject({ index: anchor.index, offset: anchor.offset });
  }
  await save('responsive', states);
});

test('public directory, assistant return and source replacement', async ({ page }) => {
  const sources = Array.from({ length: 30 }, (_, i) => ({ lid: `${i < 15 ? 1 : 2}.${i % 15 + 1}`, parent: i < 15 ? '1' : '2', kind: 'paragraph', text: `Paragraph ${i}. ` + '阅读位置沿原文保持。'.repeat(50) }));
  const { control } = await open(page, 'outline', undefined, sources);
  await tool(page, '目录');
  const search = page.locator('#outline-search'); await expect(search).toBeVisible();
  const before = await snapshot(page); await search.press('ArrowRight');
  expect((await snapshot(page)).current.id).toBe(before.current.id);
  const item = page.locator('.outline-item').last();
  await item.click(); await expect.poll(async () => (await snapshot(page)).current.start.index).toBe(15); await settled(page);
  const destination = await snapshot(page); expect(destination.current.start).not.toEqual(before.current.start);
  const close = page.getByRole('button', { name: '关闭目录', exact: true }); if (await close.isVisible()) await close.click();
  await tool(page, '问答');
  await page.getByRole('navigation', { name: '阅读工作区', exact: true }).getByRole('button', { name: '阅读', exact: true }).click(); await settled(page);
  expect((await snapshot(page)).current.start).toEqual(destination.current.start);
  await page.locator('.focus-reader-body').evaluate(el => el.setAttribute('data-old-host', 'yes'));
  control.revision = 'fp4-source-2'; await page.evaluate(() => window.dispatchEvent(new Event('pageshow')));
  await expect(page.locator('[data-old-host]')).toHaveCount(0);
  await expect(page.locator('.reader-pane .flow-paragraph').first()).toBeVisible();
  await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().click(); await settled(page);
  await save('directory-and-version', { before, destination, final: await snapshot(page) });
});

for (const width of [1440, 390]) for (const weak of [false, true]) test(`public normal 200 ${width} ${weak ? 'weak' : 'local'}`, async ({ page, browser, browserName }) => {
  await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
  const server = await open(page); const cdp = browserName === 'webkit' ? null : await page.context().newCDPSession(page);
  if (weak) {
    server.control.delay = 100;
    if (cdp) { await cdp.send('Network.enable');
      await cdp.send('Network.emulateNetworkConditions', { offline: false, latency: 100, downloadThroughput: 10_000_000 / 8, uploadThroughput: 10_000_000 / 8 });
    } else server.control.downloadBytesPerMs = 1250;
  }
  const begin = Date.now(), states = []; let direction = 1;
  try { for (let i = 0; i < 200; i++) {
    const before = await snapshot(page), current = before.current;
    if ((direction > 0 && current.eof) || (direction < 0 && !current.start.index && !current.start.offset) || i === 65 || i === 125) direction *= -1;
    await pacedReaderClick(page, 'fp4', `fp4-${i}`, direction);
    const after = await snapshot(page); states.push({ before, after });
    if (i % 20 === 0) await save(`normal-progress-${width}-${weak ? 'weak' : 'local'}`, { turns: i + 1, state: after });
    expect(after.current.id).not.toBe(before.current.id); expect(after.metrics.misses).toBe(0);
    budget(after, width);
  } } finally {
    await save(`normal-${width}-${weak ? 'weak' : 'local'}`, { browser: browser.version(), width, weak, elapsedMs: Date.now() - begin,
      environment: await page.evaluate(() => ({ userAgent: navigator.userAgent, viewport: [innerWidth, innerHeight], dpr: devicePixelRatio, longTasksSupported: PerformanceObserver.supportedEntryTypes.includes('longtask') })),
      states, samples: await page.evaluate(() => (window as any).fp4.samples()), calls: server.calls, longTasks: await page.evaluate(() => (window as any).fp4.longTasks()) });
  }
  const samples = await page.evaluate(() => (window as any).fp4.samples());
  for (const sample of samples) { expect(sample.end).not.toBeNull(); noWork({ work: sample.before }, { work: sample.after }); }
  expect(samples.length).toBe(200); await cdp?.detach();
});

for (const width of [1440, 390]) for (const weak of [false, true]) test(`render trace ${width} ${weak ? 'weak' : 'local'}`, async ({ page }) => {
  await page.setViewportSize({ width, height: width === 390 ? 844 : 900 }); const server = await open(page, 'complex', undefined, undefined, true);
  const cdp = await page.context().newCDPSession(page);
  if (weak) { server.control.delay = 100; await cdp.send('Network.enable'); await cdp.send('Network.emulateNetworkConditions', { offline: false, latency: 100, downloadThroughput: 10_000_000 / 8, uploadThroughput: 10_000_000 / 8 }); }
  const stop = await startFrameTrace(page, /^fp4-\d+-input$/);
  let begin = Date.now();
  for (let i = 0; i < 20; i++) {
    await page.evaluate(name => (window as any).fp4.arm(name), `fp4-${i}`);
    const backward = i >= 10 && i < 15;
    if (i % 5 === 4) { await page.locator('.reader-pane').focus(); await page.keyboard.press(backward ? 'PageUp' : 'PageDown'); }
    else await page.getByRole('button', { name: backward ? '上一屏' : '下一屏', exact: true }).click();
    await page.waitForTimeout(Math.max(1, begin + (i + 1) * 1000 - Date.now()));
    if (i === 9 && process.env.FP7_ENGINE === 'webview2') {
      const pause = Date.now(); await stop.checkpoint(); begin += Date.now() - pause;
    }
  }
  const name = `render-${width}-${weak ? 'weak' : 'local'}`;
  const trace = await stop(path.join(output, `${name}.trace.json.gz`));
  await save(name, { trace, samples: await page.evaluate(() => (window as any).fp4.samples()), longTasks: await page.evaluate(() => (window as any).fp4.longTasks()), final: await snapshot(page) });
  await cdp.detach();
});

test('rapid replacement and unavailable target keep the last readable screen', async ({ page }) => {
  const { control } = await open(page); control.offline = true;
  await page.locator('.reader-pane').focus();
  for (let i = 0; i < 10; i++) { await page.keyboard.press('PageDown'); await page.waitForTimeout(25); }
  await expect(page.getByRole('button', { name: '重试', exact: true })).toBeVisible();
  const waiting = await snapshot(page); expect(waiting.metrics.misses).toBeGreaterThan(0);
  await page.waitForTimeout(250); expect((await snapshot(page)).current.id).toBe(waiting.current.id);
  expect(await page.locator('.focus-screen:not([hidden])').count()).toBe(1);
  control.offline = false; await page.keyboard.press('PageUp'); await settled(page);
  const reversed = await snapshot(page); expect(reversed.current.end).toEqual(waiting.current.start);
  // Inputs arrive while a 200ms transition is still running. No queued motion
  // may later override the final direction once the user has stopped.
  for (let i = 0; i < 8; i++) { await page.keyboard.press(i % 2 ? 'PageUp' : 'PageDown'); await page.waitForTimeout(40); }
  await settled(page); const final = await snapshot(page);
  await page.waitForTimeout(500); expect((await snapshot(page)).current.id).toBe(final.current.id);
  expect(final.current.start).toEqual(reversed.current.start);
  await save('rapid-unavailable', { waiting, reversed, final });
});

test('render trace touch drag and settle', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 }); await open(page, 'complex', undefined, undefined, true);
  const cdp = await page.context().newCDPSession(page), stop = await startFrameTrace(page, /^fp4-\d+-input$/);
  const begin = Date.now(), gestures = [];
  for (let i = 0; i < 20; i++) {
    const box = (await page.locator('.focus-reader-surface').boundingBox())!, direction = i % 2 ? -1 : 1;
    const x = box.x + box.width * (direction > 0 ? .75 : .25);
    const y = await page.evaluate(({ x, top, height }) => {
      for (let y = top + 20; y < top + height - 20; y += 12) {
        const el = document.elementFromPoint(x, y);
        if (el?.closest('.focus-reader-surface') && !el.closest('a,button,.focus-object,.katex')) return y;
      }
      throw new Error('No gesture point');
    }, { x, top: box.y, height: box.height });
    const before = await snapshot(page);
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
    const started = Date.now(); await page.evaluate(name => (window as any).fp4.arm(name), `fp4-${i}`);
    for (let n = 1; n <= 12; n++) {
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: x - direction * box.width * .6 * n / 12, y }] });
      await page.waitForTimeout(16);
    }
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
    await settled(page); const after = await snapshot(page);
    gestures.push({ before, after, pointerStartToReleaseMs: Date.now() - started });
    expect(after.current.id).not.toBe(before.current.id);
    await page.waitForTimeout(Math.max(1, begin + (i + 1) * 1000 - Date.now()));
  }
  const trace = await stop(path.join(output, 'render-touch.trace.json.gz'));
  const samples = await page.evaluate(() => (window as any).fp4.samples());
  await save('render-touch', { trace, gestures, samples, longTasks: await page.evaluate(() => (window as any).fp4.longTasks()), final: await snapshot(page) });
  for (const sample of samples) noWork({ work: sample.before }, { work: sample.after });
  expect(samples.length).toBe(20); await cdp.detach();
});
