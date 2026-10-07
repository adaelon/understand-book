import { test, expect, type Page, type BrowserContext } from '@playwright/test';

async function host(context: BrowserContext, lostResponse = false) {
  const posts: unknown[] = [], failures: string[] = [];
  const observeErrors = (page: Page) => page.on('console', message => {
    if (message.type() === 'error' && /(?:TypeError|ReferenceError):/.test(message.text())) failures.push(message.text());
  });
  context.pages().forEach(observeErrors);
  context.on('page', observeErrors);
  let holdPdf: Promise<void> | null = null, releasePdf = () => {}, pdfRequests = 0;
  context.on('response', response => {
    const path = new URL(response.url()).pathname;
    if (response.status() >= 400 && !path.includes('/auth/me') && !(response.status() === 404 && path.endsWith('/source_manifest')))
      failures.push(`${response.status()} ${path}`);
  });
  await context.route('https://localhost:4189/api/**', async route => {
    const request = route.request(), path = new URL(request.url()).pathname;
    const admission = path.endsWith('/agent/runs') && request.method() === 'POST';
    if (!admission && !path.endsWith('/pdf/original')) { await route.continue(); return; }
    const response = await route.fetch();
    if (admission) {
      posts.push(request.postDataJSON());
      if (lostResponse) { lostResponse = false; await route.abort('connectionreset'); return; }
    } else { pdfRequests++; if (holdPdf) await holdPdf; }
    await route.fulfill({ response });
  });
  return { posts, failures, pdfRequests: () => pdfRequests,
    holdPdf: () => { holdPdf = new Promise(resolve => { releasePdf = resolve; }); }, releasePdf: () => releasePdf() };
}
async function login(page: Page, owner = 'A') {
  await page.goto('https://localhost:4189/');
  await page.getByLabel('账号', { exact: true }).fill(owner);
  await page.getByLabel('密码', { exact: true }).fill('fixture-only-password');
  await page.getByRole('button', { name: '登录', exact: true }).click();
  await expect(page.getByRole('heading', { name: '选择阅读材料' })).toBeVisible();
}
async function open(page: Page, book = 'mu5-x') {
  await page.getByRole('button', { name: book, exact: true }).click();
  await expect(page.locator('.workspace-shell')).toBeVisible();
  await expect(page.locator('.new-chat')).toBeEnabled();
}
async function logout(page: Page) {
  const done = page.waitForResponse(r => r.url().endsWith('/api/auth/logout'));
  if (!await page.locator('.topbar').isVisible()) {
    const expand = page.getByRole('button', { name: '展开工具栏', exact: true });
    if (await expand.isVisible()) await expand.click();
    else await page.getByRole('button', { name: '菜单', exact: true }).click();
  }
  await page.locator('.network-account summary').click();
  await page.getByRole('button', { name: '退出登录', exact: true }).click();
  await done;
  await expect(page.getByRole('button', { name: '登录', exact: true })).toBeEnabled();
}

