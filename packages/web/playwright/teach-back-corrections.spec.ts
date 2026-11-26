import { expect, test, type Locator, type Page } from '@playwright/test';
import { setupComparisonHistory, unrelatedQuestion } from './fixtures/teach-back-rounds';
import { checkGraphArrows, inspectGraphArrows } from './fixtures/teach-back-layout';
import { scenario } from './fixtures/tutor-continuation';

async function openConversation(page: Page, mode: string, locale: string) {
  const mobile = mode === '320' || mode === '390';
  await page.setViewportSize({ width: mobile ? Number(mode) : 1440, height: mobile ? 844 : 1000 });
  await page.evaluate(lang => localStorage.setItem('understand-book.locale', lang), locale);
  await page.reload();
  if (mobile) {
    await page.locator('.workspace-mobile-nav').getByRole('button', { name: locale === 'zh' ? '问答' : 'Q&A', exact: true }).click();
  } else if (mode === 'wide') {
    await page.getByRole('button', { name: locale === 'zh' ? '问答全屏' : 'Full-screen Q&A', exact: true }).click();
  }
}

async function selectStopByKeyboard(card: Locator) {
  const first = card.locator('.teach-back-select').first();
  const stop = card.locator('[data-row-id="r2"]');
  await first.focus();
  await first.press('ArrowDown');
  await expect(stop).toBeFocused();
  await stop.press('Enter');
  await expect(stop).toHaveAttribute('aria-pressed', 'true');
  return stop;
}

