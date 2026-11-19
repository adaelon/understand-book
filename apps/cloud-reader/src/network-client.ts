import { mergeBoardDeliveries } from '@understand-book/web/reader/board-workspace';
import { t as uiText } from '@understand-book/web/reader/i18n';
import { ApiError } from '@understand-book/web/reader/api-error';
import type { AgentHistoryResponse } from '@understand-book/web/reader/api';
import { installReaderHost } from '@understand-book/web/reader/reader-host';
import { spendNotice, spendFailureMessage } from './account-allowance';
import { initialRun, type RunDescriptor, type RunSnapshot } from '@understand-book/web/reader/agent-run-state';
import { discardSubmissionBody, readSubmissionBody, saveSubmissionBody } from '@understand-book/web/reader/submission-body-store';
import { network, sceneKey, installWorkspace, forgetNetwork, savedWorkspace, saveWorkspace, type NetworkWorkspace, type LearningWorkspace, type PublishedBookRef } from './network-context';

export const learningWorkspaces = {
  list: () => networkFetch<{ workspaces: LearningWorkspace[] }>('GET', '/learning-workspaces'),
  create: (text: string) => networkFetch<LearningWorkspace>('POST', '/learning-workspaces', { goal: { text, constraints: [] } }),
  goal: (id: string, text: string, constraints: string[]) => networkFetch<LearningWorkspace>('POST', `/learning-workspaces/${encodeURIComponent(id)}/goal`, { goal: { text, constraints } }),
  member: (id: string, reference: PublishedBookRef, add: boolean) => networkFetch<LearningWorkspace>(add ? 'POST' : 'DELETE', `/learning-workspaces/${encodeURIComponent(id)}/members`, { published_book_ref: reference }),
  archive: (id: string, archived: boolean) => networkFetch<LearningWorkspace>('POST', `/learning-workspaces/${encodeURIComponent(id)}/archive`, { archived }),
};

export const sharedWorkspaces = {
  list: (offset = 0, signal?: AbortSignal, bookId?: string) => networkFetch<import('@understand-book/web/reader/shared-workspaces').SharedReleasePage>('GET', `/workspace-releases?limit=12&offset=${offset}${bookId ? `&book_id=${encodeURIComponent(bookId)}` : ''}`, undefined, signal),
  read: (id: string, version: number, signal?: AbortSignal) => networkFetch<import('@understand-book/web/reader/shared-workspaces').SharedRelease>('GET', `/workspace-releases/${encodeURIComponent(id)}/versions/${version}`, undefined, signal),
  use: (id: string, version: number, operationId: string, text: string) => networkFetch<import('@understand-book/web/reader/shared-workspaces').SharedUseResult>('POST', `/workspace-releases/${encodeURIComponent(id)}/versions/${version}/use`, { operation_id: operationId, goal: { text, constraints: [] } }),
  instance: (id: string) => networkFetch<{ instance: import('@understand-book/web/reader/shared-workspaces').SharedInstance | null }>('GET', `/learning-workspaces/${encodeURIComponent(id)}/shared-release`),
  preview: (id: string, version: number) => networkFetch<import('@understand-book/web/reader/shared-workspaces').SharedUpdatePreview>('GET', `/learning-workspaces/${encodeURIComponent(id)}/shared-release/preview?version=${version}`),
  update: (id: string, operationId: string, expectedVersion: number, version: number) => networkFetch<import('@understand-book/web/reader/shared-workspaces').SharedUseResult>('POST', `/learning-workspaces/${encodeURIComponent(id)}/shared-release/update`, { operation_id: operationId, expected_version: expectedVersion, version }),
};

export const learningRecords = {
  list: () => networkFetch<{ records: import('@understand-book/web/reader/learning-records').LearningRecordSummary[] }>('GET', '/me/learning-records'),
  read: (sessionId: string, throughSeq: number) => networkFetch<import('@understand-book/web/reader/learning-records').LearningRecordDetail>('GET', `/me/learning-records/${encodeURIComponent(sessionId)}?through_seq=${throughSeq}`),
  linked: (workspaceId: string) => networkFetch<{ records: import('@understand-book/web/reader/learning-records').ImportedLearningRecord[] }>('GET', `/learning-workspaces/${encodeURIComponent(workspaceId)}/records`),
  link: (workspaceId: string, sessionId: string, throughSeq: number, goalRevision: number) => networkFetch<import('@understand-book/web/reader/learning-records').LinkedLearningRecord>('POST', `/learning-workspaces/${encodeURIComponent(workspaceId)}/records`, { session_id: sessionId, through_seq: throughSeq, goal_revision: goalRevision }),
};

