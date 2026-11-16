import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Uses video_workspace_browser_host: real publications, media bytes, personal scopes and shared updates.
const backend = 'http://127.0.0.1:4190', control = 'http://127.0.0.1:4191';
const output = resolve('../../target/video-integration-20261018/browser');
async function scene(page: Page) {
  return page.evaluate(async () => (await import('/src/network-context.ts')).network.value.workspace);
}
test.afterEach(async ({ page }, info) => {
  if (info.status !== info.expectedStatus) {
    await page.screenshot({ path: info.outputPath('failure.png'), fullPage: true });
    writeFileSync(info.outputPath('body.txt'), await page.locator('body').innerText());
  }
});

test('mixed shared workspace plays video, keeps question context, and only adopts the selected shared release', async ({ page, context, browser }) => {
  test.setTimeout(150_000); page.setDefaultTimeout(20_000);
  const fixture = await page.request.get(`${control}/fixture`).then(response => response.json());
  const failures: string[] = [], requests: string[] = [];
  const otherContext = await browser.newContext({ baseURL: 'http://127.0.0.1:4174', viewport: { width: 1440, height: 1000 } });
  const other = await otherContext.newPage();
  const proxy = async (target: BrowserContext, cookie: string) => {
    await target.route(/\/api\/|\/books\//, async route => {
      const request = route.request(), url = new URL(request.url()); requests.push(url.pathname);
      const response = await route.fetch({ url: `${backend}${url.pathname}${url.search}`, headers: {
        ...await request.allHeaders(), host: '127.0.0.1:4190', origin: fixture.origin, cookie,
      } });
      await route.fulfill({ response });
    });
    await target.route('**/video-workspace-acceptance', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><html><head><meta name="viewport" content="width=device-width, initial-scale=1"></head><body><div id="app"></div><script type="module">
      import {createApp} from '/node_modules/.vite/deps/vue.js';
      import NetworkApp from '/src/NetworkApp.vue'; import '/src/style.css';
      createApp(NetworkApp).mount('#app');
    </script></body></html>` }));
  };
  const read = async (path: string, cookie = fixture.reader_cookie) => {
    const response = await page.request.get(`${backend}${path}`, { headers: { origin: fixture.origin, cookie } });
    expect(response.ok(), `${path}: ${await response.text()}`).toBe(true); return response.json();
  };
  const publicationPath = (reference: { book_id: string; publication_id: string }, leaf: string) => `/api/books/${encodeURIComponent(reference.book_id)}/publications/${encodeURIComponent(reference.publication_id)}/${leaf}`;
  const manage = async (target: Page) => {
    await target.getByRole('button', { name: '学习资料', exact: true }).click();
    await expect(target.getByRole('region', { name: '当前工作区', exact: true })).toBeVisible();
  };
  const use = async (target: Page, goal: string) => {
    await target.goto('/video-workspace-acceptance');
    await target.getByRole('button', { name: '浏览共享预构建工作区', exact: true }).click();
    await target.locator('.shared-card').filter({ hasText: fixture.release_name ?? '书籍与视频共同学习' }).getByRole('button', { name: '查看目标与路线', exact: true }).click();
    const detail = target.getByRole('article', { name: '共享工作区详情', exact: true });
    await detail.getByRole('textbox', { name: '确认你的学习目标', exact: true }).fill(goal);
    await detail.getByRole('button', { name: '确认目标并开始学习', exact: true }).click();
    await expect.poll(async () => (await scene(target))?.learning_workspace_goal?.text).toBe(goal);
    return scene(target);
  };
  for (const target of [page, other]) target.on('pageerror', error => failures.push(error.message));
  await proxy(context, fixture.reader_cookie); await proxy(otherContext, fixture.admin_cookie);
  await page.setViewportSize({ width: 1440, height: 1000 });
  const initialCosts = await read('/api/admin/cost-summary', fixture.admin_cookie);
  const initialVideo = await read(publicationPath(fixture.video1, 'video_context'));
  const first = await use(page, '结合书与视频复习课程');
  const second = await use(other, '保留我的原版学习计划');
  expect(first.published_book_ref).toEqual(fixture.book);
  expect(first.learning_workspace_id).not.toBe(second.learning_workspace_id);
  await expect(page.locator('.reader-pane')).toBeVisible();
  await page.locator('.agent-input textarea').fill('联系书中的内容解释视频');
  await manage(page);
  await page.locator('.learning-member').nth(1).getByRole('button', { name: '开始阅读', exact: true }).click();
  const video = page.locator('.video-reader video');
  await video.waitFor();
  await page.waitForFunction(() => (document.querySelector('.video-reader video') as HTMLVideoElement)?.readyState >= 2);
  await expect(page.locator('.agent-input textarea')).toHaveValue('联系书中的内容解释视频');
  expect((await scene(page)).learning_workspace_id).toBe(first.learning_workspace_id);
  expect((await scene(page)).published_book_ref).toEqual(fixture.video1);
  await expect(page.locator('.left-rail')).toHaveCount(0);
  await expect(page.locator('.guide-chapter-button')).toHaveCount(0);
  await video.evaluate((element: HTMLVideoElement) => { element.currentTime = 2; });
  await page.waitForFunction(() => { const element = document.querySelector('.video-reader video') as HTMLVideoElement; return !element.seeking && element.currentTime >= 2; });
  await page.getByRole('button', { name: '选择时间范围', exact: true }).click();
  const range = page.getByRole('form', { name: '时间范围编辑' });
  await range.getByLabel('起点（秒）').fill('1'); await range.getByLabel('终点（秒）').fill('4');
  await range.getByRole('button', { name: '保存时间范围' }).click();
  await expect(page.getByLabel('已选时间范围')).toContainText('0:01');
  await page.getByRole('button', { name: '框选画面', exact: true }).click();
  const bounds = (await page.getByLabel('画面框选区域').boundingBox())!;
  await page.mouse.move(bounds.x + bounds.width * .1, bounds.y + bounds.height * .2);
  await page.mouse.down(); await page.mouse.move(bounds.x + bounds.width * .65, bounds.y + bounds.height * .75); await page.mouse.up();
  await expect(page.getByAltText('待发送的问题图片', { exact: true })).toBeVisible();
  await page.locator('.agent-input textarea').fill('解释我选的画面和时间段');
  const getDraft = () => page.evaluate(async () => {
    const { api } = await import('/src/api.ts');
    return api.videoDraft({ window_id: sessionStorage.getItem('video-question-window')!, action: 'get' });
  });
  await expect.poll(async () => (await getDraft()).draft?.message).toBe('解释我选的画面和时间段');
  const draft = (await getDraft()).draft!;
  expect(draft.context.publication).toEqual(fixture.video1);
  expect(draft.context.range).toMatchObject({ start_ms: 1000, end_ms: 4000 });
  expect(draft.context.region).toBeTruthy(); expect(draft.context.image.origin).toBe('selection');
  await page.reload();
  await page.locator('.learning-space-card.selected').getByRole('button', { name: '打开工作区', exact: true }).click();
  await expect(page.locator('.agent-input textarea')).toHaveValue('解释我选的画面和时间段');
  await expect(page.getByAltText('待发送的问题图片', { exact: true })).toBeVisible();
  expect((await getDraft()).draft?.context).toEqual(draft.context);
  mkdirSync(output, { recursive: true }); await page.screenshot({ path: resolve(output, 'mixed-video-draft.png'), fullPage: true });
  await page.getByRole('button', { name: '取消草稿', exact: true }).click();
  const published = await page.request.get(`${control}/publish-next`); expect(published.ok()).toBe(true);
  // Stay idle beyond two standalone adoption intervals. A personal instance must remain on v1.
  await page.waitForTimeout(6500);
  expect((await scene(page)).published_book_ref).toEqual(fixture.video1);
  expect(requests.some(path => path.endsWith('/reader/adopt_same_source_publication'))).toBe(false);
  await manage(page);
  const instance = page.getByRole('region', { name: '共享工作区个人实例', exact: true });
  await instance.getByRole('button', { name: '检查共享版本', exact: true }).click();
  await instance.getByRole('button', { name: '查看新版变化', exact: true }).click();
  const preview = page.getByRole('region', { name: '共享工作区更新预览', exact: true });
  await expect(preview).toContainText('第 1 版 → 第 2 版');
  await preview.getByRole('button', { name: '选择更新到此版本', exact: true }).click();
  await expect.poll(async () => (await scene(page)).published_book_ref).toEqual(fixture.video2);
  await expect(instance).toContainText('当前使用第 2 版');
  await page.locator('.learning-member').nth(1).getByRole('button', { name: '继续阅读', exact: true }).click();
  await expect(video).toBeVisible();
  await expect.poll(async () => (await scene(page))?.learning_workspace_goal?.text).toBe('结合书与视频复习课程');
  expect((await scene(other)).published_book_ref).toEqual(fixture.book);
  expect((await read(`/api/learning-workspaces/${second.learning_workspace_id}/shared-release`, fixture.admin_cookie)).instance.version).toBe(1);
  expect(await read(publicationPath(fixture.video1, 'video_context'))).toEqual(initialVideo);
  const finalCosts = await read('/api/admin/cost-summary', fixture.admin_cookie);
  for (const owner of ['account', 'platform']) expect(finalCosts[owner].request_count).toBe(initialCosts[owner].request_count);
  await page.screenshot({ path: resolve(output, 'mixed-video-updated.png'), fullPage: true });
  expect(failures).toEqual([]);
  writeFileSync(resolve(output, 'mixed-video-verification.json'), JSON.stringify({ status: 'passed', first_scope: first.learning_workspace_id, second_scope: second.learning_workspace_id, video1: fixture.video1, video2: fixture.video2, range: draft.context.range, image_origin: draft.context.image.origin, automatic_adoptions: 0, model_requests: 0, errors: failures }, null, 2));
  await otherContext.close();
});
