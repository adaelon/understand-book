import { expect, test } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

const output = resolve('../../docs/performance/board-primary-navigation-20261102');
test('primary navigation opens, creates, reopens and refreshes real boards on desktop and mobile', async ({ page, context, request }) => {
  mkdirSync(output, { recursive: true });
  const seed = await request.get('http://127.0.0.1:4231/fixture').then(response => response.json());
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await context.route('**/api/**', async route => {
    const incoming = route.request(), url = new URL(incoming.url());
    const response = await route.fetch({ url: `http://127.0.0.1:4230${url.pathname}${url.search}`,
      headers: { ...await incoming.allHeaders(), host: 'reader.example', origin: 'https://reader.example', cookie: seed.cookie }, timeout: 45_000 });
    await route.fulfill({ response });
  });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto('/?view=boards');
  await expect(page.getByRole('heading', { name: '学习画板', exact: true })).toBeVisible();
  await expect(page.locator('.board-library-empty')).toBeVisible();
  await page.screenshot({ path: resolve(output, 'empty-desktop.png') });
  await page.locator('.board-library-heading button').click();
  await page.locator('.board-library-create input').fill('为什么平均速度不能直接相加再除以二？');
  await page.locator('.board-library-create select').selectOption(JSON.stringify(seed.publication));
  await page.getByRole('button', { name: '创建并打开画板', exact: true }).click();
  await expect(page.locator('.board-workspace')).toBeVisible();
  await expect(page.locator('.board-title-input')).toHaveValue('为什么平均速度不能直接相加再除以二？');
  await expect(page.locator('.network-primary-nav')).toBeVisible();
  const navigation = await page.locator('.network-primary-nav').boundingBox();
  const workspace = await page.locator('.board-workspace-overlay').boundingBox();
  expect(workspace!.y).toBeGreaterThanOrEqual(navigation!.y + navigation!.height - 1);
  await page.screenshot({ path: resolve(output, 'board-desktop.png') });
  const firstId = new URL(page.url()).searchParams.get('board_id'); expect(firstId).toBeTruthy();
  await page.locator('.network-primary-nav').getByRole('button', { name: '学习画板', exact: true }).click();
  await expect(page.locator('.board-library-card')).toHaveCount(1);
  await page.locator('.board-library-intro summary').click();
  await page.locator('.board-library-uses button').nth(1).click();
  await page.locator('.board-library-create input').fill('等距离与等时间：两种平均方法的适用条件');
  await page.getByRole('button', { name: '创建并打开画板', exact: true }).click();
  await expect(page.locator('.board-title-input')).toHaveValue('等距离与等时间：两种平均方法的适用条件');
  await page.locator('.network-primary-nav').getByRole('button', { name: '学习画板', exact: true }).click();
  await expect(page.locator('.board-library-card')).toHaveCount(2);
  await page.screenshot({ path: resolve(output, 'library-desktop.png') });
  for (const width of [390, 320]) {
    await page.setViewportSize({ width, height: 844 });
    await expect(page.locator('.network-primary-nav').getByRole('button', { name: '学习画板', exact: true })).toBeInViewport();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: resolve(output, `library-${width}.png`) });
    await page.locator('.board-library-card').filter({ hasText: '为什么平均速度' }).getByRole('button', { name: '打开画板', exact: true }).click();
    await expect(page.locator('.board-title-input')).toHaveValue('为什么平均速度不能直接相加再除以二？');
    await expect(page.locator('.network-primary-nav').getByRole('button', { name: '学习画板', exact: true })).toBeInViewport();
    await page.screenshot({ path: resolve(output, `board-${width}.png`) });
    await page.reload();
    await expect(page.locator('.board-title-input')).toHaveValue('为什么平均速度不能直接相加再除以二？');
    expect(new URL(page.url()).searchParams.get('board_id')).toBe(firstId);
    await page.locator('.network-primary-nav').getByRole('button', { name: '学习画板', exact: true }).click();
    await expect(page.locator('.board-library-card')).toHaveCount(2);
  }
  await page.goto('/?view=boards&lang=en');
  await expect(page.getByRole('heading', { name: 'Learning boards', exact: true })).toBeVisible();
  await expect(page.locator('.network-primary-nav').getByRole('button', { name: 'Learning boards', exact: true })).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: resolve(output, 'library-320-en.png') });
  for (const [width, language] of [[1440, 'zh-CN'], [390, 'zh-CN'], [320, 'en']] as const) {
    await page.setViewportSize({ width, height: 844 });
    await page.goto(`/home.html?lang=${language}`);
    const entry = page.locator('.home-board-entry');
    await expect(entry).toBeInViewport();
    const brand = await page.locator('.home-brand > span:last-child').boundingBox();
    const actions = await page.locator('.home-nav-actions').boundingBox();
    expect(brand!.x + brand!.width).toBeLessThanOrEqual(actions!.x);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: resolve(output, `home-${width}.png`) });
    await entry.click();
    await expect(page.locator('.board-library')).toBeVisible();
    await expect(page.locator('.board-library-card')).toHaveCount(2);
  }
  expect(errors).toEqual([]);
  await context.unrouteAll({ behavior: 'ignoreErrors' });
});
