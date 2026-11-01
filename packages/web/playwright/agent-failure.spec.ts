import { expect, test } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

for (const width of [390, 1440]) {
  test(`public failure preserves a usable conversation at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 844 });
    await page.route('**/me/notes/attachments', route => route.fulfill({ json: { items: [] } }));
    for (const code of ['TOOL_ARGUMENTS_INVALID', 'FUTURE_INTERNAL_ERROR']) {
      await page.goto(`/mobile-workspace-visual.html?failed-answer=${code}`);
      if (width < 768) await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
      await expect(page.locator('.incomplete')).toHaveText('这次回答未能完成，请重试。');
      await expect(page.locator('.incomplete')).toBeVisible();
      await expect(page.locator('body')).not.toContainText('$.move');
      await expect(page.locator('body')).not.toContainText('anyOf');
      await expect(page.locator('.send-agent')).toBeEnabled();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    }
    const output = resolve('../../docs/performance/tutor-error-stop-20261101');
    mkdirSync(output, { recursive: true });
    await page.screenshot({ path: resolve(output, `public-failure-${width}.png`), fullPage: true });
  });
}
