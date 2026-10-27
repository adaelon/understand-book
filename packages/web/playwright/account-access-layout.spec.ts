import { test, expect } from '@playwright/test';

for (const viewport of [{ width: 390, height: 600 }, { width: 320, height: 568 }, { width: 1280, height: 600 }, { width: 390, height: 844 }, { width: 1440, height: 900 }]) {
  test(`registration buttons remain reachable at ${viewport.width}x${viewport.height}`, async ({ page }, info) => {
    await page.setViewportSize(viewport);
    await page.route('**/api/auth/me', route => route.fulfill({ status: 401, json: {} }));
    await page.goto('/?account=register');
    await expect(page.getByRole('heading', { name: '创建阅读账号' })).toBeInViewport();
    await page.evaluate(() => document.fonts.ready);
    await page.screenshot({ path: info.outputPath(`register-${viewport.width}x${viewport.height}.png`) });
    await page.locator('.account-access').hover();
    await page.mouse.wheel(0, 1500);
    const back = page.getByRole('button', { name: '返回登录', exact: true });
    await expect(back).toBeInViewport({ ratio: 1 });
    await expect(page.getByRole('button', { name: '发送注册验证码', exact: true })).toBeInViewport({ ratio: 1 });
    await page.screenshot({ path: info.outputPath(`register-bottom-${viewport.width}x${viewport.height}.png`) });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await back.click();
    await expect(page.getByRole('heading', { name: '回到你的阅读' })).toBeVisible();
  });
}

test('registration validates and reveals passwords, then advances to email verification', async ({ page }, info) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.route('**/api/auth/me', route => route.fulfill({ status: 401, json: {} }));
  const requests: Record<string, string>[] = [];
  await page.route('**/api/auth/register/start', async route => {
    requests.push(route.request().postDataJSON());
    await route.fulfill({ json: { request_id: 'registration-preview', expires_at: Math.floor(Date.now() / 1000) + 900, resend_after: 60 } });
  });
  await page.route('**/api/auth/register/complete', route => route.fulfill({ json: { completed: true } }));
  await page.goto('/?account=register');
  await page.getByLabel('邮箱', { exact: true }).fill('reader@example.com');
  const password = page.getByLabel('密码', { exact: true });
  const confirm = page.getByLabel('确认密码', { exact: true });
  await password.fill('short');
  await confirm.fill('short');
  await page.getByLabel('内测码', { exact: false }).fill('ABCDE-ABCDE-ABCDE-ABCDE');
  await page.getByRole('button', { name: '发送注册验证码' }).click();
  await expect(password).toBeFocused();
  await expect(password).toHaveAttribute('aria-invalid', 'true');
  expect(requests).toHaveLength(0);
  await password.fill('readmore');
  await page.getByRole('button', { name: '显示密码', exact: true }).click();
  await expect(password).toHaveAttribute('type', 'text');
  await page.getByRole('button', { name: '显示密码', exact: true }).click();
  await expect(password).toHaveAttribute('type', 'password');
  await page.getByRole('button', { name: '发送注册验证码' }).click();
  await expect(confirm).toBeFocused();
  await expect(page.getByRole('alert')).toHaveText('两次输入的密码不一致。');
  await page.screenshot({ path: info.outputPath('registration-validation.png') });
  await confirm.fill('readmore');
  await page.getByRole('button', { name: '发送注册验证码' }).click();
  await expect(page.getByRole('heading', { name: '验证邮箱' })).toBeVisible();
  await expect(page.getByLabel('邮箱验证码', { exact: true })).toBeFocused();
  await expect(page.locator('[aria-current="step"]')).toContainText('验证邮箱');
  expect(requests).toEqual([{ email: 'reader@example.com', password: 'readmore', invite_code: 'ABCDE-ABCDE-ABCDE-ABCDE' }]);
  await page.screenshot({ path: info.outputPath('registration-verification.png') });
  await page.getByLabel('邮箱验证码', { exact: true }).fill('123456');
  await page.getByRole('button', { name: '完成注册', exact: true }).click();
  await expect(page.getByRole('heading', { name: '回到你的阅读' })).toBeVisible();
  await expect(page.getByLabel('邮箱或账号')).toHaveValue('reader@example.com');
});

test('English registration stays translated and fits a narrow screen', async ({ page }, info) => {
  await page.setViewportSize({ width: 320, height: 740 });
  await page.route('**/api/auth/me', route => route.fulfill({ status: 401, json: {} }));
  await page.goto('/?account=register&lang=en');
  await expect(page.getByRole('heading', { name: 'Create a reading account' })).toBeVisible();
  await expect(page.locator('.account-card')).not.toContainText(/[\u3400-\u9fff]/);
  expect(await page.locator('.account-access').evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
  await page.screenshot({ path: info.outputPath('registration-english.png') });
});

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
