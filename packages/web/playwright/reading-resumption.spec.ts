import { expect, test } from '@playwright/test';
import { mkdirSync } from 'node:fs';

for (const width of [320, 390, 768, 1440]) test(`RS5 reads the saved scene without navigation at ${width}px`, async ({ page }) => {
  await page.setViewportSize({ width, height: 900 });
  const calls: string[] = [], failures: string[] = [];
  page.on('pageerror', error => failures.push(error.message));
  const reference = { book_id: '理解复杂系统：从概念到实践', publication_id: 'original' };
  await page.addInitScript(() => sessionStorage.setItem('understand-book:workspace:reader', JSON.stringify({ workspace_id: 'this-window' })));
  await page.route('**/api/**', async route => {
    const request = route.request(), path = new URL(request.url()).pathname;
    calls.push(`${request.method()} ${path}`);
    if (path === '/api/auth/me') return route.fulfill({ json: { user_id: 'reader', csrf_token: 'csrf', email: 'reader@example.com' } });
    if (path === '/api/library') return route.fulfill({ json: { books: [
      { published_book_ref: { ...reference, publication_id: 'new' }, is_default: true },
      { published_book_ref: reference },
      { published_book_ref: { book_id: '学习的艺术', publication_id: 'other-window' } },
    ] } });
    if (path.endsWith('/resumption')) return route.fulfill({ json: { workspace_id: 'this-window', published_book_ref: reference,
      selected_chat: 'original-chat', position_label: '段落 · 反馈与系统边界',
      position_excerpt: '理解一个系统，先分清系统的边界，再观察其中的反馈。相同的行动，在不同条件下可能产生不同的结果。',
      last_question: '正反馈一定会让系统失稳吗？如果同时存在负反馈，应该怎样判断最后的变化？' } });
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
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  expect(await card.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
  mkdirSync('../../docs/performance/reading-share-rs5-20261010', { recursive: true });
  await page.screenshot({ path: `../../docs/performance/reading-share-rs5-20261010/bookshelf-${width}.png`, fullPage: true });
  await card.getByRole('button', { name: '继续阅读' }).click();
  await expect(page.getByRole('alert')).toContainText('现场已更新，请重试');
  await expect(card.getByRole('button', { name: '继续阅读' })).toBeEnabled();
  expect(calls.filter(call => call.startsWith('POST '))).toEqual(['POST /api/workspaces/this-window/attach']);
  expect(failures).toEqual([]);
});
