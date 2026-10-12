// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installIdentity, network, type NetworkWorkspace } from './network-context';
vi.mock('./App.vue', () => ({ default: { name: 'App', props: ['mcpSource'],
  methods: { beforeNoteLeave: async () => true }, emits: ['mcp-source-consumed'],
  template: '<div class="reader"><button @click="$emit(\'mcp-source-consumed\')">完成定位</button></div>' } }));
import NetworkApp from './NetworkApp.vue';
let wrapper: ReturnType<typeof mount>, signedIn: boolean, authorized: boolean;
let calls: string[];
const identity = { user_id: 'alice', csrf_token: 'csrf' };
const publication = { book_id: 'book', publication_id: 'original' };
const sourceLink = '/?book_id=book&publication_id=original&start_lid=1.1&end_lid=1.2&start_utf16=8000&end_utf16=9000';
beforeEach(() => {
  signedIn = false; authorized = true; calls = []; sessionStorage.clear();
  network.value = { ...network.value, workspace: null, identity: null, enabled: false };
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    calls.push(url);
    if (url === '/api/auth/me') return signedIn ? Response.json(identity) : Response.json({}, { status: 401 });
    if (url === '/api/auth/login') { signedIn = true; return Response.json(identity); }
    if (url === '/api/library') return Response.json({ books: [] });
    if (url === '/api/workspaces') {
      expect(JSON.parse(String(init.body)).published_book_ref).toEqual(publication);
      if (!authorized) return Response.json({ error_code: 'NOT_FOUND' }, { status: 404 });
      return Response.json({ workspace_id: 'source', generation: 1, revision: 0, selected_chat: null,
        published_book_ref: publication, reader: {} } as NetworkWorkspace);
    }
    if (url === '/api/account/mcp') return Response.json({ credentials: [], mcp_url: 'https://reader.example/mcp',
      usage: { free_remaining: 1000, free_used: 0, free_granted: 1000, paid_remaining: 0,
        free_resets_at: '2026-11-01T00:00:00+08:00', call_packs: [] } });
    throw new Error(url);
  }));
});
afterEach(() => { wrapper?.unmount(); vi.unstubAllGlobals(); sessionStorage.clear(); history.replaceState({}, '', '/'); network.value = { ...network.value, workspace: null, identity: null, enabled: false }; });
async function start(url: string) { history.replaceState({}, '', url); wrapper = mount(NetworkApp); await flushPromises(); }
async function login() {
  await wrapper.get('input[autocomplete="username"]').setValue('alice');
  await wrapper.get('input[type="password"]').setValue('password-fixture');
  await wrapper.get('form').trigger('submit'); await flushPromises();
}
it('restores the exact source after login, including a non-default version and second-page coordinates', async () => {
  await start(sourceLink); expect(calls).toEqual(['/api/auth/me']);
  expect(location.search).toContain('start_utf16=8000');
  await login();
  expect(network.value.workspace?.published_book_ref).toEqual(publication);
  expect(wrapper.find('.network-library').exists()).toBe(false);
  expect(wrapper.getComponent({ name: 'App' }).props('mcpSource')).toEqual({ published_book_ref: publication,
    start_lid: '1.1', end_lid: '1.2', start_utf16: 8000, end_utf16: 9000 });
  await wrapper.get('.reader button').trigger('click'); expect(location.search).toBe('');
});
it('opens an already signed-in source and reports unauthorized material without opening another version', async () => {
  signedIn = true; authorized = false;
  await start(sourceLink);
  expect(wrapper.text()).toContain('未获授权或该版本已不可用');
  expect(network.value.workspace).toBeNull(); expect(wrapper.find('.reader').exists()).toBe(false);
  expect(location.search).toContain('publication_id=original');
  authorized = true;
  await wrapper.findAll('button').find(b => b.text() === '重试打开来源')!.trigger('click'); await flushPromises();
  expect(network.value.workspace?.published_book_ref).toEqual(publication);
});
it('opens the purchase destination after login even for an account with no materials, then clears it on account change', async () => {
  await start('/?account=mcp'); await login();
  expect(wrapper.get('[aria-labelledby="account-mcp-title"]').text()).toContain('免费剩余 1000');
  expect(network.value.workspace).toBeNull(); expect(calls).toContain('/api/account/mcp');
  expect(calls).not.toContain('/api/workspaces');
  installIdentity({ user_id: 'bob', csrf_token: 'other' }); await flushPromises();
  expect(wrapper.find('[aria-labelledby="account-mcp-title"]').exists()).toBe(false);
});
it('keeps malformed source links visible as errors without creating a workspace', async () => {
  signedIn = true; await start('/?book_id=book');
  expect(wrapper.text()).toContain('来源链接不完整'); expect(calls).not.toContain('/api/workspaces');
});
