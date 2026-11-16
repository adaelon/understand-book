import { expect, test } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Uses tests::board_browser_host and the real Reader. Every board request reaches disk.
const backend = process.env.WB1_BACKEND_URL ?? 'http://127.0.0.1:4218';
const output = resolve('../../tmp/board-wb1/browser');
test.afterEach(async ({ page }, info) => {
  if (info.status !== info.expectedStatus) {
    mkdirSync(output, { recursive: true });
    await page.screenshot({ path: resolve(output, 'failure.png'), fullPage: true });
    writeFileSync(resolve(output, 'failure.txt'), await page.locator('body').innerText());
  }
});

test('WB1 four retained types survive source navigation, editing, reopen and original-chat deletion', async ({ page, request }) => {
  mkdirSync(output, { recursive: true });
  const seed = await request.get(`${backend}/test/fixture`).then(r => r.json());
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('/');
  await page.locator('.topbar .tools-toggle').click();
  await page.getByRole('button', { name: '当前资料的画板', exact: true }).click();
  await page.getByRole('textbox', { name: '新画板名称' }).fill('平均速度为什么不能直接平均');
  await page.getByRole('button', { name: '新建画板', exact: true }).click();
  const board = page.getByRole('region', { name: '知识画板工作区', exact: true });
  const add = async (type: string) => {
    await board.locator('.board-add-button').click();
    const dialog = page.getByRole('dialog', { name: '添加到画板', exact: true });
    await dialog.getByRole('tab', { name: type, exact: true }).click();
    await dialog.locator('.board-catalog-item').first().click();
    await expect(dialog).not.toBeVisible();
  };
  await add('预构建知识');
  await add('Agent 回答'); // Retains the embedded demonstration in the same commit.
  await add('我的理解');
  await expect(board.locator('.board-card')).toHaveCount(4);
  const boardId = new URL(page.url()).searchParams.get('board_id')!;
  expect(boardId).toBeTruthy();
  const read = async () => {
    const response = await request.post(`${backend}/boards/read`, { data: { board_id: boardId } });
    expect(response.ok(), await response.text()).toBe(true); return response.json();
  };
  let saved = await read();
  expect(new Set(saved.document.items.map((item: any) => item.target.kind))).toEqual(new Set(['prebuild', 'answer', 'presentation', 'understanding']));
  const answer = saved.document.items.find((item: any) => item.target.kind === 'answer');
  await board.locator(`[data-item-id="${answer.item_id}"] .board-card-open`).click();
  await expect(board.locator('.board-reading-pane:visible')).toContainText('慢的路段占用了更多时间');
  await board.getByRole('textbox', { name: '画板名称' }).fill('平均速度 · 我的完整板书');
  await board.getByRole('textbox', { name: '画板名称' }).press('Enter');
  const card = board.locator(`[data-item-id="${answer.item_id}"] .board-card-label`);
  const position = await card.boundingBox(); expect(position).toBeTruthy();
  await page.mouse.move(position!.x + 60, position!.y + 8); await page.mouse.down();
  await page.mouse.move(position!.x + 25, position!.y + 45, { steps: 5 }); await page.mouse.up();
  await expect.poll(async () => (await read()).document.items.find((item: any) => item.item_id === answer.item_id).position.x).not.toBe(answer.position.x);
  await board.locator('.board-reading-pane:visible .board-reading-sources summary').click();
  await board.locator('.board-reading-pane:visible .board-reading-sources button').first().click();
  await page.getByRole('dialog', { name: '画板原文', exact: true }).getByRole('button', { name: '在阅读器中打开', exact: true }).click();
  await expect(board).not.toBeVisible();
  await page.getByRole('button', { name: '返回画板', exact: true }).filter({ visible: true }).first().click();
  await expect(board).toBeVisible();
  await board.locator('.board-reading-pane:visible').getByRole('button', { name: /打开回答中的演示/ }).click();
  const frame = page.frameLocator('.board-reading-pane:visible iframe');
  await expect(frame.getByRole('slider', { name: '回程速度' })).toHaveValue('1');
  await frame.getByRole('slider', { name: '回程速度' }).focus();
  await frame.getByRole('slider', { name: '回程速度' }).press('ArrowRight');
  await frame.getByRole('slider', { name: '回程速度' }).press('ArrowRight');
  await frame.getByRole('button', { name: '显示比较结论' }).click();
  await expect(frame.getByRole('slider', { name: '回程速度' })).toHaveValue('2');
  await expect(board.locator('.board-save-state')).toHaveText('已保存');
  await page.screenshot({ path: resolve(output, 'desktop.png') });
  await board.getByRole('button', { name: '关闭画板，回到阅读' }).click();
  await expect(board).toHaveCount(0);
  const deleted = await request.post(`${backend}/agent/history/delete`, { data: { session_id: seed.session_id } });
  expect(deleted.ok(), await deleted.text()).toBe(true);
  expect((await request.post(`${backend}/test/reopen`)).ok()).toBe(true);
  await page.goto(`/?board_id=${encodeURIComponent(boardId)}`);
  await expect(board.locator('.board-card')).toHaveCount(4);
  await expect(board.getByRole('textbox', { name: '画板名称' })).toHaveValue('平均速度 · 我的完整板书');
  await expect(frame.getByRole('slider', { name: '回程速度' })).toHaveValue('2');
  await expect(frame.locator('#conclusion')).toBeVisible();
  await board.locator(`[data-item-id="${answer.item_id}"] .board-card-open`).click();
  await expect(board.locator('.board-reading-pane:visible')).toContainText('慢的路段占用了更多时间');
  await board.locator('.board-reading-pane:visible .board-reading-sources summary').click();
  await board.locator('.board-reading-pane:visible .board-reading-sources button').first().click();
  await page.getByRole('dialog', { name: '画板原文', exact: true }).getByRole('button', { name: '在阅读器中打开', exact: true }).click();
  await expect(board).not.toBeVisible();
  await page.getByRole('button', { name: '返回画板', exact: true }).filter({ visible: true }).first().click();
  saved = await read();
  const understanding = saved.document.items.find((item: any) => item.target.kind === 'understanding');
  await board.locator(`[data-item-id="${understanding.item_id}"] .board-card-open`).click();
  await expect(board.locator('.board-understanding-quote:visible')).toContainText(seed.learner_quote);
  await expect(board.locator('.board-save-state')).toHaveText('已保存');
  await page.screenshot({ path: resolve(output, 'desktop-understanding.png') });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(board.locator('.board-understanding-quote:visible')).toBeVisible();
  await page.screenshot({ path: resolve(output, 'mobile-reading.png') });
  await board.getByRole('group', { name: '画板与全文' }).getByRole('button', { name: /^画板/ }).click();
  await board.getByRole('button', { name: '显示全部内容', exact: true }).click();
  await page.screenshot({ path: resolve(output, 'mobile-canvas.png') });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await board.getByRole('button', { name: '关闭画板，回到阅读' }).click();
  await expect(board).toHaveCount(0);
  writeFileSync(resolve(output, 'result.json'), JSON.stringify({ boardId, kinds: saved.document.items.map((item: any) => item.target.kind), originalChatDeleted: true, errors }, null, 2));
  expect(errors).toEqual([]);
});

