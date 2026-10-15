import { expect, test, type Page } from '@playwright/test';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const output = path.resolve(process.env.FP3_OUTPUT ?? '../../docs/performance/focus-reading-fp3-20261014/results');
test.beforeAll(() => mkdir(output, { recursive: true }));
const save = (name: string, value: unknown) => writeFile(path.join(output, `${name}.json`), JSON.stringify(value, null, 2));
async function backend(page: Page, material = 'complex', chunk?: string) {
  const chunks = chunk ? [chunk] : Array.from({ length: 10 }, (_, i) => String(i));
  const sources = (await Promise.all(chunks.map(c => readFile(path.resolve(`../../tmp/focus-reading-fp1/public/fp1/${material}/${c}.json`), 'utf8').then(JSON.parse)))).flat();
  let offset = 0;
  const tree = sources.map((s: any) => { const start = offset; offset += s.text.length + 2; return { lid: s.lid, kind: s.kind, children: [], span: { start, end: offset - 2 } }; });
  const source = sources.map((s: any) => s.text).join('\n\n');
  const calls: { method: string; path: string; at: number }[] = [];
  const control = { offline: false, delay: 0, formulaDelay: 0, revision: 'fp3-source-1' };
  await page.route('**/api/**', async route => {
    const request = route.request(), url = new URL(request.url()), p = url.pathname.slice(4);
    calls.push({ method: request.method(), path: p + url.search, at: Date.now() });
    const answer = (value: unknown, status = 200) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(value) }).catch(() => {});
    if (p === '/book/text') {
      if (control.offline) return route.abort();
      if (control.delay) await new Promise(r => setTimeout(r, control.delay));
      const lid = url.searchParams.get('lid'), end = url.searchParams.get('end') ?? lid;
      const a = tree.find((n: any) => n.lid === lid), b = tree.find((n: any) => n.lid === end);
      if (!a || !b) return answer({ error_code: 'NOT_FOUND' }, 404);
      return answer({ lid, text: source.slice(a.span.start, b.span.end) });
    }
    if (p === '/book/formula_semantics') { if (control.formulaDelay) await new Promise(r => setTimeout(r, control.formulaDelay)); return answer({ error_code: 'NOT_FOUND' }, 404); }
    const first = tree[0].lid;
    const data: Record<string, unknown> = {
      '/desktop/status': { desktop_host: false, reader_only: true, active_book: true },
      '/book/build_workbench': { book_id: material, readiness: { route: 'reader', reasons: [], stages: {} }, input: { manifest: null }, source_review: { unresolved: [] } },
      '/book/manifest': { tree, stats_by_lid: {} },
      '/book/asset_manifest': { images: sources.map((s: any) => s.imageAsset).filter(Boolean) },
      '/book/source_fingerprint': { book_id: material, source_fingerprint: control.revision },
      '/book/source_manifest': null,
      '/reader/state': { book_id: material, revision: 0, viewport: { anchor_lid: first, top_lid: first, bottom_lid: tree[7]?.lid ?? first, width: 8, visible_lids: tree.slice(0, 8).map((n: any) => n.lid) }, profile: { profile_id: 'technical_learning', profile_version: '1' }, layout: { rev: 0, open_slots: [] } },
      '/profile/manifest': { profile_id: 'technical_learning', slots: [] },
      '/memory/recall': [], '/memory/profile': {}, '/profile/memory': {}, '/profile/backfill': { jobs: [], sessions: [] },
      '/agent/history': { active_session_id: 'fp3-chat', sessions: [], current: { id: 'fp3-chat', book_id: material, turns: [], goals: [] } },
      '/build_intent/artifacts': { overlay: null }, '/build_intent/usage.event': {},
      '/tutor/state': { control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} },
      '/tutor/readiness': { status: 'ready', limitations: [], reason: 'ready', teaching_assets: { status: 'ready', limitations: [] } },
    };
    return p in data ? answer(data[p]) : answer({ error_code: 'NOT_FOUND', message: p }, 404);
  });
  // Real imported external image URLs can be unavailable. No external network
  // dependency in acceptance; the product must keep a stable missing-image box.
  await page.route(/^https?:\/\/(?!127\.0\.0\.1)/, route => route.abort());
  return { control, calls, sources };
}
async function open(page: Page, material = 'complex', chunk?: string) {
  page.on('pageerror', error => console.log('App page error:', error.message));
  const server = await backend(page, material, chunk);
  await page.goto('/playwright/fixtures/focus-ready.html');
  await page.waitForFunction(() => (window as any).fp3?.available(), undefined, { timeout: 15000 });
  await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().click({ timeout: 15000 });
  await page.evaluate(() => (window as any).fp3.activate());
  await expect.poll(() => page.evaluate(() => (window as any).fp3.snapshot()?.phase)).toBe('ready');
  return server;
}
const snapshot = (page: Page) => page.evaluate(() => (window as any).fp3.snapshot());
async function settled(page: Page) { await expect.poll(async () => (await snapshot(page))?.preparing).toBe(false); }
async function settings(page: Page) {
  if (!await page.getByRole('button', { name: '阅读设置', exact: true }).first().isVisible()) {
    const expand = page.getByRole('button', { name: '展开工具栏', exact: true });
    if (await expand.isVisible()) await expand.click();
    else await page.getByRole('button', { name: '菜单', exact: true }).filter({ visible: true }).first().click({ timeout: 5000 });
  }
  await page.getByRole('button', { name: '阅读设置', exact: true }).filter({ visible: true }).first().click({ timeout: 5000 });
}
function budget(s: any) {
  for (const r of [s.resources, s.peak]) {
    expect(r.screens).toBeLessThanOrEqual(4); expect(r.pages).toBeLessThanOrEqual(8);
    expect(r.sourceBytes).toBeLessThanOrEqual(524288); expect(r.parsedBytes).toBeLessThanOrEqual(2097152);
    expect(r.domNodes).toBeLessThanOrEqual(12000); expect(r.decodedImagePixels).toBeLessThanOrEqual(8000000);
    expect(r.preparationTasks).toBeLessThanOrEqual(1);
  }
  expect(s.boundaryCount).toBeLessThanOrEqual(128);
}
function noShowWork(sample: any) {
  for (const key of ['alignments', 'preparations', 'geometryProbes']) expect(sample.after.work[key]).toBe(sample.before.work[key]);
}

