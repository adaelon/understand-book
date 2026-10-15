import { test } from './focus-test';
import { expect, type Page } from '@playwright/test';
import { mkdir, writeFile, readFile } from 'node:fs/promises';
import { gunzipSync } from 'node:zlib';
import { startFrameTrace, waitAfterInput } from './focus-reading-trace';
import path from 'node:path';
import { backend } from './focus-app-backend';

const output = path.resolve(process.env.FOCUS_EVIDENCE_DIR ?? '../../docs/performance/focus-reading-fp5-20261015/results');
test.beforeAll(() => mkdir(output, { recursive: true }));
test.beforeEach(async ({ page }) => { page.setDefaultTimeout(12_000); });
test.afterEach(async ({ page }, info) => {
  if (info.status !== info.expectedStatus) {
    const name = `failure-${info.title.replace(/[^a-zA-Z0-9]+/g, '-').slice(0, 75)}`;
    await writeFile(path.join(output, `${name}.json`), JSON.stringify(await page.evaluate(() => ({
      state: (window as any).fp4.snapshot(), interactions: (window as any).fp4.samples(),
      selection: window.getSelection()?.toString(), visibility: document.visibilityState,
    })), null, 2));
    await page.screenshot({ path: path.join(output, `${name}.png`) });
  }
});
const save = (name: string, value: unknown) => writeFile(path.join(output, `${name}.json`), JSON.stringify(value, null, 2));
const snapshot = (page: Page) => page.evaluate(() => (window as any).fp4.snapshot());
async function settled(page: Page) {
  await expect.poll(async () => { const s = await snapshot(page); return s?.phase === 'ready' && !s.preparing && s.turn.phase === 'idle'; }).toBe(true);
}
async function enter(page: Page) {
  await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().click(); await settled(page);
}
async function open(page: Page, material = 'complex', supplied?: any[]) {
  const server = await backend(page, material, undefined, supplied);
  await page.goto(process.env.FOCUS_ENTRY_URL ?? '/playwright/fixtures/focus-turn.html');
  await page.waitForFunction(() => (window as any).fp4?.available()); await enter(page); return server;
}
async function tool(page: Page, name: string) {
  await page.locator('.focus-tools-toggle').click();
  await page.getByRole('navigation', { name: '专注阅读工具' }).getByRole('button', { name, exact: true }).click();
}
const longSources = Array.from({ length: 36 }, (_, i) => ({ lid: `1.${i + 1}`, kind: 'paragraph', text: `Paragraph ${i + 1}. ` + '原文范围必须准确，阅读位置始终保持。'.repeat(200) }));
async function selectAcross(page: Page) {
  return page.evaluate(() => {
    const pages = document.querySelectorAll('.focus-screen:not([hidden]):not([inert]) .focus-page');
    const left = pages[0].querySelector('[data-lid]') as HTMLElement, right = pages[1].querySelector('[data-lid]') as HTMLElement;
    const texts = (el: Element) => { const out: Text[] = [], w = document.createTreeWalker(el, NodeFilter.SHOW_TEXT); while (w.nextNode()) if (w.currentNode.textContent?.length) out.push(w.currentNode as Text); return out; };
    const a = texts(left).at(-1)!, b = texts(right)[0];
    const range = document.createRange(); range.setStart(a, a.length - 12); range.setEnd(b, 12);
    const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range);
    document.dispatchEvent(new Event('selectionchange'));
    return { lid: left.dataset.lid!, start: Number(left.dataset.sourceEnd) - 12, end: Number(right.dataset.sourceStart) + 12, text: range.toString() };
  });
}
test('cross-page selection, highlights, note draft, question, source and reflow use the same canonical range', async ({ page }) => {
  const server = await open(page, 'operations', longSources);
  const selected = await selectAcross(page);
  await expect(page.locator('.hl-popover')).toBeVisible();
  await page.locator('.hl-popover').getByRole('button', { name: '高亮', exact: true }).click();
  await expect.poll(() => server.control.annotations.length).toBe(1);
  expect(server.control.annotations[0]).toMatchObject({ anchor: { lid: selected.lid }, range: { start: selected.start, end: selected.end }, content: longSources[0].text.slice(selected.start, selected.end) });
  await expect(page.locator('.focus-screen:not([hidden]) .hl-mark')).toHaveCount(2);
  await selectAcross(page);
  await page.locator('.hl-popover').getByRole('button', { name: '笔记', exact: true }).click();
  const editor = page.locator('.note-editor-panel'); await expect(editor).toBeVisible();
  await editor.locator('textarea').fill('这份笔记草稿跨工具和重排保留。');
  await page.evaluate(() => window.getSelection()?.removeAllRanges());
  await tool(page, '阅读设置');
  await page.getByRole('slider', { name: '字号', exact: true }).fill('22');
  await page.getByRole('button', { name: '关闭阅读设置' }).click(); await settled(page);
  await page.getByRole('button', { name: '继续编辑笔记' }).click();
  await expect(editor.locator('textarea')).toHaveValue('这份笔记草稿跨工具和重排保留。');
  await editor.getByRole('button', { name: '保存笔记', exact: true }).click();
  await expect.poll(() => server.control.annotations.length).toBe(2);
  const note = server.control.annotations[1]; expect(note.selection_context.ranges).toEqual([{ lid: selected.lid, range: { start: selected.start, end: selected.end } }]);
  const marker = page.locator('.focus-screen:not([hidden]) .focus-annotation-marker').first(); await expect(marker).toBeVisible();
  await marker.click(); await page.getByRole('dialog', { name: '正文批注' }).getByRole('button', { name: '查看来源' }).click();
  await expect(page.getByRole('button', { name: '关闭来源预览' })).toBeVisible();
  const before = await snapshot(page); await page.getByRole('button', { name: '在阅读区打开' }).click(); await settled(page);
  expect((await snapshot(page)).current.start.offset).toBe(selected.start);
  await page.getByRole('button', { name: '返回阅读位置', exact: true }).filter({ visible: true }).first().click(); await settled(page);
  expect((await snapshot(page)).current.start).toMatchObject({ index: before.current.start.index, offset: before.current.start.offset });
  await selectAcross(page); await page.locator('.hl-popover').getByRole('button', { name: '问 AI', exact: true }).click();
  const input = page.getByRole('textbox', { name: '围绕当前阅读内容提问' }); await input.fill('为什么这个范围如此？');
  await page.getByRole('navigation', { name: '阅读工作区', exact: true }).getByRole('button', { name: '阅读', exact: true }).click();
  await tool(page, '问答'); await expect(input).toHaveValue('为什么这个范围如此？');
  await page.getByRole('button', { name: '发送', exact: true }).click();
  await expect.poll(() => server.calls.filter(c => c.path === '/agent/runs').length).toBe(1);
  const question = server.calls.find(c => c.path === '/agent/runs')!.body;
  expect(question.question_quote.ranges.every((r: any) => r.lid === '1.1')).toBe(true);
  expect(question.question_quote.resolved_quote).toBe(question.question_quote.ranges.map((r: any) => longSources[0].text.slice(r.range.start, r.range.end)).join(''));
  await save('operations', { selected, note, question, before, calls: server.calls });
});

