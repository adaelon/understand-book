// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { defineComponent, type ComponentPublicInstance } from 'vue';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installIdentity, network, type NetworkWorkspace } from './network-context';
import type { BoardSummary } from './generated/BoardSummary';

const opened = vi.hoisted(() => ({ boards: [] as { board_id: string; book_id: string; learning_workspace_id: string | null }[], allowLeave: true }));
vi.mock('./App.vue', () => ({ default: defineComponent({
  name: 'App', emits: ['board-visibility'],
  methods: {
    beforeNoteLeave: async () => opened.allowLeave,
    openBoardLocation(this: ComponentPublicInstance, target: { board_id: string }) {
      opened.boards.push({ ...target, book_id: network.value.workspace!.published_book_ref.book_id, learning_workspace_id: network.value.workspace!.learning_workspace_id ?? null });
      this.$emit('board-visibility', true);
    },
    closeBoard(this: ComponentPublicInstance) { this.$emit('board-visibility', false); },
  },
  template: '<div class="test-reader">Reading workspace</div>',
}) }));
import NetworkApp from './NetworkApp.vue';

const book = { book_id: 'book', publication_id: 'book-v1' }, video = { book_id: 'video', publication_id: 'video-v2' };
const identity = { user_id: 'board-reader', csrf_token: 'csrf' };
const board = (id: string, reference = book, learning: string | null = null): BoardSummary => ({
  board_id: id, learning_workspace_id: learning, ...reference, source_revision: null, title: id === 'book-board' ? '为什么需要缓存？' : '视频中的请求与响应',
  revision: 1, created_at: '2026-11-01T10:00:00Z', updated_at: '2026-11-02T09:00:00Z', item_count: 3,
});
let items: BoardSummary[], requests: { url: string; body: any }[], scene: NetworkWorkspace | null;
let failedList: boolean, wrapper: ReturnType<typeof mount> | undefined;
beforeEach(() => {
  sessionStorage.clear(); localStorage.clear(); history.replaceState({}, '', '/');
  installIdentity(identity); opened.boards = []; opened.allowLeave = true; requests = []; scene = null; failedList = false;
  items = [board('book-board'), board('video-board', video, 'video-learning')];
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit = {}) => {
    const body = init.body ? JSON.parse(String(init.body)) : null; requests.push({ url, body });
    if (url === '/api/auth/me') return Response.json(identity);
    if (url === '/api/account/presence') return Response.json({});
    if (url === '/api/library') return Response.json({ books: [{ published_book_ref: book, title: '网络基础' }, { published_book_ref: video, title: '网络课程', media_kind: 'video' }] });
    if (url === '/api/videos') return Response.json({ videos: [] });
    if (url === '/api/learning-workspaces') return Response.json({ workspaces: [{ learning_workspace_id: 'video-learning', goal: { text: '理解网络课程', revision: 1, constraints: [], created_at: 1 }, goal_history: [], members: [{ published_book_ref: video, readable: true, added_at: 1 }], archived: false, created_at: 1, updated_at: 1 }] });
    if (url === '/api/me/reading-resumption') return Response.json(null);
    if (url === '/api/me/boards') return failedList ? Response.json({ message: 'temporary failure' }, { status: 503 }) : Response.json({ items });
    if (url === '/api/workspaces' || url.endsWith('/book/open')) {
      scene = { workspace_id: 'window', generation: (scene?.generation ?? 0) + 1, revision: 1, reader: {} as any, published_book_ref: body.published_book_ref, learning_workspace_id: body.learning_workspace_id ?? null, selected_chat: null };
      return Response.json(scene);
    }
    if (url === '/api/workspaces/window' || url === '/api/workspaces/window/attach') return Response.json(scene);
    if (url === '/api/workspaces/window/boards/create') {
      const created = { ...board('created', scene!.published_book_ref), title: body.title };
      items.push(created); return Response.json({ ...scene, result: created });
    }
    throw new Error(`Unexpected ${url}`);
  }));
});
afterEach(() => {
  wrapper?.unmount(); wrapper = undefined; vi.unstubAllGlobals(); vi.restoreAllMocks();
  sessionStorage.clear(); localStorage.clear(); network.value = { ...network.value, enabled: false, identity: null, workspace: null };
});
async function start() { wrapper = mount(NetworkApp, { global: { stubs: { MaterialCover: true, YeyeMascot: true, PersonalMaterials: true } } }); await flushPromises(); return wrapper; }
async function home() { await wrapper!.get('.network-primary-nav [data-tour="boards-entry"]').trigger('click'); await flushPromises(); }

it('makes boards a primary destination before opening material and opens the correct video workspace', async () => {
  const view = await start();
  expect(view.get('.network-primary-nav').text()).toContain('学习画板');
  expect(network.value.workspace).toBeNull();
  await home();
  expect(view.findAll('.board-library-card')).toHaveLength(2);
  expect(requests.filter(item => item.url === '/api/workspaces')).toHaveLength(0);
  expect(location.search).toBe('?view=boards');
  await view.findAll('.board-library-card')[1].get('footer button').trigger('click'); await flushPromises();
  expect(opened.boards).toEqual([{ board_id: 'video-board', book_id: 'video', learning_workspace_id: 'video-learning' }]);
  expect(view.get('.network-primary-nav [data-tour="boards-entry"]').attributes('aria-current')).toBe('page');
  expect(view.find('.board-library').exists()).toBe(false);
  await home();
  expect(view.findAll('.board-library-card')).toHaveLength(2);
  expect(opened.boards).toHaveLength(1);
});

it('creates a board from the primary destination and opens the acknowledged board', async () => {
  const view = await start(); await home();
  await view.get('.board-library-heading button').trigger('click');
  await view.get('.board-library-create input').setValue('理解缓存命中');
  await view.get('.board-library-create').trigger('submit'); await flushPromises();
  expect(opened.boards).toEqual([{ board_id: 'created', book_id: 'book', learning_workspace_id: null }]);
  expect(requests.filter(item => item.url.endsWith('/boards/create'))).toHaveLength(1);
  expect(requests.find(item => item.url.endsWith('/boards/create'))!.body.title).toBe('理解缓存命中');
  await home(); expect(view.findAll('.board-library-card')).toHaveLength(3);
});

it('reopens the primary boards address without selecting a book and retries a failed list', async () => {
  history.replaceState({}, '', '/?view=boards'); failedList = true;
  const view = await start();
  expect(view.find('.board-library-error').exists()).toBe(true);
  expect(view.find('.board-library-empty').exists()).toBe(false);
  failedList = false; await view.get('.board-library-error button').trigger('click'); await flushPromises();
  await view.get('.board-library-search input').setValue('网络课程');
  expect(view.findAll('.board-library-card')).toHaveLength(1);
  expect(view.get('.board-library-card').text()).toContain('视频中的请求与响应');
  expect(network.value.workspace).toBeNull();
});

it('keeps unavailable boards visible and honors unsaved-reader navigation', async () => {
  items.push(board('missing', { book_id: 'removed', publication_id: 'old' }));
  const view = await start(); await home();
  const missing = view.findAll('.board-library-card')[2];
  expect(missing.text()).toContain('画板内容仍保留');
  expect(missing.get('footer button').attributes('disabled')).toBeDefined();
  await view.findAll('.board-library-card')[0].get('footer button').trigger('click'); await flushPromises();
  opened.allowLeave = false; await home();
  expect(view.find('.board-library').exists()).toBe(false);
});
