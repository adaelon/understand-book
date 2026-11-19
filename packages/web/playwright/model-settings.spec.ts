import { expect, test } from '@playwright/test';

test('model settings handles long model names, purpose overrides and explicit text tests', async ({ page }, info) => {
  await page.setViewportSize({ width: 1100, height: 1000 });
  await page.goto('/model-settings-visual.html?scenario=long');
  await expect(page.getByTestId('default-model')).toHaveValue('reading-model');
  await expect(page.getByTestId('service-key')).toHaveValue('');
  await page.getByTestId('test-model').click();
  await expect(page.getByRole('status')).toContainText('工具调用和结构化输出尚未测试');
  await page.getByText('按用途选择模型', { exact: true }).click();
  await expect(page.getByRole('combobox', { name: '教学与练习反馈', exact: true })).toHaveValue('teaching-model');
  await page.screenshot({ path: info.outputPath('model-settings-long.png'), fullPage: true });
  await page.getByTestId('model-id').fill('changed-model');
  await page.getByTestId('test-model').click();
  await expect(page.getByRole('status')).toContainText('草稿文本生成通过，配置尚未保存');
  const draftCalls = await page.evaluate(() => (window as unknown as { __MODEL_SETTINGS_PREVIEW__: {saves:unknown[];testInputs:Array<{models:Array<{model_id:string}>}|null>} }).__MODEL_SETTINGS_PREVIEW__);
  expect(draftCalls.saves).toHaveLength(0);
  expect(draftCalls.testInputs[1]?.models[0].model_id).toBe('changed-model');
  await page.getByTestId('save-model-settings').click();
  await expect(page.getByText('已保存并应用', { exact: true })).toBeVisible();
  const calls = await page.evaluate(() => (window as unknown as { __MODEL_SETTINGS_PREVIEW__: {saves:unknown[];tests:string[]} }).__MODEL_SETTINGS_PREVIEW__);
  expect(calls.saves).toHaveLength(1);
  expect(calls.tests).toEqual(['reading-model','reading-model']);
});

test('narrow English settings stays within the viewport and retains a failed save', async ({ page }, info) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/model-settings-visual.html?scenario=save-failure&lang=en');
  await page.getByTestId('model-id').fill('changed-but-not-saved');
  await page.getByTestId('save-model-settings').click();
  await expect(page.getByRole('alert')).toContainText('Settings file is not writable');
  await expect(page.getByTestId('model-id')).toHaveValue('changed-but-not-saved');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.screenshot({ path: info.outputPath('model-settings-narrow-failure.png'), fullPage: true });
});

test('empty settings saves an unauthenticated local model without implicit calls', async ({ page }, info) => {
  await page.goto('/model-settings-visual.html?scenario=empty');
  await page.getByRole('button', {name:'添加模型服务',exact:true}).click();
  await page.getByRole('combobox', {name:'服务预设',exact:true}).selectOption('local');
  await expect(page.getByTestId('service-auth')).toHaveValue('none');
  await page.getByTestId('model-id').fill('local-tool-model');
  await page.getByTestId('model-native-tools').selectOption({label:'支持'});
  await page.getByTestId('save-model-settings').click();
  await expect(page.getByText('已保存并应用', {exact:true})).toBeVisible();
  const tests = await page.evaluate(() => (window as unknown as {__MODEL_SETTINGS_PREVIEW__:{tests:string[]}}).__MODEL_SETTINGS_PREVIEW__.tests);
  expect(tests).toEqual([]);
  await page.screenshot({ path: info.outputPath('model-settings-local.png'), fullPage:true });
});
