// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import LearningRecordsPanel from './LearningRecordsPanel.vue';
import { installIdentity, installWorkspace, network, type LearningWorkspace, type NetworkWorkspace } from '../network-context';
import type { ImportedLearningRecord, LearningRecordDetail, LearningRecordSummary } from '../learning-records';

const target: LearningWorkspace = { learning_workspace_id: 'target', goal: { text: '期末考试冲刺', constraints: [], revision: 3, created_at: 1 }, goal_history: [], members: [], archived: false, created_at: 1, updated_at: 1 };
const record: LearningRecordSummary = { session_id: 'old-chat', title: '链式法则的旧讨论', book_id: 'calculus', source_learning_workspace_id: null, source_goal: null, published_book_ref: { book_id: 'calculus', publication_id: 'p1' }, published_book_refs: [{ book_id: 'calculus', publication_id: 'p1' }], turn_count: 2, through_seq: 12, updated_at: '1' };
const imported: ImportedLearningRecord = { selection: { source_learning_workspace_id: null, chat_session_id: 'old-chat', turn_ids: ['t1', 't2'], through_seq: 12 }, linked_at: '2', through_seq: 12, title: record.title, book_id: 'calculus', available: true };
const detail: LearningRecordDetail = { session_id: 'old-chat', title: record.title, learning_workspace_id: null, book_id: 'calculus', through_seq: 12,
  turns: [{ turn_id: 't1', user_turn_ordinal: 1, user: '链式法则为什么相乘？', status: 'completed', outcome: { answer: '分别计算每一层的变化率。' } as any, question_source_label: null, question_quote: null, effect_labels: [], workspace_goal: null, accepted_workspace_goal: null }],
  recap: { session_id: 'old-chat', through_seq: 12, through_at: '1', generated_at: '1', questions: [], sources: [], effects: [], continuations: [] },
  sources: [{ turn_id: 't1', source_ref_id: 'source', label: '参考书第一节', quote: '复合函数的变化率', published_book_ref: { book_id: 'reference', publication_id: 'original' }, unavailable_reason: null,
    view: { source_ref_id: 'source', label: '参考书第一节', highlighted_quote: '复合函数的变化率', context_before: '', context_after: '', stale: true, can_open_in_reader: false } }],
  learning: { selection: imported.selection, linked_at: '', facts: [{ event_id: 'f1', kind: 'question', occurred_at: '1', causal_refs: [] }], assessments: [], evidence: [
    { evidence_id: 'e1', label: '链式法则', nature: 'hypothesis', interpretation: '原来的解释', learner_quote: '为什么相乘', teaching_implication: '', correction: null, supersedes: null, fact_refs: ['f1'] },
    { evidence_id: 'e2', label: '链式法则', nature: 'hypothesis', interpretation: '已理解复合变化', learner_quote: '每一层都会影响下一层', teaching_implication: '', correction: '我理解了', supersedes: 'e1', fact_refs: ['f1'] },
  ] },
};

afterEach(() => { vi.unstubAllGlobals(); document.body.innerHTML = ''; sessionStorage.clear(); network.value = { ...network.value, enabled: false, identity: null, workspace: null }; });
function setup(records = [record], fail = false, initialLinked: ImportedLearningRecord[] = [], recordDetail = detail) {
  installIdentity({ user_id: 'reader', csrf_token: 'csrf' });
  const scene = { workspace_id: 'scene', learning_workspace_id: 'target', generation: 1, revision: 1, selected_chat: 'target-chat', published_book_ref: { book_id: 'target-book', publication_id: 'current' }, reader: {} } as NetworkWorkspace;
  installWorkspace(scene);
  let linked: ImportedLearningRecord[] = initialLinked;
  const fetch = vi.fn(async (url: string, init: RequestInit) => {
    if (url === '/api/me/learning-records') return Response.json({ records });
    if (url === '/api/me/learning-records/old-chat?through_seq=12') return Response.json(recordDetail);
    if (url === '/api/learning-workspaces/target/records' && init.method === 'GET') return Response.json({ records: linked });
    if (url === '/api/learning-workspaces/target/records' && init.method === 'POST') {
      if (fail) return Response.json({ error_code: 'GOAL_CHANGED', message: '大目标已修改，请重新确认。' }, { status: 409 });
      linked = [imported]; return Response.json(imported);
    }
    throw new Error(`Unexpected ${init.method} ${url}`);
  });
  vi.stubGlobal('fetch', fetch);
  return { fetch, scene };
}

