import { shallowRef } from 'vue';

export interface PublishedBookRef { book_id: string; publication_id: string }
export interface ChatDraft {
  message: string;
  quote: import('./api').AskQuote | null;
  goalId: string | null;
}
export interface NetworkIdentity {
  user_id: string;
  csrf_token: string;
  capabilities?: { presentation: { authoring: boolean; reason: string } };
}
export interface NetworkWorkspace {
  workspace_id: string; generation: number; revision: number;
  published_book_ref: PublishedBookRef; selected_chat: string | null;
  reader: import('./api').ReaderState;
  presentation?: import('./generated/PresentationFollowUp').PresentationFollowUp | null;
}
export const network = shallowRef<{
  identity: NetworkIdentity | null; workspace: NetworkWorkspace | null;
  attachment: string; epoch: number; enabled: boolean; linked: boolean;
}>({ identity: null, workspace: null, attachment: '', epoch: 0, enabled: false, linked: false });

export function sceneKey() {
  const n = network.value, w = n.workspace;
  return `${n.epoch}:${n.identity?.user_id}:${w?.workspace_id}:${w?.generation}:${w?.published_book_ref.book_id}:${w?.published_book_ref.publication_id}`;
}
/** Same local owner as Server UserRuntime; network identity is unavailable until authenticated. */
export function readerPreferenceOwner(): string | null {
  return network.value.enabled ? network.value.identity?.user_id ?? null : 'local';
}
export function installIdentity(identity: NetworkIdentity | null) {
  network.value = { enabled: true, identity, workspace: null, attachment: crypto.randomUUID(), epoch: network.value.epoch + 1, linked: false };
}
export function installWorkspace(workspace: NetworkWorkspace, attachment = network.value.attachment, linked = network.value.linked) {
  if (network.value.linked && network.value.workspace && workspace.generation !== network.value.workspace.generation) {
    network.value = { ...network.value, workspace: null, epoch: network.value.epoch + 1 };
    return;
  }
  network.value = { ...network.value, workspace, attachment, linked };
  if (!linked && network.value.identity) sessionStorage.setItem(workspaceStorageKey(), JSON.stringify({ workspace_id: workspace.workspace_id }));
}
export function workspaceStorageKey() { return `understand-book:workspace:${network.value.identity?.user_id}`; }
export function forgetNetwork() {
  const owner = network.value.identity?.user_id;
  if (owner) {
    for (const storage of [sessionStorage, localStorage]) {
      for (const key of Object.keys(storage)) {
        if (key.startsWith(`understand-book:pending:${owner}:`) || key === workspaceStorageKey()) storage.removeItem(key);
      }
    }
  }
  installIdentity(null);
}

/** Components may retain promises after unmount; their next request must retain the old scene. */
export function bindSceneApi<T extends object>(api: T): T {
  const key = sceneKey();
  return new Proxy(api, { get(target, property) {
    const value = Reflect.get(target, property);
    if (typeof value !== 'function') return value;
    return (...args: unknown[]) => {
      if (network.value.enabled && sceneKey() !== key) throw new Error('阅读现场已切换');
      return Reflect.apply(value, target, args);
    };
  } });
}
