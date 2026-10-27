// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { watch } from 'vue';
import { installIdentity, installWorkspace, network, savedWorkspace, saveWorkspace, type NetworkWorkspace } from './network-context';
import { openLibraryMaterial, recoverWorkspaceBinding, workspaceAction } from './network-client';

const source = { book_id: 'old-book', publication_id: 'old-publication' };
const target = { book_id: 'target-book', publication_id: 'target-publication' };
const scene = { workspace_id: 'w', generation: 2, revision: 3, selected_chat: 'chat', published_book_ref: source,
  learning_workspace_id: 'learning', reader: { viewport: { top_lid: 'chapter.7' } } } as unknown as NetworkWorkspace;
const json = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status });
const input = (init: RequestInit) => JSON.parse(String(init.body));
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}

beforeEach(() => {
  sessionStorage.clear(); localStorage.clear();
  installIdentity({ user_id: 'reader', csrf_token: 'csrf' });
});
afterEach(() => { vi.unstubAllGlobals(); });

describe('library opening request chain', () => {
  it('uses the attached fork receipt directly, without installing the restored old book', async () => {
    saveWorkspace('w', 'library');
    const attached = { ...scene, workspace_id: 'fork', generation: 4, revision: 5 };
    const opened = { ...attached, generation: 5, revision: 6, published_book_ref: target };
    const installed: NetworkWorkspace[] = [];
    const stop = watch(() => network.value.workspace, value => { if (value) installed.push(value); }, { flush: 'sync' });
    const fetch = vi.fn(async (url: string, init: RequestInit) => {
      if (init.method === 'GET') return json(scene);
      expect(network.value.workspace).toBeNull();
      if (url.endsWith('/attach')) return json(attached);
      expect(savedWorkspace()?.workspace_id).toBe('fork');
      expect(input(init)).toMatchObject({ attachment_id: network.value.attachment, generation: 4, expected_revision: 5,
        published_book_ref: target, learning_workspace_id: 'learning' });
      return json(opened);
    });
    vi.stubGlobal('fetch', fetch);
    try {
      expect(await openLibraryMaterial(target, 'learning')).toEqual(opened);
      expect(fetch.mock.calls.map(([url, init]) => `${init.method} ${url}`)).toEqual([
        'GET /api/workspaces/w', 'POST /api/workspaces/w/attach', 'POST /api/workspaces/fork/book/open',
      ]);
      expect(installed).toEqual([opened]);
    } finally { stop(); }
  });

  it('resumes an exact material and scope in two calls, preserving chat and position', async () => {
    saveWorkspace('w', 'library');
    const attached = { ...scene, generation: 4, revision: 5 };
    const fetch = vi.fn(async (_url: string, init: RequestInit) => json(init.method === 'GET' ? scene : attached));
    vi.stubGlobal('fetch', fetch);
    await openLibraryMaterial(source, 'learning');
    expect(fetch.mock.calls.map(([url]) => url)).toEqual(['/api/workspaces/w', '/api/workspaces/w/attach']);
    expect(network.value.workspace).toEqual(attached);
    expect(network.value.workspace?.reader).toEqual(scene.reader);
    expect(network.value.workspace?.selected_chat).toBe('chat');
  });

  it('validates the warm same-material binding before preserving its chat and position', async () => {
    installWorkspace(scene);
    const resumed = { ...scene, generation: 5, revision: 6 };
    const fetch = vi.fn(async (_url: string, init: RequestInit) => json(init.method === 'GET' ? { ...scene, generation: 4 } : resumed));
    vi.stubGlobal('fetch', fetch);
    await openLibraryMaterial(source, 'learning');
    expect(fetch.mock.calls.map(([url]) => url)).toEqual(['/api/workspaces/w', '/api/workspaces/w/attach']);
    expect(network.value.workspace).toEqual(resumed);
  });

  it('opens a warm different material with one optimistic write', async () => {
    installWorkspace(scene);
    const fetch = vi.fn(async (_url: string, init: RequestInit) => {
      expect(input(init)).toMatchObject({ generation: 2, expected_revision: 3, published_book_ref: target });
      return json({ ...scene, generation: 3, revision: 4, published_book_ref: target });
    });
    vi.stubGlobal('fetch', fetch);
    await openLibraryMaterial(target, 'learning');
    expect(fetch.mock.calls.map(([url]) => url)).toEqual(['/api/workspaces/w/book/open']);
  });

  it('opens the same publication when the requested learning scope differs', async () => {
    installWorkspace(scene);
    const fetch = vi.fn(async (_url: string, init: RequestInit) => {
      expect(input(init).learning_workspace_id).toBeNull();
      return json({ ...scene, generation: 3, revision: 4, learning_workspace_id: null });
    });
    vi.stubGlobal('fetch', fetch);
    await openLibraryMaterial(source, null);
    expect(fetch).toHaveBeenCalledOnce();
    expect(network.value.workspace?.learning_workspace_id).toBeNull();
  });

  it('chooses a saved readable learning member after recovery within the same chain', async () => {
    saveWorkspace('w', 'library');
    const restored = { ...scene, learning_workspace_positions: { learning: target } };
    const fetch = vi.fn(async (url: string, init: RequestInit) => {
      if (url.endsWith('/book/open')) {
        expect(input(init).published_book_ref).toEqual(target);
        return json({ ...restored, generation: 4, revision: 5, published_book_ref: target });
      }
      return json(restored);
    });
    vi.stubGlobal('fetch', fetch);
    await openLibraryMaterial(source, 'learning', undefined, [source, target]);
    expect(fetch).toHaveBeenCalledTimes(3);
    expect(network.value.workspace?.published_book_ref).toEqual(target);
  });

  it('ignores a remembered learning member that is no longer readable', async () => {
    saveWorkspace('w', 'library');
    const restored = { ...scene, learning_workspace_positions: { learning: target } };
    const fetch = vi.fn(async () => json(restored));
    vi.stubGlobal('fetch', fetch);
    await openLibraryMaterial(source, 'learning', undefined, [source]);
    expect(fetch).toHaveBeenCalledTimes(2);
    expect(network.value.workspace?.published_book_ref).toEqual(source);
  });

  it('creates a workspace only after a saved pointer is confirmed missing', async () => {
    saveWorkspace('missing', 'library');
    const fetch = vi.fn(async (_url: string, init: RequestInit) => init.method === 'GET'
      ? json({ error_code: 'OBJECT_NOT_FOUND' }, 404)
      : json({ ...scene, workspace_id: 'created', published_book_ref: target }));
    vi.stubGlobal('fetch', fetch);
    await openLibraryMaterial(target, 'learning');
    expect(fetch.mock.calls.map(([url]) => url)).toEqual(['/api/workspaces/missing', '/api/workspaces']);
    expect(network.value.workspace?.workspace_id).toBe('created');
  });

  it('does not replace a saved workspace when reading it fails transiently', async () => {
    saveWorkspace('w', 'library');
    const fetch = vi.fn(async () => json({ error_code: 'UNAVAILABLE' }, 503));
    vi.stubGlobal('fetch', fetch);
    await expect(openLibraryMaterial(target, null)).rejects.toMatchObject({ status: 503 });
    expect(fetch).toHaveBeenCalledOnce();
    expect(network.value.workspace).toBeNull();
  });

  it('refreshes and retries a warm revision conflict once using the new revision', async () => {
    installWorkspace(scene);
    let posts = 0;
    const fetch = vi.fn(async (_url: string, init: RequestInit) => {
      if (init.method === 'GET') return json({ ...scene, revision: 8 });
      if (++posts === 1) return json({ error_code: 'WORKSPACE_REVISION_CONFLICT' }, 409);
      expect(input(init).expected_revision).toBe(8);
      return json({ ...scene, generation: 3, revision: 9, published_book_ref: target });
    });
    vi.stubGlobal('fetch', fetch);
    await openLibraryMaterial(target, 'learning');
    expect(fetch.mock.calls.map(([url]) => url)).toEqual([
      '/api/workspaces/w/book/open', '/api/workspaces/w', '/api/workspaces/w/book/open',
    ]);
    expect(network.value.workspace?.revision).toBe(9);
  });

  it('recovers an idle binding after an explicit stale rejection before opening', async () => {
    installWorkspace(scene);
    let posts = 0;
    const restored = { ...scene, generation: 4, revision: 5 };
    const fetch = vi.fn(async (url: string, init: RequestInit) => {
      if (url.endsWith('/book/open') && ++posts === 1) return json({ error_code: 'WORKSPACE_STALE' }, 409);
      if (url.endsWith('/book/open')) {
        expect(input(init)).toMatchObject({ generation: 4, expected_revision: 5 });
        return json({ ...restored, published_book_ref: target });
      }
      return json(restored);
    });
    vi.stubGlobal('fetch', fetch);
    await openLibraryMaterial(target, 'learning');
    expect(fetch.mock.calls.map(([url]) => url)).toEqual([
      '/api/workspaces/w/book/open', '/api/workspaces/w', '/api/workspaces/w/attach', '/api/workspaces/w/book/open',
    ]);
  });

  it('reconciles a lost open response on manual retry without resetting the completed target', async () => {
    installWorkspace(scene);
    const committed = { ...scene, generation: 3, revision: 4, published_book_ref: target, selected_chat: 'kept-chat' };
    const fetch = vi.fn(async (url: string) => {
      if (url.endsWith('/book/open')) throw new TypeError('response lost');
      return json(committed);
    });
    vi.stubGlobal('fetch', fetch);
    await expect(openLibraryMaterial(target, 'learning')).rejects.toThrow('response lost');
    expect(fetch).toHaveBeenCalledOnce();
    await openLibraryMaterial(target, 'learning');
    expect(fetch.mock.calls.map(([url]) => url)).toEqual([
      '/api/workspaces/w/book/open', '/api/workspaces/w', '/api/workspaces/w/attach',
    ]);
    expect(network.value.workspace?.selected_chat).toBe('kept-chat');
  });

  it('retains an acknowledged fork when opening its target subsequently fails', async () => {
    saveWorkspace('w', 'library');
    const fork = { ...scene, workspace_id: 'fork', generation: 3, revision: 4 };
    let attempts = 0;
    const fetch = vi.fn(async (url: string, init: RequestInit) => {
      if (url.endsWith('/book/open') && ++attempts === 1) throw new TypeError('response lost');
      if (url.endsWith('/book/open')) return json({ ...fork, published_book_ref: target });
      return json(init.method === 'GET' && url.endsWith('/w') ? scene : fork);
    });
    vi.stubGlobal('fetch', fetch);
    await expect(openLibraryMaterial(target, 'learning')).rejects.toThrow('response lost');
    expect(network.value.workspace).toBeNull();
    expect(savedWorkspace()?.workspace_id).toBe('fork');
    await openLibraryMaterial(target, 'learning');
    expect(fetch.mock.calls.slice(3).map(([url]) => url)).toEqual([
      '/api/workspaces/fork', '/api/workspaces/fork/attach', '/api/workspaces/fork/book/open',
    ]);
  });

  it('retries an unacknowledged creation using the same attachment and target', async () => {
    let attempts = 0;
    const fetch = vi.fn(async () => {
      if (++attempts === 1) throw new TypeError('response lost');
      return json({ ...scene, workspace_id: 'created', published_book_ref: target });
    });
    vi.stubGlobal('fetch', fetch);
    await expect(openLibraryMaterial(target, 'learning')).rejects.toThrow('response lost');
    expect(fetch).toHaveBeenCalledOnce();
    await openLibraryMaterial(target, 'learning');
    expect(fetch.mock.calls).toHaveLength(2);
    expect(fetch.mock.calls[1]).toEqual(fetch.mock.calls[0]);
    expect(network.value.workspace?.workspace_id).toBe('created');
  });
});

