// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { defineComponent } from 'vue';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { installIdentity, network, workspaceStorageKey, type NetworkWorkspace, type PublishedBookRef } from './network-context';
import MaterialCover from './components/MaterialCover.vue';

const observation = vi.hoisted(() => ({ mountedBooks: [] as string[] }));
vi.mock('./App.vue', () => ({ default: defineComponent({
  name: 'App', props: ['readerVisible'], emits: ['show-library'],
  methods: { beforeNoteLeave: async () => true },
  setup() {
    const bookId = network.value.workspace?.published_book_ref.book_id ?? '';
    observation.mountedBooks.push(bookId);
    return { bookId };
  },
  template: '<div class="reader">{{ bookId }}<button @click="$emit(\'show-library\')">书架</button></div>',
}) }));
import NetworkApp from './NetworkApp.vue';

const identity = { user_id: 'opening-reader', csrf_token: 'csrf' };
const originalBook = { book_id: '上次的资料', publication_id: 'original' };
const targetBook = { book_id: '准备打开的资料', publication_id: 'target' };
const books = [
  { published_book_ref: originalBook, title: '上次的资料', cover: { kind: 'pdf', url: '/original.pdf' } },
  { published_book_ref: targetBook, title: '准备打开的资料', cover: { kind: 'pdf', url: '/target.pdf' } },
];
type Request = { method: string; url: string; body: Record<string, unknown>; signal?: AbortSignal | null };
type HeldRequest = { path: string; receive: (respond: () => Response, signal?: AbortSignal | null) => Promise<Response>; release: () => void };
let workspace: NetworkWorkspace;
let requests: Request[];
let holds: HeldRequest[];
let waiting: HeldRequest[];
let wrapper: ReturnType<typeof mount> | undefined;

beforeEach(() => {
  sessionStorage.clear(); localStorage.clear(); history.replaceState({}, '', '/');
  requests = []; holds = []; waiting = []; observation.mountedBooks = [];
  installIdentity(identity);
  sessionStorage.setItem(workspaceStorageKey(), JSON.stringify({ workspace_id: 'this-window', view: 'library' }));
  workspace = { workspace_id: 'this-window', generation: 3, revision: 8, selected_chat: 'original-chat',
    published_book_ref: originalBook, reader: { viewport: { top_lid: '1.16' } } } as NetworkWorkspace;
  vi.stubGlobal('fetch', vi.fn(async (input: string, init: RequestInit) => {
    const url = String(input);
    const body = init.body ? JSON.parse(String(init.body)) : {};
    requests.push({ method: init.method ?? 'GET', url, body, signal: init.signal });
    const respond = () => {
      if (url === '/api/auth/me') return Response.json(identity);
      if (url === '/api/library') return Response.json({ books });
      if (url === '/api/videos') return Response.json({ videos: [] });
      if (url === '/api/learning-workspaces') return Response.json({ workspaces: [] });
      if (url === '/api/me/reading-resumption' || url === '/api/workspaces/this-window/resumption') {
        return Response.json({ workspace_id: workspace.workspace_id, published_book_ref: workspace.published_book_ref,
          selected_chat: workspace.selected_chat, position_label: '段落 · 能量守恒', position_excerpt: '沿着原来的位置继续阅读。', last_question: '为什么守恒？' });
      }
      if (url === '/api/workspaces/this-window' || url === '/api/workspaces/this-window/attach') return Response.json(workspace);
      if (url === '/api/workspaces/this-window/book/open') {
        workspace = { ...workspace, generation: workspace.generation + 1, revision: workspace.revision + 1,
          published_book_ref: body.published_book_ref as PublishedBookRef, selected_chat: null,
          learning_workspace_id: body.learning_workspace_id as string | null };
        return Response.json(workspace);
      }
      throw new Error(`Unexpected ${init.method} ${url}`);
    };
    const index = holds.findIndex(hold => hold.path === url);
    return index < 0 ? respond() : holds.splice(index, 1)[0].receive(respond, init.signal);
  }));
});

