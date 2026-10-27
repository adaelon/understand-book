// @vitest-environment happy-dom
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { api, ApiError } from './api';
import { network, installIdentity, installWorkspace, forgetNetwork, bindSceneApi, readerKey, type NetworkWorkspace } from './network-context';
import { admissionSnapshot, networkFetch, recoverSubmissions, pendingSubmissions, workspaceAction, retrySubmission, recoverWorkspaceBinding } from './network-client';
import { readReaderSurfacePreference, writeReaderSurfacePreference } from './reader-surface';
const scene = { workspace_id: 'w', generation: 2, revision: 3, selected_chat: 'chat', published_book_ref: { book_id: 'b', publication_id: 'p' }, reader: {} } as NetworkWorkspace;
const json = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status });
beforeEach(() => { sessionStorage.clear(); localStorage.clear(); installIdentity({ user_id: 'A', csrf_token: 'csrf-A' }); installWorkspace(scene); });
afterEach(() => { vi.unstubAllGlobals(); network.value = { ...network.value, enabled: false }; });
describe('MU8 authorized browser context', () => {
  it('admits the first whole-board question once into the chat created for it', async () => {
    installWorkspace({ ...scene, selected_chat: null });
    let current = { ...scene, selected_chat: null } as NetworkWorkspace;
    const requests: Record<string, unknown>[] = [];
    const fetch = vi.fn(async (url: string, init?: RequestInit) => {
      if (url.endsWith('/chat/new')) { current = { ...scene, generation: 3, selected_chat: 'first-board-chat' }; return json(current); }
      if (url.endsWith('/agent/runs')) {
        requests.push(JSON.parse(String(init?.body)));
        return json({ turn_id: 'board-answer', session_id: current.selected_chat, book_id: 'b', dispatch_state: 'completed' });
      }
      return json(current);
    });
    vi.stubGlobal('fetch', fetch);
    const context = { board_id: 'empty-board', board_revision: 1 };
    expect(await api.agentRunCreate('请画出状态与上下文的关系', { session_id: '', client_request_id: 'first-board-question', board_follow_up: context })).toMatchObject({ turn_id: 'board-answer', session_id: 'first-board-chat' });
    expect(fetch.mock.calls.filter(([url]) => url.endsWith('/chat/new'))).toHaveLength(1);
    expect(requests).toEqual([expect.objectContaining({ session_id: 'first-board-chat', client_request_id: 'first-board-question', board_follow_up: context })]);
    expect(pendingSubmissions()).toEqual([]);
  });
  it('rejects a real chat change after first-board initialization without sending its captured question', async () => {
    installWorkspace({ ...scene, selected_chat: null });
    let created = false;
    const fetch = vi.fn(async (url: string) => {
      if (url.endsWith('/chat/new')) { created = true; return json({ ...scene, generation: 3, selected_chat: 'created' }); }
      return json(created ? { ...scene, generation: 3, selected_chat: 'changed-elsewhere' } : { ...scene, selected_chat: null });
    });
    vi.stubGlobal('fetch', fetch);
    await expect(api.agentRunCreate('保留我的问题', { session_id: '', client_request_id: 'captured', board_follow_up: { board_id: 'board', board_revision: 1 } })).rejects.toMatchObject({ errorCode: 'CHAT_SELECTION_CHANGED' });
    expect(fetch.mock.calls.some(([url]) => url.endsWith('/agent/runs'))).toBe(false);
    expect(network.value.workspace?.selected_chat).toBe('changed-elsewhere');
    expect(pendingSubmissions()).toEqual([]);
  });
  it('recovers a board question with the exact captured target and request key', async () => {
    const input = { session_id: 'chat', client_request_id: 'wb2-question', board_follow_up: { board_id: 'board-A', board_revision: 8, item_id: 'point-A', block_id: 'block-A' } };
    let saved: unknown;
    vi.stubGlobal('fetch', vi.fn(async (url: string, init?: RequestInit) => {
      if (url.includes('client_request_id=')) return json({ turn_id: 'board-answer', session_id: 'chat', book_id: 'b', dispatch_state: 'completed' });
      if (url.endsWith('/agent/runs')) { saved = JSON.parse(String(init?.body)); throw new Error('response lost'); }
      return json(scene);
    }));
    expect(await api.agentRunCreate('解释这一点', input)).toMatchObject({ turn_id: 'board-answer' });
    expect(saved).toMatchObject({ ...input, message: '解释这一点' });
    expect(pendingSubmissions()).toEqual([]);
  });
  it('bounds a stalled video draft request without submitting or automatically retrying it', async () => {
    vi.useFakeTimers();
    try {
      const fetch = vi.fn((_url: string, options: RequestInit) => new Promise<Response>((_, reject) => {
        options.signal?.addEventListener('abort', () => reject(options.signal!.reason), { once: true });
      }));
      vi.stubGlobal('fetch', fetch);
      const save = api.videoDraft({ window_id: 'window', action: 'save', draft: { message: 'question', context: {} as never } });
      const rejected = expect(save).rejects.toThrow('视频提问保存超时，请重试。');
      await vi.advanceTimersByTimeAsync(15_000);
      await rejected;
      expect(fetch).toHaveBeenCalledOnce();
      expect(fetch.mock.calls[0][0]).toBe('/api/workspaces/w/reader/video_draft');
      expect(pendingSubmissions()).toEqual([]);
    } finally { vi.useRealTimers(); }
  });

  it('saves video drafts in one round trip and refreshes only an explicit revision conflict', async () => {
    let conflict = false;
    const calls: string[] = [];
    const draft = { message: 'question', context: { draft_id: 'video-draft' } };
    vi.stubGlobal('fetch', vi.fn(async (url: string, options: RequestInit) => {
      calls.push(`${options.method} ${url}`);
      if (options.method === 'GET') return json({ ...scene, revision: 5 });
      if (conflict) { conflict = false; return json({ error_code: 'WORKSPACE_REVISION_CONFLICT' }, 409); }
      const input = JSON.parse(String(options.body));
      expect(input.draft).toEqual(draft);
      return json({ ...scene, revision: input.expected_revision + 1, result: { draft } });
    }));
    const input = { window_id: 'window', action: 'save' as const, draft: draft as never };
    expect(await api.videoDraft(input)).toEqual({ draft });
    expect(calls).toEqual(['POST /api/workspaces/w/reader/video_draft']);
    conflict = true; calls.length = 0;
    await api.videoDraft(input);
    expect(calls).toEqual(['POST /api/workspaces/w/reader/video_draft', 'GET /api/workspaces/w', 'POST /api/workspaces/w/reader/video_draft']);
    expect(network.value.workspace?.revision).toBe(6);
  });

  it.each([false, true])('SE5 routes all feedback operations with the current attachment (linked=%s)', async linked => {
    installWorkspace(scene, linked ? 'linked-attachment' : 'parent-attachment', linked);
    const reference = { presentation_id: 'p', revision: 2 };
    const selection = { choice: 'like' as const, reasons: [], comment: '' };
    const receipt = { session_id: 'chat', turn_id: 't', reference, state_revision: 3, saved_state_ref: 'saved' };
    const feedback = { session_id: 'chat', turn_id: 't', reference, feedback_revision: 4, selection, saved_state: receipt, created_at: 'now', updated_at: 'now' };
    const fetch = vi.fn(async (url: string, options: RequestInit) => json(options.method === 'GET' ? scene : { ...scene, result: feedback }));
    vi.stubGlobal('fetch', fetch);
    expect(await api.presentationFeedbackRead('chat', 't', reference)).toEqual(feedback);
    expect(await api.presentationFeedbackSet('chat', 't', reference, selection)).toEqual(feedback);
    expect(await api.presentationFeedbackClear('chat', 't', reference)).toEqual(feedback);
    expect(await api.presentationFeedbackAttachState('chat', 't', reference, 4, receipt)).toEqual(feedback);
    const posts = fetch.mock.calls.filter(([, options]) => options.method === 'POST');
    expect(posts.map(([url]) => url)).toEqual(['read', 'set', 'clear', 'attach_state'].map(op => `/api/workspaces/w/presentation/feedback/${op}`));
    for (const [, options] of posts) {
      expect(JSON.parse(String(options.body))).toMatchObject({ session_id: 'chat', turn_id: 't', reference,
        attachment_id: linked ? 'linked-attachment' : 'parent-attachment', generation: 2, expected_revision: 3 });
      expect(options.headers).toMatchObject({ 'X-CSRF-Token': 'csrf-A' });
    }
    expect(JSON.parse(String(posts[1][1].body)).selection).toEqual(selection);
    expect(JSON.parse(String(posts[3][1].body))).toMatchObject({ feedback_revision: 4, saved_state: receipt });
    expect(network.value.workspace).toMatchObject(scene);
  });
  it('keeps the original choice head and key on lost-response recovery', async () => {
    const input = { session_id:'chat', client_request_id:'tc5-choice', expected_chat_head:'original', teaching_choice:{delivery_ref:'delivery',option_id:'compare'} };
    let saved: any;
    const fetch=vi.fn(async (url: string, init?: RequestInit) => {
      if(url.includes('client_request_id=')) return json({turn_id:'accepted',session_id:'chat',book_id:'b',dispatch_state:'completed'});
      if(url.endsWith('/agent/runs')) {saved=JSON.parse(String(init?.body)); throw new Error('lost');}
      return json(scene);
    });
    vi.stubGlobal('fetch',fetch);
    expect(await api.agentRunCreate('',input)).toMatchObject({turn_id:'accepted'});
    expect(saved).toMatchObject(input); expect(pendingSubmissions()).toEqual([]);
    expect(fetch.mock.calls.filter(([url])=>url.endsWith('/agent/runs'))).toHaveLength(1);
  });
  it('rejects a typed action captured in a different chat before posting it', async () => {
    const fetch=vi.fn(async () => json(scene)); vi.stubGlobal('fetch',fetch);
    await expect(api.agentRunCreate('',{session_id:'old-chat',client_request_id:'tc5-old',entry_action:'clarify_answer',answer_ref:{turn_id:'old-answer'},expected_chat_head:'old-answer'})).rejects.toMatchObject({errorCode:'CHAT_SELECTION_CHANGED'});
    expect(fetch).toHaveBeenCalledTimes(1); expect(pendingSubmissions()).toEqual([]);
  });
  it('preserves the purpose, captured scope and original head when a submission response is lost', async () => {
    const input = { session_id: 'chat', client_request_id: 'learning-purpose-exam', expected_chat_head: 'previous-turn',
      entry_action: 'learn_with_purpose' as const, learning_purpose: 'exam' as const, chapter_lid: '1',
      source_id: 'b', source_revision: 'source-revision', question_quote: null };
    let saved: unknown;
    const fetch = vi.fn(async (url: string, init?: RequestInit) => {
      if (url.includes('client_request_id=')) return json({ turn_id: 'accepted-purpose', session_id: 'chat', book_id: 'b', dispatch_state: 'completed' });
      if (url.endsWith('/agent/runs')) { saved = JSON.parse(String(init?.body)); throw new Error('response lost'); }
      return json(scene);
    });
    vi.stubGlobal('fetch', fetch);
    expect(await api.agentRunCreate('', input)).toMatchObject({ turn_id: 'accepted-purpose' });
    expect(saved).toMatchObject(input);
    expect(fetch.mock.calls.filter(([url]) => url.endsWith('/agent/runs'))).toHaveLength(1);
    expect(pendingSubmissions()).toEqual([]);
  });
  it('does not put selection translation in the reader write queue', async () => {
    let release!: (value: Response) => void;
    const fetch = vi.fn(async (url: string) => {
      if (url.endsWith('/reader/selection.translate')) return new Promise<Response>(resolve => { release = resolve; });
      return json(scene);
    });
    vi.stubGlobal('fetch', fetch);
    const translation = api.pdfSelectionTranslate({ status: 'resolved', raw_quote: 'X', resolved_quote: 'X', ranges: [{ lid: '1.1', range: { start: 0, end: 1 } }] });
    await vi.waitFor(() => expect(release).toBeTypeOf('function'));
    expect(fetch).toHaveBeenCalledTimes(1);
    const write = workspaceAction('checkpoint');
    await vi.waitFor(() => expect(fetch.mock.calls.some(([url]) => url.endsWith('/checkpoint'))).toBe(true));
    release(json({ result: { translation_markdown: '译文', target_locale: 'zh-CN' } }));
    await expect(translation).resolves.toMatchObject({ translation_markdown: '译文' });
    await write;
  });
  it('preserves reader commands across a chat change while rejecting old responses and old chat commands', async () => {
    const reader = bindSceneApi(api, readerKey), chat = bindSceneApi(api);
    const initialReader = readerKey();
    let release!: (value: Response) => void;
    vi.stubGlobal('fetch', vi.fn(async (url: string) => {
      if (url.endsWith('/library')) return new Promise<Response>(resolve => { release = resolve; });
      if (url.endsWith('/chat/new')) return json({ ...scene, generation: 3, selected_chat: 'new' });
      if (url.endsWith('/memory/recall')) return json({ result: [] });
      return json(scene);
    }));
    const oldResponse = networkFetch('GET', '/library');
    const rejected = expect(oldResponse).rejects.toMatchObject({ errorCode: 'CLIENT_CONTEXT_STALE' });
    await workspaceAction('chat/new');
    expect(readerKey()).toBe(initialReader);
    release(json({ stale: true })); await rejected;
    expect(() => chat.recall()).toThrow('阅读现场已切换');
    expect(() => chat.recall()).toThrow(ApiError);
    expect(await reader.recall()).toEqual([]);
    // Revision-only updates must not undo the preserved Reader identity.
    installWorkspace({ ...network.value.workspace!, revision: 6 });
    expect(readerKey()).toBe(initialReader);
    installWorkspace({ ...network.value.workspace!, generation: 4 });
    expect(() => reader.recall()).toThrow('阅读现场已切换');
  });

  it('reads Tutor admission from the selected publication independently of teaching completeness', async () => {
    const fetch = vi.fn(async () => json({ status: 'ready', teaching_assets: { status: 'preparing' } }));
    vi.stubGlobal('fetch', fetch);
    expect(await api.tutorReadiness()).toMatchObject({ status: 'ready', teaching_assets: { status: 'preparing' } });
    expect(fetch.mock.calls[0]).toEqual(['/api/books/b/publications/p/tutor_readiness', expect.objectContaining({ method: 'GET' })]);
  });
  it('reattaches an evicted workspace before installing its new generation, then history reads succeed', async () => {
    const requests: string[] = [];
    let attached = false;
    const cold = { ...scene, generation: 3, revision: 4 };
    const restored = { ...cold, generation: 4, revision: 5 };
    vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
      requests.push(`${init.method} ${url}`);
      if (init.method === 'GET') return json(cold);
      if (url.endsWith('/attach')) {
        expect(network.value.workspace?.generation).toBe(2);
        expect(JSON.parse(String(init.body))).toMatchObject({ generation: 3, expected_revision: 4, attachment_id: network.value.attachment });
        attached = true; return json(restored);
      }
      expect(url).toBe('/api/workspaces/w/chat/history');
      return attached ? json({ result: { active_session_id: 'chat' } }) : json({ error_code: 'WORKSPACE_STALE' }, 409);
    }));
    await expect(api.agentHistory()).rejects.toMatchObject({ errorCode: 'WORKSPACE_STALE' });
    await Promise.all([recoverWorkspaceBinding(), recoverWorkspaceBinding()]);
    expect(network.value.workspace).toEqual(restored);
    expect(await api.agentHistory()).toEqual({ active_session_id: 'chat' });
    expect(requests.filter(r => r.endsWith('/attach'))).toHaveLength(1);
    expect(requests.some(r => r.includes('/agent/runs'))).toBe(false);
  });
  it('does not reattach a linked window or install a recovery response after an account switch', async () => {
    installWorkspace(scene, 'linked', true);
    const fetch = vi.fn(); vi.stubGlobal('fetch', fetch);
    await expect(recoverWorkspaceBinding()).rejects.toMatchObject({ errorCode: 'WORKSPACE_STALE' });
    expect(fetch).not.toHaveBeenCalled();
    installWorkspace(scene, 'parent', false);
    let release!: (response: Response) => void;
    fetch.mockImplementation(() => new Promise<Response>(resolve => { release = resolve; }));
    const recovery = recoverWorkspaceBinding();
    await new Promise(resolve => setTimeout(resolve, 0));
    installIdentity({ user_id: 'B', csrf_token: 'csrf-B' });
    release(json(scene));
    await expect(recovery).rejects.toMatchObject({ errorCode: 'CLIENT_CONTEXT_STALE' });
    expect(network.value.workspace).toBeNull();
    expect(fetch).toHaveBeenCalledOnce();
  });
  it('reads recap with its exact cutoff through the private GET route without workspace writes', async () => {
    const fetch = vi.fn(async () => json({ session_id: 'chat:1', through_seq: 17 }));
    vi.stubGlobal('fetch', fetch);
    expect(await api.sessionRecap('chat:1', 17)).toMatchObject({ through_seq: 17 });
    expect(fetch).toHaveBeenCalledOnce();
    expect(fetch.mock.calls[0]).toEqual(['/api/agent/history/recap?session_id=chat%3A1&through_seq=17', expect.objectContaining({ method: 'GET' })]);
  });
  it('preserves publication cover URLs in the reader book picker', async () => {
    const cover = { kind: 'pdf', url: '/api/books/b/publications/p/pdf/original' };
    vi.stubGlobal('fetch', vi.fn(async () => json({ books: [{ published_book_ref: scene.published_book_ref, cover }] })));
    expect((await api.bookLibrary()).books[0]).toMatchObject({ cover, dir: JSON.stringify(scene.published_book_ref), route: 'reader' });
  });
  it('reads with one request while a write waits and never rolls back its newer revision', async () => {
    let releaseWrite!: (value: Response) => void;
    let releaseRead!: (value: Response) => void;
    const calls: string[] = [];
    vi.stubGlobal('fetch', vi.fn((url: string) => {
      calls.push(url);
      if (url.endsWith('/memory/recall')) return new Promise<Response>(resolve => { releaseRead = resolve; });
      if (url.endsWith('/checkpoint')) return Promise.resolve(json({ ...scene, revision: 5 }));
      return new Promise<Response>(resolve => { releaseWrite = resolve; });
    }));
    const write = workspaceAction('checkpoint');
    await new Promise(resolve => setTimeout(resolve, 0));
    const read = api.recall();
    expect(calls).toEqual(['/api/workspaces/w', '/api/workspaces/w/memory/recall']);
    releaseWrite(json({ ...scene, revision: 4 }));
    await write;
    releaseRead(json({ ...scene, revision: 3, result: [] }));
    expect(await read).toEqual([]);
    expect(network.value.workspace?.revision).toBe(5);
  });
  it('rejects a late A response after B signs in and blocks retained A component commands', async () => {
    let release!: (value: Response) => void;
    vi.stubGlobal('fetch', vi.fn(() => new Promise<Response>(resolve => { release = resolve; })));
    const bound = bindSceneApi(api);
    const old = networkFetch('GET', '/library');
    forgetNetwork(); installIdentity({ user_id: 'B', csrf_token: 'csrf-B' }); installWorkspace(scene);
    release(json({ secret: 'A' }));
    await expect(old).rejects.toMatchObject({ errorCode: 'CLIENT_CONTEXT_STALE' });
    expect(() => bound.recall()).toThrow('阅读现场已切换');
  });
  it('saves the key before POST, discovers a lost response by exact key, never POSTs again', async () => {
    const requests: string[] = [];
    let requestKey = '';
    vi.stubGlobal('fetch', vi.fn(async (url: string, options: RequestInit) => {
      requests.push(`${options.method} ${url}`);
      if (url === '/api/workspaces/w') return json(scene);
      if (options.method === 'POST') {
        const input = JSON.parse(String(options.body)); requestKey = input.client_request_id;
        expect(pendingSubmissions()[0].key).toBe(requestKey);
        expect(input).toMatchObject({ generation: 2, expected_revision: 3, session_id: 'chat', published_book_ref: scene.published_book_ref });
        expect(options.headers).toMatchObject({ 'X-CSRF-Token': 'csrf-A' });
        throw new TypeError('response lost');
      }
      expect(url).toContain(`client_request_id=${requestKey}`);
      return json({ turn_id: 'turn', session_id: 'chat', book_id: 'b', dispatch_state: 'queued' });
    }));
    expect(await api.agentRunCreate('question')).toMatchObject({ turn_id: 'turn' });
    await recoverSubmissions();
    expect(requests.filter(r => r.startsWith('POST'))).toHaveLength(1);
    expect(pendingSubmissions()).toEqual([]);
  });
  it('keeps an unconfirmed key across refresh, and logout removes it', async () => {
    sessionStorage.setItem('understand-book:pending:A:key', JSON.stringify({ key: 'key', body: { message: 'original' }, workspace: 'w' }));
    vi.stubGlobal('fetch', vi.fn(async () => json({ error_code: 'OBJECT_NOT_FOUND' }, 404)));
    await recoverSubmissions(); expect(pendingSubmissions()).toHaveLength(1);
    forgetNetwork(); expect(sessionStorage.getItem('understand-book:pending:A:key')).toBeNull();
  });
  it('explicitly retries the original key and clears a definite rejection', async () => {
    const item = { key: 'original', workspace: 'w', body: { message: 'same question', client_request_id: 'original', generation: 1 } };
    sessionStorage.setItem('understand-book:pending:A:original', JSON.stringify(item));
    const calls: string[] = [];
    vi.stubGlobal('fetch', vi.fn(async (_url: string, options: RequestInit) => {
      calls.push(options.method!);
      if (options.method === 'GET') return json({ error_code: 'OBJECT_NOT_FOUND' }, 404);
      expect(JSON.parse(String(options.body))).toEqual(item.body);
      return json({ error_code: 'WORKSPACE_STALE' }, 409);
    }));
    await expect(retrySubmission('original')).rejects.toMatchObject({ errorCode: 'WORKSPACE_STALE' });
    expect(calls).toEqual(['GET', 'POST']); expect(pendingSubmissions()).toEqual([]);
  });
  it('uses exact publication URLs and expires an attached page when its parent changes generation', () => {
    expect(api.pdfOriginalUrl()).toBe('/api/books/b/publications/p/pdf/original');
    installWorkspace(scene, 'attached', true);
    installWorkspace({ ...scene, generation: 3 });
    expect(network.value.workspace).toBeNull();
    expect(() => api.pdfOriginalUrl()).toThrow('请先选择材料');
  });
  it('serializes CAS mutations using the latest server revision', async () => {
    let revision = 3;
    vi.stubGlobal('fetch', vi.fn(async (_url: string, options: RequestInit) => {
      if (options.method === 'GET') return json({ ...scene, revision });
      expect(JSON.parse(String(options.body)).expected_revision).toBe(revision);
      return json({ ...scene, revision: ++revision, result: { ok: true } });
    }));
    await Promise.all([workspaceAction('reader/scroll', { delta: 1 }), workspaceAction('reader/scroll', { delta: 1 })]);
    expect(network.value.workspace?.revision).toBe(5);
  });
  it('rejects commands after generation change without replaying navigation', async () => {
    const fetch = vi.fn(async () => json({ error_code: 'WORKSPACE_STALE' }, 409)); vi.stubGlobal('fetch', fetch);
    await expect(workspaceAction('reader/goto', { lid: '1.1' })).rejects.toMatchObject({ errorCode: 'WORKSPACE_STALE' });
    expect(fetch).toHaveBeenCalledTimes(1);
  });
  it('namespaces the same book device preference by user', () => {
    const identity = { bookId: 'b', sourceFingerprint: 's', userId: 'A' };
    writeReaderSurfacePreference(localStorage, identity, 'markdown');
    expect(readReaderSurfacePreference(localStorage, { ...identity, userId: 'B' })).toBeNull();
    expect(readReaderSurfacePreference(localStorage, identity)).toBe('markdown');
  });
});


