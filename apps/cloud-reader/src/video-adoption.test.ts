// @vitest-environment happy-dom
import { expect, it, vi } from 'vitest';
import { createVideoAdoption } from '@understand-book/web/reader/video-adoption';
import { installIdentity, installWorkspace, readerKey, sceneKey, network, type NetworkWorkspace } from './network-context';

it('only requests gaps, serializes requests, recovers current projections and ignores disposed responses', async () => {
  let idle = false;
  let resolve!: (v: any) => void;
  const request = vi.fn(() => new Promise<any>(r => { resolve = r; }));
  const apply = vi.fn();
  const adoption = createVideoAdoption(() => idle, request, apply);
  await adoption.refresh(); expect(request).not.toHaveBeenCalled();
  idle = true; const first = adoption.refresh();
  expect(adoption.adopting.value).toBe(true);
  await adoption.refresh(); expect(request).toHaveBeenCalledTimes(1);
  resolve({ status: 'current', video: { publication: { publication_id: 'p2' } } }); await first;
  expect(apply).toHaveBeenCalledTimes(1);
  const second = adoption.refresh(); adoption.dispose();
  resolve({ status: 'adopted', video: {} }); await second;
  expect(apply).toHaveBeenCalledTimes(1);
});

it('preserves mounted Reader through adoption, lost-response recovery and chat change; ordinary open remounts', () => {
  installIdentity({ user_id: 'vi9', csrf_token: 'csrf' });
  const first = { workspace_id: 'w', generation: 1, revision: 1, selected_chat: 'chat',
    published_book_ref: { book_id: 'b', publication_id: 'p1' }, reader: {} } as NetworkWorkspace;
  installWorkspace(first);
  const reader = readerKey(), scene = sceneKey();
  const next = { ...first, revision: 2, published_book_ref: { book_id: 'b', publication_id: 'p2' } };
  installWorkspace(next, network.value.attachment, false, 'enrichment');
  expect(readerKey()).toBe(reader); expect(sceneKey()).not.toBe(scene);
  installWorkspace({ ...next, revision: 3, published_book_ref: { book_id: 'b', publication_id: 'p3' } });
  expect(readerKey()).toBe(reader);
  const chat = { ...network.value.workspace!, generation: 2, selected_chat: 'other' };
  installWorkspace(chat, network.value.attachment, false, 'chat'); expect(readerKey()).toBe(reader);
  installWorkspace({ ...chat, generation: 3, published_book_ref: first.published_book_ref }); expect(readerKey()).not.toBe(reader);
  installIdentity(null);
});


it('remounts a learning workspace on selected publication updates and scope changes', () => {
  installIdentity({ user_id: 'shared-video', csrf_token: 'csrf' });
  const first = { workspace_id: 'w', learning_workspace_id: 'learning-a', generation: 1, revision: 1, selected_chat: 'chat',
    published_book_ref: { book_id: 'video', publication_id: 'v1' }, reader: {} } as NetworkWorkspace;
  installWorkspace(first);
  const initial = readerKey();
  installWorkspace({ ...first, revision: 2, published_book_ref: { book_id: 'video', publication_id: 'v2' } });
  expect(readerKey()).not.toBe(initial);
  const updated = readerKey();
  installWorkspace({ ...network.value.workspace!, learning_workspace_id: 'learning-b' });
  expect(readerKey()).not.toBe(updated);
  installIdentity(null);
});
