import { expect, test } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const output = process.env.OM6_OUTPUT;
const local = process.env.OM6_LOCAL_HOST;
const networkControl = process.env.OM6_NETWORK_CONTROL;
const networkBackend = process.env.OM6_NETWORK_BACKEND;
test.afterEach(async ({ page }, info) => {
  if (info.status !== info.expectedStatus) {
    await page.screenshot({ path: info.outputPath('failure.png'), fullPage: true });
    writeFileSync(info.outputPath('body.txt'), await page.locator('body').innerText());
  }
});

test('OM6 local Reader explains pending input and reopens assisted borrowing with real model text', async ({ page }) => {
  test.skip(!local, 'Start om6_local_browser_host and the original Vite API proxy');
  test.setTimeout(120_000);
  await page.setViewportSize({ width: 1280, height: 1000 });
  await page.goto('/agent-presentation-visual.html');
  let fixture = await page.request.get(`${local}/fixture`).then(r => r.json());
  let frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#speed')).toHaveValue('2');
  await expect(frame.locator('#reading')).toContainText('B 走 10 米');
  await page.getByRole('textbox', { name: '针对当前演示追问' }).fill(fixture.next_request);
  await page.getByRole('button', { name: '发送追问', exact: true }).click();
  await expect(page.getByTestId('follow-up-status')).toHaveText('追问已完成');
  let stored = await page.request.get(`${local}/om6/evidence`).then(r => r.json());
  const speed = stored.reports[0];
  expect(speed.outcome.incomplete).toBe(false);
  expect(speed.delivery.payload.answer).toBe(speed.outcome.answer);
  expect(speed.outcome.answer).toContain('10');
  expect(speed.outcome.answer).toContain('2');
  expect(speed.outcome.answer).toContain('应用');
  expect(speed.prepare.learner_context.current_request).toBe(fixture.next_request);
  // Follow the generated operational support through the actual learner control.
  const saved = page.waitForResponse(response => response.url().endsWith('/agent/presentation.state.save')
    && response.request().postDataJSON()?.state?.observation?.rendered?.distance_b_m === 20);
  await frame.locator('#apply').click();
  await expect(frame.locator('#reading')).toContainText('B 走 20 米');
  expect((await saved).ok()).toBe(true);
  await page.request.post(`${local}/om6/reopen`);
  await page.reload();
  frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#reading')).toContainText('B 走 20 米');
  await expect(frame.locator('#speed')).toHaveValue('2');
  if (output) {
    mkdirSync(output, { recursive: true });
    await frame.locator('#reading').screenshot({ path: join(output, 'local-speed-reading.png') });
    await page.screenshot({ path: join(output, 'local-speed-reopened.png'), fullPage: true });
  }
  await page.request.post(`${local}/om6/case/borrowing`);
  await page.reload();
  fixture = await page.request.get(`${local}/fixture`).then(r => r.json());
  await page.locator('textarea[data-workspace-input="agent"]').fill(fixture.next_request);
  await page.getByRole('button', { name: '发送', exact: true }).click();
  await expect(page.getByTestId('follow-up-status')).toHaveText('追问已完成');
  stored = await page.request.get(`${local}/om6/evidence`).then(r => r.json());
  const borrowing = stored.reports[0];
  expect(borrowing.outcome.incomplete).toBe(false);
  expect(borrowing.outcome.answer).toContain('println!');
  expect(borrowing.outcome.answer).toContain('&Foo');
  const work = borrowing.prepare.learner_context.learning_opportunity.continuity.previous_work
    .find((row: any) => row.learner_quote_excerpt.text === 'bar 是 &Foo，foo 仍拥有值。');
  expect(work.assistance_refs.length).toBeGreaterThan(0);
  expect(work.assessment_status).toBe('unassessed');
  expect(borrowing.selection.payload.move.response_expectation ?? null).toBeNull();
  expect(stored.events.filter((row: any) => row[1].kind === 'learner_action')).toHaveLength(0);
  await page.request.post(`${local}/om6/reopen`);
  await page.reload();
  await expect(page.locator('.answer-markdown')).toContainText('println!');
  await expect(page.locator('.answer-markdown')).toContainText('&Foo');
  if (output) {
    writeFileSync(join(output, 'local-browser.json'), JSON.stringify({ speed: speed.outcome, borrowing: borrowing.outcome }, null, 2));
    await page.screenshot({ path: join(output, 'local-borrowing-reopened.png'), fullPage: true });
  }
});

