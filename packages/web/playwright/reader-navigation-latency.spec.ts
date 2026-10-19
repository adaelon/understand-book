import { test, expect } from '@playwright/test';
import { backend } from './focus-app-backend';
let pageErrors: string[];
test.beforeEach(async ({ page }) => { pageErrors = []; page.on('pageerror', error => pageErrors.push(error.message)); });

test.afterEach(async ({ page }, info) => {
  if (info.status !== info.expectedStatus) console.log('navigation-state', await page.evaluate(() => ({
    focus: document.querySelector('.workspace-shell')?.getAttribute('data-focus-reading'),
    state: (window as any).fp4?.snapshot(),
    error: document.querySelector('.error')?.textContent,
  })));
  expect(pageErrors).toEqual([]);
});

const sources = Array.from({ length: 48 }, (_, i) => ({ parent: `1.${Math.floor(i / 16) + 1}`,
  lid: `1.${Math.floor(i / 16) + 1}.${i % 16 + 1}`, kind: 'paragraph', text: `原文第${i + 1}段。` + '目录跳转后应保持正确的原文位置。'.repeat(100) }));
const snapshot = (page: any) => page.evaluate(() => (window as any).fp4.snapshot());
const settled = async (page: any) => expect.poll(async () => {
  const s = await snapshot(page); return s?.phase === 'ready' && !s.preparing && s.turn.phase === 'idle';
}).toBe(true);

test('cold directory navigation uses bundled text, warm return reuses it, and late clicks cannot replace the last target', async ({ page }) => {
  const server = await backend(page, 'navigation', undefined, sources);
  await page.goto('/playwright/fixtures/focus-turn.html');
  await expect(page.locator('.prose [data-lid="1.1.1"]')).toBeAttached();
  server.control.gotoDelays['1.2'] = 700;
  const before = server.calls.length;
  await page.getByRole('button', { name: 'Chapter 1.2' }).click();
  await expect(page.getByRole('status').filter({ hasText: '正在跳转' })).toBeVisible();
  await page.getByRole('button', { name: 'Chapter 1.3' }).click();
  await expect(page.locator('.prose [data-lid="1.3.1"]')).toBeAttached();
  await expect(page.locator('.outline-item[aria-current="location"]')).toHaveAttribute('title', 'Chapter 1.3');
  // Wait for the deliberately older response, then prove it cannot restore chapter 2.
  await page.waitForTimeout(850);
  await expect(page.locator('.prose [data-lid="1.2.1"]')).toHaveCount(0);
  // Neighbor prefetch and the auxiliary chapter summary may read independently;
  // the confirmed destination itself must use the bundled response.
  expect(server.calls.slice(before).filter(c => /^\/book\/text\?lid=1\.[23]\.1(&|$)/.test(c.path))).toHaveLength(0);
  await page.getByRole('button', { name: 'Chapter 1.1' }).click();
  await expect(page.locator('.prose [data-lid="1.1.1"]')).toBeAttached();
  expect(server.calls.filter(c => c.path === '/reader/goto').at(-1)?.body.include_text).toBeUndefined();
});

test('a slow previous position save does not block a focus jump, overwrite it, or break reload resumption', async ({ page }) => {
  const server = await backend(page, 'navigation-focus', undefined, sources);
  await page.goto('/playwright/fixtures/focus-turn.html');
  await expect(page.locator('.prose [data-lid="1.1.1"]')).toBeAttached();
  await page.waitForFunction(() => (window as any).fp4.ready());
  await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().click();
  await settled(page);
  await expect(page.locator('.focus-screen:not([hidden])')).toHaveCSS('background-color', 'rgb(255, 255, 255)');
  await expect(page.getByRole('button', { name: '展开工具栏', exact: true })).toHaveCount(0);
  await page.screenshot({ path: '../../tmp/reader-navigation-20261016/focus-white-desktop.png' });
  await expect.poll(() => server.control.position?.lid).toBe('1.1.1');
  server.control.positionDelay = 1800;
  const savesBefore = server.calls.filter(c => c.path === '/reader/position').length;
  await page.getByRole('button', { name: '下一屏', exact: true }).click();
  await settled(page);
  await expect.poll(() => server.calls.filter(c => c.path === '/reader/position').length).toBeGreaterThan(savesBefore);
  const oldSavedAt = server.control.positionSavedAt;
  await page.getByRole('button', { name: '阅读工具', exact: true }).click();
  await page.getByRole('navigation', { name: '专注阅读工具' }).getByRole('button', { name: '目录', exact: true }).click();
  await page.getByRole('button', { name: 'Chapter 1.2' }).click();
  await page.getByRole('button', { name: 'Chapter 1.3' }).click();
  await expect.poll(async () => (await snapshot(page))?.current?.start?.index, { timeout: 1300 }).toBe(32);
  expect(server.control.positionSavedAt).toBe(oldSavedAt);
  await expect.poll(() => server.control.position?.lid, { timeout: 6000 }).toBe('1.3.1');
  expect((await snapshot(page)).current.start.index).toBe(32);
  expect(server.calls.filter(c => c.path === '/reader/goto')).toHaveLength(0);
  server.control.positionDelay = 0;
  await page.reload();
  await expect(page.locator('.prose [data-lid="1.3.1"]')).toBeAttached();
  await page.waitForFunction(() => (window as any).fp4.ready());
  await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().click();
  await settled(page);
  expect((await snapshot(page)).current.start.index).toBe(32);
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator('.focus-screen:not([hidden]) .focus-page')).toHaveCount(1);
  await settled(page);
  await expect(page.locator('.focus-screen:not([hidden])')).toHaveCSS('background-color', 'rgb(255, 255, 255)');
  await page.getByRole('button', { name: '阅读工具', exact: true }).click();
  await expect(page.getByRole('navigation', { name: '专注阅读工具' })).toBeVisible();
  await page.screenshot({ path: '../../tmp/reader-navigation-20261016/focus-white-mobile.png' });
  await page.getByRole('button', { name: '退出专注', exact: true }).filter({ visible: true }).click();
  await expect(page.locator('.workspace-shell')).toHaveAttribute('data-focus-reading', 'false');
  await expect(page.locator('.prose .flow-text[data-lid="1.3.1"]')).toBeVisible();
});

test('an old edge load cannot append its paragraphs after a directory jump', async ({ page }) => {
  const server = await backend(page, 'navigation-edge', undefined, sources);
  await page.goto('/playwright/fixtures/focus-turn.html');
  await expect(page.locator('.prose [data-lid="1.1.1"]')).toBeAttached();
  await page.waitForFunction(() => (window as any).fp4.ready());
  server.control.delay = 900;
  await page.locator('.reader-pane').evaluate(el => { el.scrollTop = el.scrollHeight; });
  await expect.poll(() => server.calls.some(c => c.path.startsWith('/book/text?lid=1.1.9'))).toBe(true);
  await page.getByRole('button', { name: 'Chapter 1.3' }).click();
  await expect(page.locator('.prose [data-lid="1.3.1"]')).toBeAttached();
  await page.waitForTimeout(1000);
  await expect(page.locator('.prose [data-lid="1.1.9"]')).toHaveCount(0);
});
