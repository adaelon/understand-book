import { expect, test, type Page } from '@playwright/test';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';

const bridge = readFileSync(new URL('../src/presentation-bridge.js', import.meta.url), 'utf8');
const applyHtml = readFileSync(new URL('./fixtures/presentation-apply-state.html', import.meta.url), 'utf8');
const evidence = process.env.OM1_OUTPUT;
const readerHost = process.env.OM1_READER_HOST;

async function mountBridge(page: Page, html: string, restoredState: unknown = null) {
  await page.goto('about:blank');
  await page.evaluate(({ bridge, html, restoredState }) => {
    (window as any).events = [];
    window.addEventListener('message', event => (window as any).events.push(event.data));
    const frame = document.createElement('iframe');
    frame.setAttribute('sandbox', 'allow-scripts');
    frame.style.cssText = 'width:960px;height:720px';
    frame.srcdoc = `<script>(${bridge})(${JSON.stringify({ channel: 'om1', sources: [], initialState: {}, restoredState })})</script>${html}`;
    document.body.append(frame);
  }, { bridge, html, restoredState });
  await expect.poll(() => page.evaluate(() => (window as any).events.some((e: any) => e.kind === 'ready'))).toBe(true);
  return page.frameLocator('iframe');
}

let requestId = 0;
async function snapshot(page: Page) {
  const id = ++requestId;
  await page.locator('iframe').evaluate((node: HTMLIFrameElement, id) =>
    node.contentWindow!.postMessage({ channel: 'om1', kind: 'snapshot', request_id: id }, '*'), id);
  await expect.poll(() => page.evaluate(id => (window as any).events.some((e: any) => e.request_id === id && e.kind === 'state'), id)).toBe(true);
  return page.evaluate(id => (window as any).events.find((e: any) => e.request_id === id && e.kind === 'state').state, id);
}

test('OM1 pending input, cancellation, apply, reset and paused reopen use one calculation state', async ({ page }) => {
  await page.clock.install();
  let frame = await mountBridge(page, applyHtml);
  await frame.locator('#speed').fill('2');
  const pending = await snapshot(page);
  expect(pending.values.page).toMatchObject({ input_speed_b_m_s: 2, applied_speed_b_m_s: 1, time_s: 10 });
  expect(pending.observation.state).toMatchObject({ speed_b_m_s: 2, phase: 'awaiting_apply' });
  expect(pending.observation.rendered).toEqual({ speed_b_m_s: 1, time_s: 10, distance_a_m: 10, distance_b_m: 10 });
  expect(pending.observed_result).toContain('B 走 10.0 米');
  expect(await snapshot(page)).toEqual(pending); // Reading does not apply or change the scene.
  frame = await mountBridge(page, applyHtml, pending);
  await expect(frame.locator('#speed')).toHaveValue('2');
  await expect(frame.locator('#result')).toContainText('B 走 10.0 米');
  expect(await snapshot(page)).toEqual(pending);

  await frame.locator('#apply').evaluate(node => {
    (node as HTMLButtonElement).click();
    (document.getElementById('cancel') as HTMLButtonElement).click();
  });
  await page.clock.fastForward(1000);
  const cancelled = await snapshot(page);
  expect(cancelled.observation.state.phase).toBe('cancelled');
  expect(cancelled.observation.rendered).toEqual(pending.observation.rendered);
  await frame.locator('#apply').click();
  expect((await snapshot(page)).observation.state.phase).toBe('calculating');
  await page.clock.fastForward(1000);
  await expect(frame.locator('#phase')).toHaveText('结果已应用');
  const applied = await snapshot(page);
  expect(applied.observation.rendered.distance_b_m).toBe(20);
  expect(applied.values.page.applied_speed_b_m_s).toBe(2);
  expect(applied.observed_result).toContain('B 走 20.0 米');

  await frame.locator('#play').click();
  await page.clock.fastForward(450);
  await frame.locator('#pause').click();
  const paused = await snapshot(page);
  expect(paused.observation.state.playing).toBe(false);
  expect(paused.observation.state.time_s).toBeGreaterThan(0);
  expect(paused.observation.state.time_s).toBeLessThan(10);
  frame = await mountBridge(page, applyHtml, paused);
  await page.clock.fastForward(1000);
  expect(await snapshot(page)).toEqual(paused);
  await frame.locator('#reset').click();
  const reset = await snapshot(page);
  expect(reset.observation.state).toMatchObject({ speed_b_m_s: 1, time_s: 10, phase: 'ready', playing: false });
  expect(reset.observation.rendered.distance_b_m).toBe(10);
});

test('OM1 absent or failed readers retain current DOM and controls without stale semantics', async ({ page }) => {
  const frame = await mountBridge(page, applyHtml);
  expect((await snapshot(page)).observation.rendered.distance_b_m).toBe(10);
  await frame.locator('#speed').fill('2');
  await frame.locator('body').evaluate(() => {
    (window as any).presentation.registerStateReader(() => { throw new Error('calculation state unavailable'); });
  });
  const failed = await snapshot(page);
  expect(failed).not.toHaveProperty('observation');
  expect(failed.values.page).toBeNull();
  expect(failed.values.controls.find((c: any) => c.key === 'speed').value).toBe('2');
  expect(failed.observed_result).toContain('B 走 10.0 米');
  expect(await page.evaluate(() => (window as any).events.some((e: any) => e.kind === 'error'))).toBe(false);
  await frame.locator('body').evaluate(() => {
    (window as any).presentation.registerStateReader(() => ({ values: { input_speed_b_m_s: 2 } }));
  });
  const missing = await snapshot(page);
  expect(missing).not.toHaveProperty('observation');
  expect(missing.values.page.input_speed_b_m_s).toBe(2);
});

