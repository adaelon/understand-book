import { expect, test } from '@playwright/test';

test.use({ viewport: { width: 390, height: 669 }, hasTouch: true });

async function selectAnswer(page: import('@playwright/test').Page, index = 0) {
  await page.locator('.answer-markdown p').nth(index).scrollIntoViewIfNeeded();
  await page.locator('.answer-markdown p').nth(index).evaluate(el => {
    const range = document.createRange();
    range.selectNodeContents(el);
    const selection = window.getSelection()!;
    selection.removeAllRanges(); selection.addRange(range);
    document.dispatchEvent(new Event('selectionchange'));
  });
}

test.beforeEach(async ({ page }) => {
  await page.goto('/mobile-workspace-visual.html?long-chat&task&learning');
  await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
});

test('touch selection offers notes beside the selected answer and taps save the excerpt', async ({ page }) => {
  await selectAnswer(page);
  const menu = page.locator('.answer-popover');
  await expect(menu).toBeVisible();
  const selected = (await page.locator('.answer-markdown p').first().boundingBox())!;
  const box = (await menu.boundingBox())!;
  expect(box.y - (selected.y + selected.height)).toBeGreaterThanOrEqual(4);
  expect(box.y - (selected.y + selected.height)).toBeLessThanOrEqual(20);
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(390);
  await page.screenshot({ path: test.info().outputPath('selection.png') });
  await menu.getByRole('button', { name: '记笔记', exact: true }).tap();
  await expect(page.locator('.fixture-answer-note')).toHaveText(await page.locator('.answer-markdown p').first().innerText());
  await expect(menu).toHaveCount(0);
});

test('selection menu follows scrolling and clears on collapse or leaving the conversation', async ({ page }) => {
  const transcript = page.locator('.transcript');
  await selectAnswer(page, 1);
  const menu = page.locator('.answer-popover');
  await expect(menu).toBeVisible();
  const original = (await menu.boundingBox())!.y;
  await transcript.evaluate(el => { el.scrollTop += 30; });
  await expect.poll(async () => (await menu.boundingBox())!.y).toBeCloseTo(original - 30, 0);
  await page.evaluate(() => window.getSelection()?.removeAllRanges());
  await expect(menu).toHaveCount(0);
  await selectAnswer(page, 1);
  await expect(menu).toBeVisible();
  await page.locator('.workspace-mobile-nav').getByRole('button', { name: '阅读', exact: true }).tap();
  await expect(menu).toHaveCount(0);
});

test('goal and teaching controls leave most of the mobile screen for answers with direct entries', async ({ page }) => {
  const transcript = page.locator('.transcript');
  await expect.poll(async () => (await transcript.boundingBox())!.height).toBeGreaterThan(669 * .60);
  await test.info().attach('mobile-layout-390', { body: JSON.stringify({ viewportHeight: 669, transcriptHeight: (await transcript.boundingBox())!.height }), contentType: 'application/json' });
  await page.screenshot({ path: test.info().outputPath('conversation-390.png') });
  await expect(page.locator('.agent-head').getByRole('button', { name: '1 项待办', exact: true })).toBeVisible();
  await page.locator('.agent-head').getByRole('button', { name: '1 项待办', exact: true }).tap();
  await expect(page.getByRole('dialog', { name: '当前任务详情' })).toBeVisible();
  await page.getByRole('button', { name: '关闭任务详情' }).tap();
  await page.locator('.agent-input').getByRole('button', { name: '我的理解', exact: true }).tap();
  await expect(page.locator('.fixture-learning-action')).toHaveText('understanding');
  await page.getByRole('button', { name: '调整目标', exact: true }).tap();
  const goalDialog = page.getByRole('dialog', { name: '调整目标', exact: true });
  await expect(goalDialog).toBeVisible();
  await goalDialog.getByRole('textbox', { name: '学习目标', exact: true }).fill('理解软硬件分工');
  await goalDialog.getByRole('button', { name: '保存目标', exact: true }).tap();
  await expect(goalDialog).toHaveCount(0);
  await expect(page.locator('.goal-text')).toHaveText('理解软硬件分工');
  await page.getByRole('button', { name: '问答操作', exact: true }).tap();
  await page.getByRole('button', { name: '学习详情', exact: true }).tap();
  await expect(page.locator('.fixture-learning-action')).toHaveText('details');
  await page.setViewportSize({ width: 320, height: 568 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
  await expect(page.locator('.agent-input textarea')).toBeInViewport();
  await expect.poll(async () => (await transcript.boundingBox())!.height).toBeGreaterThan(568 * .55);
  await test.info().attach('mobile-layout-320', { body: JSON.stringify({ viewportHeight: 568, transcriptHeight: (await transcript.boundingBox())!.height }), contentType: 'application/json' });
  await page.screenshot({ path: test.info().outputPath('conversation-320.png') });
});

test('selection actions fit a narrow viewport and also work with the keyboard', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 568 });
  await selectAnswer(page);
  const menu = page.locator('.answer-popover');
  await expect(menu).toBeVisible();
  const box = (await menu.boundingBox())!;
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(320);
  await menu.getByRole('button', { name: '记笔记', exact: true }).focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('.fixture-answer-note')).not.toBeEmpty();
  await expect(menu).toHaveCount(0);
});
