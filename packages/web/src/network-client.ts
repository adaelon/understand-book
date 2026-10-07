import { ApiError, type AgentHistoryResponse } from './api';
import { initialRun, type RunDescriptor, type RunSnapshot } from './agent-run-state';
import { network, sceneKey, installWorkspace, forgetNetwork, workspaceStorageKey, type NetworkWorkspace } from './network-context';

export async function networkFetch<T>(method: string, path: string, body?: unknown): Promise<T> {
  const key = sceneKey();
  const identity = network.value.identity;
  const res = await fetch(`/api${path}`, {
    method, credentials: 'same-origin', cache: 'no-store',
    headers: { 'Content-Type': 'application/json', ...(identity ? { 'X-CSRF-Token': identity.csrf_token } : {}) },
    ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
  });
  const value = await res.json();
  if (path !== '/auth/logout' && key !== sceneKey()) throw new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', '阅读现场已切换');
  if (!res.ok) {
    if (res.status === 401 && identity && key === sceneKey()) forgetNetwork();
    const message = value.error_code === 'WORKSPACE_STALE' ? '阅读连接已失效，请重试恢复连接' : value.message ?? '请求未完成';
    throw new ApiError(res.status, value.error_code ?? `HTTP_${res.status}`, value.category ?? (res.status === 409 ? 'conflict' : 'unavailable'), message);
  }
  return value;
}

// Serialize CAS writes from this page. Each operation captures its scene before joining the queue.
let writes: Promise<unknown> = Promise.resolve();
let bindingRecovery: { key: string; task: Promise<void> } | null = null;
/** Reclaim a cold main scene before exposing its new generation to the Reader. */
export function recoverWorkspaceBinding(): Promise<void> {
  const key = sceneKey();
  if (bindingRecovery?.key === key) return bindingRecovery.task;
  if (network.value.linked) return Promise.reject(new ApiError(409, 'WORKSPACE_STALE', 'conflict', '请从原阅读窗口重新打开附属窗口'));
  const result = writes.catch(() => {}).then(async () => {
    if (key !== sceneKey()) throw new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', '阅读现场已切换');
    const n = network.value;
    const saved = sessionStorage.getItem(workspaceStorageKey());
    const id = n.workspace?.workspace_id ?? (saved ? JSON.parse(saved).workspace_id : null);
    if (!id) return;
    const latest = await networkFetch<NetworkWorkspace>('GET', `/workspaces/${id}`);
    const attached = await networkFetch<NetworkWorkspace>('POST', `/workspaces/${id}/attach`, {
      attachment_id: n.attachment, generation: latest.generation, expected_revision: latest.revision,
    });
    installWorkspace(attached);
  });
  const recovery = { key, task: result.finally(() => { if (bindingRecovery === recovery) bindingRecovery = null; }) };
  bindingRecovery = recovery;
  writes = recovery.task;
  return recovery.task;
}
export function workspaceAction<T = unknown>(action: string, body: Record<string, unknown> = {}): Promise<T> {
  const key = sceneKey();
  const result = writes.catch(() => {}).then(async () => {
    if (key !== sceneKey()) throw new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', '阅读现场已切换');
    const n = network.value, w = n.workspace;
    if (!w) throw new ApiError(409, 'WORKSPACE_REQUIRED', 'conflict', '请先选择材料');
    // A Run or linked view can advance revision independently; read it before a new user command.
    const latest = await networkFetch<NetworkWorkspace>('GET', `/workspaces/${w.workspace_id}`);
    if (latest.generation !== w.generation) {
      installWorkspace(latest);
      throw new ApiError(409, 'WORKSPACE_STALE', 'conflict', '现场已更新，请重新执行当前操作');
    }
    installWorkspace(latest);
    const value = await networkFetch<NetworkWorkspace & { result?: T }>('POST', `/workspaces/${w.workspace_id}/${action}`, {
      ...body, attachment_id: n.attachment, generation: latest.generation, expected_revision: latest.revision,
    });
    if (value.workspace_id) installWorkspace(value);
    return value as T;
  });
  writes = result;
  return result;
}
export function publishedUrl(leaf: string) {
  const ref = network.value.workspace?.published_book_ref;
  if (!ref) throw new Error('请先选择材料');
  return `/books/${encodeURIComponent(ref.book_id)}/publications/${encodeURIComponent(ref.publication_id)}/${leaf}`;
}

