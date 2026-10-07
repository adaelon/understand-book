import { expect, test } from '@playwright/test';

test('RE3 long paragraph marker closes at its source line and on buffer recycling', async ({ page }) => {
  await page.goto('/reader-typography-visual.html?annotations');
  const marker = page.locator('[data-annotation-lid="1.1"]');
  await expect(marker).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  const quote = await page.locator('.prose [data-lid="1.1"]').evaluate(el => {
    const node = el.querySelector('.reader-cjk')!.firstChild!;
    const range = document.createRange(); range.setStart(node, 0); range.setEnd(node, 4);
    getSelection()!.removeAllRanges(); getSelection()!.addRange(range);
    return getSelection()!.toString();
  });
  await page.locator('.reader-pane').dispatchEvent('pointerdown');
  await marker.evaluate((el: HTMLButtonElement) => el.click());
  await expect(page.locator('.annotation-preview')).toBeVisible();
  expect(await page.evaluate(() => getSelection()!.toString())).toBe(quote);
  await page.evaluate(() => document.dispatchEvent(new Event('pointerup')));
  await page.getByRole('button', { name: '笔记 2', exact: true }).click();
  await expect(page.locator('.annotation-preview')).toContainText('同段后面的引用');
  await page.locator('.reader-pane').evaluate(el => { el.scrollTop = 700; });
  await expect(page.locator('.annotation-preview')).toHaveCount(0);
  await page.locator('.reader-pane').evaluate(el => { el.scrollTop = 0; });
  await marker.click();
  await expect(page.locator('.annotation-preview')).toBeVisible();
  await page.evaluate(() => (window as any).reFixture.recycle());
  await expect(page.locator('.annotation-preview, .annotation-marker')).toHaveCount(0);
});
