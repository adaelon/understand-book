import { test } from './focus-test';
import { expect } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { backend } from './focus-app-backend';

test('measurement starts at a public turn input, not an unpressed mouse move', async ({ page }) => {
  await backend(page, 'epub');
  await page.goto(process.env.FOCUS_ENTRY_URL!);
  await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().click();
  await expect.poll(() => page.evaluate(() => {
    const s = (window as any).fp4.snapshot(); return s?.phase === 'ready' && !s.preparing;
  })).toBe(true);
  await page.evaluate(() => (window as any).fp4.arm('hover-before-turn'));
  await page.locator('.focus-reader-surface').hover();
  expect(await page.evaluate(() => (window as any).fp4.samples())).toHaveLength(0);
  await page.getByRole('button', { name: '下一屏', exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as any).fp4.samples()[0]?.end)).toBeTruthy();
  const samples = await page.evaluate(() => (window as any).fp4.samples());
  expect(samples).toHaveLength(1); expect(samples[0].inputType).toBe('click');
});

test('production EPUB keeps reading while a background answer completes', async ({ page, browser }) => {
  const server = await backend(page, 'epub');
  const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
  await page.addInitScript(() => {
    const host = window as any; host.fp7Events = [];
    host.EventSource = class extends EventTarget {
      onopen?: () => void; closed = false;
      constructor(public url: string) { super(); host.fp7Events.push(this); setTimeout(() => this.onopen?.(), 0); }
      close() { this.closed = true; }
    };
  });
  const descriptor = { book_id: 'epub', session_id: 'fp3-chat', turn_id: 'fp7-background' };
  const running = { descriptor, last_seq: 0, execution_state: 'waiting_model', persistence_state: 'pending', activities: [], final_view: null, error: null };
  await page.route('**/api/agent/runs/fp7-background', route => route.fulfill({ json: running }));
  let question: any, completed: any;
  await page.route('**/api/agent/history', route => route.fulfill({ json: {
    active_session_id: 'fp3-chat', sessions: [], current: { id: 'fp3-chat', book_id: 'epub', goals: [],
      turns: completed ? [completed] : question ? [{ turn_id: descriptor.turn_id, user: '解释当前原文。', user_turn_ordinal: 0, status: 'pending_assistant', outcome: null, effect_labels: [] }] : [] },
  } }));
  await page.route('**/api/agent/runs', route => {
    question = route.request().postDataJSON();
    return route.fulfill({ json: { ...descriptor, dispatch_state: 'claimed', persistence_state: 'pending', snapshot: running } });
  });
  await page.goto(process.env.FOCUS_ENTRY_URL ?? '/playwright/fixtures/focus-turn.html');
  await expect(page.locator('.reader-pane .flow-paragraph').first()).toBeVisible();
  await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().click();
  const snapshot = () => page.evaluate(() => (window as any).fp4.snapshot());
  const settled = () => expect.poll(async () => { const s = await snapshot(); return s?.phase === 'ready' && !s.preparing && s.turn.phase === 'idle'; }).toBe(true);
  await settled();
  const initial = await snapshot();
  await page.locator('.focus-tools-toggle').click();
  await page.getByRole('navigation', { name: '专注阅读工具' }).getByRole('button', { name: '问答', exact: true }).click();
  await page.getByRole('textbox', { name: '围绕当前阅读内容提问' }).fill('解释当前原文。');
  await page.getByRole('button', { name: '发送', exact: true }).click();
  await expect.poll(() => question).toBeTruthy();
  await expect.poll(() => page.evaluate(() => (window as any).fp7Events.some((s: any) => !s.closed))).toBe(true);
  await page.getByRole('navigation', { name: '阅读工作区', exact: true }).getByRole('button', { name: '阅读', exact: true }).click();
  await settled();
  expect((await snapshot()).current.start).toEqual(initial.current.start);
  for (let i = 0; i < 8; i++) { await page.getByRole('button', { name: '下一屏', exact: true }).click(); await settled(); }
  const before = await snapshot();
  const final = { ...running, last_seq: 1, execution_state: 'completed', persistence_state: 'saved', final_view: {
    turn_id: descriptor.turn_id, user: '解释当前原文。', user_turn_ordinal: 0, status: 'completed',
    outcome: { answer: '后台回答已完成，阅读仍在原位。', answer_view: { parts: [{ kind: 'markdown', text: '后台回答已完成，阅读仍在原位。' }], sources: [] },
      effects: [], trace: [], incomplete: false, warning: null, turns: 1, tokens_spent: 1, memory_updates: [],
      profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] } },
    question_source_label: null, question_quote: null, effect_labels: [],
  } };
  completed = final.final_view;
  await page.evaluate(value => {
    for (const source of (window as any).fp7Events.filter((s: any) => !s.closed)) source.dispatchEvent(new MessageEvent('run.completed', {
      data: JSON.stringify({ turn_id: value.descriptor.turn_id, seq: 1, elapsed_ms: 1000, type: 'run.completed', payload: value }), lastEventId: '1',
    }));
  }, final);
  await page.waitForTimeout(150); expect((await snapshot()).current.start).toEqual(before.current.start);
  await page.getByRole('button', { name: '下一屏', exact: true }).click(); await settled();
  const after = await snapshot(); expect(after.current.start).toEqual(before.current.end);
  await page.locator('.focus-tools-toggle').click();
  await page.getByRole('navigation', { name: '专注阅读工具' }).getByRole('button', { name: '问答', exact: true }).click();
  await expect(page.getByText('后台回答已完成，阅读仍在原位。', { exact: true })).toBeVisible();
  expect(errors).toEqual([]);
  const output = path.resolve(process.env.FOCUS_EVIDENCE_DIR!); await mkdir(output, { recursive: true });
  await writeFile(path.join(output, 'background-answer.json'), JSON.stringify({ browser: browser.version(), initial, before, after, question, errors, calls: server.calls }, null, 2));
});

