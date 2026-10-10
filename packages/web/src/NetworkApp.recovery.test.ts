// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { defineComponent, onMounted } from 'vue';
import { afterEach, expect, it, vi } from 'vitest';
import { api } from './api';
import { installIdentity, installWorkspace, network, type NetworkWorkspace } from './network-context';
const observation = vi.hoisted(() => ({ errors: [] as unknown[], mounts: 0 }));
vi.mock('./App.vue', () => ({ default: defineComponent({
  name: 'App', props: ['chatDraft'], emits: ['update:chatDraft'],
  setup() {
    onMounted(() => { observation.mounts++; void api.agentHistory().catch(e => observation.errors.push(e)); });
  },
  template: '<div><slot name="account" /><textarea :value="chatDraft?.message" @input="$emit(\'update:chatDraft\', { message: $event.target.value, quote: null, goalId: null })" /></div>',
}) }));
import NetworkApp from './NetworkApp.vue';
import { readerPreferenceOwner, workspaceStorageKey } from './network-context';

it('INV5 displays email after login while preferences and workspace storage retain the server user ID', async () => {
  let signedIn = false;
  const identity = { user_id: 'legacy', email: 'reader@example.com', csrf_token: 'csrf' };
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    if (url === '/api/auth/login') {
      expect(JSON.parse(String(init.body))).toEqual({ username: ' Reader@Example.com ', password: 'fixture-only-password' });
      signedIn = true; return Response.json(identity);
    }
    if (url === '/api/auth/me') return signedIn ? Response.json(identity) : Response.json({}, { status: 401 });
    if (url === '/api/library') return Response.json({ books: [] });
    throw new Error(url);
  }));
  const wrapper = mount(NetworkApp);
  try {
    await flushPromises();
    expect(wrapper.text()).toContain('邮箱或账号');
    await wrapper.get('input[autocomplete="username"]').setValue(' Reader@Example.com ');
    await wrapper.get('input[type="password"]').setValue('fixture-only-password');
    await wrapper.get('form').trigger('submit'); await flushPromises();
    expect(wrapper.text()).toContain('reader@example.com');
    expect(readerPreferenceOwner()).toBe('legacy');
    expect(workspaceStorageKey()).toBe('understand-book:workspace:legacy');
  } finally { wrapper.unmount(); }
});

afterEach(() => {
  vi.unstubAllGlobals(); vi.restoreAllMocks(); sessionStorage.clear();
  history.replaceState({}, '', '/');
  network.value = { ...network.value, enabled: false, identity: null, workspace: null };
  observation.errors = []; observation.mounts = 0;
});

it('INV8 opens a reset deep link over an existing identity, strips the token and clears identity on success', async () => {
  history.replaceState({}, '', '/?account=reset-password#token=reset-secret');
  installIdentity({ user_id: 'A', csrf_token: 'csrf' });
  const broadcast = vi.fn();
  vi.stubGlobal('BroadcastChannel', class { postMessage = broadcast; addEventListener() {} close() {} });
  const fetcher = vi.fn(async (url: string, init: RequestInit) => {
    expect(url).toBe('/api/auth/password/reset');
    expect(JSON.parse(String(init.body))).toEqual({ token: 'reset-secret', new_password: 'new-password-123' });
    return Response.json({ completed: true });
  });
  vi.stubGlobal('fetch', fetcher);
  const wrapper = mount(NetworkApp);
  try {
    await flushPromises();
    expect(wrapper.text()).toContain('设置新密码'); expect(location.hash).toBe(''); expect(fetcher).not.toHaveBeenCalled();
    for (const input of wrapper.findAll('input')) await input.setValue('new-password-123');
    await wrapper.get('form').trigger('submit'); await flushPromises();
    expect(network.value.identity).toBeNull(); expect(network.value.workspace).toBeNull();
    expect(wrapper.text()).toContain('密码已更新'); expect(wrapper.find('input[autocomplete="username"]').exists()).toBe(true);
    expect(broadcast).toHaveBeenCalledWith('changed'); expect(location.search).toBe('');
  } finally { wrapper.unmount(); }
});

it('shows only the page animation during identity recovery, then exposes the login form', async () => {
  let finish!: (response: Response) => void;
  vi.stubGlobal('fetch', vi.fn(() => new Promise<Response>(resolve => { finish = resolve; })));
  const wrapper = mount(NetworkApp);
  try {
    await flushPromises();
    expect(wrapper.get('.network-loading').text()).toBe('');
    expect(wrapper.get('.book-loading').attributes('aria-label')).toBe('正在加载');
    expect(wrapper.find('form').exists()).toBe(false);
    finish(Response.json({ error_code: 'UNAUTHENTICATED' }, { status: 401 }));
    await flushPromises();
    expect(wrapper.find('.network-loading').exists()).toBe(false);
    expect(wrapper.find('input[autocomplete="username"]').exists()).toBe(true);
  } finally { wrapper.unmount(); }
});

it('keeps the page animation until the initial authorized catalog arrives', async () => {
  let finish!: (response: Response) => void;
  vi.stubGlobal('fetch', vi.fn(async (url: string) => {
    if (url === '/api/auth/me') return Response.json({ user_id: 'A', csrf_token: 'csrf' });
    if (url === '/api/library') return new Promise<Response>(resolve => { finish = resolve; });
    throw new Error(`Unexpected request: ${url}`);
  }));
  const wrapper = mount(NetworkApp);
  try {
    await flushPromises();
    expect(wrapper.get('.network-loading').text()).toBe('');
    expect(wrapper.text()).not.toContain('暂无获授权的材料');
    finish(Response.json({ books: [] }));
    await flushPromises();
    expect(wrapper.find('.network-loading').exists()).toBe(false);
    expect(wrapper.find('.network-library').exists()).toBe(true);
  } finally { wrapper.unmount(); }
});

