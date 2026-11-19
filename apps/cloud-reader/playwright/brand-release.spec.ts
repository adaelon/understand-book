import { expect, test } from '@playwright/test';

for (const width of [320, 390, 1440]) {
  test(`production brand assets and account entry fit at ${width}px`, async ({ page, request }, info) => {
    const failures: string[] = [];
    page.on('pageerror', error => failures.push(error.message));
    page.on('response', response => { if (response.status() >= 400 && !response.url().includes('/api/')) failures.push(`${response.status()} ${response.url()}`); });
    await page.setViewportSize({ width, height: 900 });
    await page.goto('/home.html');
    await page.evaluate(() => document.fonts.ready);
    await expect.poll(() => page.locator('.home-nav .brand-logo').evaluate((image: HTMLImageElement) => image.complete && image.naturalWidth > 0)).toBe(true);
    expect(await page.locator('.home-hero h1').evaluate(node => getComputedStyle(node).fontFamily)).toContain('Smiley Sans');
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    const icons = await page.locator('link[rel="icon"], link[rel="apple-touch-icon"], link[rel="manifest"]').evaluateAll(links => links.map(link => (link as HTMLLinkElement).href));
    expect(icons).toHaveLength(3);
    for (const icon of icons) expect((await request.get(icon)).ok()).toBe(true);
    await page.screenshot({ path: info.outputPath(`home-${width}.png`) });
    await page.route('**/api/auth/me', route => route.fulfill({ status: 401, json: { error: 'unauthorized' } }));
    await page.goto('/?account=register');
    await expect(page.getByRole('heading', { name: '创建阅读账号' })).toBeVisible();
    await expect.poll(() => page.locator('.account-access .brand-logo').evaluate((image: HTMLImageElement) => image.complete && image.naturalWidth > 0)).toBe(true);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: info.outputPath(`register-${width}.png`) });
    await page.getByRole('button', { name: '返回登录' }).click();
    await expect(page.getByRole('heading', { name: '回到你的阅读' })).toBeVisible();
    await expect(page.locator('.network-login .brand-logo')).toBeVisible();
    expect(failures).toEqual([]);
  });
}
