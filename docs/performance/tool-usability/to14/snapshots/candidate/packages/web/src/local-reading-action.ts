// Only typed local reading actions need a persisted unknown-submission record.
// The server checks the original identity before current source/head validity.
type Request = <T>(method: 'GET' | 'POST', path: string, body?: unknown) => Promise<T>;
export interface PendingReadingAction { key: string; body: Record<string, unknown> }
const prefix = 'understand-book:local-reading-action:';
export function pendingReadingActions(): PendingReadingAction[] {
  return Object.keys(sessionStorage).filter(k => k.startsWith(prefix)).map(k => JSON.parse(sessionStorage.getItem(k)!));
}
function changed() { window.dispatchEvent(new Event('local-reading-action-changed')); }
function clear(key: string) { sessionStorage.removeItem(prefix + key); changed(); }
function rejected(error: unknown) {
  const e = error as { status?: number; errorCode?: string };
  return !!e.status && e.status < 500 && e.errorCode !== 'CLIENT_CONTEXT_STALE';
}
export async function recoverReadingAction(request: Request, item: PendingReadingAction, retry = false) {
  const result = await request<{ accepted?: false; turn_id?: string; session_id?: string; book_id?: string }>('POST', '/agent/reading-action-status', item.body);
  if (result.turn_id) { clear(item.key); return result; }
  if (!retry) return null;
  return send(request, item);
}
async function send(request: Request, item: PendingReadingAction): Promise<unknown> {
  try { const result = await request('POST', '/agent/runs', item.body); clear(item.key); return result; }
  catch (error) {
    if (rejected(error)) clear(item.key);
    else {
      try { const accepted = await recoverReadingAction(request, item); if (accepted) return accepted; }
      catch { /* Leave the original identity for explicit recovery. */ }
    }
    throw error;
  }
}
export async function createReadingAction<T>(request: Request, body: Record<string, unknown>): Promise<T> {
  const previous = pendingReadingActions().find(item => item.body.session_id === body.session_id);
  if (previous) {
    await recoverReadingAction(request, previous);
    if (pendingReadingActions().some(item => item.key === previous.key)) throw new Error('原操作结果待核对，请先核对原操作。');
  }
  const key = String(body.client_request_id);
  const item = { key, body };
  sessionStorage.setItem(prefix + key, JSON.stringify(item)); changed();
  return await send(request, item) as T;
}
