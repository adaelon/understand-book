import { expect, test } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const host = process.env.OM4_READER_HOST;
test('OM4 real Reader observes, changes, applies and persists one version; cancelled results never replay', async ({ page }) => {
  test.skip(!host, 'Run om4_browser_host and point the Vite proxy to it');
  await page.setViewportSize({ width: 1280, height: 1000 });
  await page.goto('/agent-presentation-visual.html');
  const fixture = await page.request.get(`${host}/fixture`).then(r => r.json());
  let frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#result')).toContainText('B 走 10.0 米');
  const results: any[] = [];
  const command = async (request: any) => {
    const response = await page.request.post(`${host}/om4/command`, { data: { reference: fixture.reference, ...request } });
    expect(response.ok()).toBe(true);
    let result: any;
    await expect.poll(async () => {
      result = await page.request.get(`${host}/om4/result`).then(r => r.json());
      return result !== null;
    }).toBe(true);
    results.push(result);
    return result;
  };
  const observe = () => command({ operation: 'observe' });
  const act = (basis: any, action: any) => command({ operation: 'act', based_on: basis.saved_state_ref, action });
  let current = await observe();
  expect(current.status).toBe('observed');
  expect(current.available_actions).toHaveLength(6);
  current = await act(current, { kind: 'set_parameters', values: { input_speed_b_m_s: 2 } });
  expect(current.status).toBe('applied');
  expect(current.state.observation.state.speed_b_m_s).toBe(2);
  expect(current.state.observation.rendered.distance_b_m).toBe(10);
  await expect(frame.locator('#speed')).toHaveValue('2');
  current = await act(current, { kind: 'invoke', action_id: 'apply' });
  expect(current.status).toBe('applied');
  expect(current.state.observation.rendered.distance_b_m).toBe(20);
  await expect(frame.locator('#result')).toContainText('B 走 20.0 米');
  const applied = current;
  const rejected = await act(current, { kind: 'set_parameters', values: { input_speed_b_m_s: 4 } });
  expect(rejected.status).toBe('failed');
  expect(rejected.state.values.page.input_speed_b_m_s).toBe(2);
  expect(rejected.receipt.reference).toEqual(fixture.reference);
  for (const [kind, region_id] of [['highlight', 'result'], ['reveal', 'relation'], ['annotate', 'annotation']]) {
    current = await act(current, { kind, region_id });
    expect(current.status).toBe('applied');
    await expect(frame.locator(`#${region_id}`)).toBeVisible();
  }
  await frame.locator('#speed').fill('3');
  current = await act(current, { kind: 'set_parameters', values: { input_speed_b_m_s: 1 } });
  expect(current.status).toBe('conflict');
  expect(current.state.values.page.input_speed_b_m_s).toBe(3);
  await expect(frame.locator('#speed')).toHaveValue('3');
  current = await act(current, { kind: 'invoke', action_id: 'apply' });
  expect(current.state.observation.rendered.distance_b_m).toBe(30);
  const oldPage = current;
  await page.reload();
  frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#result')).toContainText('B 走 30.0 米');
  await expect(frame.locator('#relation')).toBeVisible();
  await expect(frame.locator('#annotation')).toBeVisible();
  const unavailable = await act(oldPage, { kind: 'invoke', action_id: 'reset' });
  expect(unavailable.status).toBe('unavailable');
  current = await observe();
  current = await act(current, { kind: 'set_parameters', values: { input_speed_b_m_s: 2 } });
  let release!: () => void;
  const held = new Promise<void>(resolve => { release = resolve; });
  let awaitingReceipt = false;
  await page.route('**/api/agent/presentation.interaction', async route => {
    if (route.request().postDataJSON().operation === 'complete') {
      awaitingReceipt = true;
      await held;
    }
    await route.continue();
  });
  const pending = act(current, { kind: 'invoke', action_id: 'apply' });
  await expect.poll(() => awaitingReceipt).toBe(true);
  await expect(frame.locator('#result')).toContainText('B 走 20.0 米');
  const cancelled = await page.request.post(`${host}/agent/runs/${fixture.run_descriptor.turn_id}/cancel`, { data: {} });
  expect(cancelled.ok()).toBe(true);
  const ended = await pending;
  expect(ended.error_code).toBe('AGENT_RUN_CANCELLED');
  release();
  await page.unrouteAll({ behavior: 'wait' });
  await page.reload();
  frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#result')).toContainText('B 走 20.0 米');
  const original = await page.request.post(`${host}/agent/presentation.read`, { data: {
    session_id: fixture.session_id, turn_id: fixture.turn_id, reference: fixture.reference, saved_state: applied.receipt,
  } }).then(r => r.json());
  expect(original.reference).toEqual(fixture.reference);
  expect(original.restored_state).toEqual(applied.state);
  for (const result of results.filter(r => r.receipt)) expect(result.receipt.reference).toEqual(fixture.reference);
  if (process.env.OM4_OUTPUT) {
    mkdirSync(process.env.OM4_OUTPUT, { recursive: true });
    writeFileSync(join(process.env.OM4_OUTPUT, 'reader-evidence.json'), JSON.stringify({ results, original }, null, 2));
    await page.screenshot({ path: join(process.env.OM4_OUTPUT, 'reader-applied.png'), fullPage: true });
  }
});
