// @vitest-environment happy-dom
import { defineComponent, h } from 'vue';
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { api } from './api';
import { installIdentity, installWorkspace, network, readerKey, type NetworkWorkspace } from './network-context';
import { useTutorControl } from './useTutorControl';
import ConversationLearningGoal from './components/ConversationLearningGoal.vue';
import TutorPanel from './components/TutorPanel.vue';
import type { TutorSession } from './generated/TutorSession';
import type { TutorState } from './generated/TutorState';

const session = (id: string, source: string, goal: string): TutorSession => ({
  id, revision: 1, status: 'paused', user_intent: goal, explicit_constraints: [],
  current_focus: { interpretation: '', target_object_refs: [], capability_targets: [] },
  material_scope: [{ source_id: source, scope_refs: [], role: 'primary' }],
  default_teaching_intent: null, path_instance_ref: null, progress_ref: null, workspace_goal_revision: null,
});
const goals = {
  a: session('a', 'book-a', '解释计算图'), b: session('b', 'book-b', '理解模型训练'),
  workspaceA: session('workspace-a', 'book-a', '定位推理任务瓶颈'),
  workspaceB: session('workspace-b', 'book-b', '核算训练资源'),
  shared: { ...session('shared', 'book-a', '比较计算图和训练资源'), material_scope: [
    { source_id: 'book-a', scope_refs: [], role: 'primary' as const },
    { source_id: 'book-b', scope_refs: [], role: 'primary' as const },
  ] },
};
const currentByScope: Record<string, string | null> = {
  ':book-a': 'a', ':book-b': 'b', ':book-empty': null,
  'workspace:book-a': 'workspace-a', 'workspace:book-b': 'workspace-b',
  'shared:book-a': 'shared', 'shared:book-b': 'shared',
};
const sessions = Object.fromEntries(Object.values(goals).map(value => [value.id, value]));
let generation = 0;
function open(source: string, workspace: string | null = null) {
  installWorkspace({ workspace_id: 'reader', generation: ++generation, revision: generation,
    learning_workspace_id: workspace, selected_chat: null,
    published_book_ref: { book_id: source, publication_id: `publication-${source}` }, reader: {},
  } as NetworkWorkspace);
}
const json = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status });

beforeEach(() => {
  sessionStorage.clear(); localStorage.clear(); generation = 0;
  installIdentity({ user_id: 'reader', csrf_token: 'csrf' }); open('book-a');
});
afterEach(() => { vi.unstubAllGlobals(); network.value = { ...network.value, enabled: false }; });

it('restores each book and workspace goal in both entry and details without starting teaching', async () => {
  const fetch = vi.fn(async (url: string, init: RequestInit) => {
    const parsed = new URL(url, 'https://example.test');
    if (parsed.pathname.endsWith('/tutor_readiness')) return json({ status: 'ready', source_id: network.value.workspace!.published_book_ref.book_id,
      source_revision: 'r1', limitations: [], reason: '可以学习', teaching_assets: { status: 'ready', teaching_map_revision: 'r1', limitations: [], reason: '' } });
    expect(init.method).toBe('GET'); expect(parsed.pathname).toBe('/api/tutor/state');
    const scope = `${parsed.searchParams.get('learning_workspace_id') ?? ''}:${parsed.searchParams.get('source_id')}`;
    expect(scope in currentByScope).toBe(true);
    return json({ workspace_goal_revision: null, control: { enabled: true, revision: 5, current_tutor_session_id: currentByScope[scope] }, sessions } satisfies TutorState);
  });
  vi.stubGlobal('fetch', fetch);
  const Reader = defineComponent({ setup() {
    const tutor = useTutorControl(); void tutor.load();
    return () => h('div', [h(ConversationLearningGoal, {
      session: tutor.current.value, enabled: !!tutor.state.value?.control.enabled,
      available: !!tutor.state.value, busy: tutor.busy.value, ready: true, error: tutor.error.value,
      workspaceScoped: !!network.value.workspace?.learning_workspace_id,
    }), h(TutorPanel, { state: tutor.state.value, busy: tutor.busy.value, error: tutor.error.value, pending: !!tutor.pending.value,
      sourceId: network.value.workspace?.published_book_ref.book_id, label: tutor.label.value,
      readiness: tutor.readiness.value, initialView: 'current',
    })]);
  } });
  const w = mount(defineComponent({ setup: () => () => h(Reader, { key: readerKey() }) }),
    { global: { stubs: { Teleport: true, UnderstandingSpace: true } } });
  try {
    await flushPromises();
    expect(w.get('.goal-text').text()).toBe('解释计算图');
    await w.get('[aria-label="调整目标"]').trigger('click');
    await w.get('.goal-editor-overlay input').setValue('尚未保存的另一个目标');
    for (const [source, workspace, goal] of [
      ['book-b', null, '理解模型训练'], ['book-empty', null, null], ['book-a', null, '解释计算图'],
      ['book-a', 'workspace', '定位推理任务瓶颈'], ['book-b', 'workspace', '核算训练资源'],
      ['book-a', 'shared', '比较计算图和训练资源'], ['book-b', 'shared', '比较计算图和训练资源'],
      ['book-a', 'workspace', '定位推理任务瓶颈'], ['book-a', null, '解释计算图'],
    ] as const) {
      open(source, workspace); await flushPromises();
      expect(w.find('.goal-editor-overlay').exists()).toBe(false);
      if (goal) {
        expect(w.get('.goal-text').text()).toBe(goal);
        expect(w.get('.session-goal h3').text()).toBe(goal);
        expect(w.get('.goal-resume').text()).toContain('已暂停');
      } else {
        expect(w.find('.goal-text').exists()).toBe(false);
        expect(w.get('.goal-empty').text()).toContain('这本书还没有学习目标');
        expect(w.find('.session-goal').exists()).toBe(false);
      }
    }
    expect(fetch.mock.calls.every(([, init]) => init.method === 'GET')).toBe(true);
  } finally { w.unmount(); }
});

it('sends goal saves to the same source and workspace as reads', async () => {
  open('书籍 a', '工作区 a');
  const fetch = vi.fn(async () => json({ workspace_goal_revision: null, control: { enabled: true, revision: 1, current_tutor_session_id: null }, sessions: {} }));
  vi.stubGlobal('fetch', fetch);
  const mutation = { operation_id: 'save-goal', expected_revision: 0, action: { kind: 'start' as const, user_intent: '理解架构',
    explicit_constraints: [], material_scope: [{ source_id: '书籍 a', scope_refs: [], role: 'primary' as const }], default_teaching_intent: null } };
  await api.tutorState(); await api.tutorMutate(mutation);
  for (const call of vi.mocked(globalThis.fetch).mock.calls) {
    const url = new URL(String(call[0]), 'https://example.test');
    expect(url.searchParams.get('learning_workspace_id')).toBe('工作区 a');
    expect(url.searchParams.get('source_id')).toBe('书籍 a');
  }
  expect(JSON.parse(String(vi.mocked(globalThis.fetch).mock.calls[1][1]?.body))).toEqual(mutation);
});
