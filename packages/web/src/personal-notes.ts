import { reactive, ref } from 'vue';
import { network } from './network-context';
import { ApiError } from './api';
import type { NoteContent } from './generated/NoteContent';
import type { NoteContext } from './generated/NoteContext';
import type { PersonalNoteView } from './generated/PersonalNoteView';
import type { NoteRevision } from './generated/NoteRevision';
import type { NoteRef } from './generated/NoteRef';
import type { NoteDiscussion } from './generated/NoteDiscussion';
import type { NoteLink } from './generated/NoteLink';
import type { NoteSearchHit } from './generated/NoteSearchHit';
import { placeThought, type NoteDestination } from './note-placements';
export type { NoteContent, NoteContext, PersonalNoteView, NoteRevision, NoteRef, NoteLink };

export const notesOwner = () => network.value.enabled ? network.value.identity?.user_id ?? null : 'local';
export async function noteRequest<T>(action: string, input: unknown): Promise<T> {
  return ownerRequest<T>(`/me/notes/${action}`, 'POST', input);
}
/** User-owned requests remain valid when the reading scene changes. */
export async function ownerRequest<T>(path: string, method = 'GET', input?: unknown): Promise<T> {
  const owner = notesOwner();
  if (!owner) throw new Error('登录后可以保存自己的想法');
  const identity = network.value.identity;
  const result = await fetch(`/api${path}`, {
    method, credentials: 'same-origin', cache: 'no-store',
    headers: { 'Content-Type': 'application/json', ...(identity ? { 'X-CSRF-Token': identity.csrf_token } : {}) },
    ...(input === undefined ? {} : { body: JSON.stringify(input) }),
  });
  const value = await result.json();
  if (notesOwner() !== owner) throw new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', '当前账号已切换，原草稿保留在原账号');
  if (!result.ok) throw new ApiError(result.status, value.error_code, value.category, value.message ?? '想法暂时无法保存');
  return value as T;
}
export const personalNotes = {
  search: (query: string, bookId?: string) => noteRequest<{hits: NoteSearchHit[]; semantic: {status: string; reason?: string}}>('search',{query,book_id:bookId,limit:20}),
  list: (before?: string, bookId?: string) => noteRequest<{ notes: PersonalNoteView[]; next_cursor: string | null }>('list', { before, book_id: bookId }),
  read: (reference: NoteRef) => noteRequest<PersonalNoteView>('read', reference),
  history: (noteId: string, before?: number) => noteRequest<{ revisions: NoteRevision[]; discussions: NoteDiscussion[] }>('history', { note_id: noteId, before }),
  restore: (noteId: string, base: number, revision: number, operationId: string) => noteRequest<PersonalNoteView>('restore', { note_id: noteId, base_revision: base, revision, operation_id: operationId }),
  delete: (noteId: string) => noteRequest('delete', { note_id: noteId }),
  links: (noteId: string) => noteRequest<{ links: NoteLink[] }>('links', { note_id: noteId }),
  link: (from: NoteRef, to: NoteRef, reason?: string) => noteRequest<NoteLink>('link', { from, to, reason }),
  unlink: (linkId: string) => noteRequest('unlink', { link_id: linkId }),
};
export const notesPanel = reactive<{ open: boolean; view: 'capture' | 'library' | 'detail'; draftId: string | null; reference: NoteRef | null }>({ open: false, view: 'library', draftId: null, reference: null });
export const notesChanged = ref(0);
export const pendingNoteSource = ref<NoteContext | null>(null);
export const noteSourceError = ref('');
const thoughtContextReaders: { read: () => Partial<NoteContext> | null; priority: number }[] = [];
/** Read the visible scene at capture time, then freeze it in the device draft. */
export function registerThoughtContext(reader: () => Partial<NoteContext> | null, priority = 0) {
  const entry = { read: reader, priority }; thoughtContextReaders.push(entry);
  return () => { const index = thoughtContextReaders.indexOf(entry); if (index >= 0) thoughtContextReaders.splice(index, 1); };
}
export function currentThoughtContext(): Partial<NoteContext> | null {
  // A temporary source reader covers its host; do not guess the board/message behind it.
  if (document.querySelector('.board-source-dialog')) return null;
  for (const entry of [...thoughtContextReaders].reverse().sort((a,b) => b.priority-a.priority)) { const context = entry.read(); if (context) return context; }
  return null;
}
export function openPersonalNotes(reference?: NoteRef) { notesPanel.open = true; notesPanel.view = reference ? 'detail' : 'library'; notesPanel.reference = reference ?? null; notesPanel.draftId = null; }
/** The global capture entry resumes work without creating another empty draft. */
export function openThoughtCapture(context = currentThoughtContext()) {
  const available = readNoteDrafts();
  const draft = available.find(value => value.draft_id === notesPanel.draftId) ?? available[0];
  if (!draft) return startThought(context ?? {});
  notesPanel.reference = null; notesPanel.draftId = draft.draft_id; notesPanel.view = 'capture'; notesPanel.open = true;
  return draft;
}
export const emptyNoteContext = (): NoteContext => ({ book_id: null, publication_id: null, workspace_id: null, lid: null, video_time_ms: null, session_id: null, turn_id: null, label: null });
export const emptyNoteContent = (context: Partial<NoteContext> = {}, content = ''): NoteContent => ({
  content, title: null, authorship: 'user', context: { ...emptyNoteContext(), ...context }, source_refs: [], change_note: null, based_on_turn: null, source: null, capture: null,
});
export function noteContentFromView(note: PersonalNoteView): NoteContent {
  return { content: note.value.record.content, title: note.value.title, authorship: note.value.authorship,
    context: JSON.parse(JSON.stringify(note.value.context)), source_refs: JSON.parse(JSON.stringify(note.value.source_refs)), change_note: null, based_on_turn: note.value.based_on_turn, source: JSON.parse(JSON.stringify(note.value.record.note ?? null)),
    capture: { anchor: note.value.record.anchor, selection_context: note.value.record.selection_context ?? null, placement: note.value.record.note_placement ?? null } };
}
export interface NoteDraft {
  draft_id: string; owner: string; note_id: string | null; base_revision: number | null; value: NoteContent; updated_at: string;
  pending: { operation_id: string; value: NoteContent; base_revision: number | null } | null;
  destination?: NoteDestination;
  saved_note?: PersonalNoteView;
}
const drafts = reactive(new Map<string, NoteDraft>());
export const draftStorageError = ref('');
const keyPrefix = (owner: string) => `understand-book:thought-draft:${encodeURIComponent(owner)}:`;
export function readNoteDrafts(): NoteDraft[] {
  const owner = notesOwner(); if (!owner) return [];
  try {
    for (const key of Object.keys(localStorage).filter(key => key.startsWith(keyPrefix(owner)))) {
      const saved = JSON.parse(localStorage.getItem(key)!);
      if (saved?.owner === owner && typeof saved.value?.content === 'string' && !drafts.has(saved.draft_id)) drafts.set(saved.draft_id, saved);
    }
  } catch { draftStorageError.value = '设备草稿暂时无法读取；已打开的文字仍保留在这里。'; }
  return [...drafts.values()].filter(draft => draft.owner === owner).sort((a, b) => b.updated_at.localeCompare(a.updated_at));
}
export function keepNoteDraft(draft: NoteDraft) {
  draft.updated_at = new Date().toISOString(); drafts.set(draft.draft_id, draft);
  try { localStorage.setItem(keyPrefix(draft.owner) + draft.draft_id, JSON.stringify(draft)); draftStorageError.value = ''; }
  catch { draftStorageError.value = '设备空间不足，草稿暂未写入设备；请保持此页面并尝试保存。'; }
}
export function discardNoteDraft(draft: NoteDraft) {
  localStorage.removeItem(keyPrefix(draft.owner) + draft.draft_id); drafts.delete(draft.draft_id);
}
export function startThought(context: Partial<NoteContext> = currentThoughtContext() ?? {}, text = '', value?: NoteContent): NoteDraft {
  const owner = notesOwner(); if (!owner) throw new Error('请先登录');
  const draft: NoteDraft = { draft_id: crypto.randomUUID(), owner, note_id: null, base_revision: null, value: value ?? emptyNoteContent(context, text), updated_at: '', pending: null };
  keepNoteDraft(draft); notesPanel.open = true; notesPanel.view = 'capture'; notesPanel.draftId = draft.draft_id; notesPanel.reference = null;
  return drafts.get(draft.draft_id)!;
}
export function editThought(note: PersonalNoteView): NoteDraft {
  const existing = readNoteDrafts().find(draft => draft.note_id === note.note_id);
  if (existing) return existing;
  const owner = notesOwner(); if (!owner) throw new Error('请先登录');
  const draft: NoteDraft = { draft_id: crypto.randomUUID(), owner, note_id: note.note_id, base_revision: note.current_revision, value: noteContentFromView(note), updated_at: '', pending: null };
  keepNoteDraft(draft); return drafts.get(draft.draft_id)!;
}
export async function captureUserThought(sessionId: string, turnId: string) {
  const value = await noteRequest<NoteContent>('from_turn', { session_id: sessionId, turn_id: turnId });
  return startThought(value.context, value.content, value);
}
export async function saveThought(draft: NoteDraft): Promise<PersonalNoteView> {
  if (!draft.pending) { draft.pending = { operation_id: crypto.randomUUID(), value: JSON.parse(JSON.stringify(draft.value)), base_revision: draft.base_revision }; keepNoteDraft(draft); }
  try {
    const note = draft.saved_note ?? await noteRequest<PersonalNoteView>(draft.note_id ? 'revise' : 'create', { note_id: draft.note_id, ...draft.pending });
    if (draft.destination) {
      draft.saved_note = note; keepNoteDraft(draft);
      try { await placeThought(note.note_id, draft.destination); }
      catch (failure) { throw new Error(`想法已保存，放置未完成：${(failure as Error).message}`); }
    }
    discardNoteDraft(draft); notesChanged.value++; return note;
  } catch (error) {
    // A validation/conflict response proves no new write occurred. Unknown transport outcomes retain the original operation for retry.
    if (error instanceof ApiError && error.status < 500 && ![408,429].includes(error.status) && error.errorCode !== 'CLIENT_CONTEXT_STALE') { draft.pending = null; keepNoteDraft(draft); }
    throw error;
  }
}

/** Line-based comparison preserves literal code, equations and original wording. */
export function compareNoteText(left: string, right: string): { kind: 'same' | 'removed' | 'added'; text: string }[] {
  const a = left.split('\n'), b = right.split('\n'); let start = 0, end = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  while (end < a.length - start && end < b.length - start && a[a.length - end - 1] === b[b.length - end - 1]) end++;
  return [...a.slice(0, start).map(text => ({ kind: 'same' as const, text })),
    ...a.slice(start, a.length - end).map(text => ({ kind: 'removed' as const, text })),
    ...b.slice(start, b.length - end).map(text => ({ kind: 'added' as const, text })),
    ...a.slice(a.length - end).map(text => ({ kind: 'same' as const, text }))];
}