test('App readiness, bounded source ownership and show work', async ({ page }) => {
  const { calls } = await open(page); await settled(page);
  const baseline = calls.length, samples = [];
  for (let i = 0; i < 12; i++) {
    const sample = await page.evaluate(() => (window as any).fp3.move(1)); samples.push(sample);
    expect(sample.ready).toBe(true); noShowWork(sample); budget(sample.after);
    await page.waitForTimeout(1000);
  }
  expect(calls.slice(baseline).filter(c => c.method !== 'GET')).toEqual([]);
  expect(calls.filter(c => c.path.includes('formula_semantics'))).toEqual([]);
  await save('smoke', { samples, calls }); await page.screenshot({ path: path.join(output, 'app-ready.png') });
});

for (const width of [1440, 390]) for (const weak of [false, true]) test(`normal 200 turns ${width} ${weak ? 'weak' : 'local'}`, async ({ page, browser }) => {
  await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
  const server = await open(page); await settled(page);
  const cdp = await page.context().newCDPSession(page);
  if (weak) { server.control.delay = 100; await cdp.send('Network.enable'); await cdp.send('Network.emulateNetworkConditions', { offline: false, latency: 100, downloadThroughput: 10_000_000 / 8, uploadThroughput: 10_000_000 / 8 }); }
  const at = Date.now(), samples = []; let direction = 1;
  for (let i = 0; i < 200; i++) {
    const current = (await snapshot(page)).current;
    if ((direction > 0 && current.eof) || (direction < 0 && current.start.index === 0 && current.start.offset === 0) || i === 65 || i === 125) direction *= -1;
    const sample = await page.evaluate(d => (window as any).fp3.move(d), direction); samples.push(sample);
    if (i % 20 === 0 || !sample.ready) await save(`normal-progress-${width}-${weak ? 'weak' : 'local'}`, { turns: i + 1, samples });
    budget(sample.after); noShowWork(sample);
    await page.waitForTimeout(Math.max(1, at + (i + 1) * 1000 - Date.now()));
  }
  await save(`normal-${width}-${weak ? 'weak' : 'local'}`, { browser: browser.version(), width, weak, elapsedMs: Date.now() - at, samples, calls: server.calls, longTasks: await page.evaluate(() => (window as any).fp3.longTasks()) });
  expect(samples.filter(s => !s.ready)).toEqual([]);
  await cdp.detach();
});

