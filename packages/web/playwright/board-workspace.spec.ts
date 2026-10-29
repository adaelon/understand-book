import { expect, test, type Page } from '@playwright/test';
import { boardFixture, boardFixtureCatalog, boardFixtureItems, boardFixturePresentation, boardFixtureReceipt } from '../src/fixtures/board-workspace';
import type { PresentationState } from '../src/generated/PresentationState';
import type { BoardQuestionRequest } from '../src/board-workspace';

const sourceChapterTitles = ['第一章 · 概率与观察', '第二章 · 条件概率与贝叶斯定理', '第三章 · 用证据更新判断'];
const sourceReaderSegments = sourceChapterTitles.flatMap((title, chapter) =>
  Array.from({ length: 41 }, (_, paragraph) => {
    const lid = `${chapter + 1}.${paragraph + 1}`;
    const quote = boardFixtureItems[0].source_bindings[0].preview_snapshot;
    const prefix = '新证据会改变我们对事件的判断。\n\n';
    return {
      lid, kind: paragraph === 0 ? 'heading' : 'paragraph',
      text: paragraph === 0 ? `## ${title}` : lid === '2.3'
        ? `${prefix}${quote}\n\n因此，解释检测结果时，要同时考虑患病人群与健康人群。`
        : `第 ${chapter + 1} 章第 ${paragraph + 1} 段。观察一个检测结果之前，先考虑人群中事件原本发生的比例；得到新的证据之后，再比较两种解释产生这个结果的可能性。这样可以把抽象的概率，放回实际问题的条件之中。\n\n阅读时可以停下来想一想：这里的分母代表哪一群人？同一份证据是否会因为起点不同而带来不同的判断？`,
      highlights: lid === '2.3' ? [{ start: prefix.length, end: prefix.length + quote.length }] : [],
    };
  }));

async function serveBoard(page: Page) {
  const saved = boardFixture();
  let state: PresentationState | null = null;
  let stateRevision = 1;
  const requests: string[] = [];
  const sourceReads: { anchor_lid?: string; direction?: string; include_outline?: boolean }[] = [];
  const sourceOpens: { source_ref_id: string; lid?: string }[] = [];
  await page.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname;
    const body = route.request().postDataJSON() ?? {};
    requests.push(path);
    let response: unknown = {};
    if (path.endsWith('/boards/list')) response = { items: [{ ...saved.document, item_count: saved.document.items.length }] };
    else if (path.endsWith('/boards/read')) response = saved;
    else if (path.endsWith('/boards/conversation')) response = { sessions: [] };
    else if (path.endsWith('/boards/source.resolve')) response = { source_ref_id: body.source_ref_id, label: '第二章 · 条件概率与贝叶斯定理', material_title: '概率学习材料', heading_path: ['第二章', '条件概率与贝叶斯定理'], highlighted_quote: boardFixtureItems[0].source_bindings[0].preview_snapshot, context_before: '新证据会改变我们对事件的判断。\n\n', context_after: '\n\n因此，解释检测结果时，要同时考虑患病人群与健康人群。', stale: false, can_open_in_reader: true };
    else if (path.endsWith('/boards/source.read')) {
      sourceReads.push(body);
      const requestedLid = body.anchor_lid ?? '2.3';
      const anchor = sourceReaderSegments.findIndex(segment => segment.lid === requestedLid || segment.lid.startsWith(`${requestedLid}.`));
      if (anchor < 0) { await route.fulfill({ status: 400, json: { message: '原文位置不存在' } }); return; }
      const start = body.direction === 'before' ? Math.max(0, anchor - 20) : body.direction === 'after' ? anchor + 1 : Math.max(0, anchor - 6);
      const end = body.direction === 'before' ? anchor : Math.min(sourceReaderSegments.length, start + 20);
      const segments = sourceReaderSegments.slice(start, end);
      response = {
        source_ref_id: body.source_ref_id, source_lid: '2.3', focus_lid: sourceReaderSegments[anchor].lid, segments,
        previous_lid: start > 0 ? segments[0]?.lid ?? null : null,
        next_lid: end < sourceReaderSegments.length ? segments.at(-1)?.lid ?? null : null,
        ...(body.include_outline === false ? {} : { outline: sourceChapterTitles.map((title, chapter) => ({ lid: String(chapter + 1), title, depth: 1 })) }),
      };
    }
    else if (path.endsWith('/boards/source.open')) { sourceOpens.push(body); response = { source_ref_id: body.source_ref_id, opened: true }; }
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
  return { saved, requests, sourceReads, sourceOpens, state: () => state };
}

