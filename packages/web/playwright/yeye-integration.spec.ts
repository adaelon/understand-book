import { expect, test, type Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import type { RunSnapshot } from '../src/agent-run-state';
import type { ResidentGoal } from '../src/api';

const output = process.env.YEYE_EVIDENCE_DIR ?? fileURLToPath(new URL('../../../docs/performance/yeye-brand-yy6-20261013/', import.meta.url));
const animationRequest = (url: string) => /\/(?:idle|reading)[-.].*\.json|\/lottie[-.].*\.js/.test(url);
async function screenshot(page: Page, name: string) {
  mkdirSync(output, { recursive: true }); await page.screenshot({ path: `${output}/${name}.png` });
}
async function advance(page: Page, ms: number) { await page.clock.fastForward(ms); await page.clock.runFor(60); }
async function visibility(page: Page, visible: boolean) {
  await page.evaluate(value => {
    Object.defineProperty(document, 'visibilityState', { configurable: true, value: value ? 'visible' : 'hidden' });
    document.dispatchEvent(new Event('visibilitychange'));
  }, visible);
}

// Real production NetworkApp/App/RightRail/player, with fixed API facts and a
// controllable EventSource transport so every execution state is reproducible.
async function reader(page: Page, options: { goals?: ResidentGoal[] } = {}) {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.clock.install();
  await page.addInitScript(() => {
    sessionStorage.setItem('understand-book:workspace:reader', JSON.stringify({ workspace_id: 'yy6' }));
    const host = window as any;
    host.yy6Sources = [];
    host.yy7Nods = [];
    new MutationObserver(records => {
      for (const record of records) {
        const element = record.target as HTMLElement;
        if (element.dataset.yeyeNodding === 'true') host.yy7Nods.push({
          animation: getComputedStyle(element).animationName,
          identity: element.closest('.mobile-learning-title') ? 'mobile' : element.closest('.rail-title') ? 'fullscreen' : 'tab',
        });
      }
    }).observe(document, { subtree: true, attributes: true, attributeFilter: ['data-yeye-nodding'] });
    host.EventSource = class extends EventTarget {
      onopen?: () => void; onerror?: () => void; closed = false;
      constructor(public url: string) {
        super(); host.yy6Sources.push(this);
        setTimeout(() => { if (!this.closed) this.onopen?.(); }, 0);
      }
      close() { this.closed = true; }
    };
  });
  const reference = { book_id: 'book', publication_id: 'v1' };
  const profile = { profile_id: 'technical_learning', profile_version: '1', ui_slots: [], slots: [], layout_presets: [], allowed_layout_actions: [], agent_tools: [] };
  const state = { book_id: 'book', revision: 0, profile,
    viewport: { anchor_lid: '1.1', top_lid: '1.1', bottom_lid: '1.2', width: 1, visible_lids: ['1.1', '1.2'] },
    layout: { rev: 0, open_slots: [], panel_sizes: {}, slot_order: [], pinned_evidence: [] } };
  const workspace = { workspace_id: 'yy6', generation: 1, revision: 0, published_book_ref: reference, selected_chat: 'chat', reader: state };
  const descriptor = { ...reference, book_id: 'book', session_id: 'chat', turn_id: 'turn', workspace_id: 'yy6', workspace_generation: 1, published_book_ref: reference };
  let snapshot: RunSnapshot = { descriptor, last_seq: 0, execution_state: 'waiting_model', persistence_state: 'pending', activities: [], final_view: null, error: null };
  let admitted = false;
  let disconnected = false;
  let goals = options.goals ?? [];
  let historyReads = 0;
  let releaseHistory: (() => void) | undefined;
  let heldHistory: Promise<void> | undefined;
  const admission = () => ({ ...descriptor, dispatch_state: 'claimed', persistence_state: snapshot.persistence_state, snapshot });
  await page.route('**/api/**', async route => {
    const request = route.request(), url = new URL(request.url()), path = url.pathname;
    const action = path.replace(/^\/api\/workspaces\/yy6\//, '');
    const book = path.replace(/^\/api\/books\/book\/publications\/v1\//, '');
    let result: unknown;
    if (path === '/api/auth/me') result = { user_id: 'reader', csrf_token: 'csrf', email: 'reader@example.com' };
    else if (path === '/api/library') result = { books: [{ published_book_ref: reference, title: '推理中的缓存' }] };
    else if (path === '/api/workspaces/yy6' || action === 'attach' || action === 'reader/state' || action === 'reader/viewport') result = workspace;
    else if (action === 'resumption') result = { workspace_id: 'yy6', published_book_ref: reference, selected_chat: 'chat', position_label: '推理中的缓存', last_question: '缓存如何节省计算？' };
    else if (action === 'chat/select') {
      workspace.selected_chat = request.postDataJSON().session_id; workspace.generation++; workspace.revision++;
      result = workspace;
    }
    else if (action === 'chat/history') {
      historyReads++;
      result = { result: { active_session_id: workspace.selected_chat,
        sessions: options.goals ? ['chat', 'older-chat'].map(id => ({ id, title: id === 'chat' ? '当前缓存任务' : '旧任务记录', created_at: '', updated_at: '', turn_count: 1, turns: [] })) : [],
        current: { id: workspace.selected_chat, book_id: 'book',
          turns: admitted ? [snapshot.final_view ?? { turn_id: 'turn', user: '解释缓存如何节省计算', user_turn_ordinal: 0, status: snapshot.persistence_state === 'pending' ? 'pending_assistant' : 'failed', outcome: null, question_source_label: null, question_quote: null, effect_labels: [] }] : [],
          goals: workspace.selected_chat === 'chat' ? goals : [{ ...yy7Goal, revision: 2, status: 'completed', result_refs: ['note:old'] }] } } };
      const pendingHistory = heldHistory; heldHistory = undefined;
      await pendingHistory;
    }
    else if (action === 'agent/runs') { admitted = true; result = admission(); }
    else if (path === '/api/agent/runs/turn') {
      if (disconnected) return route.fulfill({ status: 503, json: { message: '连接暂不可用' } });
      result = admission();
    }
    else if (path === '/api/agent/runs/turn/cancel') { snapshot = { ...snapshot, execution_state: 'cancelling' }; result = admission(); }
    else if (book === 'manifest') result = { tree: [
      { lid: '1', kind: 'chapter', children: ['1.1', '1.2'], span: { start: 0, end: 180 }, title: '推理中的缓存' },
      { lid: '1.1', kind: 'paragraph', children: [], span: { start: 8, end: 100 } },
      { lid: '1.2', kind: 'paragraph', children: [], span: { start: 100, end: 180 } },
    ], stats_by_lid: {} };
    else if (book === 'asset_manifest') result = { images: [] };
    else if (book === 'source_fingerprint') result = { book_id: 'book', source_fingerprint: 'fixture' };
    else if (book === 'text') { const lid = url.searchParams.get('lid'); result = { lid, text: lid === '1' ? '# 推理中的缓存' : 'KV Cache 保存历史 token 的 Key 和 Value，供后续生成使用。缓存减少了重复计算，也增加了显存占用。' }; }
    else if (action === 'profile/manifest') result = { result: profile };
    else if (action === 'memory/recall') result = { result: [] };
    else if (action === 'profile/memory') result = { result: { current_book_id: 'book', facts: [], pending_candidates: [], evidence: [], collection_rules: [], status: { pending_sensitive_confirmation: false }, snapshot: { revision: 0, facts: [] } } };
    else if (action === 'profile/backfill') result = { result: { jobs: [], sessions: [] } };
    else if (path === '/api/tutor/state') result = { control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} };
    else if (book === 'tutor_readiness') result = { status: 'ready', source_id: 'book', source_revision: 's1', limitations: [], reason: '可用', teaching_assets: { status: 'ready', limitations: [], reason: '可用' } };
    else if (action === 'tutor/understanding') result = { result: { rows: [], interpretations: [], stale: false, next: null } };
    else return route.fulfill({ status: 404, json: { message: 'Unused fixture route: ' + path } });
    await route.fulfill({ json: result });
  });
  await page.goto('/');
  await page.getByRole('button', { name: '继续阅读', exact: true }).click();
  if (!admitted) await expect(page.locator('.conversation-welcome')).toBeAttached();
  if (page.viewportSize()!.width < 1024) await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
  await expect(page.locator('.welcome-mascot')).toBeVisible();
  await expect(page.locator('.welcome-mascot')).toHaveAttribute('data-yeye-active', 'true');
  await page.clock.runFor(100);
  if (options.goals) await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1_000));
  return {
    errors,
    get historyReads() { return historyReads; },
    holdHistory() { heldHistory = new Promise(resolve => { releaseHistory = resolve; }); },
    releaseHistory() { releaseHistory?.(); heldHistory = undefined; },
    async finish(nextGoals: ResidentGoal[], change: Partial<RunSnapshot> = {}) {
      goals = nextGoals;
      const execution = change.execution_state ?? 'completed';
      await this.update({ execution_state: execution, persistence_state: 'saved',
        final_view: { turn_id: 'turn', user: '解释缓存如何节省计算', user_turn_ordinal: 0,
          status: execution === 'failed' ? 'failed' : execution === 'cancelled' ? 'cancelled' : 'completed',
          outcome: { answer: '缓存笔记已整理。', answer_view: { parts: [{ kind: 'markdown', text: '缓存笔记已整理。' }], sources: [] },
            effects: [], trace: [], incomplete: false, warning: null, turns: 1, tokens_spent: 1, memory_updates: [],
            profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] } }, question_source_label: null, question_quote: null, effect_labels: [] }, ...change });
    },
    async connection(connected: boolean) {
      disconnected = !connected;
      await page.evaluate(value => {
        const source = (window as any).yy6Sources.at(-1);
        if (value) source.onopen(); else source.onerror();
      }, connected);
    },
    async send() {
      await page.locator('.agent-input textarea').fill('解释缓存如何节省计算');
      await page.locator('.agent-input').getByRole('button', { name: '发送', exact: true }).click();
      await expect.poll(() => admitted).toBe(true);
      await expect(page.locator('.run-status')).toBeVisible();
      await page.clock.runFor(100);
      await this.update({});
    },
    async update(change: Partial<RunSnapshot>) {
      snapshot = { ...snapshot, ...change, last_seq: snapshot.last_seq + 1 };
      await page.evaluate(value => {
        const source = (window as any).yy6Sources.at(-1);
        source.dispatchEvent(new MessageEvent('run.snapshot', { data: JSON.stringify({ turn_id: 'turn', seq: value.last_seq, type: 'run.snapshot', elapsed_ms: 0, payload: value }) }));
      }, snapshot);
    },
  };
}