test('selection owns reflow and prepared/animated pages cannot enter references', async ({ page }) => {
  await open(page, 'selection', longSources); const selected = await selectAcross(page), before = await snapshot(page);
  await page.setViewportSize({ width: 1024, height: 800 }); await page.waitForTimeout(150);
  expect((await snapshot(page)).current.id).toBe(before.current.id);
  expect(await page.evaluate(() => window.getSelection()?.getRangeAt(0).toString())).toBe(selected.text);
  await page.keyboard.press('ArrowRight'); expect((await snapshot(page)).current.id).toBe(before.current.id);
  await page.evaluate(() => window.getSelection()?.removeAllRanges()); await settled(page);
  await expect.poll(async () => (await snapshot(page)).current.id).not.toBe(before.current.id);
  const after = await snapshot(page); expect(after.current.start).toMatchObject({ index: before.current.start.index, offset: before.current.start.offset });
  await page.locator('.reader-pane').focus(); await page.keyboard.press('ArrowRight');
  const preview = await page.evaluate(() => Array.from(document.querySelectorAll<HTMLElement>('.focus-screen:not([hidden])')).every(el => el.inert));
  expect(preview).toBe(true); await settled(page);
  await save('selection-ownership', { before, after, selected, preview });
});

test('formula and partial code/table selection preserve object and canonical identity', async ({ page }) => {
  const sources = [{ lid: '1.1', kind: 'paragraph', text: '公式前的正文。'.repeat(80) }, { lid: '1.2', kind: 'formula', text: '$x^2 + y^2$' },
    { lid: '1.3', kind: 'paragraph', text: ' 后面的正文。' }, { lid: '1.4', kind: 'code', text: 'const value = 42;\nreturn value;' },
    { lid: '1.5', kind: 'table', text: '| 项目 | 数量 |\n| --- | --- |\n| 苹果 | 12 |' }];
  const server = await open(page, 'objects', sources); const before = await snapshot(page);
  await page.locator('.focus-screen:not([hidden]) [data-lid="1.2"] .katex-html').click();
  await expect(page.locator('.formula-dialog')).toBeVisible();
  await page.getByRole('button', { name: '关闭公式剖面' }).click(); expect((await snapshot(page)).current.id).toBe(before.current.id);
  for (const [lid, text] of [['1.4', 'value'], ['1.5', '苹果']]) {
    await page.locator(`.focus-screen:not([hidden]) [data-lid="${lid}"]`).evaluate((el, text) => {
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT); let node: Text | null = null;
      while (walker.nextNode()) if ((walker.currentNode as Text).data.includes(text)) { node = walker.currentNode as Text; break; }
      const start = node!.data.indexOf(text), range = document.createRange(); range.setStart(node!, start); range.setEnd(node!, start + text.length);
      const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range); document.dispatchEvent(new Event('selectionchange'));
    }, text);
    await page.locator('.hl-popover').getByRole('button', { name: '高亮', exact: true }).click();
    await expect.poll(() => server.control.annotations.some(r => r.content === text)).toBe(true);
  }
  await save('objects', { before, annotations: server.control.annotations, calls: server.calls });
});