// Reads retain scene authority but do not join the CAS write queue or install an older snapshot.
export async function workspaceRead<T = unknown>(action: string, body: Record<string, unknown> = {}): Promise<T> {
  const n = network.value, w = n.workspace;
  if (!w) throw new ApiError(409, 'WORKSPACE_REQUIRED', 'conflict', '请先选择材料');
  return networkFetch<T>('POST', `/workspaces/${w.workspace_id}/${action}`, {
    ...body, attachment_id: n.attachment, generation: w.generation, expected_revision: w.revision,
  });
}
const readActions = new Set(['reader/state', 'profile/manifest', 'profile/memory', 'memory/recall',
  'reader/paper_minimap.state', 'reader/pdf_selection.resolve', 'reader/pdf_ranges.project', 'agent/source.resolve']);

interface Admission extends RunDescriptor {
  dispatch_state: string; persistence_state: string; snapshot?: RunSnapshot;
  turn?: import('./api').AgentChatTurn;
}
export function admissionSnapshot(value: Admission): RunSnapshot {
  if (value.snapshot) return { ...value.snapshot, descriptor: { ...value.snapshot.descriptor, workspace_id: value.workspace_id, workspace_generation: value.workspace_generation, published_book_ref: value.published_book_ref } };
  const snapshot = initialRun(value);
  if (value.persistence_state === 'failed') return { ...snapshot, persistence_state: 'failed', execution_state: 'ended', error: value.turn?.error as RunSnapshot['error'] ?? null };
  if (value.dispatch_state === 'queued' || value.dispatch_state === 'preparing' || value.dispatch_state === 'claimed') return { ...snapshot, execution_state: value.dispatch_state === 'claimed' ? 'running' : 'queued' };
  return { ...snapshot, persistence_state: 'saved', execution_state: value.turn?.status === 'completed' ? 'completed' : value.turn?.status === 'cancelled' ? 'cancelled' : 'failed', final_view: value.turn ?? null, error: value.turn?.error as RunSnapshot['error'] ?? null };
}
export interface PendingSubmission { key: string; body: Record<string, unknown>; workspace: string }
function pendingPrefix() { return `understand-book:pending:${network.value.identity?.user_id}:`; }
function pendingChanged() { window.dispatchEvent(new Event('submission-pending-changed')); }
export function pendingSubmissions(): PendingSubmission[] {
  return Object.keys(sessionStorage).filter(key => key.startsWith(pendingPrefix())).map(key => JSON.parse(sessionStorage.getItem(key)!));
}
export async function recoverSubmissions(): Promise<RunDescriptor[]> {
  const recovered: RunDescriptor[] = [];
  for (const item of pendingSubmissions()) {
    try {
      const value = await networkFetch<Admission>('GET', `/agent/runs?client_request_id=${encodeURIComponent(item.key)}`);
      if (value.dispatch_state === 'preparing') continue;
      sessionStorage.removeItem(pendingPrefix() + item.key);
      recovered.push(value);
    } catch (error) {
      if (error instanceof ApiError && error.errorCode === 'REQUEST_KEY_CLOSED') sessionStorage.removeItem(pendingPrefix() + item.key);
      else if (!(error instanceof ApiError && error.status === 404)) throw error;
    }
  }
  pendingChanged();
  return recovered;
}
export async function retrySubmission(key: string) {
  await recoverSubmissions();
  const item = pendingSubmissions().find(p => p.key === key);
  if (!item) { window.dispatchEvent(new Event('agent-admission-changed')); return; }
  try { await networkFetch('POST', `/workspaces/${item.workspace}/agent/runs`, item.body); }
  catch (error) {
    if (error instanceof ApiError && error.status < 500 && !['ADMISSION_PREPARING', 'CLIENT_CONTEXT_STALE'].includes(error.errorCode)) {
      sessionStorage.removeItem(pendingPrefix() + item.key); pendingChanged();
    }
    throw error;
  }
  sessionStorage.removeItem(pendingPrefix() + item.key); pendingChanged();
  window.dispatchEvent(new Event('agent-admission-changed'));
}
async function createRun(body: Record<string, unknown>) {
  if (!network.value.workspace?.selected_chat) await workspaceAction('chat/new');
  const w = network.value.workspace!;
  if (pendingSubmissions().some(p => p.body.session_id === w.selected_chat)) {
    await recoverSubmissions();
    if (pendingSubmissions().some(p => p.body.session_id === w.selected_chat)) throw new ApiError(409, 'SUBMISSION_UNCONFIRMED', 'conflict', '原问题仍待核对，请先核对或重试原提交；不会自动重提');
  }
  const latest = await networkFetch<NetworkWorkspace>('GET', `/workspaces/${w.workspace_id}`);
  if (latest.generation !== w.generation) { installWorkspace(latest); throw new ApiError(409, 'WORKSPACE_STALE', 'conflict', '现场已更新'); }
  installWorkspace(latest);
  const key = crypto.randomUUID();
  const input = { ...body, client_request_id: key, session_id: w.selected_chat, attachment_id: network.value.attachment,
    generation: latest.generation, expected_revision: latest.revision, published_book_ref: latest.published_book_ref };
  sessionStorage.setItem(pendingPrefix() + key, JSON.stringify({ key, body: input, workspace: w.workspace_id }));
  pendingChanged();
  const storageKey = pendingPrefix() + key;
  try {
    const value = await networkFetch<Admission>('POST', `/workspaces/${w.workspace_id}/agent/runs`, input);
    sessionStorage.removeItem(storageKey); pendingChanged(); window.dispatchEvent(new Event('agent-admission-changed')); 
    return value;
  } catch (error) {
    if (error instanceof ApiError && error.status < 500 && !['ADMISSION_PREPARING', 'CLIENT_CONTEXT_STALE'].includes(error.errorCode)) sessionStorage.removeItem(storageKey);
    if (!(error instanceof ApiError) || error.status >= 500 || error.errorCode === 'ADMISSION_PREPARING') {
      try {
        const accepted = await networkFetch<Admission>('GET', `/agent/runs?client_request_id=${encodeURIComponent(key)}`);
        if (accepted.dispatch_state !== 'preparing') { sessionStorage.removeItem(storageKey); pendingChanged(); window.dispatchEvent(new Event('agent-admission-changed'));  return accepted; }
      } catch { /* Keep the original key; recovery only queries it. */ }
    }
    pendingChanged();
    throw error;
  }
}
async function history(): Promise<AgentHistoryResponse> {
  return (await workspaceRead<{ result: AgentHistoryResponse }>('chat/history')).result;
}

