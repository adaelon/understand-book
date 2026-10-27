import { expect, test, type Page } from '@playwright/test';
import { boardFixture, boardFixtureCatalog, boardFixtureItems, boardFixturePresentation, boardFixtureReceipt } from '../src/fixtures/board-workspace';
import type { PresentationState } from '../src/generated/PresentationState';
import type { BoardQuestionRequest } from '../src/board-workspace';

async function serveBoard(page: Page) {
  const saved = boardFixture();
  let state: PresentationState | null = null;
  let stateRevision = 1;
  const requests: string[] = [];
  await page.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname;
    const body = route.request().postDataJSON() ?? {};
    requests.push(path);
    let response: unknown = {};
    if (path.endsWith('/boards/list')) response = { items: [{ ...saved.document, item_count: saved.document.items.length }] };
    else if (path.endsWith('/boards/read')) response = saved;
    else if (path.endsWith('/boards/search')) response = { items: saved.document.items.filter(item => `${item.title}\n${item.readable_content}`.includes(body.query)).map(item => ({ board_id: saved.document.board_id, board_revision: saved.document.revision, board_title: saved.document.title, item_id: item.item_id, block_id: item.block_id ?? null, kind: item.target.kind, title: item.title, snippet: item.readable_content })) };
    else if (path.endsWith('/boards/related')) response = { items: [{ board_id: saved.document.board_id, board_revision: saved.document.revision, board_title: saved.document.title, item_id: 'item-prebuild', block_id: null, kind: 'prebuild', title: boardFixtureItems[0].title, snippet: '' }] };
    else if (path.endsWith('/boards/update')) {
      if (body.board_revision !== saved.document.revision) { await route.fulfill({ status: 409, json: { message: '画板已更新，请重新打开', code: 'BOARD_REVISION_CONFLICT' } }); return; }
      saved.document.revision++;
      if (body.title) saved.document.title = body.title;
      for (const position of body.positions ?? []) Object.assign(saved.document.items.find(item => item.item_id === position.item_id)!, { position: position.position });
      saved.document.items = saved.document.items.filter(item => !body.remove_item_ids?.includes(item.item_id));
      response = saved.document;
    } else if (path.endsWith('/boards/view.save')) { saved.view = { ...saved.view, ...body.view, presentation_states: saved.view.presentation_states }; response = { saved: true }; }
    else if (path.endsWith('/boards/catalog')) response = { items: boardFixtureCatalog.filter(item => item.kind === body.kind), next: null };
    else if (path.endsWith('/boards/import')) {
      saved.document.revision++;
      const candidate = structuredClone(boardFixtureItems.find(item => item.target.kind === body.source.kind)!);
      candidate.item_id += `-${saved.document.revision}`; saved.document.items.push(candidate); response = saved.document;
    } else if (path.endsWith('/boards/presentation.read')) response = { ...boardFixturePresentation(), restored_state: state, restored_state_revision: state ? stateRevision : null };
    else if (path.endsWith('/boards/presentation.state')) { state = body.state; stateRevision++; response = { ...boardFixtureReceipt, state_revision: stateRevision, saved_state_ref: `scene-${stateRevision}` }; }
    else if (path.endsWith('/boards/presentation.observe')) response = { accepted: true };
    else if (path.endsWith('/boards/presentation.restore')) response = { values: { controls: [], page: { prior: 1 } }, observed_result: '8.3%', visible_step: 'compare', source_ref_ids: ['source-bayes'] };
    else { await route.fulfill({ status: 404, json: { message: path } }); return; }
    await route.fulfill({ json: response });
  });
  return { saved, requests, state: () => state };
}

