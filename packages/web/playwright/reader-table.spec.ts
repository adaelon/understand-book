import { expect, test } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.goto('/reader-typography-visual.html?tables');
  await expect(page.locator('[data-lid="1.4"] table')).toBeVisible();
});

test('EPUB table keeps its cells, canonical selection and controls in the reader', async ({ page }) => {
  await page.setViewportSize({ width: 1200, height: 900 });
  const asset = page.locator('[data-lid="1.4"]');
  await expect(asset.locator('tr')).toHaveCount(7);
  await expect(asset.locator('td')).toHaveCount(21);
  await expect(asset.locator('.asset-table')).toHaveJSProperty('tagName', 'DIV');
  await expect(page.locator('[data-lid="1.7"] th')).toHaveCount(2);
  await expect(page.locator('[data-lid="1.7"] strong')).toHaveText('阅读');

  await asset.getByRole('button', { name: '换行', exact: true }).click();
  await expect(asset.locator('td').first()).toHaveCSS('white-space', 'normal');
  const selection = await page.evaluate(async () => {
    const moduleUrl = '/src/markdown-source-map.ts';
    const { createMarkdownDomSourceMap, sourceTextForRanges } = await import(moduleUrl);
    const source = (window as any).reFixture.source('1.4');
    const root = document.querySelector('[data-lid="1.4"]')!;
    const cell = root.querySelectorAll('tr')[4].children[1];
    const range = document.createRange(); range.selectNodeContents(cell);
    const selection = getSelection()!; selection.removeAllRanges(); selection.addRange(range);
    const ranges = createMarkdownDomSourceMap(source, root).sourceRangesForRange(range);
    return { quote: sourceTextForRanges(source, ranges), ranges, expectedStart: source.indexOf('读者的私人数据') };
  });
  expect(selection.quote).toBe('读者的私人数据');
  expect(selection.ranges).toEqual([{ start: selection.expectedStart, end: selection.expectedStart + selection.quote.length }]);

  await asset.getByRole('button', { name: '展开', exact: true }).click();
  await expect(asset).toHaveClass(/asset-expanded/);
  await expect(asset.locator('td')).toHaveCount(21);
  await asset.getByRole('button', { name: '收起', exact: true }).click();
  await expect(asset).not.toHaveClass(/asset-expanded/);
  await expect(asset.getByRole('button', { name: '定位', exact: true })).toHaveCount(0);
  await asset.locator('td').first().click();
  await expect(asset.locator('tr')).toHaveCount(7);
});

test('table scroll stays local and wrapped cells fit desktop and narrow screens', async ({ page }) => {
  const asset = page.locator('[data-lid="1.4"]');
  for (const width of [1200, 390, 320]) {
    await page.setViewportSize({ width, height: 900 });
    expect(await page.locator('.reader-pane').evaluate(el => el.scrollWidth - el.clientWidth)).toBeLessThanOrEqual(1);
    await expect(asset.locator('td').first()).toHaveCSS('white-space', 'nowrap');
    await asset.getByRole('button', { name: '换行', exact: true }).click();
    await expect(asset.locator('td').first()).toHaveCSS('white-space', 'normal');
    expect(await asset.locator('.asset-table').evaluate(el => el.scrollWidth - el.clientWidth)).toBeLessThanOrEqual(1);
    await expect(asset.locator('td')).toHaveCount(21);
    if (width !== 320) await page.screenshot({ path: `../../tmp/reader-table-fix/table-${width}.png` });
    await asset.getByRole('button', { name: '不换行', exact: true }).click();
  }
});
