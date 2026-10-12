import { computed, ref } from 'vue';
import { api as sharedApi, ApiError } from './api';
import { bindSceneApi, readerKey } from './network-context';
import type { TutorState } from './generated/TutorState';
import type { TutorAction } from './generated/TutorAction';
import type { TutorMutation } from './generated/TutorMutation';

export function useTutorControl() {
  const api = bindSceneApi(sharedApi, readerKey);
  const state = ref<TutorState | null>(null);
  const busy = ref(false);
  const error = ref('');
  const pending = ref<TutorMutation | null>(null);
  const readiness = ref<Awaited<ReturnType<typeof api.tutorReadiness>> | null>(null);
  const readinessError = ref('');
  let readinessRequest = 0;
  async function loadReadiness() {
    const request = ++readinessRequest;
    readiness.value = null;
    readinessError.value = '';
    try { const result = await api.tutorReadiness(); if (request === readinessRequest) readiness.value = result; }
    catch { if (request === readinessRequest) readinessError.value = '学习就绪状态读取失败，请重试'; }
  }
  const current = computed(() => state.value?.sessions[state.value.control.current_tutor_session_id ?? ''] ?? null);
  const currentForSource = computed(() => current.value?.material_scope.some(material => material.source_id === readiness.value?.source_id) ? current.value : null);
  const hasOtherSourceSession = computed(() => !!readiness.value?.source_id && !!current.value && !currentForSource.value);
  const label = computed(() => !state.value ? '连续教学状态未加载' : !state.value.control.enabled ? '围绕你的目标逐步学'
    : readinessError.value ? '学习就绪状态读取失败'
    : !readiness.value ? '正在读取学习状态'
    : hasOtherSourceSession.value ? '当前书籍不属于本次学习'
    : current.value?.status === 'ended' ? '本次学习已结束'
    : current.value?.status === 'paused' ? '本次学习已暂停'
    : !current.value && readiness.value.status === 'ready' ? '输入第一个问题即可开始'
    : readiness.value.status === 'ready' ? '可以开始或继续学习' : '学习基础准备中');
  async function load() {
    void loadReadiness();
    busy.value = true;
    try { state.value = await api.tutorState(); error.value = ''; }
    catch (failure) { error.value = `教学状态读取失败：${failure instanceof Error ? failure.message : String(failure)}`; }
    finally { busy.value = false; }
  }
  async function submit(request: TutorMutation) {
    busy.value = true; error.value = ''; pending.value = request;
    try {
      state.value = await api.tutorMutate(request);
      pending.value = null;
    } catch (failure) {
      error.value = `教学设置未确认保存：${failure instanceof Error ? failure.message : String(failure)}`;
      if (failure instanceof ApiError && failure.status < 500) pending.value = null;
    } finally { busy.value = false; }
  }
  async function act(action: TutorAction) {
    if (busy.value || !state.value || pending.value) return;
    await submit({ operation_id: crypto.randomUUID(), expected_revision: state.value.control.revision, action });
  }
  async function retry() {
    if (busy.value) return;
    if (pending.value) { await submit(pending.value); if (!pending.value) await load(); }
    else await load();
  }
  return { state, busy, error, pending, current, currentForSource, hasOtherSourceSession, label, load, act, retry, readiness, readinessError, loadReadiness };
}
