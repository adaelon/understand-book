import { expect, test } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { distribution } from './focus-reading-trace';

const output = path.resolve(process.env.FP2_OUTPUT ?? '../../docs/performance/focus-reading-fp2-20261014/results');
const url = '/playwright/fixtures/focus-pagination.html';
test.beforeAll(() => mkdir(output, { recursive: true }));
const save = (name: string, value: unknown) => writeFile(path.join(output, `${name}.json`), JSON.stringify(value, null, 2));
function budget(r: any) {
  expect(r.screens).toBeLessThanOrEqual(4); expect(r.pages).toBeLessThanOrEqual(r.screens * 2);
  expect(r.sourceBytes).toBeLessThanOrEqual(524_288); expect(r.parsedBytes).toBeLessThanOrEqual(2_097_152);
  expect(r.domNodes).toBeLessThanOrEqual(12_000); expect(r.decodedImagePixels).toBeLessThanOrEqual(8_000_000);
  expect(r.preparationTasks).toBeLessThanOrEqual(1);
}
for (const width of [1440, 390]) for (const material of ['epub', 'complex', 'mixed']) {
  test(`canonical continuation ${material} ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
    await page.goto(url); await page.waitForFunction(() => !!(window as any).fp2);
    const opened = await page.evaluate(m => (window as any).fp2.open(m, 'stress'), material);
    const screens = [opened.screen], resources = [opened.resources];
    for (let n = 0; n < 150; n++) {
      const audit = await page.evaluate(() => (window as any).fp2.audit());
      for (const fragment of audit) expect(fragment.actual, `${fragment.lid}:${fragment.start}`).toBe(fragment.expected);
      const listItems = await page.locator('.focus-screen:not([hidden]) [data-lid="stress.list"] ol > li').evaluateAll(items => items.map(el => ({ number: el.hasAttribute('value') ? Number(el.getAttribute('value')) : (Array.from(el.parentElement!.children).indexOf(el) + Number(el.parentElement!.getAttribute('start') ?? 1)), label: el.textContent?.match(/第 (\d+) 项/)?.[1] })));
      for (const item of listItems) if (item.label) expect(item.number).toBe(Number(item.label) + 1);
      if (screens.at(-1).eof) break;
      const next = await page.evaluate(() => (window as any).fp2.turn(1));
      expect(next).not.toBeNull(); screens.push(next.screen); resources.push(next.resources);
    }
    expect(screens.at(-1).eof).toBe(true);
    const fragments = screens.flatMap(s => s.pages.flat());
    for (let index = 0; index < opened.sources.length; index++) {
      const source = opened.sources[index]; const parts = fragments.filter(f => f.index === index);
      expect(parts.length, source.lid).toBeGreaterThan(0);
      let offset = 0;
      for (const part of parts) { expect(part.start, source.lid).toBe(offset); offset = part.end; }
      expect(offset, source.lid).toBe(source.length);
    }
    // Reverse across evicted windows, rebuilding from canonical source cursors.
    for (let i = screens.length - 2; i >= 0; i--) {
      const previous = await page.evaluate(() => (window as any).fp2.turn(-1));
      expect(previous.screen.pages).toEqual(screens[i].pages); resources.push(previous.resources);
    }
    resources.forEach(budget);
    budget((await page.evaluate(() => (window as any).fp2.report())).peak);
    await page.screenshot({ path: path.join(output, `continuation-${material}-${width}.png`) });
    await save(`continuation-${material}-${width}`, { opened, screens, resources, report: await page.evaluate(() => (window as any).fp2.report()) });
  });
}

test('zero-rect whitespace, adjacent screens and canonical selection', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 }); await page.goto(url);
  await page.waitForFunction(() => !!(window as any).fp2);
  await page.evaluate(() => (window as any).fp2.open('mixed', 'whitespace', 22));
  const samples = [];
  for (let i = 0; i < 12; i++) {
    const audit = await page.evaluate(() => (window as any).fp2.audit());
    for (const f of audit) expect(f.actual).toBe(f.expected);
    const next = await page.evaluate(() => (window as any).fp2.turn(1));
    if (!next) break;
    samples.push(next);
  }
  const selection = await page.evaluate(() => {
    const engine = (window as any).fp2.engine(), screen = engine.visible();
    const f = screen.pages[0].fragments.find((f: any) => f.semanticEnd > f.semanticStart);
    const range = document.createRange(); range.selectNodeContents(f.element);
    getSelection()!.removeAllRanges(); getSelection()!.addRange(range);
    return { text: getSelection()!.toString(), ranges: engine.sourceRanges(f, range), fragment: { start: f.start, end: f.end }, hiddenSelectable: engine.resident().filter((s: any) => s !== screen).some((s: any) => !s.element.inert) };
  });
  expect(selection.text.length).toBeGreaterThan(0); expect(selection.hiddenSelectable).toBe(false);
  for (const r of selection.ranges) { expect(r.start).toBeGreaterThanOrEqual(selection.fragment.start); expect(r.end).toBeLessThanOrEqual(selection.fragment.end); }
  await save('whitespace-selection', { samples, selection });
});

for (const width of [1440, 390]) test(`warm reflow keeps source and picture ${width}`, async ({ page }) => {
  await page.setViewportSize({ width, height: width === 390 ? 844 : 900 }); await page.goto(url);
  await page.waitForFunction(() => !!(window as any).fp2);
  const cdp = await page.context().newCDPSession(page);
  let latest: { timestamp: number; data: string } | null = null;
  let changed: (() => void) | null = null;
  cdp.on('Page.screencastFrame', (frame: any) => {
    latest = { timestamp: frame.metadata.timestamp * 1000, data: frame.data };
    void cdp.send('Page.screencastFrameAck', { sessionId: frame.sessionId }); changed?.();
  });
  await cdp.send('Page.startScreencast', { format: 'jpeg', quality: 80, everyNthFrame: 1 });
  const nextChangedFrame = (after: number, before: string | undefined) => new Promise<{ timestamp: number; data: string }>((resolve, reject) => {
    const timeout = setTimeout(() => { changed = null; reject(new Error(`No changed readable browser frame: ${JSON.stringify({ after, latestAt: latest?.timestamp, samePixels: latest?.data === before })}`)); }, 3000);
    changed = () => { if (latest && latest.timestamp >= after && latest.data !== before) { clearTimeout(timeout); changed = null; resolve(latest); } };
    changed();
  });
  const samples = [];
  for (const material of ['epub', 'complex', 'mixed']) {
    await page.evaluate(m => (window as any).fp2.open(m, 'stress'), material);
    await page.evaluate(() => (window as any).fp2.warm());
    // Establish the old readable frame before timing a warm geometry change.
    // These RAFs settle capture startup; they are not presentation evidence.
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    for (let i = 0; i < 20; i++) {
      const before = (latest as { data: string } | null)?.data;
      const sample = await page.evaluate(async font => {
        const startedAt = performance.timeOrigin + performance.now();
        const result = await (window as any).fp2.reflow(font);
        return { ...result, startedAt, readyAt: performance.timeOrigin + performance.now() };
      }, i % 2 ? 19 : 22);
      const frame = await nextChangedFrame(sample.readyAt, before);
      sample.firstReadableBrowserFrameMs = frame.timestamp - sample.startedAt;
      sample.frameTimestamp = frame.timestamp;
      if (i === 19) await writeFile(path.join(output, `reflow-${material}-${width}.jpg`), Buffer.from(frame.data, 'base64'));
      expect(sample.oldStillVisible).toBe(true);
      const first = sample.screen.pages[0][0]; expect(first.index).toBe(sample.anchor.index); expect(first.start).toBe(sample.anchor.offset);
      budget(sample.resources); samples.push({ material, ...sample });
    }
  }
  const timing = distribution(samples.map(s => s.readyMs));
  await cdp.send('Page.stopScreencast'); await cdp.detach();
  const browserFrameTiming = distribution(samples.map(s => s.firstReadableBrowserFrameMs));
  await save(`reflow-${width}`, { samples, timing, browserFrameTiming, physicalPresentation: null, report: await page.evaluate(() => (window as any).fp2.report()) });
  expect(timing.p95).toBeLessThanOrEqual(100);
  for (const material of ['epub', 'complex', 'mixed']) expect(distribution(samples.filter(s => s.material === material).map(s => s.readyMs)).p95).toBeLessThanOrEqual(100);
  for (const material of ['epub', 'complex', 'mixed']) expect(distribution(samples.filter(s => s.material === material).map(s => s.firstReadableBrowserFrameMs)).p95).toBeLessThanOrEqual(100);
});

test('images, overflow tails and rotation retain the same object or character', async ({ page }) => {
  await page.goto(url); await page.waitForFunction(() => !!(window as any).fp2);
  await page.evaluate(() => (window as any).fp2.open('complex', 'images'));
  const images = [];
  for (;;) {
    images.push(await page.locator('.focus-screen:not([hidden]) img').evaluateAll(imgs => imgs.map(el => {
      const img = el as HTMLImageElement, rect = img.getBoundingClientRect();
      return { naturalWidth: img.naturalWidth, width: rect.width, height: rect.height };
    })));
    if (!await page.evaluate(() => (window as any).fp2.turn(1))) break;
  }
  for (const img of images.flat()) { expect(img.naturalWidth).toBeGreaterThan(0); expect(img.height).toBeLessThanOrEqual(790); }
  budget((await page.evaluate(() => (window as any).fp2.report())).peak);
  await page.evaluate(() => (window as any).fp2.open('mixed', 'stress'));
  let code: any;
  for (;;) {
    const target = page.locator('.focus-screen:not([hidden]) [data-lid="stress.code"]');
    if (await target.count()) {
      code = await target.evaluate(el => { el.scrollTop = el.scrollHeight; el.scrollLeft = el.scrollWidth; return { top: el.scrollTop, left: el.scrollLeft, tail: el.textContent?.includes('line 99:') }; }); break;
    }
    if (!await page.evaluate(() => (window as any).fp2.turn(1))) throw new Error('Missing code tail');
  }
  expect(code.top).toBeGreaterThan(0); expect(code.left).toBeGreaterThan(0); expect(code.tail).toBe(true);
  await page.evaluate(() => (window as any).fp2.warm());
  await page.evaluate(() => (window as any).fp2.reflow(22, { index: 5, lid: 'stress.code', offset: 0 }));
  const rotations = [];
  for (const [width, height] of [[1024, 900], [390, 844], [844, 390], [320, 844], [1440, 900]]) {
    await page.setViewportSize({ width, height });
    const sample = await page.evaluate(() => (window as any).fp2.reflow(22));
    expect(sample.screen.pages[0][0].lid).toBe('stress.code'); expect(sample.oldStillVisible).toBe(true); budget(sample.resources); rotations.push(sample);
  }
  // Resume within a long inline-formula paragraph and keep the exact source char.
  const anchor = { index: 3, lid: 'stress.long', offset: 990 };
  const resumed = await page.evaluate(anchor => (window as any).fp2.reflow(19, anchor), anchor);
  const fragment = resumed.screen.pages[0][0]; expect(fragment.lid).toBe(anchor.lid); expect(fragment.start).toBeLessThanOrEqual(anchor.offset); expect(fragment.end).toBeGreaterThan(anchor.offset);
  for (const f of await page.evaluate(() => (window as any).fp2.audit())) expect(f.actual).toBe(f.expected);
  await save('images-overflow-rotation', { images, code, rotations, resumed });
});

test('first long-link mapping and formula continuation yield the main thread', async ({ page }) => {
  await page.goto(url); await page.waitForFunction(() => !!(window as any).fp2);
  const linkStart = await page.evaluate(() => performance.now());
  await page.evaluate(() => (window as any).fp2.open('complex', 'stress'));
  await page.waitForTimeout(50);
  const linkReport = await page.evaluate(() => (window as any).fp2.report());
  expect(linkReport.longTasks.filter((t: any) => t.start >= linkStart)).toEqual([]);
  await page.evaluate(() => (window as any).fp2.open('mixed', 'stress'));
  await page.evaluate(() => (window as any).fp2.warm());
  const start = await page.evaluate(() => performance.now());
  while (await page.evaluate(() => (window as any).fp2.turn(1))) { /* no audit renderer in the timed path */ }
  await page.waitForTimeout(50);
  const report = await page.evaluate(() => (window as any).fp2.report());
  const longTasks = report.longTasks.filter((t: any) => t.start >= start);
  await save('preparation-tasks', { linkReport, report, longTasks });
  expect(longTasks).toEqual([]);
});

test('long reading 20 minutes and 500 source-preserving turns', async ({ page, browser }) => {
  test.skip(process.env.FP2_LONG !== '1', 'Opt-in wall-clock acceptance');
  test.setTimeout(1_400_000);
  await page.goto(url); await page.waitForFunction(() => !!(window as any).fp2);
  await page.evaluate(() => (window as any).fp2.open('complex'));
  const start = Date.now(), samples = []; let turns = 0, direction = 1;
  while (Date.now() - start < 1_200_000 || turns < 500) {
    let sample = await page.evaluate(d => (window as any).fp2.turn(d), direction);
    if (!sample) { direction *= -1; sample = await page.evaluate(d => (window as any).fp2.turn(d), direction); }
    expect(sample).not.toBeNull(); turns++; budget(sample.resources); budget((await page.evaluate(() => (window as any).fp2.report())).peak);
    if (turns % 50 === 0 || turns === 1) {
      samples.push({ turns, elapsedMs: Date.now() - start, ...sample });
      await save('long-reading-progress', { turns, elapsedMs: Date.now() - start, samples });
    }
    if (turns % 100 === 0) await page.evaluate(font => (window as any).fp2.reflow(font), turns % 200 ? 22 : 19);
    await page.waitForTimeout(2000);
  }
  await save('long-reading', { browser: browser.version(), viewport: page.viewportSize(), elapsedMs: Date.now() - start, turns, samples, report: await page.evaluate(() => (window as any).fp2.report()) });
});