export function openLearningMaterial(reference: PublishedBookRef, learningWorkspaceId: string | null) {
  return workspaceAction('book/open', { published_book_ref: reference, learning_workspace_id: learningWorkspaceId });
}

export const accountMcp = {
  read: () => networkFetch<import('@understand-book/web/reader/api').AccountMcpState>('GET', '/account/mcp'),
  create: (label: string) => networkFetch<{ credential: import('@understand-book/web/reader/api').McpCredential; token: string }>('POST', '/account/mcp/credentials', { label }),
  revoke: (id: string) => networkFetch('POST', `/account/mcp/credentials/${encodeURIComponent(id)}/revoke`, {}),
};

export const accountAllowance = {
  read: () => networkFetch<import('./account-allowance').AccountAllowanceSummary>('GET', '/account/allowance'),
  usage: (limit: number, offset: number) => networkFetch<import('./account-allowance').AccountUsagePage>('GET', `/account/usage?limit=${limit}&offset=${offset}`),
  costs: (offset = 0) => networkFetch<import('./account-costs').AccountCostReport>('GET', `/account/cost-summary?limit=20&offset=${offset}`),
  costUsage: (query: string) => networkFetch<import('./account-allowance').AccountUsagePage>('GET', `/account/usage?${query}`),
};

export async function networkFetch<T>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const key = sceneKey();
  const identity = network.value.identity;
  const res = await fetch(`/api${path}`, {
    method, signal, credentials: 'same-origin', cache: 'no-store',
    headers: { 'Content-Type': 'application/json', ...(identity ? { 'X-CSRF-Token': identity.csrf_token } : {}) },
    ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
  });
  const text = await res.text();
  if (path !== '/auth/logout' && key !== sceneKey()) throw new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', uiText('阅读现场已切换'));
  let value;
  try { value = JSON.parse(text); }
  catch {
    // A proxy can reject an upload before the application sees it. Keep its
    // HTTP status so a definite rejection releases the pending request key.
    if (res.ok) throw new ApiError(502, 'INVALID_RESPONSE', 'unavailable', uiText('服务返回了无法读取的响应，请稍后重试。'));
  }
  if (!res.ok) {
    if (res.status === 401 && identity && key === sceneKey()) forgetNetwork();
    const message = res.status === 413 ? uiText('图片或附件过大，请缩小后重新发送。')
      : value?.error_code === 'WORKSPACE_STALE' ? uiText('阅读连接已失效，请重试恢复连接')
      : value?.message ?? (res.status >= 500 ? uiText('服务暂时不可用，请稍后重试。') : uiText('请求未完成'));
    throw new ApiError(res.status, value?.error_code ?? `HTTP_${res.status}`, value?.category ?? (res.status === 409 ? 'conflict' : 'unavailable'), message,
      typeof value?.request_id === 'string' ? { request_id: value.request_id, expires_at: value.expires_at, resend_after: value.resend_after } : undefined);
  }
  return value;
}

/** Retry read-side recovery through the shell, which owns authentication and attachment. */
export function requestWorkspaceRecovery(failure: unknown) {
  if (!network.value.enabled || !network.value.identity || !network.value.workspace || network.value.linked) return;
  const retryable = failure instanceof ApiError
    ? failure.status >= 500 || [408, 429].includes(failure.status)
      || ['WORKSPACE_STALE', 'WORKSPACE_REVISION_CONFLICT'].includes(failure.errorCode)
    : failure instanceof TypeError || failure instanceof SyntaxError
      || (failure instanceof DOMException && failure.name === 'TimeoutError');
  if (retryable) window.dispatchEvent(new Event('workspace-recovery-needed'));
}

// Serialize CAS writes from this page. Each operation captures its scene before joining the queue.
let writes: Promise<unknown> = Promise.resolve();
let queuedNavigation: { key: string; superseded: boolean } | null = null;
let bindingRecovery: { key: string; task: Promise<void>; signal?: AbortSignal } | null = null;

