import { expect, test } from '@playwright/test';

test('RN2 records V2, keeps its draft, restores after chat deletion and reopens from disk', async ({ page }, info) => {
  test.setTimeout(90_000);
  const host = 'http://127.0.0.1:4177';
  await page.route('**/api/**', async route => {
    const response = await route.fetch({ url: route.request().url().replace(/^.*\/api/, host) });
    await route.fulfill({ response });
  });
  await page.goto('/agent-presentation-visual.html?notes');
  const frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#result')).toBeVisible();
  await frame.locator('#evidence-count').evaluate((node: HTMLInputElement) => {
    node.value = '1'; node.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await frame.locator('#next-step').click();
  await page.getByRole('button', { name: '记一下', exact: true }).click();
  const editor = page.getByRole('region', { name: '笔记编辑' });
  await expect(editor.getByText(/版本 2/)).toBeVisible();
  await editor.getByRole('textbox').fill('少一处证据，召回率就不同。');
  await editor.getByRole('button', { name: '收起', exact: true }).click();
  await frame.locator('#evidence-count').evaluate((node: HTMLInputElement) => {
    node.value = '3'; node.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await page.getByRole('button', { name: '继续编辑笔记' }).click();
  await expect(editor.getByRole('textbox')).toHaveValue('少一处证据，召回率就不同。');
  // Fail only the note mutation: the editor must keep both text and frozen receipt.
  await page.route('**/api/memory/save', route => route.fulfill({ status: 500, contentType: 'application/json', body: JSON.stringify({ error_code: 'WRITE_FAILED', category: 'internal', message: '保存暂时失败' }) }), { times: 1 });
  await editor.getByRole('button', { name: '保存笔记', exact: true }).click();
  await expect(editor.getByRole('alert')).toContainText('保存暂时失败');
  await editor.getByRole('button', { name: '保存笔记', exact: true }).click();
  await expect(editor).not.toBeVisible();
  const records = await page.request.post(`${host}/memory/recall`, { data: { type: 'note' } }).then(r => r.json());
  const note = records[0];
  expect(note.note.association.receipt.reference.revision).toBe(2);
  const read = await page.request.post(`${host}/memory/presentation.read`, { data: { mem_id: note.mem_id, restore: true } }).then(r => r.json());
  expect(read.restored_state.values.page.count).toBe(1);
  expect(read.restored_state.visible_step).toBe('explain');
  const loadCount = await page.locator('iframe').getAttribute('data-load-count');
  await page.getByRole('button', { name: '阅读工具', exact: true }).click();
  await page.getByRole('tab', { name: '笔记', exact: true }).click();
  const card = page.locator(`[data-mem-id="${note.mem_id}"]`);
  await card.locator('summary').click();
  await expect(card.getByRole('region', { name: '笔记详情' })).toBeVisible();
  expect(await page.locator('iframe').getAttribute('data-load-count')).toBe(loadCount);
  await card.getByRole('button', { name: '打开演示', exact: true }).click();
  await expect(frame.locator('#result')).toHaveText('1'); // opening did not restore
  await page.getByRole('button', { name: '收起', exact: true }).click();
  await page.request.post(`${host}/version-three`);
  await page.getByRole('button', { name: '删除原聊天' }).click();
  await page.request.post(`${host}/reopen`);
  await page.reload();
  await page.getByRole('button', { name: '选择新对话' }).click();
  await page.getByRole('button', { name: '阅读工具', exact: true }).click();
  await page.getByRole('tab', { name: '笔记', exact: true }).click();
  await card.locator('summary').click();
  await card.getByRole('button', { name: '回到记录时', exact: true }).click();
  await expect(frame.locator('#result')).toHaveText('1/3');
  await expect(frame.locator('#step')).toHaveAttribute('data-presentation-step', 'explain');
  expect(await frame.locator('#retained-chart').evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth > 0)).toBe(true);
  await page.getByRole('button', { name: '文字说明与来源', exact: true }).click();
  await expect(page.getByText('文字说明与来源 · 版本 2')).toBeVisible();
  await page.getByRole('button', { name: '关闭文字说明与来源' }).click();
  await frame.locator('[data-source-ref="source-rp2"]').first().click();
  await expect(page.getByRole('button', { name: '在正文中查看' })).toBeVisible();
  await page.getByRole('button', { name: '在正文中查看' }).click();
  await expect(page.getByTestId('reader-status')).toHaveText('已在正文中打开来源');
  await frame.locator('#evidence-count').evaluate((node: HTMLInputElement) => {
    node.value = '3'; node.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await page.getByRole('textbox', { name: '围绕当前阅读内容提问', exact: true }).fill('为什么记录时只有一处？');
  await page.getByRole('textbox', { name: '围绕当前阅读内容提问', exact: true }).press('Control+Enter');
  await expect(page.getByTestId('follow-up-status')).toHaveText('笔记追问已完成');
  const requests = await page.request.get(`${host}/requests`).then(r => r.json());
  const message = requests.at(-1).filter((m: any) => m.role === 'User').at(-1).content;
  expect(message).toContain(note.note.association.receipt.saved_state_ref);
  expect(message).toContain('"count":1');
  expect(message).toContain('"visible_step":"explain"');
  await page.screenshot({ path: info.outputPath('rn2-restored.png') });
});
