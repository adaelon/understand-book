// @vitest-environment happy-dom
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { api } from './api';
import { useTutorControl } from './useTutorControl';

afterEach(() => vi.restoreAllMocks());
beforeEach(() => { vi.spyOn(api, 'tutorReadiness').mockResolvedValue({ status: 'preparing', source_id: 'book', teaching_map_revision: null, limitations: [], reason: '准备中' }); });

it('shows source readiness independently and ignores a previous book response', async () => {
  const tutor = useTutorControl();
  let finish!: (value: Awaited<ReturnType<typeof api.tutorReadiness>>) => void;
  vi.mocked(api.tutorReadiness).mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
  const old = tutor.loadReadiness();
  vi.mocked(api.tutorReadiness).mockResolvedValueOnce({ status: 'ready', source_id: 'current-book', teaching_map_revision: 'v1', limitations: ['省略连接'], reason: '材料已就绪' });
  await tutor.loadReadiness();
  finish({ status: 'preparing', source_id: 'old-book', teaching_map_revision: null, limitations: [], reason: '准备中' });
  await old;
  expect(tutor.readiness.value?.source_id).toBe('current-book');
  expect(tutor.state.value).toBeNull();
  vi.mocked(api.tutorReadiness).mockRejectedValueOnce(new Error('unavailable'));
  await tutor.loadReadiness();
  expect(tutor.readiness.value).toBeNull();
  expect(tutor.readinessError.value).toContain('失败');
});
it('keeps the durable state on write failure and retries the exact operation', async () => {
  vi.spyOn(api, 'tutorState').mockResolvedValue({ control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} });
  const mutate = vi.spyOn(api, 'tutorMutate').mockRejectedValueOnce(new Error('disk full')).mockResolvedValue({ control: { enabled: true, revision: 1, current_tutor_session_id: null }, sessions: {} });
  const tutor = useTutorControl();
  await tutor.load();
  await tutor.act({ kind: 'set_enabled', enabled: true });
  expect(tutor.state.value?.control.enabled).toBe(false);
  expect(tutor.error.value).toContain('未确认保存');
  expect(tutor.pending.value).not.toBeNull();
  vi.mocked(api.tutorState).mockResolvedValue({ control: { enabled: true, revision: 1, current_tutor_session_id: null }, sessions: {} });
  await tutor.retry();
  expect(mutate.mock.calls[1][0]).toEqual(mutate.mock.calls[0][0]);
  expect(tutor.pending.value).toBeNull();
  expect(tutor.label.value).toContain('教学准备中');
});

it('does not invent disabled state when the persisted control cannot be read', async () => {
  vi.spyOn(api, 'tutorState').mockRejectedValue(new Error('unavailable'));
  const mutate = vi.spyOn(api, 'tutorMutate');
  const tutor = useTutorControl();
  await tutor.load();
  await tutor.act({ kind: 'set_enabled', enabled: true });
  expect(tutor.state.value).toBeNull();
  expect(tutor.label.value).toContain('未加载');
  expect(mutate).not.toHaveBeenCalled();
});