it('confirms the chosen target goal and snapshot, then disables a repeated import without changing the reader or chat', async () => {
  const own = { ...record, session_id: 'target-chat', source_learning_workspace_id: 'target', title: '当前区聊天' };
  const foreign = { ...record, session_id: 'foreign', source_learning_workspace_id: 'other', source_goal: '学习深度学习', title: '另一区的讨论' };
  const { fetch, scene } = setup([record, own, foreign]);
  const w = mount(LearningRecordsPanel, { props: { target } });
  try {
    await flushPromises(); expect(w.text()).toContain('期末考试冲刺'); expect(w.text()).toContain('第 3 版'); expect(w.text()).toContain('学习深度学习'); expect(w.text()).not.toContain('当前区聊天');
    await w.get('input[value="old-chat"]').setValue(true);
    await w.findAll('button').find(button => button.text() === '确认带入 1 份记录')!.trigger('click'); await flushPromises();
    const writes = fetch.mock.calls.filter(([, init]) => init.method === 'POST'); expect(writes).toHaveLength(1);
    expect(JSON.parse(String(writes[0][1].body))).toEqual({ session_id: 'old-chat', through_seq: 12, goal_revision: 3 });
    expect(w.get('input[value="old-chat"]').attributes('disabled')).toBeDefined(); expect(w.text()).toContain('已带入');
    expect(w.findAll('button').find(button => button.text() === '确认带入 0 份记录')!.attributes('disabled')).toBeDefined();
    expect(network.value.workspace).toEqual(scene);
  } finally { w.unmount(); }
});

it('previews original conversations, reference-book sources and revision history without selecting a chat or making writes', async () => {
  const { fetch, scene } = setup(); const w = mount(LearningRecordsPanel, { props: { target }, attachTo: document.body });
  try {
    await flushPromises(); await w.findAll('button').find(button => button.text() === '查看原记录')!.trigger('click'); await flushPromises();
    const dialog = document.querySelector('[role="dialog"]')!;
    expect(dialog.textContent).toContain('分别计算每一层的变化率'); expect(dialog.textContent).toContain('参考书第一节'); expect(dialog.textContent).toContain('复合函数的变化率');
    expect(dialog.textContent).toContain('原来的解释'); expect(dialog.textContent).toContain('已理解复合变化'); expect(dialog.textContent).toContain('你的纠正：我理解了');
    expect(dialog.querySelector('textarea')).toBeNull(); expect(fetch.mock.calls.every(([, init]) => init.method === 'GET')).toBe(true); expect(network.value.workspace).toEqual(scene);
    (Array.from(dialog.querySelectorAll('button')).find(button => button.textContent === '关闭原记录') as HTMLButtonElement).click(); await flushPromises(); expect(document.querySelector('[role="dialog"]')).toBeNull();
  } finally { w.unmount(); }
});

it('keeps a failed selection available when the server reports that the goal changed', async () => {
  setup([record], true); const w = mount(LearningRecordsPanel, { props: { target } });
  try {
    await flushPromises(); await w.get('input').setValue(true); await w.findAll('button').find(button => button.text() === '确认带入 1 份记录')!.trigger('click'); await flushPromises();
    expect(w.get('[role="alert"]').text()).toContain('大目标已修改'); expect((w.get('input').element as HTMLInputElement).checked).toBe(true); expect(w.text()).not.toContain('已带入 1 份学习记录');
  } finally { w.unmount(); }
});

it('keeps the old-record entry readable before choosing any workspace', async () => {
  const { fetch } = setup(); const w = mount(LearningRecordsPanel);
  try { await flushPromises(); expect(w.text()).toContain('已有聊天与学习记录'); expect(w.find('input').exists()).toBe(false); expect(w.text()).toContain('查看原记录'); expect(fetch.mock.calls).toHaveLength(1); }
  finally { w.unmount(); }
});

it('allows the user to select a newer prefix while retaining the previous association', async () => {
  setup([{ ...record, through_seq: 18, turn_count: 3 }], false, [imported]); const w = mount(LearningRecordsPanel, { props: { target } });
  try { await flushPromises(); expect(w.text()).toContain('有新增记录'); expect(w.get('input').attributes('disabled')).toBeUndefined(); await w.get('input').setValue(true); expect(w.text()).toContain('确认带入 1 份记录'); }
  finally { w.unmount(); }
});

it('shows the accepted historical goal and this turn’s explicit redirection without borrowing today’s target goal', async () => {
  setup([record], false, [], { ...detail, turns: [{ ...detail.turns[0], workspace_goal: { text: '理解深度学习基础', constraints: ['先补链式法则'], revision: 1, created_at: 1 },
    accepted_workspace_goal: { text: '三天内完成期末复习', constraints: ['只覆盖考试范围'], revision: 2, created_at: 2 } }] });
  const w = mount(LearningRecordsPanel, { props: { target }, attachTo: document.body });
  try {
    await flushPromises(); await w.findAll('button').find(button => button.text() === '查看原记录')!.trigger('click'); await flushPromises();
    const dialog = document.querySelector('[role="dialog"]')!;
    expect(dialog.querySelector('[aria-label="当时的大目标"]')?.textContent).toContain('理解深度学习基础');
    expect(dialog.querySelector('[aria-label="当时的大目标"]')?.textContent).toContain('先补链式法则');
    expect(dialog.querySelector('[aria-label="本回合确认的新目标"]')?.textContent).toContain('三天内完成期末复习');
    expect(dialog.querySelector('[aria-label="本回合确认的新目标"]')?.textContent).toContain('第 2 版');
    expect(dialog.textContent).not.toContain(target.goal.text);
  } finally { w.unmount(); }
});
