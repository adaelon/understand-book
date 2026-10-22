import { expect, test } from '@playwright/test';
import { setup, scenario, source } from './fixtures/tutor-continuation';

const title = '全书结构与语义候选召回';

for (const width of [1440, 390]) test(`chapter reading entry is usable and submits once at ${width}px`, async ({ page }, testInfo) => {
  const fixture = await setup(page, 'markdown', true);
  const state = { control: { enabled: false, revision: 0, current_tutor_session_id: null as string | null }, sessions: {} as Record<string, unknown> };
  await page.route('**/api/book/manifest', route => route.fulfill({ json: { tree: [
    { lid: '1', kind: 'chapter', children: ['1.1'], span: { start: 0, end: source.length }, title: `# ${title}` },
    { lid: '1.1', kind: 'paragraph', children: [], span: { start: 0, end: source.length } },
  ], stats_by_lid: {} } }));
  await page.route('**/api/tutor/state', route => route.fulfill({ json: state }));
  await page.route('**/api/tutor/mutate', async route => {
    const body = route.request().postDataJSON();
    fixture.mutations.push(body);
    if (body.action.kind === 'set_enabled') state.control.enabled = body.action.enabled;
    if (body.action.kind === 'start') {
      state.control.current_tutor_session_id = 'chapter-session';
      state.sessions['chapter-session'] = { id: 'chapter-session', revision: 0, status: 'active', ...body.action };
    }
    state.control.revision++;
    await route.fulfill({ json: state });
  });
  await page.route('**/api/tutor/start', route => route.fulfill({ json: { started: true, message: '继续本次学习' } }));
  await page.setViewportSize({ width, height: 900 });
  await page.reload();
  if (width < 1024) await page.getByRole('button', { name: '菜单', exact: true }).click();
  const button = page.getByRole('button', { name: `开启学习并带我读这一章：${title}`, exact: true });
  await expect(button).toBeVisible();
  await expect(button).toBeEnabled();
  await page.screenshot({ path: testInfo.outputPath(`chapter-entry-${width}.png`), fullPage: true });
  await button.click();
  await expect.poll(() => fixture.requests.length).toBe(1);
  expect(fixture.requests[0]).toMatchObject({ message: '', entry_action: 'learn_with_purpose', learning_purpose: 'understand', chapter_lid: '1',
    source_id: scenario.book_id, source_revision: 'tc0-source', session_id: 'tc0-chat',
    display_user: `带我读《${title}》这一章`, question_quote: null });
  expect(fixture.requests[0]).not.toHaveProperty('question_anchor_lid');
  expect(fixture.mutations.map((request: any) => request.action.kind)).toEqual(['set_enabled']);
  await expect(page.locator('.turn').last()).toContainText(`带我读《${title}》这一章`);
  await expect(page.locator('.ans-text')).toHaveCount(1);
  expect(fixture.requests).toHaveLength(1);
  expect(fixture.errors).toEqual([]);
  await page.screenshot({ path: testInfo.outputPath(`chapter-submitted-${width}.png`), fullPage: true });
});

for (const width of [1440, 390]) test(`conversation purpose entry preserves the draft and starts once at ${width}px`, async ({ page }, testInfo) => {
  const fixture = await setup(page, 'markdown', true);
  await page.setViewportSize({ width, height: 900 });
  if (width < 1024) await page.getByRole('navigation', { name: '阅读工作区' }).getByRole('button', { name: '问答', exact: true }).click();
  const entry = page.getByRole('region', { name: '选择学习目的' });
  for (const label of ['先了解重点', '带我读懂', '复习备考', '探索一个问题']) {
    const button = entry.getByRole('button', { name: new RegExp(`^${label}`) });
    await expect(button).toBeVisible();
    await expect(button).toBeEnabled();
  }
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  const input = page.locator('.agent-input textarea');
  await input.fill('保留这份还没写完的问题');
  await page.screenshot({ path: testInfo.outputPath(`purpose-entry-${width}.png`), fullPage: true });
  await entry.getByRole('button', { name: /^带我读懂/ }).click();
  await expect.poll(() => fixture.requests.length).toBe(1);
  expect(fixture.requests[0]).toMatchObject({ message: '', entry_action: 'learn_with_purpose', learning_purpose: 'understand',
    chapter_lid: '1', source_id: scenario.book_id, source_revision: 'tc0-source', session_id: 'tc0-chat',
    client_request_id: expect.any(String), expected_chat_head: null, question_quote: null });
  expect(fixture.requests[0]).not.toHaveProperty('question_anchor_lid');
  expect(fixture.mutations.map((request: any) => request.action.kind)).toEqual(['set_enabled']);
  await expect(page.locator('.ans-text')).toHaveCount(1);
  await expect(input).toHaveValue('保留这份还没写完的问题');
  expect(fixture.requests).toHaveLength(1);
  expect(fixture.errors).toEqual([]);
  await page.screenshot({ path: testInfo.outputPath(`purpose-submitted-${width}.png`), fullPage: true });
});