test('offline keeps prepared pages and preserves the old picture for an unavailable target', async ({ page }) => {
  const { control } = await open(page); await settled(page);
  control.offline = true;
  let missed: any;
  for (let i = 0; i < 30; i++) {
    const result = await page.evaluate(() => (window as any).fp3.move(1));
    if (!result.ready) { missed = result; break; }
    noShowWork(result); await page.waitForTimeout(60);
  }
  expect(missed).toBeTruthy(); expect(missed.after.current).toEqual(missed.before.current);
  await expect.poll(async () => (await snapshot(page)).phase).toBe('error');
  await expect(page.locator('.focus-screen:not([hidden])')).toBeVisible();
  const previous = await page.evaluate(() => (window as any).fp3.move(-1)); expect(previous.ready).toBe(true);
  control.offline = false;
  await save('offline', { missed, previous, state: await snapshot(page) });
});

test('delayed body is superseded by the latest target and hidden preparation stops', async ({ page }) => {
  const { control } = await open(page); await settled(page);
  const old = (await snapshot(page)).current;
  control.delay = 700;
  await page.evaluate(() => (window as any).fp3.activate({ index: 180, offset: 0 }));
  await page.waitForTimeout(100);
  expect((await snapshot(page)).current).toEqual(old);
  await page.evaluate(() => (window as any).fp3.activate({ index: 80, offset: 0 }));
  await expect.poll(async () => (await snapshot(page)).current.start.index, { timeout: 15000 }).toBe(80);
  const current = (await snapshot(page)).current;
  await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, value: true }); document.dispatchEvent(new Event('visibilitychange')); });
  await page.waitForTimeout(1000);
  const hidden = await snapshot(page); expect(hidden.preparing).toBe(false); expect(hidden.current).toEqual(current);
  await page.evaluate(() => { delete (document as unknown as { hidden?: boolean }).hidden; document.dispatchEvent(new Event('visibilitychange')); });
  control.delay = 0;
  await save('body-cancellation-hidden', { old, current, hidden, final: await snapshot(page) });
});

test('retry resumes a failed background prefetch while the current screen stays readable', async ({ page }) => {
  const { control } = await open(page); await settled(page);
  control.offline = true;
  for (let i = 0; i < 12; i++) {
    expect(await page.evaluate(() => (window as any).fp3.move(1))).toMatchObject({ ready: true });
    await page.waitForTimeout(150);
    if ((await snapshot(page)).error) break;
  }
  const failed = await snapshot(page);
  expect(failed.phase).toBe('ready'); expect(failed.error).not.toBeNull();
  control.offline = false;
  await page.locator('.focus-ready-status').getByRole('button', { name: '重试' }).click();
  await expect.poll(async () => (await snapshot(page)).error).toBeNull(); await settled(page);
  expect((await snapshot(page)).current).toEqual(failed.current);
  await save('background-retry', { failed, recovered: await snapshot(page) });
});

