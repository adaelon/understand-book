import { chromium } from 'playwright';
import { mkdir, writeFile, copyFile, unlink } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { zipSync, strToU8 } from 'fflate';
const root = path.resolve('tmp/reader-latency-release-20261021');
const browser = await chromium.connectOverCDP('http://127.0.0.1:19337', { timeout: 15000 });
const context = browser.contexts()[0], page = context.pages().find(page => page.url().includes('tauri.localhost')) ?? context.pages()[0];
const errors = [], cloudRequests = [];
page.on('pageerror', error => errors.push(error.message));
page.on('request', request => { if (request.url().startsWith('https://understandbook.top') || new URL(request.url()).pathname.startsWith('/api/')) cloudRequests.push(request.url()); });
const check = async (condition, message) => assert(await condition(), message);
try {
  await page.getByRole('heading', { name: '本机书库', exact: true }).waitFor();
  await mkdir(path.join(root, 'fixtures'), { recursive: true });
  const text = path.join(root, 'fixtures', '期末考纲.txt');
  await writeFile(text, '期末考试范围：导数、定积分与微分方程。\n本机文件无需购买云端存储套餐。', 'utf8');
  const markdown = path.join(root, 'fixtures', '复习笔记.md');
  await writeFile(markdown, '# 复习笔记\n\n本机阅读：$x^2$ 的导数。', 'utf8');
  const pdf = path.join(root, 'fixtures', '课堂讲义.pdf');
  await copyFile('tmp/cloud-release-20261021/converted26-pptx.pdf', pdf);
  const epub = path.join(root, 'fixtures', '复习教材.epub');
  await writeFile(epub, zipSync({
    'META-INF/container.xml': strToU8('<container><rootfiles><rootfile full-path="OPS/content.opf"/></rootfiles></container>'),
    'OPS/content.opf': strToU8('<package><manifest><item id="a" href="chapter.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="a"/></spine></package>'),
    'OPS/chapter.xhtml': strToU8('<html><body><h1>本机教材第一章</h1><p>学习导数与积分。</p></body></html>'),
  }));
  const imported = [];
  const existing = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('desktop_local_materials', { scope: null }));
  for (const file of [text, markdown, pdf, epub]) {
    imported.push(existing.find(item => item.filename === path.basename(file)) ?? await page.evaluate(path => window.__TAURI_INTERNALS__.invoke('desktop_import_local_material', { path, scope: { account_id: '', workspace_id: '' }, workspaceName: '期末复习' }), file));
  }
  await unlink(text);
  await page.reload();
  await page.getByText('期末考纲.txt', { exact: true }).waitFor();
  await page.screenshot({ path: path.join(root, 'local-library.png'), fullPage: true });
  await page.locator('.local-item').filter({ hasText: '期末考纲.txt' }).getByRole('button', { name: '打开资料', exact: true }).click();
  await page.getByText('期末考试范围：导数、定积分与微分方程。', { exact: false }).waitFor();
  await page.getByRole('button', { name: '返回资料', exact: true }).click();
  await page.locator('.local-item').filter({ hasText: '复习教材.epub' }).getByRole('button', { name: '打开资料', exact: true }).click();
  await page.frameLocator('iframe').getByText('学习导数与积分。').waitFor();
  await page.getByRole('button', { name: '返回资料', exact: true }).click();
  await page.locator('.local-item').filter({ hasText: '复习笔记.md' }).getByRole('button', { name: '打开资料', exact: true }).click();
  await page.frameLocator('iframe').getByRole('heading', { name: '复习笔记', exact: true }).waitFor();
  await page.getByRole('button', { name: '返回资料', exact: true }).click();
  await page.locator('.local-item').filter({ hasText: '课堂讲义.pdf' }).getByRole('button', { name: '打开资料', exact: true }).click();
  await page.locator('canvas').first().waitFor({ timeout: 20000 });
  await page.waitForFunction(() => [...document.querySelectorAll('canvas')].some(canvas => canvas.width > 0 && canvas.height > 0));
  await page.screenshot({ path: path.join(root, 'local-pdf.png'), fullPage: true });
  await page.getByRole('button', { name: '返回资料', exact: true }).click();
  await context.setOffline(true);
  await page.reload();
  await page.getByText('期末考纲.txt', { exact: true }).waitFor();
  await page.locator('.local-item').filter({ hasText: '期末考纲.txt' }).getByRole('button', { name: '打开资料', exact: true }).click();
  await page.getByText('本机文件无需购买云端存储套餐。', { exact: false }).waitFor();
  assert.deepEqual(cloudRequests, []);
  assert.deepEqual(errors, []);
  await context.setOffline(false);
  await page.getByRole('button', { name: '返回资料', exact: true }).click();
  await page.getByRole('button', { name: '进入云端学习', exact: true }).click();
  await page.waitForURL(url => url.origin === 'https://understandbook.top', { timeout: 60000 });
  await page.waitForLoadState('domcontentloaded');
  await page.getByRole('link', { name: '登录', exact: true }).waitFor({ timeout: 45000 });
  await page.getByRole('link', { name: '登录', exact: true }).click();
  await page.waitForURL(url => url.searchParams.get('account') === 'login');
  await page.waitForFunction(() => !!document.querySelector('#app')?.textContent?.trim(), null, { timeout: 45000 });
  assert.equal(await page.evaluate(() => !!window.__TAURI_INTERNALS__), true);
  const cloudUrl = page.url();
  const cloudText = await page.locator('body').innerText();
  assert(!cloudText.includes('ERR_'));
  await page.screenshot({ path: path.join(root, 'cloud-entry.png'), fullPage: true });
  await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('desktop_open_local_library'));
  await page.getByRole('heading', { name: '本机书库', exact: true }).waitFor();
  await page.getByText('期末考纲.txt', { exact: true }).waitFor();
  assert.deepEqual(errors, []);
  const report = { cloud_url: cloudUrl, cloud_page_excerpt: cloudText.slice(0, 700), native_bridge_on_cloud: true, return_to_library: true, status: 'passed', imported: imported.map(item => ({ filename: item.filename, bytes: item.bytes })), offline_reading: true, deleted_original_copy_readable: true, formats_opened: ['txt','md','epub','pdf'], cloud_requests: cloudRequests, errors, model_requests: 0 };
  await writeFile(path.join(root, 'webview2.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
} catch (error) {
  await page.screenshot({ path: path.join(root, 'failure.png'), fullPage: true }).catch(() => {});
  await writeFile(path.join(root, 'failure.json'), JSON.stringify({ error: String(error), page: await page.locator('body').innerText(), errors, cloudRequests }, null, 2));
  throw error;
} finally { await browser.close(); }
