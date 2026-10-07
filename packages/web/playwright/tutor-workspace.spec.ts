import { expect, test } from '@playwright/test';

test('expanded presentation shares the original answer and draft without reloading its frame', async ({ page }) => {
  await page.request.post('http://127.0.0.1:4175/reset-scene');
  await page.route('**/api/**', async route => {
    const response = await route.fetch({ url: route.request().url().replace(/^.*\/api/, 'http://127.0.0.1:4175') });
    await route.fulfill({ response });
  });
  await page.goto('/agent-presentation-visual.html');
  const frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#result')).toBeVisible();
  await page.getByRole('button', { name: '展开', exact: true }).click();
  await expect(page.locator('.presentation-workspace .agent-input')).toBeVisible();
  await page.locator('.agent-input textarea').fill('为什么是这个结果？');
  await page.locator('.agent-input').getByRole('button', { name: '发送', exact: true }).click();
  await expect(page.getByTestId('follow-up-status')).toHaveText('追问已完成');
  await expect(page.locator('.presentation-workspace .turn')).toHaveCount(2);
  await expect(page.locator('.presentation-workspace .turn').last().locator('.a-msg')).toBeVisible();
  await page.locator('.agent-input textarea').fill('保留草稿');
  await page.getByRole('button', { name: '收起', exact: true }).click();
  await page.getByRole('button', { name: '展开', exact: true }).click();
  await expect(page.locator('.agent-input textarea')).toHaveValue('保留草稿');
  await expect(page.locator('.agent-presentation iframe')).toHaveAttribute('data-load-count', '1');
  // Locating a question must not rewind a running experiment.
  await frame.locator('#evidence-count').evaluate((node: HTMLInputElement) => {
    node.value = '3'; node.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await page.getByRole('button', { name: '定位到演示', exact: true }).click();
  await expect(frame.locator('#result')).toHaveText('1');
  await page.getByRole('button', { name: '回到提问时', exact: true }).click();
  await expect(frame.locator('#result')).toHaveText('2/3');
  await expect(page.locator('.agent-input textarea')).toHaveValue('保留草稿');
  await expect(page.locator('.agent-presentation iframe')).toHaveAttribute('data-load-count', '2');
  await page.setViewportSize({ width: 390, height: 560 });
  await expect(page.locator('.agent-input textarea')).toBeVisible();
  const input = await page.locator('.agent-input textarea').boundingBox();
  expect(input!.x + input!.width).toBeLessThanOrEqual(390);
  expect(input!.y + input!.height).toBeLessThanOrEqual(560);
  await page.getByRole('button', { name: '收起讨论', exact: true }).click();
  await expect(frame.locator('#result')).toBeVisible();
  await page.getByRole('button', { name: '展开讨论', exact: true }).click();
  await expect(page.locator('.agent-input textarea')).toHaveValue('保留草稿');
});

test('a delivered new version stays a reference until explicitly opened', async ({ page }) => {
  await page.request.post('http://127.0.0.1:4175/reset-scene');
  const fixture = await page.request.get('http://127.0.0.1:4175/fixture').then(response => response.json());
  const view = await page.request.post('http://127.0.0.1:4175/agent/presentation.read', { data: fixture }).then(response => response.json());
  let newReads = 0;
  await page.route('**/api/**', async route => {
    const request = route.request();
    if (request.url().endsWith('/agent/chat')) {
      const outcome = structuredClone(fixture.outcome);
      outcome.answer_view.parts = [{ kind: 'markdown', text: '新的版本已准备好' }, { kind: 'presentation', presentation_id: fixture.reference.presentation_id, revision: 2 }];
      await route.fulfill({ json: outcome }); return;
    }
    if (request.url().endsWith('/agent/presentation.read') && request.postDataJSON().reference.revision === 2) {
      newReads++;
      await route.fulfill({ json: { ...view, title: '新的演示', reference: { ...view.reference, revision: 2 } } }); return;
    }
    const response = await route.fetch({ url: request.url().replace(/^.*\/api/, 'http://127.0.0.1:4175') });
    await route.fulfill({ response });
  });
  await page.goto('/agent-presentation-visual.html');
  const old = page.locator('.agent-presentation iframe').first();
  await expect(page.frameLocator('.agent-presentation iframe').locator('#result')).toBeVisible();
  await page.getByRole('button', { name: '展开', exact: true }).click();
  await page.locator('.agent-input textarea').fill('增加一个例子');
  await page.locator('.agent-input').getByRole('button', { name: '发送', exact: true }).click();
  await expect(page.getByText('新的版本已准备好')).toBeVisible();
  expect(newReads).toBe(0);
  await expect(old).toBeVisible();
  await page.getByRole('button', { name: '打开演示 · 版本 2', exact: true }).click();
  await expect(page.locator('.workspace-tools .presentation-toolbar strong')).toHaveText('新的演示');
  expect(newReads).toBe(1);
  await page.getByRole('button', { name: '打开演示 · 版本 1', exact: true }).click();
  await expect(old).toBeVisible();
  await expect(old).toHaveAttribute('data-load-count', '1');
});
