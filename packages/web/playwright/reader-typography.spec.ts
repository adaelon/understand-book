import { expect, test } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  // All font traffic must remain on the local app origin.
  await page.route('**/*', route => new URL(route.request().url()).hostname === '127.0.0.1' ? route.continue() : route.abort());
  await page.goto('/reader-typography-visual.html');
  await expect(page.locator('.prose')).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
});

test('long paragraph keeps the same source character through font, size and measure changes', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.locator('.reader-pane').evaluate(el => { el.scrollTop = 2500; });
  await page.waitForTimeout(100);
  const anchor = await page.evaluate(() => (window as any).reFixture.capture());
  expect(anchor.textPosition.start).toBeGreaterThan(100);
  await page.getByRole('button', { name: '阅读设置', exact: true }).click();
  for (const [label, value] of [['字体', 'sans'], ['字号', '22'], ['版心', 'narrow']]) {
    const input = page.getByLabel(label, { exact: true });
    if (label === '字号') await input.fill(value); else await input.selectOption(value);
    await page.evaluate(() => document.fonts.ready);
    await expect.poll(async () => Math.abs((await page.evaluate(() => (window as any).reFixture.savedTop())) - anchor.textPosition.top)).toBeLessThan(44);
  }
  await page.getByRole('button', { name: '关闭阅读设置' }).click();
  await page.screenshot({ path: '../../docs/performance/reader-re1-re2/long-paragraph-after.png' });
  await page.reload();
  await expect(page.locator('.prose')).toHaveCSS('font-size', '22px');
  await page.evaluate(() => (window as any).reFixture.switchUser('B'));
  await expect(page.locator('.prose')).toHaveCSS('font-size', '19px');
  await page.evaluate(() => (window as any).reFixture.switchUser('A'));
  await expect(page.locator('.prose')).toHaveCSS('font-size', '22px');
});

test('bundled fonts render offline from external sites; narrow surfaces, assets and selection remain usable', async ({ page }) => {
  await page.evaluate(() => document.fonts.load('400 19px "Noto Serif SC Variable"', '阅读中文 English'));
  expect(await page.evaluate(() => document.fonts.check('400 19px "Noto Serif SC Variable"', '阅读中文 English'))).toBe(true);
  const fontRequests = await page.evaluate(() => performance.getEntriesByType('resource').filter(e => e.name.includes('.woff2')).map(e => e.name));
  expect(fontRequests.some(url => url.includes('noto-serif'))).toBe(true);
  expect(fontRequests.every(url => new URL(url).hostname === '127.0.0.1')).toBe(true);
  for (const [width, height] of [[1440, 900], [1024, 900], [390, 844], [844, 390], [320, 844]]) {
    await page.setViewportSize({ width, height });
    expect(await page.locator('.reader-pane').evaluate(el => el.scrollWidth - el.clientWidth)).toBeLessThanOrEqual(1);
    await page.screenshot({ path: `../../docs/performance/reader-re1-re2/after-${width}.png` });
  }
  const selected = await page.evaluate(() => {
    const node = document.querySelector('[data-lid="1.1"] .reader-cjk')!.firstChild!;
    const range = document.createRange(); range.setStart(node, 0); range.setEnd(node, 4);
    const selection = getSelection()!; selection.removeAllRanges(); selection.addRange(range);
    return selection.toString();
  });
  expect(selected).toBe('阅读不是');
  await page.evaluate(() => (window as any).reFixture.update({ font: 'sans', languageMode: 'latin', fontSizePx: 21, lineHeight: 2, cjkLetterSpacingEm: 0.04, measure: 'wide' }));
  await page.evaluate(() => document.fonts.ready);
  expect(await page.evaluate(() => getSelection()!.toString())).toBe(selected);
  await page.locator('[data-lid="1.2"]').scrollIntoViewIfNeeded();
  await expect(page.locator('.katex')).toBeVisible();
  await page.locator('[data-lid="1.5"]').scrollIntoViewIfNeeded();
  await expect(page.locator('.image-rendered')).toBeVisible();
  expect(await page.locator('.reader-pane').evaluate(el => el.scrollWidth - el.clientWidth)).toBeLessThanOrEqual(1);
  await page.addStyleTag({ content: '.prose { line-height: 2 !important; } .prose p { margin-bottom: 2em !important; } .reader-cjk { letter-spacing: .12em !important; }' });
  expect(await page.locator('.reader-pane').evaluate(el => el.scrollWidth - el.clientWidth)).toBeLessThanOrEqual(1);
  await page.screenshot({ path: '../../docs/performance/reader-re1-re2/assets-320.png' });
});

