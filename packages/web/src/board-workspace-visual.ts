import { createApp, defineComponent, h, ref } from 'vue';
import BoardWorkspace from './components/BoardWorkspace.vue';
import type { BoardQuestionRequest, BoardRunView } from './board-workspace';
import './style.css';

createApp(defineComponent({ setup() {
  const open = ref(true), source = ref('');
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
    h(BoardWorkspace, { boardId: 'board-bayes', run: run.value, onFollowUp: followUp, onRetryFollowUp: () => { if (request) void followUp(request); }, onCancelFollowUp: () => { if (run.value) run.value = { ...run.value, pending: false, can_stop: false, error: '已停止' }; }, style: `position:absolute;inset:0;${open.value ? '' : 'display:none'}`, onClose: () => { open.value = false; }, onSource: (target: { source_ref_id: string }) => { source.value = `已定位原文：${target.source_ref_id}`; open.value = false; } }),
  ]);
} })).mount('#app');
