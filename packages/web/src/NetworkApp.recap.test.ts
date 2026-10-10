// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { installIdentity, installWorkspace, network, type NetworkWorkspace } from './network-context';
import type { RecapTarget } from './session-recap';
vi.mock('./App.vue', () => ({ default: { name: 'App', methods: { beforeNoteLeave: async () => true }, props: ['recapTarget'], emits: ['recap-publication', 'recap-consumed'], template: '<div class="reader-app" />' } }));
import NetworkApp from './NetworkApp.vue';

afterEach(() => { vi.unstubAllGlobals(); network.value = { ...network.value, enabled: false, identity: null, workspace: null }; sessionStorage.clear(); });
it('JL9 opens the exact original publication and chat before mounting the destination reader', async () => {
  const identity = { user_id: 'A', csrf_token: 'csrf' };
  let workspace = { workspace_id: 'w', generation: 2, revision: 3, selected_chat: 'chat', published_book_ref: { book_id: 'book', publication_id: 'new' }, reader: {} } as NetworkWorkspace;
  installIdentity(identity); installWorkspace(workspace);
  const target: RecapTarget = { kind: 'source', session_id: 'chat', turn_id: 'turn', through_seq: 9,
    source: { source_ref_id: 'source', label: '原文', quote: '引文', evidence: [{ turn_id: 'turn', event_seq: 7 }], unavailable_reason: null, published_book_ref: { book_id: 'book', publication_id: 'original' } } };
  const writes: { path: string; body: Record<string, unknown> }[] = [];
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    const json = (data: unknown) => new Response(JSON.stringify(data), { status: 200 });
    if (url === '/api/auth/me') return json(identity);
    if (url === '/api/workspaces/w') return json(workspace);
    if (url === '/api/workspaces/w/attach') return json(workspace);
    if (url.endsWith('/book/open')) {
      const body = JSON.parse(String(init.body)); writes.push({ path: url, body });
      workspace = { ...workspace, generation: workspace.generation + 1, revision: workspace.revision + 1, selected_chat: null, published_book_ref: body.published_book_ref };
      return json(workspace);
    }
    if (url.endsWith('/chat/select')) {
      const body = JSON.parse(String(init.body)); writes.push({ path: url, body });
      workspace = { ...workspace, revision: workspace.revision + 1, selected_chat: body.session_id };
      return json(workspace);
    }
    if (url.endsWith('/chat/history')) return json({ ...workspace, result: { active_session_id: 'chat', current: { id: 'chat', turns: [] }, sessions: [] } });
    throw new Error(`Unexpected request: ${url}`);
  }));
  const wrapper = mount(NetworkApp, { global: { stubs: { AgentPresentation: true } } });
  try {
    await flushPromises();
    wrapper.getComponent({ name: 'App' }).vm.$emit('recap-publication', target);
    await flushPromises();
    expect(writes.map(w => w.path)).toEqual(['/api/workspaces/w/book/open', '/api/workspaces/w/chat/select']);
    expect(writes[0].body.published_book_ref).toEqual(target.source.published_book_ref);
    expect(writes[1].body).toMatchObject({ session_id: 'chat', generation: 3, expected_revision: 4 });
    expect(wrapper.getComponent({ name: 'App' }).props('recapTarget')).toEqual(target);
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    const returnTarget: RecapTarget = { ...target, source: { ...target.source, published_book_ref: { book_id: 'book', publication_id: 'new' } } };
    wrapper.getComponent({ name: 'App' }).vm.$emit('recap-consumed');
    wrapper.getComponent({ name: 'App' }).vm.$emit('recap-publication', returnTarget);
    await flushPromises();
    expect(writes.slice(2).map(w => w.path)).toEqual(['/api/workspaces/w/book/open', '/api/workspaces/w/chat/select']);
    expect(writes[2].body.published_book_ref).toEqual(returnTarget.source.published_book_ref);
    expect(writes[3].body).toMatchObject({ session_id: 'chat', generation: 4, expected_revision: 6 });
    expect(wrapper.getComponent({ name: 'App' }).props('recapTarget')).toEqual(returnTarget);
  } finally { wrapper.unmount(); }
});