test('only confirmed visible ranges persist, slow save never blocks turns and reload restores the character', async ({ page }) => {
  const server = await open(page, 'position', longSources);
  await expect.poll(() => server.control.position?.lid).toBe('1.1');
  server.control.positionDelay = 800;
  for (let i = 0; i < 3; i++) { await page.getByRole('button', { name: '下一屏', exact: true }).click(); await settled(page); }
  const at = await snapshot(page);
  await expect.poll(() => server.control.position?.text_position?.start, { timeout: 7000 }).toBe(at.current.start.offset);
  const position = server.control.position;
  expect(position.lid).toBe(at.current.start.lid ?? longSources[at.current.start.index].lid);
  const calls = server.calls.filter(c => c.path === '/reader/position');
  expect(server.calls.filter(c => c.path === '/reader/goto')).toHaveLength(0);
  for (const call of calls) expect(call.body.ranges.every((r: any) => r.end > r.start)).toBe(true);
  server.control.positionDelay = 0;
  await page.reload(); await page.waitForFunction(() => (window as any).fp4?.available());
  await expect.poll(async () => page.locator('.reader-pane [data-lid]').count()).toBeGreaterThan(0);
  await enter(page);
  expect((await snapshot(page)).current.start).toMatchObject({ index: at.current.start.index, offset: at.current.start.offset });
  await save('persistence', { at, position, calls, after: await snapshot(page) });
});

const dist = (values: number[]) => { const sorted = [...values].sort((a, b) => a - b); return { count: values.length, p95: sorted[Math.ceil(values.length * .95) - 1], p99: sorted[Math.ceil(values.length * .99) - 1], max: sorted.at(-1) }; };
function budget(state: any, width: number) {
  const limits = { screens: 4, pages: width <= 600 ? 4 : 8, sourceBytes: 524288, parsedBytes: 2097152, domNodes: 12000, decodedImagePixels: 8000000, preparationTasks: 1 };
  for (const [key, limit] of Object.entries(limits)) expect(state.peak[key], key).toBeLessThanOrEqual(limit);
}

