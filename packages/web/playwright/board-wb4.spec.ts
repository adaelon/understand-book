import { expect, test, type APIRequestContext, type Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import type { BoardDocument } from '../src/generated/BoardDocument';
import type { BoardDelivery } from '../src/generated/BoardDelivery';
import type { BoardItem } from '../src/generated/BoardItem';
import type { BoardView } from '../src/generated/BoardView';
import type { RunSnapshot } from '../src/agent-run-state';

const control = process.env.WB4_CONTROL_URL ?? 'http://127.0.0.1:4231';
const output = resolve('../../tmp/board-wb4/browser');
interface Fixture {
  publication: { book_id: string; publication_id: string }; learning_workspace_id: string;
  board_a: BoardDocument; board_b: BoardDocument; target_a_item_id: string; board_b_item_id: string;
  first_marker: string; second_marker: string;
  presentation_execution_environment: { kind: string; production_linux_sandbox: boolean; browser: string; capability: Record<string, unknown>; probe: string };
}
interface Probe {
  mode: string; waiting: string | null;
  requests: Array<{ mode: string; stage: number; preview_image_count: number;
    messages: Array<{ content?: string | null; tool_call_id?: string | null }> }>;
}
const terminal = ['completed', 'cancelled', 'failed', 'ended', 'interrupted'];
function url(seed: Fixture, board = seed.board_a, item = seed.target_a_item_id) {
  return `/wb4-acceptance?${new URLSearchParams({ board_id: board.board_id, board_item_id: item,
    board_book_id: seed.publication.book_id, board_publication_id: seed.publication.publication_id,
    board_learning_workspace_id: seed.learning_workspace_id })}`;
}
async function readBoard(page: Page, id: string): Promise<{ document: BoardDocument; view: BoardView }> {
  return page.evaluate(async boardId => { const path = '/src/api.ts'; return (await import(path)).api.boardRead(boardId); }, id);
}
async function snapshot(page: Page, turn: string): Promise<RunSnapshot> {
  return page.evaluate(async turnId => { const path = '/src/api.ts'; return (await import(path)).api.agentRun(turnId); }, turn);
}
async function probe(request: APIRequestContext) { return request.get(`${control}/model/requests`).then(r => r.json()) as Promise<Probe>; }
function toolResult(value: Probe, mode: string, id: string): any {
  for (const sample of [...value.requests].reverse().filter(sample => sample.mode === mode)) {
    const message = [...sample.messages].reverse().find(message => message.tool_call_id === id);
    if (message?.content) {
      const parsed = JSON.parse(message.content);
      // Later model sampling may retain only the historical receipt. Inspect
      // the original nonempty tool body recorded by an earlier real sampling.
      if (parsed.version === 'tool_result_envelope.v1' && parsed.model_body == null) continue;
      return parsed.model_body ?? parsed;
    }
  }
  throw new Error(`No actual ${mode} tool result: ${id}`);
}
function rejectFailures(value: Probe, mode: string) {
  for (const sample of value.requests.filter(sample => sample.mode === mode)) for (const message of sample.messages) {
    if (!message.tool_call_id || !message.content) continue;
    let parsed: any; try { parsed = JSON.parse(message.content); } catch { continue; }
    const result = parsed.model_body ?? parsed;
    if (parsed.status === 'error' || result.status === 'preview_failed') throw new Error(`${message.tool_call_id}: ${JSON.stringify(result)}`);
  }
}
async function checkpoint(request: APIRequestContext, mode: string, name: string) {
  await expect.poll(async () => { const value = await probe(request); rejectFailures(value, mode); return value.waiting; }, { timeout: 150_000 }).toBe(name);
  return probe(request);
}
function committed(value: Probe, mode: string, part: number): BoardDelivery {
  const result = toolResult(value, mode, `part-${part}-commit`);
  expect(result.status).toBe('board_part_committed');
  expect(result.delivery.commit_id).toBe(result.candidate_id);
  return result.delivery;
}
function retainedPresentation(document: BoardDocument, delivery: BoardDelivery) {
  const item = document.items.find(item => delivery.item_ids.includes(item.item_id) && item.target.kind === 'presentation');
  expect(item).toBeTruthy(); return item!;
}
function retainedAnswer(document: BoardDocument, delivery: BoardDelivery) {
  const item = document.items.find(item => delivery.item_ids.includes(item.item_id) && item.target.kind === 'answer');
  expect(item).toBeTruthy(); return item!;
}

test.afterEach(async ({ page, request }, info) => {
  await request.post(`${control}/model/release`).catch(() => undefined);
  if (info.status !== info.expectedStatus) {
    mkdirSync(output, { recursive: true });
    await page.screenshot({ path: resolve(output, 'failure.png'), fullPage: true });
    writeFileSync(resolve(output, 'failure.txt'), await page.locator('body').innerText());
    writeFileSync(resolve(output, 'failure-model.json'), JSON.stringify(await probe(request).catch(() => null), null, 2));
  }
});

test('WB4 real partial delivery, running recovery, cancellation and idempotent continued authoring', async ({ page, context, request }) => {
  const startedAt = new Date().toISOString();
  mkdirSync(output, { recursive: true }); page.setDefaultTimeout(25_000);
  const seed = await request.get(`${control}/fixture`).then(response => response.json()) as Fixture;
  const errors: string[] = [], httpFailures: string[] = [], optionalSourceManifestMisses: string[] = [];
  const runs: RunSnapshot[] = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('response', async response => {
    const path = new URL(response.url()).pathname;
    if (!path.startsWith('/api/') && !path.startsWith('/books/')) return;
    if (response.status() === 404 && path.endsWith('/source_manifest')) optionalSourceManifestMisses.push(path);
    else if (response.status() >= 400) httpFailures.push(`${response.status()} ${response.request().method()} ${path}: ${await response.text().catch(() => '')}`);
  });
  // Mount the actual app. API/SSE traffic is forwarded by Vite without mocking.
  await context.route(/\/wb4-acceptance(?:\?.*)?$/, route => route.fulfill({ contentType: 'text/html', body:
    `<!doctype html><html lang="zh-CN"><head><meta name="viewport" content="width=device-width, initial-scale=1"></head><body><div id="app"></div><script type="module">import {createApp} from '/node_modules/.vite/deps/vue.js'; import NetworkApp from '/src/NetworkApp.vue'; import '/src/style.css'; createApp(NetworkApp).mount('#app');</script></body></html>` }));
  await page.goto(url(seed));
  const board = page.getByRole('region', { name: '知识画板工作区', exact: true });
  const pane = board.locator('.board-reading-pane:visible');
  const card = (id: string) => board.locator(`[data-item-id="${id}"]`);
  const partButton = (part: number) => board.locator(`[data-board-part="${part}"]`);
  const openPresentation = async (item: BoardItem) => {
    await board.getByRole('button', { name: '显示全部内容', exact: true }).click();
    await card(item.item_id).locator('.board-card-open').click();
    await expect(pane.locator('iframe')).toBeVisible();
    return page.frameLocator('.board-reading-pane:visible iframe');
  };
  const send = async (message: string) => {
    await board.getByRole('textbox', { name: '针对选中内容提问' }).fill(message);
    await board.getByRole('button', { name: '发送画板追问', exact: true }).click();
  };
  await expect(board).toBeVisible();
  await board.getByRole('button', { name: '关闭画板，回到阅读', exact: true }).click();
  const teaching = page.getByRole('switch', { name: '连续教学', exact: true });
  await expect(teaching).toHaveAttribute('aria-checked', 'true'); await teaching.click();
  await expect(teaching).toHaveAttribute('aria-checked', 'false');
  await page.goto(url(seed)); await expect(board).toBeVisible();

  // A: a real committed part is usable while its author continues generating.
  await request.post(`${control}/model/mode/cancel`);
  await send('请分两部分讲解，先交付两段时间实验，再继续解释直接平均的条件。');
  const firstProbe = await checkpoint(request, 'cancel', 'first-committed');
  await expect(partButton(1)).toBeVisible();
  await expect(board.locator('.board-run-status')).toContainText('后续仍在讲解');
  await partButton(1).click(); await expect(pane).toContainText(seed.first_marker);
  await expect(pane.locator('.board-reading-heading > p')).toHaveText('分段讲解 · 已保存在画板中');
  const a1 = committed(firstProbe, 'cancel', 1);
  const firstSaved = (await readBoard(page, seed.board_a.board_id)).document;
  const firstAnswer = retainedAnswer(firstSaved, a1), firstPresentation = retainedPresentation(firstSaved, a1);
  expect(firstSaved.items).toHaveLength(seed.board_a.items.length + 3);
  const running = await snapshot(page, a1.origin_turn_id); runs.push(running);
  expect(terminal).not.toContain(running.execution_state); expect(running.board_commits).toEqual([a1]);
  let frame = await openPresentation(firstPresentation);
  await expect(frame.locator('#return-speed')).toHaveValue('1');
  await frame.locator('#return-speed').focus(); await frame.locator('#return-speed').press('ArrowRight');
  await expect(frame.locator('#return-speed')).toHaveValue('1.5');
  await expect(board.getByText('有未保存的修改', { exact: true })).toHaveCount(0);
  await page.screenshot({ path: resolve(output, 'desktop-delivered-still-generating.png') });
  await page.setViewportSize({ width: 390, height: 844 });
  await board.getByRole('group', { name: '画板与全文', exact: true }).getByRole('button', { name: '阅读全文', exact: true }).click();
  await expect(frame.locator('#return-speed')).toHaveValue('1.5');
  await page.screenshot({ path: resolve(output, 'mobile-delivered-still-generating.png') });
  await page.setViewportSize({ width: 1500, height: 1050 });
  // Reload closes the live SSE connection; the real snapshot and durable turn restore it.
  await page.reload(); await expect(board).toBeVisible(); await expect(partButton(1)).toBeVisible();
  await expect(board.locator('.board-run-status')).toContainText('后续仍在讲解');
  const recovered = await snapshot(page, a1.origin_turn_id); runs.push(recovered);
  expect(terminal).not.toContain(recovered.execution_state); expect(recovered.board_commits).toEqual([a1]);
  expect((await readBoard(page, seed.board_a.board_id)).document).toEqual(firstSaved);
  await request.post(`${control}/model/release`);
  const draftProbe = await checkpoint(request, 'cancel', 'second-candidate-previewed');
  expect(toolResult(draftProbe, 'cancel', 'part-2-preview').status).toBe('preview_ready_for_inspection');
  expect(toolResult(draftProbe, 'cancel', 'part-1-retry').already_committed).toBe(true);
  expect((await readBoard(page, seed.board_a.board_id)).document).toEqual(firstSaved);
  await board.locator('.board-run-status').getByRole('button', { name: '停止', exact: true }).click();
  await request.post(`${control}/model/release`);
  await expect.poll(async () => (await snapshot(page, a1.origin_turn_id)).execution_state).toBe('cancelled');
  await expect.poll(async () => (await snapshot(page, a1.origin_turn_id)).persistence_state).toBe('saved');
  const cancelled = await snapshot(page, a1.origin_turn_id); runs.push(cancelled);
  expect(cancelled.board_commits).toEqual([a1]); expect(cancelled.final_view?.board_commits).toEqual([a1]);
  expect((await readBoard(page, seed.board_a.board_id)).document).toEqual(firstSaved);
  await partButton(1).click(); await expect(pane).toContainText(seed.first_marker);
  await expect(board.locator('.board-saved-parts')).toContainText('讲解尚未全部完成');
  await expect(board.locator('.board-run-status')).toContainText('已停止');
  await expect(board.locator('.board-run-status')).toHaveAttribute('role', 'status');
  await expect(board.locator('.board-run-status')).not.toHaveClass(/failed/);
  await expect(board.locator('.board-run-status')).not.toContainText('Agent run was cancelled');
  await expect(pane.locator('.board-reading-heading > p')).toHaveText('分段讲解 · 已保存在画板中');
  await page.screenshot({ path: resolve(output, 'desktop-cancelled-keeps-first-part.png') });
  await page.setViewportSize({ width: 390, height: 844 });
  await board.getByRole('group', { name: '画板与全文', exact: true }).getByRole('button', { name: '阅读全文', exact: true }).click();
  await page.screenshot({ path: resolve(output, 'mobile-cancelled-keeps-first-part.png') });
  await page.setViewportSize({ width: 1500, height: 1050 });
  await board.getByRole('button', { name: '关闭画板，回到阅读', exact: true }).click();
  const historicalTurn = page.locator(`[data-turn-id="${a1.origin_turn_id}"]`).filter({ visible: true });
  await expect(historicalTurn.locator('.board-parts-progress')).toContainText('讲解尚未全部完成');
  await expect(historicalTurn.locator('.board-part-link')).toHaveCount(1);

  // B: two formal parts, idempotent retry and a later exact presentation version.
  await page.goto(url(seed, seed.board_b, seed.board_b_item_id)); await expect(board).toBeVisible();
  await request.post(`${control}/model/mode/complete`);
  await send('请分两部分接着讲解，保留每一部分和实验版本，让我边操作边阅读。');
  const bFirstProbe = await checkpoint(request, 'complete', 'first-committed');
  await expect(partButton(1)).toBeVisible();
  const b1 = committed(bFirstProbe, 'complete', 1);
  const bFirst = (await readBoard(page, seed.board_b.board_id)).document;
  const bFirstAnswer = retainedAnswer(bFirst, b1), v1 = retainedPresentation(bFirst, b1);
  frame = await openPresentation(v1);
  await frame.locator('#return-speed').focus(); await frame.locator('#return-speed').press('ArrowRight');
  await expect(frame.locator('#return-speed')).toHaveValue('1.5');
  const iframe = await pane.locator('iframe').elementHandle();
  const loadCount = await pane.locator('iframe').getAttribute('data-load-count');
  // A real pointer drag between parts changes the board revision and must survive.
  const parentBefore = bFirst.items.find(item => item.item_id === seed.board_b_item_id)!;
  const handle = await card(parentBefore.item_id).locator('.board-card-label').boundingBox(); expect(handle).toBeTruthy();
  await page.mouse.move(handle!.x + 40, handle!.y + handle!.height / 2); await page.mouse.down();
  await page.mouse.move(handle!.x + 96, handle!.y + handle!.height / 2 + 36, { steps: 6 }); await page.mouse.up();
  await expect.poll(async () => (await readBoard(page, seed.board_b.board_id)).document.items.find(item => item.item_id === parentBefore.item_id)!.position.x).not.toBe(parentBefore.position.x);
  const moved = (await readBoard(page, seed.board_b.board_id)).document.items.find(item => item.item_id === parentBefore.item_id)!.position;
  // Dragging selects the moved card. Reopen the already mounted v1 before new content arrives.
  await card(v1.item_id).locator('.board-card-open').click(); await expect(frame.locator('#return-speed')).toHaveValue('1.5');
  await request.post(`${control}/model/release`);
  const bSecondProbe = await checkpoint(request, 'complete', 'second-committed');
  await expect(partButton(2)).toBeVisible(); await expect(board.locator('.board-run-status')).toContainText('后续仍在讲解');
  const b2 = committed(bSecondProbe, 'complete', 2);
  const both = (await readBoard(page, seed.board_b.board_id)).document;
  const bSecondAnswer = retainedAnswer(both, b2), v2 = retainedPresentation(both, b2);
  expect(bFirstAnswer.item_id).not.toBe(bSecondAnswer.item_id);
  expect(toolResult(bSecondProbe, 'complete', 'part-1-retry').delivery).toEqual(b1);
  expect(toolResult(bSecondProbe, 'complete', 'part-2-read').reserved_answer_item_id).toBe(bSecondAnswer.item_id);
  expect(both.items).toHaveLength(seed.board_b.items.length + 6);
  expect(both.items.find(item => item.item_id === parentBefore.item_id)!.position).toEqual(moved);
  expect(both.deliveries.filter(item => item.origin_turn_id === b1.origin_turn_id)).toEqual([b1, b2]);
  if (v1.target.kind !== 'presentation' || v2.target.kind !== 'presentation') throw new Error('Expected exact presentation references');
  expect(v2.target.receipt.reference.presentation_id).toBe(v1.target.receipt.reference.presentation_id);
  expect(v2.target.receipt.reference.revision).toBe(v1.target.receipt.reference.revision + 1);
  await expect(frame.locator('#edition')).toContainText('第 1 版'); await expect(frame.locator('#return-speed')).toHaveValue('1.5');
  expect(await iframe!.evaluate(element => element.isConnected)).toBe(true);
  expect(await pane.locator('iframe').getAttribute('data-load-count')).toBe(loadCount);
  await page.screenshot({ path: resolve(output, 'desktop-two-parts-preserve-live-v1.png') });
  await request.post(`${control}/model/release`);
  await expect.poll(async () => (await snapshot(page, b1.origin_turn_id)).execution_state).toBe('completed');
  await expect.poll(async () => (await snapshot(page, b1.origin_turn_id)).persistence_state).toBe('saved');
  const completed = await snapshot(page, b1.origin_turn_id); runs.push(completed);
  expect(completed.board_commits).toEqual([b1, b2]); expect(completed.final_view?.board_commits).toEqual([b1, b2]);
  const final = (await readBoard(page, seed.board_b.board_id)).document;
  expect(final).toEqual(both); // The ordinary final text must not append a third answer.
  await expect(frame.locator('#return-speed')).toHaveValue('1.5');
  await partButton(2).click(); await expect(pane).toContainText(seed.second_marker);
  await expect(pane.locator('.board-reading-heading > p')).toHaveText('分段讲解 · 已保存在画板中');
  await expect(board.getByText('有未保存的修改', { exact: true })).toHaveCount(0);
  await page.screenshot({ path: resolve(output, 'desktop-final-two-parts.png') });
  await page.setViewportSize({ width: 390, height: 844 });
  await board.getByRole('group', { name: '画板与全文', exact: true }).getByRole('button', { name: '阅读全文', exact: true }).click();
  await page.screenshot({ path: resolve(output, 'mobile-final-two-parts.png') });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)).toBe(true);
  // Recover both receipts and the saved reading block through a plain board URL.
  await expect.poll(async () => (await readBoard(page, seed.board_b.board_id)).view.reading_item_id).toBe(bSecondAnswer.item_id);
  const reopen = new URL(page.url()); reopen.searchParams.delete('board_item_id'); reopen.searchParams.delete('board_block_id');
  await page.goto(reopen.toString()); await expect(partButton(1)).toBeVisible(); await expect(partButton(2)).toBeVisible();
  await board.getByRole('group', { name: '画板与全文', exact: true }).getByRole('button', { name: '阅读全文', exact: true }).click();
  await expect(pane).toContainText(seed.second_marker); expect((await readBoard(page, seed.board_b.board_id)).document).toEqual(final);
  const finalProbe = await probe(request); rejectFailures(finalProbe, 'complete');
  for (const mode of ['cancel', 'complete']) for (const part of [1, 2]) {
    expect(toolResult(finalProbe, mode, `part-${part}-preview`).status).toBe('preview_ready_for_inspection');
    expect(finalProbe.requests.some(sample => sample.mode === mode && sample.preview_image_count > 0 && sample.messages.some(message => message.tool_call_id === `part-${part}-preview`))).toBe(true);
  }
  expect(errors).toEqual([]); expect(httpFailures).toEqual([]);
  writeFileSync(resolve(output, 'result.json'), JSON.stringify({ status: 'passed', startedAt, completedAt: new Date().toISOString(),
    presentation_execution_environment: seed.presentation_execution_environment,
    desktop: { width: 1500, height: 1050 }, mobile: { width: 390, height: 844 },
    cancelled: { delivery: a1, answer: firstAnswer, board: firstSaved }, completed: { deliveries: [b1, b2], moved, v1, v2, board: final },
    runs, errors, httpFailures, optionalSourceManifestMisses, model: finalProbe }, null, 2));
});