for (const width of [1440, 390]) test(`YY6 production home loads only the visible idle animation at ${width}px`, async ({ page }) => {
  await page.setViewportSize({ width, height: 900 }); await page.clock.install();
  const requests: string[] = [], errors: string[] = [];
  page.on('request', request => { if (animationRequest(request.url())) requests.push(request.url()); });
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('/home.html');
  const mascot = page.locator('.home-hero-companion .yeye-mascot');
  await mascot.locator('img').evaluate((image: HTMLImageElement) => image.decode());
  await expect(mascot).toHaveAttribute('data-yeye-active', 'true');
  await page.clock.runFor(100);
  expect(requests).toEqual([]); await expect(mascot.locator('svg')).toHaveCount(0);
  await advance(page, 16_000); await expect(mascot).toHaveAttribute('data-yeye-playing', 'true');
  const src = await mascot.locator('svg image').first().getAttribute('href');
  expect(src).toMatch(/yeye-idle-.*\.webp$/);
  expect((await page.request.get(src!)).status()).toBe(200);
  expect(requests.some(url => /idle.*\.json/.test(url))).toBe(true);
  expect(requests.some(url => /lottie.*\.js/.test(url))).toBe(true);
  expect(requests.some(url => /reading|success/.test(url))).toBe(false);
  expect((await page.request.get('/licenses/lottie-web-MIT.txt')).status()).toBe(200);
  await screenshot(page, `home-playing-${width}`);
  await advance(page, 4_100); await expect(mascot).toHaveAttribute('data-yeye-playing', 'false');
  await page.locator('#begin').scrollIntoViewIfNeeded();
  await expect(mascot).toHaveAttribute('data-yeye-active', 'false');
  await expect(mascot.locator('svg')).toHaveCount(0);
  const count = requests.length; await advance(page, 40_000); expect(requests).toHaveLength(count);
  await mascot.scrollIntoViewIfNeeded(); await expect(mascot).toHaveAttribute('data-yeye-active', 'true'); await advance(page, 16_000);
  await expect(mascot).toHaveAttribute('data-yeye-playing', 'true');
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await expect(mascot.locator('svg')).toHaveCount(0);
  await expect(mascot.locator('img')).toBeVisible(); expect(errors).toEqual([]);
  mkdirSync(output, { recursive: true }); writeFileSync(`${output}/home-network-${width}.json`, JSON.stringify(requests, null, 2));
});

