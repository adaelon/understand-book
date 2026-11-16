import { expect, test } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { setup } from './fixtures/tutor-continuation';

const output = resolve('../../docs/performance/reading-guidance-20261102');
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('understand-book.product-tour.v1.local.reader', 'done'));
});
const guides = {
  welcome: { question: '读到平均速度，你最想弄清什么？一个疑问或想法就可以。', draft: '我想先弄清平均速度与每段速度有什么关系。' },
  understand: { question: '关于总路程和总时间，哪一步让你犹豫？也可以说说你的理解。', draft: '我想弄清为什么要把停留的时间也算进去。' },
  explore: { question: '改变途中停留的时间，你想追问什么？', draft: '我想探讨：走同样的路，停留时间怎样影响平均速度？' },
};

for (const width of [1366, 390, 320]) test.describe(`${width}px`, () => {
test.use({ hasTouch: width < 1024, isMobile: width < 1024 });
test('ghost guidance is editable, optional and scoped', async ({ page }) => {
  const fixture = await setup(page, 'markdown', true);
  const guidanceRequests: Record<string, unknown>[] = [];
  await page.route('**/api/reader/guidance', async route => {
    const request = route.request().postDataJSON(); guidanceRequests.push(request);
    await route.fulfill({ json: guides[request.purpose as keyof typeof guides] ?? null });
  });
  await page.setViewportSize({ width, height: width > 1000 ? 900 : 844 });
  await page.reload();
  expect(await page.evaluate(() => innerWidth)).toBe(width);
  if (width < 1024) await page.getByRole('navigation', { name: '阅读工作区' }).getByRole('button', { name: '问答', exact: true }).click();
  const initial = page.locator('.agent-input textarea');
  await expect(initial).toHaveAttribute('placeholder', guides.welcome.draft);
  await expect(initial).toHaveValue('');
  await expect(page.locator('.send-agent')).toBeDisabled();
  mkdirSync(output, { recursive: true });
  await page.screenshot({ path: resolve(output, `welcome-${width}.png`) });
  await initial.focus(); await initial.press('Shift+Tab'); await expect(initial).not.toBeFocused();
  await initial.focus();
  if (width > 1000) await initial.press('Tab');
  else {
    await expect(page.locator('.agent-input .input-suggestion-hint kbd')).toBeHidden();
    await page.locator('.agent-input .input-suggestion-hint button').tap();
  }
  await expect(initial).toHaveValue(guides.welcome.draft); await expect(initial).toBeFocused();
  expect(fixture.requests).toHaveLength(0);
  await initial.fill('');
  const entry = page.locator('.learning-purpose-entry');
  await entry.locator('[data-purpose=understand]').click();
  const input = entry.getByRole('textbox');
  await expect(input).toHaveAttribute('placeholder', guides.understand.draft);
  await expect(input).toHaveValue('');
  await expect(entry.locator('.purpose-submit')).toBeDisabled();
  expect(fixture.mutations).toHaveLength(0);
  await page.screenshot({ path: resolve(output, `understand-${width}.png`) });
  if (width > 1000) await input.press('Tab');
  else await entry.locator('.input-suggestion-hint button').tap();
  await expect(input).toHaveValue(guides.understand.draft);
  await input.fill('我原来只算行驶时间，想知道停留为什么也要算。');
  await expect(entry.locator('.input-suggestion-hint')).toHaveCount(0);
  await entry.locator('.purpose-back').click();
  await entry.locator('[data-purpose=explore]').click();
  await expect(input).toHaveAttribute('placeholder', guides.explore.draft);
  await page.screenshot({ path: resolve(output, `explore-${width}.png`) });
  await entry.locator('.input-suggestion-hint button').click();
  await expect(input).toHaveValue(guides.explore.draft);
  await input.fill(guides.explore.draft + ' 我想到公交车等红灯的情况。');
  expect(fixture.requests).toHaveLength(0);
  expect(guidanceRequests.filter(request => request.purpose === 'understand')).toHaveLength(1);
  expect(guidanceRequests.filter(request => request.purpose === 'explore')).toHaveLength(1);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await entry.locator('.purpose-submit').click();
  await expect.poll(() => fixture.requests.length).toBe(1);
  expect(fixture.requests[0]).toMatchObject({ learning_purpose: 'explore', message: guides.explore.draft + ' 我想到公交车等红灯的情况。' });
  expect(fixture.errors).toEqual([]);
});
});

test('unavailable guidance keeps the real reader input usable', async ({ page }) => {
  const fixture = await setup(page, 'markdown', true);
  await page.route('**/api/reader/guidance', route => route.fulfill({ status: 503, json: { error_code: 'READING_GUIDANCE_PROVIDER_FAILED', message: 'offline' } }));
  await page.reload();
  const entry = page.locator('.learning-purpose-entry');
  await entry.locator('[data-purpose=understand]').click();
  await expect(entry.getByRole('textbox')).toHaveAttribute('placeholder', '写一个概念、一段话，或你卡住的地方。');
  await entry.getByRole('textbox').fill('我想知道总时间怎样计算');
  await entry.locator('.purpose-submit').click();
  await expect.poll(() => fixture.requests.length).toBe(1);
  expect(fixture.requests[0].message).toBe('我想知道总时间怎样计算');
});
