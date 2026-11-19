import { test, expect } from '@playwright/test';
import { readFile, writeFile } from 'node:fs/promises';

// Opt-in production acceptance in a fresh browser context and reading workspace.
test.skip(!process.env.READER_NAV_LIVE_OUTPUT, 'Requires explicit live measurement destination');
test.use({ viewport: { width: 1440, height: 900 }, proxy: process.env.HTTPS_PROXY ? { server: process.env.HTTPS_PROXY } : undefined });
test('directory click to rendered destination on the published Reader', async ({ page, context }) => {
  test.setTimeout(180_000);
  const origin = 'https://understandbook.top';
  const credentials = JSON.parse(await readFile(process.env.READER_NAV_CREDENTIALS!, 'utf8'));
  await page.goto(origin + '/index.html', { waitUntil: 'domcontentloaded' });
  await page.getByLabel('邮箱或账号', { exact: true }).fill(credentials.username);
  await page.getByLabel('密码', { exact: true }).fill(credentials.password);
  await page.getByRole('button', { name: '登录', exact: true }).click();
  await expect(page.locator('.network-book-card').first()).toBeVisible({ timeout: 30_000 });
  const get = (path: string) => page.evaluate(async path => {
    const response = await fetch(path); if (!response.ok) throw new Error(`Read failed: ${response.status}`); return response.json();
  }, path);
  const identity = await get('/api/auth/me');
  const catalog = await get('/api/library');
  const book = catalog.books.find((b: any) => b.published_book_ref.book_id === 'ai-agent-engineering');
  const ref = book.published_book_ref;
  const manifest = await get(`/api/books/${ref.book_id}/publications/${ref.publication_id}/manifest`);
  const outline = manifest.tree.filter((n: any) => n.children.length || ['chapter', 'section'].includes(n.kind));
  const leaves = manifest.tree.filter((n: any) => !n.children.length);
  const calls: { path: string; method: string; at: number }[] = [];
  const errors: string[] = [];
  let attachment = '', workspaceId = '';
  page.on('pageerror', e => errors.push(e.message));
  page.on('request', r => {
    const path = new URL(r.url()).pathname;
    if (path.startsWith('/api/')) calls.push({ path, method: r.method(), at: Date.now() });
    if (path === '/api/workspaces' && r.method() === 'POST') attachment = r.postDataJSON().attachment_id;
  });
  const result: any = { at: new Date().toISOString(), publication: ref, viewport: '1440x900', proxy: !!process.env.HTTPS_PROXY, metric: 'click event to destination DOM visible + two animation frames', samples: [] };
  try {
    const created = page.waitForResponse(r => new URL(r.url()).pathname === '/api/workspaces' && r.request().method() === 'POST');
    await page.locator('.network-book-card').filter({ hasText: book.title ?? ref.book_id }).click();
    workspaceId = (await (await created).json()).workspace_id;
    await expect(page.locator('.outline-item')).toHaveCount(outline.length, { timeout: 45_000 });
    await expect(page.locator('.prose [data-lid]').first()).toBeVisible({ timeout: 45_000 });
    for (let i = 0; i < Number(process.env.READER_NAV_ITERATIONS ?? 6); i++) {
      const target = ['1.4.9.13', '1.8.14'][i % 2];
      const leaf = leaves.find((n: any) => n.lid === target || n.lid.startsWith(target + '.')).lid;
      const button = page.locator('.outline-item').nth(outline.findIndex((n: any) => n.lid === target));
      await button.scrollIntoViewIfNeeded();
      await page.evaluate(({ leaf }) => {
        (window as any).navigationTiming = null;
        document.addEventListener('click', function measure(event) {
          if (!(event.target as Element).closest('.outline-item')) return;
          document.removeEventListener('click', measure, true);
          const start = performance.now();
          (window as any).navigationStarted = start;
          const frame = () => {
            const el = document.querySelector<HTMLElement>(`.reader-pane .prose [data-lid="${leaf}"]`);
            const pane = document.querySelector('.reader-pane')?.getBoundingClientRect();
            const box = el?.getBoundingClientRect();
            if (el?.textContent?.trim() && pane && box && box.bottom > pane.top && box.top < pane.bottom && box.height > 0) {
              requestAnimationFrame(() => requestAnimationFrame(() => { (window as any).navigationTiming = performance.now() - start; }));
            } else requestAnimationFrame(frame);
          };
          requestAnimationFrame(frame);
        }, true);
      }, { leaf });
      const before = calls.length;
      await button.click();
      await page.waitForFunction(() => (window as any).navigationTiming !== null, null, { timeout: 20_000 });
      const ms = await page.evaluate(() => (window as any).navigationTiming);
      const requests = await page.evaluate(() => performance.getEntriesByType('resource').filter(e => e.startTime >= (window as any).navigationStarted && e.name.endsWith('/reader/goto')).map(e => ({
        start: e.startTime - (window as any).navigationStarted,
        end: e.startTime + e.duration - (window as any).navigationStarted,
        ttfb: (e as PerformanceResourceTiming).responseStart - e.startTime,
        duration: e.duration,
      })));
      await expect(button).toHaveAttribute('aria-current', 'location');
      result.samples.push({ iteration: i, target, leaf, ms: Math.round(ms * 10) / 10, navigation_requests: requests, calls: calls.slice(before) });
    }
    result.scriptErrors = errors;
    expect(errors).toEqual([]);
    result.status = 'passed';
  } finally {
    // Preserve measured failures/results even if the network drops during cleanup.
    await writeFile(process.env.READER_NAV_LIVE_OUTPUT!, JSON.stringify(result, null, 2));
    // Detach only this test's workspace; leave every pre-existing workspace intact.
    try {
      if (workspaceId) {
        const w = await get('/api/workspaces/' + workspaceId);
        expect(await page.evaluate(async ({workspaceId, attachment, w, csrf}) => (await fetch('/api/workspaces/' + workspaceId + '/detach', { method: 'POST', headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': csrf }, body: JSON.stringify({ attachment_id: attachment, generation: w.generation, expected_revision: w.revision }) })).ok,
          { workspaceId, attachment, w, csrf: identity.csrf_token })).toBe(true);
      }
      expect(await page.evaluate(async csrf => (await fetch('/api/auth/logout', { method: 'POST', headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': csrf }, body: '{}' })).ok, identity.csrf_token)).toBe(true);
      result.cleanup = 'passed';
    } catch {
      result.cleanup = 'failed';
    }
    await writeFile(process.env.READER_NAV_LIVE_OUTPUT!, JSON.stringify(result, null, 2));
  }
});