for (const width of [1440, 390]) test(`idle reattachment restores chat and unsent draft at ${width}px`, async ({ page, context }) => {
  const seen = await host(context);
  let attachment = '';
  page.on('request', request => {
    if (request.method() === 'POST' && request.url().includes('/api/workspaces/')) {
      attachment = request.postDataJSON()?.attachment_id || attachment;
    }
  });
  await page.setViewportSize({ width, height: 900 });
  await login(page); await open(page);
  if (width === 390) await page.getByRole('button', { name: '问答', exact: true }).click();
  const input = page.locator('textarea[data-workspace-input="agent"]');
  for (const question of ['第一轮问题', '最新一轮问题']) {
    if (width === 390 && !await input.isVisible()) await page.getByRole('button', { name: '问答', exact: true }).click();
    await input.fill(question);
    await page.getByRole('button', { name: '发送', exact: true }).click();
    await expect(page.locator('.turn').first().locator('.u-msg')).toHaveText(question);
    await expect(page.locator('.turn').first().locator('.answer-markdown')).toContainText('MU8 原问题的回答');
    await expect(page.locator('.stop-agent')).toHaveCount(0);
  }
  await expect(page.locator('.turn .u-msg')).toHaveText(['最新一轮问题', '第一轮问题']);
  if (width === 390 && !await input.isVisible()) await page.getByRole('button', { name: '问答', exact: true }).click();
  await input.fill('恢复后继续编辑的草稿');
  await page.waitForLoadState('networkidle');
  const workspaceId = await page.evaluate(() => JSON.parse(sessionStorage.getItem('understand-book:workspace:A')!).workspace_id);
  const identity = await (await page.request.get('https://localhost:4189/api/auth/me')).json();
  const w = await (await page.request.get(`https://localhost:4189/api/workspaces/${workspaceId}`)).json();
  const detach = await page.request.post(`https://localhost:4189/api/workspaces/${workspaceId}/detach`, {
    headers: { Origin: 'https://localhost:4189', 'X-CSRF-Token': identity.csrf_token },
    data: { attachment_id: attachment, generation: w.generation, expected_revision: w.revision },
  });
  expect(detach.status()).toBe(200);
  if (width === 390) {
    await page.route('**/api/workspaces/*/chat/history', route => route.fulfill({ status: 409,
      json: { error_code: 'WORKSPACE_STALE', category: 'conflict' } }), { times: 1 });
  }
  const reattached = page.waitForResponse(r => r.url().endsWith('/attach') && r.status() === 200);
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await reattached;
  if (width === 390) {
    await page.getByRole('button', { name: '问答', exact: true }).click();
    await expect(page.locator('.transcript [role="alert"]')).toContainText('阅读连接已失效');
    await page.locator('.transcript [role="alert"]').getByRole('button', { name: '重试', exact: true }).click();
  }
  await expect(page.locator('.turn .u-msg')).toHaveText(['最新一轮问题', '第一轮问题']);
  await expect(input).toHaveValue('恢复后继续编辑的草稿');
  await expect(page.locator('.transcript [role="alert"]')).toHaveCount(0);
  expect(await page.evaluate(() => JSON.parse(sessionStorage.getItem('understand-book:workspace:A')!).workspace_id)).toBe(workspaceId);
  expect(seen.posts).toHaveLength(2);
  expect(seen.failures.filter(f => !(width === 390 && f.includes('409') && f.endsWith('/chat/history')))).toEqual([]);
});

test('MU12 desktop toolbar collapses and outline progress stays inside short viewports', async ({ page, context }) => {
  await host(context);
  await page.setViewportSize({ width: 1280, height: 560 });
  await login(page); await open(page);
  await expect(page.locator('.network-notices')).toHaveCount(0);
  const progress = page.locator('.rail-position');
  await expect(progress).toBeVisible();
  const checkBounds = async () => {
    const box = await progress.boundingBox();
    expect(box!.y + box!.height).toBeLessThanOrEqual(page.viewportSize()!.height);
    expect(await page.locator('.left-rail').evaluate(el => el.scrollHeight - el.clientHeight)).toBeLessThanOrEqual(1);
  };
  await checkBounds();
  const before = await page.locator('.workspace-shell').boundingBox();
  await page.getByRole('button', { name: '收起工具栏', exact: true }).click();
  await expect(page.locator('.topbar')).toBeHidden();
  const after = await page.locator('.workspace-shell').boundingBox();
  expect(after!.height).toBeGreaterThan(before!.height);
  await checkBounds();
  await page.getByRole('button', { name: '展开工具栏', exact: true }).click();
  await expect(page.locator('.topbar')).toBeVisible();
  await page.locator('.topbar-focus').click();
  await expect(page.locator('.topbar')).toBeHidden();
  await page.getByRole('button', { name: '展开工具栏', exact: true }).click();
  await page.locator('.topbar-focus').click();
  await page.setViewportSize({ width: 1100, height: 440 });
  await checkBounds();
  await page.reload();
  await expect(page.locator('.workspace-shell')).toBeVisible();
  await checkBounds();
  await logout(page);
});
test('RE typography persists per account and does not recreate either reading workspace', async ({ page, context }) => {
  await host(context);
  await login(page); await open(page);
  const other = await context.newPage(); await other.goto('https://localhost:4189/'); await open(other, 'mu5-y');
  const workspaceKey = (p: Page) => p.evaluate(() => Object.entries(sessionStorage).filter(([key]) => key.startsWith('understand-book:workspace:')));
  const first = await workspaceKey(page), second = await workspaceKey(other);
  const creates: string[] = [];
  page.on('request', request => { if (request.method() === 'POST') creates.push(new URL(request.url()).pathname); });
  await page.getByRole('button', { name: '阅读设置', exact: true }).click();
  await page.getByLabel('字体', { exact: true }).selectOption('sans');
  await page.getByLabel('字号', { exact: true }).fill('22');
  await page.getByRole('button', { name: '关闭阅读设置' }).click();
  await expect(page.locator('.prose')).toHaveCSS('font-size', '22px');
  expect(await workspaceKey(page)).toEqual(first); expect(await workspaceKey(other)).toEqual(second);
  expect(creates.filter(path => path === '/api/workspaces' || path.endsWith('/attach') || path.endsWith('/fork') || path.endsWith('/layout/apply'))).toEqual([]);
  await page.reload(); await expect(page.locator('.prose')).toHaveCSS('font-size', '22px');
  await page.screenshot({ path: '../../docs/performance/reader-re1-re2/network-settings.png' });
  await other.close(); await logout(page);
  await login(page, 'B'); await open(page);
  await expect(page.locator('.prose')).toHaveCSS('font-size', '19px');
  await logout(page);
  await login(page, 'A'); await open(page);
  await expect(page.locator('.prose')).toHaveCSS('font-size', '22px');
  await logout(page);
});