describe('opening cancellation and identity authority', () => {
  it('does not supersede reader navigation across an intervening library open', async () => {
    installWorkspace(scene);
    const pending = deferred<Response>();
    const calls: string[] = [];
    const fetch = vi.fn(async (url: string, init: RequestInit) => {
      calls.push(url);
      if (url.endsWith('/reader/position')) return pending.promise;
      if (url.endsWith('/book/open')) return json({ ...scene, generation: 3, published_book_ref: target });
      expect(input(init).lid).toBe('chapter.8');
      return json(scene);
    });
    vi.stubGlobal('fetch', fetch);
    const earlier = workspaceAction('reader/position');
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledOnce());
    const beforeOpen = workspaceAction('reader/goto', { lid: 'chapter.8' });
    const opening = openLibraryMaterial(target, 'learning');
    const afterOpen = workspaceAction('reader/goto', { lid: 'chapter.9' });
    const rejected = expect(afterOpen).rejects.toMatchObject({ errorCode: 'CLIENT_CONTEXT_STALE' });
    pending.resolve(json(scene));
    await earlier; await beforeOpen; await opening; await rejected;
    expect(calls).toEqual(['/api/workspaces/w/reader/position', '/api/workspaces/w/reader/goto', '/api/workspaces/w/book/open']);
  });

  it('releases an aborted queued open immediately, without sending it or overlapping the earlier write', async () => {
    installWorkspace(scene);
    const pending = deferred<Response>();
    const fetch = vi.fn(async (url: string) => url.endsWith('/reader/position') ? pending.promise
      : json({ ...scene, revision: 4, published_book_ref: target }));
    vi.stubGlobal('fetch', fetch);
    const earlier = workspaceAction('reader/position', { top_lid: 'chapter.8' });
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledOnce());
    const controller = new AbortController();
    const timedOut = openLibraryMaterial(target, 'learning', controller.signal);
    const rejected = expect(timedOut).rejects.toThrow('开书超时');
    controller.abort(new Error('开书超时'));
    await rejected;
    const retry = openLibraryMaterial(target, 'learning');
    await Promise.resolve();
    expect(fetch).toHaveBeenCalledOnce();
    pending.resolve(json({ ...scene, revision: 4 }));
    await earlier; await retry;
    expect(fetch.mock.calls.map(([url]) => url)).toEqual(['/api/workspaces/w/reader/position', '/api/workspaces/w/book/open']);
  });

  it('keeps an in-flight request ordered after timeout and remembers its late receipt without installing', async () => {
    installWorkspace(scene);
    const pending = deferred<Response>();
    const committed = { ...scene, generation: 3, revision: 4, published_book_ref: target };
    const fetch = vi.fn(async (url: string) => url.endsWith('/book/open') ? pending.promise : json(committed));
    vi.stubGlobal('fetch', fetch);
    const controller = new AbortController();
    const opening = openLibraryMaterial(target, 'learning', controller.signal);
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledOnce());
    const rejected = expect(opening).rejects.toThrow('开书超时');
    controller.abort(new Error('开书超时'));
    await rejected;
    const retry = openLibraryMaterial(target, 'learning');
    await Promise.resolve();
    expect(fetch).toHaveBeenCalledOnce();
    pending.resolve(json(committed));
    // The queued retry reads the receipt; the cancelled operation never mounts it.
    const installed: NetworkWorkspace[] = [];
    const stop = watch(() => network.value.workspace, value => { if (value) installed.push(value); }, { flush: 'sync' });
    try {
      await retry;
      expect(installed).toEqual([committed]);
      expect(fetch.mock.calls.map(([url]) => url)).toEqual([
        '/api/workspaces/w/book/open', '/api/workspaces/w', '/api/workspaces/w/attach',
      ]);
    } finally { stop(); }
  });

  it('stops after an aborted cold read even if its response arrives late', async () => {
    saveWorkspace('w', 'library');
    const pending = deferred<Response>();
    const fetch = vi.fn(() => pending.promise);
    vi.stubGlobal('fetch', fetch);
    const controller = new AbortController();
    const opening = openLibraryMaterial(target, 'learning', controller.signal);
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledOnce());
    const rejected = expect(opening).rejects.toThrow('开书超时');
    controller.abort(new Error('开书超时')); await rejected;
    pending.resolve(json(scene));
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(fetch).toHaveBeenCalledOnce();
    expect(network.value.workspace).toBeNull();
  });

  it('never installs or stores an opening receipt after an identity switch', async () => {
    const pending = deferred<Response>();
    const fetch = vi.fn(() => pending.promise);
    vi.stubGlobal('fetch', fetch);
    const opening = openLibraryMaterial(target, 'learning');
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledOnce());
    installIdentity({ user_id: 'another-reader', csrf_token: 'another-csrf' });
    pending.resolve(json({ ...scene, published_book_ref: target }));
    await expect(opening).rejects.toMatchObject({ errorCode: 'CLIENT_CONTEXT_STALE' });
    expect(network.value.workspace).toBeNull();
    expect(savedWorkspace()).toBeNull();
  });

  it('applies the continuation deadline while waiting for the write queue', async () => {
    installWorkspace(scene);
    const pending = deferred<Response>();
    const fetch = vi.fn(() => pending.promise);
    vi.stubGlobal('fetch', fetch);
    const earlier = workspaceAction('reader/position', { top_lid: 'chapter.8' });
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledOnce());
    const controller = new AbortController();
    const recovering = recoverWorkspaceBinding(undefined, controller.signal);
    const rejected = expect(recovering).rejects.toThrow('恢复超时');
    controller.abort(new Error('恢复超时')); await rejected;
    pending.resolve(json({ ...scene, revision: 4 }));
    await earlier;
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(fetch).toHaveBeenCalledOnce();
  });

  it('rejects continuation if the same publication and chat moved to another learning scope', async () => {
    saveWorkspace('w', 'library');
    const fetch = vi.fn(async () => json({ ...scene, learning_workspace_id: 'another-learning' }));
    vi.stubGlobal('fetch', fetch);
    await expect(recoverWorkspaceBinding({ workspace_id: 'w', published_book_ref: source, selected_chat: 'chat',
      learning_workspace_id: 'learning', position_label: null, position_excerpt: null, last_question: null }))
      .rejects.toMatchObject({ errorCode: 'WORKSPACE_STALE' });
    expect(fetch).toHaveBeenCalledOnce();
    expect(network.value.workspace).toBeNull();
  });
});