for (const locale of ['zh', 'en']) {
  for (const mode of ['wide', 'rail', '390', '320']) {
    test(`selected-row correction and saved round replay in ${locale} at ${mode}`, async ({ page }, info) => {
      const dispute = mode === 'wide' || mode === '320';
      const kind = dispute ? 'dispute_interpretation' : 'revise_understanding';
      const fixture = await setupComparisonHistory(page, dispute);
      await openConversation(page, mode, locale);
      const card = page.locator('.teach-back-comparison').last();
      // The follow-up opens its group on both graphs. Choosing an endpoint or an edge label
      // must select the corresponding group on the opposite graph, including shared nodes.
      const focus = (id: string) => card.locator(`.teach-back-select[data-row-id="${id}"]`);
      await expect(focus('r2')).toHaveAttribute('aria-pressed', 'true');
      await card.locator('[data-side="book"] [data-node-id="bn1"]').click();
      await expect(focus('r1')).toHaveAttribute('aria-pressed', 'true');
      await expect(card.locator('[data-side="student"] [data-edge-id="s1"]')).toHaveAttribute('data-active', 'true');
      await expect(card.locator('[data-side="student"] [data-node-id="sn3"]')).toHaveAttribute('data-active', 'false');
      await card.locator('[data-side="student"] [data-edge-label="s2"]').click();
      await expect(card.locator('[data-side="book"] [data-edge-id="b2"]')).toHaveAttribute('data-active', 'true');
      await expect(focus('r2')).toHaveAttribute('aria-pressed', 'true');
      await selectStopByKeyboard(card);
      const evidence = card.locator('.teach-back-evidence');
      await expect(evidence.locator('blockquote')).toHaveText(['total time', fixture.oldQuote]);
      const editorButton = card.locator(`[data-action="${kind}"]`);
      await editorButton.click();
      const composer = page.locator('.agent-input textarea');
      await expect(composer).toHaveAttribute('placeholder', locale === 'zh' ? '回应追问，或问新问题…' : 'Reply or ask a question…');
      expect(await composer.evaluate(el => el.scrollHeight <= el.clientHeight + 1)).toBe(true);
      const editor = card.locator('form textarea');
      const submit = card.locator('form button[type="submit"]');
      await expect(editor).toBeFocused();
      const message = dispute ? 'My clock keeps ticking while the wheels stop.' : 'I include the whole stop in elapsed time.';
      await editor.fill(message);
      await submit.evaluate(el => el.scrollIntoView({ block: 'center' }));
      await expect(editor).toBeInViewport();
      await expect(submit).toBeInViewport();
      await expect(submit).toBeEnabled();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await page.screenshot({ path: info.outputPath('correcting-selected-row.png') });
      await submit.click();
      await expect.poll(() => fixture.requests.length).toBe(1);
      expect(fixture.requests[0]).toMatchObject({ message, entry_action: 'explain_back',
        expected_chat_head: fixture.firstId, answer_ref: { turn_id: fixture.origin },
        expression_id: expect.any(String), operation_id: expect.any(String),
        teach_back_reply: { kind, row_id: 'r2', round_id: fixture.first.round_id } });
      const latest = page.locator('[data-turn-id="tc2-turn-2"]');
      await expect(latest.locator('.u-msg')).toHaveText(message);
      await expect(card.locator('.teach-back-row.selected')).toContainText('Both include the entire stop');
      const support = card.locator('.teach-back-support');
      if (dispute) {
        await expect(support).toContainText(locale === 'zh' ? '本轮按首次原话重新解读' : 'This round rechecks your first explanation.');
        await expect(support).not.toContainText(locale === 'zh' ? '1 轮三图对照' : 'earlier comparison feedback');
      } else {
        await expect(support).toContainText(locale === 'zh' ? '1 轮三图对照' : 'earlier comparison feedback (1)');
      }
      await expect(evidence.locator('blockquote')).toHaveText(['total time', dispute ? fixture.oldQuote : message]);
      // Confirm replay from a fresh history read and a changed source revision.
      await page.route('**/api/book/source_fingerprint', route => route.fulfill({ json: {
        book_id: fixture.first.source.source_id, source_fingerprint: 'new-source',
      } }));
      await openConversation(page, mode, locale);
      await expect(card.locator('[data-row-id="clarified-stop"]')).toBeVisible();
      await card.locator(`button[data-round-id="${fixture.first.round_id}"]`).click();
      await expect(support).toHaveText(locale === 'zh' ? '表达前已看过原讲解。' : 'You had seen the original explanation.');
      await selectStopByKeyboard(card);
      await expect(card.locator('.teach-back-select')).toHaveCount(2);
      await expect(evidence.locator('blockquote')).toHaveText(['total time', fixture.oldQuote]);
      await expect(evidence.locator('section').first().getByRole('button')).toBeDisabled();
      await expect(evidence).toContainText(locale === 'zh' ? '原书版本已变' : 'The book has changed.');
      await evidence.scrollIntoViewIfNeeded();
      await page.screenshot({ path: info.outputPath('reopened-quotation.png') });
      await card.locator('button[data-round-id="tb:tc2-turn-2:r1"]').click();
      await expect(card.locator('.teach-back-select')).toHaveCount(1);
      await expect(evidence.locator('blockquote')).toHaveText(['total time', dispute ? fixture.oldQuote : message]);
      await card.locator('.teach-back-rounds').evaluate(el => el.scrollIntoView({ block: 'start' }));
      await expect(support).toBeInViewport();
      await page.screenshot({ path: info.outputPath('clarification-replay.png') });
      expect(fixture.requests).toHaveLength(1);
      expect(fixture.errors).toEqual([]);
    });
  }
}

test('starts closed-book practice at the original reading answer after seeing its comparison', async ({ page }, info) => {
  const fixture = await setupComparisonHistory(page);
  let start: { answer_turn_id?: string; chat_session_id?: string } | undefined;
  await page.route('**/api/tutor/closed-book', route => {
    const request = route.request().postDataJSON();
    if (request.action === 'start') start = request;
    return route.fulfill({ json: start ? {
      id:'reading-practice', owner_id:start.chat_session_id, chat_session_id:start.chat_session_id, course:false,
      source_id:fixture.first.source.source_id, source_revision:fixture.first.source.source_revision,
      status:'active', revision:1, visible_turn_ids:['new-problem'], target:'比较另一段旅程的平均速度', answer_origin:{turn_id:start.answer_turn_id},
    } : null });
  });
  await openConversation(page, '320', 'zh');
  const card = page.locator('.teach-back-comparison').last();
  await selectStopByKeyboard(card);
  await expect.poll(() => fixture.displayed.some(r => r.round_id === fixture.first.round_id)).toBe(true);
  const practice = page.getByRole('button',{name:'练到会',exact:true});
  await expect(practice).toBeEnabled();
  await page.screenshot({path:info.outputPath('comparison-before-practice.png')});
  await practice.click();
  expect(start?.answer_turn_id).toBe(fixture.origin);
  await expect(page.getByRole('dialog')).toContainText('比较另一段旅程的平均速度');
  await expect(card).toHaveCount(0);
});

