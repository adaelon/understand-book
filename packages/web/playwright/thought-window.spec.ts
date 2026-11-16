import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

const output = resolve('../../docs/performance/personal-notes-20261030/floating');
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    for (const owner of ['dock-reader', 'note-preview'])
      localStorage.setItem(`understand-book.product-tour.v1.${owner}.library`, 'done');
  });
  const notes = new Map<string, any>();
  await page.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname;
    const body = route.request().postDataJSON() ?? {};
    let data: unknown = {};
    if (path.endsWith('/auth/me')) data = { user_id: 'dock-reader', email: 'reader@example.test', csrf_token: 'test' };
    else if (path.endsWith('/library')) data = { books: [
      { published_book_ref: { book_id: 'learning', publication_id: 'v1' }, title: '学习与理解' },
      { published_book_ref: { book_id: 'systems', publication_id: 'v1' }, title: '系统设计中的因果关系' },
    ] };
    else if (path.endsWith('/videos')) data = { videos: [] };
    else if (path.endsWith('/learning-workspaces')) data = { workspaces: [] };
    else if (path.endsWith('/reading-resumption')) data = null;
    else if (path.includes('/workspace-releases')) data = { items: [], total: 0, limit: 12, offset: 0 };
    else if (path.endsWith('/me/notes/list')) data = { notes: [...notes.values()], next_cursor: null };
    else if (path.endsWith('/me/notes/create')) {
      const note = { note_id: `saved-${notes.size + 1}`, current_revision: 1, value: {
        ...body.value, revision: 1, saved_at: new Date().toISOString(),
        record: { content: body.value.content, note: body.value.source, anchor: { lid: '' } },
      } };
      notes.set(note.note_id, note); data = note;
    } else if (path.endsWith('/me/notes/read')) data = notes.get(body.note_id);
    else if (path.endsWith('/me/notes/history')) data = { revisions: [notes.get(body.note_id)?.value], discussions: [] };
    else if (path.endsWith('/me/notes/links')) data = { links: [] };
    else if (path.endsWith('/me/notes/attachments') || path.endsWith('/me/notes/board_locations')) data = { items: [] };
    else if (path.endsWith('/me/notes/chats')) data = { sessions: [] };
    await route.fulfill({ json: data });
  });
  await page.goto('/');
  await expect(page.getByRole('button', { name: '记一下', exact: true })).toBeVisible();
  await expect(page.locator('.network-library')).toBeVisible();
});

async function assertFloatingWindow(page: Page) {
  const content = (await page.locator('.application-content').boundingBox())!;
  const frame = (await page.getByRole('region', { name: '我的想法', exact: true }).boundingBox())!;
  const viewport = await page.evaluate(() => ({ width: innerWidth, height: visualViewport!.height, top: visualViewport!.offsetTop }));
  expect(content.width).toBe(viewport.width);
  expect(frame.x).toBeGreaterThanOrEqual(12);
  expect(frame.y).toBeGreaterThanOrEqual(viewport.top + 12);
  expect(frame.x + frame.width).toBeLessThanOrEqual(viewport.width - 12);
  expect(frame.y + frame.height).toBeLessThanOrEqual(viewport.top + viewport.height - 12);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  expect(await page.locator('.application-content').evaluate(el => getComputedStyle(el).filter)).toBe('none');
}

test('a global thought records its initial location and can be made independent', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.evaluate(async () => { const path = '/playwright/fixtures/thought-window.ts'; (await import(path)).setReadingContext('学习与理解 · 从熟悉到理解'); });
  await page.getByRole('button', { name: '记一下', exact: true }).click();
  const source = page.getByRole('button', { name: '选择想法来源', exact: true });
  const input = page.getByRole('textbox', { name: '想法正文', exact: true });
  await expect(source).toHaveText('学习与理解 · 从熟悉到理解');
  await input.fill('熟悉一段表述，并不意味着我已经能解释它。');
  await input.press('Escape');
  await page.evaluate(async () => { const path = '/playwright/fixtures/thought-window.ts'; (await import(path)).setReadingContext('另一份材料', 'systems', '3.2'); });
  await page.getByRole('button', { name: '记一下', exact: true }).click();
  await expect(source).toHaveText('学习与理解 · 从熟悉到理解');
  await source.click();
  await expect(page.getByRole('button', { name: /关联当前位置/ })).toContainText('另一份材料');
  mkdirSync(output, { recursive: true });
  await page.screenshot({ path: resolve(output, 'context-choice-1440.png') });
  await page.getByRole('region', { name: '我的想法', exact: true }).screenshot({ path: resolve(output, 'context-choice-window.png') });
  await page.getByRole('button', { name: /独立想法.*不关联页面位置/ }).click();
  await expect(source).toHaveText('独立想法');
  await expect(input).toHaveValue(/熟悉一段表述/);
  const request = page.waitForRequest('**/api/me/notes/create');
  await page.getByRole('button', { name: '保存想法', exact: true }).click();
  expect((await request).postDataJSON().value.context.book_id).toBeNull();
});

