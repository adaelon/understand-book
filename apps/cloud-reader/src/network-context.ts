import { discardSubmissionBody } from '@understand-book/web/reader/submission-body-store';
import { network, submittedRunDrafts, type NetworkIdentity, type NetworkWorkspace } from '@understand-book/web/reader/reader-context';
export * from '@understand-book/web/reader/reader-context';

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
        if (key.startsWith(`understand-book:pending:${owner}:`)) {
          const item = JSON.parse(storage.getItem(key)!);
          if (item.bodyStored) discardSubmissionBody(key);
          storage.removeItem(key);
        } else if (key.startsWith(`understand-book:learning:${owner}:`)) storage.removeItem(key);
      }
    }
  }
  installIdentity(null);
}

