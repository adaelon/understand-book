import { expect, test, type Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Start server's ignored w5_browser_host before this test. The actual multi-user
// service handles every product request against its temporary published books.
const control = 'http://127.0.0.1:4177';
const backend = 'http://127.0.0.1:4176';
const output = resolve('../../docs/performance/workspace-w4-w5-20261018');

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

test('W5 creates two workspaces and preserves cross-book conversation, goals, drafts and waiting results', async ({ page, context }) => {
  test.setTimeout(120_000);
  page.setDefaultTimeout(15_000);
  const fixture = await page.request.get(`${control}/fixture`).then(response => response.json());
  const failures: string[] = [];
  page.on('pageerror', error => failures.push(error.message));
  page.on('console', message => { if (message.type() === 'error') console.log('Browser:', message.text()); });
  await context.route(/\/api\/|\/books\//, async route => {
    const request = route.request(), url = new URL(request.url());
    const headers = { ...await request.allHeaders(), host: 'reader.example', origin: 'https://reader.example', cookie: fixture.cookie };
    const response = await route.fetch({ url: `${backend}${url.pathname}${url.search}`, headers, timeout: 45_000 });
    if (!response.ok()) console.log('HTTP', response.status(), request.method(), url.pathname, await response.text());
    await route.fulfill({ response });
  });
  await context.route('**/w5-acceptance', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><html><head><meta name="viewport" content="width=device-width, initial-scale=1"></head><body><div id="app"></div><script type="module">
    import {createApp} from '/node_modules/.vite/deps/vue.js';
    import NetworkApp from '/src/NetworkApp.vue'; import '/src/style.css';
    createApp(NetworkApp).mount('#app');
  </script></body></html>` }));
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto('/w5-acceptance');
  const create = async (goal: string) => {
    await page.getByLabel('这次想学会什么？').fill(goal);
    await page.getByRole('button', { name: '创建工作区', exact: true }).click();
    await expect(page.getByRole('region', { name: '当前工作区', exact: true })).toContainText(goal);
  };
  const add = async (id: string) => {
    const title = id.replaceAll('-', ' ');
    const details = page.locator('.learning-add');
    if (await details.getAttribute('open') === null) await details.locator('summary').click();
    await page.locator('.learning-add-book').filter({ hasText: title }).getByRole('button', { name: '加入工作区' }).click();
    await expect(page.locator('.learning-member').filter({ hasText: title })).toBeVisible();
  };
  const open = async (id: string) => {
    await page.locator('.learning-member').filter({ hasText: id.replaceAll('-', ' ') }).getByRole('button', { name: /^(开始阅读|继续阅读)$/ }).click();
    await expect(page.locator('.topbar')).toBeVisible();
    await expect(page.getByRole('textbox', { name: '围绕当前阅读内容提问' })).toBeEnabled();
  };
  const manage = () => page.getByRole('button', { name: '学习资料', exact: true }).click();
  const enter = async (goal: string) => {
    await page.locator('.learning-space-card').filter({ hasText: goal }).getByRole('button', { name: '打开工作区' }).click();
    await expect.poll(async () => (await scene(page))?.learning_workspace_goal?.text).toBe(goal);
  };
  const input = page.getByRole('textbox', { name: '围绕当前阅读内容提问' });
  const ask = async (message: string) => {
    await input.fill(message);
    await page.getByRole('button', { name: '发送', exact: true }).click();
  };

  await create('学习深度学习');
  await expect(page.locator('.learning-empty')).toContainText('工作区已创建');
  await add(fixture.first.book_id);
  await add(fixture.second.book_id);
  await open(fixture.first.book_id);
  await ask('你好，开始学习');
  await expect(page.locator('.a-msg')).toContainText(['你好！']);
  const firstScene = await scene(page), aChat = firstScene.selected_chat, aScope = firstScene.learning_workspace_id;
  await input.fill('A 工作区尚未发送的问题');
  await manage();
  await open(fixture.second.book_id);
  await expect(input).toHaveValue('A 工作区尚未发送的问题');
  expect((await scene(page)).selected_chat).toBe(aChat);
  await page.request.post(`${control}/model`, { data: { book: 'second' } });
  await ask('请接着这本资料解释并给出原文');
  await expect(page.locator('.agent-source-button').last()).toBeEnabled();
  await page.locator('.agent-source-button').last().click();
  await expect(page.getByRole('dialog', { name: '回答来源' })).toBeVisible();
  await expect(page.locator('.source-excerpt')).not.toBeEmpty();
  await page.getByRole('button', { name: '关闭来源' }).click();
  expect((await scene(page)).selected_chat).toBe(aChat);
  await input.fill('A 独立草稿');

  await manage();
  await create('期末考试冲刺');
  await add(fixture.first.book_id);
  await open(fixture.first.book_id);
  await expect(input).toHaveValue('');
  const bScope = (await scene(page)).learning_workspace_id;
  expect(bScope).not.toBe(aScope);
  await input.fill('B 独立草稿');
  await manage();
  await enter('学习深度学习');
  await expect(input).toHaveValue('A 独立草稿');
  expect((await scene(page)).published_book_ref).toEqual(fixture.second);
  expect((await scene(page)).selected_chat).toBe(aChat);

  await manage();
  await page.getByRole('button', { name: '修改大目标' }).click();
  await page.getByRole('textbox', { name: '工作区大目标', exact: true }).fill('三天内复习深度学习期末考点');
  await page.getByRole('button', { name: '保存大目标' }).click();
  await expect(page.getByRole('region', { name: '当前工作区', exact: true })).toContainText('第 2 版');
  await enter('三天内复习深度学习期末考点');
  await expect(input).toHaveValue('A 独立草稿');
  await page.reload();
  await enter('三天内复习深度学习期末考点');
  await expect(input).toHaveValue('A 独立草稿');
  expect((await scene(page)).selected_chat).toBe(aChat);

  await page.request.post(`${control}/model`, { data: { blocked: true } });
  await ask('这个回答完成后仍留在 A');
  await expect(page.getByRole('button', { name: '停止', exact: true })).toBeVisible();
  await manage();
  await enter('期末考试冲刺');
  await expect(input).toHaveValue('B 独立草稿');
  await page.request.post(`${control}/release`);
  await expect(page.locator('.a-msg')).toHaveCount(0);
  await manage();
  await enter('三天内复习深度学习期末考点');
  await expect(page.locator('.a-msg').last()).toContainText('你好！');
  await expect(page.locator('.turn')).toHaveCount(3);

  mkdirSync(output, { recursive: true });
  await page.evaluate(() => document.fonts.ready);
  await page.screenshot({ path: resolve(output, 'workspace-reader-desktop.png'), fullPage: true });
  await manage();
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.getByRole('region', { name: '当前工作区', exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: resolve(output, 'workspace-management-mobile.png'), fullPage: true });
  await enter('期末考试冲刺');
  await expect(page.locator('.topbar')).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: resolve(output, 'workspace-reader-mobile.png'), fullPage: true });
  expect(failures).toEqual([]);
});