test('dragging and resizing preserve the note and remember the frame after reopening and reload', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  const background = await page.locator('.application-content').boundingBox();
  await page.getByRole('button', { name: '记一下', exact: true }).click();
  const note = page.getByRole('region', { name: '我的想法', exact: true });
  const input = note.getByRole('textbox', { name: '想法正文', exact: true });
  await input.fill('读得熟悉，不等于能解释。\n试试合上书，举一个自己的例子。');
  const first = (await note.boundingBox())!;
  await page.mouse.move(first.x + 90, first.y + 26); await page.mouse.down();
  await page.mouse.move(first.x - 350, first.y - 234, { steps: 12 }); await page.mouse.up();
  const moved = (await note.boundingBox())!;
  expect(moved).toEqual({ ...first, x: first.x - 440, y: first.y - 260 });
  const handle = (await note.getByRole('button', { name: '调整想法窗口大小' }).boundingBox())!;
  await page.mouse.move(handle.x + 10, handle.y + 10); await page.mouse.down();
  await page.mouse.move(handle.x + 150, handle.y + 130, { steps: 12 }); await page.mouse.up();
  const resized = (await note.boundingBox())!;
  expect(resized).toEqual({ ...moved, width: moved.width + 140, height: moved.height + 120 });
  expect(await page.locator('.application-content').boundingBox()).toEqual(background);
  await expect(input).toHaveValue(/合上书/);
  await input.press('Escape');
  await expect(page.getByRole('button', { name: '记一下', exact: true })).toBeFocused();
  await page.getByRole('button', { name: '记一下', exact: true }).click();
  expect(await note.boundingBox()).toEqual(resized);
  await page.reload(); await page.getByRole('button', { name: '记一下', exact: true }).click();
  expect(await note.boundingBox()).toEqual(resized);
  await expect(input).toHaveValue(/合上书/);
  const grip = note.getByRole('button', { name: '移动想法窗口', exact: true });
  await grip.focus(); await grip.press('ArrowLeft');
  await note.getByRole('button', { name: '调整想法窗口大小' }).focus();
  await page.keyboard.press('ArrowDown');
  expect(await note.boundingBox()).toEqual({ ...resized, x: resized.x - 10, height: resized.height + 10 });
  await assertFloatingWindow(page);
  mkdirSync(output, { recursive: true });
  await page.screenshot({ path: resolve(output, 'moved-and-resized-1440.png') });
});

test('touch drag and resize work, and a temporary keyboard viewport does not overwrite the preferred position', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole('button', { name: '记一下', exact: true }).click();
  const note = page.getByRole('region', { name: '我的想法', exact: true });
  const original = (await note.boundingBox())!;
  const touch = await page.context().newCDPSession(page);
  await touch.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: original.x + 90, y: original.y + 26 }] });
  await touch.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: original.x + 90, y: original.y - 154 }] });
  await touch.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await expect.poll(async () => (await note.boundingBox())!.y).toBe(original.y - 180);
  const handle = (await note.getByRole('button', { name: '调整想法窗口大小' }).boundingBox())!;
  await touch.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: handle.x + 10, y: handle.y + 10 }] });
  await touch.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: handle.x + 10, y: handle.y + 50 }] });
  await touch.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await expect.poll(async () => (await note.boundingBox())!.height).toBe(original.height + 40);
  const preferred = await note.boundingBox();
  await page.evaluate(() => {
    Object.defineProperty(window.visualViewport, 'height', { configurable: true, value: 340 });
    Object.defineProperty(window.visualViewport, 'offsetTop', { configurable: true, value: 80 });
    window.visualViewport!.dispatchEvent(new Event('resize'));
  });
  await assertFloatingWindow(page);
  await page.evaluate(() => {
    delete (window.visualViewport as any).height; delete (window.visualViewport as any).offsetTop;
    window.visualViewport!.dispatchEvent(new Event('resize'));
  });
  await expect.poll(() => note.boundingBox()).toEqual(preferred);
  await touch.detach();
});

