import { t as uiText } from './i18n';
import { shallowReactive, shallowRef } from 'vue';
import { ApiError } from './api-error';

export interface PublishedBookRef { book_id: string; publication_id: string }
export interface WorkspaceGoal { text: string; constraints: string[]; revision: number; created_at: number }
export interface LearningWorkspace {
  learning_workspace_id: string; goal: WorkspaceGoal; goal_history: WorkspaceGoal[];
  members: { published_book_ref: PublishedBookRef; readable: boolean; added_at: number }[];
  archived: boolean; created_at: number; updated_at: number;
  shared_release?: { release_id: string; version: number };
}
export interface ChatDraft {
  message: string;
  quote: import('./api').AskQuote | null;
  goalId: string | null;
  presentationRequested?: boolean;
}
// Admission may outlive its App instance during connection or publication changes.
// Keep its restorable composer draft in the identity scope, never browser storage.
export const submittedRunDrafts = shallowReactive(new Map<string, ChatDraft>());
export interface NetworkIdentity {
  user_id: string;
  is_guest?: boolean;
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
/** Retained callbacks stay bound to their component's scene or Reader lifetime. */
export function bindSceneApi<T extends object>(api: T, scopeKey = sceneKey): T {
  const key = scopeKey();
  return new Proxy(api, { get(target, property) {
    const value = Reflect.get(target, property);
    if (typeof value !== 'function') return value;
    return (...args: unknown[]) => {
      if (network.value.enabled && scopeKey() !== key) throw new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', uiText('阅读现场已切换'));
      return Reflect.apply(value, target, args);
    };
  } });
}
