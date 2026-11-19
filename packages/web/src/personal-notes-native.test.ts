// @vitest-environment happy-dom
import { flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { NoteDraft, PersonalNoteView } from './personal-notes';

const native = vi.hoisted(() => ({ invoke: vi.fn(), onCloseRequested: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: native.invoke, isTauri: () => true }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ onCloseRequested: native.onCloseRequested }) }));
type CloseHandler = (event: { preventDefault: () => void }) => Promise<void>;
let disk: Record<string, NoteDraft> | null;
let failWrites: boolean;
let writeBarrier: (() => Promise<void>) | null;
let closeHandler: CloseHandler;
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));

beforeEach(() => {
  vi.resetModules(); vi.clearAllMocks();
  window.__UNDERSTAND_BOOK_LOCAL__ = { noteDrafts: true, managedBuildEngine: false };
  localStorage.clear(); disk = null; failWrites = false; writeBarrier = null;
  native.onCloseRequested.mockImplementation(async (handler: CloseHandler) => { closeHandler = handler; return () => {}; });
  native.invoke.mockImplementation(async (command: string, args: {
    initialDrafts: Record<string, NoteDraft>; draftId: string; draft: NoteDraft | null;
  }) => {
    if (command === 'load_local_note_drafts') { disk ??= clone(args.initialDrafts); return clone(disk); }
    if (command === 'write_local_note_draft') {
      if (writeBarrier) await writeBarrier();
      if (failWrites) throw new Error('device is full');
      disk ??= {};
      if (args.draft === null) delete disk[args.draftId]; else disk[args.draftId] = clone(args.draft);
      return;
    }
    throw new Error(`Unexpected native command: ${command}`);
  });
});
afterEach(() => { delete window.__UNDERSTAND_BOOK_LOCAL__; vi.unstubAllEnvs(); vi.unstubAllGlobals(); });

async function openReader() {
  vi.resetModules();
  const notes = await import('./personal-notes');
  await notes.initializeNoteDraftStorage();
  return notes;
}

it('imports existing origin drafts once and preserves all pending save and placement state', async () => {
  const notes = await import('./personal-notes');
  const value = notes.emptyNoteContent({ book_id: 'book', video_time_ms: 1500 }, '未写完的想法');
  const draft: NoteDraft = {
    draft_id: 'existing', owner: 'local', note_id: 'note', base_revision: 2, value,
    updated_at: '2026-11-16T10:00:00Z', pending: { operation_id: 'retry-same-save', value: clone(value), base_revision: 2 },
    destination: { kind: 'board', board_id: 'board', workspace_id: null, label: '原画板' },
    saved_note: { note_id: 'note', current_revision: 3 } as PersonalNoteView,
  };
  localStorage.setItem('understand-book:thought-draft:local:existing', JSON.stringify(draft));
  await notes.initializeNoteDraftStorage();
  expect(notes.readNoteDrafts()).toEqual([draft]);
  expect(disk).toEqual({ existing: draft });
  expect(native.onCloseRequested).toHaveBeenCalledTimes(1);
  await notes.initializeNoteDraftStorage();
  expect(native.onCloseRequested).toHaveBeenCalledTimes(1);
});

it('restores from a different origin and never revives a deleted draft from an old origin mirror', async () => {
  const first = await openReader();
  const draft = first.startThought({ book_id: 'original-book', lid: '1.2' }, '跨端口保留的正文');
  draft.pending = { operation_id: 'save-once', value: clone(draft.value), base_revision: null };
  draft.destination = { kind: 'board', board_id: 'board', workspace_id: null, label: '原画板' };
  draft.saved_note = { note_id: 'already-created', current_revision: 1 } as PersonalNoteView;
  first.keepNoteDraft(draft); await first.flushNoteDrafts();
  const firstOrigin = Object.fromEntries(Object.keys(localStorage).map(key => [key, localStorage.getItem(key)!]));
  // The next server port has an independent, initially empty localStorage namespace.
  localStorage.clear();
  const second = await openReader();
  expect(second.readNoteDrafts()).toEqual([clone(draft)]);
  second.discardNoteDraft(second.readNoteDrafts()[0]); await second.flushNoteDrafts();
  expect(disk).toEqual({});
  // Reopening the original port must ignore its old mirror, including on repeated reads.
  localStorage.clear(); for (const [key, value] of Object.entries(firstOrigin)) localStorage.setItem(key, value);
  const third = await openReader();
  expect(third.readNoteDrafts()).toEqual([]);
  expect(third.readNoteDrafts()).toEqual([]);
  expect(disk).toEqual({});
});

it('serializes native updates and waits for the latest text before allowing window close', async () => {
  const notes = await openReader();
  let release!: () => void;
  const blocked = new Promise<void>(resolve => { release = resolve; });
  writeBarrier = () => blocked;
  const draft = notes.startThought({}, '第一版'); await flushPromises();
  draft.value.content = '第二版，关闭前最后输入'; notes.keepNoteDraft(draft);
  const preventDefault = vi.fn(); let closed = false;
  const close = closeHandler({ preventDefault }).then(() => { closed = true; });
  await flushPromises();
  expect(closed).toBe(false);
  expect(native.invoke.mock.calls.filter(([name]) => name === 'write_local_note_draft')).toHaveLength(1);
  release(); await close;
  expect(preventDefault).not.toHaveBeenCalled();
  expect(disk?.[draft.draft_id].value.content).toBe('第二版，关闭前最后输入');
});

it('keeps failed writes retryable, reports the failure, and blocks closing until persistence succeeds', async () => {
  const notes = await openReader(); failWrites = true;
  const draft = notes.startThought({}, '磁盘失败时保留'); await flushPromises();
  expect(notes.draftStorageError.value).toContain('暂未写入');
  expect(notes.readNoteDrafts()[0].value.content).toBe(draft.value.content);
  const event = { preventDefault: vi.fn() };
  notes.notesPanel.open = false;
  await closeHandler(event);
  expect(event.preventDefault).toHaveBeenCalledTimes(1);
  expect(notes.notesPanel.open).toBe(true);
  expect(disk).toEqual({});
  failWrites = false; await notes.flushNoteDrafts();
  expect(notes.draftStorageError.value).toBe('');
  expect(disk?.[draft.draft_id].value.content).toBe(draft.value.content);
});

it('does not import a stale origin when the authoritative native store cannot be read', async () => {
  const notes = await import('./personal-notes');
  localStorage.setItem('understand-book:thought-draft:local:stale', JSON.stringify({
    draft_id: 'stale', owner: 'local', value: notes.emptyNoteContent({}, '旧端口残留'), updated_at: '2026-11-16',
  }));
  native.invoke.mockRejectedValue(new Error('cannot read device drafts'));
  await notes.initializeNoteDraftStorage();
  expect(notes.readNoteDrafts()).toEqual([]);
  expect(notes.draftStorageError.value).toContain('无法读取');
});

it('keeps ordinary browser storage when the host does not declare native drafts', async () => {
  delete window.__UNDERSTAND_BOOK_LOCAL__;
  const notes = await openReader();
  const draft = notes.startThought({}, '浏览器草稿');
  expect(JSON.parse(localStorage.getItem(`understand-book:thought-draft:local:${draft.draft_id}`)!).value.content).toBe('浏览器草稿');
  expect(native.invoke).not.toHaveBeenCalled();
  notes.discardNoteDraft(draft);
  expect(notes.readNoteDrafts()).toEqual([]);
});
