import { expect, test, type Locator, type Page } from '@playwright/test';
import { cp, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { startServer } from '../../../evals/semantic/server.mjs';
import type { TeachBackComparison } from '../src/generated/TeachBackComparison';
import { comparisonGraphs } from '../src/teach-back-history';
import { checkComparisonLayouts } from './fixtures/teach-back-layout';

// Opt in with an isolated Reader containing the actual §1.3.2 answer and the approved model budget.
const readerUrl = process.env.TEACH_BACK_READER_URL;
const origin = process.env.TEACH_BACK_ORIGIN_TURN;
const expression = '容量决定装不装得下。装得下以后，容量翻倍，读取也会快一倍。';

async function revealQuote(action: Locator) {
  if (!await action.isVisible()) await action.locator('xpath=ancestor::details[1]').locator(':scope > summary').click();
}

async function captureFocusOverview(page: Page, card: Locator, path: string) {
  await card.evaluate(el => el.scrollIntoView({ block: 'start' }));
  const clip = await card.evaluate(el => {
    const outer = el.getBoundingClientRect();
    const windowBottom = Math.min(innerHeight, el.closest('.transcript')!.getBoundingClientRect().bottom);
    const quotes = [...el.querySelectorAll('.teach-back-evidence section')]
      .map(side => side.querySelector('blockquote')!.getBoundingClientRect());
    const evidenceEnd = Math.max(...quotes.map(rect => rect.bottom));
    return {
      bothQuotesInView: quotes.length === 2 && evidenceEnd <= windowBottom,
      x: Math.floor(outer.left), y: Math.floor(outer.top),
      width: Math.ceil(outer.width), height: Math.ceil(Math.min(windowBottom, evidenceEnd + 32) - outer.top),
    };
  });
  const { bothQuotesInView, ...bounds } = clip;
  expect(bothQuotesInView, 'The wide overview includes both source quotations below the two graphs').toBe(true);
  await page.screenshot({ path, clip: bounds });
}

function rowQuotes(card: TeachBackComparison, row: TeachBackComparison['rows'][number]) {
  const bookEdges = card.book_graph.edges.filter(edge => row.book_edge_ids.includes(edge.id));
  const studentEdges = card.student_graph.edges.filter(edge => row.student_edge_ids.includes(edge.id));
  function nodesInGroup(nodeIds: string[], edges: {from: string; to: string}[]) {
    return new Set([...nodeIds, ...edges.flatMap(({from, to}) => [from, to])]);
  }
  const bookNodes = nodesInGroup(row.book_node_ids, bookEdges);
  const studentNodes = nodesInGroup(row.student_node_ids, studentEdges);
  const bookQuotes = [...bookEdges, ...card.book_graph.nodes.filter(n => bookNodes.has(n.id))].flatMap(item => item.evidence);
  const studentQuotes = [...studentEdges, ...card.student_graph.nodes.filter(n => studentNodes.has(n.id))].flatMap(item => item.quotes);
  return [...bookQuotes.filter((quote, i) => bookQuotes.findIndex(q => q.lid === quote.lid && q.quote === quote.quote) === i).map(q => q.quote),
    ...studentQuotes.filter((quote, i) => studentQuotes.findIndex(q => q.turn_id === quote.turn_id && q.text === quote.text) === i).map(q => q.text)];
}

async function expectAllRowQuotes(card: Locator, comparison: TeachBackComparison, row: TeachBackComparison['rows'][number]) {
  const quotes = card.locator('.teach-back-evidence blockquote');
  await expect.poll(async () => (await quotes.allTextContents()).map(t => t.trim()).sort()).toEqual(rowQuotes(comparison, row).sort());
}

test('reopen a saved course reflection with the original chat and compare the delivered rounds', async ({ page }, info) => {
  const url = process.env.TEACH_BACK_COURSE_READER_URL;
  test.skip(!url, 'Requires an isolated Reader with a delivered course reflection.');
  test.setTimeout(90_000);
  const history = await page.request.get(new URL('/api/agent/history', url).href).then(r => r.json());
  const book = history.current.book_id;
  const courseUrl = new URL('/api/book/course/' + encodeURIComponent(book), url);
  const catalog = await page.request.get(courseUrl.href).then(r => r.json());
  const runId = process.env.TEACH_BACK_COURSE_RUN_ID ?? catalog.progress.find((run: { stage: string }) => run.stage === 'explanation')?.run_id;
  expect(runId).toBeTruthy();
  courseUrl.searchParams.set('run_id', runId);
  const lesson = await page.request.get(courseUrl.href).then(r => r.json());
  const rounds = lesson.expressions.filter((e: { teach_back?: unknown; round_index: number }) => e.teach_back && e.round_index === lesson.round_index)
    .map((e: { teach_back: { session_id: string; turn_id: string; round_id: string } }) => {
      expect(e.teach_back.session_id).toBe(history.current.id);
      const turn = history.current.turns.find((t: { turn_id: string }) => t.turn_id === e.teach_back.turn_id);
      const card = turn.outcome.answer_view.parts.find((p: { kind: string }) => p.kind === 'teach_back_comparison').comparison as TeachBackComparison;
      expect(card.round_id).toBe(e.teach_back.round_id);
      return comparisonGraphs(card);
    }) as TeachBackComparison[];
  expect(rounds.length).toBeGreaterThan(0);
  const writes: Array<{ path: string; request: unknown }> = [];
  page.on('request', request => {
    const path = new URL(request.url()).pathname;
    if (request.method() === 'POST' && /\/(?:agent\/runs|tutor\/display|book\/course\/[^/]+)$/.test(path)) writes.push({ path, request: request.postDataJSON() });
  });
  try {
    await page.addInitScript(() => localStorage.setItem('understand-book.product-tour.v1.local.reader', 'done'));
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.goto(url!);
    await page.getByRole('button', { name: '本书课程 →' }).click();
    const player = page.locator('.inquiry-course:visible');
    await player.locator('.lesson-list > li').filter({ has: page.getByRole('heading', { name: lesson.summary.question, exact: true }) })
      .getByRole('button', { name: '继续学习', exact: true }).click();
    await expect(player.locator('.lesson-player')).toBeVisible();
    await page.reload();
    const historical = !lesson.expressions.at(-1)?.teach_back;
    const previous = player.locator('.feedback-history');
    if (historical) await previous.locator(':scope > summary').click();
    for (const [index, round] of rounds.entries()) {
      let card = player.locator('.activity-tutor .teach-back-comparison');
      if (historical) {
        const expression = lesson.expressions.find((e: { job_ref: string }) => e.job_ref === round.round_id);
        const entry = previous.locator('article.review').filter({ has: page.locator(':scope > details > blockquote', { hasText: expression.response }) });
        const details = entry.locator(':scope > details');
        if (await details.getAttribute('open') === null) await details.locator(':scope > summary').click();
        card = entry.locator('.teach-back-comparison');
      } else if (rounds.length > 1) {
        await card.locator('button[data-round-id="' + round.round_id + '"]').click();
      }
      await expect(card).toBeVisible();
      await expect(card.locator('.teach-back-select')).toHaveCount(round.rows.length);
      const help = round.support_delivery_refs?.filter(ref => ref.startsWith('tb:')).length ?? 0;
      await expect(card.locator('.teach-back-support')).toContainText(help ? `原讲解和 ${help} 轮三图对照` : '表达前已看过原讲解。');
      const row = round.rows.find(r => ['divergent','clarified'].includes(r.status)) ?? round.rows[0];
      const select = card.locator('[data-row-id="' + row.row_id + '"]');
      await select.click();
      await expectAllRowQuotes(card, round, row);
      await select.evaluate(node => node.scrollIntoView({ block: 'start' }));
      await page.screenshot({ path: info.outputPath('course-round-' + (index + 1) + '.png') });
    }
    expect(writes.filter(item => item.path.endsWith('/agent/runs') || item.path.includes('/book/course/'))).toHaveLength(0);
  } finally {
    await writeFile(info.outputPath('browser-actions.json'), JSON.stringify({ runId, chat: history.current.id, writes }, null, 2));
  }
});

test('submit the AI Infra teach-back through the live Reader and reopen its displayed evidence', async ({ page }, info) => {
  test.skip(!readerUrl || !origin, 'Requires an isolated Reader and a delivered answer turn.');
  test.setTimeout(360_000);
  const ledger: Array<{ path: string; status: number; request: unknown; at: string }> = [];
  page.on('response', response => {
    const path = new URL(response.url()).pathname;
    if (response.request().method() === 'POST' && /\/(agent\/runs|tutor\/display|reader\/goto)$/.test(path)) {
      ledger.push({ path, status: response.status(), request: response.request().postDataJSON(), at: new Date().toISOString() });
    }
  });
  await page.addInitScript(() => {
    localStorage.setItem('understand-book.product-tour.v1.local.reader', 'done');
  });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(readerUrl!);
  const head = page.locator('.transcript .turn').last();
  const card = page.locator('.teach-back-comparison').last();
  const receipt = () => ledger.find(item => item.path.endsWith('/tutor/display') && 'round_id' in (item.request as object));
  try {
    if (process.env.TEACH_BACK_REOPEN_ONLY !== '1') {
      await expect(head).toHaveAttribute('data-turn-id', origin!, { timeout: 30_000 });
      const dock = page.getByRole('group', { name: '当前回答的学习操作' });
      await expect(dock.getByRole('button', { name: '练到会', exact: true })).toBeVisible();
      await expect(dock.getByRole('button', { name: '给我一道应用题', exact: true })).toBeVisible();
      const explain = dock.getByRole('button', { name: '我来解释一下', exact: true });
      await expect(explain).toBeEnabled();
      await page.screenshot({ path: info.outputPath('entry.png') });
      await explain.click();
      const input = page.locator('.agent-input textarea');
      await expect(input).toBeFocused();
      await input.fill(expression);
      await page.screenshot({ path: info.outputPath('bound-expression.png') });
      await expect(card).toHaveCount(0);
      expect(receipt()).toBeUndefined();
      await page.locator('.agent-input').getByRole('button', { name: '发送', exact: true }).click();
      for (const [i, stage] of ['正在整理你的表达', '正在核对书中原文与对照'].entries()) {
        await expect(page.getByRole('status').filter({ hasText: stage }).last()).toBeInViewport({ timeout: 90_000 });
        await expect(card).toHaveCount(0);
        await page.screenshot({ path: info.outputPath(`stage-${i + 1}.png`) });
      }
    }
    await expect(card).toBeVisible({ timeout: 150_000 });
    await page.screenshot({ path: info.outputPath('initial-feedback.png') });
    await card.scrollIntoViewIfNeeded();
    await expect.poll(receipt).toBeTruthy();
    expect(receipt()?.status).toBe(200);
    await expect(head.locator('.u-msg')).toHaveText(expression);
    const divergence = card.locator('.teach-back-select[data-status="divergent"]').first();
    await divergence.click();
    await divergence.evaluate(el => el.scrollIntoView({ block: 'start' }));
    await page.screenshot({ path: info.outputPath('selected-divergence.png') });
    const sideNotes = card.locator('.teach-back-side-note');
    const note = sideNotes.filter({ hasText: '3.6.3.15' }).first();
    await expect(note).toBeVisible();
    await expect(note).not.toHaveAttribute('open', '');
    await note.locator('summary').click();
    await note.evaluate(el => el.scrollIntoView({ block: 'start' }));
    await expect(note.locator('blockquote').first()).toBeInViewport();
    await expect(note.getByRole('button', { name: '回到原段 3.6.3.15', exact: true })).toBeEnabled();
    await writeFile(info.outputPath('expanded-side-note.txt'), await note.innerText());
    await page.screenshot({ path: info.outputPath('side-note.png') });
    await note.locator('summary').click();
    await checkComparisonLayouts(page, card, info, 'zh');
    const selected = card.locator('.teach-back-evidence');
    await selected.getByRole('button', { name: '回到我的表达' }).first().click();
    await expect(head.locator('.u-msg')).toBeInViewport();
    const sourceLink = selected.getByRole('button', { name: '回到原段 3.6.3.14', exact: true, includeHidden: true }).first();
    await revealQuote(sourceLink);
    await sourceLink.click();
    await expect(page.locator('.reader-pane [data-lid="3.6.3.14"]').first()).toBeInViewport();
    await page.screenshot({ path: info.outputPath('returned-to-source.png') });
    await page.setViewportSize({ width: 1440, height: 1000 });
    const english = new URL(readerUrl!);
    english.searchParams.set('lang', 'en');
    await page.goto(english.href);
    await expect(card).toBeVisible({ timeout: 30_000 });
    await checkComparisonLayouts(page, card, info, 'en');
    expect(ledger.filter(item => item.path.endsWith('/agent/runs'))).toHaveLength(process.env.TEACH_BACK_REOPEN_ONLY === '1' ? 0 : 1);
  } finally {
    await writeFile(info.outputPath('reader-actions.json'), JSON.stringify(ledger, null, 2));
  }
});

test('reopen the long-anchor comparison and follow the protected-trade evidence', async ({ page }, info) => {
  const saved = process.env.TEACH_BACK_WEALTH_REPLAY_STATE;
  test.skip(!saved, 'Requires the saved 40.2 Reader trial.');
  test.setTimeout(90_000);
  const root = fileURLToPath(new URL('../../../', import.meta.url));
  const state = info.outputPath('reader');
  await mkdir(info.outputDir, { recursive: true });
  await cp(saved!, state, { recursive: true });
  const server = await startServer(path.join(root, '.understand-book/justin-sun-z-library-sk-1lib-sk-z-lib-sk'),
    path.join(state, 'memory'), root, { UNDERSTAND_BOOK_PRIVATE_DIR: path.join(state, 'private') });
  try {
    const { current } = await server.api('agent/history');
    const comparison: TeachBackComparison = current.turns.at(-1).outcome.answer_view.parts
      .find((p: { kind: string }) => p.kind === 'teach_back_comparison').comparison;
    const protectedTrade = comparison.book_graph.edges.find(edge =>
      edge.evidence.some(q => q.lid === '40.2' && q.quote.includes('保护')));
    expect(protectedTrade, 'A book edge cites the passage on protection').toBeDefined();
    const focus = comparison.rows.find(r => r.book_edge_ids.includes(protectedTrade!.id))!;
    expect(focus).toBeDefined();
    const generated: string[] = [];
    page.on('request', r => {
      if (r.method() === 'POST' && new URL(r.url()).pathname.endsWith('/agent/runs')) generated.push(r.url());
    });
    await page.addInitScript(() => localStorage.setItem('understand-book.product-tour.v1.local.reader', 'done'));
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.goto(server.url);
    await page.getByRole('button', { name: '问答全屏', exact: true }).click();
    const card = page.locator('.teach-back-comparison').last();
    for (const side of ['book', 'student'] as const) {
      const graph = card.locator(`.teach-back-graph[data-side="${side}"]`);
      await expect(graph.locator('[data-node-id]')).toHaveCount(comparison[`${side}_graph`].nodes.length);
      await expect(graph.locator('path[marker-end]')).toHaveCount(comparison[`${side}_graph`].edges.length);
    }
    const topics = card.locator('.teach-back-select .focus-topic');
    await expect(topics).toHaveCount(comparison.rows.length);
    const topicNames = await topics.allInnerTexts();
    expect(topicNames.every(topic => topic.trim())).toBe(true);
    expect(new Set(topicNames).size).toBe(topicNames.length);
    await card.locator('.teach-back-select').first().click();
    await expect(card.locator('.focus-heading')).toContainText(topicNames[0]);
    await captureFocusOverview(page, card, info.outputPath('wealth-wide.png'));
    await page.setViewportSize({ width: 1440, height: 1350 });
    await card.evaluate(el => el.scrollIntoView({ block: 'start' }));
    await expect(card).toBeInViewport({ ratio: .99 });
    await card.screenshot({ path: info.outputPath('wealth-card.png') });
    await page.setViewportSize({ width: 1440, height: 1000 });
    await card.locator(`[data-side="book"] [data-edge-label="${protectedTrade!.id}"]`).click();
    const evidence = card.locator('.teach-back-evidence');
    const quoted = protectedTrade!.evidence.find(q => q.lid === '40.2' && q.quote.includes('保护'))!;
    await expect(evidence.locator('section').first().locator('blockquote').first()).toHaveText(quoted.quote);
    await captureFocusOverview(page, card, info.outputPath('wealth-protection.png'));
    await expectAllRowQuotes(card, comparison, focus);
    await card.locator('.teach-back-graphs').screenshot({ path: info.outputPath('reasoning-graphs.png') });
    const extra = evidence.locator('details.more-quotes').first();
    await expect(extra.locator('blockquote').first()).toBeHidden();
    await extra.locator('summary').click();
    await expect(extra.locator('blockquote').first()).toBeVisible();
    await card.locator('.teach-back-select').first().click();
    await card.locator(`.teach-back-select[data-row-id="${focus.row_id}"]`).click();
    await expect(extra.locator('blockquote').first()).toBeHidden();
    await checkComparisonLayouts(page, card, info, 'zh', card.locator(`.teach-back-select[data-row-id="${focus.row_id}"]`));
    await page.setViewportSize({ width: 1440, height: 1000 });
    const fullscreen = page.getByRole('button', { name: '问答全屏', exact: true });
    if (await fullscreen.isVisible()) await fullscreen.click();
    const citation = card.locator('.citation').filter({ has: page.getByText(quoted.quote, { exact: true }) }).first();
    await revealQuote(citation);
    await citation.scrollIntoViewIfNeeded();
    await expect(citation).toBeInViewport();
    await page.screenshot({ path: info.outputPath('protected-trade.png') });
    await citation.getByRole('button', { name: '回到原段 40.2', exact: true }).click();
    await expect(page.locator('.reader-pane [data-lid="40.2"]').first()).toBeInViewport();
    expect(generated).toEqual([]);
  } finally {
    await server.stop();
  }
});

test('reopen a saved Reader correction and switch between the original and corrected snapshots', async ({ page }, info) => {
  const saved = process.env.TEACH_BACK_REPLAY_STATE;
  test.skip(!saved, 'Requires a stopped isolated AI Infra Reader with a delivered revision.');
  test.setTimeout(90_000);
  const root = fileURLToPath(new URL('../../../', import.meta.url));
  const state = info.outputPath('reader');
  await mkdir(info.outputDir, { recursive: true });
  await cp(saved!, state, { recursive: true });
  const server = await startServer(path.join(root, '.understand-book/ai-infra-book-complete'),
    path.join(state, 'memory'), root, { UNDERSTAND_BOOK_PRIVATE_DIR: path.join(state, 'private') });
  const actions: Array<{ path: string; status: number; data: { round_id?: string }; support?: string[] }> = [];
  page.on('response', async response => {
    const path = new URL(response.url()).pathname;
    if (response.request().method() === 'POST' && /\/(agent\/runs|tutor\/display)$/.test(path)) {
      const support = path.endsWith('/tutor/display') && response.ok() ? (await response.json()).support_delivery_refs : undefined;
      actions.push({ path, status: response.status(), data: response.request().postDataJSON(), support });
    }
  });
  try {
    const { current } = await server.api('agent/history');
    const rounds: TeachBackComparison[] = current.turns.flatMap((turn: {
      outcome?: { answer_view?: { parts: Array<{ comparison?: TeachBackComparison }> } },
    }) => turn.outcome?.answer_view?.parts.flatMap(p => p.comparison ? [comparisonGraphs(p.comparison)] : []) ?? []);
    const revision = rounds.findLast(c => c.continuation)!;
    expect(revision).toBeDefined();
    const first = rounds.find(c => c.round_id === revision.continuation!.first_round_id)!;
    if (revision.continuation!.kind === 'dispute_interpretation') {
      expect(revision.support_delivery_refs).not.toContain(first.round_id);
    } else expect(revision.support_delivery_refs).toContain(first.round_id);
    await page.addInitScript(() => localStorage.setItem('understand-book.product-tour.v1.local.reader', 'done'));
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.goto(server.url);
    await page.getByRole('button', { name: '问答全屏', exact: true }).click();
    const card = page.locator('.teach-back-comparison').filter({
      has: page.locator(`button[data-round-id="${revision.round_id}"]`),
    }).last();
    for (const [name, comparison] of [['first', first], ['revision', revision]] as const) {
      await card.locator(`button[data-round-id="${comparison.round_id}"]`).click();
      const receipt = () => actions.find(a => a.path.endsWith('tutor/display') && a.status === 200
        && a.data.round_id === comparison.round_id);
      await expect.poll(receipt).toBeTruthy();
      const supportCount = (comparison.support_delivery_refs ?? receipt()?.support ?? []).filter(id => id.startsWith('tb:')).length;
      await expect(card.locator('.teach-back-support')).toContainText(supportCount
        ? `表达前已看过原讲解和 ${supportCount} 轮三图对照。` : '表达前已看过原讲解。');
      await expect(card.locator('.teach-back-select')).toHaveCount(comparison.rows.length);
      const selected = name === 'first' ? comparison.rows.find(r => r.row_id === revision.continuation!.row_id)!
        : comparison.rows.find(r => r.status === 'clarified') ?? comparison.rows.find(r => r.student_edge_ids.length || r.student_node_ids.length)!;
      const row = card.locator(`[data-row-id="${selected.row_id}"]`);
      await row.click();
      await expectAllRowQuotes(card, comparison, selected);
      await row.evaluate(el => el.scrollIntoView({ block: 'start' }));
      await page.screenshot({ path: info.outputPath(name + '-evidence.png') });
      await card.locator('.teach-back-rounds').evaluate(el => el.scrollIntoView({ block: 'start' }));
      await page.screenshot({ path: info.outputPath(name + '-round.png') });
    }
    expect(actions.filter(a => a.path.endsWith('agent/runs'))).toEqual([]);
  } finally {
    await writeFile(info.outputPath('reader-actions.json'), JSON.stringify(actions, null, 2));
    await server.stop();
  }
});
