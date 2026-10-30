// @vitest-environment happy-dom
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { api, ApiError } from './api';
import { useTutorControl } from './useTutorControl';

afterEach(() => vi.restoreAllMocks());
beforeEach(() => { vi.spyOn(api, 'tutorReadiness').mockResolvedValue({ status: 'preparing', source_id: 'book', source_revision: 's1', limitations: [], reason: '准备中', teaching_assets: { status: 'preparing', teaching_map_revision: null, limitations: [], reason: '资料缺失' } }); });

it('shows source readiness independently and ignores a previous book response', async () => {
  const tutor = useTutorControl();
  let finish!: (value: Awaited<ReturnType<typeof api.tutorReadiness>>) => void;
  vi.mocked(api.tutorReadiness).mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
  const old = tutor.loadReadiness();
  vi.mocked(api.tutorReadiness).mockResolvedValueOnce({ status: 'ready', source_id: 'current-book', source_revision: 's1', limitations: ['省略连接'], reason: '可以学习', teaching_assets: { status: 'ready', teaching_map_revision: 'v1', limitations: ['省略连接'], reason: '资料已就绪' } });
  await tutor.loadReadiness();
  finish({ status: 'preparing', source_id: 'old-book', source_revision: 's1', limitations: [], reason: '准备中', teaching_assets: { status: 'preparing', teaching_map_revision: null, limitations: [], reason: '资料缺失' } });
  await old;
  expect(tutor.readiness.value?.source_id).toBe('current-book');
  expect(tutor.state.value).toBeNull();
  vi.mocked(api.tutorReadiness).mockRejectedValueOnce(new Error('unavailable'));
  await tutor.loadReadiness();
  expect(tutor.readiness.value).toBeNull();
  expect(tutor.readinessError.value).toBe('学习状态暂时无法读取，请重试。');
});
it.each(['failure', 'success'])('ignores a previous chat state read after the current read completes (%s)', async outcome => {
  let finish!: (value: Awaited<ReturnType<typeof api.tutorState>>) => void;
  let reject!: (failure: Error) => void;
  const oldState = { workspace_goal_revision: null, learning_map_enabled: false, control: { enabled: false, revision: 1, current_tutor_session_id: null }, sessions: {} };
  const currentState = { ...oldState, control: { ...oldState.control, enabled: true, revision: 2 } };
  vi.spyOn(api, 'tutorState').mockReturnValueOnce(new Promise((yes, no) => { finish = yes; reject = no; }))
    .mockResolvedValue(currentState);
  const tutor = useTutorControl();
  const old = tutor.load();
  await tutor.load();
  if (outcome === 'failure') reject(new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', '阅读现场已切换'));
  else finish(oldState);
  await old;
  expect(tutor.error.value).toBe('');
  expect(tutor.state.value).toEqual(currentState);
});

it('retains an unconfirmed setting operation when its response belongs to a previous chat', async () => {
  vi.spyOn(api, 'tutorState').mockResolvedValue({ workspace_goal_revision: null, learning_map_enabled: false, control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} });
  const mutate = vi.spyOn(api, 'tutorMutate').mockRejectedValueOnce(new ApiError(409, 'CLIENT_CONTEXT_STALE', 'conflict', '阅读现场已切换'))
    .mockResolvedValue({ workspace_goal_revision: null, learning_map_enabled: false, control: { enabled: true, revision: 1, current_tutor_session_id: null }, sessions: {} });
  const tutor = useTutorControl(); await tutor.load();
  await tutor.act({ kind: 'set_enabled', enabled: true });
  expect(tutor.error.value).toBe('学习设置的保存结果待确认，请重试。');
  expect(tutor.pending.value).not.toBeNull();
  await tutor.retry();
  expect(mutate.mock.calls[1][0]).toEqual(mutate.mock.calls[0][0]);
  expect(tutor.pending.value).toBeNull();
});

