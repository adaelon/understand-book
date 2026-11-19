import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Start the ignored Rust s2_b1_browser_host first; every product request uses
// its actual control store, material grants, private workspaces and ledger.
const backend = 'http://127.0.0.1:4190', control = 'http://127.0.0.1:4191';
const output = resolve('../../docs/performance/workspace-s2-b1-20261018');
test.afterEach(async ({ page }, info) => {
  if (info.status !== info.expectedStatus) {
    await page.screenshot({ path: info.outputPath('failure.png'), fullPage: true });
    writeFileSync(info.outputPath('body.txt'), await page.locator('body').innerText());
  }
});
async function scene(page: Page) {
  return page.evaluate(async () => {
    const modulePath = '/src/network-context.ts';
    return (await import(modulePath)).network.value.workspace;
  });
}

test('S2 confirms personal goals, reuses shared materials, selects updates and preserves personal ownership', async ({ page, context, browser }) => {
  test.setTimeout(150_000); page.setDefaultTimeout(15_000);
  const fixture = await page.request.get(`${control}/fixture`).then(response => response.json());
  const failures: string[] = [];
  const otherContext = await browser.newContext({ baseURL: 'http://127.0.0.1:4174', viewport: { width: 1440, height: 1000 } });
  const other = await otherContext.newPage(); other.setDefaultTimeout(15_000);
  for (const current of [page, other]) current.on('pageerror', error => failures.push(error.message));
  const proxy = async (target: BrowserContext, cookie: string) => {
    await target.route(/\/api\/|\/books\//, async route => {
      const request = route.request(), url = new URL(request.url());
      const response = await route.fetch({ url: `${backend}${url.pathname}${url.search}`, headers: {
        ...await request.allHeaders(), host: '127.0.0.1:4190', origin: fixture.origin, cookie,
      }});
      await route.fulfill({ response });
    });
    await target.route('**/s2-acceptance', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><html><head><meta name="viewport" content="width=device-width, initial-scale=1"></head><body><div id="app"></div><script type="module">
      import {createApp} from '/node_modules/.vite/deps/vue.js';
      import NetworkApp from '/src/NetworkApp.vue'; import '/reader-src/style.css';
      createApp(NetworkApp).mount('#app');
    </script></body></html>` }));
  };
  await proxy(context, fixture.reader_cookie); await proxy(otherContext, fixture.admin_cookie);
  const read = async (path: string, cookie = fixture.reader_cookie) => {
    const response = await page.request.get(`${backend}${path}`, { headers: { host: '127.0.0.1:4190', origin: fixture.origin, cookie }});
    expect(response.ok(), `${path}: ${await response.text()}`).toBe(true); return response.json();
  };
  const controlGet = async (path: string) => { const response = await page.request.get(`${control}${path}`); expect(response.ok()).toBe(true); return response.json(); };
  const bookPath = (reference: { book_id: string; publication_id: string }) => `/api/books/${encodeURIComponent(reference.book_id)}/publications/${encodeURIComponent(reference.publication_id)}/text?lid=1.1`;
  const catalog = (await read('/api/library')).books;
  const title = (reference: { book_id: string; publication_id: string }) => catalog.find((book: any) => book.published_book_ref.book_id === reference.book_id && book.published_book_ref.publication_id === reference.publication_id)?.title || reference.book_id.replace(/[-_]+/g, ' ');
  const initialCosts = await read('/api/admin/cost-summary', fixture.admin_cookie);
  const initialOldSource = await read(bookPath(fixture.first));
  const use = async (target: Page, goal: string) => {
    await target.goto('/s2-acceptance');
    await target.getByRole('button', { name: '浏览共享预构建工作区', exact: true }).click();
    await target.getByRole('button', { name: '查看目标与路线', exact: true }).click();
    const detail = target.getByRole('article', { name: '共享工作区详情', exact: true });
    await expect(detail).toContainText('建议学习路线');
    await detail.getByRole('textbox', { name: '确认你的学习目标', exact: true }).fill(goal);
    await detail.getByRole('button', { name: '确认目标并开始学习', exact: true }).click();
    await expect.poll(async () => (await scene(target))?.learning_workspace_goal?.text).toBe(goal);
    return scene(target);
  };
  await page.setViewportSize({ width: 1440, height: 1000 });
  const firstScene = await use(page, '两周掌握微积分并完成期末复习');
  const otherScene = await use(other, '按照自己的节奏学习数学基础');
  expect(firstScene.learning_workspace_id).not.toBe(otherScene.learning_workspace_id);
  // Opening prepared material does not start a conversation or a model call.
  expect(firstScene.selected_chat).toBeNull(); expect(otherScene.selected_chat).toBeNull();
  expect(firstScene.published_book_ref).toEqual(fixture.first); expect(otherScene.published_book_ref).toEqual(fixture.first);
  const id = firstScene.learning_workspace_id, otherId = otherScene.learning_workspace_id;
  const instancePath = (value: string) => `/api/learning-workspaces/${encodeURIComponent(value)}/shared-release`;
  const manage = async (target: Page) => { await target.getByRole('button', { name: '学习资料', exact: true }).click(); await expect(target.getByRole('region', { name: '当前工作区', exact: true })).toBeVisible(); };
  await manage(page);
  const personal = page.getByRole('region', { name: '当前工作区', exact: true });
  const instance = page.getByRole('region', { name: '共享工作区个人实例', exact: true });
  const add = page.locator('.learning-add'); await add.locator('summary').click();
  await page.locator('.learning-add-book').filter({ hasText: title(fixture.second) }).getByRole('button', { name: '加入工作区', exact: true }).click();
  await expect(personal.locator('.learning-member')).toHaveCount(2);
  expect((await read(instancePath(id))).instance.version).toBe(1);
  await controlGet('/publish-next');
  await instance.getByRole('button', { name: '检查共享版本', exact: true }).click();
  await instance.getByRole('button', { name: '查看新版变化', exact: true }).click();
  const preview = page.getByRole('region', { name: '共享工作区更新预览', exact: true });
  await expect(preview).toContainText('第 1 版 → 第 2 版');
  await expect(preview).toContainText('两周掌握微积分并完成期末复习');
  await expect(preview).toContainText(`保留私人追加资料：${title(fixture.second)}`);
  await expect(preview).toContainText('公开学习目标已调整');
  await expect(preview).toContainText('主题联系或建议路线已调整');
  await preview.locator('summary').filter({ hasText: '核对资料版本' }).click();
  await expect(preview).toContainText(fixture.first.publication_id); await expect(preview).toContainText(fixture.new_first.publication_id);
  mkdirSync(output, { recursive: true }); await page.evaluate(() => document.fonts.ready);
  await instance.scrollIntoViewIfNeeded(); await page.screenshot({ path: resolve(output, 'shared-update-desktop.png'), fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 }); await preview.scrollIntoViewIfNeeded();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: resolve(output, 'shared-update-mobile.png'), fullPage: true });
  await preview.getByRole('button', { name: '保留当前版本', exact: true }).click();
  expect((await read(instancePath(id))).instance.version).toBe(1);
  await controlGet('/revoke-reader-material');
  await instance.getByRole('button', { name: '查看新版变化', exact: true }).click();
  await expect(preview.getByRole('button', { name: '选择更新到此版本', exact: true })).toBeDisabled();
  const denied = await page.request.get(`${backend}${bookPath(fixture.new_first)}`, { headers: { host: '127.0.0.1:4190', origin: fixture.origin, cookie: fixture.reader_cookie }});
  expect(denied.ok()).toBe(false);
  await preview.getByRole('button', { name: '保留当前版本', exact: true }).click(); await controlGet('/grant-reader-material');
  await instance.getByRole('button', { name: '查看新版变化', exact: true }).click();
  await preview.getByRole('button', { name: '选择更新到此版本', exact: true }).click();
  await expect(instance).toContainText('当前使用第 2 版');
  await expect.poll(async () => (await scene(page)).published_book_ref).toEqual(fixture.new_first);
  await expect(personal).toContainText('两周掌握微积分并完成期末复习');
  await expect(personal.locator('.learning-member')).toHaveCount(2);
  expect((await read(`/api/learning-workspaces/${id}`)).members.map((item: any) => item.published_book_ref)).toEqual(expect.arrayContaining([fixture.new_first, fixture.second]));
  expect((await read(instancePath(otherId), fixture.admin_cookie)).instance.version).toBe(1);
  expect((await scene(other)).published_book_ref).toEqual(fixture.first);
  expect(await read(bookPath(fixture.first))).toEqual(initialOldSource);
  const foreign = await page.request.get(`${backend}${instancePath(id)}`, { headers: { host: '127.0.0.1:4190', origin: fixture.origin, cookie: fixture.admin_cookie }}); expect(foreign.status()).toBe(404);

  await controlGet('/archive');
  await instance.getByRole('button', { name: '检查共享版本', exact: true }).click(); await expect(instance).toContainText('共享入口已归档');
  await page.getByRole('button', { name: '浏览共享预构建工作区', exact: true }).click();
  const shared = page.getByRole('region', { name: '共享预构建工作区', exact: true });
  await shared.locator('.shared-card').filter({ hasText: '第 2 版' }).getByRole('button', { name: '查看目标与路线', exact: true }).click();
  await expect(shared).toContainText('此共享工作区已归档'); await expect(shared.getByRole('button', { name: '确认目标并开始学习', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: '收起共享工作区', exact: true }).click();
  await page.setViewportSize({ width: 1440, height: 1000 }); await instance.scrollIntoViewIfNeeded();
  await page.screenshot({ path: resolve(output, 'shared-instance-desktop.png'), fullPage: true });
  await personal.getByRole('button', { name: '归档工作区', exact: true }).click();
  await expect(personal.locator('.learning-archive')).toContainText('工作区已归档');
  for (const button of await personal.locator('.learning-member').getByRole('button', { name: /^(开始阅读|继续阅读)$/ }).all()) await expect(button).toBeDisabled();
  await personal.getByRole('button', { name: '恢复工作区', exact: true }).click();
  await page.locator('.learning-space-card').filter({ hasText: '两周掌握微积分并完成期末复习' }).getByRole('button', { name: '打开工作区', exact: true }).click();
  await expect.poll(async () => (await scene(page))?.learning_workspace_goal?.text).toBe('两周掌握微积分并完成期末复习');
  expect((await scene(page)).published_book_ref).toEqual(fixture.new_first); expect((await scene(page)).selected_chat).toBe(firstScene.selected_chat);
  await controlGet('/restore');
  const finalCosts = await read('/api/admin/cost-summary', fixture.admin_cookie);
  for (const owner of ['account', 'platform']) {
    expect(finalCosts[owner].request_count).toBe(initialCosts[owner].request_count);
    expect(finalCosts[owner].confirmed_cost_micro_cny).toBe(initialCosts[owner].confirmed_cost_micro_cny);
    expect(finalCosts[owner].account_debit_micro_cny).toBe(initialCosts[owner].account_debit_micro_cny);
  }
  expect((await read('/api/learning-workspaces')).workspaces).toHaveLength(1);
  expect((await read('/api/learning-workspaces', fixture.admin_cookie)).workspaces).toHaveLength(1);
  expect(failures).toEqual([]); await otherContext.close();
});
