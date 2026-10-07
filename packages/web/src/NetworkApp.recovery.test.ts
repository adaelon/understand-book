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
  template: '<textarea :value="chatDraft?.message" @input="$emit(\'update:chatDraft\', { message: $event.target.value, quote: null, goalId: null })" />',
}) }));
import NetworkApp from './NetworkApp.vue';

afterEach(() => {
  vi.unstubAllGlobals(); vi.restoreAllMocks(); sessionStorage.clear();
  network.value = { ...network.value, enabled: false, identity: null, workspace: null };
  observation.errors = []; observation.mounts = 0;
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