test('WB1 saves four content types, layout, full answer and live presentation; reopening and source return preserve the workspace', async ({ page }, info) => {
  await page.setViewportSize({ width: 1440, height: 960 });
  const fixture = await serveBoard(page);
  const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto('/board-workspace-visual.html');
  await expect(page.locator('.board-card')).toHaveCount(4);
  await expect(page.locator('.board-reading-pane:visible')).toContainText('基准发生率改变，同样的检测结果也会得到不同的后验判断。');
  await page.screenshot({ path: info.outputPath('board-desktop.png') });
  const header = page.locator('[data-item-id="item-answer"] .board-card-label');
  const box = (await header.boundingBox())!;
  await page.mouse.move(box.x + 100, box.y + 7); await page.mouse.down(); await page.mouse.move(box.x + 150, box.y + 42, { steps: 5 }); await page.mouse.up();
  await expect.poll(() => fixture.saved.document.items[1].position.x).toBeGreaterThan(320);
  await page.getByRole('textbox', { name: '画板名称', exact: true }).fill('贝叶斯：我的持续理解'); await page.getByRole('textbox', { name: '画板名称', exact: true }).press('Enter');
  await expect.poll(() => fixture.saved.document.title).toBe('贝叶斯：我的持续理解');
  await page.locator('[data-item-id="item-presentation"] .board-card-open').click();
  const frame = page.frameLocator('.board-reading-pane:visible iframe');
  await expect(frame.locator('#prior')).toBeVisible();
  await frame.locator('#prior').focus(); await page.keyboard.press('End');
  await expect(frame.locator('#posterior')).toHaveText('90.0%');
  await expect.poll(() => (fixture.state()?.values as any)?.page?.prior).toBe(50);
  await page.locator('[data-item-id="item-understanding"] .board-card-open').click();
  await expect(page.locator('.board-reading-pane:visible .board-understanding-quote')).toContainText('阳性的人里面，也会有健康的人');
  await page.locator('[data-item-id="item-presentation"] .board-card-open').click();
  await expect(frame.locator('#prior')).toHaveValue('50');
  await expect(page.locator('iframe')).toHaveAttribute('data-load-count', '1');
  await page.locator('[data-item-id="item-understanding"] .board-card-open').click();
  await page.locator('.board-reading-pane:visible .board-reading-sources button').click();
  await expect(page.getByTestId('board-returned-source')).toContainText('已定位原文：source-bayes');
  await page.getByRole('button', { name: '返回画板', exact: true }).click();
  await page.locator('[data-item-id="item-presentation"] .board-card-open').click();
  await expect(frame.locator('#prior')).toHaveValue('50');
  await expect.poll(() => fixture.saved.view.selected_item_id).toBe('item-presentation');
  await page.reload();
  await expect(page.locator('.board-card')).toHaveCount(4);
  await expect(page.getByRole('textbox', { name: '画板名称', exact: true })).toHaveValue('贝叶斯：我的持续理解');
  await expect(page.frameLocator('.board-reading-pane:visible iframe').locator('#prior')).toHaveValue('50');
  await page.screenshot({ path: info.outputPath('board-presentation-reopened.png') });
  expect(errors).toEqual([]);
  expect(fixture.requests.some(path => path.startsWith('/api/agent/'))).toBe(false);
});

test('mobile card dragging stays on the canvas, reading and demo state survive view switches, and English tabs fit', async ({ page }, info) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const fixture = await serveBoard(page);
  await page.goto('/board-workspace-visual.html');
  await expect(page.locator('.board-canvas')).toBeVisible();
  const header = page.locator('[data-item-id="item-prebuild"] .board-card-label');
  const box = (await header.boundingBox())!;
  await page.mouse.move(box.x + 70, box.y + 5); await page.mouse.down(); await page.mouse.move(box.x + 96, box.y + 28, { steps: 5 }); await page.mouse.up();
  await expect(page.locator('.board-canvas')).toBeVisible();
  await expect.poll(() => fixture.saved.document.items[0].position.x).toBeGreaterThan(0);
  await page.getByRole('button', { name: '显示全部内容', exact: true }).click();
  await page.screenshot({ path: info.outputPath('board-mobile-canvas.png') });
  await page.locator('[data-item-id="item-presentation"] .board-card-open').click();
  const frame = page.frameLocator('.board-reading-pane:visible iframe');
  await expect(frame.locator('#prior')).toBeVisible();
  await frame.locator('#prior').focus(); await page.keyboard.press('End');
  await expect(frame.locator('#posterior')).toHaveText('90.0%');
  await page.locator('.board-mobile-switch button').first().click();
  await expect(page.locator('.board-canvas')).toBeVisible();
  await page.locator('.board-mobile-switch button').last().click();
  await expect(frame.locator('#prior')).toHaveValue('50');
  await expect(page.locator('iframe')).toHaveAttribute('data-load-count', '1');
  await page.screenshot({ path: info.outputPath('board-mobile-reading.png') });
  await page.goto('/board-workspace-visual.html?lang=en');
  await page.locator('.board-add-button').click();
  await expect(page.locator('.board-kind-tabs')).toBeVisible();
  const overflow = await page.locator('.board-add-dialog').evaluate(node => node.scrollWidth > node.clientWidth);
  expect(overflow).toBe(false);
  await page.screenshot({ path: info.outputPath('board-mobile-english.png') });
});