const yy7Goal: ResidentGoal = { id: 'cache-notes', revision: 1, status: 'open', interpretation: '整理缓存笔记',
  requirements: [], working: { focus: '整理例子', open_questions: [], next_move: '保存笔记', items: [] }, result_refs: [] };
const yy7Completed: ResidentGoal = { ...yy7Goal, revision: 2, status: 'completed', result_refs: ['note:cache'] };
const yy7Output = fileURLToPath(new URL('../../../docs/performance/yeye-brand-yy7-20261013/', import.meta.url));
async function nods(page: Page) { return page.evaluate(() => (window as any).yy7Nods as { animation: string; identity: string }[]); }
async function finishAndRead(page: Page, fixture: Awaited<ReturnType<typeof reader>>, goals = [yy7Completed], change: Partial<RunSnapshot> = {}) {
  const read = page.waitForResponse(response => response.url().endsWith('/chat/history'));
  await fixture.finish(goals, change); await (await read).finished(); await page.clock.runFor(60);
}

for (const layout of ['tab', 'fullscreen', 'mobile'] as const) test(`YY7 saved task nods once in the ${layout} identity and preserves the draft`, async ({ page }) => {
  await page.setViewportSize({ width: layout === 'mobile' ? 390 : 1440, height: 900 });
  const fixture = await reader(page, { goals: [yy7Goal] });
  if (layout === 'fullscreen') await page.getByRole('button', { name: '问答全屏', exact: true }).click();
  await fixture.send();
  const input = page.locator('.agent-input textarea'); await input.fill('下一条问题的草稿'); await input.focus();
  await fixture.update({ execution_state: 'finalizing' });
  await page.clock.runFor(1_000); expect(await nods(page)).toEqual([]);
  await finishAndRead(page, fixture);
  await expect.poll(() => nods(page)).toEqual([{ animation: expect.stringContaining('yeye-nod'), identity: layout }]);
  expect(await page.locator('[data-yeye-nodding=true]').count()).toBe(1);
  await expect(input).toHaveValue('下一条问题的草稿'); await expect(input).toBeFocused();
  await expect(page.locator('.a-msg')).toContainText('缓存笔记已整理');
  await page.clock.runFor(220);
  const transform = await page.locator('[data-yeye-nodding=true]').evaluate(element => getComputedStyle(element).transform);
  expect(transform).not.toBe('none');
  mkdirSync(yy7Output, { recursive: true });
  await page.screenshot({ path: `${yy7Output}/completion-${layout}.png` });
  writeFileSync(`${yy7Output}/completion-${layout}.json`, JSON.stringify({ nods: await nods(page), transform, width: page.viewportSize()!.width }, null, 2));
  await advance(page, 1_000); expect(await page.locator('[data-yeye-nodding=true]').count()).toBe(0);
  await fixture.update({}); await advance(page, 5_000); expect(await nods(page)).toHaveLength(1);
  expect(fixture.errors).toEqual([]);
});

