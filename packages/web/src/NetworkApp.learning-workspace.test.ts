// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { installIdentity, installWorkspace, network, type LearningWorkspace, type NetworkWorkspace } from './network-context';
import { api } from './api';
import { learningDraftKey, readLearningDraft, saveLearningDraft } from './learning-workspace-state';
vi.mock('./App.vue', () => ({ default: { name: 'App', props: ['chatDraft'], emits: ['update:chatDraft', 'show-library'],
  methods: { beforeNoteLeave: async () => true },
  template: `<div class="test-reader"><textarea aria-label="问题草稿" :value="chatDraft?.message" @input="$emit('update:chatDraft', { message: $event.target.value, quote: null, goalId: null })" /><button @click="$emit('show-library')">资料</button></div>`,
} }));
import NetworkApp from './NetworkApp.vue';

afterEach(() => { vi.unstubAllGlobals(); sessionStorage.clear(); network.value = { ...network.value, enabled: false, identity: null, workspace: null }; });

it('creates empty goal workspaces, manages exact materials and restores each workspace draft through switches and refresh', async () => {
  installIdentity({ user_id: 'reader', csrf_token: 'csrf' });
  const references = [{ book_id: 'one', publication_id: 'p1' }, { book_id: 'two', publication_id: 'p2' }];
  const spaces: LearningWorkspace[] = [];
  let scene: NetworkWorkspace | null = null;
  const requests: { url: string; body: any }[] = [];
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    const body = init.body ? JSON.parse(String(init.body)) : null; requests.push({ url, body });
    if (url === '/api/auth/me') return Response.json(network.value.identity);
    if (url === '/api/videos') return Response.json({ videos: [] });
    if (url === '/api/library') return Response.json({ books: references.map((published_book_ref, index) => ({ published_book_ref, title: `资料${index + 1}` })) });
    if (url === '/api/learning-workspaces' && init.method === 'GET') return Response.json({ workspaces: spaces });
    if (url === '/api/learning-workspaces') {
      const value = { learning_workspace_id: `scope-${spaces.length}`, goal: { ...body.goal, revision: 1, created_at: 1 }, goal_history: [], members: [], archived: false, created_at: 1, updated_at: 1 };
      spaces.push(value); return Response.json(value);
    }
    if (url.includes('/learning-workspaces/')) {
      const value = spaces.find(space => url.includes(space.learning_workspace_id))!;
      if (url.endsWith('/shared-release')) return Response.json({ instance: null });
      if (url.endsWith('/goal')) value.goal = { ...body.goal, revision: value.goal.revision + 1, created_at: 2 };
      if (url.endsWith('/archive')) value.archived = body.archived;
      if (url.endsWith('/members')) value.members = init.method === 'DELETE'
        ? value.members.filter(item => item.published_book_ref.publication_id !== body.published_book_ref.publication_id)
        : [...value.members, { published_book_ref: body.published_book_ref, readable: true, added_at: 1 }];
      return Response.json(value);
    }
    if (url === '/api/workspaces' || url.endsWith('/book/open')) {
      const scope = spaces.find(space => space.learning_workspace_id === body.learning_workspace_id)!;
      scene = { workspace_id: 'reader-scene', generation: (scene?.generation ?? 0) + 1, revision: 1, reader: {} as any,
        published_book_ref: body.published_book_ref, learning_workspace_id: scope.learning_workspace_id, learning_workspace_goal: scope.goal,
        learning_workspace_positions: { ...scene?.learning_workspace_positions, [scope.learning_workspace_id]: body.published_book_ref }, selected_chat: `chat-${scope.learning_workspace_id}` };
      return Response.json(scene);
    }
    if (url.endsWith('/resumption')) return Response.json({ ...scene, last_question: null });
    if (url.startsWith('/api/workspaces/')) return Response.json(scene);
    throw new Error(url);
  }));
  let wrapper = mount(NetworkApp, { global: { stubs: { BookCover: true } } });
  const flush = () => flushPromises();
  const clickText = async (selector: string, text: string) => {
    const button = wrapper.findAll(selector).find(item => item.text() === text); expect(button).toBeTruthy(); await button!.trigger('click'); await flush();
  };
  try {
    await flush();
    await wrapper.get('.learning-create input').setValue('学习深度学习'); await wrapper.get('.learning-create').trigger('submit'); await flush();
    expect(wrapper.get('.learning-empty').text()).toContain('工作区已创建'); expect(scene).toBeNull();
    await clickText('.learning-add-book button', '加入工作区'); await clickText('.learning-add-book button', '加入工作区');
    expect(wrapper.findAll('.learning-member')).toHaveLength(2);
    await clickText('.learning-member button', '开始阅读');
    expect(wrapper.get('[aria-label="工作区大目标"]').text()).toContain('学习深度学习');
    const originalGoal = spaces[0].goal;
    spaces[0].goal = { ...originalGoal, revision: 2, text: '深度学习与链式法则' };
    window.dispatchEvent(new CustomEvent('learning-workspace-goal-changed', { detail: { learning_workspace_id: 'scope-other' } })); await flush();
    expect(wrapper.get('[aria-label="工作区大目标"]').text()).toContain('学习深度学习');
    window.dispatchEvent(new CustomEvent('learning-workspace-goal-changed', { detail: { learning_workspace_id: 'scope-0' } })); await flush();
    expect(wrapper.get('[aria-label="工作区大目标"]').text()).toContain('深度学习与链式法则');
    await wrapper.get('.test-reader textarea').setValue('空间 A 的问题');
    await clickText('.workspace-goal-bar button', '工作区与资料');
    await clickText('.learning-member button', '开始阅读');
    expect(network.value.workspace?.selected_chat).toBe('chat-scope-0'); expect(network.value.workspace?.published_book_ref).toEqual(references[1]);
    expect((wrapper.get('.test-reader textarea').element as HTMLTextAreaElement).value).toBe('空间 A 的问题');
    await clickText('.workspace-goal-bar button', '工作区与资料');
    await wrapper.get('.learning-create input').setValue('期末考试冲刺'); await wrapper.get('.learning-create').trigger('submit'); await flush();
    await clickText('.learning-add-book button', '加入工作区'); await clickText('.learning-member button', '开始阅读');
    await wrapper.get('.test-reader textarea').setValue('空间 B 的问题');
    await clickText('.workspace-goal-bar button', '工作区与资料');
    const a = wrapper.findAll('.learning-space-card').find(card => card.text().includes('深度学习与链式法则'))!;
    await a.findAll('button').find(button => button.text() === '打开工作区')!.trigger('click'); await flush();
    expect(network.value.workspace?.published_book_ref).toEqual(references[1]);
    await clickText('.workspace-goal-bar button', '工作区与资料'); await clickText('.learning-member button', '开始阅读');
    expect((wrapper.get('.test-reader textarea').element as HTMLTextAreaElement).value).toBe('空间 A 的问题');
    wrapper.unmount(); network.value = { ...network.value, workspace: null }; wrapper = mount(NetworkApp, { global: { stubs: { BookCover: true } } }); await flush();
    await clickText('.learning-space-card button', '打开工作区');
    expect((wrapper.get('.test-reader textarea').element as HTMLTextAreaElement).value).toBe('空间 A 的问题');
    await clickText('.workspace-goal-bar button', '工作区与资料');
    await clickText('.learning-details button', '修改大目标'); await wrapper.get('.learning-goal-edit textarea').setValue('三天内复习深度学习'); await wrapper.get('.learning-goal-edit').trigger('submit'); await flush();
    expect(wrapper.get('.learning-details').text()).toContain('第 3 版'); expect(spaces[1].goal.text).toBe('期末考试冲刺');
    await clickText('.learning-member button', '移出资料'); expect(wrapper.findAll('.learning-member')).toHaveLength(1);
    await clickText('.learning-archive button', '归档工作区'); expect(wrapper.get('.learning-archive').text()).toContain('工作区已归档');
    expect(wrapper.get('.learning-member button').attributes('disabled')).toBeDefined(); expect(spaces[0].goal.text).toBe('三天内复习深度学习');
    await clickText('.learning-archive button', '恢复工作区'); expect(wrapper.get('.learning-member button').attributes('disabled')).toBeUndefined();
    const bookRequests = requests.filter(item => item.url.endsWith('/book/open'));
    expect(bookRequests.every(item => item.body.learning_workspace_id)).toBe(true);
  } finally { wrapper.unmount(); }
});

