import { expect, test } from '@playwright/test';

for (const size of [{ width: 390, height: 669 }, { width: 390, height: 844 }, { width: 320, height: 568 }]) {
  test(`mobile reading, drafts and response at ${size.width}x${size.height}`, async ({ page }, info) => {
    await page.setViewportSize(size);
    await page.goto('/mobile-learning-preview.html?embedded=1');
    const transcript = page.locator('.transcript');
    const composer = page.locator('.agent-input textarea');
    const respond = page.getByRole('button', { name: '回答问题', exact: true });
    await expect(respond).toBeVisible();
    await transcript.evaluate(node => { node.scrollTop = 0; });
    await expect(composer).toBeInViewport();
    await expect(page.locator('.agent-compose-row').getByRole('button', { name: '发送', exact: true })).toBeVisible();
    const reading = (await transcript.boundingBox())!;
    expect(reading.height).toBeGreaterThan(size.height * (size.width < 360 ? .70 : .75));
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(size.width);
    await info.attach('reading-space', { body: JSON.stringify({ viewport: size, reading, ratio: reading.height / size.height }), contentType: 'application/json' });
    await page.screenshot({ path: info.outputPath('reading.png') });

    await transcript.evaluate(node => { node.scrollTop = 100; });
    await page.getByRole('button', { name: '学习与会话', exact: true }).click();
    const sheet = page.getByRole('dialog', { name: '学习与会话', exact: true });
    await expect(sheet.getByRole('switch', { name: '连续教学' })).toBeVisible();
    await page.screenshot({ path: info.outputPath('learning-panel.png') });
    await sheet.getByRole('button', { name: '调整目标', exact: true }).click();
    await page.getByRole('textbox', { name: '学习目标' }).fill('把需求、资源与工作去向连起来');
    await page.getByRole('button', { name: '保存目标' }).click();
    await expect(page.locator('.mobile-learning-title')).toContainText('把需求、资源与工作去向连起来');
    await sheet.getByRole('button', { name: '关闭学习与会话' }).click();
    expect(await transcript.evaluate(node => node.scrollTop)).toBe(100);

    await composer.fill('哪些网络工作适合交给网卡处理？');
    await page.screenshot({ path: info.outputPath('asking.png') });
    await composer.press('Escape');
    await expect(composer).toHaveValue('哪些网络工作适合交给网卡处理？');
    await expect(composer).toBeVisible();

    await respond.click();
    const answer = page.getByRole('textbox', { name: '活动回答', exact: true });
    await expect(answer).toBeFocused();
    await expect(answer).toBeInViewport();
    await answer.fill('网络处理占用 CPU，把适合的工作交给网卡，就能把资源留给应用。');
    await expect(page.locator('.mobile-response-toolbar').getByRole('button', { name: '提交回答', exact: true })).toBeInViewport();
    await expect(composer).not.toBeVisible();
    await page.screenshot({ path: info.outputPath('answering.png') });
    await page.getByRole('button', { name: '收起作答' }).click();
    await respond.click();
    await expect(answer).toHaveValue('网络处理占用 CPU，把适合的工作交给网卡，就能把资源留给应用。');
    await page.locator('.mobile-response-toolbar').getByRole('button', { name: '提交回答', exact: true }).click();
    await page.getByRole('button', { name: '我的理解', exact: true }).click();
    await expect(page.getByRole('dialog', { name: '我的理解', exact: true })).toContainText('网络处理占用 CPU');
    await page.getByRole('button', { name: '关闭我的理解', exact: true }).click();
    await expect(composer).toHaveValue('哪些网络工作适合交给网卡处理？');
    await expect(composer).toBeVisible();
    const beforeReturn = await transcript.evaluate(node => node.scrollTop);
    await page.getByRole('button', { name: '返回阅读', exact: true }).click();
    await expect(page.locator('.reader-pane')).toBeVisible();
    await page.getByRole('button', { name: '继续问这本书 →' }).click();
    expect(await transcript.evaluate(node => node.scrollTop)).toBe(beforeReturn);
    await page.locator('.agent-compose-row').getByRole('button', { name: '发送', exact: true }).click();
    await expect(page.locator('.turn').last()).toContainText('哪些网络工作适合交给网卡处理？');
    await expect(composer).toBeVisible();
    await expect(composer).toHaveValue('');
  });
}

test('response editor and submit follow the visible viewport when the keyboard opens', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 669 });
  await page.goto('/mobile-learning-preview.html?embedded=1');
  await page.getByRole('button', { name: '回答问题', exact: true }).click();
  await page.evaluate(() => {
    Object.defineProperty(window.visualViewport, 'height', { configurable: true, value: 350 });
    Object.defineProperty(window.visualViewport, 'offsetTop', { configurable: true, value: 0 });
    window.visualViewport!.dispatchEvent(new Event('resize'));
  });
  await expect(page.locator('html')).toHaveAttribute('data-workspace-keyboard', 'true');
  await expect.poll(async () => {
    const submit = await page.locator('.mobile-response-toolbar').getByRole('button', { name: '提交回答', exact: true }).boundingBox();
    return submit!.y + submit!.height;
  }).toBeLessThanOrEqual(350);
  const response = (await page.getByRole('textbox', { name: '活动回答', exact: true }).boundingBox())!;
  expect(response.y).toBeGreaterThanOrEqual(56);
  expect(response.y + response.height).toBeLessThanOrEqual(290);
});