test('canvas fullscreen fills the viewport and source previews preserve the board and draft', async ({ page }, info) => {
  await page.setViewportSize({ width: 1440, height: 960 });
  const fixture = await serveBoard(page);
  const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto('/board-workspace-visual.html');
  await expect(page.locator('.board-card')).toHaveCount(4);
  await page.getByRole('textbox', { name: '在画板上提问', exact: true }).fill('先保留这个问题，读完原文再继续');
  const normal = (await page.locator('.board-canvas').boundingBox())!;
  await page.screenshot({ path: info.outputPath('board-sources-desktop.png') });
  await page.getByRole('button', { name: '全屏画板', exact: true }).click();
  await expect.poll(() => page.evaluate(() => !!document.fullscreenElement)).toBe(true);
  await expect(page.locator('.board-topbar')).toBeHidden();
  await expect(page.locator('.board-side-rail')).toBeHidden();
  await expect(page.locator('.board-conversation')).toBeHidden();
  const full = await page.locator('.board-canvas').evaluate(node => ({ width: node.clientWidth, height: node.clientHeight, viewportWidth: innerWidth, viewportHeight: innerHeight }));
  expect(full.width).toBe(full.viewportWidth); expect(full.height).toBe(full.viewportHeight);
  expect(full.width).toBeGreaterThan(normal.width); expect(full.height).toBeGreaterThan(normal.height);
  await page.getByRole('button', { name: '显示全部内容', exact: true }).click();
  await page.screenshot({ path: info.outputPath('board-fullscreen.png') });
  await page.locator('.board-fullscreen-tools').getByRole('button', { name: '讲解', exact: true }).click();
  const sources = page.locator('.board-reading-pane:visible .board-reading-sources');
  await expect(sources.locator('summary')).toContainText('原文引用 · 1 处');
  await sources.locator('summary').click(); await sources.locator('button').click();
  await expect(page.getByRole('dialog', { name: '画板原文' })).toBeVisible();
  await expect(page.locator('.board-source-content mark')).toContainText('后验概率');
  await expect(page.locator('.board-source-content')).toContainText('新证据会改变');
  expect(fixture.requests.filter(path => path.endsWith('/boards/source.open'))).toHaveLength(0);
  await page.screenshot({ path: info.outputPath('board-fullscreen-source-preview.png') });
  await page.getByRole('button', { name: '关闭来源预览，返回画板', exact: true }).click();
  await page.evaluate(() => document.exitFullscreen());
  await expect(page.locator('.board-workspace')).not.toHaveClass(/board-canvas-fullscreen/);
  await expect(page.getByRole('textbox', { name: '在画板上提问', exact: true })).toHaveValue('先保留这个问题，读完原文再继续');
  await sources.locator('button').click();
  await page.getByRole('button', { name: '在阅读器中打开', exact: true }).click();
  await expect(page.getByTestId('board-returned-source')).toContainText('已定位原文：source-bayes');
  await page.getByRole('button', { name: '返回画板', exact: true }).click();
  await expect(page.getByRole('textbox', { name: '在画板上提问', exact: true })).toHaveValue('先保留这个问题，读完原文再继续');
  await expect(page.locator('[data-item-id="item-answer"] .board-card-open')).toHaveAttribute('aria-pressed', 'true');
  expect(errors).toEqual([]);
});