for (const related of [true, false]) {
  test(`main composer ${related ? 'answers a comparison follow-up' : 'resumes ordinary chat after a new topic'}`, async ({ page }) => {
    const fixture = await setupComparisonHistory(page);
    await openConversation(page, 'rail', 'zh');
    const card = page.locator('.teach-back-comparison').last();
    await selectStopByKeyboard(card);
    await expect.poll(() => fixture.displayed.some(x => x.round_id === fixture.first.round_id)).toBe(true);
    const composer = page.locator('.agent-input');
    const message = related ? 'Yes. Count the whole stop.' : unrelatedQuestion;
    await composer.locator('textarea').fill(message);
    await composer.getByRole('button', { name: '发送', exact: true }).click();
    await expect.poll(() => fixture.requests.length).toBe(1);
    expect(fixture.requests[0]).toMatchObject({ message, teach_back_reply: {
      kind: 'follow_up', round_id: fixture.first.round_id, row_id: 'r2',
    } });
    if (related) {
      await expect(card.locator('.teach-back-evidence')).toContainText(message);
    } else {
      const latest = page.locator('[data-turn-id="tc2-turn-2"]');
      await expect(latest).toContainText('Open the chapter menu');
      await expect(latest.locator('.teach-back-comparison')).toHaveCount(0);
      await composer.locator('textarea').fill('Where can I see my bookmarks?');
      await composer.getByRole('button', { name: '发送', exact: true }).click();
      await expect.poll(() => fixture.requests.length).toBe(2);
      expect(fixture.requests[1]).not.toHaveProperty('teach_back_reply');
    }
    expect(fixture.errors).toEqual([]);
  });
}

test('returning to the original chat restores its comparison and question', async ({ page }) => {
  const fixture = await setupComparisonHistory(page);
  let active = 'tc0-chat';
  const other = { ...fixture.turns[0], turn_id: 'other-answer', user: 'Where is the cycling route?' };
  function history() {
    const book_id = fixture.first.source.source_id;
    const sessions = [
      { id: 'tc0-chat', title: 'Average speed calculation', turns: fixture.turns, turn_count: fixture.turns.length },
      { id: 'other-chat', title: 'Cycling directions', turns: [other], turn_count: 1 },
    ];
    return { active_session_id: active, sessions, current: { ...sessions.find(s => s.id === active), book_id, goals: [] } };
  }
  await page.route('**/api/agent/history', route => route.fulfill({ json: history() }));
  await page.route('**/api/agent/history/select', route => {
    active = route.request().postDataJSON().session_id;
    return route.fulfill({ json: history() });
  });
  await openConversation(page, 'wide', 'zh');
  const card = page.locator('.teach-back-comparison');
  await selectStopByKeyboard(card);
  const composer = page.locator('.agent-input textarea');
  await expect(composer).toHaveAttribute('placeholder', /回应追问/);
  async function switchTo(title: string) {
    await page.getByRole('button', { name: /^本书历史/ }).click();
    await page.locator('.history-card').filter({ hasText: title }).getByRole('button', { name: '打开对话' }).click();
  }
  await switchTo('Cycling directions');
  await expect(card).toHaveCount(0);
  await expect(composer).not.toHaveAttribute('placeholder', /回应追问/);
  await switchTo('Average speed calculation');
  await selectStopByKeyboard(card);
  await expect(card.locator('.teach-back-evidence blockquote')).toHaveText(['total time', fixture.oldQuote]);
  await expect(composer).toHaveAttribute('placeholder', /回应追问/);
  expect(fixture.requests).toEqual([]);
  expect(fixture.errors).toEqual([]);
});

