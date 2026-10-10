import { expect, test } from '@playwright/test';

test('readiness updates independently, exposes source gaps and preserves the saved learning request', async ({ page }) => {
  await page.request.post('http://127.0.0.1:4175/reset-tutor');
  await page.route('**/api/**', async route => {
    const response = await route.fetch({ url: route.request().url().replace(/^.*\/api/, 'http://127.0.0.1:4175') });
    await route.fulfill({ response });
  });
  await page.goto('/agent-presentation-visual.html?tutor');
  await page.getByRole('button', { name: '管理教学会话' }).click();
  const panel = page.getByRole('dialog', { name: '学习会话', exact: true });
  await expect(panel.getByTestId('teaching-readiness')).toContainText('接纳');
  await panel.getByRole('textbox').fill('辨析平均值与各段速度');
  await panel.getByRole('button', { name: '保存学习意图' }).click();
  // The Server contract is tested with real version files. This browser projection
  // supplies a newly completed background publication without running another build.
  await page.route('**/api/tutor/readiness', route => route.fulfill({ json: { status: 'ready', source_id: 'fixture-book', source_revision: 's1',
    limitations: ['作者未提供各段测量记录'], reason: '可以学习', teaching_assets: { status: 'ready', teaching_map_revision: 'v1', limitations: ['作者未提供各段测量记录'], reason: '资料已就绪' } } }));
  await panel.getByRole('button', { name: '刷新学习状态' }).click();
  await expect(panel.getByTestId('teaching-readiness')).toContainText('可以开始或继续学习');
  await expect(panel.getByText('作者未提供各段测量记录')).toBeVisible();
  await expect(panel.getByRole('heading', { name: '辨析平均值与各段速度 当前' })).toBeVisible();
  await expect(page.getByRole('switch').first()).toHaveAttribute('aria-checked', 'false');
  await page.route('**/api/tutor/readiness', route => route.fulfill({ status: 503, json: { error_code: 'UNAVAILABLE', category: 'unavailable', message: '读取失败' } }));
  await panel.getByRole('button', { name: '刷新学习状态' }).click();
  await expect(panel.getByTestId('teaching-readiness')).toContainText('读取失败');
  await expect(panel.getByTestId('teaching-readiness')).not.toContainText('可以开始或继续学习');
});