test('mobile fullscreen fallback, source reading and missing citations stay usable', async ({ page }, info) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.addInitScript(() => { Element.prototype.requestFullscreen = async () => { throw new Error('Fullscreen unavailable'); }; });
  const fixture = await serveBoard(page);
  await page.goto('/board-workspace-visual.html');
  await expect(page.locator('.board-card')).toHaveCount(4);
  await page.getByRole('textbox', { name: '在画板上提问', exact: true }).fill('留在画板的草稿');
  await page.getByRole('button', { name: '全屏画板', exact: true }).click();
  await expect(page.locator('.board-fullscreen-notice')).toContainText('浏览器未允许全屏');
  const full = await page.locator('.board-canvas').boundingBox();
  expect(full?.width).toBe(390); expect(full?.height).toBe(844);
  await page.getByRole('button', { name: '显示全部内容', exact: true }).click();
  await page.screenshot({ path: info.outputPath('board-fullscreen-mobile.png') });
  await page.locator('.board-fullscreen-tools').getByRole('button', { name: '讲解', exact: true }).click();
  const sources = page.locator('.board-reading-pane:visible .board-reading-sources');
  await sources.locator('summary').click(); await sources.locator('button').click();
  await expect(page.getByRole('dialog', { name: '画板原文' })).toBeVisible();
  const dialog = (await page.locator('.board-source-dialog').boundingBox())!;
  expect(dialog.x).toBeGreaterThanOrEqual(0); expect(dialog.x + dialog.width).toBeLessThanOrEqual(390);
  await page.screenshot({ path: info.outputPath('board-source-mobile.png') });
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: '画板原文' })).toBeHidden();
  await expect(page.locator('.board-workspace')).toHaveClass(/board-canvas-fullscreen/);
  await page.keyboard.press('Escape');
  await expect(page.locator('.board-workspace')).not.toHaveClass(/board-canvas-fullscreen/);
  await expect(page.getByRole('textbox', { name: '在画板上提问', exact: true })).toHaveValue('留在画板的草稿');
  fixture.saved.document.items[1].source_bindings = [];
  await page.reload();
  await page.locator('.board-mobile-switch').getByRole('button', { name: '讲解', exact: true }).click();
  await expect(page.locator('.board-reading-pane:visible .board-source-missing')).toContainText('本条讲解未附原文引用');
  await expect(page.locator('.board-reading-pane:visible .board-reading-sources')).toHaveCount(0);
  await page.goto('/board-workspace-visual.html?lang=en');
  await page.getByRole('button', { name: 'Full screen board', exact: true }).click();
  await page.locator('.board-fullscreen-tools').getByRole('button', { name: 'Ask', exact: true }).click();
  const toolbarFits = await page.locator('.board-fullscreen-tools').evaluate(node => [...node.querySelectorAll('button')].every(button => button.getBoundingClientRect().right <= innerWidth));
  expect(toolbarFits).toBe(true);
  expect(await page.locator('.board-canvas-tools').evaluate(node => node.getBoundingClientRect().right <= innerWidth)).toBe(true);
  await page.screenshot({ path: info.outputPath('board-fullscreen-mobile-english.png') });
});

test('temporary source reader scrolls through the book, preserves its anchor and opens the current reading position', async ({ page }, info) => {
  await page.setViewportSize({ width: 1440, height: 960 });
  const fixture = await serveBoard(page);
  const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto('/board-workspace-visual.html');
  await page.getByRole('textbox', { name: '在画板上提问', exact: true }).fill('读完这一章，再继续这个问题');
  const sources = page.locator('.board-reading-pane:visible .board-reading-sources');
  await sources.locator('summary').click(); await sources.locator('button').click();
  const reader = page.locator('.board-source-reader');
  const scroll = reader.locator('[data-source-reader-scroll]');
  const paragraphs = reader.locator('article[data-source-lid]');
  await expect(reader.locator('article[data-source-lid="2.3"]')).toBeVisible();
  await expect(reader.locator('mark')).toContainText('后验概率');
  await expect(reader.locator('mark')).toBeInViewport();
  const dialog = (await page.locator('.board-source-dialog').boundingBox())!;
  expect(dialog.width).toBeGreaterThan(1200); expect(dialog.height).toBeGreaterThan(800);
  await expect(reader.locator('[data-outline-lid]')).toHaveCount(3);
  expect(fixture.sourceReads[0].include_outline).toBe(true);
  const initialCount = await paragraphs.count();
  const initialFirst = await paragraphs.first().getAttribute('data-source-lid');
  const initialLast = await paragraphs.last().getAttribute('data-source-lid');
  await scroll.hover(); await page.mouse.wheel(0, -100000);
  await expect.poll(() => fixture.sourceReads.some(request => request.direction === 'before')).toBe(true);
  await expect.poll(() => paragraphs.count()).toBeGreaterThan(initialCount);
  // Loading preceding text must keep the paragraph the reader reached in view.
  await expect.poll(async () => {
    const viewport = (await scroll.boundingBox())!;
    const anchor = (await reader.locator(`[data-source-lid="${initialFirst}"]`).boundingBox())!;
    return Math.abs(anchor.y - viewport.y);
  }).toBeLessThan(180);
  const precedingCount = await paragraphs.count();
  await scroll.hover(); await page.mouse.wheel(0, 100000);
  await expect.poll(() => fixture.sourceReads.some(request => request.direction === 'after')).toBe(true);
  await expect.poll(() => paragraphs.count()).toBeGreaterThan(precedingCount);
  expect(await paragraphs.last().getAttribute('data-source-lid')).not.toBe(initialLast);
  const lids = await paragraphs.evaluateAll(nodes => nodes.map(node => node.getAttribute('data-source-lid')));
  expect(new Set(lids).size).toBe(lids.length);
  await reader.locator('[data-outline-lid="3"]').click();
  await expect(reader.locator('article[data-source-lid="3.1"]')).toBeInViewport();
  await expect.poll(() => fixture.sourceReads.some(request => request.direction === 'around' && request.anchor_lid === '3')).toBe(true);
  await reader.getByRole('button', { name: '回到引用处', exact: true }).click();
  await expect(reader.locator('mark')).toBeInViewport();
  expect(fixture.sourceOpens).toHaveLength(0);
  expect(fixture.requests.filter(path => /\/(?:reader\/|reader\.|navigate|goto)/.test(path))).toEqual([]);
  await page.screenshot({ path: info.outputPath('board-source-reader-desktop.png') });
  await page.getByRole('button', { name: '关闭来源预览，返回画板', exact: true }).click();
  await expect(page.getByRole('textbox', { name: '在画板上提问', exact: true })).toHaveValue('读完这一章，再继续这个问题');
  await expect(page.locator('[data-item-id="item-answer"] .board-card-open')).toHaveAttribute('aria-pressed', 'true');
  await sources.locator('button').click();
  await expect(reader.locator('article[data-source-lid="2.3"]')).toBeVisible();
  await expect(reader.locator('mark')).toBeInViewport();
  await reader.locator('[data-outline-lid="3"]').click();
  await expect(reader.locator('article[data-source-lid="3.1"]')).toBeInViewport();
  await page.getByRole('button', { name: '在阅读器中打开', exact: true }).click();
  await expect.poll(() => fixture.sourceOpens.at(-1)?.lid).toBe('3.1');
  await expect(page.getByTestId('board-returned-source')).toContainText('已定位原文：source-bayes');
  await page.getByRole('button', { name: '返回画板', exact: true }).click();
  await expect(page.getByRole('textbox', { name: '在画板上提问', exact: true })).toHaveValue('读完这一章，再继续这个问题');
  expect(errors).toEqual([]);
});

