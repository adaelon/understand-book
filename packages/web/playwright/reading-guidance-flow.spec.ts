import { expect, test } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { scenario, selectSource, setup } from './fixtures/tutor-continuation';
import type { ReadingGuidanceRequest } from '../src/reading-guidance';

const output = resolve('../../docs/performance/reading-guidance-20261102/flow');
// These are returning-reader flows; onboarding has its own UI and tests.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('understand-book.product-tour.v1.local.reader', 'done'));
});
const guides = {
  followup: { question: '总路程与总时间的关系，你想从哪一步继续说？', draft: '关于总时间怎样计算，我的想法是……' },
  thought: { question: '你原来只算行驶时间，是怎么考虑的？', draft: '我原来只算行驶时间，是因为……' },
  selection: { question: '这段对总时间的说明，你想追问什么？', draft: '关于这里的总时间，我想说的是……' },
};

for (const width of [1366, 390, 320]) test.describe(`reading guidance flow ${width}px`, () => {
  test.use({ hasTouch: width < 1024, isMobile: width < 1024 });
  test('continues delivered explanations and the reader’s thought, then follows a new selection', async ({ page }) => {
    const fixture = await setup(page, 'markdown', true, true);
    const calls: ReadingGuidanceRequest[] = [];
    await page.route('**/api/reader/guidance', async route => {
      const request = route.request().postDataJSON() as ReadingGuidanceRequest; calls.push(request);
      await route.fulfill({ json: request.purpose === 'selection' ? guides.selection
        : request.recent_turns?.at(-1)?.user.includes('只应该算行驶时间') ? guides.thought : guides.followup });
    });
    await page.setViewportSize({ width, height: width > 1000 ? 900 : 844 });
    await page.reload();
    if (width < 1024) await page.getByRole('navigation', { name: '阅读工作区' }).getByRole('button', { name: '问答', exact: true }).click();
    const input = page.locator('.agent-input textarea');
    await expect(input).toHaveAttribute('placeholder', guides.followup.draft);
    expect(calls.at(-1)).toMatchObject({ purpose: 'followup', recent_turns: [{ user: 'Explain average speed', assistant: scenario.answer }] });
    expect(fixture.requests).toHaveLength(0);
    mkdirSync(output, { recursive: true });
    await page.screenshot({ path: resolve(output, `followup-${width}.png`) });

    fixture.turns[0].user = '我觉得只应该算行驶时间。';
    await page.reload();
    if (width < 1024) await page.getByRole('navigation', { name: '阅读工作区' }).getByRole('button', { name: '问答', exact: true }).click();
    await expect(input).toHaveAttribute('placeholder', guides.thought.draft);
    await expect(page.locator('.reading-input-invitation')).toHaveText(guides.thought.question);
    const callsBeforeEditing = calls.length;
    await page.screenshot({ path: resolve(output, `thought-${width}.png`) });
    if (width > 1000) await input.press('Tab');
    else await page.locator('.agent-input .input-suggestion-hint button').tap();
    await expect(input).toHaveValue(guides.thought.draft);
    await input.fill('我原来只算行驶时间，是因为停下来时路程没有增加。');
    await expect(page.locator('.reading-input-invitation')).toHaveCount(0);
    expect(fixture.requests).toHaveLength(0);
    await input.fill('');
    await expect(input).toHaveAttribute('placeholder', guides.thought.draft);
    expect(calls).toHaveLength(callsBeforeEditing);

    if (width < 1024) await page.getByRole('button', { name: '返回阅读', exact: true }).click();
    await selectSource(page, 'markdown');
    await page.locator('.hl-popover').getByRole('button', { name: '问 AI', exact: true }).click();
    await expect(input).toHaveAttribute('placeholder', guides.selection.draft);
    expect(calls.at(-1)).toMatchObject({ purpose: 'selection', quote: scenario.quote, lid: '1.1' });
    await expect(page.locator('.ask-draft')).toContainText(scenario.quote);
    await page.screenshot({ path: resolve(output, `selection-${width}.png`) });
    expect(await page.evaluate(() => innerWidth)).toBe(width);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.locator('.agent-input .input-suggestion-hint button').click();
    await input.fill('我想问这段里的总时间：为什么要包含停留时间？');
    await page.locator('.send-agent').click();
    await expect.poll(() => fixture.requests.length).toBe(1);
    expect(fixture.requests[0]).toMatchObject({ message: '我想问这段里的总时间：为什么要包含停留时间？', question_quote: { quote: scenario.quote, lid: '1.1' } });
    expect(fixture.errors).toEqual([]);
  });
});

test('a resolved PDF selection uses the selected source in its guide', async ({ page }) => {
  const fixture = await setup(page, 'pdf', true, true);
  let selection: ReadingGuidanceRequest | undefined;
  await page.route('**/api/reader/guidance', async route => {
    const request = route.request().postDataJSON();
    if (request.purpose === 'selection') selection = request;
    await route.fulfill({ json: request.purpose === 'selection' ? guides.selection : null });
  });
  await selectSource(page, 'pdf');
  await page.locator('.hl-popover').getByRole('button', { name: '问 AI', exact: true }).click();
  await expect(page.locator('.agent-input textarea')).toHaveAttribute('placeholder', guides.selection.draft);
  expect(selection).toMatchObject({ purpose: 'selection', quote: scenario.quote, lid: '1.1' });
  expect(fixture.requests).toHaveLength(0);
});

test('a quiet model decision preserves the normal input and submit flow', async ({ page }) => {
  const fixture = await setup(page, 'markdown', true, true);
  let calls = 0;
  await page.route('**/api/reader/guidance', route => { calls++; return route.fulfill({ json: null }); });
  await page.reload();
  await expect.poll(() => calls).toBe(1);
  await expect(page.locator('.reading-input-invitation')).toHaveCount(0);
  await page.locator('.agent-input textarea').fill('我还有一个问题');
  await page.locator('.send-agent').click();
  await expect.poll(() => fixture.requests.length).toBe(1);
  expect(fixture.requests[0].message).toBe('我还有一个问题');
});

for (const pending of ['response', 'choice'] as const) test(`no model guide competes with an open ${pending}`, async ({ page }) => {
  await setup(page, 'markdown', true, true, true);
  let calls = 0;
  await page.route('**/api/reader/guidance', route => { calls++; return route.fulfill({ json: guides.followup }); });
  if (pending === 'response') await page.route('**/api/tutor/activities', route => route.fulfill({ json: { activities: [{
    delivery_ref: 'response', status: 'active', attempted: false, help_seen: [],
    move: { move_id: 'practice', prompt: '请写出你的计算方法', actions: ['submit'], response_expectation: { public_prompt: '请写出你的计算方法', block_refs: [] } },
  }] } }));
  await page.reload();
  if (pending === 'response') await expect(page.locator('textarea[data-workspace-input=tutor]')).toBeVisible();
  else await expect(page.locator('.tutor-continuation')).toBeVisible();
  await page.locator('.agent-input textarea').fill('我想先问别的');
  await page.locator('.agent-input textarea').fill('');
  await expect(page.locator('.reading-input-invitation')).toHaveCount(0);
  expect(calls).toBe(0);
});
