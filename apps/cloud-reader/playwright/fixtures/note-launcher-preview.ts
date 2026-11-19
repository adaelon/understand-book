// Local, interactive preview using the real application shell and note components.
// All preview API data stays in this browser; this entry is not a production build input.
import { createApp } from 'vue';
import NoteLauncherPreview from './NoteLauncherPreview.vue';
import { network } from '@understand-book/web/reader/reader-context';
const storageKey = 'note-launcher-preview-saved';
const notes = new Map<string, any>(JSON.parse(localStorage.getItem(storageKey) || '[]'));
const originalFetch = window.fetch.bind(window);
window.fetch = async (input, init) => {
  const url = new URL(input instanceof Request ? input.url : String(input), location.href);
  if (url.origin !== location.origin || !url.pathname.startsWith('/api/')) return originalFetch(input, init);
  const path = url.pathname, body = typeof init?.body === 'string' ? JSON.parse(init.body) : {};
  let data: unknown = {};
  if (path.endsWith('/auth/me')) data = { user_id: 'note-preview', email: 'preview@local.test', csrf_token: 'preview' };
  else if (path.endsWith('/library')) data = { books: [] };
  else if (path.endsWith('/videos')) data = { videos: [] };
  else if (path.endsWith('/learning-workspaces')) data = { workspaces: [] };
  else if (path.endsWith('/reading-resumption')) data = null;
  else if (path.includes('/workspace-releases')) data = { items: [], total: 0, limit: 12, offset: 0 };
  else if (path.endsWith('/me/notes/list')) data = { notes: [...notes.values()].reverse(), next_cursor: null };
  else if (path.endsWith('/me/notes/create') || path.endsWith('/me/notes/revise')) {
    const id = body.note_id || crypto.randomUUID(), previous = notes.get(id);
    const revision = (previous?.current_revision ?? 0) + 1, now = new Date().toISOString();
    const value = { ...body.value, revision, saved_at: now, record: { content: body.value.content, note: body.value.source, anchor: { lid: body.value.context?.lid ?? null } } };
    data = { note_id: id, current_revision: revision, created_at: previous?.created_at ?? now, value, revisions: [value, ...(previous?.revisions ?? [])] };
    notes.set(id, data); localStorage.setItem(storageKey, JSON.stringify([...notes]));
  } else if (path.endsWith('/me/notes/read')) data = notes.get(body.note_id);
  else if (path.endsWith('/me/notes/history')) data = { revisions: notes.get(body.note_id)?.revisions ?? [], discussions: [] };
  else if (path.endsWith('/me/notes/delete')) { notes.delete(body.note_id); localStorage.setItem(storageKey, JSON.stringify([...notes])); data = { removed: true }; }
  else if (path.endsWith('/me/notes/search')) data = { hits: [], semantic: { status: 'unavailable' } };
  else if (path.endsWith('/me/notes/links')) data = { links: [] };
  else if (path.endsWith('/me/notes/attachments') || path.endsWith('/me/notes/board_locations')) data = { items: [] };
  else if (path.endsWith('/me/notes/chats')) data = { sessions: [] };
  else if (path.endsWith('/tutor/activities')) data = { activities: [] };
  return Response.json(data ?? {});
};
await import('../../src/network-main');
await new Promise<void>(resolve => {
  if (document.querySelector('.network-library')) { resolve(); return; }
  const observer = new MutationObserver(() => {
    if (document.querySelector('.network-library')) { observer.disconnect(); resolve(); }
  });
  observer.observe(document.body, { subtree: true, childList: true });
});
network.value = { ...network.value, enabled: false };
const host = document.createElement('div');
host.style.cssText = 'position:absolute;inset:0;background:var(--canvas);z-index:1';
document.querySelector('.application-content')!.append(host);
createApp(NoteLauncherPreview).mount(host);
