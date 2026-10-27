import { expect, test, type Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import type { BoardDocument } from '../src/generated/BoardDocument';
import type { BoardView } from '../src/generated/BoardView';

const control = process.env.WB3_CONTROL_URL ?? 'http://127.0.0.1:4221';
const output = resolve('../../tmp/board-wb3/browser');
interface Fixture {
  publication: { book_id: string; publication_id: string };
  learning_workspace_id: string; session_id: string;
  board_a: BoardDocument; target_a_item_id: string;
  answer_marker: string; comparison_marker: string;
  new_learner_quote: string; new_evidence_id: string;
}
interface Probe {
  waiting: boolean;
  requests: Array<{ mode: string; stage: number; preview_image_count: number;
    messages: Array<{ content?: string | null; tool_call_id?: string | null }> }>;
}
function url(seed: Fixture, item = seed.target_a_item_id) {
  return `/wb3-acceptance?${new URLSearchParams({
    board_id: seed.board_a.board_id, board_item_id: item,
    board_book_id: seed.publication.book_id, board_publication_id: seed.publication.publication_id,
    board_learning_workspace_id: seed.learning_workspace_id,
  })}`;
}
async function readBoard(page: Page, id: string): Promise<{ document: BoardDocument; view: BoardView }> {
  return page.evaluate(async boardId => { const path = '/src/api.ts'; return (await import(path)).api.boardRead(boardId); }, id);
}
function toolResult(probe: Probe, id: string) {
  for (const request of [...probe.requests].reverse()) {
    const message = [...request.messages].reverse().find(message => message.tool_call_id === id);
    if (message?.content) { const value = JSON.parse(message.content); return value.model_body ?? value; }
  }
  throw new Error(`No real tool result for ${id}`);
}
function rejectToolFailure(probe: Probe) {
  for (const sample of probe.requests.filter(sample => sample.mode === probe.requests.at(-1)?.mode)) for (const message of sample.messages) {
    if (!message.tool_call_id || !message.content) continue;
    let value: any;
    try { value = JSON.parse(message.content); } catch { continue; }
    if (value.status === 'error') throw new Error(`${message.tool_call_id}: ${JSON.stringify(value.model_body ?? value)}`);
    if (value.model_body?.status === 'preview_failed') throw new Error(`${message.tool_call_id}: real preview rejected candidate ${JSON.stringify(value.model_body.observations?.map((observation: any) => observation.issues))}`);
  }
}
function layoutPages(probe: Probe, mode: string) {
  const pages = new Map<string, { complete: boolean; total: number; records: Array<{ item_id: string }> }>();
  for (const sample of probe.requests.filter(sample => sample.mode === mode)) for (const message of sample.messages) {
    if (!message.tool_call_id?.startsWith('layout-page-') || !message.content) continue;
    const value = JSON.parse(message.content).model_body;
    if (value?.status === 'board_page' && value.collection === 'layout') pages.set(message.tool_call_id, value);
  }
  const result = [...pages.values()];
  expect(result.at(-1)?.complete).toBe(true);
  expect(result.flatMap(page => page.records)).toHaveLength(result.at(-1)!.total);
  if (mode !== 'native') expect(result.flatMap(page => page.records.map(item => item.item_id))).toEqual(expect.arrayContaining([
    'drawing-native-time', 'drawing-native-total', 'drawing-native-condition',
  ]));
  return result;
}
async function expectPresentationRegionVisible(page: Page) {
  await expect.poll(async () => {
    const result = await page.frameLocator('.board-reading-pane:visible iframe').locator('#result').boundingBox();
    const pane = await page.locator('.board-reading-pane:visible').filter({ has: page.locator('iframe') }).boundingBox();
    return !!result && !!pane && result.y >= pane.y && result.y + result.height <= pane.y + pane.height;
  }).toBe(true);
}

test.afterEach(async ({ page, request }, info) => {
  await request.post(`${control}/model/release`).catch(() => undefined);
  if (info.status !== info.expectedStatus) {
    mkdirSync(output, { recursive: true });
    await page.screenshot({ path: resolve(output, 'failure.png'), fullPage: true });
    writeFileSync(resolve(output, 'failure.txt'), await page.locator('body').innerText());
    const model = await request.get(`${control}/model/requests`).then(response => response.json()).catch(() => null);
    writeFileSync(resolve(output, 'failure-model.json'), JSON.stringify(model, null, 2));
  }
});

test('WB3 real preview, native linked explanation, persistent playback and new-chat retained demonstration', async ({ page, context, request }) => {
  mkdirSync(output, { recursive: true });
  page.setDefaultTimeout(25_000);
  const seed = await request.get(`${control}/fixture`).then(response => response.json()) as Fixture;
  const errors: string[] = [], httpFailures: string[] = [], optionalSourceManifestMisses: string[] = [];
  const networkCommands: unknown[] = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('response', async response => {
    const path = new URL(response.url()).pathname;
    if (!path.startsWith('/api/') && !path.startsWith('/books/')) return;
    if (response.status() === 404 && path.endsWith('/source_manifest')) optionalSourceManifestMisses.push(path);
    else if (response.status() >= 400) httpFailures.push(`${response.status()} ${response.request().method()} ${path}: ${await response.text().catch(() => '')}`);
  });
  page.on('request', incoming => {
    if (incoming.method() !== 'POST') return;
    try { const body = incoming.postDataJSON(); if (body?.operation === 'complete' && body?.command_id) networkCommands.push(body); }
    catch { /* unrelated request body */ }
  });
  // Only the entry document is synthetic. Product APIs, SSE, author previews and
  // presentation receipts pass through the streaming Vite HTTP proxy unchanged.
  await context.route(/\/wb3-acceptance(?:\?.*)?$/, route => route.fulfill({ contentType: 'text/html', body:
    `<!doctype html><html lang="zh-CN"><head><meta name="viewport" content="width=device-width, initial-scale=1"></head><body><div id="app"></div><script type="module">
    import {createApp} from '/node_modules/.vite/deps/vue.js'; import NetworkApp from '/src/NetworkApp.vue'; import '/src/style.css'; createApp(NetworkApp).mount('#app');
    </script></body></html>` }));
  await page.goto(url(seed));
  const board = page.getByRole('region', { name: '知识画板工作区', exact: true });
  const pane = board.locator('.board-reading-pane:visible');
  const card = (id: string) => board.locator(`[data-item-id="${id}"]`);
  await expect(board).toBeVisible();
  await board.getByRole('button', { name: '关闭画板，回到阅读', exact: true }).click();
  const teaching = page.getByRole('switch', { name: '连续教学', exact: true });
  await expect(teaching).toHaveAttribute('aria-checked', 'true');
  await teaching.click();
  await expect(teaching).toHaveAttribute('aria-checked', 'false');
  await page.goto(url(seed));
  await expect(card(seed.target_a_item_id).locator('.board-card-open')).toHaveAttribute('aria-pressed', 'true');
  const before = (await readBoard(page, seed.board_a.board_id)).document;
  await request.post(`${control}/model/mode/native`);
  await request.post(`${control}/model/block`);
  await board.getByRole('textbox', { name: '针对选中内容提问' }).fill('请把两段的时间、整体平均速度和适用条件连成可以逐步讲解的原生板书。');
  await board.getByRole('button', { name: '发送画板追问', exact: true }).click();
  await expect.poll(async () => {
    const probe = await request.get(`${control}/model/requests`).then(r => r.json()) as Probe;
    rejectToolFailure(probe);
    return probe.waiting;
  }, { timeout: 120_000 }).toBe(true);
  const candidateProbe = await request.get(`${control}/model/requests`).then(response => response.json()) as Probe;
  expect(toolResult(candidateProbe, 'author-preview').status).toBe('preview_ready_for_inspection');
  expect(candidateProbe.requests.some(sample => sample.preview_image_count > 0)).toBe(true);
  const duringPreview = (await readBoard(page, seed.board_a.board_id)).document;
  expect(duringPreview).toEqual(before);
  await request.post(`${control}/model/release`);
  await expect(board.getByRole('button', { name: '定位新回答', exact: true })).toBeVisible({ timeout: 90_000 });
  await board.getByRole('button', { name: '定位新回答', exact: true }).click();
  await expect(pane).toContainText(seed.answer_marker);
  let saved = (await readBoard(page, seed.board_a.board_id)).document;
  const answer = saved.items.find(item => item.target.kind === 'answer' && item.readable_content.includes(seed.answer_marker))!;
  expect(answer).toBeTruthy();
  expect(answer.explanation_blocks.map(block => block.block_id)).toEqual(['native-time', 'native-total', 'native-condition']);
  expect(saved.items.filter(item => item.target.kind === 'drawing')).toHaveLength(3);
  expect(saved.links).toHaveLength(2);
  expect(saved.mappings).toHaveLength(4);
  expect(saved.routes.find(route => route.route_id === 'route-native')?.steps).toHaveLength(3);
  expect(answer.source_bindings).not.toHaveLength(0);

  // Stable native shapes are linked to exact explanation blocks, in both directions.
  await pane.locator('[data-board-block-id="native-time"]').getByRole('button', { name: '在画板中定位', exact: true }).click();
  await expect(card('drawing-native-time')).toHaveClass(/board-highlighted-item/);
  await expect(card('drawing-native-time')).toBeInViewport();
  await expect(pane.locator('[data-board-block-id="native-time"]')).toHaveClass(/block-highlighted/);
  await board.getByRole('button', { name: '显示全部内容', exact: true }).click();
  await card('drawing-native-condition').click();
  await expect(pane.locator('[data-board-block-id="native-condition"]')).toHaveClass(/block-highlighted/);
  await expect(pane).toContainText('只有各段用时相同');
  await page.screenshot({ path: resolve(output, 'desktop-linking.png') });

  // Playback controls operate on persisted route/view state without another model call.
  const requestsBeforePlayback = (await request.get(`${control}/model/requests`).then(r => r.json()) as Probe).requests.length;
  const contentRevision = saved.revision;
  await board.getByRole('combobox', { name: '选择讲解路径', exact: true }).selectOption('route-native');
  const playback = board.getByRole('group', { name: '板书讲解', exact: true });
  await board.getByRole('button', { name: '播放讲解', exact: true }).click();
  await expect(playback).toHaveAttribute('data-playing', 'true');
  await board.getByRole('button', { name: '暂停讲解', exact: true }).click();
  await board.getByRole('combobox', { name: '跳到讲解步骤', exact: true }).selectOption('step-native-0');
  await expect.poll(async () => (await readBoard(page, seed.board_a.board_id)).view.playback?.step_id).toBe('step-native-0');
  await board.getByRole('button', { name: '下一步', exact: true }).click();
  await expect.poll(async () => (await readBoard(page, seed.board_a.board_id)).view.playback?.step_id).toBe('step-native-1');
  await board.getByRole('button', { name: '上一步', exact: true }).click();
  await expect.poll(async () => (await readBoard(page, seed.board_a.board_id)).view.playback?.step_id).toBe('step-native-0');
  await board.getByRole('button', { name: '下一步', exact: true }).click();
  await expect.poll(async () => (await readBoard(page, seed.board_a.board_id)).view.playback?.step_id).toBe('step-native-1');
  await page.reload();
  await expect(board).toBeVisible();
  expect((await readBoard(page, seed.board_a.board_id)).view.playback?.step_id).toBe('step-native-1');
  await board.locator('.board-canvas').hover({ position: { x: 45, y: 100 } });
  await page.mouse.wheel(0, 110);
  await expect.poll(async () => (await readBoard(page, seed.board_a.board_id)).view.playback?.following).toBe(false);
  await board.getByRole('button', { name: '继续跟随', exact: true }).click();
  await expect.poll(async () => (await readBoard(page, seed.board_a.board_id)).view.playback?.following).toBe(true);
  await page.screenshot({ path: resolve(output, 'desktop-playback.png') });
  await board.getByRole('button', { name: '全图', exact: true }).click();
  await expect.poll(async () => (await readBoard(page, seed.board_a.board_id)).view.playback?.show_all).toBe(true);
  expect((await readBoard(page, seed.board_a.board_id)).document.revision).toBe(contentRevision);
  expect((await request.get(`${control}/model/requests`).then(r => r.json()) as Probe).requests.length).toBe(requestsBeforePlayback);

  const presentation = saved.items.find(item => item.target.kind === 'presentation')!;
  await board.getByRole('button', { name: '显示全部内容', exact: true }).click();
  await card(presentation.item_id).locator('.board-card-open').click();
  let frame = page.frameLocator('.board-reading-pane:visible .agent-presentation iframe');
  await expect(frame.locator('#return-speed')).toHaveValue('1');
  await frame.locator('#return-speed').focus();
  await frame.locator('#return-speed').press('Home');
  await frame.locator('#return-speed').press('ArrowRight');
  await expect(frame.locator('#speed-label')).toHaveText('1.5');
  const frameLoads = await pane.locator('iframe').getAttribute('data-load-count');
  const mountedFrame = await pane.locator('iframe').elementHandle();
  await pane.getByRole('button', { name: '展开', exact: true }).click();
  await expect(frame.locator('#return-speed')).toHaveValue('1.5');
  await page.screenshot({ path: resolve(output, 'desktop-presentation-expanded.png') });
  await page.getByRole('button', { name: '收起', exact: true }).click();
  await frame.locator('#result').click();
  await expect(pane.locator('[data-board-block-id="native-total"]')).toHaveClass(/block-highlighted/);
  await expectPresentationRegionVisible(page);
  await page.screenshot({ path: resolve(output, 'desktop-presentation-linked.png') });
  await board.getByRole('button', { name: '显示全部内容', exact: true }).click();
  await card(presentation.item_id).locator('.board-card-open').click();
  frame = page.frameLocator('.board-reading-pane:visible .agent-presentation iframe');
  await expect(frame.locator('#return-speed')).toHaveValue('1.5');
  expect(await pane.locator('iframe').getAttribute('data-load-count')).toBe(frameLoads);
  expect(await mountedFrame!.evaluate(element => element.isConnected)).toBe(true);

  // Delete the old chat through its actual history control, create a new actor chat, and
  // let SSE deliver observe/set/invoke commands to the retained presentation.
  await board.getByRole('button', { name: '关闭画板，回到阅读', exact: true }).click();
  await page.getByRole('button', { name: '问答操作', exact: true }).filter({ visible: true }).click();
  await page.getByTitle('打开对话历史', { exact: true }).filter({ visible: true }).click();
  const history = page.getByRole('dialog', { name: '对话历史', exact: true });
  page.once('dialog', dialog => dialog.accept());
  const deleted = page.waitForResponse(response => response.url().endsWith('/api/agent/history/delete') && response.request().method() === 'POST');
  await history.locator('.history-card.active').getByRole('button', { name: '删除', exact: true }).click();
  expect((await deleted).ok()).toBe(true);
  // Deletion changes Reader ownership. Re-enter that scene and confirm the old
  // history is absent before creating the next actor session.
  await page.goto(url(seed, presentation.item_id));
  await expect(pane).toContainText(presentation.title);
  expect(await page.evaluate(async id => { const path = '/src/api.ts'; return (await (await import(path)).api.agentHistory()).sessions.some(session => session.id === id); }, seed.session_id)).toBe(false);
  await board.getByRole('button', { name: '关闭画板，回到阅读', exact: true }).click();
  await page.getByRole('button', { name: '新对话', exact: true }).filter({ visible: true }).first().click();
  await expect.poll(() => page.evaluate(async () => {
    const path = '/src/network-context.ts'; return (await import(path)).network.value.workspace?.selected_chat;
  })).toBeTruthy();
  await page.goto(url(seed, presentation.item_id));
  await expect(pane).toContainText(presentation.title);
  await expect(page.frameLocator('.board-reading-pane:visible iframe').locator('#return-speed')).toHaveValue('1.5');
  await request.post(`${control}/model/mode/interaction`);
  await board.getByRole('textbox', { name: '针对选中内容提问' }).fill('请在这个保留的演示中把回程速度改为 2，并显示比较结论。');
  await board.getByRole('button', { name: '发送画板追问', exact: true }).click();
  await expect(board.getByRole('button', { name: '定位新回答', exact: true })).toBeVisible({ timeout: 90_000 });
  await expect(page.frameLocator('.board-reading-pane:visible iframe').locator('#result')).toContainText('2.40 m/s');
  await expect(page.frameLocator('.board-reading-pane:visible iframe').locator('#conclusion')).toBeVisible();
  const interactionProbe = await request.get(`${control}/model/requests`).then(response => response.json()) as Probe;
  layoutPages(interactionProbe, 'interaction');
  const observed = toolResult(interactionProbe, 'retained-observe'), applied = toolResult(interactionProbe, 'retained-apply');
  expect(observed.status).toBe('observed'); expect(applied.status).toBe('applied');
  expect(applied.receipt.session_id).toBe(seed.session_id);
  if (presentation.target.kind !== 'presentation') throw new Error('Missing retained presentation');
  expect(applied.receipt.reference).toEqual(presentation.target.receipt.reference);
  saved = (await readBoard(page, seed.board_a.board_id)).document;
  const interactionAnswer = saved.items.find(item => item.target.kind === 'answer' && item.readable_content.includes('新对话继续旧演示'))!;
  expect(interactionAnswer.target.kind).toBe('answer');
  if (interactionAnswer.target.kind === 'answer') expect(interactionAnswer.target.session_id).not.toBe(seed.session_id);

  // A new real learner expression goes through tutor evidence and is retained
  // alongside the exact formal knowledge object in the same authored delivery.
  await page.goto(url(seed));
  await expect(card(seed.target_a_item_id).locator('.board-card-open')).toHaveAttribute('aria-pressed', 'true');
  await request.post(`${control}/model/mode/understanding`);
  await board.getByRole('textbox', { name: '针对选中内容提问' }).fill(seed.new_learner_quote);
  await board.getByRole('button', { name: '发送画板追问', exact: true }).click();
  await expect.poll(async () => {
    const probe = await request.get(`${control}/model/requests`).then(response => response.json()) as Probe;
    rejectToolFailure(probe);
    return (await readBoard(page, seed.board_a.board_id)).document.items.some(item => item.target.kind === 'answer' && item.readable_content.includes(seed.comparison_marker));
  }, { timeout: 120_000 }).toBe(true);
  await expect(board.getByRole('button', { name: '定位新回答', exact: true })).toBeVisible({ timeout: 120_000 });
  await board.getByRole('button', { name: '定位新回答', exact: true }).click();
  await expect(pane).toContainText(seed.comparison_marker);
  saved = (await readBoard(page, seed.board_a.board_id)).document;
  const understanding = saved.items.find(item => item.target.kind === 'understanding' && item.target.evidence_id === seed.new_evidence_id)!;
  expect(understanding).toBeTruthy();
  if (understanding.target.kind !== 'understanding') throw new Error('Missing real new evidence');
  expect(understanding.target.learner_quote).toBe(seed.new_learner_quote);
  expect(understanding.target.interpretation).toContain('等路程与等时间');
  const comparison = saved.items.find(item => item.target.kind === 'answer' && item.readable_content.includes(seed.comparison_marker))!;
  const drawings = saved.items.filter(item => item.item_id.startsWith('drawing-comparison-'));
  expect(drawings).toHaveLength(3);
  const formal = saved.items.find(item => item.item_id === seed.target_a_item_id)!;
  if (formal.target.kind !== 'prebuild') throw new Error('Missing actual formal knowledge object');
  expect(JSON.stringify(formal.target.object_snapshot)).toContain('只有各段用时相同');
  for (const drawing of drawings) if (drawing.target.kind === 'drawing') {
    expect(drawing.target.based_on.map(ref => ref.item_id)).toEqual(expect.arrayContaining([seed.target_a_item_id, understanding.item_id]));
  }
  await pane.locator('[data-board-block-id="comparison-formal-condition"]').getByRole('button', { name: '在画板中定位', exact: true }).click();
  await expect(card('drawing-comparison-formal-condition')).toHaveClass(/board-highlighted-item/);
  await page.screenshot({ path: resolve(output, 'desktop-understanding-comparison.png') });
  await expect.poll(async () => {
    const view = (await readBoard(page, seed.board_a.board_id)).view;
    return { item: view.reading_item_id, block: view.selected_block_id };
  }).toEqual({ item: comparison.item_id, block: 'comparison-formal-condition' });
  const reopening = new URL(page.url());
  reopening.searchParams.delete('board_item_id');
  reopening.searchParams.delete('board_block_id');
  await page.goto(reopening.toString());
  await expect(pane).toContainText(seed.comparison_marker);
  await page.reload();
  await expect(pane).toContainText(seed.comparison_marker);
  await expect(pane.locator('[data-board-block-id="comparison-formal-condition"]')).toHaveClass(/block-highlighted/);
  expect((await readBoard(page, seed.board_a.board_id)).document.items.filter(item => item.item_id === comparison.item_id)).toHaveLength(1);
  await page.setViewportSize({ width: 390, height: 844 });
  const tabs = board.getByRole('group', { name: '画板与全文', exact: true });
  await tabs.getByRole('button', { name: '阅读全文', exact: true }).click();
  await expect(pane).toContainText('正式条件');
  await page.screenshot({ path: resolve(output, 'mobile-reading.png') });
  await tabs.getByRole('button', { name: /^画板/ }).click();
  await board.getByRole('button', { name: '显示全部内容', exact: true }).click();
  await page.screenshot({ path: resolve(output, 'mobile-canvas.png') });
  await tabs.getByRole('button', { name: '阅读全文', exact: true }).click();
  await pane.locator('[data-board-block-id="comparison-formal-condition"]').getByRole('button', { name: '在画板中定位', exact: true }).click();
  await expect.poll(async () => (await card('drawing-comparison-formal-condition').boundingBox())?.width ?? 0).toBeGreaterThanOrEqual(280);
  await expect(card('drawing-comparison-formal-condition')).toBeInViewport({ ratio: 0.95 });
  await expect(card('drawing-comparison-formal-condition')).toHaveClass(/board-highlighted-item/);
  await page.screenshot({ path: resolve(output, 'mobile-canvas-focused.png') });
  await page.goto(url(seed, presentation.item_id));
  await tabs.getByRole('button', { name: '阅读全文', exact: true }).click();
  const retainedMobileFrame = page.frameLocator('.board-reading-pane:visible iframe');
  await expect(retainedMobileFrame.locator('#return-speed')).toHaveValue('2');
  await retainedMobileFrame.locator('#result').click();
  await expect(board.locator('.board-reader-stack.has-region')).toBeVisible();
  await expect(board.locator('.block-highlighted')).toContainText('1.5 m/s');
  await expectPresentationRegionVisible(page);
  await page.screenshot({ path: resolve(output, 'mobile-presentation-linked.png') });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  const model = await request.get(`${control}/model/requests`).then(response => response.json()) as Probe;
  const recoveredLayouts = Object.fromEntries(['native', 'interaction', 'understanding'].map(mode => [mode, layoutPages(model, mode)]));
  expect(toolResult(model, 'new-evidence').evidence_ref).toBe(seed.new_evidence_id);
  writeFileSync(resolve(output, 'result.json'), JSON.stringify({ board_id: seed.board_a.board_id, original_chat_deleted: seed.session_id,
    viewports: { desktop: { width: 1500, height: 1050 }, mobile: { width: 390, height: 844 } },
    native_answer_item_id: answer.item_id, understanding_item_id: understanding.item_id, comparison_answer_item_id: comparison.item_id,
    original_revision: before.revision, native_committed_revision: contentRevision, retained_revision: saved.revision,
    preview: toolResult(candidateProbe, 'author-preview'), understanding_preview: toolResult(model, 'author-preview'), new_evidence: toolResult(model, 'new-evidence'),
    preview_image_counts: model.requests.map(sample => ({ mode: sample.mode, stage: sample.stage, images: sample.preview_image_count })),
    observed, applied, recoveredLayouts, networkCommands, final_board: saved, errors, httpFailures, optionalSourceManifestMisses }, null, 2));
  expect(errors).toEqual([]); expect(httpFailures).toEqual([]);
});
