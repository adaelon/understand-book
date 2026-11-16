import { expect, test } from '@playwright/test';
import { setup } from './fixtures/tutor-continuation';

for (const width of [1440, 390]) test(`ordinary continuation needs no teaching switch at ${width}px`, async ({ page }) => {
  const fixture = await setup(page, 'markdown', true, true);
  await page.setViewportSize({ width, height: 900 });
  if (width < 1024) await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
  await expect(page.getByRole('switch', { name: '连续教学' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: '开启连续教学' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: '继续理解这段', exact: true })).toBeVisible();
  await page.screenshot({ path: `../../tmp/tutor-quality-20261115/unified-${width}.png` });
  await page.getByRole('button', { name: '继续理解这段', exact: true }).click();
  await expect.poll(() => fixture.requests.length).toBe(1);
  expect(fixture.requests[0]).toMatchObject({ entry_action: 'continue_answer', answer_ref: { turn_id: 'tc0-answer' } });
  expect(fixture.mutations).toEqual([]);
  expect(fixture.errors).toEqual([]);
});

test('explicit practice submits directly with the legacy control disabled', async ({ page }) => {
  const fixture = await setup(page, 'markdown', true, true);
  await page.getByRole('button', { name: '学习设置', exact: true }).click();
  await page.getByRole('button', { name: '为这本书设定目标', exact: true }).click();
  await page.locator('[data-purpose="practice"]').click();
  await page.locator('.purpose-setup textarea').fill('用总路程与总时间出一道练习题');
  await page.locator('.purpose-submit').click();
  await expect.poll(() => fixture.requests.length).toBe(1);
  expect(fixture.requests[0]).toMatchObject({ entry_action: 'learn_with_purpose', learning_purpose: 'practice' });
  expect(fixture.mutations).toEqual([]);
});