for (const action of ['scroll', 'scene', 'navigate']) {
  test(`delayed fonts cannot restore after ${action}`, async ({ page }) => {
    let release!: () => void;
    const held = new Promise<void>(resolve => { release = resolve; });
    let pending = 0;
    await page.route('**/*noto-sans*.woff2', async route => { pending++; await held; await route.continue(); });
    await page.locator('.reader-pane').evaluate(el => { el.scrollTop = 2000; });
    await page.waitForTimeout(100);
    await page.evaluate(() => (window as any).reFixture.update({ font: 'sans', languageMode: 'cjk-mixed', fontSizePx: 22, lineHeight: 1.85, cjkLetterSpacingEm: 0.015, measure: 'narrow' }));
    await expect.poll(() => pending).toBeGreaterThan(0);
    if (action === 'scene') await page.evaluate(() => (window as any).reFixture.changeScene());
    if (action === 'navigate') await page.evaluate(() => (window as any).reFixture.navigate());
    else await page.locator('.reader-pane').evaluate(el => { el.scrollTop = 100; el.dispatchEvent(new Event('scroll')); });
    const top = await page.locator('.reader-pane').evaluate(el => el.scrollTop);
    release(); await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(150);
    expect(Math.abs(await page.locator('.reader-pane').evaluate(el => el.scrollTop) - top)).toBeLessThan(2);
  });
}

test('defers requested reflow during selection drag and IME composition', async ({ page }) => {
  await page.locator('.reader-pane').dispatchEvent('pointerdown');
  await page.evaluate(() => (window as any).reFixture.update({ font: 'serif', languageMode: 'cjk-mixed', fontSizePx: 22, lineHeight: 1.85, cjkLetterSpacingEm: 0.015, measure: 'standard' }));
  await expect(page.locator('.prose')).toHaveCSS('font-size', '19px');
  await page.evaluate(() => document.dispatchEvent(new Event('pointerup')));
  await expect(page.locator('.prose')).toHaveCSS('font-size', '22px');
  await page.evaluate(() => {
    document.dispatchEvent(new Event('compositionstart'));
    (window as any).reFixture.update({ font: 'sans', languageMode: 'cjk-mixed', fontSizePx: 18, lineHeight: 1.8, cjkLetterSpacingEm: 0, measure: 'wide' });
  });
  await expect(page.locator('.prose')).toHaveCSS('font-size', '22px');
  await page.evaluate(() => document.dispatchEvent(new Event('compositionend')));
  await expect(page.locator('.prose')).toHaveCSS('font-size', '18px');
});

test('a font becoming ready during a drag waits for pointer release', async ({ page }) => {
  let release!: () => void;
  const held = new Promise<void>(resolve => { release = resolve; });
  let pending = 0;
  await page.route('**/*noto-sans*.woff2', async route => { pending++; await held; await route.continue(); });
  await page.evaluate(() => (window as any).reFixture.update({ font: 'sans', languageMode: 'cjk-mixed', fontSizePx: 22, lineHeight: 1.85, cjkLetterSpacingEm: 0, measure: 'standard' }));
  await expect.poll(() => pending).toBeGreaterThan(0);
  await page.locator('.reader-pane').dispatchEvent('pointerdown');
  release(); await page.evaluate(() => document.fonts.ready);
  await expect(page.locator('.prose')).toHaveCSS('font-size', '19px');
  await page.evaluate(() => document.dispatchEvent(new Event('pointerup')));
  await expect(page.locator('.prose')).toHaveCSS('font-size', '22px');
});
