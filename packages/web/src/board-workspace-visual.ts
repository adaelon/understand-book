import { createApp, defineComponent, h, ref } from 'vue';
import BoardWorkspace from './components/BoardWorkspace.vue';
import BoardSourcePreview from './components/BoardSourcePreview.vue';
import { api, type SourcePopupView } from './api';
import type { BoardQuestionRequest, BoardRunView, BoardSourceRequest } from './board-workspace';
import './style.css';

createApp(defineComponent({ setup() {
  const open = ref(true), source = ref('');
  const sourceRequest = ref<BoardSourceRequest | null>(null), sourcePreview = ref<SourcePopupView | null>(null);
  const sourceLoading = ref(false), sourceOpening = ref(false), sourceError = ref('');
  async function previewSource(target: BoardSourceRequest) {
    sourceRequest.value = target; sourcePreview.value = null; sourceLoading.value = true; sourceError.value = '';
    try { sourcePreview.value = await api.boardSourceResolve(target); }
    catch (failure) { sourceError.value = String(failure); }
    finally { sourceLoading.value = false; }
  }
  async function openSourceReader(lid?: string) {
    if (!sourceRequest.value) return;
    sourceOpening.value = true; sourceError.value = '';
    try {
      await api.boardSourceOpen({ ...sourceRequest.value, ...(lid ? { lid } : {}) });
      source.value = `已定位原文：${sourceRequest.value.source_ref_id}${lid ? ` · ${lid}` : ''}`;
      sourceRequest.value = null; open.value = false;
    } catch (failure) { sourceError.value = String(failure); }
    finally { sourceOpening.value = false; }
  }
  const run = ref<BoardRunView | null>(null);
  let request: BoardQuestionRequest | null = null;
  async function followUp(next: BoardQuestionRequest) {
    request = next;
    run.value = { request_id: next.client_request_id, board_id: next.context.board_id, item_id: next.context.item_id, question: next.message, accepted: true, pending: true, can_retry: false, can_stop: true, status_text: '正在根据画板内容组织解释…' };
    try {
      const response = await fetch('/api/fixture/board-follow-up', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(next) });
      if (!response.ok) throw new Error('这次追问暂时没有完成，请重试。');
      run.value = { ...run.value, pending: false, can_stop: false, status_text: '已保存到画板', delivery: await response.json() };
    } catch (failure) { run.value = { ...run.value!, pending: false, can_stop: false, can_retry: true, error: String(failure) }; }
  }
  return () => h('main', { style: 'height:100%;position:relative' }, [
    h('div', { style: 'padding:32px', 'data-testid': 'board-returned-source' }, [h('h1', '条件概率与贝叶斯定理'), h('p', source.value || '在原文中继续阅读。'), h('button', { onClick: () => { open.value = true; } }, '返回画板')]),
    h(BoardWorkspace, { boardId: 'board-bayes', run: run.value, onFollowUp: followUp, onRetryFollowUp: () => { if (request) void followUp(request); }, onCancelFollowUp: () => { if (run.value) run.value = { ...run.value, pending: false, can_stop: false, error: '已停止' }; }, style: `position:absolute;inset:0;${open.value ? '' : 'display:none'}`, onClose: () => { open.value = false; }, onSource: previewSource }),
    sourceRequest.value ? h(BoardSourcePreview, { source: sourcePreview.value, sourceRequest: sourceRequest.value, loading: sourceLoading.value, opening: sourceOpening.value, error: sourceError.value, onClose: () => { sourceRequest.value = null; }, onOpenReader: openSourceReader, onRetry: () => { if (sourceRequest.value) void previewSource(sourceRequest.value); } }) : null,
  ]);
} })).mount('#app');
