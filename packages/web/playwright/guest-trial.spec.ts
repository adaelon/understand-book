import { expect, test, type BrowserContext } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

// Uses the real Rust fixture and the multi-user production frontend. The proxy
// only supplies HTTPS/Host routing; identity, cookies and persistence are real.
const origin = 'https://reader.example';
const output = resolve('../../tmp/guest-trial-20261027');
async function proxy(context: BrowserContext) {
  await context.route(`${origin}/**`, async route => {
    const request = route.request(), url = new URL(request.url());
    const api = url.pathname.startsWith('/api/');
    const response = await route.fetch({
      url: `http://127.0.0.1:${api ? 4196 : 4195}${url.pathname}${url.search}`,
      headers: { ...await request.allHeaders(), host: api ? 'reader.example' : '127.0.0.1:4195', origin },
    });
    await route.fulfill({ response });
  });
}

test('real invite redemption, balance, refresh, used-code error and mobile English flow', async ({ page, context, browser }) => {
  mkdirSync(output, { recursive: true });
  const fixture = await page.request.get('http://127.0.0.1:4197/fixture').then(r => r.json());
  await proxy(context);
  const errors: string[] = [];
  page.on('pageerror', e => errors.push(e.message));
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(`${origin}/?account=guest`);
  await expect(page.getByRole('heading', { name: '从一个问题，开始读懂。' })).toBeVisible();
  await expect(page.getByLabel('内测码', { exact: true })).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  await page.screenshot({ path: `${output}/guest-desktop.png`, fullPage: true });
  await page.getByLabel('内测码', { exact: true }).fill('ABCDE-ABCDE-ABCDE-ABCDE');
  await page.getByRole('button', { name: '开始体验', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('内测码已使用');
  await page.getByLabel('内测码', { exact: true }).fill(fixture.code);
  await page.getByRole('button', { name: '开始体验', exact: true }).click();
  await expect(page.locator('.network-library')).toBeVisible();
  await expect(page.locator('.guest-session-notice')).toContainText('已赠送 5 元额度');
  const me = await page.evaluate(() => fetch('/api/auth/me').then(r => r.json()));
  expect(me.is_guest).toBe(true);
  expect((await context.cookies()).find(c => c.name === '__Host-ub_session')).toMatchObject({ secure: true, httpOnly: true });
  await page.getByRole('button', { name: '查看剩余额度' }).click();
  const allowance = page.getByRole('dialog', { name: 'AI 使用额度' });
  await expect(allowance).toContainText('可用 ¥5.00 / 已赠送 ¥5.00');
  await page.screenshot({ path: `${output}/guest-allowance.png`, fullPage: true });
  await allowance.getByRole('button', { name: '返回阅读与对话' }).click();
  await page.reload();
  await expect(page.locator('.network-library')).toBeVisible();
  expect(await page.evaluate(() => fetch('/api/auth/me').then(r => r.json()).then(v => v.user_id))).toBe(me.user_id);
  await page.screenshot({ path: `${output}/guest-library.png`, fullPage: true });
  await page.locator('.network-book-card').first().click();
  await expect(page.locator('.network-shell > .app')).toBeVisible();
  await expect(page.locator('.app-loading')).toBeHidden({ timeout: 30_000 });
  await expect(page.locator('.workspace-shell')).toBeVisible();
  await page.screenshot({ path: `${output}/guest-reader.png`, fullPage: true });
  const fresh = await browser.newContext({ viewport: { width: 390, height: 844 } });
  await proxy(fresh);
  const mobile = await fresh.newPage(); mobile.on('pageerror', e => errors.push(e.message));
  await mobile.goto(`${origin}/?account=guest`);
  await expect(mobile.getByLabel('内测码', { exact: true })).toBeVisible();
  await mobile.screenshot({ path: `${output}/guest-mobile.png`, fullPage: true });
  expect(await mobile.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await mobile.getByLabel('内测码', { exact: true }).fill(fixture.code);
  await mobile.getByRole('button', { name: '开始体验', exact: true }).click();
  await expect(mobile.getByRole('alert')).toContainText('内测码已使用');
  await mobile.goto(`${origin}/?account=guest&lang=en`);
  await expect(mobile.getByRole('heading', { name: 'Understanding starts with a question.' })).toBeVisible();
  await mobile.getByLabel('Invitation code', { exact: true }).fill(fixture.second_code);
  await mobile.getByRole('button', { name: 'Start exploring', exact: true }).click();
  await expect(mobile.locator('.network-library')).toBeVisible();
  const second = await mobile.evaluate(() => fetch('/api/auth/me').then(r => r.json()));
  expect(second.user_id).not.toBe(me.user_id);
  await mobile.screenshot({ path: `${output}/guest-mobile-library.png`, fullPage: true });
  await fresh.close();
  expect(errors).toEqual([]);
});
