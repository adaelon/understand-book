import { network, type ChatDraft } from './network-context';

export function learningSelectionKey() { return `understand-book:learning:${network.value.identity?.user_id}:selected`; }
export function savedLearningSelection(): string | null { return sessionStorage.getItem(learningSelectionKey()); }
export function saveLearningSelection(id: string | null) {
  if (id) sessionStorage.setItem(learningSelectionKey(), id);
  else sessionStorage.removeItem(learningSelectionKey());
}
type DraftScope = { conversation: string; material: string };
// Plain questions follow the conversation across materials. Quoted questions
// stay with their original material, so a LID never silently changes meaning.
export function learningDraftKey(): DraftScope | null {
  const n = network.value, w = n.workspace;
  if (!n.identity || !w?.learning_workspace_id) return null;
  const conversation = `understand-book:learning:${n.identity.user_id}:draft:${JSON.stringify([w.learning_workspace_id, w.selected_chat])}`;
  return { conversation, material: `${conversation}:${JSON.stringify(w.published_book_ref)}` };
}
export function readLearningDraft(key: DraftScope | null): ChatDraft | null {
  const value = key && (sessionStorage.getItem(key.material) ?? sessionStorage.getItem(key.conversation));
  return value ? JSON.parse(value) : null;
}
export function saveLearningDraft(key: DraftScope | null, draft: ChatDraft | null) {
  if (!key) return;
  if (draft?.quote) {
    sessionStorage.setItem(key.material, JSON.stringify(draft)); sessionStorage.removeItem(key.conversation);
  } else {
    sessionStorage.removeItem(key.material);
    if (draft && (draft.message || draft.goalId)) sessionStorage.setItem(key.conversation, JSON.stringify(draft));
    else sessionStorage.removeItem(key.conversation);
  }
}