for (const kind of ['ordinary answer', 'missing result', 'failed', 'cancelled', 'unsaved', 'created completed'] as const) test(`YY7 does not nod for ${kind}`, async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const fixture = await reader(page, { goals: kind === 'created completed' ? [] : [yy7Goal] }); await fixture.send();
  const goals = kind === 'ordinary answer' ? [yy7Goal] : kind === 'missing result' ? [{ ...yy7Completed, result_refs: [] }] : [yy7Completed];
  const change: Partial<RunSnapshot> = kind === 'failed' || kind === 'cancelled' ? { execution_state: kind }
    : kind === 'unsaved' ? { persistence_state: 'failed' } : {};
  if (kind === 'unsaved') {
    const reads = fixture.historyReads;
    await fixture.finish(goals, change); await expect(page.locator('.run-status')).toContainText('结果未保存');
    expect(fixture.historyReads).toBe(reads);
  } else await finishAndRead(page, fixture, goals, change);
  await advance(page, 5_000); expect(await nods(page)).toEqual([]); expect(fixture.errors).toEqual([]);
});

for (const hidden of ['bookshelf', 'background', 'reduced motion', 'mobile reader'] as const) test(`YY7 consumes completion while hidden by ${hidden}`, async ({ page }) => {
  await page.setViewportSize({ width: hidden === 'mobile reader' ? 390 : 1440, height: 900 });
  const fixture = await reader(page, { goals: [yy7Goal] }); await fixture.send();
  if (hidden === 'bookshelf') await page.getByRole('button', { name: '打开书', exact: true }).click();
  if (hidden === 'background') await visibility(page, false);
  if (hidden === 'reduced motion') await page.emulateMedia({ reducedMotion: 'reduce' });
  if (hidden === 'mobile reader') await page.getByRole('button', { name: '返回阅读', exact: true }).click();
  await finishAndRead(page, fixture);
  if (hidden === 'bookshelf') await page.getByRole('button', { name: '继续阅读', exact: true }).click();
  if (hidden === 'background') await visibility(page, true);
  if (hidden === 'reduced motion') await page.emulateMedia({ reducedMotion: 'no-preference' });
  if (hidden === 'mobile reader') await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
  await advance(page, 5_000); expect(await nods(page)).toEqual([]);
  await expect(page.locator('.a-msg')).toContainText('缓存笔记已整理'); expect(fixture.errors).toEqual([]);
});