for (const viewport of [{ width: 1440, height: 1000 }, { width: 1024, height: 768 }, { width: 390, height: 844 }, { width: 320, height: 568 }]) {
  test(`capture floats over unchanged, usable content and persists at ${viewport.width}px`, async ({ page }) => {
    await page.setViewportSize(viewport);
    const contentBefore = await page.locator('.application-content').boundingBox();
    const entry = page.getByRole('button', { name: '记一下', exact: true });
    await entry.click();
    const dock = page.getByRole('region', { name: '我的想法', exact: true });
    const input = dock.getByRole('textbox', { name: '想法正文', exact: true });
    await expect(input).toBeFocused();
    await assertFloatingWindow(page);
    expect(await page.locator('.application-content').boundingBox()).toEqual(contentBefore);
    await input.fill('固定的是参数，变化的是输入。这里的表示变化，是否就是激活的变化？');
    await page.getByRole('button', { name: '浏览共享预构建工作区', exact: true }).click();
    await expect(page.getByRole('button', { name: '收起共享工作区', exact: true })).toBeVisible();
    await expect(input).toHaveValue(/参数/);
    await input.focus(); await input.press('Escape');
    await expect(dock).toBeHidden();
    await page.reload(); await entry.click();
    await expect(input).toHaveValue(/参数/);
    await expect(page.locator('.thought-toolbar .draft-dot')).toHaveCount(1);
    mkdirSync(output, { recursive: true });
    await page.screenshot({ path: resolve(output, `capture-${viewport.width}.png`) });
    if (viewport.width === 1440) await dock.screenshot({ path: resolve(output, 'note-window.png') });
    await dock.getByRole('button', { name: '保存想法', exact: true }).click();
    await expect(dock).toBeHidden();
    await expect(page.locator('.toolbar-notice')).toContainText('已保存');
    await page.getByRole('button', { name: '记一下', exact: true }).click();
    await page.getByRole('button', { name: '我的想法', exact: true }).click();
    await dock.locator('.note-list button').filter({ hasText: '固定的是参数' }).click();
    await expect(dock.locator('.thought-prose')).toContainText('是否就是激活');
    await assertFloatingWindow(page);
  });
}

test('the original source window remains open, scrollable and keyboard-independent while writing', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.evaluate(async () => {
    const path = '/playwright/fixtures/thought-window.ts';
    (await import(path)).openReadingWindow();
  });
  const source = page.getByRole('dialog', { name: '画板原文', exact: true });
  await expect(source).toBeVisible();
  const original = await source.elementHandle();
  const sourceBefore = await source.boundingBox();
  await page.getByRole('button', { name: '记一下', exact: true }).click();
  const input = page.getByRole('textbox', { name: '想法正文', exact: true });
  await expect(input).toBeFocused(); await input.fill('读得熟悉，不等于能解释。试试合上书，举一个自己的例子。');
  await input.press('Tab');
  expect(await source.evaluate(el => el.contains(document.activeElement))).toBe(false);
  await assertFloatingWindow(page);
  expect(await source.boundingBox()).toEqual(sourceBefore);
  const scroller = source.locator('.board-source-content');
  await scroller.hover({ position: { x: 40, y: 100 } }); await page.mouse.wheel(0, 350);
  await expect.poll(() => scroller.evaluate(el => el.scrollTop)).toBeGreaterThan(100);
  const offset = await scroller.evaluate(el => el.scrollTop);
  await input.focus(); await input.press('Escape');
  await expect(source).toBeVisible();
  expect(await original!.evaluate(el => el.isConnected)).toBe(true);
  expect(await scroller.evaluate(el => el.scrollTop)).toBe(offset);
  await page.keyboard.press('Control+Shift+M'); await expect(input).toBeFocused();
  await expect(input).toHaveValue(/合上书/);
  await page.screenshot({ path: resolve(output, 'reading-and-capture-1440.png') });
  await page.setViewportSize({ width: 390, height: 844 });
  await assertFloatingWindow(page);
  await expect(source.getByRole('button', { name: '关闭来源预览，返回画板', exact: true })).toBeInViewport();
  await expect(source.getByRole('button', { name: '返回画板', exact: true })).toBeInViewport();
  await page.screenshot({ path: resolve(output, 'reading-and-capture-390.png') });
});