test('an explicit revision of a historical row keeps that round as its parent', async ({ page }, info) => {
  const fixture = await setupComparisonHistory(page);
  await openConversation(page, 'wide', 'en');
  const card = page.locator('.teach-back-comparison').last();
  const composer = page.locator('.agent-input');
  await selectStopByKeyboard(card);
  await expect(composer.locator('textarea')).toHaveAttribute('placeholder', /Reply or ask a question/);
  await composer.locator('textarea').fill('Count the stopped time.');
  await composer.getByRole('button', { name: 'Send', exact: true }).click();
  const original = card.locator(`button[data-round-id="${fixture.first.round_id}"]`);
  await original.click();
  await selectStopByKeyboard(card);
  await expect(composer.locator('textarea')).not.toHaveAttribute('placeholder', /Reply or ask a question/);
  await card.locator('[data-action="revise_understanding"]').click();
  await card.locator('form textarea').fill('On that original trip I use elapsed time, including the stop.');
  await card.locator('form button[type="submit"]').click();
  await expect.poll(() => fixture.requests.length).toBe(2);
  expect(fixture.requests[1]).toMatchObject({ expected_chat_head: 'tc2-turn-2', teach_back_reply: {
    kind: 'revise_understanding', round_id: fixture.first.round_id, row_id: 'r2',
  } });
  await expect(card.locator('button[data-round-id]')).toHaveCount(3);
  const turn = page.locator('[data-turn-id="tc2-turn-3"]');
  await expect(turn.getByRole('button', { name: 'Add a thought here' }).first()).toBeVisible();
  await expect(turn.getByRole('button', { name: 'Save to my thoughts' })).toBeVisible();
  await turn.locator('.u-msg').evaluate(el => el.scrollIntoView({ block: 'start' }));
  await page.screenshot({ path: info.outputPath('historical-row-revision.png') });
  expect(fixture.errors).toEqual([]);
});

test('a saved slice-3 round opens as a one-hop graph with its isolated conclusion and both quotations', async ({ page }, info) => {
  const fixture = await setupComparisonHistory(page);
  const turn = fixture.turns.find(entry => entry.turn_id === fixture.firstId)!;
  const oldQuote = 'Stopped time can be removed.';
  turn.outcome = { ...turn.outcome as object, answer_view: { sources: [], parts: [{ kind: 'teach_back_comparison', comparison: {
    round_id: fixture.first.round_id, source: fixture.first.source, side_notes: [],
    book_relations: [{ id: 'old-b', relation_type: 'requires', premise: 'Divide by elapsed time', conclusion: 'Count the stop',
      evidence: [{ lid: '1.1', quote: 'total time' }] }],
    student_relations: [{ id: 'old-s', relation_type: 'concludes', conclusion: 'Exclude the stop',
      quotes: [{ turn_id: fixture.firstId, text: oldQuote }] }],
    rows: [{ row_id: 'old-stop', book_relation_ids: ['old-b'], student_relation_ids: ['old-s'],
      status: 'divergent', summary: 'The denominators differ.' }],
  } }] } };
  const jumps: string[] = [];
  await page.route('**/api/reader/goto', route => {
    jumps.push(route.request().postDataJSON().lid);
    return route.fulfill({ json: { viewport: { anchor_lid: '1.1', top_lid: '1.1', bottom_lid: '1.1', width: 1, visible_lids: ['1.1'] },
      texts: [{ lid: '1.1', text: scenario.quote }] } });
  });
  await openConversation(page, 'rail', 'zh');
  const card = page.locator('.teach-back-comparison').last();
  await card.locator('.teach-back-select[data-row-id="old-stop"]').click();
  await expect(card.locator('[data-side="book"] [data-node-id]')).toHaveCount(2);
  await expect(card.locator('[data-side="book"] path[marker-end]')).toHaveCount(1);
  await expect(card.locator('[data-side="student"] [data-node-id]')).toHaveCount(1);
  await expect(card.locator('[data-side="student"] path[marker-end]')).toHaveCount(0);
  const evidence = card.locator('.teach-back-evidence');
  await expect(evidence.locator('blockquote')).toHaveText(['total time', oldQuote]);
  await evidence.getByRole('button', { name: '回到我的表达' }).click();
  await expect(page.locator(`[data-turn-id="${fixture.firstId}"] .u-msg`)).toBeInViewport();
  await evidence.getByRole('button', { name: '回到原段 1.1' }).click();
  await expect.poll(() => jumps).toEqual(['1.1']);
  await expect(page.locator('.reader-pane [data-lid="1.1"]').first()).toBeInViewport();
  await page.screenshot({ path: info.outputPath('legacy-source-jump.png') });
  expect(fixture.requests).toHaveLength(0);
});