test('RE5 focus preserves the live presentation and both workspaces, then clears on account switch', async ({ page, context }) => {
  const seen = await host(context);
  await login(page); await open(page);
  const other = await context.newPage(); await other.goto('https://localhost:4189/'); await open(other, 'mu5-y');
  await page.getByTitle('打开对话历史').click();
  await page.locator('.history-card').filter({ hasText: 'MU8 演示' }).getByRole('button', { name: '打开对话', exact: true }).click();
  const frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.getByText('MU8 interactive scene', { exact: true })).toBeVisible();
  await frame.locator('body').evaluate(el => { el.dataset.focusInstance = 'same-document'; });
  await page.locator('textarea[data-workspace-input="agent"]').fill('RE5 unsent question');
  const workspaceKey = (p: Page) => p.evaluate(() => sessionStorage.getItem('understand-book:workspace:A'));
  const first = await workspaceKey(page), second = await workspaceKey(other);
  const mutations: string[] = [];
  page.on('request', request => { if (request.method() === 'POST') mutations.push(new URL(request.url()).pathname); });
  await page.getByRole('button', { name: '专注阅读', exact: true }).click();
  await expect(page.locator('.right-rail')).toBeHidden();
  await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
  await expect(frame.locator('body')).toHaveAttribute('data-focus-instance', 'same-document');
  await expect(page.locator('textarea[data-workspace-input="agent"]')).toHaveValue('RE5 unsent question');
  await page.getByRole('button', { name: '展开工具栏', exact: true }).click();
  await page.getByRole('button', { name: '退出专注', exact: true }).click();
  await expect(frame.locator('body')).toHaveAttribute('data-focus-instance', 'same-document');
  expect(await workspaceKey(page)).toBe(first); expect(await workspaceKey(other)).toBe(second);
  expect(mutations.filter(path => /layout|attach|fork/.test(path) || path === '/api/workspaces')).toEqual([]);
  expect(seen.posts).toHaveLength(0);
  await page.getByRole('button', { name: '专注阅读', exact: true }).click();
  await other.close(); await logout(page);
  await login(page, 'B'); await open(page);
  await expect(page.locator('.workspace-shell')).toHaveAttribute('data-focus-reading', 'false');
  await expect(page.locator('.agent-presentation iframe')).toHaveCount(0);
  await expect(page.locator('textarea[data-workspace-input="agent"]')).toHaveValue('');
  await logout(page);
});
test('full Reader account switch clears draft and original question survives a lost 202 without another POST', async ({ page, context }) => {
  const seen = await host(context, true);
  const errors: string[] = []; page.on('pageerror', e => errors.push(e.message));
  await login(page); await open(page);
  await page.locator('textarea[data-workspace-input="agent"]').fill('MU8 accepted once');
  await page.getByRole('button', { name: '发送', exact: true }).click();
  await expect(page.getByText('MU8 原问题的回答', { exact: true })).toBeVisible();
  await page.evaluate(() => { window.dispatchEvent(new Event('online')); window.dispatchEvent(new Event('pageshow')); });
  await expect.poll(() => seen.posts.length).toBe(1);
  await page.locator('textarea[data-workspace-input="agent"]').fill('A private unsent draft');
  await logout(page);
  await expect(page.locator('.workspace-shell')).toHaveCount(0);
  await page.getByLabel('账号', { exact: true }).fill('B');
  await page.getByLabel('密码', { exact: true }).fill('fixture-only-password');
  await page.getByRole('button', { name: '登录', exact: true }).click(); await open(page);
  await expect(page.locator('textarea[data-workspace-input="agent"]')).toHaveValue('');
  await expect(page.getByText('MU8 accepted once', { exact: true })).toHaveCount(0);
  expect(errors).toEqual([]); expect(seen.failures).toEqual([]);
  await logout(page);
});
test('independent tabs, refresh and mobile layout changes retain scene ownership', async ({ page, context }) => {
  const seen = await host(context);
  await login(page); await open(page);
  const first = await page.evaluate(() => JSON.parse(sessionStorage.getItem('understand-book:workspace:A')!).workspace_id);
  const other = await context.newPage(); await other.goto('https://localhost:4189/'); await open(other, 'mu5-y');
  const second = await other.evaluate(() => JSON.parse(sessionStorage.getItem('understand-book:workspace:A')!).workspace_id);
  expect(second).not.toBe(first);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole('button', { name: '问答', exact: true }).click();
  const input = page.locator('textarea[data-workspace-input="agent"]');
  await input.fill('输入法草稿');
  await input.dispatchEvent('compositionstart');
  await page.setViewportSize({ width: 844, height: 390 });
  await input.dispatchEvent('compositionend');
  await expect(input).toHaveValue('输入法草稿');
  expect(await page.evaluate(() => JSON.parse(sessionStorage.getItem('understand-book:workspace:A')!).workspace_id)).toBe(first);
  await page.reload(); await expect(page.locator('.workspace-shell')).toBeVisible();
  expect(await other.evaluate(() => JSON.parse(sessionStorage.getItem('understand-book:workspace:A')!).workspace_id)).toBe(second);
  expect(seen.posts).toHaveLength(0); expect(seen.failures).toEqual([]);
  await other.goto('about:blank');
  await logout(page);
});