test('WB2 freezes the exact demo scene, delivers beside the parent without moving the reader, and locates full answer searches', async ({ page }, info) => {
  await page.setViewportSize({ width: 1440, height: 960 });
  const fixture = await serveBoard(page);
  const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
  let received: BoardQuestionRequest | undefined;
  let complete!: () => void;
  const completion = new Promise<void>(resolve => { complete = resolve; });
  await page.route('**/api/fixture/board-follow-up', async route => {
    received = route.request().postDataJSON(); await completion;
    const parent = fixture.saved.document.items.find(item => item.item_id === received!.context.item_id)!;
    const before = fixture.saved.document.revision;
    fixture.saved.document.revision++;
    const position = { x: parent.position.x + parent.size.width + 72, y: parent.position.y };
    for (;;) {
      const collision = fixture.saved.document.items.find(item => position.x < item.position.x + item.size.width && position.x + 280 > item.position.x && position.y < item.position.y + item.size.height && position.y + 220 > item.position.y);
      if (!collision) break;
      position.y = collision.position.y + collision.size.height + 32;
    }
    fixture.saved.document.items.push({ ...structuredClone(boardFixtureItems[1]), item_id: 'answer-follow-up', block_id: 'block-answer-follow-up', reply_to_item_id: parent.item_id, title: '先验上升时，阳性的含义为什么改变', readable_content: '## 根据你刚才的现场\n\n同样的证据遇到不同的起点，会得到不同的后验判断。\n\n完整解释保留这个搜索词：条件独立不是无条件独立。', target: { kind: 'answer', session_id: 'board-chat', turn_id: 'board-turn', question: received!.message, answer_parts: [] }, position });
    const delivery = { board_id: fixture.saved.document.board_id, before_revision: before, after_revision: fixture.saved.document.revision, item_ids: ['answer-follow-up'], block_ids: ['block-answer-follow-up'], origin_session_id: 'board-chat', origin_turn_id: 'board-turn', parent_item_id: parent.item_id };
    fixture.saved.document.deliveries.push(delivery);
    await route.fulfill({ json: delivery });
  });
  await page.goto('/board-workspace-visual.html');
  await page.locator('[data-item-id="item-presentation"] .board-card-open').click();
  const frame = page.frameLocator('.board-reading-pane:visible iframe');
  await expect(frame.locator('#prior')).toBeVisible();
  await frame.locator('#prior').focus(); await page.keyboard.press('End');
  await expect.poll(() => (fixture.state()?.values as any)?.page?.prior).toBe(50);
  await page.getByRole('textbox', { name: '针对选中内容提问' }).fill('请解释我刚才调到的现场');
  await page.getByRole('button', { name: '发送画板追问' }).click();
  await expect.poll(() => received?.context.item_id).toBe('item-presentation');
  expect(received?.context.presentation?.saved_state_ref).toMatch(/^scene-/);
  const frozenReceipt = structuredClone(received!.context.presentation);
  await expect(page.getByRole('textbox', { name: '针对选中内容提问' })).toHaveValue('');
  await frame.locator('#prior').focus(); await page.keyboard.press('Home');
  await expect.poll(() => (fixture.state()?.values as any)?.page?.prior).toBe(1);
  await page.locator('[data-item-id="item-understanding"] .board-card-open').click();
  await expect(page.locator('.board-understanding-quote:visible')).toBeVisible();
  await page.screenshot({ path: info.outputPath('board-wb2-running-desktop.png') });
  complete();
  await expect(page.locator('.board-card')).toHaveCount(5);
  await expect(page.locator('[data-item-id="item-understanding"] .board-card-open')).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('iframe')).toHaveAttribute('data-load-count', '1');
  expect(received?.context.presentation).toEqual(frozenReceipt);
  await expect(page.locator('[data-board-reply-link="answer-follow-up"]')).toHaveCount(1);
  await page.getByRole('button', { name: '定位新回答', exact: true }).click();
  await expect(page.locator('.board-located [data-board-block-id="block-answer-follow-up"]')).toContainText('条件独立不是无条件独立');
  await page.screenshot({ path: info.outputPath('board-wb2-delivered-desktop.png') });
  await page.locator('.board-reply-parent').click();
  await expect(frame.locator('#prior')).toHaveValue('1');
  await page.getByRole('button', { name: '搜索画板与全文', exact: true }).click();
  await page.getByRole('searchbox', { name: '搜索画板与全文', exact: true }).fill('条件独立不是无条件独立');
  await expect(page.locator('.board-search-result')).toHaveCount(1);
  await page.locator('.board-search-result').click();
  await expect(page.locator('.board-located [data-board-block-id="block-answer-follow-up"]')).toBeVisible();
  await expect.poll(() => fixture.saved.view.selected_item_id).toBe('answer-follow-up');
  await page.reload();
  await expect(page.locator('[data-board-reply-link="answer-follow-up"]')).toHaveCount(1);
  await expect(page.locator('.board-reading-pane:visible')).toContainText('条件独立不是无条件独立');
  expect(errors).toEqual([]);
});

test('WB2 mobile search and linked prebuild locations leave room for the selected-object question', async ({ page }, info) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const fixture = await serveBoard(page);
  await page.goto('/board-workspace-visual.html');
  await page.getByRole('button', { name: '搜索画板与全文', exact: true }).click();
  await page.getByRole('searchbox', { name: '搜索画板与全文', exact: true }).fill('基准发生率改变');
  await page.locator('.board-search-result').click();
  await expect(page.locator('.board-located [data-board-block-id="block-item-answer"]')).toContainText('基准发生率改变');
  await expect(page.getByRole('textbox', { name: '针对选中内容提问' })).toBeVisible();
  await page.screenshot({ path: info.outputPath('board-wb2-mobile-reading.png') });
  expect(await page.locator('.board-workspace').evaluate(node => node.scrollWidth > node.clientWidth)).toBe(false);
  await page.getByRole('button', { name: '添加内容', exact: true }).click();
  expect(fixture.requests.filter(path => path.endsWith('/boards/related'))).toHaveLength(0);
  await page.getByRole('button', { name: '查找已关联的画板', exact: true }).click();
  await page.locator('.board-related-results button').click();
  await expect(page.locator('.board-reading-pane:visible')).toContainText('来自材料预构建');
  await expect(page.getByRole('dialog', { name: '添加到画板' })).toHaveCount(0);
});
