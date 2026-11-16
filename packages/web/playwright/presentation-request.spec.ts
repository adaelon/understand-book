import { expect, test } from '@playwright/test';
import { mkdir } from 'node:fs/promises';

const evidence = '../../docs/performance/presentation-request-20261102';

for (const [width, language] of [[1440, 'zh-CN'], [390, 'zh-CN'], [320, 'zh-CN'], [390, 'en']] as const) {
  test(`presentation output is discoverable and usable at ${width}px in ${language}`, async ({ page }) => {
    await mkdir(evidence, { recursive: true });
    await page.setViewportSize({ width, height: width < 400 ? 844 : 900 });
    await page.goto(`/mobile-workspace-visual.html?learning&lang=${language}`);
    if (width < 1024) {
      await page.locator('.workspace-mobile-nav button').filter({ hasText: language === 'en' ? 'Q&A' : '问答' }).click();
    } else {
      await page.getByRole('button', { name: '问答全屏', exact: true }).click();
    }
    const toggle = page.locator('.presentation-request');
    const input = page.locator('.agent-input textarea');
    const draft = await input.inputValue();
    await expect(toggle).toBeVisible();
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'true');
    await expect(input).toHaveValue(draft);
    await expect(page.locator('.presentation-request-hint')).toHaveText(language === 'en' ? 'Create a page for this answer' : '本次回答制作演示页');
    const send = page.locator('.agent-input .send-agent');
    await expect(send).toBeVisible();
    for (const control of [toggle, send, input]) {
      const box = await control.boundingBox();
      expect(box).not.toBeNull();
      expect(box!.x).toBeGreaterThanOrEqual(0);
      expect(box!.x + box!.width).toBeLessThanOrEqual(width);
      expect(box!.y + box!.height).toBeLessThanOrEqual(width < 400 ? 844 : 900);
    }
    await page.screenshot({ path: `${evidence}/${width}-${language}.png` });
    await page.locator('.agent-input').screenshot({ path: `${evidence}/composer-${width}-${language}.png` });
    await toggle.press('Space');
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
    await expect(input).toHaveValue(draft);
    await send.click();
    await expect(input).toHaveValue('');
  });
}
