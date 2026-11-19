// @vitest-environment happy-dom
import './network-client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { api, type WorkspaceNoteImportSource } from '@understand-book/web/reader/api';
import { installIdentity, installWorkspace, network, type NetworkWorkspace } from './network-context';

const scene = {
  workspace_id: 'reader-window', learning_workspace_id: 'learning-cello', generation: 2, revision: 8,
  selected_chat: null, published_book_ref: { book_id: 'cello-book', publication_id: 'edition-1' }, reader: {},
} as NetworkWorkspace;
const notebook = { notes: [], relations: [] };
const json = (value: unknown) => new Response(JSON.stringify(value), { status: 200 });
type Request = { url: string; init: RequestInit; body: Record<string, unknown> };

function recordRequests(result: unknown = notebook, refreshedScene = scene) {
  const requests: Request[] = [];
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    requests.push({ url, init, body: init.body ? JSON.parse(String(init.body)) : {} });
    if (init.method === 'GET') return json(refreshedScene);
    return json(url.startsWith('/api/workspaces/') ? { ...refreshedScene, result } : result);
  }));
  return requests;
}

beforeEach(() => {
  sessionStorage.clear();
  localStorage.clear();
  installIdentity({ user_id: 'reader', csrf_token: 'notes-csrf' });
});
afterEach(() => {
  vi.unstubAllGlobals();
  network.value = { ...network.value, enabled: false, identity: null, workspace: null };
});