test('YY7 invalidates a completion when the host hides and returns during the saved-history read', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const fixture = await reader(page, { goals: [yy7Goal] }); await fixture.send();
  fixture.holdHistory(); const reads = fixture.historyReads;
  await fixture.finish([yy7Completed]); await expect.poll(() => fixture.historyReads).toBeGreaterThan(reads);
  expect(await nods(page)).toEqual([]);
  await page.getByRole('button', { name: '打开书', exact: true }).click();
  await page.getByRole('button', { name: '继续阅读', exact: true }).click();
  const read = page.waitForResponse(response => response.url().endsWith('/chat/history'));
  fixture.releaseHistory(); await (await read).finished();
  await advance(page, 5_000); expect(await nods(page)).toEqual([]); expect(fixture.errors).toEqual([]);
});

async function selectHistory(page: Page, title: string) {
  const menu = page.locator('.chat-menu-toggle');
  if (await menu.isVisible()) await menu.click();
  await page.getByTitle('打开对话历史').click();
  const sessionId = title === '旧任务记录' ? 'older-chat' : 'chat';
  // Concurrent saved-history reads can finish here too; wait for the selected chat.
  const read = page.waitForResponse(async response => response.url().endsWith('/chat/history')
    && (await response.json()).result.current.id === sessionId);
  await page.locator('.history-card').filter({ hasText: title }).getByRole('button', { name: '打开对话', exact: true }).click();
  await (await read).finished();
  await expect(page.getByTitle('打开对话历史')).toBeEnabled();
  await expect(page.locator('.history-dialog')).toHaveCount(0);
}

test('YY7 changing chats, returning to completed history and reloading never replay the nod', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const fixture = await reader(page, { goals: [yy7Goal] }); await fixture.send(); await finishAndRead(page, fixture);
  await expect.poll(() => nods(page)).toHaveLength(1); await advance(page, 1_000);
  await selectHistory(page, '旧任务记录'); await advance(page, 1_000);
  await selectHistory(page, '当前缓存任务'); await advance(page, 1_000);
  expect(await nods(page)).toHaveLength(1);
  await page.reload(); await page.getByRole('button', { name: '继续阅读', exact: true }).click();
  await expect(page.locator('.a-msg')).toContainText('缓存笔记已整理');
  await advance(page, 5_000); expect(await nods(page)).toEqual([]); expect(fixture.errors).toEqual([]);
});

test('YY7 a late saved read cannot deliver feedback into another chat', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const fixture = await reader(page, { goals: [yy7Goal] }); await fixture.send();
  fixture.holdHistory(); const reads = fixture.historyReads;
  await fixture.finish([yy7Completed]); await expect.poll(() => fixture.historyReads).toBeGreaterThan(reads);
  await selectHistory(page, '旧任务记录');
  expect(await nods(page), 'no feedback before releasing the old chat history').toEqual([]);
  const read = page.waitForResponse(response => response.url().endsWith('/chat/history'));
  fixture.releaseHistory(); await (await read).finished(); await advance(page, 1_000);
  expect(await nods(page)).toEqual([]); expect(fixture.errors).toEqual([]);
});

