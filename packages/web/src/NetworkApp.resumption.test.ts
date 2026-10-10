// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installIdentity, network, workspaceStorageKey, type NetworkWorkspace } from './network-context';
vi.mock('./App.vue', () => ({ default: { name: 'App', methods: { beforeNoteLeave: async () => true },
  emits: ['show-library'], template: '<div class="reader"><button @click="$emit(\'show-library\')">打开书</button><textarea /></div>' } }));
import NetworkApp from './NetworkApp.vue';

const identity = { user_id: 'reader', csrf_token: 'csrf' };
let workspace: NetworkWorkspace, calls: string[], authorized: boolean, question: string | null;
let wrapper: ReturnType<typeof mount> | undefined;
const books = [{ published_book_ref: { book_id: '同一本书', publication_id: 'new' }, is_default: true },
  { published_book_ref: { book_id: '同一本书', publication_id: 'original' } },
  { published_book_ref: { book_id: '另一窗口的书', publication_id: 'other' } }];
beforeEach(() => {
  sessionStorage.clear(); calls = []; authorized = true; question = '为什么守恒？';
  installIdentity(identity);
  sessionStorage.setItem(workspaceStorageKey(), JSON.stringify({ workspace_id: 'this-window' }));
  workspace = { workspace_id: 'this-window', generation: 3, revision: 8, selected_chat: 'original-chat',
    published_book_ref: books[1].published_book_ref, reader: { viewport: { top_lid: '1.16' } } } as NetworkWorkspace;
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    calls.push(`${init.method} ${url}`);
    if (url === '/api/auth/me') return Response.json(identity);
    if (url === '/api/library') return Response.json({ books: authorized ? books : [books[2]] });
    if (!authorized) return Response.json({ error_code: 'NOT_FOUND', message: '材料不可用' }, { status: 404 });
    if (url === '/api/workspaces/this-window/resumption') return Response.json({ workspace_id: workspace.workspace_id,
      published_book_ref: workspace.published_book_ref, selected_chat: workspace.selected_chat,
      position_label: '段落 · 能量守恒', position_excerpt: '沿着原来的位置继续阅读。', last_question: question });
    if (url === '/api/workspaces/this-window') return Response.json(workspace);
    if (url === '/api/workspaces/this-window/attach') {
      const body = JSON.parse(String(init.body));
      expect(body).toMatchObject({ generation: 3, expected_revision: 8 });
      return Response.json(workspace);
    }
    throw new Error(`Unexpected ${init.method} ${url}`);
  }));
});
afterEach(() => {
  wrapper?.unmount(); wrapper = undefined; vi.unstubAllGlobals(); sessionStorage.clear();
  network.value = { ...network.value, enabled: false, identity: null, workspace: null };
});
async function start() {
  // A refreshed page starts without installed private state, retaining only its workspace pointer.
  network.value = { ...network.value, identity: null, workspace: null };
  wrapper = mount(NetworkApp, { global: { stubs: { BookCover: true } } });
  await flushPromises();
}
it('previews only this window, then restores the exact publication, chat and saved position on click', async () => {
  await start();
  expect(wrapper!.get('[aria-label="阅读接续"]').text()).toContain('同一本书 · 版本 2');
  expect(wrapper!.get('[aria-label="阅读接续"]').text()).toContain('为什么守恒？');
  expect(wrapper!.find('.reader').exists()).toBe(false);
  expect(calls).toEqual(['GET /api/auth/me', 'GET /api/library', 'GET /api/workspaces/this-window/resumption']);
  await wrapper!.get('.resumption-continue').trigger('click'); await flushPromises();
  expect(network.value.workspace).toEqual(workspace);
  expect(calls.slice(-2)).toEqual(['GET /api/workspaces/this-window', 'POST /api/workspaces/this-window/attach']);
  expect(wrapper!.find('.network-library').exists()).toBe(false);
  // Returning to the shelf leaves the reader and its unfinished text intact.
  const reader = wrapper!.getComponent({ name: 'App' }).vm;
  await wrapper!.get('textarea').setValue('未发送的想法');
  await wrapper!.get('.reader button').trigger('click'); await flushPromises();
  expect(wrapper!.getComponent({ name: 'App' }).vm).toBe(reader);
  await wrapper!.get('.resumption-continue').trigger('click'); await flushPromises();
  expect((wrapper!.get('textarea').element as HTMLTextAreaElement).value).toBe('未发送的想法');
});
it('does not invent a question and leaves the normal grid when no workspace was saved', async () => {
  question = null; await start();
  expect(wrapper!.find('.resumption-question').exists()).toBe(false);
  expect(wrapper!.text()).toContain('已保存位置');
  wrapper!.unmount(); wrapper = undefined; sessionStorage.clear(); calls = [];
  await start();
  expect(wrapper!.find('[aria-label="阅读接续"]').exists()).toBe(false);
  expect(wrapper!.findAll('.network-book-card')).toHaveLength(3);
  expect(calls.some(call => call.includes('/workspaces/'))).toBe(false);
});
it('removes an unavailable continuation on recheck and does not attach after access is revoked', async () => {
  await start(); authorized = false;
  await wrapper!.get('.resumption-continue').trigger('click'); await flushPromises();
  expect(wrapper!.find('.resumption-continue').exists()).toBe(false);
  expect(wrapper!.text()).toContain('原阅读材料或现场已不可用');
  expect(calls.some(call => call.includes('/attach'))).toBe(false);
  expect(network.value.workspace).toBeNull();
});
it('refreshes authorization while the shelf is visible and clears the old card on identity change', async () => {
  await start(); authorized = false;
  window.dispatchEvent(new Event('focus')); await flushPromises();
  expect(wrapper!.find('.resumption-continue').exists()).toBe(false);
  expect(wrapper!.findAll('.network-book-card')).toHaveLength(1);
  installIdentity({ user_id: 'other-reader', csrf_token: 'other' }); await flushPromises();
  expect(wrapper!.text()).not.toContain('为什么守恒？');
});
it('refreshes a changed publication instead of attaching a different scene behind the visible card', async () => {
  await start();
  workspace = { ...workspace, published_book_ref: books[0].published_book_ref };
  await wrapper!.get('.resumption-continue').trigger('click'); await flushPromises();
  expect(network.value.workspace).toBeNull();
  expect(calls.some(call => call.includes('/attach'))).toBe(false);
  expect(wrapper!.get('[aria-label="阅读接续"]').text()).toContain('版本 1');
  expect(wrapper!.get('[role="alert"]').text()).toContain('接续现场已更新');
});