// The caller may stop waiting while an earlier write still owns the queue. Keep
// that underlying task in `writes`, so cancellation never overlaps CAS writes.
export function abortable<T>(task: Promise<T>, signal?: AbortSignal): Promise<T> {
  if (!signal) return task;
  return new Promise<T>((resolve, reject) => {
    const aborted = () => reject(signal.reason ?? new DOMException('Aborted', 'AbortError'));
    signal.addEventListener('abort', aborted, { once: true });
    task.then(resolve, reject).finally(() => signal.removeEventListener('abort', aborted));
    if (signal.aborted) aborted();
  });
}
function assertOpeningActive(key: string, signal?: AbortSignal) {
  signal?.throwIfAborted();
  if (key !== sceneKey()) throw new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', uiText('阅读现场已切换'));
}
// Acknowledged attach may fork into another workspace. Retain its pointer before
// opening the target, without mounting that intermediate book in the Reader.
let openingRecovery: { key: string; workspaceId: string | null } | null = null;

/** Open from the library with one warm write or one cold read/attach/open chain. */
export function openLibraryMaterial(reference: PublishedBookRef, learningWorkspaceId: string | null, signal?: AbortSignal,
  learningMembers?: PublishedBookRef[]): Promise<NetworkWorkspace> {
  const key = sceneKey();
  queuedNavigation = null;
  const result = writes.catch(() => {}).then(async () => {
    assertOpeningActive(key, signal);
    const n = network.value;
    if (n.linked) throw new ApiError(409, 'WORKSPACE_STALE', 'conflict', uiText('请从原阅读窗口重新打开附属窗口'));
    const recovery = openingRecovery?.key === key ? openingRecovery : null;
    let target = reference;
    const chooseTarget = (workspace: NetworkWorkspace) => {
      const previous = learningWorkspaceId && learningMembers && workspace.learning_workspace_positions?.[learningWorkspaceId];
      if (previous && learningMembers!.some(member => member.book_id === previous.book_id && member.publication_id === previous.publication_id)) target = previous;
    };
    const matches = (workspace: NetworkWorkspace) => workspace.published_book_ref.book_id === target.book_id
      && workspace.published_book_ref.publication_id === target.publication_id
      && (workspace.learning_workspace_id ?? null) === learningWorkspaceId;
    const receipt = (workspace: NetworkWorkspace) => {
      // A late acknowledgement is useful for the next explicit retry, but may
      // never restore an old account or install after this attempt timed out.
      if (key !== sceneKey()) throw new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', uiText('阅读现场已切换'));
      openingRecovery = { key, workspaceId: workspace.workspace_id };
      saveWorkspace(workspace.workspace_id, 'library');
      assertOpeningActive(key, signal);
      return workspace;
    };
    const read = async (id: string) => {
      const workspace = await networkFetch<NetworkWorkspace>('GET', `/workspaces/${id}`, undefined, signal);
      assertOpeningActive(key, signal);
      return workspace;
    };
    const attach = async (workspace: NetworkWorkspace) => {
      assertOpeningActive(key, signal);
      openingRecovery = { key, workspaceId: workspace.workspace_id };
      return receipt(await networkFetch<NetworkWorkspace>('POST', `/workspaces/${workspace.workspace_id}/attach`, {
        attachment_id: n.attachment, generation: workspace.generation, expected_revision: workspace.revision,
      }, signal));
    };
    const create = async () => {
      assertOpeningActive(key, signal);
      openingRecovery = { key, workspaceId: null };
      return receipt(await networkFetch<NetworkWorkspace>('POST', '/workspaces', {
        attachment_id: n.attachment, published_book_ref: target, learning_workspace_id: learningWorkspaceId,
      }, signal));
    };
    const post = async (workspace: NetworkWorkspace) => {
      assertOpeningActive(key, signal);
      openingRecovery = { key, workspaceId: workspace.workspace_id };
      return receipt(await networkFetch<NetworkWorkspace>('POST', `/workspaces/${workspace.workspace_id}/book/open`, {
        attachment_id: n.attachment, generation: workspace.generation, expected_revision: workspace.revision,
        published_book_ref: target, learning_workspace_id: learningWorkspaceId,
      }, signal));
    };
    let workspace = n.workspace;
    if (workspace) chooseTarget(workspace);
    // A same-material click means resume, including its chat and position.
    // Read/attach also recovers a binding evicted while the library was open.
    if (!workspace || recovery || matches(workspace)) {
      const id = recovery ? recovery.workspaceId : workspace?.workspace_id ?? savedWorkspace()?.workspace_id;
      let latest: NetworkWorkspace | null = null;
      if (id) {
        try { latest = await read(id); }
        catch (error) { if (!(error instanceof ApiError) || error.status !== 404) throw error; }
      }
      workspace = latest ? await attach(latest) : await create();
      chooseTarget(workspace);
    }
    if (!matches(workspace)) {
      try { workspace = await post(workspace); }
      catch (error) {
        assertOpeningActive(key, signal);
        if (!(error instanceof ApiError) || !['WORKSPACE_REVISION_CONFLICT', 'WORKSPACE_STALE'].includes(error.errorCode)) throw error;
        // These responses reject the attempted write before mutation. An
        // uncertain/lost response waits for an explicit user retry instead.
        const latest = await read(workspace.workspace_id);
        workspace = error.errorCode === 'WORKSPACE_STALE' || latest.generation !== workspace.generation
          ? await attach(latest) : latest;
        if (!matches(workspace)) workspace = await post(workspace);
      }
    }
    assertOpeningActive(key, signal);
    installWorkspace(workspace, n.attachment, false);
    openingRecovery = null;
    return workspace;
  });
  writes = result;
  return abortable(result, signal);
}
export interface ReadingResumption {
  workspace_id: string;
  learning_workspace_id?: string | null;
  published_book_ref: import('./network-context').PublishedBookRef;
  selected_chat: string | null;
  position_label: string | null;
  position_excerpt: string | null;
  last_question: string | null;
}
function savedWorkspaceId(): string | null {
  return network.value.workspace?.workspace_id ?? savedWorkspace()?.workspace_id ?? null;
}
export async function readReadingResumption(): Promise<ReadingResumption | null> {
  const id = savedWorkspaceId();
  return networkFetch<ReadingResumption | null>('GET', id ? `/workspaces/${id}/resumption` : '/me/reading-resumption');
}
/** Reclaim a cold main scene before exposing its new generation to the Reader. */
export function recoverWorkspaceBinding(expected?: ReadingResumption, signal?: AbortSignal): Promise<void> {
  const key = sceneKey();
  if (bindingRecovery?.key === key && !bindingRecovery.signal?.aborted) return abortable(bindingRecovery.task, signal);
  if (network.value.linked) return Promise.reject(new ApiError(409, 'WORKSPACE_STALE', 'conflict', uiText('请从原阅读窗口重新打开附属窗口')));
  const result = writes.catch(() => {}).then(async () => {
    assertOpeningActive(key, signal);
    const n = network.value;
    const id = expected?.workspace_id ?? savedWorkspaceId();
    if (!id) return;
    const latest = await networkFetch<NetworkWorkspace>('GET', `/workspaces/${id}`, undefined, signal);
    assertOpeningActive(key, signal);
    if (expected && (latest.workspace_id !== expected.workspace_id
      || latest.published_book_ref.book_id !== expected.published_book_ref.book_id
      || latest.published_book_ref.publication_id !== expected.published_book_ref.publication_id
      || (latest.learning_workspace_id ?? null) !== (expected.learning_workspace_id ?? null)
      || latest.selected_chat !== expected.selected_chat)) {
      throw new ApiError(409, 'WORKSPACE_STALE', 'conflict', uiText('阅读记录已更新，请查看最新记录后继续。'));
    }
    const attached = await networkFetch<NetworkWorkspace>('POST', `/workspaces/${id}/attach`, {
      attachment_id: n.attachment, generation: latest.generation, expected_revision: latest.revision,
    }, signal);
    if (key !== sceneKey()) throw new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', uiText('阅读现场已切换'));
    openingRecovery = { key, workspaceId: attached.workspace_id };
    saveWorkspace(attached.workspace_id, savedWorkspace()?.view ?? 'library');
    assertOpeningActive(key, signal);
    installWorkspace(attached);
    openingRecovery = null;
  });
  const recovery = { key, signal, task: result.finally(() => { if (bindingRecovery === recovery) bindingRecovery = null; }) };
  bindingRecovery = recovery;
  writes = recovery.task;
  return abortable(recovery.task, signal);
}
export function workspaceAction<T = unknown>(action: string, body: Record<string, unknown> = {}): Promise<T> {
  const key = sceneKey();
  const optimistic = action === 'reader/goto' || action === 'reader/position' || action === 'reader/video_draft';
  const navigation = action === 'reader/goto' ? { key, superseded: false } : null;
  if (navigation && queuedNavigation?.key === key) queuedNavigation.superseded = true;
  // A different command is an ordering barrier: never drop navigation across it.
  queuedNavigation = navigation;
  const result = writes.catch(() => {}).then(async () => {
    if (navigation?.superseded) throw new DOMException('Navigation superseded', 'AbortError');
    if (queuedNavigation === navigation) queuedNavigation = null;
    if (key !== sceneKey()) throw new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', uiText('阅读现场已切换'));
    const n = network.value, w = n.workspace;
    if (!w) throw new ApiError(409, 'WORKSPACE_REQUIRED', 'conflict', uiText('请先选择材料'));
    const controller = action === 'reader/video_draft' ? new AbortController() : undefined;
    const timeout = controller ? setTimeout(() => controller.abort(new Error(uiText('视频提问保存超时，请重试。'))), 15_000) : undefined;
    const refresh = async () => {
      const latest = await networkFetch<NetworkWorkspace>('GET', `/workspaces/${w.workspace_id}`, undefined, controller?.signal);
      if (latest.generation !== w.generation) {
        installWorkspace(latest);
        throw new ApiError(409, 'WORKSPACE_STALE', 'conflict', uiText('阅读内容已更新，请重试当前操作。'));
      }
      installWorkspace(latest);
      return latest;
    };
    const post = (stamp: NetworkWorkspace) => networkFetch<NetworkWorkspace & { result?: T }>('POST', `/workspaces/${w.workspace_id}/${action}`, {
      ...body, attachment_id: n.attachment, generation: stamp.generation, expected_revision: stamp.revision,
    }, controller?.signal);
    // Reader positions and video drafts carry an absolute target. The server rejects a stale CAS
    // before mutation; only that explicit conflict permits one refresh/retry.
    try {
      let value;
      try { value = await post(optimistic ? w : await refresh()); }
      catch (error) {
        if (!optimistic || !(error instanceof ApiError) || error.errorCode !== 'WORKSPACE_REVISION_CONFLICT') throw error;
        value = await post(await refresh());
      }
      if (value.workspace_id) installWorkspace(value, n.attachment, n.linked,
        action === 'chat/new' || action === 'chat/select' ? 'chat' : action === 'reader/adopt_same_source_publication' ? 'enrichment' : undefined);
      return value as T;
    } finally {
      clearTimeout(timeout);
    }
  });
  writes = result;
  return result;
}
export function publishedUrl(leaf: string) {
  const ref = network.value.workspace?.published_book_ref;
  if (!ref) throw new Error(uiText('请先选择材料'));
  return `/books/${encodeURIComponent(ref.book_id)}/publications/${encodeURIComponent(ref.publication_id)}/${leaf}`;
}