test('attached presentation uses the parent chat and a new document channel', async ({ page, context }) => {
  const seen = await host(context);
  await login(page); await open(page);
  await page.getByTitle('打开对话历史').click();
  const row = page.locator('.history-card').filter({ hasText: 'MU8 演示' });
  await row.getByRole('button', { name: '打开对话', exact: true }).click();
  await expect(page.getByRole('button', { name: '在附属窗口打开' })).toBeVisible();
  const childEvent = page.waitForEvent('popup');
  await page.getByRole('button', { name: '在附属窗口打开' }).click();
  const child = await childEvent;
  await expect(child.locator('.agent-presentation iframe')).toBeVisible();
  await expect(child.frameLocator('iframe').getByText('MU8 interactive scene', { exact: true })).toBeVisible();
  await child.getByText('文字说明与来源', { exact: true }).click();
  await child.locator('.presentation-readable button').first().click();
  await expect(child.getByLabel('演示来源')).toContainText('XXXXXXXXXX');
  const readerUpdated = page.waitForResponse(response => response.url().endsWith('/reader/state'));
  await child.getByRole('button', { name: '在原阅读区打开', exact: true }).click();
  expect((await (await readerUpdated).json()).reader.selection).toBe('1.1');
  await child.getByRole('button', { name: '关闭来源', exact: true }).click();
  await child.getByRole('button', { name: '解释现在的结果', exact: true }).click();
  await expect(child.getByText('问题已提交到原对话，可回原窗口查看回答')).toBeVisible();
  expect(seen.posts).toHaveLength(1);
  expect((seen.posts[0] as any).presentation_follow_up.session_id).toBe((seen.posts[0] as any).session_id);
  await page.locator('.new-chat').click();
  await expect(child.locator('.agent-presentation iframe')).toHaveCount(0);
  await child.goto('about:blank');
  await logout(page);
});

