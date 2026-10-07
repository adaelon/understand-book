import { expect, test } from '@playwright/test';

test('two host activities retain scene, attempts and displayed help across reopening', async ({ page }) => {
  let failHint = true;
  await page.route('**/api/**', async route => {
    const request = route.request();
    if (request.url().endsWith('/tutor/action') && request.postDataJSON().action === 'request_hint' && failHint) {
      failHint = false;
      await route.fulfill({ status: 503, json: { error_code: 'TEST_UNAVAILABLE', category: 'unavailable', message: '提示暂不可用' } }); return;
    }
    const response = await route.fetch({ url: request.url().replace(/^.*\/api/, 'http://127.0.0.1:4175') });
    await route.fulfill({ response });
  });
  const facts = async () => (await page.request.get('http://127.0.0.1:4175/facts').then(r => r.json())).map((row: any) => row[1]);
  await page.goto('/agent-presentation-visual.html');
  const frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#result')).toHaveText('2');
  const one = page.locator('.agent-presentation .tutor-activities article').filter({ has: page.getByText('Observe one', { exact: true }) });
  const two = page.locator('.agent-presentation .tutor-activities article').filter({ has: page.getByText('Observe two', { exact: true }) });
  await two.scrollIntoViewIfNeeded();
  await one.scrollIntoViewIfNeeded();
  await expect.poll(async () => (await facts()).filter((f: any) => f.kind === 'displayed').length).toBe(2);
  await frame.locator('#speed').evaluate((node: HTMLInputElement) => { node.value = '4'; node.dispatchEvent(new Event('input', { bubbles: true })); });
  await expect(frame.locator('#result')).toHaveText('4');
  expect((await facts()).filter((f: any) => f.kind === 'learner_action')).toHaveLength(0);
  await one.getByRole('textbox').fill('total distance / total time');
  await one.getByRole('button', { name: '提交回答', exact: true }).click();
  await expect(one.getByText('回答已记录，尚未评分。')).toBeVisible();
  const submitted = (await facts()).find((f: any) => f.kind === 'learner_action');
  expect(submitted.payload.scene.state.values.page.speed).toBe(4);
  expect(submitted.payload.assessment).toBe('unassessed');
  await one.getByRole('button', { name: '提示', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('提示暂不可用');
  expect((await facts()).filter((f: any) => f.kind === 'help_displayed')).toHaveLength(0);
  await page.getByRole('alert').getByRole('button', { name: '重试' }).click();
  await expect(one.getByText('Compare the two times.')).toBeVisible();
  await expect.poll(async () => (await facts()).filter((f: any) => f.kind === 'help_displayed').length).toBe(1);
  await two.getByRole('textbox').fill('my independent response');
  await two.getByRole('button', { name: '提交回答', exact: true }).click();
  await expect(two.getByText('回答已记录，尚未评分。')).toBeVisible();
  const before = await facts();
  expect(before.filter((f: any) => f.kind === 'learner_action' && f.payload.action === 'submit')).toHaveLength(2);
  const state = await page.request.get('http://127.0.0.1:4175/tutor/state').then(r => r.json());
  await page.request.post('http://127.0.0.1:4175/tutor/mutate', { data: { operation_id: 'browser-off', expected_revision: state.control.revision, action: { kind: 'set_enabled', enabled: false } } });
  await page.request.post('http://127.0.0.1:4175/reopen');
  await page.reload();
  await expect(one.getByText('此活动已展示过帮助；重开页面会保留这项记录。')).toBeVisible();
  await expect(one.getByRole('button', { name: '改答', exact: true })).toBeDisabled();
  await one.getByRole('button', { name: '直接讲解', exact: true }).click();
  await expect(one.getByText('Use total distance divided by total time.')).toBeVisible();
  await expect.poll(async () => (await facts()).filter((f: any) => f.kind === 'help_displayed').length).toBe(2);
  await expect(frame.locator('#speed')).toHaveValue('4');
  await page.unrouteAll({ behavior: 'wait' });
});