test('a failed save can be retried and a narrow keyboard viewport keeps capture usable', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole('button', { name: '记一下', exact: true }).click();
  const dock = page.getByRole('region', { name: '我的想法', exact: true });
  const input = dock.getByRole('textbox', { name: '想法正文', exact: true });
  await input.fill('不能把熟悉感当作理解。');
  await page.evaluate(() => {
    Object.defineProperty(window.visualViewport, 'height', { configurable: true, value: 460 });
    window.visualViewport!.dispatchEvent(new Event('resize'));
  });
  await expect(page.locator('html')).toHaveAttribute('data-workspace-keyboard', 'true');
  await assertFloatingWindow(page);
  const save = dock.getByRole('button', { name: '保存想法', exact: true });
  await save.scrollIntoViewIfNeeded();
  expect((await save.boundingBox())!.y + (await save.boundingBox())!.height).toBeLessThanOrEqual(460);
  await page.route('**/api/me/notes/create', route => route.fulfill({ status: 503, json: { message: '暂时无法连接，请重试', error_code: 'UNAVAILABLE', category: 'temporary' } }), { times: 1 });
  await save.click();
  await expect(dock.getByRole('alert')).toContainText('暂时无法连接');
  await expect(dock.locator('.draft-status')).toContainText('暂时无法连接');
  await expect(dock.locator('.draft-status')).toBeInViewport();
  await expect(input).toHaveValue('不能把熟悉感当作理解。');
  await page.screenshot({ path: resolve(output, 'keyboard-retry-390.png') });
  await dock.getByRole('button', { name: '重试这次保存', exact: true }).click();
  await expect(dock).toBeHidden();
  await expect(page.locator('.toolbar-notice')).toContainText('已保存');
});

test('opening capture preserves the visible reader and assistant in wide and small desktop layouts', async ({ page }) => {
  await page.evaluate(async () => {
    const path = '/playwright/fixtures/thought-window.ts';
    (await import(path)).openReaderWorkspace();
  });
  const workspace = page.locator('.workspace-shell');
  for (const viewport of [{ width: 1440, height: 1000 }, { width: 1024, height: 768 }]) {
    await page.setViewportSize(viewport);
    await expect(workspace).toHaveAttribute('data-mode', 'wide');
    await page.getByRole('button', { name: '记一下', exact: true }).click();
    await expect(page.getByRole('textbox', { name: '想法正文', exact: true })).toBeFocused();
    await assertFloatingWindow(page);
    await expect(workspace).toHaveAttribute('data-mode', 'wide');
    await expect(workspace.getByText('正在看的原文：模型参数在推理过程中保持固定。', { exact: true })).toBeVisible();
    await expect(workspace.getByText('正在看的解释：激活依赖当前输入。', { exact: true })).toBeVisible();
    await page.getByRole('button', { name: '收起我的想法', exact: true }).click();
  }
});

test('the thought entry leaves the send button clickable with long drafts and short viewports', async ({ page }) => {
  await page.evaluate(async () => { const path = '/playwright/fixtures/send-button.ts'; (await import(path)).openConversation(); });
  const input = page.locator('textarea[data-workspace-input="agent"]');
  const send = page.locator('.send-agent');
  const evidence = resolve('../../docs/performance/note-launcher-20261030');
  mkdirSync(evidence, { recursive: true });
  let sends = 0;
  for (const viewport of [{ width: 1440, height: 900 }, { width: 1024, height: 715 }, { width: 783, height: 715 }, { width: 390, height: 844 }, { width: 320, height: 568 }, { width: 1024, height: 440 }]) {
    await page.setViewportSize(viewport);
    await input.fill('我又加了些东西，船往下沉了一点，可是还浮着。它都变重了，为什么没有一直沉到底呀？\n'.repeat(3));
    await expect(send).toBeInViewport();
    expect((await page.locator('.application-content').boundingBox())!.height).toBe(viewport.height);
    await expect.poll(async () => {
      const launcher = (await page.locator('.thought-toolbar').boundingBox())!;
      return launcher.y + launcher.height;
    }).toBeLessThanOrEqual((await page.locator('.agent-input').boundingBox())!.y);
    // A real click (without force) detects the floating toolbar intercepting the pointer.
    await send.click({ timeout: 3000 });
    await expect(page.locator('[data-sends]')).toHaveAttribute('data-sends', String(++sends));
    const before = await page.locator('.application-content').boundingBox();
    await page.getByRole('button', { name: '记一下', exact: true }).click();
    expect(await page.locator('.application-content').boundingBox()).toEqual(before);
    await page.getByRole('textbox', { name: '想法正文', exact: true }).fill('先记录这个疑问，继续读。');
    await page.getByRole('button', { name: '收起我的想法', exact: true }).click();
    expect(await page.locator('.application-content').boundingBox()).toEqual(before);
    await send.click({ timeout: 3000 });
    await expect(page.locator('[data-sends]')).toHaveAttribute('data-sends', String(++sends));
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: resolve(evidence, `send-${viewport.width}x${viewport.height}.png`) });
  }
});