export async function networkRequest<T>(method: 'GET' | 'POST', path: string, body?: unknown): Promise<T> {
  const input = (body ?? {}) as Record<string, unknown>;
  if (path === '/profile/memory/apply' && !network.value.workspace?.selected_chat) await workspaceAction('chat/new');
  let value: unknown;
  if (path === '/agent/runs') value = await createRun(input);
  else if (/^\/agent\/runs\//.test(path)) value = admissionSnapshot(await networkFetch<Admission>(method, path, body));
  else if (path === '/agent/history') value = await history();
  else if (path.startsWith('/agent/history/recap?')) value = await networkFetch(method, path);
  else if (path === '/agent/new' || path === '/agent/history/select') {
    await workspaceAction(path === '/agent/new' ? 'chat/new' : 'chat/select', input);
    const result = await history(); value = path === '/agent/new' ? { ok: true, history: result } : result;
  } else if (path === '/agent/history/delete') {
    await networkFetch('POST', path, body);
    const w = network.value.workspace!;
    installWorkspace(await networkFetch<NetworkWorkspace>('GET', `/workspaces/${w.workspace_id}`));
    value = await history();
  } else if (path === '/book/open') {
    await workspaceAction('book/open', { published_book_ref: JSON.parse(String(input.dir)) });
    value = { ok: true, book_id: network.value.workspace!.published_book_ref.book_id };
  } else if (path === '/book/library') {
    const result = await networkFetch<{ books: { published_book_ref: import('./network-context').PublishedBookRef; cover?: import('./api').BookCoverSource | null }[] }>('GET', '/library');
    value = { root: '', books: result.books.map(b => ({ name: b.published_book_ref.book_id, book_id: b.published_book_ref.book_id, dir: JSON.stringify(b.published_book_ref), route: 'reader', cover: b.cover })) };
  } else if (path.startsWith('/book/')) value = await networkFetch(method, publishedUrl(path.slice(6)), body);
  else if (path === '/tutor/state' || path === '/tutor/mutate') value = await networkFetch(method, path, body);
  else if (path === '/tutor/readiness') value = await networkFetch('GET', publishedUrl('teaching_readiness'));
  else if (path.startsWith('/agent/presentation.read') || path.startsWith('/agent/presentation.observe')) {
    if (input.saved_state) await workspaceAction('presentation/restore', { saved_state: input.saved_state });
    const result = await workspaceAction<{ result: unknown }>(path.endsWith("observe") ? "presentation/observe" : "presentation/read", input);
    value = result.result;
  } else {
    const action = path === '/agent/presentation.state.save' ? 'presentation/save' : path.split('?')[0].slice(1);
    const request = readActions.has(action) ? workspaceRead : workspaceAction;
    const data = await request<NetworkWorkspace & { result?: unknown }>(action, input);
    value = path === '/reader/state' ? data.reader : data.result ?? data;
  }
  return value as T;
}
