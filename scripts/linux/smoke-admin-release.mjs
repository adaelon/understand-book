// Already configured ADM release. stdin: {admin:{username,password},reader:{username,password}}.
// Logs in and reads data; never grants money or submits a model request.
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';

const base = process.env.ADM10_URL;
const output = process.env.ADM10_EVIDENCE;
assert(base?.startsWith('https://') && output, 'Set ADM10_URL and ADM10_EVIDENCE');
let input = '';
for await (const chunk of process.stdin) input += chunk;
const credentials = JSON.parse(input);
const browser = await chromium.launch({ headless: true,
  ...(process.env.ADM10_BROWSER ? { executablePath: process.env.ADM10_BROWSER } : {}) });
const context = await browser.newContext({ ignoreHTTPSErrors: process.env.ADM10_TEST_CERTIFICATE === '1', viewport: { width: 1360, height: 900 } });
const checks = [], errors = [];
context.on('page', page => page.on('pageerror', error => errors.push(error.message)));
const page = await context.newPage();
page.setDefaultTimeout(20000);
await fs.mkdir(output, { recursive: true });
const passed = value => { checks.push(value); console.log(value); };
async function login(account) {
  await page.getByLabel('邮箱或账号', { exact: true }).fill(account.username);
  await page.getByLabel('密码', { exact: true }).fill(account.password);
  await page.getByRole('button', { name: '登录', exact: true }).click();
}
async function get(path, status = 200) {
  const response = await context.request.get(base + path);
  assert.equal(response.status(), status, path);
  assert.match(response.headers()['content-type'], /application\/json/);
  return response.json();
}
try {
  const redirect = await context.request.get(base + '/admin', { maxRedirects: 0 });
  assert.equal(redirect.status(), 308);
  for (const path of ['/admin/assets/adm10-missing.js', '/admin/adm10-missing.css']) {
    assert.equal((await context.request.get(base + path)).status(), 404);
  }
  await get('/api/admin/users', 401);
  await page.goto(base + '/admin/usage');
  await login(credentials.admin);
  await page.getByRole('heading', { name: '全站用量', exact: true }).waitFor();
  await page.reload();
  await page.getByRole('heading', { name: '全站用量', exact: true }).waitFor();
  const report = await get('/api/admin/usage');
  assert.equal(report.timezone, 'Asia/Hong_Kong');
  const scripts = await page.locator('script[src]').evaluateAll(nodes => nodes.map(n => n.getAttribute('src')));
  assert(scripts.length && scripts.every(src => src.startsWith('/admin/')));
  for (const src of scripts) {
    const response = await context.request.get(base + src);
    assert.equal(response.status(), 200);
    assert.match(response.headers()['content-type'], /javascript/);
  }
  passed('Admin redirect, deep-link refresh, JS MIME, missing-resource 404 and authenticated JSON');
  const rejected = await context.request.post(base + '/api/admin/users', { headers: { Origin: base }, data: {} });
  assert.equal(rejected.status(), 403);
  assert.equal((await rejected.json()).error_code, 'CSRF_REJECTED');
  passed('Missing CSRF rejected before account creation');
  await page.goto(base + '/admin/users/' + encodeURIComponent(credentials.reader.username));
  await page.getByRole('heading', { name: '使用与回访', exact: true }).waitFor();
  await page.reload();
  await page.getByRole('heading', { name: '使用与回访', exact: true }).waitFor();
  await page.screenshot({ path: `${output}/admin-detail.png`, fullPage: true });
  const reader = await context.newPage();
  await reader.goto(base);
  await reader.getByRole('heading', { name: '选择阅读材料' }).waitFor();
  await reader.getByRole('button', { name: '使用额度', exact: true }).click();
  if (process.env.ADM10_CONTACT) await reader.getByText(`额度开通与续用联系方式：${process.env.ADM10_CONTACT}`).waitFor();
  const readerScripts = await reader.locator('script[src]').evaluateAll(nodes => nodes.map(n => n.getAttribute('src')));
  assert(readerScripts.every(src => !src.startsWith('/admin/')));
  await reader.getByRole('button', { name: '返回阅读与对话' }).click();
  await reader.getByRole('button', { name: '退出登录', exact: true }).click();
  await page.bringToFront();
  await page.getByRole('button', { name: '登录', exact: true }).waitFor();
  passed('Reader shares real identity, contact is visible, scripts stay separate, Reader logout clears Admin');
  await login(credentials.reader);
  await page.getByRole('heading', { name: '无管理权限', exact: true }).waitFor();
  await get('/api/admin/usage', 403);
  const allowance = await get('/api/account/allowance');
  assert(allowance.current_allowance);
  await reader.bringToFront();
  await reader.reload();
  await reader.getByRole('heading', { name: '选择阅读材料' }).waitFor();
  await reader.getByRole('button', { name: '使用额度', exact: true }).click();
  await reader.setViewportSize({ width: 390, height: 844 });
  await reader.getByRole('heading', { name: '调用明细', exact: false }).waitFor();
  assert(await reader.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  await reader.screenshot({ path: `${output}/reader-allowance-mobile.png`, fullPage: true });
  passed('Ordinary reader is denied Admin, retains own allowance and mobile access');
  assert.deepEqual(errors, []);
  await fs.writeFile(`${output}/release-browser.json`, JSON.stringify({ checks, errors, model_requests: 0 }, null, 2));
} finally { await browser.close(); }