describe('JL6 workspace effect disposition', () => {
  it('uses the authenticated workspace stamp and returns the server receipt', async () => {
    const receipt = { started: { disposition_id: 'd', effect_id: 'memory:m', action: 'undo' },
      receipt: { disposition_id: 'd', original_object_id: 'm', result_object_id: null, error: null } };
    const fetch = vi.fn(async (url: string, options: RequestInit) => {
      if (url === '/api/workspaces/w') return json(scene);
      expect(url).toBe('/api/workspaces/w/agent/effect/dispose');
      expect(JSON.parse(String(options.body))).toMatchObject({ session_id: 'chat', turn_id: 'turn', effect_id: 'memory:m', action: 'undo', generation: 2, expected_revision: 3 });
      expect(options.headers).toMatchObject({ 'X-CSRF-Token': 'csrf-A' });
      return json({ ...scene, revision: 4, result: receipt });
    });
    vi.stubGlobal('fetch', fetch);
    expect(await api.disposeEffect({ session_id: 'chat', turn_id: 'turn', effect_id: 'memory:m', action: 'undo' })).toEqual(receipt);
    expect(network.value.workspace?.revision).toBe(4);
  });
});

it('projects durable board parts from pending, cancelled and failed admissions without a live event buffer', () => {
  const part = { board_id: 'board', commit_id: 'saved-part', before_revision: 1, after_revision: 2, item_ids: ['answer'], block_ids: ['block'], origin_session_id: 'session', origin_turn_id: 'turn', parent_item_id: 'point' };
  for (const dispatch of ['claimed', 'cancelled', 'failed']) {
    const recovered = admissionSnapshot({ book_id: 'b', session_id: 'session', turn_id: 'turn', dispatch_state: dispatch, persistence_state: dispatch === 'claimed' ? 'pending' : dispatch === 'failed' ? 'failed' : 'saved',
      turn: { turn_id: 'turn', user_turn_ordinal: 1, user: '继续解释', question_source_label: null, question_quote: null, status: dispatch === 'claimed' ? 'pending_assistant' : dispatch === 'cancelled' ? 'cancelled' : 'failed', effect_labels: [], board_commits: [part] } });
    expect(recovered.board_commits).toEqual([part]);
    expect(recovered.persistence_state).toBe(dispatch === 'claimed' ? 'pending' : dispatch === 'failed' ? 'failed' : 'saved');
  }
});