test('cold font keeps old pages, obsolete font work cancels and gesture preparation pauses', async ({ page }) => {
  await open(page); await settled(page);
  const old = (await snapshot(page)).current;
  let release!: () => void, held = 0;
  const gate = new Promise<void>(resolve => { release = resolve; });
  await page.route('**/*noto-sans*.woff2', async route => { held++; await gate; await route.continue().catch(() => {}); });
  await settings(page);
  await page.getByLabel('字体', { exact: true }).selectOption('sans');
  await expect.poll(() => held).toBeGreaterThan(0);
  expect((await snapshot(page)).current).toEqual(old);
  const moved = await page.evaluate(() => (window as any).fp3.move(1));
  noShowWork(moved);
  await page.waitForTimeout(300);
  expect((await snapshot(page)).phase).toBe('waiting');
  await page.getByLabel('字体', { exact: true }).selectOption('serif');
  await expect.poll(async () => (await snapshot(page)).current.id).not.toBe(old.id);
  release();
  await page.getByRole('button', { name: '关闭阅读设置' }).click(); await settled(page);
  await page.evaluate(() => (window as any).fp3.pause(true));
  const before = await snapshot(page);
  await page.setViewportSize({ width: 1024, height: 900 }); await page.waitForTimeout(150);
  expect((await snapshot(page)).work.preparations).toBe(before.work.preparations);
  expect((await snapshot(page)).current).toEqual(before.current);
  await page.evaluate(() => (window as any).fp3.pause(false));
  await expect.poll(async () => (await snapshot(page)).current.id).not.toBe(before.current.id);
  await settled(page);
  const reflowed = await snapshot(page);
  expect(reflowed.current.start).toEqual(before.current.start);
  const backward = await page.evaluate(() => (window as any).fp3.move(-1));
  expect(backward.ready).toBe(true); expect(backward.after.current.end).toMatchObject({ index: reflowed.current.start.index, offset: reflowed.current.start.offset });
  const forward = await page.evaluate(() => (window as any).fp3.move(1));
  expect(forward.ready).toBe(true); expect(forward.after.current.start).toEqual(reflowed.current.start);
  await save('font-and-pause', { old, before, final: await snapshot(page) });
});

for (const material of ['epub', 'mixed']) test(`real ${material} source and unavailable external pictures`, async ({ page }) => {
  const { sources } = await open(page, material); await settled(page);
  const samples = [];
  for (let i = 0; i < 20; i++) {
    const current = (await snapshot(page)).current;
    const sample = await page.evaluate(d => (window as any).fp3.move(d), current.eof || i >= 10 ? -1 : 1);
    samples.push(sample); expect(sample.ready).toBe(true); noShowWork(sample); budget(sample.after);
    await page.waitForTimeout(1000);
  }
  if (material === 'mixed') {
    const index = sources.findIndex((s: any) => s.kind === 'image'); expect(index).toBeGreaterThanOrEqual(0);
    await page.evaluate(index => (window as any).fp3.activate({ index, offset: 0 }), index);
    await expect.poll(async () => (await snapshot(page)).phase).toBe('ready');
    await expect(page.locator('.focus-screen:not([hidden]) .focus-image-unavailable').first()).toBeVisible();
    const before = await page.locator('.focus-screen:not([hidden]) .focus-image-unavailable').first().boundingBox();
    await page.waitForTimeout(200);
    expect(await page.locator('.focus-screen:not([hidden]) .focus-image-unavailable').first().boundingBox()).toEqual(before);
  }
  await save(`material-${material}`, { samples, final: await snapshot(page) });
});

test('delayed image decode gates readiness and failed images settle without late shifts', async ({ page }) => {
  await page.addInitScript(() => {
    const original = HTMLImageElement.prototype.decode;
    let release!: () => void;
    const gate = new Promise<void>(r => { release = r; });
    Object.assign(window, { releaseDecode: release, decodeCalls: 0 });
    HTMLImageElement.prototype.decode = async function () {
      (window as any).decodeCalls++; await gate; return original.call(this);
    };
  });
  await backend(page, 'complex', 'images');
  await page.goto('/playwright/fixtures/focus-ready.html');
  await page.waitForFunction(() => (window as any).fp3?.available());
  await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().click();
  await page.evaluate(() => (window as any).fp3.activate());
  await expect.poll(() => page.evaluate(() => (window as any).decodeCalls)).toBeGreaterThan(0);
  expect((await snapshot(page)).current).toBeNull();
  await page.evaluate(() => (window as any).releaseDecode());
  await expect.poll(async () => (await snapshot(page)).phase).toBe('ready'); await settled(page);
  const dimensions = await page.locator('.focus-screen:not([hidden]) img').evaluateAll(imgs => imgs.map(el => { const img = el as HTMLImageElement; return { width: img.naturalWidth, height: img.naturalHeight, rendered: img.getBoundingClientRect().height }; }));
  for (const image of dimensions) { expect(image.width).toBeGreaterThan(0); expect(image.rendered).toBeGreaterThan(0); }
  budget(await snapshot(page)); await save('image-decode', { dimensions, state: await snapshot(page) });
});