// Reads retain scene authority but do not join the CAS write queue or install an older snapshot.
export async function workspaceRead<T = unknown>(action: string, body: Record<string, unknown> = {}, signal?: AbortSignal): Promise<T> {
  const n = network.value, w = n.workspace;
  if (!w) throw new ApiError(409, 'WORKSPACE_REQUIRED', 'conflict', uiText('请先选择材料'));
  return networkFetch<T>('POST', `/workspaces/${w.workspace_id}/${action}`, {
    ...body, attachment_id: n.attachment, generation: w.generation, expected_revision: w.revision,
  }, signal);
}
const readActions = new Set(['boards/list', 'boards/read', 'boards/conversation', 'boards/catalog', 'boards/search', 'boards/related', 'boards/source.resolve', 'boards/source.read', 'boards/presentation.read', 'boards/presentation.observe', 'memory/presentation.read', 'memory/presentation.observe', 'reader/state', 'profile/manifest', 'profile/memory', 'memory/recall', 'memory/prepare_note',
  'reader/paper_minimap.state', 'reader/pdf_selection.resolve', 'reader/pdf_ranges.project', 'agent/source.resolve', 'agent/source.read', 'notes/source.read', 'reader/selection.translate', 'reader/guidance', 'tutor/ask-step', 'tutor/grade', 'knowledge/tree', 'knowledge/resume']);

