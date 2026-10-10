import { test, expect } from '@playwright/test';

for (const viewport of [{ width: 390, height: 600 }, { width: 320, height: 568 }, { width: 1280, height: 600 }]) {
  test(`registration buttons remain reachable at ${viewport.width}x${viewport.height}`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await page.route('**/api/auth/me', route => route.fulfill({ status: 401, json: {} }));
    await page.goto('/?account=register');
    await expect(page.getByRole('heading', { name: '创建阅读账号' })).toBeInViewport();
    await page.locator('.account-access').hover();
    await page.mouse.wheel(0, 1500);
    const back = page.getByRole('button', { name: '返回登录', exact: true });
    await expect(back).toBeInViewport({ ratio: 1 });
    await expect(page.getByRole('button', { name: '发送注册验证码', exact: true })).toBeInViewport({ ratio: 1 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await back.click();
    await expect(page.getByRole('heading', { name: '回到你的阅读' })).toBeVisible();
  });
}

for (const view of ['forgot-password', 'reset-password']) {
  test(`${view} can return to login in a short window`, async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 320 });
    await page.route('**/api/auth/me', route => route.fulfill({ status: 401, json: {} }));
    await page.goto(`/?account=${view}${view === 'reset-password' ? '#token=layout-fixture' : ''}`);
    await page.locator('.account-access').hover();
    await page.mouse.wheel(0, 1500);
    const back = page.getByRole('button', { name: '返回登录', exact: true });
    await expect(back).toBeInViewport({ ratio: 1 });
    await back.click();
    await expect(page.getByRole('heading', { name: '回到你的阅读' })).toBeVisible();
  });
}