test('cross-material media sources preview in place without losing the board', async ({ page, request }) => {
  // Board persistence is real; only source replies are controlled to exercise the
  // existing cross-material video/music contracts in the complete Reader shell.
  const seed = await request.get(`${backend}/test/fixture`).then(r => r.json());
  const created = await request.post(`${backend}/boards/create`, { data: { title: '跨资料来源' } }).then(r => r.json());
  const imported = await request.post(`${backend}/boards/import`, { data: {
    board_id: created.board_id, board_revision: created.revision, source: seed.sources.prebuild,
  } }).then(r => r.json());
  const item = imported.items[0];
  const audio = Buffer.alloc(44 + 96000);
  audio.write('RIFF', 0); audio.writeUInt32LE(audio.length - 8, 4); audio.write('WAVEfmt ', 8);
  audio.writeUInt32LE(16, 16); audio.writeUInt16LE(1, 20); audio.writeUInt16LE(1, 22);
  audio.writeUInt32LE(8000, 24); audio.writeUInt32LE(16000, 28); audio.writeUInt16LE(2, 32); audio.writeUInt16LE(16, 34);
  audio.write('data', 36); audio.writeUInt32LE(96000, 40);
  await page.route('**/wb1-source.wav', route => {
    const range = /bytes=(\d+)-(\d*)/.exec(route.request().headers().range ?? '');
    const start = range ? Number(range[1]) : 0, end = range?.[2] ? Number(range[2]) : audio.length - 1;
    return route.fulfill({ status: range ? 206 : 200, contentType: 'audio/wav', body: audio.subarray(start, end + 1),
      headers: { 'Accept-Ranges': 'bytes', ...(range ? { 'Content-Range': `bytes ${start}-${end}/${audio.length}` } : {}) } });
  });
  const publication = { book_id: 'other-material', publication_id: 'original-publication' };
  const base = { source_ref_id: item.source_bindings[0].source_ref_id, label: '原资料依据', material_title: '另一份材料',
    highlighted_quote: '来源保留在原发布中', context_before: '', context_after: '', stale: false, can_open_in_reader: true, media_url: '/wb1-source.wav' };
  let source: any = { ...base, media: { publication, duration_ms: 60000, transcript: [], translations: [],
    target: { kind: 'audio', publication, location: { video: { media_id: 'video', media_revision: 'v1' }, start_ms: 1000, end_ms: 5000 } } } };
  let openRequests = 0;
  await page.route('**/boards/source.resolve', route => route.fulfill({ json: source }));
  await page.route('**/boards/source.open', route => { openRequests += 1; return route.abort(); });
  await page.goto(`/?board_id=${encodeURIComponent(created.board_id)}`);
  const board = page.getByRole('region', { name: '知识画板工作区', exact: true });
  await board.locator(`[data-item-id="${item.item_id}"] .board-card-open`).click();
  await board.locator('.board-reading-pane:visible .board-reading-sources summary').click();
  const origin = board.locator('.board-reading-pane:visible .board-reading-sources button').first();
  await origin.click();
  const preview = page.getByRole('dialog', { name: '画板原文', exact: true });
  await expect(preview.locator('.media-source-preview')).toBeVisible();
  await expect.poll(() => preview.locator('audio').evaluate((node: HTMLAudioElement) => node.currentTime)).toBe(1);
  await expect(board).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(preview).toHaveCount(0);
  source = { ...base, can_open_in_reader: false, music: { title: '原录音', label: '主题片段', preview: '音乐主题的原始片段',
    target: { publication, audio: { start_ms: 1000, end_ms: 5000 } }, material: {} } };
  await origin.click();
  await expect(preview.locator('.music-source-preview')).toContainText('音乐主题的原始片段');
  await expect(board).toBeVisible();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: resolve(output, 'mobile-source-preview.png') });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.keyboard.press('Escape');
  expect(openRequests).toBe(0);
});