describe('workspace notebook browser transport', () => {
  it('uses the explicit account workspace with CSRF for every notebook action without opening a Reader', async () => {
    const requests = recordRequests();
    const scope = 'learning-cello';
    expect(await api.workspaceNotes(scope)).toEqual(notebook);
    await api.workspaceNote('note-a', scope);
    await api.workspaceNoteCreate({ title: '具体反馈', content: '记录本次练习的变化。' }, scope);
    await api.workspaceNoteUpdate({ note_id: 'note-a', expected_revision: 3, title: '修订', content: '修订内容' }, scope);
    await api.workspaceNoteDelete({ note_id: 'note-a', expected_revision: 4 }, scope);
    await api.workspaceNoteRelationSave({ from_note_id: 'note-a', to_note_id: 'note-b', label: '举例说明', reason: '这次练习体现了这条原则。' }, scope);
    await api.workspaceNoteRelationSave({ relation_id: 'relation-a', expected_revision: 5, from_note_id: 'note-a', to_note_id: 'note-b', label: '限制条件', reason: '进一步说明适用范围。' }, scope);
    await api.workspaceNoteRelationDelete({ relation_id: 'relation-a', expected_revision: 6 }, scope);

    expect(requests.map(request => request.url)).toEqual(
      ['list', 'read', 'create', 'update', 'delete', 'relation_save', 'relation_save', 'relation_delete']
        .map(operation => `/api/learning-workspaces/${scope}/notes/${operation}`),
    );
    for (const request of requests) {
      expect(request.init).toMatchObject({ method: 'POST', credentials: 'same-origin', headers: { 'X-CSRF-Token': 'notes-csrf' } });
      expect(request.body).not.toHaveProperty('expected_revision');
      expect(request.body).not.toHaveProperty('generation');
      expect(request.body).not.toHaveProperty('attachment_id');
    }
    expect(requests[1].body).toEqual({ note_id: 'note-a' });
    expect(requests[2].body).toEqual({ title: '具体反馈', content: '记录本次练习的变化。' });
    expect(requests[3].body).toMatchObject({ note_revision: 3 });
    expect(requests[4].body).toEqual({ note_id: 'note-a', note_revision: 4 });
    expect(requests[5].body).not.toHaveProperty('relation_revision');
    expect(requests[6].body).toMatchObject({ relation_revision: 5 });
    expect(requests[7].body).toEqual({ relation_id: 'relation-a', relation_revision: 6 });
    expect(network.value.workspace).toBeNull();
  });

  it('keeps note and relation CAS revisions separate from the active Reader scene revision', async () => {
    installWorkspace(scene);
    const result = { note_id: 'note-a', revision: 4 };
    const requests = recordRequests(result, { ...scene, revision: 12 });
    expect(await api.workspaceNoteUpdate({ note_id: 'note-a', expected_revision: 3, title: '修订', content: '修改后的内容' })).toEqual(result);
    await api.workspaceNoteDelete({ note_id: 'note-a', expected_revision: 4 });
    await api.workspaceNoteRelationSave({ relation_id: 'relation-a', expected_revision: 5, from_note_id: 'note-a', to_note_id: 'note-b', label: '支持', reason: '提供了具体例子。' });
    await api.workspaceNoteRelationDelete({ relation_id: 'relation-a', expected_revision: 6 });

    const posts = requests.filter(request => request.init.method === 'POST');
    expect(posts.map(request => request.url)).toEqual(
      ['update', 'delete', 'relation_save', 'relation_delete'].map(operation => `/api/workspaces/reader-window/notes/${operation}`),
    );
    for (const request of posts) {
      expect(request.body).toMatchObject({ expected_revision: 12, generation: 2, attachment_id: network.value.attachment });
    }
    expect(posts[0].body).toMatchObject({ note_revision: 3, title: '修订', content: '修改后的内容' });
    expect(posts[1].body).toMatchObject({ note_revision: 4 });
    expect(posts[2].body).toMatchObject({ relation_revision: 5, label: '支持', reason: '提供了具体例子。' });
    expect(posts[3].body).toMatchObject({ relation_revision: 6 });
    expect(network.value.workspace?.selected_chat).toBeNull();
  });

  it('uses the active Reader route for an account notebook without an explicit scope', async () => {
    installWorkspace(scene);
    const requests = recordRequests();
    expect(await api.workspaceNotes()).toEqual(notebook);
    await api.workspaceNote('note-a', null);
    expect(requests.filter(request => request.init.method === 'POST').map(request => request.url)).toEqual([
      '/api/workspaces/reader-window/notes/list', '/api/workspaces/reader-window/notes/read',
    ]);
    expect(network.value.workspace?.learning_workspace_id).toBe('learning-cello');
  });

  it('keeps resident notebooks on local endpoints even when given a learning workspace hint', async () => {
    network.value = { ...network.value, enabled: false };
    const requests = recordRequests();
    expect(await api.workspaceNotes('learning-cello')).toEqual(notebook);
    await api.workspaceNoteUpdate({ note_id: 'note-a', expected_revision: 3, title: '修订', content: '本地内容' }, 'learning-cello');
    await api.workspaceNoteRelationDelete({ relation_id: 'relation-a', expected_revision: 6 });
    expect(requests.map(request => request.url)).toEqual(['/api/notes/list', '/api/notes/update', '/api/notes/relation_delete']);
    expect(requests[1].body).toEqual({ note_id: 'note-a', note_revision: 3, title: '修订', content: '本地内容' });
    expect(requests[2].body).toEqual({ relation_id: 'relation-a', relation_revision: 6 });
  });

  it.each([true, false])('preserves answer and reading-note import requests (account mode: %s)', async enabled => {
    installWorkspace(scene);
    network.value = { ...network.value, enabled };
    const imported = { note_id: 'imported-note', revision: 1 };
    const requests = recordRequests(imported);
    const sources: WorkspaceNoteImportSource[] = [
      { kind: 'answer', session_id: 'original-chat', turn_id: 'original-turn' },
      { kind: 'reading_note', mem_id: 'reading-memory' },
    ];
    for (const source of sources) {
      expect(await api.workspaceNoteImport({ source, title: '保留的想法', content: '用户整理后的正文' })).toEqual(imported);
    }
    const posts = requests.filter(request => request.init.method === 'POST');
    expect(posts).toHaveLength(2);
    for (const [index, request] of posts.entries()) {
      expect(request.url).toBe(enabled ? '/api/workspaces/reader-window/notes/import' : '/api/notes/import');
      expect(request.body).toMatchObject({ source: sources[index], title: '保留的想法', content: '用户整理后的正文' });
      expect(request.body.source).toEqual(sources[index]);
    }
    expect(network.value.workspace?.selected_chat).toBeNull();
  });
});