for (const width of [1440, 390]) test(`P4 warm public typography and entry ${width}`, async ({ page, browser }) => {
  await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
  const server = await open(page); const cdp = await page.context().newCDPSession(page);
  let latest: { timestamp: number; data: string } | null = null, changed: (() => void) | null = null;
  cdp.on('Page.screencastFrame', (frame: any) => {
    latest = { timestamp: frame.metadata.timestamp * 1000, data: frame.data };
    void cdp.send('Page.screencastFrameAck', { sessionId: frame.sessionId }); changed?.();
  });
  await cdp.send('Page.startScreencast', { format: 'jpeg', quality: 80, everyNthFrame: 1 });
  const nextFrame = (after: number, before?: string) => new Promise<{ timestamp: number; data: string }>((resolve, reject) => {
    const timer = setTimeout(() => { changed = null; reject(new Error('No new readable browser frame')); }, 3000);
    changed = () => { if (latest && latest.timestamp >= after && latest.data !== before) { clearTimeout(timer); changed = null; resolve(latest); } }; changed();
  });
  const samples: any[] = [];
  const stopTrace = await startFrameTrace(page);
  const mark = async (sample: any) => page.evaluate(({sample, id}) => {
    performance.mark(`fp5-display-${id}-input`, {startTime: sample.startedAt-performance.timeOrigin});
    performance.mark(`fp5-display-${id}-ready`, {startTime: sample.readyAt-performance.timeOrigin});
  }, {sample, id:samples.length-1});
  await tool(page, '阅读设置');
  const coldFontStart = Date.now();
  await page.getByRole('combobox', { name: '字体', exact: true }).selectOption('sans'); await settled(page);
  const coldFontWaitMs = Date.now() - coldFontStart;
  await page.getByRole('combobox', { name: '字体', exact: true }).selectOption('serif'); await settled(page);
  for (let i = 0; i < 40; i++) {
    const kind = i < 20 ? 'font-size' : 'font-family';
    const before = (latest as { data: string } | null)?.data;
    const sample = await page.evaluate(async ({kind, value}) => {
      const original = document.querySelector<HTMLElement>('.focus-screen:not([hidden]):not([aria-hidden="true"])')!;
      const snapshot = (window as any).fp4.snapshot();
      const startedAt = performance.timeOrigin + performance.now();
      let oldStillVisible = true;
      const ready = new Promise<number>((resolve, reject) => {
        const timer = setTimeout(() => { observer.disconnect(); reject(new Error(`Reflow did not publish: ${JSON.stringify((window as any).fp4.snapshot())}`)); }, 10000);
        const observer = new MutationObserver(() => {
          const visible = document.querySelector('.focus-screen:not([hidden]):not([aria-hidden="true"])');
          if (!visible) oldStillVisible = false;
          if (visible && visible !== original) { clearTimeout(timer); observer.disconnect(); resolve(performance.timeOrigin + performance.now()); }
        }); observer.observe(document.querySelector('.focus-reader-surface')!, { attributes: true, subtree: true, childList: true });
      });
      const control = document.querySelector<HTMLInputElement | HTMLSelectElement>(kind === 'font-size' ? 'input[aria-label="字号"]' : 'select[aria-label="字体"]')!;
      control.value = value; control.dispatchEvent(new Event(kind === 'font-size' ? 'input' : 'change', { bubbles: true }));
      const readyAt = await ready;
      return { startedAt, readyAt, oldStillVisible, before: snapshot.current, after: (window as any).fp4.snapshot().current };
    }, {kind, value: kind === 'font-size' ? (i % 2 ? '19' : '22') : (i % 2 ? 'serif' : 'sans')});
    const frame = await nextFrame(sample.readyAt, before);
    samples.push({ kind, ...sample, firstReadableBrowserFrameMs: frame.timestamp - sample.startedAt });
    await mark(samples.at(-1)); await save(`p4-progress-${width}`, samples);
    expect(sample.oldStillVisible).toBe(true); expect(sample.after.start).toMatchObject({ index: sample.before.start.index, offset: sample.before.start.offset }); await settled(page);
  }
  await page.getByRole('button', { name: '关闭阅读设置' }).click();
  for (let i = 0; i < 20; i++) {
    const beforeFrame = (latest as { data: string } | null)?.data;
    await page.evaluate(() => {
      const before = (window as any).fp4.snapshot().current;
      const original = document.querySelector('.focus-screen:not([hidden]):not([aria-hidden="true"])');
      let oldStillVisible = true;
      const requestedAt = performance.timeOrigin + performance.now();
      let startedAt = requestedAt;
      window.addEventListener('resize', event => { startedAt = performance.timeOrigin + event.timeStamp; }, { once: true });
      (window as any).fp5DisplayChange = new Promise((resolve, reject) => {
        const timer = setTimeout(() => { observer.disconnect(); reject(new Error('Resize did not publish')); }, 10000);
        const observer = new MutationObserver(() => {
          const visible = document.querySelector('.focus-screen:not([hidden]):not([aria-hidden="true"])');
          if (!visible) oldStillVisible = false;
          if (visible && visible !== original) { clearTimeout(timer); observer.disconnect(); resolve({requestedAt, startedAt, readyAt:performance.timeOrigin+performance.now(), oldStillVisible, before, after:(window as any).fp4.snapshot().current}); }
        }); observer.observe(document.querySelector('.focus-reader-surface')!, { attributes:true, subtree:true, childList:true });
      });
    });
    await page.setViewportSize(i % 2 ? { width, height: width === 390 ? 844 : 900 } : width === 390 ? {width:844, height:390} : {width:720, height:900});
    const sample = await page.evaluate(() => (window as any).fp5DisplayChange);
    const frame = await nextFrame(sample.readyAt, beforeFrame);
    samples.push({kind:'spread', ...sample, firstReadableBrowserFrameMs:frame.timestamp-sample.startedAt});
    expect(sample.oldStillVisible).toBe(true); expect(sample.after.start).toMatchObject({ index: sample.before.start.index, offset: sample.before.start.offset }); await settled(page);
    await mark(samples.at(-1)); await save(`p4-progress-${width}`, samples);
  }
  for (let i = 0; i < 20; i++) {
    await tool(page, '退出专注'); await expect(page.locator('.focus-reader-body')).toBeHidden();
    const before = (latest as { data: string } | null)?.data;
    const sample = await page.evaluate(async () => {
      let oldStillVisible = true;
      const startedAt = performance.timeOrigin + performance.now();
      const ready = new Promise<number>((resolve, reject) => {
        const timer = setTimeout(() => { observer.disconnect(); reject(new Error(`Entry did not publish: ${JSON.stringify((window as any).fp4.snapshot())}`)); }, 10000);
        const observer = new MutationObserver(() => {
          const continuous = document.querySelector('.reader-pane > .prose'), visible = document.querySelector('.focus-screen:not([hidden]):not([aria-hidden="true"])');
          if (!continuous && !visible) oldStillVisible = false;
          if (visible) { clearTimeout(timer); observer.disconnect(); resolve(performance.timeOrigin + performance.now()); }
        }); observer.observe(document.querySelector('.reader-pane')!, { childList: true, attributes: true, subtree: true });
      });
      const button = Array.from(document.querySelectorAll<HTMLButtonElement>('button')).find(b => b.textContent?.trim() === '专注阅读' && b.getBoundingClientRect().width)!;
      button.click(); const readyAt = await ready;
      return { startedAt, readyAt, oldStillVisible };
    });
    const frame = await nextFrame(sample.readyAt, before); samples.push({ kind: 'entry', ...sample, firstReadableBrowserFrameMs: frame.timestamp - sample.startedAt });
    await mark(samples.at(-1)); await save(`p4-progress-${width}`, samples);
    expect(sample.oldStillVisible).toBe(true); await settled(page);
  }
  await cdp.send('Page.stopScreencast'); await cdp.detach();
  const traceFile = path.join(output, `display-${width}.trace.json.gz`);
  await stopTrace(traceFile);
  const events = JSON.parse(gunzipSync(await readFile(traceFile)).toString()).traceEvents;
  const shots = events.filter((e:any) => e.name === 'Screenshot' && e.args?.snapshot).sort((a:any,b:any) => a.ts-b.ts);
  for (const [i,sample] of samples.entries()) {
    const input = events.find((e:any) => e.name === `fp5-display-${i}-input`);
    const ready = events.find((e:any) => e.name === `fp5-display-${i}-ready`);
    const nextInput = events.find((e:any) => e.name === `fp5-display-${i+1}-input`)?.ts ?? Infinity;
    const before = shots.filter((e:any) => e.ts < input.ts).at(-1);
    const first = shots.find((e:any) => e.ts >= ready.ts && e.ts < nextInput && e.args.snapshot !== before?.args.snapshot);
    sample.screencastUpperBoundMs = sample.firstReadableBrowserFrameMs;
    // Some WebView2 builds stop the trace filmstrip at 450 pictures. The
    // independently acknowledged screencast above still supplies a real new
    // frame after ready; retain its conservative timestamp when that happens.
    sample.captureSource = first ? 'Tracing.Screenshot' : 'Page.screencastFrame';
    if (first) sample.firstReadableBrowserFrameMs = (first.ts-input.ts)/1000;
    sample.trace = {inputTs:input.ts, readyTs:ready.ts, readableTs:first?.ts ?? null};
  }
  const timing = Object.fromEntries(['font-size', 'font-family', 'spread', 'entry'].map(kind => [kind, dist(samples.filter(s => s.kind === kind).map(s => s.firstReadableBrowserFrameMs))]));
  await save(`p4-${width}`, { browser: browser.version(), width, coldFontWaitMs, samples, timing, final: await snapshot(page), calls: server.calls });
  for (const value of Object.values(timing)) expect(value.p95).toBeLessThanOrEqual(100);
});

