import { expect, test, type BrowserContext } from '@playwright/test';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Start the ignored Rust q5_s1_browser_host and admin Vite on port 4184 first.
const backend = 'http://127.0.0.1:4188', control = 'http://127.0.0.1:4189';
const output = resolve('../../docs/performance/quota-q5-s1-20261018');
test.afterEach(async ({ page }, info) => {
  if (info.status !== info.expectedStatus) {
    await page.screenshot({ path: info.outputPath('failure.png'), fullPage: true });
    writeFileSync(info.outputPath('body.txt'), await page.locator('body').innerText());
  }
});

test('Q5/S1 real ledger summaries, late settlement, public versions and current material access', async ({ page, context, browser }) => {
  test.setTimeout(150_000);
  const fixture = await page.request.get(`${control}/fixture`).then(r => r.json());
  const failures: string[] = [];
  const adminContext = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
  const admin = await adminContext.newPage();
  for (const current of [page, admin]) current.on('pageerror', error => failures.push(error.message));
  const proxy = async (target: BrowserContext, cookie: string) => target.route(/\/api\/|\/books\//, async route => {
    const request = route.request(), url = new URL(request.url());
    const response = await route.fetch({ url: `${backend}${url.pathname}${url.search}`, headers: {
      ...await request.allHeaders(), host: 'reader.example', origin: 'https://reader.example', cookie,
    }});
    await route.fulfill({ response });
  });
  await proxy(context, fixture.reader_cookie);
  await proxy(adminContext, fixture.admin_cookie);
  const api = async (path: string, cookie = fixture.reader_cookie) => {
    const response = await page.request.get(`${backend}${path}`, { headers: { host: 'reader.example', origin: 'https://reader.example', cookie }});
    expect(response.ok(), `${path}: ${await response.text()}`).toBe(true);
    return response.json();
  };
  await context.route('**/q5-acceptance', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><html><head><meta name="viewport" content="width=device-width, initial-scale=1"></head><body><div id="app"></div><script type="module">
    import {createApp} from '/node_modules/.vite/deps/vue.js';
    import NetworkApp from '/src/NetworkApp.vue'; import '/reader-src/style.css';
    createApp(NetworkApp).mount('#app');
  </script></body></html>` }));
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto('/q5-acceptance');
  await page.getByRole('button', { name: '使用额度', exact: true }).click();
  await page.getByRole('button', { name: '按任务与工作区查看', exact: true }).click();
  const costs = page.getByRole('region', { name: '任务与工作区用量', exact: true });
  await costs.getByRole('button', { name: '按任务', exact: true }).click();
  const lesson = costs.locator('.cost-row').filter({ hasText: 'lesson-parent' });
  await expect(lesson.locator('.cost-period')).toHaveCount(2);
  await expect(lesson).toContainText('4 次调用');
  await expect(lesson).toContainText('1 次费用尚未确认');
  await expect(lesson).toContainText('2.7%');
  await expect(lesson).toContainText('2.3%');
  await expect(lesson).not.toContainText('5%');
  await lesson.getByRole('button', { name: '查看调用明细' }).click();
  await expect(costs.getByRole('region', { name: '所选用量明细' }).locator('li')).toHaveCount(4);
  await costs.getByRole('button', { name: '按工作区', exact: true }).click();
  const workspace = costs.locator('.cost-row').filter({ has: page.getByRole('heading', { name: '深度学习期末复习', exact: true }) });
  await expect(workspace).toContainText('4 次调用');
  await expect(costs.locator('.cost-row').filter({ hasText: '概率课程复习' })).toContainText('1 次调用');
  await costs.getByRole('button', { name: '账号级整理', exact: true }).click();
  await expect(costs.locator('.cost-row')).toContainText('1 次调用');
  await expect(costs.locator('.cost-row')).toContainText('0.5%');
  await costs.getByRole('button', { name: '查看调用明细' }).click();
  await expect(costs.getByRole('region', { name: '所选用量明细' }).locator('li')).toHaveCount(1);

  await admin.goto('http://127.0.0.1:4184/admin/usage');
  const platform = admin.getByRole('region', { name: '平台准备成本', exact: true });
  await expect(platform).toContainText('0.09');
  await expect(platform).toContainText('尚未完整核算');
  await expect(platform.locator('div').filter({ has: admin.locator('dt').filter({ hasText: /^账号扣减（元）$/ }) }).locator('dd')).toHaveText('0.00');
  const parent = admin.getByRole('region', { name: '统一成本汇总', exact: true }).getByRole('row').filter({ hasText: 'lesson-parent' });
  await parent.getByRole('button', { name: '查看归集明细' }).click();
  await expect(admin.getByRole('region', { name: '归集调用明细' }).getByRole('row')).toHaveCount(5);
  await page.request.get(`${control}/settle-late`);
  await costs.getByRole('button', { name: '刷新汇总', exact: true }).click();
  await costs.getByRole('button', { name: '按任务', exact: true }).click();
  await expect(lesson).toContainText('3.4%');
  await expect(lesson).toContainText('2.3%');
  await expect(lesson).not.toContainText('费用尚未确认');
  await admin.getByRole('button', { name: '刷新费用归集' }).click();
  await expect(parent).toContainText('0.08 / 0.08 / 0.00');
  const today = admin.getByRole('region', { name: /^今日 ·/ });
  await expect(today.locator('div').filter({ has: admin.locator('dt').filter({ hasText: /^账号扣减（元）$/ }) }).locator('dd')).toHaveText('0.092');
  mkdirSync(output, { recursive: true });
  await costs.scrollIntoViewIfNeeded();
  await page.screenshot({ path: resolve(output, 'reader-costs-desktop.png'), fullPage: true });
  await admin.getByRole('region', { name: '统一成本汇总', exact: true }).scrollIntoViewIfNeeded();
  await admin.screenshot({ path: resolve(output, 'admin-costs-desktop.png'), fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  await costs.getByRole('button', { name: '按工作区', exact: true }).click();
  await costs.scrollIntoViewIfNeeded();
  expect(await page.locator('dialog').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
  await page.screenshot({ path: resolve(output, 'reader-costs-mobile.png'), fullPage: true });

  const before = await api('/api/admin/cost-summary', fixture.admin_cookie);
  await admin.goto('http://127.0.0.1:4184/admin/workspace-releases');
  await admin.getByRole('button', { name: '发布新工作区', exact: true }).click();
  const dialog = admin.getByRole('dialog', { name: '发布新工作区', exact: true });
  for (const [name, value] of Object.entries({ release_id: 'q5-s1-course', name: '深度学习公开课程', publisher: '课程运营', public_goal: '掌握神经网络的基本概念与应用', applicability: '入门学习者', prerequisites: '基础代数', visible_to_users: 'A, B' })) await dialog.locator(`[name="${name}"]`).fill(value);
  const label = (ref: { book_id: string; publication_id: string }) => `${ref.book_id} · ${ref.publication_id}`;
  const materials = [fixture.first, fixture.second];
  for (const ref of materials) {
    await dialog.getByRole('group', { name: '选择确切资料版本', exact: true }).getByLabel(label(ref), { exact: true }).check();
    await dialog.getByRole('group', { name: label(ref), exact: true }).getByLabel('资料理解', { exact: true }).check();
  }
  for (const [part, title, description] of [['主题联系', '概念与应用', '对照两份资料中的核心概念'], ['建议路线', '先概念再练习', '阅读概念后完成应用练习']]) {
    await dialog.getByRole('button', { name: `添加${part}`, exact: true }).click();
    const group = dialog.getByRole('group', { name: `${part} 1`, exact: true });
    await group.getByLabel(`${part}标题`, { exact: true }).fill(title);
    await group.getByLabel(`${part}说明`, { exact: true }).fill(description);
    for (const ref of materials) await group.getByLabel(label(ref), { exact: true }).check();
  }
  const submit = async (title: string) => {
    const form = admin.getByRole('dialog', { name: title, exact: true });
    await form.getByRole('button', { name: '确认提交', exact: true }).click();
    await expect(form).not.toBeVisible();
    await expect(admin.getByText('操作已完成', { exact: false })).toBeVisible();
  };
  await submit('发布新工作区');
  const row = (version: number) => admin.getByRole('row').filter({ hasText: `q5-s1-course · v${version}` });
  await expect(row(1)).toContainText('完整可用');
  await row(1).getByRole('button', { name: '查看版本', exact: true }).click();
  const detail = admin.getByRole('region', { name: '共享工作区版本详情', exact: true });
  await expect(detail).toContainText('发布时：完整可用');
  await expect(detail).toContainText('当前：完整可用');
  await detail.scrollIntoViewIfNeeded();
  await admin.screenshot({ path: resolve(output, 'shared-release-desktop.png'), fullPage: true });
  const downloadPromise = admin.waitForEvent('download');
  await row(1).getByRole('button', { name: '导出公开清单', exact: true }).click();
  const download = await downloadPromise;
  const exported = JSON.parse(readFileSync((await download.path())!, 'utf8'));
  expect(Object.keys(exported).sort()).toEqual(['manifest', 'release_id', 'schema', 'version']);
  expect(exported.manifest.materials.map((item: any) => item.published_book_ref)).toEqual(materials);
  for (const privateValue of [fixture.workspace, fixture.workspace2, '深度学习期末复习', 'visible_to_users', 'input_directory', 'rate_snapshot']) expect(JSON.stringify(exported)).not.toContain(privateValue);
  await row(1).getByRole('button', { name: '修订为新版本', exact: true }).click();
  await admin.getByRole('dialog', { name: '发布工作区新版本', exact: true }).locator('[name="public_goal"]').fill('掌握神经网络并完成应用练习');
  await submit('发布工作区新版本');
  await expect(row(1)).toBeVisible(); await expect(row(2)).toBeVisible();
  const path = '/api/workspace-releases/q5-s1-course/versions';
  expect(await api(`${path}/1/export`)).toEqual(exported);
  expect((await api(`${path}/2`)).manifest.public_goal).toBe('掌握神经网络并完成应用练习');
  await page.request.get(`${control}/revoke-reader-material`);
  await admin.getByRole('button', { name: '刷新发布', exact: true }).click();
  await expect(row(2)).toContainText('尚未完整就绪');
  await row(2).getByRole('button', { name: '查看版本', exact: true }).click();
  await expect(detail).toContainText('发布时：完整可用');
  await expect(detail).toContainText('当前：尚未完整就绪');
  expect((await api(`${path}/2`)).availability.status).toBe('incomplete');
  expect((await api(`${path}/2`, fixture.admin_cookie)).availability.status).toBe('ready');
  await page.request.get(`${control}/grant-reader-material`);
  expect((await api(`${path}/2`)).availability.status).toBe('ready');
  for (const cookie of [fixture.reader_cookie, fixture.admin_cookie]) {
    expect((await api('/api/workspace-releases', cookie)).total).toBe(2);
    expect(await api(`${path}/1/export`, cookie)).toEqual(exported);
  }
  const after = await api('/api/admin/cost-summary', fixture.admin_cookie);
  for (const owner of ['account', 'platform']) {
    expect(after[owner].request_count).toBe(before[owner].request_count);
    expect(after[owner].confirmed_cost_micro_cny).toBe(before[owner].confirmed_cost_micro_cny);
    expect(after[owner].account_debit_micro_cny).toBe(before[owner].account_debit_micro_cny);
  }
  await admin.getByRole('button', { name: '刷新发布', exact: true }).click();
  await row(2).getByRole('button', { name: '查看版本', exact: true }).click();
  await expect(detail).toContainText('当前：完整可用');
  await admin.setViewportSize({ width: 390, height: 844 });
  await detail.scrollIntoViewIfNeeded();
  expect(await admin.locator('body').evaluate(el => el.scrollWidth <= window.innerWidth)).toBe(true);
  await admin.screenshot({ path: resolve(output, 'shared-release-mobile.png'), fullPage: true });
  expect(failures).toEqual([]);
  await adminContext.close();
});
