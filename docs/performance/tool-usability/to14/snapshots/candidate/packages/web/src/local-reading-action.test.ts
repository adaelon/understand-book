// @vitest-environment happy-dom
import { beforeEach, expect, it, vi } from 'vitest';
import { createReadingAction, pendingReadingActions, recoverReadingAction } from './local-reading-action';
beforeEach(() => sessionStorage.clear());
const body = { message: '', session_id: 'chat', client_request_id: 'same-key', expected_chat_head: 'origin', teaching_choice: { delivery_ref: 'd', option_id: 'compare' } };
it('keeps unknown input across reload and queries before an explicit retry with unchanged identity', async () => {
  const request = vi.fn().mockRejectedValueOnce(new Error('lost')).mockResolvedValueOnce({ accepted: false });
  await expect(createReadingAction(request, body)).rejects.toThrow('lost');
  const [saved] = pendingReadingActions(); expect(saved.body).toEqual(body);
  request.mockResolvedValueOnce({ accepted: false }).mockResolvedValueOnce({ turn_id: 'accepted' });
  await recoverReadingAction(request, saved, true);
  expect(request.mock.calls.slice(-2)).toEqual([['POST', '/agent/reading-action-status', body], ['POST', '/agent/runs', body]]);
  expect(pendingReadingActions()).toEqual([]);
});
it('recovers accepted input without resubmitting after a lost response', async () => {
  const request = vi.fn().mockRejectedValueOnce(new Error('lost')).mockResolvedValueOnce({ turn_id: 'accepted', session_id: 'chat' });
  expect(await createReadingAction(request, body)).toMatchObject({ turn_id: 'accepted' });
  expect(request).toHaveBeenCalledTimes(2); expect(pendingReadingActions()).toEqual([]);
});
it('preserves unknown input when query fails and clears definitively rejected input', async () => {
  const request = vi.fn().mockRejectedValue(new Error('offline'));
  await expect(createReadingAction(request, body)).rejects.toThrow();
  await expect(createReadingAction(request, { ...body, client_request_id: 'new' })).rejects.toThrow();
  expect(pendingReadingActions()).toHaveLength(1);
  request.mockResolvedValueOnce({ accepted: false }).mockRejectedValueOnce({ status: 409, errorCode: 'INTERACTION_STALE' });
  await expect(recoverReadingAction(request, pendingReadingActions()[0], true)).rejects.toMatchObject({ status: 409 });
  expect(pendingReadingActions()).toEqual([]);
});
