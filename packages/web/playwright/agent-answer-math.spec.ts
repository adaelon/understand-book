import { expect, test } from '@playwright/test';

for (const viewport of [{ width: 390, height: 844 }, { width: 1440, height: 1000 }]) {
  test(`screenshot answer renders inline and display math at ${viewport.width}px`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await page.goto('/mobile-workspace-visual.html?math-answer');
    if (viewport.width < 768) {
      await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
    }
    const answer = page.locator('.a-msg .ans-text');
    await expect(answer.locator('.katex-display')).toHaveCount(2);
    await expect(answer.locator('p > .katex')).toHaveCount(10);
    await expect(answer.locator('.katex-error')).toHaveCount(0);
    await page.evaluate(() => document.fonts.ready);
    const formula = answer.locator('.katex-display').first();
    await expect(formula.locator('.katex-html')).toBeVisible();
    expect(await formula.locator('.katex-html').evaluate(el => getComputedStyle(el).fontFamily)).toContain('KaTeX_Main');
    expect(await formula.locator('.katex-mathml').evaluate(el => getComputedStyle(el).position)).toBe('absolute');
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
    for (const element of await answer.locator('.katex-display').all()) {
      expect(await element.evaluate(el => el.scrollWidth - el.clientWidth)).toBeLessThanOrEqual(1);
    }
    await page.locator('.transcript').evaluate(el => { el.scrollTop = 0; });
    await page.screenshot({ path: test.info().outputPath(`math-answer-${viewport.width}.png`) });
    await answer.screenshot({ path: test.info().outputPath(`math-answer-content-${viewport.width}.png`) });
  });
}
