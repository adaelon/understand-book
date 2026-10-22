import { shallowReactive, shallowRef } from 'vue';
import { ApiError } from './api';

export interface PublishedBookRef { book_id: string; publication_id: string }
export interface WorkspaceGoal { text: string; constraints: string[]; revision: number; created_at: number }
export interface LearningWorkspace {
  learning_workspace_id: string; goal: WorkspaceGoal; goal_history: WorkspaceGoal[];
  members: { published_book_ref: PublishedBookRef; readable: boolean; added_at: number }[];
  archived: boolean; created_at: number; updated_at: number;
}
export interface ChatDraft {
  message: string;
  quote: import('./api').AskQuote | null;
  goalId: string | null;
}
// Admission may outlive its App instance during connection or publication changes.
// Keep its restorable composer draft in the identity scope, never browser storage.
export const submittedRunDrafts = shallowReactive(new Map<string, ChatDraft>());
export interface NetworkIdentity {
  user_id: string;
  email?: string | null;
  csrf_token: string;
  capabilities?: { admin?: boolean; presentation: { authoring: boolean; reason: string } };
}
export interface NetworkWorkspace {
  workspace_id: string; generation: number; revision: number;
  learning_workspace_id?: string | null;
  learning_workspace_goal?: WorkspaceGoal | null;
  learning_workspace_positions?: Record<string, PublishedBookRef>;
  published_book_ref: PublishedBookRef; selected_chat: string | null;
  reader: import('./api').ReaderState;
  presentation?: import('./generated/PresentationFollowUp').PresentationFollowUp | null;
}
export const network = shallowRef<{
  identity: NetworkIdentity | null; workspace: NetworkWorkspace | null;
  attachment: string; epoch: number; enabled: boolean; linked: boolean;
  readerGeneration?: number;
  readerPublication?: string;
}>({ identity: null, workspace: null, attachment: '', epoch: 0, enabled: false, linked: false });

export function sceneKey() {
  const n = network.value, w = n.workspace;
  return `${n.epoch}:${n.identity?.user_id}:${w?.workspace_id}:${w?.learning_workspace_id ?? ''}:${w?.generation}:${w?.published_book_ref.book_id}:${w?.published_book_ref.publication_id}`;
}
// Chat commands advance request authority without replacing the mounted book.
export function readerKey() {
  const n = network.value, w = n.workspace;
  return `${n.epoch}:${n.identity?.user_id}:${w?.workspace_id}:${w?.learning_workspace_id ?? ''}:${n.readerGeneration ?? w?.generation}:${w?.published_book_ref.book_id}:${n.readerPublication ?? w?.published_book_ref.publication_id}`;
}
/** Same local owner as Server UserRuntime; network identity is unavailable until authenticated. */
export function readerPreferenceOwner(): string | null {
  return network.value.enabled ? network.value.identity?.user_id ?? null : 'local';
}
export function installIdentity(identity: NetworkIdentity | null) {
  submittedRunDrafts.clear();
  network.value = { enabled: true, identity, workspace: null, attachment: crypto.randomUUID(), epoch: network.value.epoch + 1, linked: false };
}
export function installWorkspace(workspace: NetworkWorkspace, attachment = network.value.attachment, linked = network.value.linked, transition?: 'chat' | 'enrichment') {
  if (network.value.linked && network.value.workspace && workspace.generation !== network.value.workspace.generation) {
    network.value = { ...network.value, workspace: null, epoch: network.value.epoch + 1 };
    return;
  }
  const previous = network.value.workspace;
  const sameBook = previous?.workspace_id === workspace.workspace_id
    && previous.learning_workspace_id === workspace.learning_workspace_id
    && previous.published_book_ref.book_id === workspace.published_book_ref.book_id
    && previous.published_book_ref.publication_id === workspace.published_book_ref.publication_id;
  // Ordinary book/open advances generation. Same-generation publication changes
  // also occur when recovering an enrichment response lost in transit.
  const enrichment = !previous?.learning_workspace_id && !workspace.learning_workspace_id
    && previous?.workspace_id === workspace.workspace_id
    && previous.published_book_ref.book_id === workspace.published_book_ref.book_id && previous.generation === workspace.generation;
  const keepReader = (sameBook || enrichment) && attachment === network.value.attachment
    && (previous.generation === workspace.generation || transition === 'chat');
  const readerGeneration = keepReader ? network.value.readerGeneration ?? previous.generation : workspace.generation;
  const readerPublication = keepReader ? network.value.readerPublication ?? previous!.published_book_ref.publication_id : workspace.published_book_ref.publication_id;
  network.value = { ...network.value, workspace, attachment, linked, readerGeneration, readerPublication };
  if (!linked && network.value.identity) {
    const saved = savedWorkspace();
    saveWorkspace(workspace.workspace_id, saved?.workspace_id === workspace.workspace_id ? saved.view ?? 'reader' : 'reader');
  }
}
export function workspaceStorageKey() { return `understand-book:workspace:${network.value.identity?.user_id}`; }
type SavedWorkspace = { workspace_id: string; view?: 'reader' | 'library' };
export function savedWorkspace(): SavedWorkspace | null {
  if (!network.value.identity) return null;
  const saved = sessionStorage.getItem(workspaceStorageKey()) ?? localStorage.getItem(workspaceStorageKey());
  return saved ? JSON.parse(saved) : null;
}
export function saveWorkspace(workspaceId: string, view: 'reader' | 'library') {
  if (!network.value.identity || network.value.linked) return;
  const saved = JSON.stringify({ workspace_id: workspaceId, view });
  // Each open tab keeps its own scene; a new tab can recover this device's last visit.
  sessionStorage.setItem(workspaceStorageKey(), saved);
  localStorage.setItem(workspaceStorageKey(), saved);
}
export function forgetNetwork() {
  const owner = network.value.identity?.user_id;
  if (owner) {
    for (const storage of [sessionStorage, localStorage]) {
      for (const key of Object.keys(storage)) {
        if (key.startsWith(`understand-book:pending:${owner}:`) || key.startsWith(`understand-book:learning:${owner}:`)) storage.removeItem(key);
      }
    }
  }
  installIdentity(null);
}

/** Retained callbacks stay bound to their component's scene or Reader lifetime. */
export function bindSceneApi<T extends object>(api: T, scopeKey = sceneKey): T {
  const key = scopeKey();
  return new Proxy(api, { get(target, property) {
    const value = Reflect.get(target, property);
    if (typeof value !== 'function') return value;
    return (...args: unknown[]) => {
      if (network.value.enabled && scopeKey() !== key) throw new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', '阅读现场已切换');
      return Reflect.apply(value, target, args);
    };
  } });
}