interface Admission extends RunDescriptor {
  dispatch_state: string; persistence_state: string; snapshot?: RunSnapshot;
  turn?: import('@understand-book/web/reader/api').AgentChatTurn;
}
export function admissionSnapshot(value: Admission): RunSnapshot {
  if (value.snapshot) return { ...value.snapshot, board_commits: mergeBoardDeliveries(value.snapshot.board_commits, value.snapshot.final_view?.board_commits, value.turn?.board_commits), descriptor: { ...value.snapshot.descriptor, workspace_id: value.workspace_id, workspace_generation: value.workspace_generation, published_book_ref: value.published_book_ref } };
  const snapshot = { ...initialRun(value), board_commits: mergeBoardDeliveries(value.turn?.board_commits) };
  if (value.persistence_state === 'failed') return { ...snapshot, persistence_state: 'failed', execution_state: 'ended', error: value.turn?.error as RunSnapshot['error'] ?? null };
  if (value.dispatch_state === 'queued' || value.dispatch_state === 'preparing' || value.dispatch_state === 'claimed') return { ...snapshot, execution_state: value.dispatch_state === 'claimed' ? 'running' : 'queued' };
  return { ...snapshot, persistence_state: 'saved', execution_state: value.turn?.status === 'completed' ? 'completed' : value.turn?.status === 'cancelled' ? 'cancelled' : 'failed', final_view: value.turn ?? null, error: value.turn?.error as RunSnapshot['error'] ?? null };
}
export interface PendingSubmission { key: string; body: Record<string, unknown>; workspace: string; bodyStored?: boolean }
// A durable request can still be awaiting its original POST in this page.
const sendingSubmissions = new Set<string>();
function pendingPrefix() { return `understand-book:pending:${network.value.identity?.user_id}:`; }
function pendingChanged() { window.dispatchEvent(new Event('submission-pending-changed')); }
function clearPendingSubmission(storageKey: string) {
  const saved = sessionStorage.getItem(storageKey);
  if (saved && (JSON.parse(saved) as PendingSubmission).bodyStored) discardSubmissionBody(storageKey);
  sessionStorage.removeItem(storageKey);
}
export function pendingSubmissions(): PendingSubmission[] {
  return Object.keys(sessionStorage).filter(key => key.startsWith(pendingPrefix())).map(key => JSON.parse(sessionStorage.getItem(key)!));
}
export function unconfirmedSubmissions(): PendingSubmission[] {
  return pendingSubmissions().filter(item => !sendingSubmissions.has(pendingPrefix() + item.key));
}
export async function recoverSubmissions(): Promise<RunDescriptor[]> {
  const recovered: RunDescriptor[] = [];
  for (const item of unconfirmedSubmissions()) {
    try {
      const value = await networkFetch<Admission>('GET', `/agent/runs?client_request_id=${encodeURIComponent(item.key)}`);
      if (value.dispatch_state === 'preparing') continue;
      clearPendingSubmission(pendingPrefix() + item.key);
      recovered.push(value);
    } catch (error) {
      if (error instanceof ApiError && error.errorCode === 'REQUEST_KEY_CLOSED') clearPendingSubmission(pendingPrefix() + item.key);
      else if (!(error instanceof ApiError && error.status === 404)) throw error;
    }
  }
  pendingChanged();
  return recovered;
}
export async function retrySubmission(key: string) {
  await recoverSubmissions();
  const item = unconfirmedSubmissions().find(p => p.key === key);
  if (!item) { window.dispatchEvent(new Event('agent-admission-changed')); return; }
  if (item.bodyStored) {
    const scene = sceneKey();
    const body = await readSubmissionBody(pendingPrefix() + item.key);
    if (scene !== sceneKey()) throw new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', uiText('阅读现场已切换'));
    if (!body) {
      clearPendingSubmission(pendingPrefix() + item.key); pendingChanged();
      throw new ApiError(400, 'SUBMISSION_BODY_MISSING', 'validation', uiText('原问题的附件已不在浏览器中，请重新添加附件后发送。'));
    }
    await postSubmission({ key: item.key, workspace: item.workspace, body });
  } else await postSubmission(item);
}
async function createRun(body: Record<string, unknown>) {
  const initial = network.value.workspace;
  // The first question owns the chat it creates. An explicitly captured existing
  // chat still has to match, including after the workspace refresh below.
  if (body.session_id && !initial?.selected_chat) {
    throw new ApiError(409, 'CHAT_SELECTION_CHANGED', 'conflict', uiText('聊天已切换，请返回原聊天重新发起操作'));
  }
  const created = !initial?.selected_chat ? await workspaceAction<NetworkWorkspace>('chat/new') : null;
  const w = network.value.workspace!;
  const requestedSession = !body.session_id && created ? created.selected_chat : body.session_id;
  if (pendingSubmissions().some(p => p.body.session_id === w.selected_chat)) {
    await recoverSubmissions();
    if (pendingSubmissions().some(p => p.body.session_id === w.selected_chat)) throw new ApiError(409, 'SUBMISSION_UNCONFIRMED', 'conflict', uiText('原问题仍待核对，请先核对或重试原提交；不会自动重提'));
  }
  const latest = await networkFetch<NetworkWorkspace>('GET', `/workspaces/${w.workspace_id}`);
  if (latest.generation !== w.generation) { installWorkspace(latest); throw new ApiError(409, 'WORKSPACE_STALE', 'conflict', body.board_follow_up
    ? uiText('讨论已更新，问题仍保留在画板。请重试以继续当前讨论。')
    : uiText('阅读内容已更新，请返回原对话重新发送问题。')); }
  if ((requestedSession || body.entry_action || body.teaching_choice || body.board_follow_up) && (requestedSession !== w.selected_chat || latest.selected_chat !== w.selected_chat)) {
    installWorkspace(latest);
    throw new ApiError(409, 'CHAT_SELECTION_CHANGED', 'conflict', body.board_follow_up
      ? uiText('讨论已更新，问题仍保留在画板。请重试以继续当前讨论。')
      : uiText('聊天已切换，请返回原聊天重新发起操作'));
  }
  installWorkspace(latest);
  const key = typeof body.client_request_id === 'string' ? body.client_request_id : crypto.randomUUID();
  const input = { ...body, client_request_id: key, session_id: w.selected_chat, attachment_id: network.value.attachment,
    generation: latest.generation, expected_revision: latest.revision, published_book_ref: latest.published_book_ref };
  return postSubmission({ key, body: input, workspace: w.workspace_id });
}
async function postSubmission(item: PendingSubmission) {
  const storageKey = pendingPrefix() + item.key;
  const scene = sceneKey();
  const serialized = JSON.stringify(item);
  const large = serialized.length > 64 * 1024;
  sendingSubmissions.add(storageKey);
  try {
    try {
      if (large) await saveSubmissionBody(storageKey, (JSON.parse(serialized) as PendingSubmission).body);
      if (scene !== sceneKey()) throw new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', uiText('阅读现场已切换'));
      sessionStorage.setItem(storageKey, large ? JSON.stringify({
        key: item.key, workspace: item.workspace, bodyStored: true,
        body: { message: item.body.message, display_user: item.body.display_user, session_id: item.body.session_id },
      } satisfies PendingSubmission) : serialized);
    } catch (error) {
      if (large) discardSubmissionBody(storageKey);
      if (error instanceof ApiError) throw error;
      throw new ApiError(400, 'SUBMISSION_STORAGE_FAILED', 'validation', uiText('问题尚未发送，浏览器暂时无法保存重试信息。请保留内容后重试。'));
    }
    pendingChanged();
    let value: Admission;
    try {
      value = await networkFetch<Admission>('POST', `/workspaces/${item.workspace}/agent/runs`, item.body);
    } catch (error) {
      if (error instanceof ApiError && error.status < 500 && error.errorCode !== 'ADMISSION_PREPARING') {
        if (error.errorCode !== 'CLIENT_CONTEXT_STALE') clearPendingSubmission(storageKey);
        throw error;
      }
      // A lost reply may already have created the run. Query the original key before offering a retry.
      try { value = await networkFetch<Admission>('GET', `/agent/runs?client_request_id=${encodeURIComponent(item.key)}`); }
      catch { throw error; }
      if (value.dispatch_state === 'preparing') throw error;
    }
    clearPendingSubmission(storageKey);
    window.dispatchEvent(new Event('agent-admission-changed'));
    return value;
  } finally {
    sendingSubmissions.delete(storageKey);
    pendingChanged();
  }
}
async function history(): Promise<AgentHistoryResponse> {
  return (await workspaceRead<{ result: AgentHistoryResponse }>('chat/history')).result;
}

