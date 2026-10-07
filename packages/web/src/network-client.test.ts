// @vitest-environment happy-dom
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { api } from './api';
import { network, installIdentity, installWorkspace, forgetNetwork, bindSceneApi, type NetworkWorkspace } from './network-context';
import { networkFetch, recoverSubmissions, pendingSubmissions, workspaceAction, retrySubmission, recoverWorkspaceBinding } from './network-client';
import { readReaderSurfacePreference, writeReaderSurfacePreference } from './reader-surface';
const scene = { workspace_id: 'w', generation: 2, revision: 3, selected_chat: 'chat', published_book_ref: { book_id: 'b', publication_id: 'p' }, reader: {} } as NetworkWorkspace;
const json = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status });
beforeEach(() => { sessionStorage.clear(); localStorage.clear(); installIdentity({ user_id: 'A', csrf_token: 'csrf-A' }); installWorkspace(scene); });
afterEach(() => { vi.unstubAllGlobals(); network.value = { ...network.value, enabled: false }; });
describe('MU8 authorized browser context', () => {
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
      if (url.endsWith('/reader/goto')) return Promise.resolve(json({ ...scene, revision: 5 }));
      return new Promise<Response>(resolve => { releaseWrite = resolve; });
    }));
    const write = workspaceAction('reader/goto', { lid: '1.1' });
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
    await Promise.all([workspaceAction('reader/goto', { lid: '1.1' }), workspaceAction('reader/goto', { lid: '1.2' })]);
    expect(network.value.workspace?.revision).toBe(5);
  });
  it('rejects commands after generation change without replaying navigation', async () => {
    const fetch = vi.fn(async () => json({ ...scene, generation: 4 })); vi.stubGlobal('fetch', fetch);
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