it('ADM8 leaves the Reader and draft mounted while viewing allowance, then clears the modal on session change', async () => {
  const identity = { user_id: 'A', csrf_token: 'csrf' };
  const workspace = { workspace_id: 'w', generation: 2, revision: 3, selected_chat: 'chat',
    published_book_ref: { book_id: 'b', publication_id: 'p' }, reader: {} } as NetworkWorkspace;
  installIdentity(identity); installWorkspace(workspace);
  const requests: string[] = [];
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    requests.push(`${init.method} ${url}`);
    if (url === '/api/auth/me') return Response.json(identity);
    if (url === '/api/workspaces/w' || url.endsWith('/attach')) return Response.json(workspace);
    if (url.endsWith('/chat/history')) return Response.json({ result: { active_session_id: 'chat', sessions: [], current: { id: 'chat', book_id: 'b', turns: [] } } });
    if (url.endsWith('/allowance')) return Response.json({ current_allowance: null });
    if (url.includes('/account/usage')) return Response.json({ items: [], total: 0, offset: 0, limit: 20 });
    throw new Error(url);
  }));
  const wrapper = mount(NetworkApp);
  try {
    await flushPromises();
    await wrapper.get('textarea').setValue('额度不足也要保留的草稿');
    const mounts = observation.mounts;
    requests.length = 0;
    await wrapper.findAll('button').find(b => b.text() === '使用额度')!.trigger('click'); await flushPromises();
    expect(wrapper.get('dialog').text()).toContain('当前没有有效额度期');
    installWorkspace({ ...workspace, revision: workspace.revision + 1 }); await flushPromises();
    expect(wrapper.find('dialog').exists()).toBe(true);
    await wrapper.findAll('button').find(b => b.text() === '返回阅读与对话')!.trigger('click');
    expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('额度不足也要保留的草稿');
    expect(observation.mounts).toBe(mounts);
    expect(requests).toEqual(['GET /api/account/allowance', 'GET /api/account/usage?limit=20&offset=0']);
    await wrapper.findAll('button').find(b => b.text() === '使用额度')!.trigger('click'); await flushPromises();
    installIdentity({ user_id: 'A', csrf_token: 'new-session' }); await flushPromises();
    expect(wrapper.find('dialog').exists()).toBe(false);
    expect(wrapper.find('textarea').exists()).toBe(false);
  } finally { wrapper.unmount(); }
});

it('recovers a hidden idle page before remounting history, preserves its draft, and submits no questions', async () => {
  const identity = { user_id: 'A', csrf_token: 'csrf' };
  let workspace = { workspace_id: 'w', generation: 2, revision: 3, selected_chat: 'chat',
    published_book_ref: { book_id: 'b', publication_id: 'p' }, reader: {} } as NetworkWorkspace;
  installIdentity(identity); installWorkspace(workspace);
  let attachment: string | null = network.value.attachment;
  const calls: string[] = [];
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    calls.push(`${init.method} ${url}`);
    const input = init.body ? JSON.parse(String(init.body)) : {};
    if (url === '/api/auth/me') return Response.json(identity);
    if (url === '/api/workspaces/w') return Response.json(workspace);
    if (url.endsWith('/attach')) {
      expect(input.generation).toBe(workspace.generation);
      if (attachment !== input.attachment_id) {
        attachment = input.attachment_id;
        workspace = { ...workspace, generation: workspace.generation + 1, revision: workspace.revision + 1 };
      }
      return Response.json(workspace);
    }
    if (url.endsWith('/chat/history')) {
      if (input.generation !== workspace.generation || input.attachment_id !== attachment)
        return Response.json({ error_code: 'WORKSPACE_STALE' }, { status: 409 });
      return Response.json({ result: { active_session_id: 'chat', sessions: [], current: { id: 'chat', book_id: 'b', turns: [] } } });
    }
    throw new Error(`Unexpected request: ${url}`);
  }));
  const wrapper = mount(NetworkApp, { global: { stubs: { AgentPresentation: true } } });
  try {
    await flushPromises();
    await wrapper.get('textarea').setValue('还没发出去的问题');
    const before = observation.mounts;
    workspace = { ...workspace, generation: workspace.generation + 1, revision: workspace.revision + 1 };
    attachment = null;
    calls.length = 0;
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
    document.dispatchEvent(new Event('visibilitychange'));
    await flushPromises();
    expect(calls).toEqual(['GET /api/auth/me', 'GET /api/workspaces/w', 'POST /api/workspaces/w/attach', 'POST /api/workspaces/w/chat/history']);
    expect(observation.mounts).toBe(before + 1);
    expect(observation.errors).toEqual([]);
    expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('还没发出去的问题');
    installWorkspace({ ...workspace, selected_chat: 'other' });
    await flushPromises();
    expect(wrapper.getComponent({ name: 'App' }).props('chatDraft')?.message).not.toBe('还没发出去的问题');
  } finally { wrapper.unmount(); }
});
