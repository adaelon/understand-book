import { t as uiText } from './i18n';
import { computed, ref } from 'vue';
import { api as sharedApi, ApiError } from './api';
import { bindSceneApi, readerKey } from './reader-context';
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
  let stateRequest = 0;
  let scopeGeneration = 0;
  async function loadReadiness() {
    const request = ++readinessRequest;
    readiness.value = null;
    readinessError.value = '';
    try { const result = await api.tutorReadiness(); if (request === readinessRequest) readiness.value = result; }
    catch (failure) {
      if (request === readinessRequest && !(failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE')) {
        readinessError.value = uiText('学习状态暂时无法读取，请重试。');
      }
    }
  }
  // The server resolves the saved stage for this source and learning workspace.
  // Readiness is independent: a preparing source must still show its saved goal.
  const current = computed(() => state.value?.sessions[state.value.control.current_tutor_session_id ?? ''] ?? null);
  const label = computed(() => !state.value ? uiText('学习状态未加载')
    : readinessError.value ? uiText('学习就绪状态读取失败')
    : !readiness.value ? uiText('正在读取学习状态')
    : current.value?.status === 'ended' ? uiText('本次学习已结束')
    : current.value?.status === 'paused' ? uiText('本次学习已暂停')
    : !current.value && readiness.value.status === 'ready' ? uiText('输入第一个问题即可开始')
    : readiness.value.status === 'ready' ? uiText('可以开始或继续学习') : uiText('学习基础准备中'));
  async function load() {
    const request = ++stateRequest;
    void loadReadiness();
    busy.value = true;
    try {
      const result = await api.tutorState();
      if (request === stateRequest) { state.value = result; if (!pending.value) error.value = ''; }
    } catch (failure) {
      if (request === stateRequest && !(failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE')) {
        error.value = uiText('学习状态读取失败：{0}', [failure instanceof Error ? failure.message : String(failure)]);
      }
    } finally { if (request === stateRequest) busy.value = false; }
  }
  async function submit(request: TutorMutation) {
    const scope = scopeGeneration;
    stateRequest++;
    busy.value = true; error.value = ''; pending.value = request;
    try {
      const result = await api.tutorMutate(request);
      if (scope !== scopeGeneration) return;
      state.value = result;
      pending.value = null;
    } catch (failure) {
      if (scope !== scopeGeneration) return;
      if (failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE') {
        error.value = uiText('学习设置的保存结果待确认，请重试。');
      } else {
        error.value = uiText('学习设置未确认保存：{0}', [failure instanceof Error ? failure.message : String(failure)]);
        if (failure instanceof ApiError && failure.status < 500) pending.value = null;
      }
    } finally { if (scope === scopeGeneration) busy.value = false; }
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
  function reset() {
    scopeGeneration++;
    stateRequest++; readinessRequest++;
    state.value = null; readiness.value = null; pending.value = null;
    busy.value = false; error.value = ''; readinessError.value = '';
  }
  return { state, busy, error, pending, current, label, load, act, retry, reset, readiness, readinessError, loadReadiness };
}
