import { onBeforeUnmount, ref, watch } from 'vue';
import { noteRequest, notesChanged, notesOwner, startThought, keepNoteDraft } from './personal-notes';
import type { NoteAttachmentTarget } from './generated/NoteAttachmentTarget';
import type { NoteAttachmentView } from './generated/NoteAttachmentView';
import type { NoteContext } from './generated/NoteContext';

export type NoteDestination = { kind: 'board'; board_id: string; workspace_id: string | null; item_id?: string; label: string }
  | { kind: 'attachment'; target: NoteAttachmentTarget; label: string };
export type NoteDestinationChoice = { key: string; label: string; detail: string; resolve: () => NoteDestination | Promise<NoteDestination> };
const providers = new Set<() => NoteDestinationChoice[]>();
/** Covered hosts stay mounted to preserve reading state, but are not the current scene. */
export function thoughtSurfaceVisible(element: HTMLElement | undefined): boolean {
  const rect = element?.getBoundingClientRect();
  if (!element || !rect?.width || !rect.height || rect.bottom <= 0 || rect.top >= window.innerHeight
    || rect.right <= 0 || rect.left >= window.innerWidth || getComputedStyle(element).visibility === 'hidden') return false;
  // These are the application's existing layers, in their visual stacking order.
  for (const selector of ['.board-source-dialog', '.personal-chat-overlay', '.board-workspace-overlay',
    '.agent-presentation.expanded[role="dialog"]', '.right-rail.presentation-workspace', '.right-rail.fullscreen']) {
    const layer = [...document.querySelectorAll<HTMLElement>(selector)].find(candidate =>
      candidate.getClientRects().length && getComputedStyle(candidate).visibility !== 'hidden');
    if (layer) return layer.contains(element);
  }
  return true;
}
export function registerNoteDestinations(provider: () => NoteDestinationChoice[]) { providers.add(provider); return () => providers.delete(provider); }
export function availableNoteDestinations() { return [...providers].flatMap(provider => provider()); }
export async function placeThought(noteId: string, destination: NoteDestination) {
  if (destination.kind === 'board') await noteRequest('attach_board', { note_id: noteId, ...destination });
  else await noteRequest('attach', { note_id: noteId, target: destination.target });
  notesChanged.value++;
  window.dispatchEvent(new Event('personal-notes-changed'));
}
export async function removeThoughtPlacement(id: string) {
  await noteRequest('unattach', { attachment_id: id }); notesChanged.value++;
}
export function startThoughtHere(destination: NoteDestination, context: Partial<NoteContext> = {}) {
  const draft = startThought({ ...context, label: destination.label,
    ...(destination.kind === 'attachment' ? { session_id: destination.target.kind === 'message' ? destination.target.session_id : destination.target.receipt.session_id,
      turn_id: destination.target.kind === 'message' ? destination.target.turn_id : destination.target.receipt.turn_id, origin: destination.target }
      : { workspace_id: destination.workspace_id, board_id: destination.board_id, board_item_id: destination.item_id }),
  });
  draft.destination = destination; keepNoteDraft(draft); return draft;
}

const cache = new Map<string, { version: number; promise: Promise<NoteAttachmentView[]> }>();
let cacheOwner: string | null = null;
/** All mounted messages in one session share one read, including after an edit. */
export function useNoteAttachments(session: () => string) {
  const items = ref<NoteAttachmentView[]>([]), error = ref('');
  let request = 0;
  watch([session, notesChanged, notesOwner], async ([sessionId]) => {
    const current = ++request; items.value = []; error.value = '';
    if (cacheOwner !== notesOwner()) { cache.clear(); cacheOwner = notesOwner(); }
    if (!sessionId || !notesOwner()) return;
    const key = `${notesOwner()}:${sessionId}`;
    let entry = cache.get(key);
    if (!entry || entry.version !== notesChanged.value) {
      entry = { version: notesChanged.value, promise: noteRequest<{items: NoteAttachmentView[]}>('attachments', { session_id: sessionId }).then(result => result.items) };
      cache.set(key, entry);
      if (cache.size > 20) cache.delete(cache.keys().next().value!);
    }
    try { const result = await entry.promise; if (current === request) items.value = result; }
    catch (failure) { cache.delete(key); if (current === request) error.value = (failure as Error).message; }
  }, { immediate: true });
  onBeforeUnmount(() => request++);
  return { items, error, retry: () => notesChanged.value++ };
}
export const activeNoteMessage = ref('');
export const pendingNotePlacementLocation = ref<NoteDestination | null>(null);
export type BoardNoteLocation = Extract<NoteDestination, {kind:'board'}> & {book_id:string;publication_id:string|null;item_id:string};
