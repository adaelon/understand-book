import { expect, test, type Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Uses the real multi-user HTTP service hosted by ignored w5_browser_host.
// Only the model replies and the local reverse-proxy identity are controlled.
const control = 'http://127.0.0.1:4177', backend = 'http://127.0.0.1:4176';
const output = resolve('../../docs/performance/workspace-w6-20261018');
async function scene(page: Page) {
  return page.evaluate(async () => {
    const path = '/src/network-context.ts';
    return (await import(path)).network.value.workspace;
  });
}
test.afterEach(async ({ page }, info) => {
  if (info.status !== info.expectedStatus) {
    await page.screenshot({ path: info.outputPath('failure.png'), fullPage: true });
    writeFileSync(info.outputPath('body.txt'), await page.locator('body').innerText());
  }
});

test('W6 preserves original chat and sources, links only selected history and keeps the current reading scene', async ({ page, context }) => {
  test.setTimeout(120_000);
  page.setDefaultTimeout(15_000);
  const fixture = await page.request.get(`${control}/fixture`).then(response => response.json());
  const failures: string[] = [];
  page.on('pageerror', error => failures.push(error.message));
  await context.route(/\/api\/|\/books\//, async route => {
    const request = route.request(), url = new URL(request.url());
    const headers = { ...await request.allHeaders(), host: 'reader.example', origin: 'https://reader.example', cookie: fixture.cookie };
    const response = await route.fetch({ url: `${backend}${url.pathname}${url.search}`, headers, timeout: 45_000 });
    if (!response.ok() && !url.pathname.endsWith('source_manifest.json')) console.log('HTTP', response.status(), request.method(), url.pathname, await response.text());
    await route.fulfill({ response });
  });
  await context.route('**/w6-acceptance', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><html><head><meta name="viewport" content="width=device-width, initial-scale=1"></head><body><div id="app"></div><script type="module">
    import {createApp} from '/node_modules/.vite/deps/vue.js';
    import NetworkApp from '/src/NetworkApp.vue'; import '/reader-src/style.css';
    createApp(NetworkApp).mount('#app');
  </script></body></html>` }));
  const api = async (path: string) => {
    const response = await page.request.get(`${backend}${path}`, { headers: { host: 'reader.example', cookie: fixture.cookie } });
    expect(response.ok()).toBe(true);
    return response.json();
  };
  const originalRecord = async (chat: string, through: number) => {
    const detail = await api(`/api/me/learning-records/${chat}?through_seq=${through}`);
    // The read projection has its own generation time; original facts do not.
    delete detail.recap.generated_at;
    return detail;
  };
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto('/w6-acceptance');
  await page.locator('.network-book-card').filter({ hasText: fixture.first.book_id.replaceAll('-', ' ') }).click();
  const input = page.getByRole('textbox', { name: '围绕当前阅读内容提问' });
  const ask = async (message: string) => {
    await input.fill(message);
    await page.getByRole('button', { name: '发送', exact: true }).click();
    await expect(page.locator('.a-msg').last()).not.toBeEmpty();
    await expect(input).toBeEnabled();
  };
  const manage = async () => {
    await page.getByRole('button', { name: '学习资料', exact: true }).click();
  };
  await page.request.post(`${control}/model`, { data: { book: 'first' } });
  await ask('旧记录：我想分清速度和变化率，请展示原文');
  await expect(page.locator('.agent-source-button').last()).toBeEnabled();
  const originalScene = await scene(page), originalChat = originalScene.selected_chat;
  expect(originalScene.learning_workspace_id).toBeNull();
  const old = (await api('/api/me/learning-records')).records.find((record: any) => record.session_id === originalChat);
  const originalDetail = await originalRecord(originalChat, old.through_seq);
  expect(originalDetail.sources.length).toBeGreaterThan(0);
  await page.locator('.topbar .tools-toggle').click();
  await page.locator('.topbar').getByRole('button', { name: '新对话', exact: true }).click();
  await expect(page.locator('.a-msg')).toHaveCount(0);
  await page.request.post(`${control}/model`, { data: {} });
  await ask('你好，这是另一段尚未选择的交流');
  const unselectedChat = (await scene(page)).selected_chat;
  expect(unselectedChat).not.toBe(originalChat);

  await manage();
  await page.getByRole('button', { name: '旧聊天与学习记录', exact: true }).click();
  const archive = page.getByRole('region', { name: '已有聊天与学习记录', exact: true });
  await expect(archive.locator('.record-choice')).toHaveCount(2);
  await archive.locator('.record-choice').filter({ hasText: old.title }).getByRole('button', { name: '查看原记录' }).click();
  const preview = page.locator('.record-preview');
  await expect(preview.getByRole('region', { name: '原聊天', exact: true })).toContainText('旧记录：我想分清速度和变化率');
  await expect(preview.locator('.source-excerpt')).not.toBeEmpty();
  await page.getByRole('button', { name: '关闭原记录' }).click();
  expect((await scene(page)).selected_chat).toBe(unselectedChat);
  await page.getByRole('button', { name: '收起旧记录', exact: true }).click();

  const goal = '期末考试冲刺：讲清速度与变化率';
  await page.getByLabel('这次想学会什么？').fill(goal);
  await page.getByRole('button', { name: '创建工作区', exact: true }).click();
  await expect(page.getByRole('region', { name: '当前工作区', exact: true })).toContainText(goal);
  await page.locator('.learning-add-book').filter({ hasText: fixture.first.book_id.replaceAll('-', ' ') }).getByRole('button', { name: '加入工作区' }).click();
  await page.locator('.learning-member').getByRole('button', { name: '开始阅读' }).click();
  await expect(input).toBeEnabled();
  const targetScene = await scene(page), scope = targetScene.learning_workspace_id;
  await input.fill('工作区里未发送的问题');
  await manage();
  await page.getByRole('button', { name: '带入已有学习记录', exact: true }).click();
  const chooser = page.getByRole('region', { name: '带入已有学习记录', exact: true });
  await expect(chooser.locator('.record-choice')).toHaveCount(2);
  const selected = chooser.locator('.record-choice').filter({ hasText: old.title });
  await selected.getByRole('button', { name: '查看原记录' }).click();
  await expect(preview.locator('.source-excerpt')).not.toBeEmpty();
  expect((await scene(page)).selected_chat).toBe(targetScene.selected_chat);
  expect((await scene(page)).learning_workspace_id).toBe(scope);
  await page.getByRole('button', { name: '关闭原记录' }).click();
  await selected.getByRole('checkbox').check();
  await chooser.getByRole('button', { name: '确认带入 1 份记录' }).click();
  await expect(selected.getByRole('checkbox')).toBeDisabled();
  await expect(chooser).toContainText('已带入 1 份学习记录');
  const linked = await api(`/api/learning-workspaces/${scope}/records`);
  expect(linked.records.map((record: any) => record.selection.chat_session_id)).toEqual([originalChat]);
  expect(await originalRecord(originalChat, old.through_seq)).toEqual(originalDetail);
  mkdirSync(output, { recursive: true });
  await page.screenshot({ path: resolve(output, 'selected-records-desktop.png'), fullPage: true });

  await page.reload();
  await page.locator('.learning-space-card').filter({ hasText: goal }).getByRole('button', { name: '管理工作区' }).click();
  await page.getByRole('button', { name: '带入已有学习记录', exact: true }).click();
  await expect(selected.getByRole('checkbox')).toBeDisabled();
  await page.locator('.learning-space-card').filter({ hasText: goal }).getByRole('button', { name: '打开工作区' }).click();
  await expect(input).toHaveValue('工作区里未发送的问题');
  await page.getByRole('button', { name: '我的理解', exact: true }).click();
  const imported = page.getByRole('region', { name: '带入的原记录', exact: true });
  await expect(imported).toContainText(old.title);
  await expect(imported.locator('article')).toHaveCount(1);
  await imported.getByRole('button', { name: '查看原记录' }).click();
  await expect(preview.locator('.source-excerpt')).not.toBeEmpty();
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: resolve(output, 'original-record-mobile.png'), fullPage: true });
  await page.getByRole('button', { name: '关闭原记录' }).click();
  expect((await scene(page)).selected_chat).toBe(targetScene.selected_chat);
  expect(await originalRecord(originalChat, old.through_seq)).toEqual(originalDetail);
  expect(failures).toEqual([]);
});
