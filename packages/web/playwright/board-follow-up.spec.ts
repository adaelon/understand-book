import { expect, test, type Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import type { BoardDocument } from '../src/generated/BoardDocument';
import type { BoardView } from '../src/generated/BoardView';

// Every product request reaches the real multi-user server and private storage.
// The control host only seeds fixtures and gates the scripted model response.
const backend = process.env.WB2_BACKEND_URL ?? 'http://127.0.0.1:4220';
const control = process.env.WB2_CONTROL_URL ?? 'http://127.0.0.1:4221';
const output = resolve('../../tmp/board-wb2/browser');

interface Fixture {
  cookie: string;
  publication: { book_id: string; publication_id: string };
  learning_workspace_id: string;
  session_id: string;
  learner_quote: string;
  answer_marker: string;
  board_a: BoardDocument;
  board_b: BoardDocument;
  target_a_item_id: string;
  target_b_item_id: string;
  understanding_item_id: string;
}
interface ProbeState {
  blocked: boolean;
  waiting: boolean;
  completed: number;
  requests: Array<{ stage: number; messages: Array<{ content?: string | null }> }>;
}

function boardUrl(seed: Fixture, itemId = seed.target_a_item_id) {
  return `/wb2-acceptance?${new URLSearchParams({
    board_id: seed.board_a.board_id,
    board_book_id: seed.publication.book_id,
    board_publication_id: seed.publication.publication_id,
    board_learning_workspace_id: seed.learning_workspace_id,
    board_item_id: itemId,
  })}`;
}

async function readBoard(page: Page, boardId: string): Promise<{ document: BoardDocument; view: BoardView }> {
  return page.evaluate(async id => {
    const modulePath = '/src/api.ts';
    return (await import(modulePath)).api.boardRead(id);
  }, boardId);
}

test.afterEach(async ({ page, request }, info) => {
  // Always release a blocked model so a failure cannot hang the next host run.
  await request.post(`${control}/model/release`).catch(() => undefined);
  if (info.status !== info.expectedStatus) {
    mkdirSync(output, { recursive: true });
    await page.screenshot({ path: resolve(output, 'failure.png'), fullPage: true });
    writeFileSync(resolve(output, 'failure.txt'), await page.locator('body').innerText());
    const probe = await request.get(`${control}/model/requests`).then(response => response.json()).catch(() => null);
    writeFileSync(resolve(output, 'failure-model.json'), JSON.stringify(probe, null, 2));
  }
});

test('WB2 freezes the selected object, retains the complete reply and finds it again through search and understanding', async ({ page, context, request }) => {
  mkdirSync(output, { recursive: true });
  page.setDefaultTimeout(20_000);
  const fixtureResponse = await request.get(`${control}/fixture`);
  expect(fixtureResponse.ok(), await fixtureResponse.text()).toBe(true);
  const seed = await fixtureResponse.json() as Fixture;
  const errors: string[] = [];
  const httpFailures: string[] = [];
  const optionalSourceManifestMisses: string[] = [];
  const sentFollowUps: unknown[] = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('request', request => {
    if (request.method() !== 'POST') return;
    try {
      const body = request.postDataJSON();
      if (body?.board_follow_up) sentFollowUps.push(body.board_follow_up);
    } catch { /* Non-JSON requests are unrelated to this submission. */ }
  });
  await context.route(/\/api\/|\/books\//, async route => {
    const incoming = route.request(), url = new URL(incoming.url());
    const response = await route.fetch({
      url: `${backend}${url.pathname}${url.search}`,
      headers: { ...await incoming.allHeaders(), host: 'reader.example', origin: 'https://reader.example', cookie: seed.cookie },
      timeout: 120_000,
    });
    // Plain text publications may have no PDF source manifest. Reader explicitly
    // accepts this 404; all board, run, source and other failures remain failures.
    if (response.status() === 404 && incoming.method() === 'GET' && url.pathname.endsWith('/source_manifest')) optionalSourceManifestMisses.push(url.pathname);
    else if (!response.ok()) httpFailures.push(`${response.status()} ${incoming.method()} ${url.pathname}: ${await response.text()}`);
    await route.fulfill({ response });
  });
  await context.route(/\/wb2-acceptance(?:\?.*)?$/, route => route.fulfill({
    contentType: 'text/html',
    body: `<!doctype html><html lang="zh-CN"><head><meta name="viewport" content="width=device-width, initial-scale=1"></head><body><div id="app"></div><script type="module">
      import {createApp} from '/node_modules/.vite/deps/vue.js';
      import NetworkApp from '/src/NetworkApp.vue'; import '/src/style.css';
      createApp(NetworkApp).mount('#app');
    </script></body></html>`,
  }));

  await page.goto(boardUrl(seed));
  const board = page.getByRole('region', { name: '知识画板工作区', exact: true });
  const card = (id: string) => board.locator(`[data-item-id="${id}"]`);
  const pane = board.locator('.board-reading-pane:visible');
  const targetA = seed.board_a.items.find(item => item.item_id === seed.target_a_item_id)!;
  const targetB = seed.board_a.items.find(item => item.item_id === seed.target_b_item_id)!;
  await expect(board.getByRole('textbox', { name: '画板名称' })).toHaveValue(seed.board_a.title);
  // The seed has a real earlier teaching observation. This scenario continues
  // with an ordinary question, using the product switch to retain that history
  // while leaving teaching-design generation to its own runtime tests.
  await board.getByRole('button', { name: '关闭画板，回到阅读' }).click();
  const teaching = page.getByRole('switch', { name: '连续教学', exact: true });
  await expect(teaching).toHaveAttribute('aria-checked', 'true');
  await teaching.click();
  await expect(teaching).toHaveAttribute('aria-checked', 'false');
  await page.goto(boardUrl(seed));
  await expect(card(targetA.item_id).locator('.board-card-open')).toHaveAttribute('aria-pressed', 'true');
  await expect(pane).toContainText(targetA.title);
  const before = (await readBoard(page, seed.board_a.board_id)).document;
  const otherBefore = (await readBoard(page, seed.board_b.board_id)).document;
  expect(new Set(before.items.map(item => item.target.kind))).toEqual(new Set(['prebuild', 'answer', 'presentation', 'understanding']));

  expect((await request.post(`${control}/model/block`)).ok()).toBe(true);
  const question = '这张平均速度卡片为什么不能直接平均两个速度？请保留完整推理，并引用原文。';
  await board.getByRole('textbox', { name: '针对选中内容提问' }).fill(question);
  await board.getByRole('button', { name: '发送画板追问', exact: true }).click();
  const probe = async () => await request.get(`${control}/model/requests`).then(response => response.json()) as ProbeState;
  await expect.poll(async () => (await probe()).waiting, { timeout: 45_000, message: 'The real agent must reach the gated final response after source.present.' }).toBe(true);
  await expect(board.locator('.board-run-status')).toContainText(targetA.title);
  const frozen = await probe();
  const firstMessages = frozen.requests.find(item => item.stage === 0)?.messages ?? [];
  const frozenContext = firstMessages.map(message => message.content ?? '').filter(text => text.includes('Board follow-up at submission')).join('\n');
  expect(frozenContext).toContain(`"board_id":"${seed.board_a.board_id}"`);
  expect(frozenContext).toContain(`"item_id":"${targetA.item_id}"`);
  expect(frozenContext).not.toContain(`"item_id":"${targetB.item_id}"`);

  await board.getByRole('button', { name: '显示全部内容', exact: true }).click();
  await card(targetB.item_id).locator('.board-card-open').click();
  await expect(board.locator('.board-question-target')).toContainText(targetB.title);
  // Move the original parent while B remains selected. The reply must use A's
  // current layout without replacing the reader's newer selection or position.
  const handle = await card(targetA.item_id).locator('.board-card-label').boundingBox();
  expect(handle).toBeTruthy();
  await page.mouse.move(handle!.x + 40, handle!.y + handle!.height / 2);
  await page.mouse.down();
  await page.mouse.move(handle!.x + 92, handle!.y + handle!.height / 2 + 34, { steps: 6 });
  await page.mouse.up();
  await expect.poll(async () => (await readBoard(page, seed.board_a.board_id)).document.items.find(item => item.item_id === targetA.item_id)!.position.x).not.toBe(targetA.position.x);
  await expect(card(targetB.item_id).locator('.board-card-open')).toHaveAttribute('aria-pressed', 'true');
  const moved = (await readBoard(page, seed.board_a.board_id)).document.items.find(item => item.item_id === targetA.item_id)!.position;
  await expect(board.locator('.board-save-state')).toHaveText('已保存');
  await page.screenshot({ path: resolve(output, 'desktop-waiting-selection.png') });

  expect((await request.post(`${control}/model/release`)).ok()).toBe(true);
  await expect.poll(async () => (await readBoard(page, seed.board_a.board_id)).document.deliveries.length, { timeout: 45_000 }).toBe(before.deliveries.length + 1);
  await expect(board.getByRole('button', { name: '定位新回答', exact: true })).toBeVisible();
  let saved = (await readBoard(page, seed.board_a.board_id)).document;
  const replies = saved.items.filter(item => item.readable_content.includes(seed.answer_marker));
  expect(replies).toHaveLength(1);
  const answer = replies[0];
  expect(answer.target.kind).toBe('answer');
  expect(answer.reply_to_item_id).toBe(targetA.item_id);
  expect(answer.block_id).toBeTruthy();
  expect(answer.source_bindings.length).toBeGreaterThan(0);
  expect(answer.readable_content).toContain('1.5 m/s');
  expect(answer.readable_content).toContain('只有各段用时相同');
  expect(saved.items).toHaveLength(before.items.length + 1);
  expect(saved.items.find(item => item.item_id === targetA.item_id)!.position).toEqual(moved);
  expect(answer.position.x).toBeGreaterThan(moved.x);
  const delivery = saved.deliveries.find(item => item.item_ids.includes(answer.item_id))!;
  expect(delivery.parent_item_id).toBe(targetA.item_id);
  expect(delivery.block_ids).toContain(answer.block_id);
  expect((await readBoard(page, seed.board_b.board_id)).document).toEqual(otherBefore);
  await expect(card(targetB.item_id).locator('.board-card-open')).toHaveAttribute('aria-pressed', 'true');
  await expect(board.locator(`[data-board-reply-link="${answer.item_id}"]`)).toHaveCount(1);
  await board.getByRole('button', { name: '定位新回答', exact: true }).click();
  await expect(pane).toContainText(seed.answer_marker);
  await expect(pane.locator('.board-reply-parent')).toContainText(targetA.title);
  const headingBox = await pane.locator('.board-reading-heading').boundingBox();
  const paneBox = await pane.boundingBox();
  expect(headingBox!.y).toBeGreaterThanOrEqual(paneBox!.y - 1);
  await expect(board.locator('.board-save-state')).toHaveText('已保存');
  await page.screenshot({ path: resolve(output, 'desktop-answer.png') });

  // Rehydrating a completed run must not append its retained answer again.
  await page.reload();
  await expect(board).toBeVisible();
  await expect(board.locator('.board-card')).toHaveCount(saved.items.length);
  const reopened = (await readBoard(page, seed.board_a.board_id)).document;
  expect(reopened.deliveries).toEqual(saved.deliveries);
  expect(reopened.items.filter(item => item.readable_content.includes(seed.answer_marker)).map(item => item.item_id)).toEqual([answer.item_id]);
  expect(reopened.items.find(item => item.item_id === targetA.item_id)!.position).toEqual(moved);

  await board.getByRole('button', { name: '搜索画板与全文', exact: true }).click();
  const gallery = page.getByRole('dialog', { name: '我的知识画板', exact: true });
  await gallery.getByRole('searchbox', { name: '搜索画板与全文' }).fill(seed.answer_marker);
  const found = gallery.locator('.board-search-result');
  await expect(found).toHaveCount(1);
  await expect(found).toContainText(seed.board_a.title);
  await expect(found).not.toContainText('###');
  await expect(found).not.toContainText('**');
  await page.screenshot({ path: resolve(output, 'desktop-search.png') });
  await found.click();
  await expect(gallery).not.toBeVisible();
  await expect(card(answer.item_id).locator('.board-card-open')).toHaveAttribute('aria-pressed', 'true');
  await expect(pane.locator(`[data-board-block-id="${answer.block_id}"]`)).toContainText(seed.answer_marker);

  await board.getByRole('button', { name: '关闭画板，回到阅读' }).click();
  await expect(board).not.toBeVisible();
  await page.getByRole('button', { name: '我的理解', exact: true }).filter({ visible: true }).first().click();
  const understanding = page.getByRole('dialog', { name: '学习详情', exact: true });
  await understanding.locator('.concept').filter({ hasText: '平均速度与慢段权重' }).first().click();
  await expect(understanding.locator('.quote')).toContainText(seed.learner_quote);
  const related = understanding.getByRole('region', { name: '相关画板', exact: true });
  await expect(related).toContainText(seed.board_a.title);
  const relatedAnswer = related.getByRole('button').filter({ hasText: answer.title });
  await expect(relatedAnswer).toHaveCount(1);
  await page.screenshot({ path: resolve(output, 'desktop-understanding-related.png') });
  await relatedAnswer.click();
  await expect(understanding).not.toBeVisible();
  await expect(board).toBeVisible();
  await expect(card(answer.item_id).locator('.board-card-open')).toHaveAttribute('aria-pressed', 'true');
  await expect(pane.locator(`[data-board-block-id="${answer.block_id}"]`)).toContainText(seed.answer_marker);
  await board.getByRole('button', { name: '显示全部内容', exact: true }).click();
  await card(seed.understanding_item_id).locator('.board-card-open').click();
  await expect(pane.locator('.board-understanding-quote')).toContainText(seed.learner_quote);

  // Deleting the original conversation exercises the independent retained copy,
  // including source authorization after a fresh Reader/board initialization.
  await board.getByRole('button', { name: '关闭画板，回到阅读' }).click();
  await page.evaluate(async sessionId => {
    const modulePath = '/src/api.ts';
    await (await import(modulePath)).api.agentHistoryDelete(sessionId);
  }, seed.session_id);
  await page.goto(boardUrl(seed, answer.item_id));
  await expect(pane).toContainText(seed.answer_marker);
  await pane.locator('.board-reading-sources summary').click();
  await pane.locator('.board-reading-sources button').first().click();
  await page.getByRole('dialog', { name: '画板原文', exact: true }).getByRole('button', { name: '在阅读器中打开', exact: true }).click();
  await expect(board).not.toBeVisible();
  await page.getByRole('button', { name: '返回画板', exact: true }).filter({ visible: true }).first().click();
  await expect(board).toBeVisible();
  await expect(pane).toContainText(seed.answer_marker);
  await expect(card(answer.item_id).locator('.board-card-open')).toHaveAttribute('aria-pressed', 'true');
  saved = (await readBoard(page, seed.board_a.board_id)).document;
  expect(saved.deliveries).toEqual(reopened.deliveries);
  expect(saved.items.filter(item => item.readable_content.includes(seed.answer_marker))).toHaveLength(1);
  await board.getByRole('button', { name: '显示全部内容', exact: true }).click();
  await page.screenshot({ path: resolve(output, 'desktop-retained-board.png') });

  await page.setViewportSize({ width: 390, height: 844 });
  const tabs = board.getByRole('group', { name: '画板与全文', exact: true });
  await tabs.getByRole('button', { name: '阅读全文', exact: true }).click();
  await expect(pane).toContainText(seed.answer_marker);
  await page.screenshot({ path: resolve(output, 'mobile-reading.png') });
  await tabs.getByRole('button', { name: /^画板/ }).click();
  await board.getByRole('button', { name: '显示全部内容', exact: true }).click();
  await page.screenshot({ path: resolve(output, 'mobile-canvas.png') });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  writeFileSync(resolve(output, 'result.json'), JSON.stringify({
    board_id: seed.board_a.board_id, original_parent_id: targetA.item_id, later_selection_id: targetB.item_id,
    answer_item_id: answer.item_id, answer_block_id: answer.block_id, delivery,
    moved_parent_position: moved, retained_item_count: saved.items.length,
    original_chat_deleted: seed.session_id, frozen_context: frozenContext, sent_follow_ups: sentFollowUps,
    errors, httpFailures, optionalSourceManifestMisses,
  }, null, 2));
  expect(errors).toEqual([]);
  expect(httpFailures).toEqual([]);
});
