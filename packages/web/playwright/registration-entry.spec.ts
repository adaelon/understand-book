import { expect, test } from '@playwright/test';

for (const width of [390, 1440]) {
  test(`registration is the only beta entry at ${width}px`, async ({ page }, info) => {
    const errors: string[] = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
    await page.route('**/api/auth/me', route => route.fulfill({ status: 401, json: {} }));
    await page.goto('/home.html#begin');
    await page.evaluate(() => document.fonts.ready);
    const entry = page.getByRole('link', { name: '已有内测码，开始注册', exact: false });
    await entry.scrollIntoViewIfNeeded();
    await expect(entry).toHaveAttribute('href', './?account=register');
    await expect(page.locator('.home-beta-notice')).toHaveText('内测期间 AI Token 免费使用，不限额度。');
    await expect(page.locator('a[href*="account=guest"]')).toHaveCount(0);
    await expect(page.locator('body')).not.toContainText('无需注册');
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: info.outputPath(`registration-entry-${width}.png`) });

    await entry.click();
    await expect(page.getByRole('heading', { name: '创建阅读账号' })).toBeVisible();
    await expect(page.locator('.welcome-credit')).toContainText('内测期间 AI Token 免费使用');
    await expect(page.locator('.welcome-credit')).toContainText('Token 不限额度');
    await page.screenshot({ path: info.outputPath(`registration-free-tokens-${width}.png`) });
    await page.getByRole('button', { name: '返回登录', exact: true }).click();
    await expect(page.getByRole('heading', { name: '回到你的阅读' })).toBeVisible();
    await expect(page.getByRole('button', { name: '内测码免注册体验', exact: true })).toHaveCount(0);
    await page.getByRole('button', { name: '注册账号', exact: true }).click();
    await expect(page.getByRole('heading', { name: '创建阅读账号' })).toBeVisible();

    await page.goto('/?account=guest');
    await expect(page).toHaveURL(/account=register$/);
    await expect(page.getByRole('heading', { name: '创建阅读账号' })).toBeVisible();
    await expect(page.getByLabel('邮箱', { exact: true })).toBeVisible();
    expect(errors).toEqual([]);
  });
}