test('the recording entry avoids the running answer stop control while scrolling and resizing', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 918 });
  await page.evaluate(async () => { const path = '/playwright/fixtures/send-button.ts'; (await import(path)).openConversation(false, true); });
  const stop = page.locator('.run-progress-footer .stop-work-button');
  const entry = page.locator('.thought-entry');
  const evidence = resolve('../../docs/performance/note-launcher-avoidance-20261103');
  mkdirSync(evidence, { recursive: true });
  for (const viewport of [{ width: 1280, height: 918 }, { width: 1024, height: 440 }, { width: 390, height: 844 }]) {
    await page.setViewportSize(viewport);
    await page.locator('.transcript').evaluate(el => { el.scrollTop = el.scrollHeight; });
    await expect(stop).toBeInViewport();
    await expect(entry).toBeVisible();
    await expect.poll(async () => {
      const a = (await entry.boundingBox())!, b = (await stop.boundingBox())!;
      return a.x < b.x + b.width + 12 && a.x + a.width + 12 > b.x
        && a.y < b.y + b.height + 12 && a.y + a.height + 12 > b.y;
    }).toBe(false);
    await stop.click({ timeout: 3000 });
    await page.screenshot({ path: resolve(evidence, `running-${viewport.width}x${viewport.height}.png`) });
  }
  await expect(page.locator('[data-stops]')).toHaveAttribute('data-stops', '3');
  // Dropping a remembered position onto a control must still yield after release.
  const initial = (await entry.boundingBox())!, control = (await stop.boundingBox())!;
  await page.mouse.move(initial.x + initial.width / 2, initial.y + initial.height / 2);
  await page.mouse.down();
  await page.mouse.move(control.x + control.width / 2, control.y + control.height / 2, { steps: 10 });
  await page.mouse.up();
  await expect.poll(async () => {
    const a = (await entry.boundingBox())!, b = (await stop.boundingBox())!;
    return a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y;
  }).toBe(false);
  await expect(page.getByRole('region', { name: '我的想法', exact: true })).toBeHidden();
  await stop.click({ timeout: 3000 });
  await expect(page.locator('[data-stops]')).toHaveAttribute('data-stops', '4');
});

test('dragging the recording entry does not open it and its position survives reopening and reload', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const entry = page.getByRole('button', { name: '记一下', exact: true });
  const initial = (await entry.boundingBox())!;
  await page.mouse.move(initial.x + initial.width / 2, initial.y + initial.height / 2);
  await page.mouse.down(); await page.mouse.move(700, 360, { steps: 12 }); await page.mouse.up();
  await expect(page.getByRole('region', { name: '我的想法', exact: true })).toBeHidden();
  const moved = await entry.boundingBox();
  expect(moved!.x).toBeLessThan(initial.x - 200);
  await entry.click();
  const input = page.getByRole('textbox', { name: '想法正文', exact: true });
  await expect(input).toBeFocused();
  await input.fill('随时记下一个还没有想通的问题。');
  await input.press('Escape');
  expect(await entry.boundingBox()).toEqual(moved);
  await page.reload();
  await expect.poll(() => entry.boundingBox()).toEqual(moved);
  await entry.click(); await expect(input).toHaveValue('随时记下一个还没有想通的问题。');
  await input.press('Escape'); await entry.press('Alt+ArrowLeft');
  expect((await entry.boundingBox())!.x).toBe(moved!.x - 10);
});

