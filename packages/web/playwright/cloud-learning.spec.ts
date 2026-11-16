import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { createHash, randomBytes } from 'node:crypto';

// Opt-in Rust fixture: cargo test -p server cloud_learning_browser_host -- --ignored --nocapture
// Product APIs, admin writes and sources are forwarded unchanged to the actual HTTP server.
const backend = 'http://127.0.0.1:4200', control = 'http://127.0.0.1:4201';
const output = resolve('../../docs/performance/cloud-learning-20261020/browser');
test.afterEach(async ({ page, context }, info) => {
  if (info.status !== info.expectedStatus) {
    mkdirSync(output, { recursive: true });
    await page.screenshot({ path: resolve(output, 'failure.png'), fullPage: true });
    writeFileSync(resolve(output, 'failure.txt'), await page.locator('body').innerText());
  }
  await context.unrouteAll({ behavior: 'ignoreErrors' });
});

test('cloud subscription, personal course sprint, storage limits and desktop consent use actual services', async ({ page, context, browser }) => {
  page.setDefaultTimeout(15_000);
  const fixture = await page.request.get(`${control}/fixture`).then(r => r.json());
  const countsBefore = await page.request.get(`${control}/counts`).then(r => r.json());
  let readerCookie = fixture.reader_cookie;
  let interruptUpload = false, interrupted = false;
  const errors: string[] = [], requests: { method: string; path: string; status: number }[] = [];
  const adminContext = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
  const admin = await adminContext.newPage();
  for (const current of [page, admin]) current.on('pageerror', error => errors.push(error.message));
  const proxy = async (target: BrowserContext, cookie: () => string) => {
    await target.route(/\/api\/|\/books\//, async route => {
      const request = route.request(), url = new URL(request.url());
      const body = request.method() === 'POST' ? request.postDataJSON() : null;
      if (target === context && interruptUpload && body?.action === 'upload' && body.offset > 0) {
        interruptUpload = false; interrupted = true;
        await route.abort('connectionreset'); return;
      }
      const response = await route.fetch({ url: `${backend}${url.pathname}${url.search}`, headers: {
        ...await request.allHeaders(), host: '127.0.0.1:4200', origin: fixture.origin, cookie: cookie(),
      }});
      requests.push({ method: request.method(), path: url.pathname, status: response.status() });
      await route.fulfill({ response });
    });
  };
  await proxy(context, () => readerCookie); await proxy(adminContext, () => fixture.admin_cookie);
  const api = async (path: string, input?: unknown, adminUser = false) => {
    const response = await page.request.fetch(`${backend}/api${path}`, { method: input === undefined ? 'GET' : 'POST',
      headers: { origin: fixture.origin, cookie: adminUser ? fixture.admin_cookie : readerCookie, 'X-CSRF-Token': adminUser ? fixture.admin_csrf : fixture.reader_csrf },
      ...(input === undefined ? {} : { data: input }),
    });
    expect(response.ok(), `${path}: ${await response.text()}`).toBe(true);
    return response.json();
  };
  const openPlans = async () => {
    await page.getByRole('button', { name: '使用额度', exact: true }).first().click();
    await page.getByRole('button', { name: '套餐与权益 · 月付 / 冲刺', exact: true }).click();
    return page.getByRole('region', { name: '云端学习套餐', exact: true });
  };
  const pay = async (owner: string, title: string) => {
    await admin.goto(`http://127.0.0.1:4205/admin/users/${owner}`);
    await admin.getByRole('button', { name: '额度与套餐', exact: true }).click();
    await admin.getByRole('button', { name: '开通 / 续费套餐', exact: true }).click();
    const dialog = admin.getByRole('dialog', { name: '开通 / 续费套餐', exact: true });
    const option = dialog.locator('option').filter({ hasText: title });
    await dialog.locator('[name="product_id"]').selectOption(await option.getAttribute('value') ?? '');
    await dialog.locator('[name="channel"]').fill('浏览器验收无真实付款');
    await dialog.locator('[name="note"]').fill('隔离临时数据库验收');
    await dialog.locator('button[type=submit]').click();
    await expect(dialog).not.toBeVisible();
    await expect(admin.getByText(`已开通 ${title}`, { exact: false })).toBeVisible();
  };
  mkdirSync(output, { recursive: true });
  await page.goto('/');
  const plans = await openPlans();
  await expect(plans).toContainText('当前没有生效的云端套餐');
  for (const duration of ['1 天冲刺', '3 天冲刺', '7 天冲刺', '按月学习']) {
    await plans.getByRole('button', { name: duration, exact: true }).click();
    await expect(plans.locator('.product')).toHaveCount(3);
  }
  await expect(plans).not.toContainText('尚未定价草稿');
  await plans.getByRole('button', { name: '1 天冲刺', exact: true }).click();
  await plans.locator('.product').first().getByRole('button', { name: '查看开通方式' }).click();
  await expect(plans).toContainText('选择套餐不会扣款');
  expect((await api('/account/plans')).current).toBeNull();
  await page.locator('dialog').evaluate(el => { el.scrollTop = 0; });
  await page.screenshot({ path: resolve(output, 'plans-before-purchase-desktop.png') });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.locator('dialog').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
  await plans.locator('.duration').scrollIntoViewIfNeeded();
  await page.screenshot({ path: resolve(output, 'plans-mobile.png') });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await pay('A', '基础版1天（浏览器验收）');
  await plans.getByRole('button', { name: '刷新套餐与权益' }).click();
  await expect(plans).toContainText('当前：基础版1天（浏览器验收）');
  const firstPlan = (await api('/account/plans')).current;
  expect(firstPlan.expires_at - firstPlan.starts_at).toBe(86400);
  await pay('A', '进阶版月度（浏览器验收）');
  await plans.getByRole('button', { name: '刷新套餐与权益' }).click();
  await expect(plans).toContainText('已安排下一期：进阶版月度（浏览器验收）');
  const purchased = await api('/account/plans');
  expect(purchased.next.starts_at).toBe(purchased.current.expires_at);
  expect(purchased.current.tier).toBe('basic');
  await page.screenshot({ path: resolve(output, 'plans-current-next.png') });
  await admin.screenshot({ path: resolve(output, 'admin-purchase-receipt.png'), fullPage: true });
  await page.getByRole('button', { name: '返回阅读与对话', exact: true }).click();
  await page.reload();
  await page.getByRole('button', { name: '浏览共享预构建工作区', exact: true }).click();
  const course = page.locator('.shared-card').filter({ hasText: fixture.title });
  await course.getByRole('button', { name: '查看目标与路线', exact: true }).click();
  const detail = page.getByRole('article', { name: '共享工作区详情', exact: true });
  await detail.getByLabel('确认你的学习目标', { exact: true }).fill('我的三天引用与视频冲刺');
  await detail.getByLabel('用于考试冲刺', { exact: true }).check();
  const examDate = new Date(Date.now() + 3 * 86400000).toISOString().slice(0, 10);
  await detail.getByLabel('考试日期', { exact: true }).fill(examDate);
  await detail.getByLabel('已知考试范围', { exact: true }).fill('来源版本、原文引用与视频时间片段');
  await detail.getByLabel('每天可用分钟数（选填）', { exact: true }).fill('45');
  await detail.getByLabel('直接提问，跳过诊断', { exact: true }).check();
  await detail.getByRole('button', { name: '确认目标并开始学习', exact: true }).click();
  await expect(page.locator('.topbar')).toBeVisible();
  const workspaces = (await api('/learning-workspaces')).workspaces;
  const workspace = workspaces.find((item: any) => item.goal.text === '我的三天引用与视频冲刺');
  expect(workspace.members).toHaveLength(2);
  const workspaceId = workspace.learning_workspace_id;
  expect((await api(`/learning-workspaces/${workspaceId}/exam-sprint`)).sprint.settings.exam_date).toBe(examDate);
  expect((await api('/account/storage')).total_bytes).toBe(0);
  await page.getByRole('button', { name: '学习资料', exact: true }).click();
  await expect(page.locator('.learning-goal-heading')).toContainText('我的三天引用与视频冲刺');
  const current = page.getByRole('region', { name: '当前工作区', exact: true });
  const sprint = current.getByRole('region', { name: '考试冲刺安排', exact: true });
  await expect(sprint).toContainText(examDate); await expect(sprint).toContainText('每天 45 分钟');
  await sprint.getByRole('button', { name: '调整考试安排' }).click();
  await sprint.getByLabel('考试范围', { exact: true }).fill('只复习原文引用和片段定位');
  await sprint.getByRole('button', { name: '保存考试安排' }).click();
  await expect(sprint).toContainText('只复习原文引用和片段定位');
  await sprint.scrollIntoViewIfNeeded();
  await sprint.screenshot({ path: resolve(output, 'private-course-sprint.png') });
  const personal = current.getByRole('region', { name: '个人资料准备', exact: true });
  const input = personal.getByLabel('上传个人资料', { exact: true });
  await input.setInputFiles({ name: 'too-large.txt', mimeType: 'text/plain', buffer: Buffer.alloc(98305, 65) });
  await expect(personal).toContainText('单份资料不能超过');
  expect((await api('/account/storage')).total_bytes).toBe(0);
  interruptUpload = true;
  await input.setInputFiles({ name: 'interrupted.txt', mimeType: 'text/plain', buffer: Buffer.alloc(65536, 65) });
  await expect(personal.getByRole('button', { name: '继续这次上传' })).toBeVisible();
  expect(interrupted).toBe(true);
  expect((await api('/account/storage')).reserved_bytes).toBe(65536);
  await personal.getByRole('button', { name: '取消并释放空间', exact: true }).click();
  await expect.poll(async () => (await api('/account/storage')).total_bytes).toBe(0);
  await input.setInputFiles({ name: 'my-notes.txt', mimeType: 'text/plain', buffer: Buffer.alloc(81920, 65) });
  await expect(personal).toContainText('等待准备');
  expect((await api('/account/storage')).used_bytes).toBe(81920);
  await input.setInputFiles({ name: 'no-space.txt', mimeType: 'text/plain', buffer: Buffer.alloc(81920, 66) });
  await expect(personal).toContainText('剩余空间不足');
  expect((await api('/account/storage')).total_bytes).toBe(81920);
  await personal.getByRole('button', { name: '整理空间', exact: true }).click();
  await personal.screenshot({ path: resolve(output, 'storage-used-limit-and-cancel.png') });

  // Return the second account to the same app origin: first account fields must disappear.
  readerCookie = fixture.admin_cookie; await page.reload();
  await expect(page.locator('body')).not.toContainText('我的三天引用与视频冲刺');
  const secondPlans = await openPlans(); await expect(secondPlans).toContainText('当前没有生效的云端套餐');
  expect((await api('/account/storage')).total_bytes).toBe(0);
  const forbidden = await page.request.get(`${backend}/api/learning-workspaces/${workspaceId}`, { headers: { origin: fixture.origin, cookie: fixture.admin_cookie } });
  expect(forbidden.status()).toBe(404);
  await page.screenshot({ path: resolve(output, 'second-account-isolation.png') });
  await page.getByRole('button', { name: '返回阅读与对话', exact: true }).click();

  // Browser consent is real. The native loopback receiver is simulated only at its callback.
  readerCookie = fixture.reader_cookie;
  const verifier = randomBytes(32).toString('base64url'), state = randomBytes(32).toString('base64url');
  const redirect = 'http://127.0.0.1:4209/callback';
  let callback: URL | null = null;
  await context.route('http://127.0.0.1:4209/callback?**', async route => {
    callback = new URL(route.request().url());
    await route.fulfill({ contentType: 'text/html', body: '<p>浏览器验收：已返回桌面应用接收地址</p>' });
  });
  const query = new URLSearchParams({ desktop_authorize: '1', redirect_uri: redirect, code_challenge: createHash('sha256').update(verifier).digest('base64url'), state });
  await page.goto(`/?${query}`);
  const consent = page.getByRole('complementary', { name: '授权 Windows 登录' });
  await expect(consent).toContainText('使用 A 登录');
  await consent.screenshot({ path: resolve(output, 'windows-browser-consent.png') });
  await consent.getByRole('button', { name: '确认登录 Windows', exact: true }).click();
  await expect.poll(() => callback?.searchParams.get('state')).toBe(state);
  const exchange = await api('/auth/desktop/exchange', { code: callback!.searchParams.get('code'), code_verifier: verifier, redirect_uri: redirect });
  const desktopCookie = `${fixture.reader_cookie.split('=')[0]}=${exchange.session_token}`;
  const desktopMe = await page.request.get(`${backend}/api/auth/me`, { headers: { origin: fixture.origin, cookie: desktopCookie } });
  expect((await desktopMe.json()).user_id).toBe('A');
  expect(desktopCookie).not.toBe(fixture.reader_cookie);
  const countsAfter = await page.request.get(`${control}/counts`).then(r => r.json());
  expect(countsAfter.model_calls).toBe(countsBefore.model_calls);
  expect(countsAfter.publications).toBe(countsBefore.publications);
  expect(countsAfter.receipts - countsBefore.receipts).toBe(2);
  expect(errors).toEqual([]);
  writeFileSync(resolve(output, 'results.json'), JSON.stringify({
    test: 'cloud-learning-actual-http', data_scope: 'isolated temporary database; original fixture materials; no real payment or model execution',
    three_tiers_all_durations: true, draft_excluded: true, day_seconds: firstPlan.expires_at - firstPlan.starts_at,
    next_period_starts_at_current_expiry: true, private_course_materials: workspace.members.length,
    exam_date: examDate, interrupted_upload_reserved_bytes: 65536, cancel_released: true, used_bytes: 81920,
    per_file_limit_rejected: true, total_limit_rejected: true, second_account_isolated: true,
    desktop_browser_consent: true, independent_desktop_session: true, native_callback: 'simulated loopback receiver',
    counts_before: countsBefore, counts_after: countsAfter, page_errors: errors, requests,
  }, null, 2));
  await adminContext.unrouteAll({ behavior: 'ignoreErrors' }); await adminContext.close();
});