test('a copied tab forks the live workspace and a late PDF cannot appear for B', async ({ page, context }) => {
  const seen = await host(context);
  try {
    await login(page); await open(page, 'sample-book');
    await expect(page.locator('.pdf-page-canvas')).toBeVisible();
    await expect(page.locator('.pdf-text-layer span').first()).toBeVisible();
    await expect(page.locator('.pdf-page-error')).toHaveCount(0);
    await expect.poll(() => page.locator('.pdf-page-canvas').evaluate((canvas: HTMLCanvasElement) => canvas.width)).toBeGreaterThan(0);
    expect(seen.pdfRequests()).toBeGreaterThan(0);
    const before = await page.evaluate(() => sessionStorage.getItem('understand-book:workspace:A')!);
    const copyEvent = page.waitForEvent('popup');
    await page.evaluate(() => window.open('/', '_blank'));
    const copy = await copyEvent;
    await expect(copy.locator('.workspace-shell')).toBeVisible();
    const after = await copy.evaluate(() => sessionStorage.getItem('understand-book:workspace:A')!);
    expect(JSON.parse(after).workspace_id).not.toBe(JSON.parse(before).workspace_id);
    await copy.goto('about:blank');
    const count = seen.pdfRequests(); seen.holdPdf();
    await page.reload(); await expect.poll(seen.pdfRequests).toBeGreaterThan(count);
    await logout(page);
    await page.getByLabel('账号', { exact: true }).fill('B');
    await page.getByLabel('密码', { exact: true }).fill('fixture-only-password');
    await page.getByRole('button', { name: '登录', exact: true }).click();
    await expect(page.getByRole('heading', { name: '选择阅读材料' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'sample-book', exact: true })).toHaveCount(0);
    seen.releasePdf(); await open(page);
    await expect(page.locator('.pdf-page-canvas')).toHaveCount(0);
    expect(seen.posts).toHaveLength(0);
    expect(seen.failures).toEqual([]);
    await logout(page);
  } finally { seen.releasePdf(); }
});

test.afterEach(async ({ context }) => { await context.unrouteAll({ behavior: 'ignoreErrors' }); });



test('RE annotations keep saved ownership and discard a late save after switching users', async ({ page, context }) => {
  const seen = await host(context);
  await login(page); await open(page);
  await page.locator('.prose [data-lid="1.1"]').click();
  await expect(page.getByRole('toolbar', { name: '段落操作', exact: true })).toHaveCount(0);
  await page.locator('.prose [data-lid="1.1"]').evaluate(el => {
    const range = document.createRange(); range.selectNodeContents(el);
    getSelection()!.removeAllRanges(); getSelection()!.addRange(range);
  });
  await page.locator('.reader-pane').dispatchEvent('pointerup');
  await page.locator('.hl-popover').getByRole('button', { name: '笔记', exact: true }).click();
  await page.locator('.note-modal textarea').fill('RE A private annotation');
  await page.locator('.note-modal').getByRole('button', { name: '保存', exact: true }).click();
  await expect(page.locator('.note-modal')).toHaveCount(0);
  const marker = page.locator('[data-annotation-lid="1.1"]');
  await expect(marker).toBeVisible(); await marker.click();
  await expect(page.getByRole('dialog', { name: '正文批注', exact: true })).toContainText('RE A private annotation');
  await page.getByRole('dialog', { name: '正文批注', exact: true }).getByRole('button', { name: '编辑', exact: true }).click();
  await page.locator('.note-modal textarea').fill('RE A saved before logout');
  let release!: () => void;
  const held = new Promise<void>(resolve => { release = resolve; });
  let saved = false;
  await page.route('**/memory/replace', async route => {
    const response = await route.fetch(); saved = response.ok();
    await held; await route.fulfill({ response });
  });
  await page.locator('.note-modal').getByRole('button', { name: '保存', exact: true }).click();
  await expect.poll(() => saved).toBe(true);
  await page.locator('.note-modal .nd-close').click();
  await logout(page);
  await login(page, 'B'); await open(page);
  release();
  await expect(page.locator('.note-modal, .annotation-preview, .annotation-marker')).toHaveCount(0);
  await expect(page.locator('.app')).not.toContainText('RE A saved before logout');
  await logout(page);
  await login(page, 'A'); await open(page);
  await page.locator('[data-annotation-lid="1.1"]').click();
  await expect(page.getByRole('dialog', { name: '正文批注', exact: true })).toContainText('RE A saved before logout');
  await page.screenshot({ path: '../../docs/performance/reader-re3-re4/network-owner.png' });
  expect(seen.failures).toEqual([]);
  await logout(page);
});
