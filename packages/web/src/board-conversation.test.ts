// @vitest-environment happy-dom
import { expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import { mergeBoardConversation, type BoardDiscussionTurn } from './board-conversation';
import BoardDiscussion from './components/BoardDiscussion.vue';
import type { AgentChatTurn, BoardConversationSession, OuterOutcome } from './api';
import type { RunActivity } from './agent-run-state';

it('shows real board work and restores terminal activities instead of a stale running spinner', async () => {
  const working = { step_id: 3, parent_step_id: null, kind: 'tool', name: 'board.author', label: '整理画板', status: 'running', started_ms: 100, duration_ms: null, error_code: null, result_count: null, usage_total_tokens: null, usage: null, model_first_text_ms: null, model_name: null, model_name_source: null, accepted_evidence_count: null, evidence_refs: [] } as RunActivity;
  const live: BoardDiscussionTurn = { sessionId: 'chat', turnId: 'turn', user: '画出联系', pending: true, outcome: null, activities: [working] };
  const wrapper = mount(BoardDiscussion, { props: { turns: [live], items: [] } });
  expect(wrapper.get('[data-step-id="3"]').text()).toContain('整理画板');
  const saved = { ...stored('turn', '画出联系', '图解已保存。'), run_summary: { effects: [], trace: [], activities: [{ ...working, status: 'succeeded' as const }] } };
  const merged = mergeBoardConversation([{ session_id: 'chat', title: '讨论', turns: [saved] }], [live]);
  expect(merged[0].activities?.[0].status).toBe('succeeded');
  await wrapper.setProps({ turns: merged });
  expect(wrapper.find('.activity-spinner').exists()).toBe(false);
  expect(wrapper.text()).toContain('查看工作过程');
  wrapper.unmount();
});

function outcome(answer: string): OuterOutcome { return { answer, answer_view: { parts: [{ kind: 'markdown', text: answer }], sources: [] }, incomplete: false,
  warning: null, turns: 1, tokens_spent: 0, effects: [], trace: [], memory_updates: [], profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] } }; }
function stored(turn_id: string, user: string, answer: string | null): AgentChatTurn {
  return { turn_id, user_turn_ordinal: 1, user, status: answer ? 'completed' : 'pending_assistant', outcome: answer ? outcome(answer) : null,
    question_source_label: null, question_quote: null, effect_labels: [] };
}
it('keeps a previous session and merges the live draft into its persisted session and turn exactly once', () => {
  const sessions: BoardConversationSession[] = [
    { session_id: 'old-chat', title: '初次理解', turns: [stored('same-id', '什么是状态？', '状态表示当前已经积累的信息。')] },
    { session_id: 'current-chat', title: '继续理解', turns: [stored('same-id', '上下文如何影响状态？', null)] },
  ];
  const live: BoardDiscussionTurn = { sessionId: 'current-chat', turnId: 'same-id', user: '上下文如何影响状态？', outcome: null, pending: true,
    draft: { message_id: 1, revision: 2, operation: 'replace', view: { parts: [{ kind: 'markdown', text: '我们先把上下文中的 **token** 连到状态矩阵。' }], sources: [] } }, runStatus: '正在画出关系' };
  const turns = mergeBoardConversation(sessions, [live]);
  expect(turns).toHaveLength(2);
  const wrapper = mount(BoardDiscussion, { props: { turns, items: [] } });
  expect(wrapper.findAll('.board-discussion-question')).toHaveLength(2);
  expect(wrapper.text()).toContain('状态表示当前已经积累的信息');
  expect(wrapper.get('.answer-draft').text()).toContain('token');
  expect(wrapper.get('.board-discussion-status').text()).toContain('正在画出关系');
  wrapper.unmount();
});
it('lets a saved result replace a retained running state after leaving its chat', () => {
  const live: BoardDiscussionTurn = { sessionId: 'old', turnId: 'turn', user: '继续', outcome: null, pending: true,
    draft: { message_id: 1, revision: 1, operation: 'replace', view: { parts: [{ kind: 'markdown', text: '之前的草稿' }], sources: [] } } };
  const [merged] = mergeBoardConversation([{ session_id: 'old', title: '原讨论', turns: [stored('turn', '继续', '完整的解释已保存。')] }], [live]);
  expect(merged.pending).toBe(false); expect(merged.outcome?.answer).toBe('完整的解释已保存。');
  const wrapper = mount(BoardDiscussion, { props: { turns: [merged], items: [] } });
  expect(wrapper.text()).toContain('完整的解释已保存。'); expect(wrapper.find('.answer-draft').exists()).toBe(false);
  wrapper.unmount();
});
it('preserves an interrupted draft and its actual error together', () => {
  const turn: BoardDiscussionTurn = { sessionId: 'chat', turnId: 'turn', user: '画出联系', outcome: null, pending: false, error: '连接暂时中断',
    draft: { message_id: 1, revision: 1, operation: 'replace', view: { parts: [{ kind: 'markdown', text: '已经理解的第一步' }], sources: [] } } };
  const wrapper = mount(BoardDiscussion, { props: { turns: [turn], items: [] } });
  expect(wrapper.get('.answer-draft').text()).toContain('已经理解的第一步');
  expect(wrapper.get('[role="alert"]').text()).toContain('连接暂时中断');
  wrapper.unmount();
});
