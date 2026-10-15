// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import NetworkApp from './NetworkApp.vue';
import { api } from './api';
import { installIdentity, installWorkspace, network, type NetworkWorkspace } from './network-context';
import { workspaceAction } from './network-client';

const reads = vi.hoisted(() => Object.fromEntries([
  'manifest', 'assetManifest', 'sourceFingerprint', 'sourceManifest', 'profileManifest', 'text',
  'recall', 'save', 'profileMemory', 'tutorState', 'tutorReadiness', 'agentRunCreate', 'agentRun',
].map(name => [name, vi.fn()])));
vi.mock('./api', async original => {
  const actual = await original<typeof import('./api')>();
  return { ...actual, api: { ...actual.api, ...reads } };
});
vi.mock('./components/PdfReaderPane.vue', () => ({ default: { name: 'PdfReaderPane', template: '<div />' } }));

let workspace: NetworkWorkspace;
let requests: string[];
let wrapper: ReturnType<typeof mount>;
const history = () => ({ active_session_id: workspace.selected_chat ?? '', sessions: [], current: {
  id: workspace.selected_chat ?? '', book_id: 'book', goals: [], turns: workspace.selected_chat === 'old' ? [{
    turn_id: 'old-turn', user: '原对话', status: 'completed', answer: '旧回答', effect_labels: [],
  }] : [],
} });

beforeEach(() => {
  vi.resetAllMocks(); requests = [];
  installIdentity({ user_id: 'reader', csrf_token: 'csrf' });
  workspace = { workspace_id: 'workspace', generation: 1, revision: 1, selected_chat: 'old',
    published_book_ref: { book_id: 'book', publication_id: 'publication' }, reader: {
      book_id: 'book', revision: 0, viewport: { anchor_lid: '1', top_lid: '1', bottom_lid: '1', visible_lids: ['1'], width: 1 },
      profile: { profile_id: 'technical_learning', profile_version: '1' }, layout: { rev: 0, open_slots: [] },
    } } as unknown as NetworkWorkspace;
  installWorkspace(workspace);
  reads.manifest.mockResolvedValue({ tree: [{ lid: '1', kind: 'paragraph', children: [], span: { start: 0, end: 4 } }], stats_by_lid: {} });
  reads.assetManifest.mockResolvedValue({ images: [] });
  reads.sourceFingerprint.mockResolvedValue({ book_id: 'book', source_fingerprint: 'book' });
  reads.sourceManifest.mockResolvedValue(null);
  reads.profileManifest.mockResolvedValue({ profile_id: 'technical_learning', slots: [] });
  reads.text.mockResolvedValue({ lid: '1', text: '当前正文' });
  reads.recall.mockResolvedValue([]); reads.profileMemory.mockResolvedValue({});
  reads.tutorState.mockResolvedValue({ control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} });
  reads.tutorReadiness.mockResolvedValue({ status: 'ready', source_id: 'book', limitations: [] });
  reads.agentRunCreate.mockResolvedValue({ answer: '新回答', effects: [] });
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    requests.push(url);
    if (url === '/api/auth/logout' || url.endsWith('/detach')) return Response.json({ ok: true });
    if (url === '/api/auth/me') return Response.json(network.value.identity);
    if (url === '/api/workspaces/workspace' || url.endsWith('/attach')) return Response.json(workspace);
    if (url.endsWith('/chat/new') || url.endsWith('/chat/select')) {
      const body = JSON.parse(String(init.body));
      workspace = { ...workspace, selected_chat: url.endsWith('/new') ? 'new' : body.session_id,
        generation: workspace.generation + 1, revision: workspace.revision + 1 };
      return Response.json(workspace);
    }
    if (url.endsWith('/chat/history')) return Response.json({ result: history() });
    throw new Error(`Unexpected request: ${url}`);
  }));
});
afterEach(() => {
  wrapper?.unmount(); vi.restoreAllMocks(); vi.unstubAllGlobals();
  network.value = { ...network.value, enabled: false, identity: null, workspace: null };
  sessionStorage.clear(); localStorage.clear();
});
async function start() {
  wrapper = mount(NetworkApp, { shallow: true, global: { renderStubDefaultSlot: true, stubs: { App: false } } });
  await flushPromises();
}
function rail() { return wrapper.getComponent({ name: 'RightRail' }); }

