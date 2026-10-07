import { network, sceneKey, type NetworkWorkspace } from './network-context';
import { workspaceAction } from './network-client';
import type { PresentationRef } from './generated/PresentationRef';
const children = new Set<Window>();
export function isLinkedReaderChange(event: MessageEvent) {
  return event.origin === location.origin && children.has(event.source as Window) && event.data?.kind === 'reader-linked-reader-changed';
}
export function invalidateLinkedWindows() {
  for (const child of children) if (!child.closed) child.postMessage({ kind: 'reader-linked-expired' }, location.origin);
  children.clear();
}

/** Only the window explicitly opened by this host receives an attached view handle. */
export async function openLinkedPresentation(session_id: string, turn_id: string, reference: PresentationRef) {
  const key = sceneKey();
  const child = window.open(`${location.pathname}?linked=1`, '_blank');
  if (!child) throw new Error('请允许弹出窗口以打开附属演示');
  for (const previous of children) if (previous.closed) children.delete(previous);
  children.add(child);
  let ready = false;
  let receipt: (NetworkWorkspace & { attachment_id: string }) | undefined;
  const send = () => {
    if (!ready || !receipt || key !== sceneKey()) return;
    child.postMessage({ kind: 'reader-linked-context', workspace: receipt, attachment: receipt.attachment_id,
      user: network.value.identity?.user_id, session_id, turn_id,
      reference: { presentation_id: reference.presentation_id, revision: reference.revision } }, location.origin);
    window.removeEventListener('message', receive); clearTimeout(timer);
  };
  const receive = (event: MessageEvent) => {
    if (event.source === child && event.origin === location.origin && event.data?.kind === 'reader-linked-ready') { ready = true; send(); }
  };
  window.addEventListener('message', receive);
  const timer = window.setTimeout(() => window.removeEventListener('message', receive), 30000);
  try { receipt = await workspaceAction('linked/attach', { session_id, turn_id, reference }); send(); }
  catch (failure) { window.removeEventListener('message', receive); clearTimeout(timer); child.close(); throw failure; }
}