test('P5 annotated reading 20 minutes and 500 effective turns', async ({ page, browser, browserName }) => {
  test.setTimeout(1_380_000);
  const width = browserName === 'webkit' ? 390 : 1440;
  await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
  const server = await backend(page);
  server.control.annotations = server.sources.filter((s: any, i: number) => i % 4 === 0 && s.kind === 'paragraph').map((s: any, i: number) => ({
    mem_id: `long-hl-${i}`, type: 'highlight', layer: 'long_term', book_id: 'complex', anchor: { lid: s.lid },
    range: { start: 0, end: Math.min(30, s.text.length) }, content: s.text.slice(0, 30),
  }));
  await page.goto(process.env.FOCUS_ENTRY_URL ?? '/playwright/fixtures/focus-turn.html'); await page.waitForFunction(() => (window as any).fp4?.available()); await enter(page);
  await tool(page, '笔记'); await page.locator('[data-mem-id="long-hl-0"] .note-source-button').click();
  await page.getByRole('button', { name: '在阅读区打开' }).click(); await settled(page);
  const initial = await snapshot(page);
  const initialFontSize = await page.locator('.focus-screen:not([hidden]) .focus-page').first().evaluate(el => String(parseFloat(getComputedStyle(el).fontSize)));
  const started = Date.now(), samples: any[] = [], windows: any[] = []; let direction = 1, turns = 0;
  while (Date.now() - started < 1_200_000 || turns < 500) {
    const before = await snapshot(page), current = before.current;
    if (direction > 0 && current.eof || direction < 0 && !current.start.index && !current.start.offset || turns === 160 || turns === 350) direction *= -1;
    await page.evaluate(name => (window as any).fp4.arm(name), `fp5-long-${turns}`);
    await page.getByRole('button', { name: direction > 0 ? '下一屏' : '上一屏', exact: true }).click(); await settled(page);
    const after = await snapshot(page); expect(after.current.id).not.toBe(before.current.id); expect(after.metrics.misses).toBe(0); budget(after, width);
    turns++;
    if (turns % 25 === 0 || turns === 1) {
      samples.push({ turns, elapsedMs: Date.now() - started, after });
      const readingLongTasks = await page.evaluate(() => {
        const probe = (window as any).fp4, first = probe.samples()[0].input;
        return probe.longTasks().filter((task: any) => task.start >= first);
      });
      await save('long-progress', { turns, elapsedMs: Date.now() - started, samples, readingLongTasks });
    }
    if (turns % 100 === 0) {
      await tool(page, '阅读设置'); await page.getByRole('slider', { name: '字号', exact: true }).fill(turns % 200 ? '22' : '19');
      await page.getByRole('button', { name: '关闭阅读设置' }).click(); await settled(page);
      windows.push({ turns, after: await snapshot(page) });
    }
    await waitAfterInput(page, 'fp4', 2000);
  }
  // Compare the same source and geometry even when the timed loop ends between
  // the alternating 100-turn typography changes.
  await tool(page, '阅读设置');
  await page.getByRole('slider', { name: '字号', exact: true }).fill(initialFontSize);
  await page.getByRole('button', { name: '关闭阅读设置' }).click(); await settled(page);
  // Return to the opening source through its saved annotation.
  await tool(page, '笔记');
  await page.locator('[data-mem-id="long-hl-0"] .note-source-button').click();
  await page.getByRole('button', { name: '在阅读区打开' }).click(); await settled(page);
  expect((await snapshot(page)).current.start.index).toBe(0);
  const final = await snapshot(page); budget(final, width);
  expect(await page.locator('.focus-screen:not([hidden]) .focus-page').first().evaluate(el => String(parseFloat(getComputedStyle(el).fontSize)))).toBe(initialFontSize);
  expect(final.current.start).toMatchObject({ index: initial.current.start.index, offset: initial.current.start.offset });
  expect(final.resources.domNodes).toBeLessThanOrEqual(initial.resources.domNodes);
  const interactions = await page.evaluate(() => (window as any).fp4.samples());
  await save('long-reading', { browser: browser.version(), width, initialFontSize, returnedFontSize: initialFontSize, longTasksSupported: await page.evaluate(() => PerformanceObserver.supportedEntryTypes.includes('longtask')), elapsedMs: Date.now() - started, turns, initial, samples, windows, final, interactions,
    longTasks: await page.evaluate(() => (window as any).fp4.longTasks()), positionWrites: server.calls.filter(c => c.path === '/reader/position') });
  for (const sample of interactions) for (const key of ['alignments', 'preparations', 'geometryProbes', 'decorations']) expect(sample.after[key]).toBe(sample.before[key]);
});
