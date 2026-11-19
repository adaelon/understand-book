import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const source = new URL('../../../crates/server/src/', import.meta.url);
test('sign-in errors, account catalog, CSRF logout and account switch', async ({ page }) => {
  let owner = '';
  let csrf = '';
  await page.route('https://reader.example/**', async route => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    const asset = path === '/' ? ['multi_user_login.html', 'text/html'] : path === '/auth.js' ? ['multi_user_login.js', 'text/javascript'] : path === '/auth.css' ? ['multi_user_login.css', 'text/css'] : null;
    if (asset) return route.fulfill({ contentType: asset[1], body: readFileSync(new URL(asset[0], source), 'utf8') });
    if (path === '/api/auth/login') {
      const body = request.postDataJSON();
      if (body.password === 'wrong') return route.fulfill({ status: 401, json: {} });
      owner = body.username.includes('@') ? 'A' : body.username; csrf = `csrf-${owner}`;
      return route.fulfill({ json: { user_id: owner, email: owner === 'A' ? 'reader@example.com' : null, csrf_token: csrf } });
    }
    if (!owner) return route.fulfill({ status: 401, json: {} });
    if (path === '/api/auth/me') return route.fulfill({ json: { user_id: owner, csrf_token: csrf } });
    if (path === '/api/library') return route.fulfill({ json: { books: [{ published_book_ref: { book_id: `${owner}的材料`, publication_id: 'fixture' } }] } });
    if (path === '/api/auth/logout') {
      expect(request.headers()['x-csrf-token']).toBe(csrf);
      owner = ''; csrf = ''; return route.fulfill({ json: { ok: true } });
    }
    return route.fulfill({ status: 404, json: {} });
  });
  await page.goto('https://reader.example/');
  await page.getByLabel('邮箱或账号', { exact: true }).fill('A');
  await page.getByLabel('密码', { exact: true }).fill('wrong');
  await page.getByRole('button', { name: '登录', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('账号或密码不正确');
  await expect(page.getByLabel('密码', { exact: true })).toHaveValue('');
  await page.getByLabel('邮箱或账号', { exact: true }).fill('reader@example.com');
  await page.getByLabel('密码', { exact: true }).fill('fixture-only-password');
  await page.getByRole('button', { name: '登录', exact: true }).click();
  await expect(page.getByRole('listitem')).toHaveText('A的材料');
  await expect(page.locator('#identity')).toHaveText('reader@example.com');
  await page.getByRole('button', { name: '退出登录' }).click();
  await expect(page.getByRole('listitem')).toHaveCount(0);
  await page.getByLabel('邮箱或账号', { exact: true }).fill('B');
  await page.getByLabel('密码', { exact: true }).fill('fixture-only-password');
  await page.getByRole('button', { name: '登录', exact: true }).click();
  await expect(page.getByRole('listitem')).toHaveText('B的材料');
  await expect(page.getByText('A的材料')).toHaveCount(0);
  await page.getByRole('button', { name: '退出登录' }).click();
  await expect(page.getByRole('heading', { name: '回到你的阅读' })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.screenshot({ path: fileURLToPath(new URL('../../../docs/performance/linux-multi-reader-mu4-20260930/login-mobile.png', import.meta.url)), fullPage: true });
});

test('late startup identity cannot replace a newly signed-in account', async ({ page }) => {
  let releaseIdentity: () => void = () => {};
  let requestedIdentity: () => void = () => {};
  const identityRequested = new Promise<void>(resolve => { requestedIdentity = resolve; });
  const identityRelease = new Promise<void>(resolve => { releaseIdentity = resolve; });
  await page.route('https://reader.example/**', async route => {
    const path = new URL(route.request().url()).pathname;
    const asset = path === '/' ? ['multi_user_login.html', 'text/html'] : path === '/auth.js' ? ['multi_user_login.js', 'text/javascript'] : path === '/auth.css' ? ['multi_user_login.css', 'text/css'] : null;
    if (asset) return route.fulfill({ contentType: asset[1], body: readFileSync(new URL(asset[0], source), 'utf8') });
    if (path === '/api/auth/me') {
      requestedIdentity(); await identityRelease;
      return route.fulfill({ json: { user_id: 'old-account', csrf_token: 'old-token' } });
    }
    if (path === '/api/auth/login') return route.fulfill({ json: { user_id: 'new-account', csrf_token: 'new-token' } });
    if (path === '/api/library') return route.fulfill({ json: { books: [{ published_book_ref: { book_id: 'new-material', publication_id: 'fixture' } }] } });
    return route.fulfill({ status: 404, json: {} });
  });
  await page.goto('https://reader.example/');
  await identityRequested;
  await page.getByLabel('邮箱或账号', { exact: true }).fill('new-account');
  await page.getByLabel('密码', { exact: true }).fill('fixture-only-password');
  await page.getByRole('button', { name: '登录', exact: true }).click();
  await expect(page.locator('#identity')).toHaveText('new-account');
  const delivered = page.waitForResponse('https://reader.example/api/auth/me');
  releaseIdentity(); await delivered;
  await expect(page.getByRole('listitem')).toHaveText('new-material');
  await expect(page.locator('#identity')).toHaveText('new-account');
  await expect(page.getByText('old-account')).toHaveCount(0);
});
