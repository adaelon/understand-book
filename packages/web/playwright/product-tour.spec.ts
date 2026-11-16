import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

const output = resolve('../../docs/performance/product-tour-20261102');

async function step(page: Page, id: string) {
  const card = page.locator('.product-tour-card');
  await expect(card).toHaveAttribute('data-tour-step', id);
  await expect(card.locator('.product-tour-next')).toBeEnabled();
  await expect(card.locator('.product-tour-hint[role="status"]')).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => {
    const card = document.querySelector('.product-tour-card')!.getBoundingClientRect();
    const hole = document.querySelector('.product-tour-spotlight')!.getBoundingClientRect();
    return card.left >= 0 && card.top >= 0 && card.right <= innerWidth + 1 && card.bottom <= innerHeight + 1
      && hole.width > 0 && hole.height > 0
      && !(Math.max(card.left, hole.left) < Math.min(card.right, hole.right) && Math.max(card.top, hole.top) < Math.min(card.bottom, hole.bottom));
  })).toBe(true);
}
async function next(page: Page, current: string, destination?: string) {
  await step(page, current);
  await page.locator('.product-tour-next').click();
  if (destination) await step(page, destination);
}

test('real pages guide the user from materials to boards, preserve drafts, and support replay and narrow screens', async ({ page, context, request }) => {
  mkdirSync(output, { recursive: true });
  const seed = await request.get('http://127.0.0.1:4231/fixture').then(response => response.json());
  const errors: string[] = [], submissions: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await context.route('**/api/**', async route => {
    const incoming = route.request(), url = new URL(incoming.url());
    if (/agent_runs|agent\/run|\/boards\/question/.test(url.pathname) && incoming.method() === 'POST') submissions.push(url.pathname);
    const response = await route.fetch({ url: `http://127.0.0.1:4230${url.pathname}${url.search}`,
      headers: { ...await incoming.allHeaders(), host: 'reader.example', origin: 'https://reader.example', cookie: seed.cookie }, timeout: 45_000 });
    await route.fulfill({ response });
  });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto('/');
  await step(page, 'purpose');
  await expect(page.locator('.product-tour-next')).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(page.locator('.product-tour-skip')).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(page.locator('.product-tour-next')).toBeFocused();
  await page.screenshot({ path: resolve(output, 'purpose-desktop.png') });
  await next(page, 'purpose', 'materials');
  await next(page, 'materials', 'thoughts');
  await next(page, 'thoughts', 'boards-entry');
  await page.screenshot({ path: resolve(output, 'navigation-desktop.png') });
  await next(page, 'boards-entry', 'board-uses');
  await page.screenshot({ path: resolve(output, 'board-home-desktop.png') });
  await next(page, 'board-uses', 'board-library');
  await next(page, 'board-library', 'board-create');
  await next(page, 'board-create');
  await expect(page.locator('.board-library-create input')).toBeFocused();
  await page.locator('.board-library-create input').fill('新手引导：为什么相同周长有不同面积？');
  await page.locator('.board-library-create select').selectOption(JSON.stringify(seed.publication));
  await page.getByRole('button', { name: '创建并打开画板', exact: true }).click();
  await step(page, 'board-canvas');
  await page.screenshot({ path: resolve(output, 'board-desktop.png') });
  await next(page, 'board-canvas', 'board-agent');
  await next(page, 'board-agent', 'thoughts');
  await next(page, 'thoughts', 'board-input');
  await next(page, 'board-input');
  const boardInput = page.locator('[data-tour="board-input"] textarea');
  await expect(boardInput).toBeFocused();
  await boardInput.fill('我觉得周长固定，长和宽仍然可以变化。');
  await page.getByRole('button', { name: '新手引导', exact: true }).click();
  await step(page, 'board-canvas');
  await page.keyboard.press('Escape');
  await expect(boardInput).toHaveValue('我觉得周长固定，长和宽仍然可以变化。');
  await expect(page.locator('#app')).not.toHaveAttribute('inert');
  await page.locator('.network-current-reader').click();
  await step(page, 'agent');
  await next(page, 'agent', 'presentation');
  await page.getByRole('button', { name: '看看演示效果', exact: true }).click();
  await page.locator('#product-tour-width').focus();
  await page.keyboard.press('ArrowRight');
  await expect(page.locator('.product-tour-demo p')).toContainText('35 平方米');
  await page.screenshot({ path: resolve(output, 'presentation-desktop.png') });
  await next(page, 'presentation', 'boards-entry');
  await next(page, 'boards-entry', 'thoughts');
  await next(page, 'thoughts', 'reader-input');
  await next(page, 'reader-input');
  const readerInput = page.locator('[data-tour="reader-input"] textarea');
  await expect(readerInput).toBeFocused();
  await readerInput.fill('我的理解是长宽的和不变，但乘积会变。');
  await page.getByRole('button', { name: '更多阅读工具', exact: true }).click();
  await page.getByRole('button', { name: '新手引导', exact: true }).click();
  await step(page, 'agent');
  await page.keyboard.press('Escape');
  await expect(readerInput).toHaveValue('我的理解是长宽的和不变，但乘积会变。');

  for (const [width, height, language] of [[390, 844, 'zh-CN'], [320, 568, 'zh-CN'], [320, 568, 'en']] as const) {
    await page.setViewportSize({ width, height });
    await page.goto(`/?view=boards&lang=${language}`);
    const help = page.getByRole('button', { name: language === 'en' ? 'Getting started' : '新手引导', exact: true });
    await expect(help).toBeInViewport();
    await help.click();
    await step(page, 'board-uses');
    if (width === 390) {
      const uses = await page.locator('[data-tour="board-uses"]').boundingBox();
      const spotlight = await page.locator('.product-tour-spotlight').boundingBox();
      expect(spotlight!.height).toBeGreaterThanOrEqual(uses!.height);
    }
    await page.screenshot({ path: resolve(output, `boards-${width}-${language}.png`) });
    await next(page, 'board-uses', 'board-library');
    await next(page, 'board-library', 'board-create');
    await page.keyboard.press('Escape');
    // A direct visit to the board library need not mount a reading workspace.
    await page.locator('.board-library-card').filter({ hasText: '新手引导：为什么相同周长有不同面积？' }).first().locator('footer button').click();
    await expect(page.locator('.board-title-input')).toHaveValue('新手引导：为什么相同周长有不同面积？');
    await help.click();
    await step(page, 'board-canvas');
    await page.screenshot({ path: resolve(output, `board-${width}-${language}.png`) });
    await next(page, 'board-canvas', 'board-agent');
    await next(page, 'board-agent', 'thoughts');
    await next(page, 'thoughts', 'board-input');
    await next(page, 'board-input');
    await expect(boardInput).toBeFocused();
    await page.locator('.network-current-reader').click();
    await page.getByRole('button', { name: language === 'en' ? 'More reading tools' : '更多阅读工具', exact: true }).click();
    await help.click();
    await step(page, 'agent');
    await next(page, 'agent', 'presentation');
    await page.locator('.product-tour-demo-toggle').click();
    await step(page, 'presentation');
    await page.screenshot({ path: resolve(output, `presentation-${width}-${language}.png`) });
    await next(page, 'presentation', 'boards-entry');
    await next(page, 'boards-entry', 'thoughts');
    await next(page, 'thoughts', 'reader-input');
    await page.screenshot({ path: resolve(output, `input-${width}-${language}.png`) });
    await next(page, 'reader-input');
    await expect(readerInput).toBeFocused();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.reload();
    await expect(page.locator('[data-tour="reader-input"]')).toBeAttached();
    // The automatic-start delay has elapsed; finishing/skipping must survive reload.
    await page.waitForTimeout(650);
    await expect(page.locator('.product-tour-card')).toHaveCount(0);
  }
  expect(errors).toEqual([]);
  expect(submissions).toEqual([]);
  await context.unrouteAll({ behavior: 'ignoreErrors' });
});
