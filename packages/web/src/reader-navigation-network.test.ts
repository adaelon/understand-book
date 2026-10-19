// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { api } from './api';
import { installIdentity, installWorkspace, network, type NetworkWorkspace } from './network-context';
import { workspaceAction } from './network-client';

const scene = { workspace_id: 'navigation', generation: 2, revision: 3, selected_chat: null,
  published_book_ref: { book_id: 'book', publication_id: 'edition' }, reader: {} } as NetworkWorkspace;
const json = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status });
beforeEach(() => { installIdentity({ user_id: 'reader', csrf_token: 'csrf' }); installWorkspace(scene); });
afterEach(() => { vi.unstubAllGlobals(); network.value = { ...network.value, enabled: false }; });

describe('directory navigation transport', () => {
  it('gets confirmed position and exact body with one POST, then reuses the returned revision', async () => {
    let revision = 3;
    const fetch = vi.fn(async (_url: string, init: RequestInit) => {
      expect(init.method).toBe('POST');
      expect(JSON.parse(String(init.body)).expected_revision).toBe(revision);
      return json({ ...scene, revision: ++revision, result: { ok: true, texts: [{ lid: '1.2', text: '原文😀' }] } });
    });
    vi.stubGlobal('fetch', fetch);
    expect(await api.goto('1.2', true)).toMatchObject({ ok: true, texts: [{ lid: '1.2', text: '原文😀' }] });
    expect(JSON.parse(String(fetch.mock.calls[0][1].body)).include_text).toBe(true);
    await api.goto('1.3');
    expect(fetch).toHaveBeenCalledTimes(2);
    expect(network.value.workspace?.revision).toBe(5);
  });

  it('refreshes and retries only an explicit uncommitted revision conflict', async () => {
    const fetch = vi.fn(async (_url: string, init: RequestInit) => {
      if (init.method === 'GET') return json({ ...scene, revision: 7 });
      if (JSON.parse(String(init.body)).expected_revision === 3) return json({ error_code: 'WORKSPACE_REVISION_CONFLICT' }, 409);
      return json({ ...scene, revision: 8, result: { ok: true } });
    });
    vi.stubGlobal('fetch', fetch);
    await api.goto('1.2');
    expect(fetch.mock.calls.map(([, init]) => init.method)).toEqual(['POST', 'GET', 'POST']);
    expect(network.value.workspace?.revision).toBe(8);
  });

  it.each(['WORKSPACE_STALE', 'WORKSPACE_STORAGE_FAILED'])('never replays %s', async error_code => {
    const fetch = vi.fn(async () => json({ error_code }, 409)); vi.stubGlobal('fetch', fetch);
    await expect(api.goto('1.2')).rejects.toMatchObject({ errorCode: error_code });
    expect(fetch).toHaveBeenCalledOnce();
  });

  it('does not retry into a changed generation after a revision conflict', async () => {
    const fetch = vi.fn(async (_url: string, init: RequestInit) => init.method === 'GET'
      ? json({ ...scene, generation: 9, revision: 10 }) : json({ error_code: 'WORKSPACE_REVISION_CONFLICT' }, 409));
    vi.stubGlobal('fetch', fetch);
    await expect(api.goto('1.2')).rejects.toMatchObject({ errorCode: 'WORKSPACE_STALE' });
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it('keeps the running navigation and only the latest queued target', async () => {
    let release!: (value: Response) => void;
    const fetch = vi.fn(async (_url: string, init: RequestInit) => {
      const input = JSON.parse(String(init.body));
      if (input.lid === '1.1') return new Promise<Response>(resolve => { release = resolve; });
      expect(input).toMatchObject({ lid: '1.3', expected_revision: 4 });
      return json({ ...scene, revision: 5, result: { ok: true } });
    });
    vi.stubGlobal('fetch', fetch);
    const first = api.goto('1.1');
    await vi.waitFor(() => expect(release).toBeTypeOf('function'));
    const second = api.goto('1.2');
    const cancelled = expect(second).rejects.toMatchObject({ name: 'AbortError' });
    const last = api.goto('1.3');
    release(json({ ...scene, revision: 4, result: { ok: true } }));
    await Promise.all([first, cancelled, last]);
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it('does not coalesce across a note command or replay an unknown network outcome', async () => {
    const calls: string[] = [];
    let revision = 3;
    vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
      if (init.method === 'GET') return json({ ...scene, revision });
      calls.push(url.split('/').at(-1)!);
      return json({ ...scene, revision: ++revision, result: { ok: true } });
    }));
    await Promise.all([api.goto('1.1'), workspaceAction('reader/note', { lid: '1.1', text: 'note' }), api.goto('1.3')]);
    expect(calls).toEqual(['goto', 'note', 'goto']);
    const fetch = vi.fn(async () => { throw new TypeError('connection lost'); }); vi.stubGlobal('fetch', fetch);
    await expect(api.goto('1.4')).rejects.toThrow('connection lost');
    expect(fetch).toHaveBeenCalledOnce();
  });
});