export async function networkRequest<T>(method: 'GET' | 'POST', path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const input = (body ?? {}) as Record<string, unknown>;
  if (path === '/profile/memory/apply' && !network.value.workspace?.selected_chat) await workspaceAction('chat/new');
  let value: unknown;
  if (path === '/agent/runs') value = await createRun(input);
  else if (/^\/agent\/runs\//.test(path)) value = admissionSnapshot(await networkFetch<Admission>(method, path, body));
  else if (path === '/agent/history') value = await history();
  else if (path.startsWith('/agent/follow-ups?') && method === 'GET') {
    const session_id = new URLSearchParams(path.slice(path.indexOf('?') + 1)).get('session_id');
    value = (await workspaceRead<{ result: unknown }>('chat/follow-ups', { session_id }, signal)).result;
  } else if (path === '/agent/follow-ups' && method === 'POST') {
    value = (await workspaceAction<{ result: unknown }>('chat/follow-ups/create', input)).result;
  }
  else if (path === '/learning-task' && method === 'GET') value = await networkFetch('GET', `/workspaces/${network.value.workspace!.workspace_id}/learning-task`);
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
    const result = await networkFetch<{ books: { published_book_ref: import('./network-context').PublishedBookRef; title?: string | null; cover?: import('@understand-book/web/reader/api').BookCoverSource | null }[] }>('GET', '/library');
    value = { root: '', books: result.books.map(b => ({ name: b.published_book_ref.book_id, title: b.title, book_id: b.published_book_ref.book_id, dir: JSON.stringify(b.published_book_ref), route: 'reader', cover: b.cover })) };
  } else if (path.startsWith('/book/')) value = await networkFetch(method, publishedUrl(path.slice(6)), body, signal);
  else if (path === '/tutor/state' || path === '/tutor/mutate') {
    const workspace = network.value.workspace;
    const scope = new URLSearchParams();
    if (workspace?.learning_workspace_id) scope.set('learning_workspace_id', workspace.learning_workspace_id);
    if (workspace?.published_book_ref.book_id) scope.set('source_id', workspace.published_book_ref.book_id);
    value = await networkFetch(method, scope.size ? `${path}?${scope}` : path, body);
  }
  else if (path === '/tutor/readiness') value = await networkFetch('GET', publishedUrl('tutor_readiness'));
  else if (path === '/agent/music/respond') {
    const result = await workspaceAction<{ result: unknown }>('music/respond', input);
    value = result.result;
  }
  else if (path === '/agent/presentation.interaction') {
    const result = await workspaceAction<{ result: unknown }>('presentation/interaction', input);
    value = result.result;
  }
  else if (path.startsWith('/agent/presentation.feedback.')) {
    const operation = path.slice('/agent/presentation.feedback.'.length);
    const result = await workspaceAction<{ result: unknown }>(`presentation/feedback/${operation}`, input);
    value = result.result;
  }
  else if (path.startsWith('/agent/presentation.read') || path.startsWith('/agent/presentation.observe')) {
    const result = await workspaceRead<{ result: unknown }>(path.endsWith("observe") ? "presentation/observe" : "presentation/read", input);
    value = result.result;
  } else if (path === '/reader/voice-status' || path === '/reader/voice-transcribe') {
    value = (await workspaceRead<{ result: unknown }>(path.slice(1), input, signal)).result;
  } else {
    const action = path === '/agent/presentation.state.save' ? 'presentation/save' : path.split('?')[0].slice(1);
    const request = readActions.has(action) ? workspaceRead : workspaceAction;
    const data = await request<NetworkWorkspace & { result?: unknown }>(action, input);
    // A missing practice or guidance decision is a valid null result. Do not
    // turn it into a workspace object, which has a different response shape.
    value = path === '/reader/state' ? data.reader : 'result' in data ? data.result : data;
  }
  return value as T;
}

// The cloud entry owns this adapter. Shared Reader modules never import this file.
installReaderHost({
  request: networkRequest, fetch: networkFetch,
  ownerHeaders: (): Record<string, string> => network.value.identity ? { 'X-CSRF-Token': network.value.identity.csrf_token } : {},
  publishedUrl, workspaceAction, recoverSubmissions, recoverWorkspaceBinding, requestWorkspaceRecovery, pendingSubmissions, retrySubmission,
  readCourseInstance: sharedWorkspaces.instance, readCourseRelease: sharedWorkspaces.read,
  openLinkedPresentation: async (...args) => (await import('./network-presentation')).openLinkedPresentation(...args),
  runNotice: spendNotice, runFailureMessage: spendFailureMessage,
});
