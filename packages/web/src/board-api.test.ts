// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { api } from './api';
import { network, installIdentity, installWorkspace, type NetworkWorkspace } from './network-context';
import { pendingReadingActions } from './local-reading-action';

const scene = { workspace_id: 'reader-window', learning_workspace_id: 'learning', generation: 2, revision: 8,
  selected_chat: null, published_book_ref: { book_id: 'book', publication_id: 'publication' }, reader: {} } as NetworkWorkspace;
const json = (value: unknown) => new Response(JSON.stringify(value), { status: 200 });
beforeEach(() => { sessionStorage.clear(); localStorage.clear(); installIdentity({ user_id: 'reader', csrf_token: 'csrf' }); installWorkspace(scene); });
afterEach(() => { vi.unstubAllGlobals(); network.value = { ...network.value, enabled: false }; });

describe('board browser transport', () => {
  it('reads board discussion across chats without selecting or creating a chat', async () => {
    const response = { board_id: 'board', sessions: [{ session_id: 'previous-chat', title: '先前的讨论', turns: [] }] };
    const fetch = vi.fn(async (_url: string, _init?: RequestInit) => json({ ...scene, result: response }));
    vi.stubGlobal('fetch', fetch);
    expect(await api.boardConversation('board')).toEqual(response);
    expect(fetch).toHaveBeenCalledOnce();
    expect(fetch.mock.calls[0][0]).toBe('/api/workspaces/reader-window/boards/conversation');
    expect(network.value.workspace?.selected_chat).toBeNull();
  });
  it('retains a local board question across a lost creation response and recovers without resubmitting', async () => {
    network.value = { ...network.value, enabled: false };
    const context = { board_id: 'board-A', board_revision: 7, item_id: 'point-A' };
    const calls: { url: string; body: unknown }[] = [];
    vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
      calls.push({ url, body: JSON.parse(String(init.body)) });
      if (url.endsWith('/agent/runs')) throw new Error('response lost');
      return json({ turn_id: 'saved-turn', session_id: 'chat', book_id: 'book' });
    }));
    expect(await api.agentRunCreate('解释这一点', { session_id: 'chat', client_request_id: 'board-question', board_follow_up: context })).toMatchObject({ turn_id: 'saved-turn' });
    expect(calls.map(call => call.url)).toEqual(['/api/agent/runs', '/api/agent/reading-action-status']);
    expect(calls[1].body).toEqual(calls[0].body);
    expect(calls[0].body).toMatchObject({ client_request_id: 'board-question', board_follow_up: context });
    expect(pendingReadingActions()).toEqual([]);
  });
  it('searches full board content and looks up understanding without changing the selected chat', async () => {
    const calls: { url: string; body: Record<string, unknown> }[] = [];
    vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
      calls.push({ url, body: JSON.parse(String(init.body)) });
      return json({ ...scene, result: { items: [] } });
    }));
    await api.boardSearch('慢段占用时间');
    await api.boardRelated({ kind: 'understanding', evidence_id: 'evidence-1' });
    expect(calls).toEqual([
      { url: '/api/workspaces/reader-window/boards/search', body: expect.objectContaining({ query: '慢段占用时间', expected_revision: 8 }) },
      { url: '/api/workspaces/reader-window/boards/related', body: expect.objectContaining({ source: { kind: 'understanding', evidence_id: 'evidence-1' } }) },
    ]);
    expect(network.value.workspace?.selected_chat).toBeNull();
  });
  it('preserves board concurrency revision separately from the reader window stamp', async () => {
    const requests: { url: string; body: Record<string, unknown> }[] = [];
    vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
      if (init.method === 'GET') return json(scene);
      requests.push({ url, body: JSON.parse(String(init.body)) });
      return json({ ...scene, result: { board_id: 'board', revision: 5 } });
    }));
    await api.boardUpdate({ board_id: 'board', expected_revision: 4, title: '认识这个概念' });
    expect(requests).toHaveLength(1);
    expect(requests[0]).toMatchObject({ url: '/api/workspaces/reader-window/boards/update',
      body: { board_id: 'board', board_revision: 4, expected_revision: 8, generation: 2, title: '认识这个概念' } });
  });
  it('reads a retained presentation without selecting or requiring its old chat', async () => {
    const fetch = vi.fn(async (_url: string, _init: RequestInit) => json({ ...scene, result: { title: '保存的实验' } }));
    vi.stubGlobal('fetch', fetch);
    expect(await api.boardPresentationRead('board', 'item')).toEqual({ title: '保存的实验' });
    expect(fetch).toHaveBeenCalledOnce();
    expect(fetch.mock.calls[0][0]).toBe('/api/workspaces/reader-window/boards/presentation.read');
    expect(JSON.parse(String(fetch.mock.calls[0][1].body))).toMatchObject({ board_id: 'board', item_id: 'item', restore: true });
    expect(network.value.workspace?.selected_chat).toBeNull();
  });
});