it('scopes Tutor calls in main and linked windows and keeps material quotations with their originating workspace', async () => {
  installIdentity({ user_id: 'reader', csrf_token: 'csrf' });
  const scene = { workspace_id: 'scene', generation: 1, revision: 1, selected_chat: 'chat', published_book_ref: { book_id: 'book', publication_id: 'publication' }, reader: {}, learning_workspace_id: 'A' } as NetworkWorkspace;
  const fetch = vi.fn(async (_url: string, _init: RequestInit) => Response.json({ control: { enabled: false, revision: 0 }, sessions: {} })); vi.stubGlobal('fetch', fetch);
  installWorkspace(scene); const keyA = learningDraftKey(); saveLearningDraft(keyA, { message: '原问题', quote: null, goalId: null });
  await api.tutorState(); expect(fetch.mock.calls[0][0]).toBe('/api/tutor/state?learning_workspace_id=A');
  installWorkspace({ ...scene, learning_workspace_id: 'B' }, 'linked', true);
  expect(readLearningDraft(learningDraftKey())).toBeNull(); await api.tutorState(); expect(fetch.mock.calls[1][0]).toBe('/api/tutor/state?learning_workspace_id=B');
  await api.tutorMutate({ operation_id: 'enable', expected_revision: 0, action: { kind: 'set_enabled', enabled: true } });
  expect(fetch.mock.calls[2][0]).toBe('/api/tutor/mutate?learning_workspace_id=B');
  expect(readLearningDraft(keyA)?.message).toBe('原问题');
});