test('YY7 a failed saved-history read remains static and reloading only establishes history', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const fixture = await reader(page, { goals: [yy7Goal] }); await fixture.send();
  await page.route('**/chat/history', route => route.fulfill({ status: 503, json: { message: '历史暂不可用' } }));
  await finishAndRead(page, fixture); await advance(page, 1_000);
  expect(await nods(page)).toEqual([]);
  await page.unroute('**/chat/history');
  await page.reload(); await page.getByRole('button', { name: '继续阅读', exact: true }).click();
  await expect(page.locator('.a-msg')).toContainText('缓存笔记已整理');
  await advance(page, 1_000); expect(await nods(page)).toEqual([]); expect(fixture.errors).toEqual([]);
});

test('YY6 reduced motion and a failed animation request keep the production home usable', async ({ page }) => {
  await page.clock.install(); await page.emulateMedia({ reducedMotion: 'reduce' });
  const requests: string[] = [];
  page.on('request', request => { if (animationRequest(request.url())) requests.push(request.url()); });
  await page.goto('/home.html'); await advance(page, 60_000); expect(requests).toEqual([]);
  await expect(page.locator('.home-hero-companion img')).toBeVisible();
  await page.route('**/idle*.json*', route => route.fulfill({ status: 503, body: '' }));
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await expect(page.locator('.home-hero-companion .yeye-mascot')).toHaveAttribute('data-yeye-active', 'true');
  await advance(page, 16_000);
  await expect.poll(() => requests.filter(url => /idle.*\.json/.test(url)).length).toBe(1);
  await expect(page.locator('.home-hero-companion img')).toBeVisible();
  await expect(page.locator('.home-hero-companion svg')).toHaveCount(0);
  await page.locator('.home-hero-actions a').first().click(); await expect(page).toHaveURL(/#learning-story$/);
});

test('YY6 bookshelf coverage, focus and background stop the retained reader and restart a fresh interval', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 }); const fixture = await reader(page);
  const welcome = page.locator('.welcome-mascot');
  await advance(page, 16_000); await expect(welcome).toHaveAttribute('data-yeye-playing', 'true');
  await page.locator('.agent-input textarea').fill('保留的草稿');
  await page.getByRole('button', { name: '打开书', exact: true }).click();
  await expect(welcome).toBeHidden(); await expect(welcome.locator('svg')).toHaveCount(0);
  await expect(page.locator('.resumption-mascot')).toHaveAttribute('data-yeye-active', 'true');
  await advance(page, 16_000); await expect(page.locator('.resumption-mascot')).toHaveAttribute('data-yeye-playing', 'true');
  expect(await page.locator('[data-yeye-playing=true]').count()).toBe(1);
  await screenshot(page, 'bookshelf-playing');
  await page.getByRole('button', { name: '继续阅读', exact: true }).click();
  await expect(page.locator('.agent-input textarea')).toHaveValue('保留的草稿');
  await expect(welcome).toHaveAttribute('data-yeye-active', 'true');
  await expect(welcome.locator('svg')).toHaveCount(0);
  await advance(page, 16_000); await expect(welcome).toHaveAttribute('data-yeye-playing', 'true');
  await page.getByRole('button', { name: '专注阅读', exact: true }).click();
  await expect(welcome.locator('svg')).toHaveCount(0);
  await advance(page, 30_000); await expect(welcome.locator('svg')).toHaveCount(0);
  await page.getByRole('button', { name: '展开工具栏', exact: true }).click();
  await page.getByRole('button', { name: '退出专注', exact: true }).click();
  await expect(welcome).toHaveAttribute('data-yeye-active', 'true');
  await page.clock.runFor(100); await advance(page, 16_000); await expect(welcome).toHaveAttribute('data-yeye-playing', 'true');
  await visibility(page, false); await expect(welcome.locator('svg')).toHaveCount(0);
  await advance(page, 40_000); await expect(welcome.locator('svg')).toHaveCount(0);
  await visibility(page, true); await page.clock.runFor(100);
  await expect(welcome).toHaveAttribute('data-yeye-playing', 'false');
  await advance(page, 16_000); await expect(welcome).toHaveAttribute('data-yeye-playing', 'true');
  expect(fixture.errors).toEqual([]);
});