test('mobile temporary source reader has a usable chapter drawer and continuous text without horizontal overflow', async ({ page }, info) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const fixture = await serveBoard(page);
  await page.goto('/board-workspace-visual.html');
  await page.locator('.board-mobile-switch').getByRole('button', { name: '讲解', exact: true }).click();
  const sources = page.locator('.board-reading-pane:visible .board-reading-sources');
  await sources.locator('summary').click(); await sources.locator('button').click();
  const reader = page.locator('.board-source-reader');
  const scroll = reader.locator('[data-source-reader-scroll]');
  await expect(reader.locator('article[data-source-lid="2.3"]')).toBeVisible();
  await expect(reader.locator('mark')).toBeInViewport();
  const directory = reader.getByRole('button', { name: '目录', exact: true });
  await expect(directory).toHaveAttribute('aria-expanded', 'false');
  await directory.click();
  await expect(directory).toHaveAttribute('aria-expanded', 'true');
  await expect(reader.getByRole('navigation', { name: '原文目录' })).toBeVisible();
  await page.screenshot({ path: info.outputPath('board-source-reader-mobile-outline.png') });
  await reader.locator('[data-outline-lid="3"]').click();
  await expect(directory).toHaveAttribute('aria-expanded', 'false');
  await expect(reader.locator('article[data-source-lid="3.1"]')).toBeInViewport();
  const before = fixture.sourceReads.length;
  await scroll.hover(); await page.mouse.wheel(0, 100000);
  await expect.poll(() => fixture.sourceReads.slice(before).some(request => request.direction === 'after')).toBe(true);
  await reader.getByRole('button', { name: '回到引用处', exact: true }).click();
  await expect(reader.locator('mark')).toBeInViewport();
  expect(await page.locator('.board-source-dialog').evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
  expect(await scroll.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
  const bounds = (await page.locator('.board-source-dialog').boundingBox())!;
  expect(bounds.x).toBeGreaterThanOrEqual(0); expect(bounds.x + bounds.width).toBeLessThanOrEqual(390);
  expect(bounds.height).toBeGreaterThan(740);
  await page.screenshot({ path: info.outputPath('board-source-reader-mobile.png') });
  await page.keyboard.press('Escape');
  await expect(reader).toBeHidden();
  expect(fixture.sourceOpens).toHaveLength(0);
  await page.goto('/board-workspace-visual.html?lang=en');
  await page.locator('.board-mobile-switch').getByRole('button', { name: 'Explanation', exact: true }).click();
  await sources.locator('summary').click(); await sources.locator('button').click();
  await expect(reader.locator('article[data-source-lid="2.3"]')).toBeVisible();
  expect(await page.locator('.board-source-dialog').evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
  expect(await page.locator('.board-source-dialog footer').evaluate(node => [...node.querySelectorAll('button')].every(button => button.getBoundingClientRect().right <= innerWidth))).toBe(true);
  await page.screenshot({ path: info.outputPath('board-source-reader-mobile-english.png') });
});

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
  await page.locator('.board-reading-pane:visible .board-reading-sources summary').click();
  await page.locator('.board-reading-pane:visible .board-reading-sources button').click();
  await page.getByRole('button', { name: '在阅读器中打开', exact: true }).click();
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
