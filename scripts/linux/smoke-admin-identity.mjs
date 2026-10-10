// Run after smoke-admin.mjs against the same isolated candidate.
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
const base = process.env.ADM7_URL ?? 'https://localhost:18443';
const output = process.env.ADM7_EVIDENCE ?? 'tmp/adm7-candidate';
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ ignoreHTTPSErrors: true });
const admin = await context.newPage(), reader = await context.newPage();
const checks = [], errors = [];
for (const page of [admin, reader]) { page.setDefaultTimeout(15000); page.on('pageerror', error => errors.push(String(error))); }
async function login(page, owner) {
  await page.getByLabel('邮箱或账号', { exact: true }).fill(owner);
  await page.getByLabel('密码', { exact: true }).fill('fixture-only-password');
  await page.getByRole('button', { name: '登录', exact: true }).click();
}
try {
  await admin.goto(`${base}/admin/users/A`); await login(admin, 'B');
  await admin.getByRole('heading', { name: 'A', exact: true }).waitFor();
  await reader.goto(base); await reader.getByRole('heading', { name: '选择阅读材料' }).waitFor();
  assert.equal(await reader.getByRole('link', { name: '运营后台' }).count(), 1);
  const readerHtml = await (await context.request.get(base)).text(); assert(!readerHtml.includes('/admin/assets/'));
  await reader.getByRole('button', { name: '退出登录', exact: true }).click();
  await admin.getByRole('heading', { name: '登录运营后台' }).waitFor();
  checks.push('Reader logout clears Admin queries and identity');
  await login(reader, 'A');
  await reader.getByRole('heading', { name: '选择阅读材料' }).waitFor();
  await admin.getByRole('heading', { name: '无管理权限' }).waitFor();
  assert.equal(await admin.getByRole('heading', { name: 'AI 使用额度（元）' }).count(), 0);
  assert.equal(await reader.getByRole('link', { name: '运营后台' }).count(), 0);
  checks.push('Reader account switch to a normal account removes Admin data and both management entries');
  await admin.route('**/api/auth/logout', async route => { const response = await route.fetch(); assert.equal(response.status(), 200); await route.abort(); });
  await admin.getByRole('button', { name: '退出登录', exact: true }).click();
  await admin.getByRole('alert').filter({ hasText: '退出请求未确认' }).waitFor();
  await reader.getByRole('heading', { name: '回到你的阅读' }).waitFor();
  checks.push('A lost logout response still notifies Reader and clears old private state');
  assert.deepEqual(errors, []);
  await fs.writeFile(`${output}/identity-result.json`, JSON.stringify({ status: 'passed', checks, errors }, null, 2));
  console.log(checks.join('\n'));
} catch (error) {
  await admin.screenshot({ path: `${output}/identity-failure.png`, fullPage: true });
  throw error;
} finally { await browser.close(); }