it('creates and selects chats without replacing the reader, reloading book data or resetting its layout', async () => {
  await start();
  const reader = wrapper.getComponent({ name: 'ReaderPane' });
  const layout = wrapper.getComponent({ name: 'ReaderWorkspace' });
  const layoutContext = layout.props('contextKey');
  const textReads = reads.text.mock.calls.length;
  reader.element.scrollTop = 240;
  rail().vm.$emit('update:agentInput', '未发送草稿');
  rail().vm.$emit('new-chat'); await flushPromises();
  expect(wrapper.getComponent({ name: 'ReaderPane' }).vm).toBe(reader.vm);
  expect(reader.element.scrollTop).toBe(240);
  expect(wrapper.getComponent({ name: 'ReaderWorkspace' }).props('contextKey')).toBe(layoutContext);
  expect(reads.manifest).toHaveBeenCalledTimes(1);
  expect(reads.text).toHaveBeenCalledTimes(textReads);
  expect(wrapper.find('.app-loading').exists()).toBe(false);
  expect(rail().props('activeChatSessionId')).toBe('new');
  expect(rail().props('chat')).toEqual([]);
  expect(rail().props('agentInput')).toBe('');
  expect(rail().props('historyLoading')).toBe(false);
  expect(rail().props('historyError')).toBeNull();
  rail().vm.$emit('update:agentInput', '新问题');
  rail().vm.$emit('send-agent'); await flushPromises();
  expect(reads.agentRunCreate).toHaveBeenCalledWith('新问题', expect.anything());
  rail().vm.$emit('select-chat', 'old'); await flushPromises();
  expect(wrapper.getComponent({ name: 'ReaderPane' }).vm).toBe(reader.vm);
  expect(rail().props('activeChatSessionId')).toBe('old');
  expect(rail().props('chat')[0].user).toBe('原对话');
  expect(reads.manifest).toHaveBeenCalledTimes(1);
  expect(wrapper.find('.banner').exists()).toBe(false);
});

it('ignores a late submission result after switching to a new chat', async () => {
  let finish!: (value: unknown) => void;
  reads.agentRunCreate.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  await start();
  rail().vm.$emit('update:agentInput', '旧问题');
  rail().vm.$emit('send-agent'); await flushPromises();
  rail().vm.$emit('new-chat'); await flushPromises();
  rail().vm.$emit('update:agentInput', '新的草稿');
  finish({ book_id: 'book', session_id: 'old', turn_id: 'late-turn' }); await flushPromises();
  expect(rail().props('activeChatSessionId')).toBe('new');
  expect(rail().props('chat')).toEqual([]);
  expect(rail().props('agentInput')).toBe('新的草稿');
  expect(rail().props('sending')).toBe(false);
  expect(reads.agentRun).not.toHaveBeenCalled();
});

it('preserves the first submitted question when admission creates its chat', async () => {
  workspace = { ...workspace, selected_chat: null }; installWorkspace(workspace);
  vi.stubGlobal('EventSource', class extends EventTarget { close() {} });
  const descriptor = { book_id: 'book', session_id: 'new', turn_id: 'first-turn' };
  reads.agentRun.mockResolvedValue({ descriptor, last_seq: 0, execution_state: 'running', persistence_state: 'pending', activities: [] });
  reads.agentRunCreate.mockImplementation(async () => { await workspaceAction('chat/new'); return descriptor; });
  await start();
  const reader = wrapper.getComponent({ name: 'ReaderPane' }).vm;
  rail().vm.$emit('update:agentInput', '首次提问');
  rail().vm.$emit('send-agent'); await flushPromises();
  expect(wrapper.getComponent({ name: 'ReaderPane' }).vm).toBe(reader);
  expect(rail().props('activeChatSessionId')).toBe('new');
  expect(rail().props('chat')[0]).toMatchObject({ user: '首次提问', turnId: 'first-turn' });
  expect(rail().props('sending')).toBe(true);
  expect(reads.agentRunCreate).toHaveBeenCalledTimes(1);
  expect(reads.manifest).toHaveBeenCalledTimes(1);
});

