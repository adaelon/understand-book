import { expect, test } from '@playwright/test';

const answer = '剪接调控会改变心肌细胞的转录本构成，并影响疾病相关通路。不同实验队列提供了相互补充的证据。';
const prompt = `## 1.4.2 SmartNIC：这一站没有「余量」可等

**先把上一站的触发条件放到桌上**

> 请求量增加，现有设备的空闲能力最终会用尽。

盯住「**空闲**」这个词。先看闲着的能力，再判断什么时候搬走工作。[[source:source_ref_methods]]

### ① 变大的是什么

- 单位时间要处理的包变多。
- 每个包需要完成的工作也变多。

这里比较两个量：包速率与每个包的策略处理。$Q$ 表示请求量。

| 工作 | 位置 |
| --- | --- |
| 包处理 | SmartNIC | [[source:source_ref_results]]

### ② 谁来处理

**把网络数据处理交给网卡。** 保留完整条件，再判断能否减轻 CPU 的工作。
`;

for (const width of [1440, 390]) test(`teaching text, citations and duplicate exposure at ${width}px`, async ({ page }, info) => {
  await page.setViewportSize({ width, height: 900 });
  const shown: string[] = [];
  const resolved: string[] = [];
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.route('**/api/tutor/activities', route => route.fulfill({ json: { activities: [
    { delivery_ref: 'lesson', status: 'active', attempted: false, help_seen: [], move: { move_id: 'explain', prompt, actions: [] } },
    { delivery_ref: 'followup', status: 'active', attempted: false, help_seen: [], move: { move_id: 'followup', prompt: answer, actions: [] } },
  ] } }));
  await page.route('**/api/tutor/display', async route => {
    shown.push(route.request().postDataJSON().delivery_ref);
    await route.fulfill({ json: { event_id: 'display' } });
  });
  await page.route('**/api/agent/source.resolve', async route => {
    const id = route.request().postDataJSON().source_ref_id;
    resolved.push(id);
    await route.fulfill({ json: { source_ref_id: id, label: '正文 · SmartNIC', highlighted_quote: '请求量增加，现有设备的空闲能力最终会用尽。', context_before: '', context_after: '', stale: false, can_open_in_reader: true } });
  });
  await page.goto('/agent-source-visual.html?tutor-format=1');
  const activities = page.locator('.tutor-activities');
  await expect(activities.locator('article')).toHaveCount(1);
  await expect(activities.locator('h2')).toHaveText('1.4.2 SmartNIC：这一站没有「余量」可等');
  await expect(activities.locator('h3')).toHaveCount(2);
  await expect(activities.locator('blockquote')).toContainText('空闲能力');
  await expect(activities.locator('.katex')).toHaveCount(1);
  await expect(activities.locator('table')).toHaveCount(1);
  await expect(activities).not.toContainText('[[source:');
  await expect.poll(() => shown.includes('followup')).toBe(true);
  await activities.locator('h2').scrollIntoViewIfNeeded();
  await expect.poll(() => shown.includes('lesson')).toBe(true);
  const source = activities.locator('.agent-source-button').first();
  await source.click();
  await expect(page.locator('.agent-source-popup')).toContainText('空闲能力');
  expect(resolved).toEqual(['source_ref_methods']);
  await page.screenshot({ path: info.outputPath(`tutor-source-${width}.png`), fullPage: true });
  await page.getByRole('button', { name: '关闭来源', exact: true }).click();
  await expect(page.locator('.agent-source-popup')).toHaveCount(0);
  await activities.locator('h2').scrollIntoViewIfNeeded();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  expect(errors).toEqual([]);
  await page.screenshot({ path: info.outputPath(`tutor-format-${width}.png`), fullPage: true });
});