afterEach(async () => {
  wrapper?.unmount(); wrapper = undefined;
  for (const hold of waiting) hold.release();
  await flushPromises();
  vi.useRealTimers(); vi.unstubAllGlobals(); vi.restoreAllMocks();
  sessionStorage.clear(); localStorage.clear();
  network.value = { ...network.value, enabled: false, identity: null, workspace: null };
});

function holdNext(path: string, respectAbort = true) {
  let release = () => {};
  const hold: HeldRequest = {
    path, release: () => release(),
    receive: (respond, signal) => new Promise((resolve, reject) => {
      let settled = false;
      const cancel = () => {
        if (settled) return;
        settled = true;
        reject(signal?.reason ?? new DOMException('Aborted', 'AbortError'));
      };
      release = () => {
        if (settled) return;
        settled = true; signal?.removeEventListener('abort', cancel); resolve(respond());
      };
      if (respectAbort) {
        signal?.addEventListener('abort', cancel, { once: true });
        if (signal?.aborted) cancel();
      }
    }),
  };
  holds.push(hold); waiting.push(hold);
  return hold;
}

async function start() {
  network.value = { ...network.value, identity: null, workspace: null };
  wrapper = mount(NetworkApp, { global: { stubs: { MaterialCover: true, YeyeMascot: true } } });
  await flushPromises();
  expect(wrapper.find('.network-library').exists()).toBe(true);
  expect(wrapper.find('.reader').exists()).toBe(false);
}
function targetCard() { return wrapper!.findAll('.network-book-card').find(card => card.text().includes(targetBook.book_id))!; }
function bannerButton(text: string) { return wrapper!.get('.book-opening').findAll('button').find(button => button.text() === text)!; }
function expectCoversPaused(paused: boolean) {
  const covers = wrapper!.findAllComponents(MaterialCover);
  expect(covers.length).toBeGreaterThan(0);
  expect(covers.every(cover => cover.props('paused') === paused)).toBe(true);
}

it('opens a board address in its exact publication and learning workspace before mounting the Reader', async () => {
  const query = new URLSearchParams({ board_id: 'saved-board', board_book_id: targetBook.book_id,
    board_publication_id: targetBook.publication_id, board_learning_workspace_id: 'original-learning' });
  history.replaceState({}, '', `/?${query}`);
  network.value = { ...network.value, identity: null, workspace: null };
  wrapper = mount(NetworkApp, { global: { stubs: { MaterialCover: true, YeyeMascot: true } } });
  await flushPromises();
  expect(network.value.workspace?.published_book_ref).toEqual(targetBook);
  expect(network.value.workspace?.learning_workspace_id).toBe('original-learning');
  expect(observation.mountedBooks).toEqual([targetBook.book_id]);
  expect(requests.filter(request => request.url.endsWith('/book/open'))).toHaveLength(1);
  expect(new URLSearchParams(location.search).get('board_id')).toBe('saved-board');
});

it('shows the target and pauses covers immediately, suppresses duplicate opens, and mounts only the requested Reader', async () => {
  await start();
  const pending = holdNext('/api/workspaces/this-window/book/open');
  const card = targetCard();
  await card.trigger('click');
  await flushPromises();
  expect(wrapper!.get('.book-opening').attributes('role')).toBe('status');
  expect(wrapper!.get('.book-opening').text()).toContain('正在打开');
  expect(wrapper!.get('.book-opening').text()).toContain(targetBook.book_id);
  expect(wrapper!.get('.network-library').attributes('aria-busy')).toBe('true');
  expect(bannerButton('取消').exists()).toBe(true);
  expectCoversPaused(true);
  expect(observation.mountedBooks).toEqual([]);
  expect(network.value.workspace).toBeNull();
  await card.trigger('click');
  await flushPromises();
  expect(requests.filter(request => request.url.endsWith('/book/open'))).toHaveLength(1);
  pending.release();
  await flushPromises();
  expect(wrapper!.find('.book-opening').exists()).toBe(false);
  expect(wrapper!.find('.network-library').exists()).toBe(false);
  expect(wrapper!.get('.reader').text()).toContain(targetBook.book_id);
  expect(observation.mountedBooks).toEqual([targetBook.book_id]);
});

