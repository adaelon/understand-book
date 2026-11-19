// @vitest-environment happy-dom
import { effectScope, nextTick, type EffectScope } from 'vue';
import { afterEach, expect, it, vi } from 'vitest';
import { network } from '@understand-book/web/reader/reader-context';
import { useAccountPresence } from './account-presence';

const send = vi.hoisted(() => vi.fn().mockResolvedValue({ok:true}));
vi.mock('./network-client', () => ({networkFetch:send}));
let scope: EffectScope;
afterEach(() => {
  scope?.stop(); network.value={...network.value,identity:null};
  vi.restoreAllMocks(); vi.useRealTimers(); send.mockClear();
});

it('sends only while signed in and visible, and stops on logout or unmount', async () => {
  vi.useFakeTimers();
  const visible=vi.spyOn(document,'visibilityState','get').mockReturnValue('visible');
  scope=effectScope(); scope.run(useAccountPresence);
  await vi.advanceTimersByTimeAsync(30000); expect(send).not.toHaveBeenCalled();
  network.value={...network.value,identity:{user_id:'A',csrf_token:'a'}};
  await nextTick(); await vi.advanceTimersByTimeAsync(1); expect(send).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(30000); expect(send).toHaveBeenCalledTimes(2);
  visible.mockReturnValue('hidden'); document.dispatchEvent(new Event('visibilitychange'));
  await vi.advanceTimersByTimeAsync(90000); expect(send).toHaveBeenCalledTimes(2);
  visible.mockReturnValue('visible'); document.dispatchEvent(new Event('visibilitychange'));
  await vi.advanceTimersByTimeAsync(1); expect(send).toHaveBeenCalledTimes(3);
  network.value={...network.value,identity:null}; await nextTick();
  await vi.advanceTimersByTimeAsync(30000); expect(send).toHaveBeenCalledTimes(3);
  network.value={...network.value,identity:{user_id:'B',csrf_token:'b'}}; await nextTick();
  await vi.advanceTimersByTimeAsync(1); expect(send).toHaveBeenCalledTimes(4);
  const signal=send.mock.calls.at(-1)![3] as AbortSignal;
  scope.stop(); expect(signal.aborted).toBe(true);
  await vi.advanceTimersByTimeAsync(30000); expect(send).toHaveBeenCalledTimes(4);
});

it('does not overlap requests and retries after a failed heartbeat', async () => {
  vi.useFakeTimers(); vi.spyOn(document,'visibilityState','get').mockReturnValue('visible');
  let reject!: (error:Error) => void;
  send.mockImplementationOnce(() => new Promise((_resolve, fail) => {reject=fail;}));
  network.value={...network.value,identity:{user_id:'A',csrf_token:'a'}};
  scope=effectScope(); scope.run(useAccountPresence);
  document.dispatchEvent(new Event('visibilitychange'));
  expect(send).toHaveBeenCalledTimes(1);
  reject(new Error('offline')); await vi.advanceTimersByTimeAsync(30000);
  expect(send).toHaveBeenCalledTimes(2);
});