test('draws each branch and merge arrow to its endpoint when the graph reflows', async ({ page }, info) => {
  const fixture = await setupComparisonHistory(page);
  const round = fixture.first;
  round.book_graph = {
    nodes: [
      {id: 'distance', label: '总路程', evidence: []}, {id: 'time', label: '总用时', evidence: []},
      {id: 'speed', label: '全程平均速度', evidence: []},
      {id: 'stop', label: '停留时间计入分母', evidence: []}, {id: 'positive', label: '总用时必须为正', evidence: []},
    ],
    edges: [
      {id: 'd', from: 'distance', to: 'speed', relation_type: 'defines', evidence: [{lid: '1.1', quote: scenario.quote}]},
      {id: 't', from: 'time', to: 'speed', relation_type: 'defines', evidence: [{lid: '1.1', quote: scenario.quote}]},
      {id: 'elapsed', from: 'speed', to: 'stop', relation_type: 'requires', evidence: [{lid: '1.1', quote: 'total time'}]},
      {id: 'valid', from: 'speed', to: 'positive', relation_type: 'requires', evidence: [{lid: '1.1', quote: 'The total time must be positive.'}]},
    ],
  };
  round.rows[0].book_edge_ids = ['d', 't'];
  round.rows[1].book_edge_ids = ['elapsed', 'valid'];
  for (const mode of ['wide', '320']) {
    await openConversation(page, mode, 'zh');
    const graph = page.locator('.teach-back-comparison [data-side="book"]');
    await graph.scrollIntoViewIfNeeded();
    await expect(graph.locator('[data-node-id]')).toHaveCount(5);
    await checkGraphArrows(graph);
    const arrows = await inspectGraphArrows(graph);
    expect(arrows.map(a => a.id)).toEqual(round.book_graph.edges.map(e => e.id));
    const wire = arrows.find(a => a.id === 'd')!.hit;
    expect(wire, 'The incoming wire has a directly clickable segment').not.toBeNull();
    await page.mouse.click(wire!.x, wire!.y);
    await expect(page.locator('.teach-back-select[data-row-id="r1"]')).toHaveAttribute('aria-pressed', 'true');
    await expect(page.locator('[data-side="student"] [data-edge-id="s1"]')).toHaveAttribute('data-active', 'true');
    await graph.evaluate(node => node.scrollIntoView({block: 'start'}));
    await page.screenshot({path: info.outputPath(mode + '-branch-merge.png')});
    if (mode === 'wide') {
      const card = page.locator('.teach-back-comparison').last();
      for (const width of [559, 560]) {
        await card.evaluate((el, width) => { (el as HTMLElement).style.width = width + 'px'; }, width);
        const [book, student] = await card.locator('.teach-back-graph').evaluateAll(graphs =>
          graphs.map(g => g.getBoundingClientRect().toJSON()));
        if (width === 559) expect(student.top).toBeGreaterThanOrEqual(book.bottom);
        else {
          expect(book.top).toBeCloseTo(student.top, 0);
          expect(student.left).toBeGreaterThanOrEqual(book.right);
        }
      }
    }
  }
});