test('OM6 network Reader adopts selected history, honors current open request and preserves the original', async ({ page, context }) => {
  test.skip(!networkControl || !networkBackend, 'Start om6_network_browser_host');
  test.setTimeout(120_000);
  const fixture = await page.request.get(`${networkControl}/fixture`).then(r => r.json());
  const failures: string[] = [];
  page.on('pageerror', error => failures.push(error.message));
  await context.route(/\/api\/|\/books\//, async route => {
    const request = route.request(), url = new URL(request.url());
    const response = await route.fetch({ url: `${networkBackend}${url.pathname}${url.search}`,
      headers: { ...await request.allHeaders(), host: 'reader.example', origin: 'https://reader.example', cookie: fixture.cookie },
      timeout: 45_000 });
    await route.fulfill({ response });
  });
  await context.route('**/om6-network', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><html>
    <head><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><div id="app"></div>
    <script type="module">import {createApp} from '/node_modules/.vite/deps/vue.js';
    import NetworkApp from '/src/NetworkApp.vue'; import '/src/style.css';
    createApp(NetworkApp).mount('#app');</script></body></html>` }));
  await page.addInitScript(id => {
    sessionStorage.setItem('understand-book:workspace:A', JSON.stringify({ workspace_id: id, view: 'reader' }));
  }, fixture.workspace.workspace_id);
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto('/om6-network');
  const input = page.getByRole('textbox', { name: '围绕当前阅读内容提问' });
  await expect(input).toBeEnabled();
  await input.fill(fixture.message);
  await page.getByRole('button', { name: '发送', exact: true }).click();
  await expect(page.locator('.turn').first().locator('.answer-markdown')).toContainText('必要');
  await expect(page.locator('.stop-agent')).toHaveCount(0);
  const evidence = await page.request.get(`${networkControl}/evidence`).then(r => r.json());
  expect(evidence.original_unchanged).toBe(true);
  const prepared = evidence.captures.find((row: any) => row.context)?.context;
  expect(prepared.learner_context.current_request).toBe(fixture.message);
  const continuity = prepared.learner_context.learning_opportunity.continuity;
  expect(continuity.previous_work.some((row: any) => row.historical &&
    row.learner_quote_excerpt.text === '我倾向减少停站，但必要的休息要保留。')).toBe(true);
  expect(JSON.stringify(continuity)).not.toContain('这条较晚的记录没有选入新工作区');
  const session = evidence.history.find((row: any) => row.id === fixture.workspace.selected_chat);
  const answer = session.turns.at(-1).outcome;
  expect(answer.incomplete).toBe(false);
  expect(answer.answer).toContain('停');
  expect(answer.answer).toContain('速度');
  expect(evidence.captures.some((row: any) => row.review?.learner_view?.learning_opportunity?.continuity)).toBe(true);
  await page.reload();
  await expect(page.locator('.turn').first().locator('.answer-markdown')).toContainText('必要');
  expect(failures).toEqual([]);
  if (output) {
    writeFileSync(join(output, 'network-browser.json'), JSON.stringify({ prepared, answer, original_unchanged: true }, null, 2));
    await page.screenshot({ path: join(output, 'network-open-design-reopened.png'), fullPage: true });
  }
  await page.goto('about:blank');
  await context.unrouteAll({ behavior: 'wait' });
});
