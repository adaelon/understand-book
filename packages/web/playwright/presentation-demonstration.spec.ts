import { expect, test } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const host = process.env.OM5_READER_HOST;
test('OM5 real Reader records demonstrated help, learner takeover, cancellation and reopened support', async ({ page }) => {
  test.skip(!host, 'Start om5_browser_host and point the Vite proxy to it');
  test.setTimeout(120_000);
  await page.setViewportSize({ width: 1280, height: 1000 });
  page.on('response', async response => {
    if (response.url().endsWith('/presentation.state.save') && !response.ok()) {
      const failed = { request: response.request().postDataJSON(), status: response.status(), body: await response.text() };
      if (process.env.OM5_OUTPUT) writeFileSync(join(process.env.OM5_OUTPUT, 'failed-save.json'), JSON.stringify(failed, null, 2));
    }
  });
  await page.goto('/agent-presentation-visual.html');
  let fixture = await page.request.get(`${host}/fixture`).then(r => r.json());
  let frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#reading')).toContainText('B 走 10 米');
  const results: any[] = [];
  const evidence = () => page.request.get(`${host}/om5/evidence`).then(r => r.json());
  const facts = (data: any) => data.events.map((row: any) => row[1]);
  const help = (data: any) => facts(data).filter((event: any) => event.kind === 'help_displayed');
  const command = async (request: any) => {
    expect((await page.request.post(`${host}/om5/command`, { data: { reference: fixture.reference, ...request } })).ok()).toBe(true);
    let result: any;
    await expect.poll(async () => {
      result = await page.request.get(`${host}/om5/result`).then(r => r.json());
      return result !== null;
    }).toBe(true);
    results.push(result);
    if (request.operation === 'observe') expect(result, JSON.stringify(result)).toHaveProperty('status', 'observed');
    return result;
  };
  const act = (basis: any, action: any) => command({ operation: 'act', based_on: basis.saved_state_ref, action });
  const respond = async (message: string, replyCount: number) => {
    await page.getByRole('textbox', { name: '针对当前演示追问' }).fill(message);
    await page.getByRole('button', { name: '发送追问', exact: true }).click();
    await expect.poll(async () => (await evidence()).replies.length).toBe(replyCount);
  };
  await frame.locator('#speed').fill('2');
  let current = await command({ operation: 'observe' });
  expect(current.state.observation.state.speed_b_m_s).toBe(2);
  expect(current.state.observation.rendered.distance_b_m).toBe(10);
  current = await act(current, { kind: 'highlight', region_id: 'result' });
  expect(help(await evidence())).toHaveLength(0);
  current = await act(current, { kind: 'invoke', action_id: 'apply' });
  await expect(frame.locator('#reading')).toContainText('B 走 20 米');
  expect(current.teaching.assistance_refs).toHaveLength(1);
  const applied = current;
  current = await act(current, { kind: 'reveal', region_id: 'relation' });
  await expect(frame.locator('#relation')).toBeVisible();
  const revealed = current;
  let stored = await evidence();
  expect(help(stored)).toHaveLength(2);
  expect(help(stored).map((event: any) => event.payload.action)).toEqual(['demonstration', 'reveal']);
  const initialHelp = help(stored).map((event: any) => event.event_id);
  await respond('速度翻倍，所以同一时间距离翻倍。', 1);
  stored = await evidence();
  const response = facts(stored).find((event: any) => event.payload.response === '速度翻倍，所以同一时间距离翻倍。');
  expect(response.payload.assistance_refs).toEqual(initialHelp);
  expect(response.payload.presentation_follow_up.reference).toEqual(fixture.reference);
  expect(facts(stored).filter((event: any) => event.kind === 'learner_action')).toHaveLength(0);
  // Independent controls do not invent teacher actions or remove prior help.
  await frame.locator('#speed').fill('3');
  await frame.locator('#apply').click();
  await expect(frame.locator('#reading')).toContainText('B 走 30 米');
  await respond('我自己改成 3 米/秒并应用，同样 10 秒走 30 米。', 2);
  stored = await evidence();
  expect(help(stored)).toHaveLength(2);
  const independent = facts(stored).find((event: any) => event.payload.response?.startsWith('我自己改成'));
  expect(independent.payload.assistance_refs).toEqual(initialHelp);
  // Save a newly displayed demonstration, hold its final tool acknowledgement,
  // then cancel. The help must survive; a later help cannot change either reply.
  expect((await page.request.post(`${host}/om5/start`)).ok()).toBe(true);
  await page.reload();
  fixture = await page.request.get(`${host}/fixture`).then(r => r.json());
  frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#reading')).toContainText('B 走 30 米');
  current = await command({ operation: 'observe' });
  current = await act(current, { kind: 'set_parameters', values: { input_speed: 1 } });
  expect(current.teaching.assistance_refs).toEqual([]);
  let release!: () => void;
  const held = new Promise<void>(resolve => { release = resolve; });
  let waiting = false;
  await page.route('**/api/agent/presentation.interaction', async route => {
    if (route.request().postDataJSON().operation === 'complete') { waiting = true; await held; }
    await route.continue();
  });
  const cancelledAction = act(current, { kind: 'invoke', action_id: 'apply' });
  await expect.poll(() => waiting).toBe(true);
  await expect(frame.locator('#reading')).toContainText('B 走 10 米');
  expect(help(await evidence())).toHaveLength(3);
  expect((await page.request.post(`${host}/agent/runs/${fixture.run_descriptor.turn_id}/cancel`)).ok()).toBe(true);
  expect((await cancelledAction).error_code).toBe('AGENT_RUN_CANCELLED');
  release(); await page.unrouteAll({ behavior: 'wait' });
  expect((await page.request.post(`${host}/om5/reopen`)).ok()).toBe(true);
  await page.reload();
  frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#reading')).toContainText('B 走 10 米');
  await expect(frame.locator('#relation')).toBeVisible();
  await respond('这次直接讲公式。', 3);
  stored = await evidence();
  expect(help(stored)).toHaveLength(3);
  for (const original of [response, independent]) {
    const again = facts(stored).find((event: any) => event.event_id === original.event_id);
    expect(again).toEqual(original);
  }
  const resumed = stored.replies.at(-1);
  expect(resumed.outcome.answer).toContain('d = v × t');
  expect(resumed.selection.move.interaction_intent).toBe('direct_explanation');
  expect(resumed.selection.move.response_expectation).toBeUndefined();
  expect(JSON.stringify(resumed.samples)).toContain(response.event_id);
  expect(JSON.stringify(resumed.samples)).toContain(initialHelp[0]);
  expect(JSON.stringify(resumed.samples)).toContain('这次直接讲公式');
  const old = await page.request.post(`${host}/agent/presentation.read`, { data: {
    session_id: fixture.session_id, turn_id: fixture.turn_id, reference: fixture.reference, saved_state: applied.receipt,
  } }).then(r => r.json());
  expect(old.restored_state.observation.rendered.distance_b_m).toBe(20);
  expect(revealed.receipt.reference.revision).toBe(1);
  if (process.env.OM5_OUTPUT) {
    mkdirSync(process.env.OM5_OUTPUT, { recursive: true });
    writeFileSync(join(process.env.OM5_OUTPUT, 'reader-evidence.json'), JSON.stringify({ results, stored, original: old,
      adapter: 'TutorRequestRecorder with deterministic ChatStubAdapter; no external model calls' }, null, 2));
    await page.screenshot({ path: join(process.env.OM5_OUTPUT, 'reader-resumed.png'), fullPage: true });
  }
});
