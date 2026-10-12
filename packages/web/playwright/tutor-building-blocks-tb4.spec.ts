import { expect, test } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const host = 'http://127.0.0.1:4185';
const evidence = resolve('../../docs/performance/tutor-building-blocks-tb4/browser');

test('TB4 actual code comparison restores its state and continues the original Tutor chat', async ({ page }) => {
  mkdirSync(evidence, { recursive: true });
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.route('**/api/**', async route => {
    const response = await route.fetch({ url: route.request().url().replace(/^.*\/api/, host), timeout: 1_000_000 });
    await route.fulfill({ response });
  });
  const fixture = await page.request.get(`${host}/fixture`).then(r => r.json());
  await page.goto('/agent-presentation-visual.html');
  const frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#leftResult')).toHaveText('结果未查看。');
  await expect(frame.locator('#rightResult')).toHaveText('结果未查看。');
  await frame.locator('#leftObserve').click();
  await expect(frame.locator('#leftResult')).toContainText('E0594');
  await frame.locator('#rightCopy').selectOption('false');
  await frame.locator('#rightObserve').click();
  await expect(frame.locator('#rightResult')).toContainText('E0594');
  await frame.locator('#rightMutable').selectOption('true');
  await expect(frame.locator('#rightResult')).toHaveText('结果未查看。');
  await expect(frame.locator('#rightCode')).toContainText('let b = &mut a;');
  await frame.locator('#rightObserve').click();
  await expect(frame.locator('#rightResult')).toContainText('b = Foo(8)\na = Foo(8)');
  const readView = () => page.request.post(`${host}/agent/presentation.read`, { data: fixture }).then(r => r.json());
  await expect.poll(async () => (await readView()).restored_state?.values?.page?.rightMutable).toBe(true);
  await page.request.post(`${host}/reopen`);
  await page.reload();
  await expect(frame.locator('#rightMutable')).toHaveValue('true');
  await expect(frame.locator('#rightCopy')).toHaveValue('false');
  await expect(frame.locator('#rightResult')).toContainText('b = Foo(8)\na = Foo(8)');
  await page.setViewportSize({ width: 390, height: 700 });
  await frame.locator('#rightObserve').scrollIntoViewIfNeeded();
  await page.screenshot({ path: resolve(evidence, 'mobile-restored.png') });
  // Finish on two shared-reference conditions. The statement below describes
  // this authored test scenario, not an observation of a real learner.
  await frame.locator('#rightMutable').selectOption('false');
  await expect(frame.locator('#rightResult')).toHaveText('结果未查看。');
  await frame.locator('#rightObserve').click();
  await expect(frame.locator('#leftResult')).toContainText('E0594');
  await expect(frame.locator('#rightResult')).toContainText('E0594');
  await expect.poll(async () => (await readView()).restored_state?.values?.page?.rightMutable).toBe(false);
  await expect.poll(async () => (await readView()).restored_state?.values?.page?.rightRevealed).toBe(true);
  const view = await readView();
  writeFileSync(resolve(evidence, 'before-followup-view.json'), JSON.stringify(view, null, 2));
  await page.setViewportSize({ width: 1200, height: 900 });
  await page.getByRole('button', { name: '展开', exact: true }).click();
  const response = '我现在选的 A 是 Copy 加 &a，B 是没有 Copy 加 &a，两组都显示 E0594。我还试过 B 的 &mut a，看到 Foo(8)。这些结果是教具给的；我推断 Copy 本身没有让共享引用能写入。下一步让我自己选一个对照说明理由，先别讲完整结论。';
  writeFileSync(resolve(evidence, 'scripted-response.json'), JSON.stringify({ identity:'authored test fixture; not a human reader', response }, null, 2));
  await page.locator('.agent-input textarea').fill(response);
  await page.locator('.agent-input').getByRole('button', { name: '发送', exact: true }).click();
  await expect(page.getByTestId('follow-up-status')).toHaveText('追问已完成', { timeout: 1_000_000 });
  await expect(page.locator('.presentation-workspace .turn')).toHaveCount(2);
  await expect(page.locator('.presentation-workspace .turn').last().locator('.a-msg')).toBeVisible();
  const sources = frame.locator('button[data-source-ref]');
  await expect(sources).toHaveCount(2);
  for (const source of await sources.all()) {
    await expect(source).toBeEnabled();
    expect(await source.textContent()).toBeTruthy();
    const colors = await source.evaluate(node => {
      const style = getComputedStyle(node);
      return { foreground: style.color, background: style.backgroundColor };
    });
    expect(colors.foreground).not.toBe(colors.background);
  }
  await sources.first().scrollIntoViewIfNeeded();
  await page.screenshot({ path: resolve(evidence, 'followup-desktop.png') });
  await sources.first().click();
  await expect(page.getByRole('dialog', { name: '回答来源' })).toBeVisible();
  await page.getByRole('button', { name: '在正文中查看', exact: true }).click();
  await expect(page.getByTestId('reader-status')).toHaveText('已在正文中打开来源');
  expect(errors).toEqual([]);
  writeFileSync(resolve(evidence, 'checks.json'), JSON.stringify({
    reference: fixture.reference, session_id: fixture.session_id, errors,
    initial_results_hidden:true, control_change_hides_old_result:true,
    shared_copy_and_noncopy:'E0594', mutable_noncopy:'b = Foo(8); a = Foo(8)',
    reloaded_conditions_and_result:true, original_chat_continued:true,
    source_labels_readable:true, source_opened:true,
    learner_identity:'scripted', external_provider_calls:0,
  }, null, 2));
});
