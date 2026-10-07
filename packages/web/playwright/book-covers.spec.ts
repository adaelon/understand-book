import { test, expect } from '@playwright/test';
import { readFileSync, mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

const output = resolve('../../docs/performance/book-covers');
const pdf = readFileSync(resolve('../core/test/fixtures/hybrid-foundation-goldset/v1/licensed-inline-formula/paper.pdf'));
const cover = `<svg xmlns="http://www.w3.org/2000/svg" width="400" height="600" viewBox="0 0 400 600"><rect width="400" height="600" fill="#294c49"/><path d="M0 390 Q120 210 230 350 T450 320V600H0Z" fill="#5b8271"/><circle cx="295" cy="190" r="72" fill="#e7c38b"/><path d="M35 55H120" stroke="#e7c38b" stroke-width="5"/><text x="35" y="116" fill="#fff4e2" font-size="36" font-family="Georgia">THE ART OF</text><text x="35" y="160" fill="#fff4e2" font-size="36" font-family="Georgia">LEARNING</text><text x="35" y="555" fill="#fff4e2" font-size="17" font-family="sans-serif">A READING COMPANION</text></svg>`;

for (const ranged of [false, true]) {
test(`automatic covers, fallback, responsive bookshelf and existing open action (${ranged ? 'ranges' : 'complete'})`, async ({ page }) => {
  mkdirSync(output, { recursive: true });
  const requests: unknown[] = [];
  const failures: string[] = [];
  page.on('pageerror', error => failures.push(error.message));
  const books = [
    { book_id: '学习的艺术', cover: { kind: 'image', url: '/api/cover.svg' } },
    { book_id: 'Inline Formula — 论文阅读', cover: { kind: 'pdf', url: '/api/paper.pdf' } },
    { book_id: '理解复杂系统：从概念到实践', cover: null },
    { book_id: '图片缺失的书籍', cover: { kind: 'image', url: '/api/missing.png' } },
    { book_id: '无法解析的 PDF', cover: { kind: 'pdf', url: '/api/broken.pdf' } },
  ].map((book, i) => ({ published_book_ref: { book_id: book.book_id, publication_id: `p${i}` }, cover: book.cover }));
  await page.route('**/api/**', async route => {
    const request = route.request(), path = new URL(request.url()).pathname;
    if (path === '/api/auth/me') return route.fulfill({ json: { user_id: 'reader', csrf_token: 'test' } });
    if (path === '/api/library') return route.fulfill({ json: { books } });
    if (path === '/api/cover.svg') return route.fulfill({ contentType: 'image/svg+xml', body: cover });
    if (path === '/api/paper.pdf') {
      if (ranged) {
        const requested = /bytes=(\d+)-(\d+)/.exec(request.headers().range ?? '');
        const start = Number(requested?.[1] ?? 0), end = Math.min(Number(requested?.[2] ?? pdf.length - 1), pdf.length - 1);
        return route.fulfill({ status: 206, contentType: 'application/pdf', headers: { 'Content-Range': `bytes ${start}-${end}/${pdf.length}` }, body: pdf.subarray(start, end + 1) });
      }
      return route.fulfill({ contentType: 'application/pdf', body: pdf });
    }
    if (path === '/api/broken.pdf') return route.fulfill({ contentType: 'application/pdf', body: 'broken' });
    if (path === '/api/workspaces' && request.method() === 'POST') {
      requests.push(request.postDataJSON());
      return route.fulfill({ status: 409, json: { message: '测试已记录选书操作' } });
    }
    return route.fulfill({ status: 404, body: '' });
  });
  await page.goto('/');
  const cards = page.locator('.network-book-card');
  await expect(cards).toHaveCount(5);
  await expect(cards.nth(0).locator('.book-cover')).toHaveClass(/book-cover-loaded/);
  await expect(cards.nth(1).locator('canvas')).toBeVisible({ timeout: 30000 });
  expect(await cards.nth(1).locator('canvas').evaluate((canvas: HTMLCanvasElement) => {
    const pixels = canvas.getContext('2d')!.getImageData(0, 0, canvas.width, canvas.height).data;
    return { width: canvas.width, ink: pixels.some((value, index) => index % 4 !== 3 && value < 180) };
  })).toMatchObject({ ink: true });
  await expect(cards.nth(3).locator('img')).toHaveCount(0);
  await expect(cards.nth(4).locator('canvas')).toBeHidden();
  await expect(cards.nth(2).locator('.book-cover-name')).toHaveText('理解复杂系统：从概念到实践');
  const coverTops = await cards.locator('.book-cover').evaluateAll(elements => elements.map(el => el.getBoundingClientRect().top));
  expect(Math.max(...coverTops) - Math.min(...coverTops)).toBeLessThan(2);
  await page.screenshot({ path: resolve(output, 'desktop.png'), fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  const first = await cards.nth(0).boundingBox(), second = await cards.nth(1).boundingBox();
  expect(Math.abs(first!.y - second!.y)).toBeLessThan(2);
  await page.screenshot({ path: resolve(output, 'mobile.png'), fullPage: true });
  await page.getByRole('button', { name: '学习的艺术', exact: true }).click();
  await expect.poll(() => requests.length).toBe(1);
  expect(requests[0]).toMatchObject({ published_book_ref: books[0].published_book_ref });
  expect(failures).toEqual([]);
});
}
