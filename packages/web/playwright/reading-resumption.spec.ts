import { expect, test } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const output = fileURLToPath(new URL('../../../docs/performance/yeye-brand-yy3-20261013/', import.meta.url));
const cases = [320, 390, 768, 901, 1024, 1440].flatMap(width => [false, true].map(longText => ({ width, longText })));
for (const { width, longText } of cases) test(`reading resumption preserves the saved scene at ${width}px${longText ? ' with long content' : ''}`, async ({ page }) => {
  await page.setViewportSize({ width, height: 900 });
  const calls: string[] = [], failures: string[] = [];
  page.on('pageerror', error => failures.push(error.message));
  const reference = { book_id: '理解复杂系统：从概念到实践', publication_id: 'original' };
  const title = longText ? '理解复杂系统：从概念到实践——系统边界、反馈回路与时间延迟如何共同影响我们的判断，以及怎样在真实案例中重新检验这些判断' : reference.book_id;
  const question = '正反馈一定会让系统失稳吗？如果同时存在负反馈，应该怎样判断最后的变化？'
    + (longText ? '如果观测时间太短、反馈存在延迟，而且系统的边界还会随着外部条件变化，我们应当先记录哪些现象？能否用一个具体例子，分别说明短期变化和长期趋势，再回到原文核对我们采用的前提是否成立？' : '');
  await page.addInitScript(() => sessionStorage.setItem('understand-book:workspace:reader', JSON.stringify({ workspace_id: 'this-window' })));
  await page.route('**/api/**', async route => {
    const request = route.request(), path = new URL(request.url()).pathname;
    calls.push(`${request.method()} ${path}`);
    if (path === '/api/auth/me') return route.fulfill({ json: { user_id: 'reader', csrf_token: 'csrf', email: 'reader@example.com' } });
    if (path === '/api/library') return route.fulfill({ json: { books: [
      { published_book_ref: { ...reference, publication_id: 'new' }, title, is_default: true },
      { published_book_ref: reference, title },
      { published_book_ref: { book_id: '学习的艺术', publication_id: 'other-window' } },
    ] } });
    if (path.endsWith('/resumption')) return route.fulfill({ json: { workspace_id: 'this-window', published_book_ref: reference,
      selected_chat: 'original-chat', position_label: '段落 · 反馈与系统边界',
      position_excerpt: '理解一个系统，先分清系统的边界，再观察其中的反馈。相同的行动，在不同条件下可能产生不同的结果。',
      last_question: question } });
    if (path === '/api/workspaces/this-window') return route.fulfill({ json: { workspace_id: 'this-window', generation: 2, revision: 8,
      published_book_ref: reference, selected_chat: 'original-chat', reader: {} } });
    if (path.endsWith('/attach')) {
      expect(request.postDataJSON()).toMatchObject({ generation: 2, expected_revision: 8 });
      return route.fulfill({ status: 409, json: { message: '现场已更新，请重试', error_code: 'WORKSPACE_REVISION_CONFLICT' } });
    }
    return route.fulfill({ status: 404, json: {} });
  });
  await page.goto('/');
  const card = page.getByRole('region', { name: '阅读接续' });
  await expect(card).toContainText('版本 2');
  await expect(card).toContainText('反馈与系统边界');
  await expect(card.getByRole('button', { name: '继续阅读' })).toBeVisible();
  await expect(page.locator('.network-book-card')).toHaveCount(3);
  expect(calls.every(call => call.startsWith('GET '))).toBe(true);
  await page.reload();
  await expect(card).toContainText('正反馈一定会让系统失稳吗？');
  expect(calls.every(call => call.startsWith('GET '))).toBe(true);
  await expect(card.locator('h2')).toHaveText(`${title} · 版本 2`);
  await expect(card.locator('.resumption-question p')).toHaveText(question);
  const mascot = card.locator('.resumption-mascot');
  await mascot.locator('img').evaluate((image: HTMLImageElement) => image.decode());
  await expect(mascot).toHaveAttribute('aria-hidden', 'true');
  if (width > 900) {
    await expect(mascot).toBeVisible();
    const picture = await mascot.boundingBox(), content = await card.locator('.resumption-content').boundingBox();
    expect(picture!.x).toBeGreaterThanOrEqual(content!.x + content!.width);
    expect(picture!.width).toBe(144);
  } else await expect(mascot).toBeHidden();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  expect(await card.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
  expect(await card.locator('h2, .resumption-question p').evaluateAll(elements =>
    elements.every(el => el.scrollWidth <= el.clientWidth && el.scrollHeight <= el.clientHeight))).toBe(true);
  const resume = card.getByRole('button', { name: '继续阅读' });
  await resume.scrollIntoViewIfNeeded();
  await expect(resume).toBeInViewport();
  await resume.focus();
  await page.keyboard.press('Tab');
  await expect(page.locator('.network-book-card').first()).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(resume).toBeFocused();
  mkdirSync(output, { recursive: true });
  if ([320, 390, 1440].includes(width)) {
    await page.evaluate(() => document.fonts.ready);
    await card.screenshot({ path: `${output}/card-${width}${longText ? '-long' : ''}.png` });
    if (!longText) {
      await page.locator('.network-library').evaluate(el => { el.scrollTop = 0; });
      await page.screenshot({ path: `${output}/bookshelf-${width}.png`, fullPage: true });
    }
  }
  await page.keyboard.press('Enter');
  await expect(page.getByRole('alert')).toContainText('现场已更新，请重试');
  await expect(card.getByRole('button', { name: '继续阅读' })).toBeEnabled();
  expect(calls.filter(call => call.startsWith('POST '))).toEqual(['POST /api/workspaces/this-window/attach']);
  expect(failures).toEqual([]);
});
