import { expect, test, type Page } from '@playwright/test';

async function setAnchor(page: Page, lid: string) {
  await page.evaluate(lid => (window as any).outlineFixture.anchor(lid), lid);
}

async function centerError(page: Page) {
  return page.locator('.outline-list').evaluate(list => {
    const row = list.querySelector('.outline-item.active')!.getBoundingClientRect();
    const bounds = list.getBoundingClientRect();
    return Math.abs((row.top + row.bottom) / 2 - (bounds.top + list.clientTop + list.clientHeight / 2));
  });
}

test('current chapter follows the middle of the outline without moving the reader', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto('/playwright/fixtures/outline-follow.html');
  await expect(page.locator('.outline-item.active')).toHaveText('章第 25 章 阅读目录');
  await expect.poll(() => centerError(page)).toBeLessThanOrEqual(1);
  await page.locator('.reader-pane').evaluate(el => { el.scrollTop = 850; });
  for (const lid of ['26.1', '27.1', '24.1']) {
    await setAnchor(page, lid);
    await expect.poll(() => centerError(page)).toBeLessThanOrEqual(1);
    expect(await page.locator('.reader-pane').evaluate(el => el.scrollTop)).toBe(850);
    expect(await page.locator('.left-rail').evaluate(el => el.scrollTop)).toBe(0);
  }
  // Reading another paragraph in the same chapter must not undo manual outline browsing.
  await page.locator('.outline-list').evaluate(el => { el.scrollTop = 120; });
  await setAnchor(page, '24.2');
  expect(await page.locator('.outline-list').evaluate(el => el.scrollTop)).toBe(120);
});

test('first and last chapters stop at the natural outline boundaries', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto('/playwright/fixtures/outline-follow.html');
  await setAnchor(page, '1.1');
  expect(await page.locator('.outline-list').evaluate(el => el.scrollTop)).toBe(0);
  await setAnchor(page, '60.1');
  expect(await page.locator('.outline-list').evaluate(el => Math.abs(el.scrollHeight - el.clientHeight - el.scrollTop))).toBeLessThanOrEqual(1);
});

for (const viewport of [{ width: 1280, height: 900 }, { width: 1100, height: 440 }, { width: 390, height: 844 }]) {
  test(`progress stays at the bottom for long, short and empty outlines at ${viewport.width}x${viewport.height}`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await page.goto('/playwright/fixtures/outline-follow.html');
    if (viewport.width < 1024) await page.evaluate(() => (window as any).outlineFixture.open());
    const checkBottom = async () => {
      await expect(page.locator('.rail-position')).toBeVisible();
      const error = await page.locator('.left-rail').evaluate(rail => {
        const bottom = rail.querySelector('.rail-position')!.getBoundingClientRect().bottom;
        return Math.abs(rail.getBoundingClientRect().bottom - parseFloat(getComputedStyle(rail).paddingBottom) - bottom);
      });
      expect(error).toBeLessThanOrEqual(1);
      expect(await page.locator('.left-rail').evaluate(el => el.scrollHeight - el.clientHeight)).toBeLessThanOrEqual(1);
    };
    await checkBottom();
    await page.evaluate(() => (window as any).outlineFixture.count(3));
    await checkBottom();
    await page.getByPlaceholder('搜索目录').fill('不存在的章节');
    await expect(page.locator('.rail-muted')).toBeVisible();
    await checkBottom();
  });
}
