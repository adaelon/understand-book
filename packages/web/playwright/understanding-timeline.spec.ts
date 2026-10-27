import { test, expect } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

const output = resolve('../../docs/performance/followup-understanding-20261024/ui');
for (const width of [320, 390, 1280]) {
  test(`understanding timeline ${width}px preserves chronology, help and correction`, async ({ page }) => {
    mkdirSync(output, { recursive: true });
    await page.setViewportSize({ width, height: 900 });
    await page.goto('/understanding-timeline-visual.html');
    await expect(page.locator('.concept')).toHaveCount(3);
    await page.screenshot({ path: resolve(output, `overview-${width}.png`) });
    await page.locator('.concept').first().click();
    await expect(page.locator('.reading-title')).toHaveText('Prefill 与 Decode 的分工');
    await page.locator('.understanding-timeline > summary').click();
    const timeline = page.getByRole('region', { name: '理解变化时间线', exact: true });
    await expect(timeline.locator('.timeline-list > li')).toHaveCount(3);
    await expect(timeline.locator('.timeline-list > li').nth(0)).toContainText('2026年10月23日');
    await expect(timeline.locator('.timeline-list > li').nth(0)).toContainText('当时得到的帮助');
    await expect(timeline.locator('.timeline-list > li').nth(1)).toContainText('解释已被更正');
    await expect(timeline.locator('.timeline-list > li').nth(2)).toContainText('对原解释的更正');
    await page.locator('.understanding-timeline > summary').evaluate(el => el.scrollIntoView({ block: 'start' }));
    await page.screenshot({ path: resolve(output, `timeline-${width}.png`) });
    await timeline.locator('.timeline-list > li').nth(2).scrollIntoViewIfNeeded();
    await page.screenshot({ path: resolve(output, `correction-${width}.png`) });
    const overflow = await page.locator('.tutor-panel').evaluate(el => ({ width: el.clientWidth, scroll: el.scrollWidth }));
    expect(overflow.scroll).toBeLessThanOrEqual(overflow.width + 1);
    await timeline.locator('.timeline-list > li').first().getByRole('button', { name: '查看这次依据' }).click();
    await expect(page.locator('.evidence .quote')).toContainText('直到模型输出停止符');
    await expect(page.locator('.evidence')).toContainText('历史解释');
    await page.getByRole('button', { name: '← 返回概念列表' }).click();
    await page.locator('.concept').last().click();
    await page.locator('.understanding-timeline > summary').click();
    await expect(page.getByRole('region', { name: '理解变化时间线', exact: true })).toContainText('原话时间未保留');
    await expect(page.getByRole('region', { name: '理解变化时间线', exact: true })).toContainText('这是目前唯一一次记录');
  });
}