test('explanations stay on demand and source version replacement disposes the old host', async ({ page }) => {
  const { control, calls } = await backend(page);
  control.formulaDelay = 1500;
  await page.goto('/playwright/fixtures/focus-ready.html');
  await page.waitForFunction(() => (window as any).fp3?.available());
  expect(calls.filter(c => c.path.includes('formula_semantics'))).toEqual([]);
  await page.getByTitle('查看公式语义剖面').first().click();
  await expect(page.getByText('正在读取公式解释…')).toBeVisible();
  await page.getByRole('button', { name: '关闭公式剖面' }).click();
  await page.evaluate(() => (window as any).fp3.activate());
  await expect.poll(async () => (await snapshot(page)).phase).toBe('ready');
  await page.locator('.focus-reader-body').evaluate(el => el.setAttribute('data-old-host', 'yes'));
  control.revision = 'fp3-source-2';
  await page.evaluate(() => window.dispatchEvent(new Event('pageshow')));
  await expect(page.locator('[data-old-host]')).toHaveCount(0);
  await page.waitForFunction(() => (window as any).fp3?.available());
  await expect(page.locator('.reader-pane .flow-paragraph').first()).toBeVisible();
  await expect(page.locator('.formula-dialog')).toHaveCount(0);
  await save('formula-and-version', { calls });
});

test('mobile weak startup task attribution', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const server = await open(page); await settled(page);
  const cdp = await page.context().newCDPSession(page);
  const traceEvents: any[] = [];
  cdp.on('Tracing.dataCollected', data => traceEvents.push(...data.value));
  await cdp.send('Tracing.start', { categories: 'devtools.timeline,v8,blink.user_timing,disabled-by-default-devtools.timeline', transferMode: 'ReportEvents' });
  server.control.delay = 100;
  await cdp.send('Network.enable');
  await cdp.send('Network.emulateNetworkConditions', { offline: false, latency: 100, downloadThroughput: 10_000_000 / 8, uploadThroughput: 10_000_000 / 8 });
  const marker = await page.evaluate(() => { performance.mark('fp3-attribution-start'); return performance.now(); });
  const samples = [], at = Date.now();
  for (let i = 0; i < 20; i++) {
    const sample = await page.evaluate(() => (window as any).fp3.move(1)); samples.push(sample);
    expect(sample.ready).toBe(true); noShowWork(sample); budget(sample.after);
    await page.waitForTimeout(Math.max(1, at + (i + 1) * 1000 - Date.now()));
  }
  const finished = new Promise<void>(resolve => cdp.once('Tracing.tracingComplete', () => resolve()));
  await cdp.send('Tracing.end'); await finished;
  await save('mobile-weak-task-attribution', { marker, samples, longTasks: await page.evaluate(() => (window as any).fp3.longTasks()), traceEvents });
  await cdp.detach();
});

test('long reading 20 minutes and 500 App turns', async ({ page, browser }) => {
  test.skip(process.env.FP3_LONG !== '1', 'Wall-clock acceptance'); test.setTimeout(1_400_000);
  const { calls } = await open(page); await settled(page);
  const at = Date.now(), samples = []; let direction = 1, turns = 0;
  while (Date.now() - at < 1_200_000 || turns < 500) {
    const s = await snapshot(page);
    if ((direction > 0 && s.current.eof) || (direction < 0 && s.current.start.index === 0 && s.current.start.offset === 0)) direction *= -1;
    const result = await page.evaluate(d => (window as any).fp3.move(d), direction);
    if (result.after.current.id !== result.before.current.id) turns++;
    budget(result.after); noShowWork(result);
    if (turns % 25 === 0 || turns === 1) {
      samples.push({ turns, elapsedMs: Date.now() - at, state: await snapshot(page) });
      await save('long-progress', { turns, elapsedMs: Date.now() - at, samples });
    }
    if (turns && turns % 100 === 0) {
      await settings(page);
      await page.getByLabel('字号', { exact: true }).evaluate((el, value) => { (el as HTMLInputElement).value = value; el.dispatchEvent(new Event('input', { bubbles: true })); }, turns % 200 ? '22' : '19');
      await page.getByRole('button', { name: '关闭阅读设置' }).click();
    }
    await page.waitForTimeout(2000);
  }
  await save('long-reading', { browser: browser.version(), turns, elapsedMs: Date.now() - at, samples, calls, final: await snapshot(page), longTasks: await page.evaluate(() => (window as any).fp3.longTasks()) });
});
