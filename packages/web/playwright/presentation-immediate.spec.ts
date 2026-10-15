import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';

// Exact delivered HTML from the 2026-10-10 incident; host assets remain the current implementation.
const html = readFileSync(new URL('./fixtures/presentation-bandwidth.html', import.meta.url), 'utf8');
const sources = [...new Set([...html.matchAll(/data-source-ref="([^"]+)"/g)].map(match => match[1]))]
  .map(source_ref_id => ({ source_ref_id, label: '演示来源' }));
const reference = { presentation_id: 'p', revision: 2 };
const scene = { workspace_id: 'immediate', generation: 1, revision: 1, selected_chat: 's',
  published_book_ref: { book_id: 'book', publication_id: 'publication' }, reader: {} };

test('delivered sliders and buttons remain immediate when saving fails or is delayed', async ({ page, context }, info) => {
  const requests: string[] = [];
  const snapshots: any[] = [];
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  let failSave = true;
  let release: (() => void) | undefined;
  let delaySave = false;
  await page.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname;
    requests.push(path);
    let result: unknown = {};
    if (path.endsWith('/presentation/read')) result = {
      reference, title: '带宽演示', entrypoint: 'index.html', content_files: { 'index.html': html }, sources,
      initial_state: {}, restored_state: null, restored_state_revision: null,
      readable_view: { parts: [], sources }, animation_assets: {}, assumptions: [],
    };
    if (path.endsWith('/presentation/observe')) {
      await route.abort('failed'); return;
    }
    if (path.endsWith('/presentation/save')) {
      snapshots.push(route.request().postDataJSON().state);
      if (delaySave) await new Promise<void>(resolve => { release = resolve; });
      if (failSave) { await route.abort('failed'); return; }
      result = { session_id: 's', turn_id: 't', reference, state_revision: snapshots.length, saved_state_ref: 'saved-scene' };
    }
    await route.fulfill({ json: { ...scene, result } });
  });
  try {
    await page.goto('/playwright/fixtures/presentation-immediate-host.html');
    const frame = page.frameLocator('iframe');
    await expect(frame.locator('#oTr')).toBeVisible();
    await expect(frame.locator('#oTr')).toHaveText('20.90');
    await expect(page.getByText('正在准备内容…')).toHaveCount(0);
    await context.setOffline(true);
    await frame.locator('#sN').focus();
    for (let n = 75; n <= 170; n += 5) {
      await page.keyboard.press('ArrowRight');
      await expect(frame.locator('#oTr')).toHaveText((n / 3.35).toFixed(2));
      await expect(frame.locator('#oTr')).toHaveCSS('opacity', '1');
      await expect(frame.locator('#sN')).toBeFocused();
    }
    await expect(frame.locator('#verdict')).toContainText('50.75 ms');
    await expect(frame.locator('[data-presentation-mask]')).toHaveCount(0);
    await expect(frame.locator('#barComp')).toHaveAttribute('style', /width:/);
    await frame.locator('#oTr').scrollIntoViewIfNeeded();
    await page.screenshot({ path: info.outputPath('offline-170.png') });
    await frame.locator('#sN').scrollIntoViewIfNeeded();
    const slider = (await frame.locator('#sN').boundingBox())!;
    await page.mouse.move(slider.x + slider.width / 2, slider.y + slider.height / 2);
    await page.mouse.down();
    await page.mouse.move(slider.x + slider.width * .9, slider.y + slider.height / 2, { steps: 12 });
    await page.mouse.up();
    const dragged = Number(await frame.locator('#sN').inputValue());
    expect(dragged).toBeGreaterThan(170);
    await expect(frame.locator('#oTr')).toHaveText((dragged / 3.35).toFixed(2));
    await expect(frame.locator('#oTr')).toHaveCSS('opacity', '1');
    await frame.locator('[data-preset="half"]').click();
    await expect(frame.locator('#oTr')).toHaveText('10.45');
    await expect(page.locator('.presentation-save-notice')).toContainText('现场保存失败');
    await expect(page.locator('iframe')).toHaveAttribute('data-load-count', '1');
    await context.setOffline(false);
    // Let the failed automatic saves finish, then freeze one explicit follow-up snapshot.
    await page.getByRole('button', { name: '解释现在的结果', exact: true }).click();
    await expect(page.locator('.presentation-save-notice')).toContainText('追问未发送');
    expect(await page.evaluate(() => (window as any).followUps)).toEqual([]);
    failSave = false; delaySave = true;
    await page.getByRole('button', { name: '解释现在的结果', exact: true }).click();
    await expect.poll(() => !!release).toBe(true);
    const captured = snapshots.at(-1);
    expect(captured.values.controls.find((control: any) => control.key === 'sBw').value).toBe('0.5');
    expect(captured.observed_result).toContain('10.45');
    const beforeBar = await frame.locator('#barComp').getAttribute('style');
    await frame.locator('#sPi').evaluate((node: HTMLInputElement) => {
      node.value = '100'; node.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await expect(frame.locator('#oTc')).toHaveText('1.40');
    expect(await frame.locator('#barComp').getAttribute('style')).not.toBe(beforeBar);
    release!(); delaySave = false;
    await expect.poll(() => page.evaluate(() => (window as any).followUps.length)).toBe(1);
    expect(captured.values.controls.find((control: any) => control.key === 'sPi').value).toBe('989.4');
    expect(requests.filter(path => path.endsWith('/presentation/observe'))).toEqual([]);
    expect(errors).toEqual([]);
    await frame.locator('#oTr').scrollIntoViewIfNeeded();
    await page.screenshot({ path: info.outputPath('immediate-result.png') });
  } finally {
    release?.();
    await context.setOffline(false);
    await page.unrouteAll({ behavior: 'ignoreErrors' });
  }
});