test('touch dragging the recording entry moves it without opening a note', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const entry = page.getByRole('button', { name: '记一下', exact: true });
  const first = (await entry.boundingBox())!;
  const touch = await page.context().newCDPSession(page);
  await touch.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: first.x + 40, y: first.y + 22 }] });
  await touch.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: first.x - 120, y: first.y - 180 }] });
  await touch.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await expect(page.getByRole('region', { name: '我的想法', exact: true })).toBeHidden();
  expect((await entry.boundingBox())!.y).toBeLessThan(first.y - 100);
  await entry.click(); await expect(page.getByRole('textbox', { name: '想法正文', exact: true })).toBeFocused();
});

test('the recording entry stays above the phone reading navigation', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.evaluate(async () => { const path = '/playwright/fixtures/thought-window.ts'; (await import(path)).openReaderWorkspace(); });
  const navigation = page.locator('.workspace-mobile-nav');
  await expect(navigation).toBeVisible();
  await expect.poll(async () => {
    const entry = (await page.locator('.thought-toolbar').boundingBox())!;
    return entry.y + entry.height;
  }).toBeLessThanOrEqual((await navigation.boundingBox())!.y);
  await navigation.getByRole('button', { name: '菜单', exact: true }).click({ trial: true });
  await page.locator('.thought-entry').click();
  await expect(page.getByRole('textbox', { name: '想法正文', exact: true })).toBeFocused();
});

test('the mobile keyboard hides the thought entry until dismissal and keeps send clickable', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.evaluate(async () => { const path = '/playwright/fixtures/send-button.ts'; (await import(path)).openConversation(true); });
  await page.locator('.thought-entry').press('Alt+ArrowLeft');
  const preferred = await page.evaluate(() => localStorage.getItem('understand-book:note-launcher:dock-reader'));
  const input = page.locator('textarea[data-workspace-input="agent"]');
  await input.fill('为什么船变重之后还能浮着？');
  await page.evaluate(() => {
    Object.defineProperty(window.visualViewport, 'height', { configurable: true, value: 460 });
    Object.defineProperty(window.visualViewport, 'offsetTop', { configurable: true, value: 40 });
    window.visualViewport!.dispatchEvent(new Event('resize'));
  });
  await expect(page.locator('html')).toHaveAttribute('data-workspace-keyboard', 'true');
  await expect(page.locator('.thought-entry')).toBeHidden();
  const send = page.locator('.send-agent');
  await send.click({ timeout: 3000 });
  await expect(page.locator('[data-sends]')).toHaveAttribute('data-sends', '1');
  const button = (await send.boundingBox())!;
  expect(button.y + button.height).toBeLessThanOrEqual(500);
  await expect(page.locator('.thought-entry')).toBeHidden();
  await page.evaluate(() => {
    delete (visualViewport as unknown as { height?: number }).height;
    delete (visualViewport as unknown as { offsetTop?: number }).offsetTop;
    visualViewport!.dispatchEvent(new Event('resize'));
  });
  await expect(page.locator('.thought-entry')).toBeVisible();
  expect(await page.evaluate(() => localStorage.getItem('understand-book:note-launcher:dock-reader'))).toBe(preferred);
  await page.evaluate(() => {
    Object.defineProperty(visualViewport, 'height', { configurable: true, value: 422 });
    Object.defineProperty(visualViewport, 'scale', { configurable: true, value: 2 });
    visualViewport!.dispatchEvent(new Event('resize'));
  });
  await expect(page.locator('.thought-entry')).toBeVisible();
  await page.evaluate(() => {
    delete (visualViewport as unknown as { height?: number }).height;
    delete (visualViewport as unknown as { scale?: number }).scale;
    visualViewport!.dispatchEvent(new Event('resize'));
  });
  await page.locator('.thought-entry').click();
  await expect(page.getByRole('textbox', { name: '想法正文', exact: true })).toBeFocused();
});