test('OM1 existing single-factor page projects immediate results without rendered or extra saves', async ({ page }) => {
  // Adapt the existing immutable example only in this acceptance document.
  const original = readFileSync(new URL('../../../skills/presentation/methods/compare-one-factor/v2/example/index.html', import.meta.url), 'utf8');
  const html = original + `<script>
    presentation.registerStateReader(() => {
      const { playing, ...values } = presentationScene.snapshot();
      const m = explanationDemo.inspect();
      return { values, visible_step: m.t + ' s', observation: {
        summary: '共同时间 ' + m.t + ' 秒，A 走 ' + m.a + ' 米，B 走 ' + m.b + ' 米。',
        state: { speed_b_m_s: m.speed, time_s: m.t, distance_a_m: m.a, distance_b_m: m.b, playing }
      }};
    });
  </script>`;
  const frame = await mountBridge(page, html);
  await frame.locator('body').evaluate(() => (window as any).presentationScene.seek({ semantic_state: 1 }));
  const first = await snapshot(page);
  expect(first.observation).not.toHaveProperty('rendered');
  expect(first.observation.state).toMatchObject({ speed_b_m_s: 2, time_s: 10, distance_a_m: 10, distance_b_m: 20 });
  await frame.locator('#speed').evaluate((node: HTMLInputElement) => {
    node.value = '3'; node.dispatchEvent(new Event('input', { bubbles: true }));
  });
  const resetTime = await snapshot(page);
  expect(resetTime.observation.state).toMatchObject({ speed_b_m_s: 3, time_s: 0, distance_b_m: 0 });
  await frame.locator('body').evaluate(() => (window as any).presentationScene.seek({ semantic_state: 1 }));
  const edited = await snapshot(page);
  expect(edited.observation.state.distance_b_m).toBe(30);
  expect(edited.observed_result).toContain('30');
  expect(await page.evaluate(() => (window as any).events.filter((e: any) => e.kind === 'state' && e.request_id === undefined))).toEqual([]);
  await mountBridge(page, html, edited);
  expect(await snapshot(page)).toEqual(edited);
});

test('OM1 real Reader saves and reopens pending inputs and sends the exact scene to the model adapter', async ({ page }) => {
  test.skip(!readerHost, 'OM1_READER_HOST points to the existing Rust presentation acceptance host');
  await page.setViewportSize({ width: 1280, height: 1100 });
  await page.route('**/api/**', async route => {
    const response = await route.fetch({ url: route.request().url().replace(/^.*\/api/, readerHost!) });
    await route.fulfill({ response });
  });
  const saves: { state: any; receipt: any }[] = [];
  page.on('response', async response => {
    if (response.url().endsWith('/presentation.state.save') && response.ok())
      saves.push({ state: response.request().postDataJSON().state, receipt: await response.json() });
  });
  await page.request.post(`${readerHost}/reset-scene`);
  await page.goto('/agent-presentation-visual.html');
  let frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#result')).toContainText('B 走 10.0 米');
  await frame.locator('#speed').fill('2');
  await frame.locator('#speed').dispatchEvent('change');
  await expect.poll(() => saves.length).toBe(1);
  const pending = saves[0];
  expect(pending.state.observation.rendered.distance_b_m).toBe(10);
  expect(pending.state.observation.state.speed_b_m_s).toBe(2);
  await page.request.post(`${readerHost}/reopen`);
  const selected = await page.request.post(`${readerHost}/agent/history/select`, { data: { session_id: pending.receipt.session_id } });
  expect(selected.ok()).toBe(true);
  await page.reload();
  frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#speed')).toHaveValue('2');
  await expect(frame.locator('#result')).toContainText('B 走 10.0 米');
  if (evidence) {
    mkdirSync(evidence, { recursive: true });
    await page.screenshot({ path: join(evidence, 'pending-reopened.png'), fullPage: true });
  }
  await page.getByRole('textbox', { name: '针对当前演示追问' }).fill('为什么没有变');
  await page.getByRole('button', { name: '发送追问', exact: true }).click();
  await expect(page.getByTestId('follow-up-status')).toHaveText('追问已完成');
  const requests = await page.request.get(`${readerHost}/requests`).then(r => r.json());
  const user = requests.at(-1).filter((message: any) => message.role === 'User').at(-1).content;
  const packet = JSON.parse(user.split('\n').find((line: string) => line.startsWith('{') && line.includes('"content_access":'))!);
  expect(packet.state.observation).toEqual(pending.state.observation);
  const asked = saves.at(-1)!;
  expect(user).toContain(asked.receipt.saved_state_ref);
  expect(user).toContain('B 走 10.0 米');
  await frame.locator('#apply').click();
  await expect(frame.locator('#result')).toContainText('B 走 20.0 米');
  await expect.poll(() => saves.at(-1)?.state.observation.rendered.distance_b_m).toBe(20);
  const applied = saves.at(-1)!;
  // Request the original receipt after a newer state exists, through the real read route.
  const exact = await page.request.post(`${readerHost}/agent/presentation.read`, { data: {
    session_id: asked.receipt.session_id, turn_id: asked.receipt.turn_id,
    reference: asked.receipt.reference, saved_state: asked.receipt,
  } }).then(r => r.json());
  expect(exact.restored_state.observation.rendered.distance_b_m).toBe(10);
  expect(exact.restored_state.values.page.input_speed_b_m_s).toBe(2);
  if (evidence) {
    await page.screenshot({ path: join(evidence, 'applied.png'), fullPage: true });
    writeFileSync(join(resolve(evidence), 'reader-evidence.json'), JSON.stringify({
      pending, asked, applied, original_restored_state: exact.restored_state, model_input: user,
      adapter: 'ChatRecordingAdapter; no external model inference',
    }, null, 2));
  }
});