it('keeps the current chat and reader when creating a chat fails', async () => {
  await start();
  const reader = wrapper.getComponent({ name: 'ReaderPane' }).vm;
  vi.spyOn(api, 'agentNew').mockRejectedValueOnce(new Error('创建失败'));
  rail().vm.$emit('update:agentInput', '保留草稿');
  rail().vm.$emit('new-chat'); await flushPromises();
  expect(wrapper.getComponent({ name: 'ReaderPane' }).vm).toBe(reader);
  expect(rail().props('activeChatSessionId')).toBe('old');
  expect(rail().props('agentInput')).toBe('保留草稿');
  expect(rail().props('historyLoading')).toBe(false);
  expect(wrapper.get('.banner').text()).toContain('创建失败');
});

it('offers a local history retry if chat creation succeeds but its history cannot be read', async () => {
  await start();
  const reader = wrapper.getComponent({ name: 'ReaderPane' }).vm;
  const originalFetch = fetch;
  vi.stubGlobal('fetch', vi.fn((url: string, init: RequestInit) => url.endsWith('/chat/history')
    ? Promise.resolve(Response.json({ error_code: 'HISTORY_UNAVAILABLE', message: '历史读取失败' }, { status: 503 }))
    : originalFetch(url, init)));
  rail().vm.$emit('new-chat'); await flushPromises();
  expect(rail().props('activeChatSessionId')).toBe('new');
  expect(rail().props('historyError')).toContain('历史读取失败');
  expect(rail().props('historyLoading')).toBe(false);
  vi.stubGlobal('fetch', originalFetch);
  rail().vm.$emit('retry-history'); await flushPromises();
  expect(rail().props('historyError')).toBeNull();
  expect(wrapper.getComponent({ name: 'ReaderPane' }).vm).toBe(reader);
  expect(requests.filter(url => url.endsWith('/chat/new'))).toHaveLength(1);
});


it('RN3 preserves an answer draft across network chat changes and processes it before logout', async () => {
  wrapper = mount(NetworkApp, { shallow: true, global: { renderStubDefaultSlot: true, stubs: { App: false,
    TopBar: { template: '<div><slot name="account" /></div>' } } } });
  await flushPromises();
  rail().vm.$emit('save-answer-selection', { turnId: 'old-turn', pending: false, outcome: { answer: '原回答' } }, '原回答');
  await flushPromises();
  const editor = () => wrapper.getComponent({ name: 'NoteEditorPanel' });
  editor().vm.$emit('update:content', '我的想法');
  rail().vm.$emit('new-chat'); await flushPromises();
  expect(editor().props('content')).toBe('我的想法');
  const logout = () => wrapper.findAll('button').find(b => b.text() === '退出登录')!;
  await logout().trigger('click'); await flushPromises();
  expect(requests).not.toContain('/api/auth/logout');
  await wrapper.get('.note-transition').findAll('button').find(b => b.text() === '继续编辑')!.trigger('click'); await flushPromises();
  expect(network.value.identity?.user_id).toBe('reader');
  await logout().trigger('click'); await flushPromises();
  reads.save.mockRejectedValueOnce(new Error('写入失败'));
  await wrapper.get('.note-transition').findAll('button').find(b => b.text() === '保存并继续')!.trigger('click'); await flushPromises();
  expect(requests).not.toContain('/api/auth/logout'); expect(editor().props('content')).toBe('我的想法');
  reads.save.mockResolvedValue({ mem_id: 'saved' });
  await wrapper.get('.note-transition').findAll('button').find(b => b.text() === '保存并继续')!.trigger('click'); await flushPromises();
  expect(reads.save).toHaveBeenLastCalledWith(expect.objectContaining({ note: { association: { kind: 'answer', session_id: 'old', turn_id: 'old-turn' }, retained_excerpt: '原回答' } }));
  expect(requests).toContain('/api/auth/logout'); expect(network.value.identity).toBeNull();
});