test('YY6 wide waiting uses real reading artwork, pauses in the background and yields to streaming text', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 }); const fixture = await reader(page);
  await page.getByRole('button', { name: '问答全屏', exact: true }).click(); await fixture.send();
  const waiting = page.locator('.waiting-mascot');
  await expect(page.locator('.run-status')).toContainText('正在等待模型资源');
  await expect(waiting.locator('svg')).toHaveCount(0);
  await advance(page, 1_200); await expect(waiting).toHaveAttribute('data-yeye-playing', 'true');
  const src = await waiting.locator('svg image').first().getAttribute('href');
  expect(src).toMatch(/yeye-reading-cream-.*\.webp$/); expect((await page.request.get(src!)).status()).toBe(200);
  await screenshot(page, 'waiting-fullscreen');
  await visibility(page, false); await expect(waiting.locator('svg')).toHaveCount(0);
  await visibility(page, true); await page.clock.runFor(100); await advance(page, 1_200);
  await expect(waiting).toHaveAttribute('data-yeye-playing', 'true');
  await fixture.update({ draft: { message_id: 1, revision: 1, operation: 'replace', view: { parts: [{ kind: 'markdown', text: '缓存复用了之前计算的结果。' }], sources: [] } } });
  await expect(waiting).toHaveCount(0); await expect(page.getByLabel('回答草稿')).toContainText('缓存复用了');
  await expect(page.getByRole('button', { name: '停止', exact: true })).toBeVisible();
  await advance(page, 20_000); expect(await page.locator('[data-yeye-playing=true]').count()).toBe(0);
  expect(fixture.errors).toEqual([]);
});

test('YY6 stop, reconnect, finalizing and persistence failure never animate a stale wait', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 }); const fixture = await reader(page);
  await page.getByRole('button', { name: '问答全屏', exact: true }).click(); await fixture.send();
  await advance(page, 1_200); await expect(page.locator('.waiting-mascot')).toHaveAttribute('data-yeye-playing', 'true');
  await fixture.connection(false);
  await expect(page.locator('.waiting-mascot')).toHaveCount(0);
  await expect(page.getByText('连接中断，正在重新连接；运行仍可继续。')).toBeVisible();
  await fixture.connection(true);
  await page.clock.runFor(100); await advance(page, 1_200); await expect(page.locator('.waiting-mascot')).toHaveAttribute('data-yeye-playing', 'true');
  await page.getByRole('button', { name: '停止', exact: true }).click();
  await expect(page.locator('.waiting-mascot')).toHaveCount(0); await expect(page.locator('.run-status')).toContainText('正在停止');
  await fixture.update({ execution_state: 'finalizing' }); await expect(page.locator('.run-status')).toContainText('正在保存结果');
  await advance(page, 2_000); await expect(page.locator('.waiting-mascot')).toHaveCount(0);
  await fixture.update({ execution_state: 'completed', persistence_state: 'failed' });
  await expect(page.locator('.run-status')).toContainText('结果未保存');
  await expect(page.getByRole('button', { name: '重试保存原结果', exact: true })).toBeVisible();
  expect(await page.locator('[data-yeye-playing=true]').count()).toBe(0); expect(fixture.errors).toEqual([]);
});

test('YY6 mobile auxiliary hiding stops welcome while compact waiting keeps the avatar and status', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 }); const fixture = await reader(page);
  const welcome = page.locator('.welcome-mascot');
  await advance(page, 16_000); await expect(welcome).toHaveAttribute('data-yeye-playing', 'true');
  await screenshot(page, 'mobile-welcome-playing');
  await page.getByRole('button', { name: '返回阅读', exact: true }).click(); await expect(welcome.locator('svg')).toHaveCount(0);
  await advance(page, 30_000); await expect(welcome.locator('svg')).toHaveCount(0);
  await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
  await fixture.send(); await advance(page, 2_000);
  await expect(page.locator('.waiting-mascot')).toHaveCount(0);
  await expect(page.locator('.mobile-learning-title .yeye-mascot img')).toBeVisible();
  await expect(page.locator('.run-status')).toContainText('正在等待模型资源');
  await expect(page.locator('.agent-input textarea')).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await screenshot(page, 'mobile-waiting'); expect(fixture.errors).toEqual([]);
});