it('aborts a stalled open after 15 seconds and lets the same target succeed on retry', async () => {
  await start();
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
  holdNext('/api/workspaces/this-window/book/open');
  await targetCard().trigger('click');
  await flushPromises();
  const stalled = requests.find(request => request.url.endsWith('/book/open'))!;
  await vi.advanceTimersByTimeAsync(15_000);
  await flushPromises();
  expect(stalled.signal?.aborted).toBe(true);
  expect(wrapper!.get('.book-opening').attributes('role')).toBe('alert');
  expect(wrapper!.get('.book-opening').text()).toContain('打开资料超时');
  expect(wrapper!.get('.network-library').attributes('aria-busy')).toBe('false');
  expectCoversPaused(false);
  expect(observation.mountedBooks).toEqual([]);
  await bannerButton('重试打开').trigger('click');
  await flushPromises();
  expect(requests.filter(request => request.url.endsWith('/book/open'))).toHaveLength(2);
  expect(wrapper!.find('.book-opening').exists()).toBe(false);
  expect(wrapper!.get('.reader').text()).toContain(targetBook.book_id);
  expect(observation.mountedBooks).toEqual([targetBook.book_id]);
});

it('returns to an interactive shelf on cancel and ignores a success arriving after cancellation', async () => {
  await start();
  const pending = holdNext('/api/workspaces/this-window/book/open', false);
  await targetCard().trigger('click');
  await flushPromises();
  const request = requests.find(request => request.url.endsWith('/book/open'))!;
  await bannerButton('取消').trigger('click');
  await flushPromises();
  expect(request.signal?.aborted).toBe(true);
  expect(wrapper!.find('.book-opening').exists()).toBe(false);
  expect(wrapper!.get('.network-library').attributes('aria-busy')).toBe('false');
  expect(targetCard().attributes('disabled')).toBeUndefined();
  expectCoversPaused(false);
  pending.release();
  await flushPromises();
  expect(network.value.workspace).toBeNull();
  expect(wrapper!.find('.network-library').exists()).toBe(true);
  expect(wrapper!.find('.reader').exists()).toBe(false);
  expect(observation.mountedBooks).toEqual([]);
});

it('keeps a failed open actionable and lets the user dismiss its banner', async () => {
  await start();
  const original = vi.mocked(fetch).getMockImplementation()!;
  vi.mocked(fetch).mockImplementation((input, init) => String(input).endsWith('/book/open')
    ? Promise.resolve(Response.json({ message: '连接暂时不可用' }, { status: 503 })) : original(input, init));
  await targetCard().trigger('click');
  await flushPromises();
  expect(wrapper!.get('.book-opening').attributes('role')).toBe('alert');
  expect(wrapper!.get('.book-opening').text()).toContain('连接暂时不可用');
  expect(bannerButton('重试打开').exists()).toBe(true);
  expectCoversPaused(false);
  await bannerButton('返回书架').trigger('click');
  expect(wrapper!.find('.book-opening').exists()).toBe(false);
  expect(wrapper!.find('.network-library').exists()).toBe(true);
  expect(targetCard().attributes('disabled')).toBeUndefined();
});

it('uses the opening feedback and cancellation flow for continuing a saved reading position', async () => {
  await start();
  const pending = holdNext('/api/workspaces/this-window/attach', false);
  await wrapper!.get('.resumption-continue').trigger('click');
  await flushPromises();
  expect(wrapper!.get('.book-opening').attributes('role')).toBe('status');
  expect(wrapper!.get('.book-opening').text()).toContain(originalBook.book_id);
  expectCoversPaused(true);
  await bannerButton('取消').trigger('click');
  await flushPromises();
  expect(wrapper!.find('.book-opening').exists()).toBe(false);
  expectCoversPaused(false);
  pending.release();
  await flushPromises();
  expect(wrapper!.find('.reader').exists()).toBe(false);
  expect(network.value.workspace).toBeNull();
  expect(observation.mountedBooks).toEqual([]);
});
