import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { setup, scenario } from './fixtures/tutor-continuation';

const output = resolve('../../docs/performance/learning-purpose-20261102');
const title = '你认为量化的本质是什么？';
const text = '量化金融，是定量研究金融市场的一种行为。本文聚焦买方量化的视角，先讨论我们赚的钱从何而来、任务是什么、应该预测什么。趋势策略着重预测未来的方向，套利策略则研究相对价格关系。';
async function openMaterial(page: Page, width: number) {
  const fixture = await setup(page, 'markdown', true);
  await page.setViewportSize({ width, height: width > 1000 ? 715 : 844 });
  await page.route('**/api/build_workbench/music/context', route => route.fulfill({ json: null }));
  await page.route('**/api/book/music_context', route => route.fulfill({ json: null }));
  await page.route('**/api/book/video_context', route => route.fulfill({ json: null }));
  await page.route('**/api/videos', route => route.fulfill({ json: { videos: [], active: null } }));
  await page.route('**/api/book/manifest', route => route.fulfill({ json: { stats_by_lid: {}, tree: [
    { lid: '1', kind: 'chapter', title, children: ['1.1','1.2','1.3'], span: { start: 0, end: 1000 } },
    { lid: '1.1', kind: 'paragraph', children: [], span: { start: 0, end: 18 } },
    { lid: '1.2', kind: 'section', title: '引言', children: ['1.2.1'], span: { start: 18, end: 500 } },
    { lid: '1.2.1', kind: 'paragraph', children: [], span: { start: 24, end: 500 } },
    { lid: '1.3', kind: 'section', title: '经典的 LSTM 股价预测', children: ['1.3.1'], span: { start: 500, end: 1000 } },
    { lid: '1.3.1', kind: 'paragraph', children: [], span: { start: 530, end: 1000 } },
  ] } }));
  await page.route('**/api/reader/state', route => route.fulfill({ json: {
    book_id: scenario.book_id, revision: 0, viewport: { anchor_lid: '1.2.1', top_lid: '1.2.1', bottom_lid: '1.2.1', width: 1, visible_lids: ['1.2.1'] },
    profile: { profile_id: 'technical_learning', profile_version: '1', ui_slots: [], slots: [], layout_presets: [], allowed_layout_actions: [], agent_tools: [] },
    layout: { rev: 0, open_slots: [], panel_sizes: {}, slot_order: [], pinned_evidence: [] },
  } }));
  await page.route('**/api/book/text?*', route => {
    const lid = new URL(route.request().url()).searchParams.get('lid');
    return route.fulfill({ json: { lid, text: lid === '1' || lid === '1.1' ? '# ' + title : lid === '1.2' ? '## 引言\n\n' + text : text } });
  });
  await page.reload();
  if (width < 1024) await page.getByRole('navigation', { name: '阅读工作区' }).getByRole('button', { name: '问答', exact: true }).click();
  const entry = page.getByRole('region', { name: '选择学习目的' });
  await expect(entry.locator('[data-purpose=understand]')).toBeEnabled();
  await expect(entry.getByRole('combobox')).toHaveValue('section');
  await expect(entry.locator('option')).toHaveText(['当前小节 · 引言', '整份材料']);
  return { ...fixture, entry };
}

for (const width of [1337, 390, 320]) test(`real subsection and all purpose forms remain usable at ${width}px`, async ({ page }) => {
  const fixture = await openMaterial(page, width), entry = fixture.entry;
  mkdirSync(output, { recursive: true });
  await page.screenshot({ path: resolve(output, `choices-${width}.png`) });
  for (const purpose of ['understand', 'exam', 'explore', 'practice']) {
    await entry.locator('[data-purpose=' + purpose + ']').click();
    expect(fixture.requests).toHaveLength(0);
    expect(fixture.mutations).toHaveLength(0);
    await expect(entry.getByRole('textbox')).toBeFocused();
    await expect(entry.locator('.purpose-submit')).toBeDisabled();
    await entry.getByRole('textbox').fill('我学过趋势和套利，想弄清两者如何判断。');
    await expect(entry.locator('.purpose-submit')).toBeEnabled();
    await entry.locator('.purpose-submit').scrollIntoViewIfNeeded();
    await expect(entry.locator('.purpose-submit')).toBeInViewport();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    const box = await entry.boundingBox();
    expect(box!.x).toBeGreaterThanOrEqual(0);
    expect(box!.x + box!.width).toBeLessThanOrEqual(width);
    if (purpose === 'explore' || purpose === 'practice') await page.screenshot({ path: resolve(output, `${purpose}-${width}.png`) });
    await entry.locator('.purpose-back').click();
  }
  await entry.locator('[data-purpose=practice]').click();
  await entry.getByRole('button', { name: '还没学过，先带我读懂' }).click();
  expect(fixture.requests).toHaveLength(0);
  await expect(entry.locator('.purpose-setup label')).toHaveText('想读懂哪里？');
  await entry.getByRole('textbox').fill('为什么套利不等于预测价格上涨？');
  await entry.locator('.purpose-submit').click();
  await expect.poll(() => fixture.requests.length).toBe(1);
  expect(fixture.requests[0]).toMatchObject({ message: '为什么套利不等于预测价格上涨？', learning_purpose: 'understand', chapter_lid: '1.2' });
  expect(fixture.errors).toEqual([]);
});

for (const purpose of ['discover', 'exam', 'explore', 'practice']) test(`submits the chosen ${purpose} purpose with the requested scope and content`, async ({ page }) => {
  const fixture = await openMaterial(page, 1337), entry = fixture.entry;
  await entry.locator('[data-purpose=' + purpose + ']').click();
  if (purpose !== 'discover') {
    await entry.getByRole('textbox').fill('我已学过趋势与套利，重点看适用条件。');
    await entry.locator('.purpose-submit').click();
  }
  await expect.poll(() => fixture.requests.length).toBe(1);
  expect(fixture.requests[0]).toMatchObject({ learning_purpose: purpose, chapter_lid: '1.2',
    message: purpose === 'discover' ? '' : '我已学过趋势与套利，重点看适用条件。' });
  expect(fixture.mutations).toHaveLength(purpose === 'discover' ? 0 : 1);
  expect(fixture.errors).toEqual([]);
});

test('English setup fits on a small phone and retains the same scope', async ({ page }) => {
  const fixture = await openMaterial(page, 1337);
  await page.getByRole('combobox', { name: 'Language / 语言' }).selectOption('en');
  await expect(page.locator('.learning-purpose-entry [data-purpose=practice]')).toContainText('Practice');
  await expect(page.locator('.banner')).toHaveCount(0);
  await page.setViewportSize({ width: 320, height: 844 });
  await page.locator('.workspace-mobile-nav button').nth(1).click();
  const entry = page.locator('.learning-purpose-entry');
  await entry.locator('[data-purpose=practice]').click();
  await entry.getByRole('textbox').fill('I have learned the definitions. Help me distinguish direction from relative prices.');
  await expect(entry.locator('.purpose-submit')).toBeInViewport();
  await expect(entry.getByRole('combobox')).toHaveValue('section');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  expect(fixture.requests).toHaveLength(0);
  await expect(page.getByRole('alert')).toHaveCount(0);
  await page.screenshot({ path: resolve(output, 'practice-en-320.png') });
});