it('keeps the durable state on write failure and retries the exact operation', async () => {
  vi.spyOn(api, 'tutorState').mockResolvedValue({ workspace_goal_revision: null, learning_map_enabled: false, control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} });
  const mutate = vi.spyOn(api, 'tutorMutate').mockRejectedValueOnce(new Error('disk full')).mockResolvedValue({ workspace_goal_revision: null, learning_map_enabled: false, control: { enabled: true, revision: 1, current_tutor_session_id: null }, sessions: {} });
  const tutor = useTutorControl();
  await tutor.load();
  await tutor.act({ kind: 'set_enabled', enabled: true });
  expect(tutor.state.value?.control.enabled).toBe(false);
  expect(tutor.error.value).toContain('未确认保存');
  expect(tutor.pending.value).not.toBeNull();
  vi.mocked(api.tutorState).mockResolvedValue({ workspace_goal_revision: null, learning_map_enabled: false, control: { enabled: true, revision: 1, current_tutor_session_id: null }, sessions: {} });
  await tutor.retry();
  expect(mutate.mock.calls[1][0]).toEqual(mutate.mock.calls[0][0]);
  expect(tutor.pending.value).toBeNull();
  expect(tutor.label.value).toContain('学习基础准备中');
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

it('does not apply a previous book save after the local reader changes books', async () => {
  const empty = { workspace_goal_revision: null, learning_map_enabled: false, control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} };
  vi.spyOn(api, 'tutorState').mockResolvedValue(empty);
  let finish!: (value: typeof empty) => void;
  vi.spyOn(api, 'tutorMutate').mockReturnValue(new Promise(resolve => { finish = resolve; }));
  const tutor = useTutorControl(); await tutor.load();
  const saving = tutor.act({ kind: 'set_enabled', enabled: true });
  tutor.reset(); await tutor.load();
  finish({ ...empty, control: { ...empty.control, enabled: true, revision: 1 } });
  await saving;
  expect(tutor.state.value).toEqual(empty);
  expect(tutor.pending.value).toBeNull();
  expect(tutor.error.value).toBe('');
});

it('restores the server-selected source goal independently of teaching readiness', async () => {
  const state = { workspace_goal_revision: null, learning_map_enabled: false, control: { enabled: true, revision: 2, current_tutor_session_id: 'learn' }, sessions: {
    learn: { id: 'learn', revision: 1, workspace_goal_revision: null, status: 'active' as const, user_intent: '理解速度', explicit_constraints: [],
      current_focus: { interpretation: '', target_object_refs: [], capability_targets: [] },
      material_scope: [{ source_id: 'book', scope_refs: [], role: 'primary' as const }], default_teaching_intent: null, path_instance_ref: null, progress_ref: null },
  } };
  vi.spyOn(api, 'tutorState').mockResolvedValue(state);
  const tutor = useTutorControl();
  await tutor.load();
  const ready = { status: 'ready' as const, source_id: 'book', source_revision: 's1', limitations: [], reason: '可以学习',
    teaching_assets: { status: 'stale' as const, teaching_map_revision: null, limitations: [], reason: '教学构建失败' } };
  vi.mocked(api.tutorReadiness).mockResolvedValue(ready);
  await tutor.loadReadiness();
  expect(tutor.label.value).toContain('可以开始或继续学习');
  expect(tutor.current.value?.id).toBe('learn');
  expect(tutor.state.value?.control.enabled).toBe(true);
  tutor.state.value!.sessions.learn!.status = 'paused';
  expect(tutor.label.value).toContain('已暂停');
  tutor.state.value!.sessions.learn!.status = 'active';
  vi.mocked(api.tutorReadiness).mockRejectedValueOnce(new Error('preparing'));
  await tutor.loadReadiness();
  expect(tutor.current.value?.id).toBe('learn');
  expect(tutor.label.value).toContain('读取失败');
  vi.mocked(api.tutorState).mockResolvedValueOnce({ ...state, control: { ...state.control, current_tutor_session_id: null } });
  await tutor.load();
  expect(tutor.current.value).toBeNull();
  expect(tutor.state.value?.sessions.learn).toBeDefined();
  await tutor.load();
  expect(tutor.current.value?.id).toBe('learn');
  expect(tutor.label.value).toContain('可以开始或继续学习');
});
