import { ApiError, type api } from './api';
import type { ClosedBookPractice } from './generated/ClosedBookPractice';

type ClosedBookRequest = Parameters<typeof api.closedBook>[0];
export type PendingClosedBookMutation = { request: ClosedBookRequest; message?: string };
type Storage = Pick<globalThis.Storage, 'getItem' | 'setItem' | 'removeItem'>;

export function closedBookDraftKey(owner: string, chat: string, practice: Pick<ClosedBookPractice, 'id' | 'attempt'> | null) {
  return `understand-book:learning:${owner}:closed-book:${JSON.stringify([chat,practice?.id,practice?.attempt])}`;
}

export function closedBookMutationKey(owner: string, chat: string) {
  return `understand-book:learning:${owner}:closed-book-operation:${chat}`;
}

export function pendingClosedBookMutation(storage: Storage, key: string): PendingClosedBookMutation | null {
  const saved = storage.getItem(key);
  return saved ? JSON.parse(saved) : null;
}

/** Keep the original operation until its outcome is known, including across a reload. */
export async function executeClosedBookMutation(
  storage: Storage, key: string, send: typeof api.closedBook, create: () => PendingClosedBookMutation,
) {
  const previous = pendingClosedBookMutation(storage,key), operation = previous ?? create();
  storage.setItem(key,JSON.stringify(operation));
  let value: ClosedBookPractice | null;
  try { value = await send(operation.request); }
  catch (error) {
    if (error instanceof ApiError && error.errorCode === 'CLOSED_BOOK_REVISION_CONFLICT') {
      value = await send({action:'state',chat_session_id:operation.request.chat_session_id});
      // The server definitively rejected this operation. Retain the learner's intent
      // for an explicit retry against the refreshed revision, with a new operation ID.
      storage.setItem(key,JSON.stringify({...operation,request:{...operation.request,revision:value?.revision ?? 0,operation_id:crypto.randomUUID()}}));
      return { value, operation, conflict:true };
    }
    if (error instanceof ApiError && error.status < 500 && error.errorCode !== 'CLIENT_CONTEXT_STALE') storage.removeItem(key);
    throw error;
  }
  // A replay receipt can predate later tutor updates. Install the current state.
  if (previous) value = await send({action:'state',chat_session_id:operation.request.chat_session_id});
  storage.removeItem(key);
  return { value, operation, conflict:false };
}