it('carries plain drafts across materials but restores quoted drafts only with their original publication', () => {
  installIdentity({ user_id: 'reader', csrf_token: 'csrf' });
  const scene = { workspace_id: 'scene', generation: 1, revision: 1, selected_chat: 'chat', published_book_ref: { book_id: 'one', publication_id: 'p1' }, reader: {}, learning_workspace_id: 'A' } as NetworkWorkspace;
  installWorkspace(scene); const first = learningDraftKey();
  saveLearningDraft(first, { message: '比较这两本书', quote: null, goalId: null });
  installWorkspace({ ...scene, published_book_ref: { book_id: 'two', publication_id: 'p2' } });
  expect(readLearningDraft(learningDraftKey())?.message).toBe('比较这两本书');
  const quoted = { message: '解释这一段', quote: { lid: '1.1', quote: '第一本的原文' }, goalId: null };
  saveLearningDraft(first, quoted);
  expect(readLearningDraft(learningDraftKey())).toBeNull();
  installWorkspace(scene); expect(readLearningDraft(learningDraftKey())).toEqual(quoted);
});

it('moves the current reader to the selected shared material version after updating while keeping workspace management open', async () => {
  installIdentity({ user_id: 'reader', csrf_token: 'csrf' });
  const old = { book_id: 'one', publication_id: 'v1' }, next = { book_id: 'one', publication_id: 'v2' }, personal = { book_id: 'notes', publication_id: 'notes' };
  const value: LearningWorkspace = { learning_workspace_id: 'shared-personal', goal: { text: '我的期末目标', constraints: [], revision: 1, created_at: 1 }, goal_history: [], members: [old, personal].map(published_book_ref => ({ published_book_ref, readable: true, added_at: 1 })), archived: false, created_at: 1, updated_at: 1 };
  const updated = { ...value, members: [personal, next].map(published_book_ref => ({ published_book_ref, readable: true, added_at: 1 })) };
  let scene: NetworkWorkspace | null = null;
  const opens: any[] = [];
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    const body = init.body ? JSON.parse(String(init.body)) : null;
    if (url === '/api/auth/me') return Response.json(network.value.identity);
    if (url === '/api/videos') return Response.json({ videos: [] });
    if (url === '/api/library') return Response.json({ books: [old, next, personal].map(published_book_ref => ({ published_book_ref, title: published_book_ref.book_id })) });
    if (url === '/api/learning-workspaces') return Response.json({ workspaces: [value] });
    if (url === '/api/workspaces' || url.endsWith('/book/open')) {
      opens.push(body); scene = { workspace_id: 'reader-scene', generation: (scene?.generation ?? 0) + 1, revision: 1, reader: {} as any, published_book_ref: body.published_book_ref, learning_workspace_id: value.learning_workspace_id, learning_workspace_goal: value.goal, selected_chat: 'private-chat' }; return Response.json(scene);
    }
    if (url.endsWith('/resumption')) return Response.json({ ...scene, last_question: null });
    if (url.startsWith('/api/workspaces/')) return Response.json(scene);
    throw new Error(url);
  }));
  const wrapper = mount(NetworkApp, { global: { stubs: { BookCover: true, SharedWorkspaceInstance: { props: ['workspace', 'books'], emits: ['updated'], data: () => ({ updated }), template: '<button class="apply-shared-update" @click="$emit(\'updated\', updated)">选择更新到此版本</button>' } } } });
  try {
    await flushPromises(); await wrapper.get('.learning-space-card button').trigger('click'); await flushPromises();
    await wrapper.get('.learning-member button').trigger('click'); await flushPromises();
    expect(network.value.workspace?.published_book_ref).toEqual(old);
    await wrapper.get('.workspace-goal-bar button').trigger('click'); await flushPromises();
    await wrapper.get('.apply-shared-update').trigger('click'); await flushPromises();
    expect(network.value.workspace?.published_book_ref).toEqual(next); expect(network.value.workspace?.selected_chat).toBe('private-chat');
    expect(opens.at(-1)).toMatchObject({ published_book_ref: next, learning_workspace_id: value.learning_workspace_id });
    expect(wrapper.get('[aria-label="当前工作区"]').isVisible()).toBe(true); expect(wrapper.findAll('.learning-member')).toHaveLength(2);
  } finally { wrapper.unmount(); }
});