for (const dpr of [2, 3]) test(`mobile native taps, source continuity and rotation DPR ${dpr}`, async ({ browser }) => {
  test.skip(process.env.FP7_ENGINE === 'webview2', 'Native desktop uses its existing window');
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: dpr, hasTouch: true, isMobile: true });
  const page = await context.newPage(); await backend(page, 'epub');
  await page.goto(process.env.FOCUS_ENTRY_URL!);
  await page.getByRole('button', { name: '专注阅读', exact: true }).filter({ visible: true }).first().tap();
  const snapshot = () => page.evaluate(() => (window as any).fp4.snapshot());
  const settled = () => expect.poll(async () => { const s = await snapshot(); return s?.phase === 'ready' && !s.preparing && s.turn.phase === 'idle'; }).toBe(true);
  await settled(); const before = await snapshot();
  const box = (await page.locator('.focus-reader-surface').boundingBox())!;
  await page.touchscreen.tap(box.x + box.width - 12, box.y + box.height * .45); await settled();
  expect((await snapshot()).current.start).toEqual(before.current.end);
  const anchor = (await snapshot()).current.start;
  await page.setViewportSize({ width: 844, height: 390 }); await settled();
  expect((await snapshot()).current.start).toMatchObject({ index: anchor.index, offset: anchor.offset });
  await page.setViewportSize({ width: 320, height: 844 }); await settled();
  expect((await snapshot()).current.start).toMatchObject({ index: anchor.index, offset: anchor.offset });
  const state = await snapshot(); expect(state.peak.pages).toBeLessThanOrEqual(8); expect(state.peak.domNodes).toBeLessThanOrEqual(12000);
  const output = path.resolve(process.env.FOCUS_EVIDENCE_DIR!); await mkdir(output, { recursive: true });
  await page.screenshot({ path: path.join(output, `native-tap-dpr-${dpr}.png`) });
  await writeFile(path.join(output, `native-tap-dpr-${dpr}.json`), JSON.stringify({ browser: browser.version(), before, state,
    environment: await page.evaluate(() => ({ ua: navigator.userAgent, dpr: devicePixelRatio, viewport: [innerWidth, innerHeight], performanceEntries: PerformanceObserver.supportedEntryTypes })) }, null, 2));
  await context.close();
});
