import { expect, test } from '@playwright/test';

test('global control persists, shares the presentation surface, and preserves draft and scene across source visits', async ({ page }, info) => {
  await page.request.post('http://127.0.0.1:4175/reset-tutor');
  await page.request.post('http://127.0.0.1:4175/reset-scene');
  await page.route('**/api/**', async route => {
    const response = await route.fetch({ url: route.request().url().replace(/^.*\/api/, 'http://127.0.0.1:4175') });
    await route.fulfill({ response });
  });
  await page.goto('/agent-presentation-visual.html?tutor');
  const toggle = page.getByRole('switch').first();
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  await page.getByRole('button', { name: '管理教学会话' }).click();
  const panel = page.getByRole('dialog', { name: '学习会话', exact: true });
  await panel.getByRole('textbox').fill('理解证据召回率');
  await panel.getByRole('button', { name: '保存学习意图' }).click();
  await expect(panel.getByRole('heading', { name: '理解证据召回率 当前' })).toBeVisible();
  await panel.getByRole('button', { name: '关闭', exact: true }).click();
  await page.getByRole('button', { name: '展开', exact: true }).click();
  const workspaceToggle = page.locator('.workspace-tools').getByRole('switch');
  await expect(workspaceToggle).toHaveAttribute('aria-checked', 'true');
  await page.locator('.agent-input textarea').fill('未发送的后续问题');
  const frame = page.frameLocator('.agent-presentation iframe');
  await frame.locator('#evidence-count').evaluate((node: HTMLInputElement) => {
    node.value = '1'; node.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await frame.locator('[data-source-ref]').first().click();
  await expect(page.getByRole('dialog', { name: '回答来源' })).toBeVisible();
  await page.getByRole('button', { name: '在正文中查看' }).click();
  await page.getByRole('button', { name: '返回演示', exact: true }).click();
  await expect(frame.locator('#result')).toHaveText('1/3');
  await expect(page.locator('.agent-presentation iframe')).toHaveAttribute('data-load-count', '1');
  await expect(page.locator('.agent-input textarea')).toHaveValue('未发送的后续问题');
  await page.setViewportSize({ width: 390, height: 700 });
  await page.getByRole('button', { name: '展开讨论', exact: true }).click();
  await expect(page.locator('.agent-input textarea')).toBeVisible();
  await expect(workspaceToggle).toBeVisible();
  const controlBox = await workspaceToggle.boundingBox();
  expect(controlBox!.x + controlBox!.width).toBeLessThanOrEqual(390);
  await page.screenshot({ path: info.outputPath('tutor-narrow-workspace.png') });
  // A failed save must not optimistically flip the visible switch.
  await page.route('**/api/tutor/mutate', route => route.fulfill({ status: 503, json: { error_code: 'LEARNING_STORAGE_UNAVAILABLE', category: 'unavailable', message: '写入失败' } }), { times: 1 });
  await workspaceToggle.click();
  await expect(workspaceToggle).toHaveAttribute('aria-checked', 'true');
  await page.locator('.workspace-tools').getByRole('button', { name: '管理教学会话' }).click();
  await expect(panel.getByRole('alert')).toContainText('未确认保存');
  await panel.getByRole('button', { name: '重试', exact: true }).click();
  await expect(panel.getByRole('status').filter({ hasText: /^Tutor 已关闭$/ })).toBeVisible();
  await expect(panel.getByText('已暂停', { exact: true })).toBeVisible();
  await page.request.post('http://127.0.0.1:4175/reopen');
  await page.reload();
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
  await page.getByRole('button', { name: '管理教学会话' }).click();
  await expect(panel.getByRole('heading', { name: '理解证据召回率 当前' })).toBeVisible();
  await expect(panel.getByText('已暂停', { exact: true })).toBeVisible();
});
