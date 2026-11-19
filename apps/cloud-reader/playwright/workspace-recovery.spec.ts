import { expect, test } from '@playwright/test';
import { boardFixture } from '@understand-book/web/reader/fixtures/board-workspace';

for (const width of [1440, 390]) test(`workspace recovery restores reading, chat and board automatically at ${width}px`, async ({ page }, info) => {
  await page.setViewportSize({ width, height: 900 });
  const errors: string[] = [], unexpected: string[] = [], questionPosts: string[] = [];
  page.on('pageerror', error => { errors.push(error.message); console.error(error.stack); });
  const reference = { book_id: 'book-probability', publication_id: 'pub-1' };
  const text = '阅读时遇到短暂断线，回来后仍能接着阅读。这段正文与之前的问题都保存在原来的阅读现场。';
  const reader = { book_id: reference.book_id, revision: 1,
    viewport: { anchor_lid: '1.1', top_lid: '1.1', bottom_lid: '1.1', visible_lids: ['1.1'], width: 1 },
    profile: { profile_id: 'technical_learning', profile_version: '1', ui_slots: [], layout_presets: [], allowed_layout_actions: [], agent_tools: [] },
    layout: { rev: 0, active_preset: null, open_slots: [], focused_slot: null, pinned_evidence: [], panel_sizes: {}, slot_order: {} } };
  let workspace = { workspace_id: 'resume-scene', generation: 1, revision: 1, selected_chat: 'original-chat',
    published_book_ref: reference, reader };
  const board = boardFixture();
  board.document.items = board.document.items.filter(item => item.target.kind !== 'presentation');
  board.view.selected_item_id = null;
  let attachment = '', failIdentity = false, failAttach = false, failManifest = false, failBoard = false;
  let manifestReads = 0, attachCount = 0, boardReads = 0;
  await page.addInitScript(() => {
    sessionStorage.setItem('understand-book:workspace:reader', JSON.stringify({ workspace_id: 'resume-scene', view: 'reader' }));
    (window as any).recoveryDocument = 'same-page';
  });
  await page.route('**/api/**', async route => {
    const request = route.request(), url = new URL(request.url()), path = url.pathname;
    const body = request.postDataJSON() ?? {};
    const json = (value: unknown) => route.fulfill({ json: value });
    const fail = () => route.fulfill({ status: 503, json: { error_code: 'TEMPORARILY_UNAVAILABLE', message: '连接暂时中断' } });
    if (path === '/api/auth/me') {
      if (failIdentity) { failIdentity = false; return fail(); }
      return json({ user_id: 'reader', csrf_token: 'csrf', email: 'reader@example.com' });
    }
    if (path === '/api/library') return json({ books: [{ published_book_ref: reference, title: '阅读恢复验证', is_default: true }] });
    if (path === '/api/videos') return json({ videos: [] });
    if (path === '/api/learning-workspaces') return json({ workspaces: [] });
    if (path === '/api/account/presence') return json({ ok: true });
    if (path === '/api/tutor/state') return json({ control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} });
    if (path.endsWith('/resumption')) return json({ ...workspace, position_label: '第一章', position_excerpt: text, last_question: '原来的问题' });
    if (path === '/api/workspaces/resume-scene') return json(workspace);
    if (path.endsWith('/attach')) {
      attachCount++;
      if (failAttach) { failAttach = false; return fail(); }
      if (attachment !== body.attachment_id) {
        attachment = body.attachment_id;
        workspace = { ...workspace, generation: workspace.generation + 1, revision: workspace.revision + 1 };
      }
      return json(workspace);
    }
    if (path.endsWith('/detach')) { attachment = ''; return json({ ok: true }); }
    const published = `/api/books/${reference.book_id}/publications/${reference.publication_id}/`;
    if (path.startsWith(published)) {
      const leaf = path.slice(published.length);
      if (leaf === 'manifest') {
        manifestReads++;
        if (failManifest) { failManifest = false; return fail(); }
        return json({ tree: [
          { lid: '1', kind: 'chapter', title: '# 阅读恢复验证', children: ['1.1'], span: { start: 0, end: text.length } },
          { lid: '1.1', kind: 'paragraph', children: [], span: { start: 0, end: text.length } },
        ], stats_by_lid: {} });
      }
      if (leaf === 'text') return json({ lid: url.searchParams.get('lid'), text });
      if (leaf === 'asset_manifest') return json({ images: [] });
      if (leaf === 'source_fingerprint') return json({ book_id: reference.book_id, source_fingerprint: 'original-source' });
      if (['video_playback', 'music_context', 'source_manifest', 'paper_metadata', 'pdf_source_map'].includes(leaf)) return json(null);
      if (leaf === 'tutor_readiness') return json({ status: 'ready', source_id: reference.book_id, source_revision: '1', limitations: [], reason: '可用', teaching_assets: { status: 'ready', limitations: [] } });
    }
    const prefix = '/api/workspaces/resume-scene/';
    if (path.startsWith(prefix)) {
      const action = path.slice(prefix.length);
      if (action === 'agent/runs' || action === 'chat/new') { questionPosts.push(path); return fail(); }
      if (body.generation !== workspace.generation || body.attachment_id !== attachment)
        return route.fulfill({ status: 409, json: { error_code: 'WORKSPACE_STALE', category: 'conflict' } });
      const result = (value: unknown) => json({ ...workspace, result: value });
      if (action === 'chat/history') return result({ active_session_id: 'original-chat', sessions: [], current: {
        id: 'original-chat', book_id: reference.book_id, goals: [], turns: [{ turn_id: 'saved-turn', user: '原来的问题',
          status: 'completed', effect_labels: [], outcome: { answer: '这是已经保存的回答，恢复连接不会重新发送问题。',
            incomplete: false, warning: null, turns: 1, tokens_spent: 0, effects: [], trace: [], memory_updates: [],
            profile_usage: { snapshot_revision: 1, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] } } }],
      } });
      if (action === 'reader/state') return result(reader);
      if (action === 'profile/manifest') return result({ profile_id: 'technical_learning', profile_version: '1', ui_slots: [], layout_presets: [], projections: [], allowed_layout_actions: [], agent_tools: [] });
      if (action === 'profile/memory') return result({ current_book_id: reference.book_id, facts: [], pending_candidates: [], evidence: [], collection_rules: [],
        status: { document_revision: 1, projection_revision: 1, profile_status: 'ready', pending_sensitive_confirmation: false, pending_review_jobs: 0, review_error: null },
        snapshot: { source_revision: 1, profile_status: 'ready', global_core: [], applicable_global: [], book_state_core: [], profile_projection: [], pending_context: [] } });
      if (action === 'memory/recall') return result([]);
      if (action === 'tutor/state') return result({ control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} });
      if (action === 'boards/list') return result({ items: [{ ...board.document, item_count: board.document.items.length }] });
      if (action === 'boards/read') { boardReads++; if (failBoard) { failBoard = false; return fail(); } return result(board); }
      if (action === 'boards/conversation') return result({ board_id: board.document.board_id, sessions: [] });
      if (action === 'boards/view.save') { board.view = body.view ?? board.view; return result({ saved: true }); }
    }
    unexpected.push(`${request.method()} ${path}`);
    console.error('Unexpected fixture request', request.method(), path);
    return route.fulfill({ status: 404, json: { error_code: 'NOT_FOUND' } });
  });
  await page.goto('/');
  await expect(page.locator('.reader-pane')).toContainText(text);
  const input = page.locator('textarea[data-workspace-input="agent"]');
  if (width === 390) await page.getByRole('button', { name: '问答', exact: true }).click();
  await expect(page.locator('.turn .u-msg')).toHaveText(['原来的问题']);
  await input.fill('返回后继续编辑的草稿');
  const initialReads = manifestReads;
  // Reproduce the server's cold-scene generation change and a background/return cycle.
  await page.evaluate(() => {
    Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' });
    document.dispatchEvent(new Event('visibilitychange'));
  });
  workspace = { ...workspace, generation: workspace.generation + 1, revision: workspace.revision + 1 };
  attachment = ''; failIdentity = true; failAttach = true; failManifest = true;
  await page.evaluate(() => {
    Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'visible' });
    document.dispatchEvent(new Event('visibilitychange'));
  });
  await expect.poll(() => manifestReads).toBe(initialReads + 2);
  if (width === 390) await page.getByRole('button', { name: '问答', exact: true }).click();
  await expect(page.locator('.turn .u-msg')).toHaveText(['原来的问题']);
  await expect(input).toHaveValue('返回后继续编辑的草稿');
  await expect(page.locator('.network-notices [role="alert"], .transcript [role="alert"]')).toHaveCount(0);
  await page.screenshot({ path: info.outputPath(`recovered-chat-${width}.png`) });
  if (width === 390) {
    await page.getByRole('button', { name: '返回阅读', exact: true }).click();
  }
  failBoard = true;
  await page.locator('.topbar .tools-toggle').click();
  await page.getByRole('button', { name: '当前资料的画板', exact: true }).click();
  await page.getByRole('button', { name: new RegExp(board.document.title) }).click();
  await expect(page.getByRole('textbox', { name: '画板名称', exact: true })).toHaveValue(board.document.title);
  expect(boardReads).toBe(2);
  await page.screenshot({ path: info.outputPath(`recovered-board-${width}.png`) });
  expect(await page.evaluate(() => (window as any).recoveryDocument)).toBe('same-page');
  expect(attachCount).toBeGreaterThanOrEqual(4);
  expect(questionPosts).toEqual([]);
  expect(errors).toEqual([]);
  expect(unexpected).toEqual([]);
});