for (const viewport of [{ width: 390, height: 844, keyboardHeight: 320 }, { width: 320, height: 568, keyboardHeight: 260 }]) {
test(`a mobile note can be saved with a short keyboard viewport and its receipt expires at ${viewport.width}px`, async ({ page }) => {
  await page.setViewportSize({ width: viewport.width, height: viewport.height });
  await page.goto('/note-launcher-preview.html');
  await page.locator('.thought-entry').click();
  await page.getByRole('textbox', { name: '想法正文', exact: true }).fill('我知道了：船下沉一些，就能排开更多水。');
  await page.evaluate(height => {
    Object.defineProperty(visualViewport, 'height', { configurable: true, value: height });
    Object.defineProperty(visualViewport, 'offsetTop', { configurable: true, value: 40 });
    visualViewport!.dispatchEvent(new Event('resize'));
  }, viewport.keyboardHeight);
  const save = page.getByRole('button', { name: '保存想法', exact: true });
  await expect.poll(async () => { const box = (await save.boundingBox())!; return box.y + box.height; }).toBeLessThanOrEqual(viewport.keyboardHeight + 40);
  await expect.poll(async () => {
    const status = (await page.locator('.draft-status-line').boundingBox())!;
    const actions = (await page.locator('.thought-actions').boundingBox())!;
    return status.y + status.height - actions.y;
  }).toBeLessThanOrEqual(0);
  const evidence = resolve('../../docs/performance/note-mobile-controls-20261101');
  mkdirSync(evidence, { recursive: true });
  await page.screenshot({ path: resolve(evidence, `mobile-note-save-${viewport.width}.png`), clip: { x: 0, y: 40, width: viewport.width, height: viewport.keyboardHeight } });
  await save.click({ timeout: 3000 });
  await expect(page.locator('.thought-entry')).toBeHidden();
  await expect(page.locator('.toolbar-notice')).toContainText('已保存');
  await expect(page.locator('.toolbar-notice')).toBeHidden({ timeout: 4500 });
  const send = page.locator('.send-agent');
  await send.click({ timeout: 3000 });
  await expect(page.locator('textarea[data-workspace-input="agent"]')).toHaveValue('');
  await page.screenshot({ path: resolve(evidence, `mobile-after-save-${viewport.width}.png`), clip: { x: 0, y: 40, width: viewport.width, height: viewport.keyboardHeight } });
  await page.evaluate(() => {
    delete (visualViewport as unknown as { height?: number }).height;
    delete (visualViewport as unknown as { offsetTop?: number }).offsetTop;
    visualViewport!.dispatchEvent(new Event('resize'));
  });
  await expect(page.locator('.thought-entry')).toBeVisible();
  await expect(page.locator('.toolbar-notice')).toBeHidden();
});
}

test('the mobile keyboard also leaves the learning goal save button clickable', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/note-launcher-preview.html');
  await page.getByRole('button', { name: '学习与会话', exact: true }).click();
  await page.getByRole('button', { name: '调整目标', exact: true }).click();
  const editor = page.locator('.conversation-learning-goal form');
  await editor.locator('input').fill('理解浮力与船的重量如何平衡');
  await page.evaluate(() => {
    Object.defineProperty(visualViewport, 'height', { configurable: true, value: 350 });
    Object.defineProperty(visualViewport, 'offsetTop', { configurable: true, value: 40 });
    visualViewport!.dispatchEvent(new Event('resize'));
  });
  await expect(page.locator('.thought-entry')).toBeHidden();
  await editor.getByRole('button', { name: '保存目标', exact: true }).click({ timeout: 3000 });
  await expect(editor).toBeHidden();
  await expect(page.locator('.goal-text')).toHaveText('理解浮力与船的重量如何平衡');
});

test('profile replaces artifacts in the main navigation and can return to the draft', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.evaluate(async () => { const path = '/playwright/fixtures/send-button.ts'; (await import(path)).openConversation(); });
  const tabs = page.locator('.context-tabs');
  await expect(tabs.getByRole('tab', { name: '成果' })).toHaveCount(0);
  const profile = tabs.getByRole('tab', { name: '画像', exact: true });
  await expect(profile).toBeVisible();
  await profile.click();
  await expect(page.locator('#reader-panel-profile')).toBeVisible();
  await expect(tabs.getByRole('button', { name: '阅读工具' })).toHaveText('工具');
  const evidence = resolve('../../docs/performance/note-launcher-20261030');
  mkdirSync(evidence, { recursive: true });
  await page.screenshot({ path: resolve(evidence, 'profile-navigation-1440.png') });
  await tabs.getByRole('button', { name: '阅读工具' }).click();
  await expect(page.locator('.secondary-tabs').getByRole('tab')).toHaveText(['公式', '笔记']);
  await tabs.getByRole('tab', { name: '问答', exact: true }).click();
  await expect(page.locator('textarea[data-workspace-input="agent"]')).toHaveValue(/为什么没有一直沉到底/);
  await page.locator('.send-agent').click();
  await expect(page.locator('[data-sends]')).toHaveAttribute('data-sends', '1');
});
