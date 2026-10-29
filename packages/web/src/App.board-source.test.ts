// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { defineComponent, ref } from 'vue';
import App from './App.vue';
import type { SourcePopupView } from './api';
import { network, submittedRunDrafts } from './network-context';

const mocks = vi.hoisted(() => ({ showReader: vi.fn(), scrollLidIntoView: vi.fn(), api: Object.fromEntries([
  'videoPlayback', 'videoContext', 'musicContext', 'videoDraft', 'desktopStatus', 'buildWorkbench', 'manifest', 'assetManifest',
  'sourceFingerprint', 'sourceManifest', 'state', 'profileManifest', 'text', 'recall', 'agentHistory', 'profileMemory',
  'profileBackfill', 'intentUsageEvent', 'intentArtifacts', 'bookLibrary', 'paperMetadata', 'tutorState', 'tutorReadiness',
  'boardSourceResolve', 'boardSourceOpen',
].map(key => [key, vi.fn()])) }));
vi.mock('./api', async original => ({ ...await original<typeof import('./api')>(), api: mocks.api }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('./components/PdfReaderPane.vue', () => ({ default: { name: 'PdfReaderPane', template: '<div />' } }));

const source = (id = 'source-A'): SourcePopupView => ({ source_ref_id: id, label: `原文 ${id}`, highlighted_quote: '六格是对手的普攻距离。', context_before: '前文。', context_after: '后文。', stale: false, can_open_in_reader: true });
const request = (id = 'source-A') => ({ board_id: 'board-A', item_id: 'answer-A', source_ref_id: id });
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(yes => { resolve = yes; });
  return { promise, resolve };
}
let wrapper: ReturnType<typeof mount> | undefined;
beforeEach(() => {
  vi.resetAllMocks();
  window.history.replaceState({}, '', '/');
  const api = mocks.api;
  api.videoContext.mockResolvedValue(null); api.musicContext.mockResolvedValue(null); api.videoPlayback.mockResolvedValue(null); api.videoDraft.mockResolvedValue({ draft: null });
  api.desktopStatus.mockResolvedValue({ desktop_host: false, reader_only: true, active_book: true });
  api.buildWorkbench.mockResolvedValue({ book_id: 'startup', readiness: { route: 'reader' }, input: { manifest: null }, source_review: { unresolved: [] } });
  api.manifest.mockResolvedValue({ tree: [{ lid: '1', kind: 'chapter', children: ['1.1'], span: { start: 0, end: 22 }, title: '# 首章' }, { lid: '1.1', kind: 'paragraph', children: [], span: { start: 5, end: 22 } }], stats_by_lid: {} });
  api.assetManifest.mockResolvedValue({ images: [] }); api.sourceFingerprint.mockResolvedValue({ book_id: 'startup', source_fingerprint: 'startup' }); api.sourceManifest.mockResolvedValue(null);
  api.state.mockResolvedValue({ book_id: 'startup', revision: 0, viewport: { anchor_lid: '1.1', top_lid: '1.1', bottom_lid: '1.1', width: 1, visible_lids: ['1.1'] }, profile: { profile_id: 'technical_learning', profile_version: '1' }, layout: { rev: 0, open_slots: [] } });
  api.profileManifest.mockResolvedValue({ profile_id: 'technical_learning', slots: [] }); api.text.mockImplementation(async (lid: string) => ({ lid, text: '可以阅读的正文' })); api.recall.mockResolvedValue([]);
  api.agentHistory.mockResolvedValue({ active_session_id: 'startup-chat', sessions: [], current: { id: 'startup-chat', book_id: 'startup', turns: [], goals: [] } });
  api.profileMemory.mockResolvedValue({}); api.profileBackfill.mockResolvedValue({ jobs: [], sessions: [] }); api.intentArtifacts.mockResolvedValue({ overlay: null }); api.intentUsageEvent.mockResolvedValue({}); api.bookLibrary.mockResolvedValue({ root: 'books', books: [] });
  api.tutorState.mockResolvedValue({ control: { enabled: true, revision: 0, current_tutor_session_id: null }, sessions: {} });
  api.tutorReadiness.mockResolvedValue({ status: 'ready', source_id: 'startup', source_revision: 's1', limitations: [], reason: '可用', teaching_assets: { status: 'stale', teaching_map_revision: null, limitations: [], reason: '教学资料构建失败' } });
  api.boardSourceResolve.mockImplementation(async ({ source_ref_id }) => source(source_ref_id));
  api.boardSourceOpen.mockResolvedValue({ source_ref_id: 'source-A', opened: true });
});
afterEach(() => {
  wrapper?.unmount(); wrapper = undefined;
  network.value = { ...network.value, enabled: false, workspace: null, identity: null };
  submittedRunDrafts.clear(); localStorage.clear(); sessionStorage.clear();
  window.history.replaceState({}, '', '/'); vi.restoreAllMocks();
});
async function setup() {
  wrapper = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: {
    ReaderWorkspace: defineComponent({ name: 'ReaderWorkspace', props: ['returnAvailable', 'returnLabel'], setup(_, { expose }) { expose({ showReader: mocks.showReader, showAssistant: vi.fn() }); return {}; }, template: '<div><slot /></div>' }),
    ReaderPane: defineComponent({ setup(_, { expose }) { expose({ captureScrollAnchor: () => null, scrollLidIntoView: mocks.scrollLidIntoView }); }, template: '<div />' }),
    BoardWorkspace: defineComponent({ name: 'BoardWorkspace', props: ['boardId', 'pendingLocate'], setup() { return { draft: ref('未发出的追问') }; }, template: '<div><textarea v-model="draft" /></div>' }),
  } } });
  await flushPromises();
  wrapper.getComponent({ name: 'RightRail' }).vm.$emit('open-board-location', { board_id: 'board-A', item_id: 'answer-A' });
  await flushPromises();
  return wrapper;
}
describe('board source navigation', () => {
  it('previews an available source first, enters the reader explicitly, and returns to the same mounted board and draft', async () => {
    const app = await setup(), board = app.getComponent({ name: 'BoardWorkspace' });
    const boardElement = board.element;
    mocks.scrollLidIntoView.mockClear();
    board.vm.$emit('source', request()); await flushPromises();
    const preview = app.getComponent({ name: 'BoardSourcePreview' });
    expect(preview.props('source')).toEqual(source());
    expect(preview.props('sourceRequest')).toEqual(request());
    expect(mocks.api.boardSourceOpen).not.toHaveBeenCalled();
    expect(app.get('.board-workspace-overlay').isVisible()).toBe(true);
    preview.vm.$emit('open-reader'); await flushPromises();
    expect(mocks.api.boardSourceOpen).toHaveBeenCalledWith(request());
    expect(preview.props('error')).toBe('');
    expect(mocks.showReader).toHaveBeenCalledOnce();
    expect(mocks.scrollLidIntoView).toHaveBeenCalledWith('1.1');
    expect(mocks.showReader.mock.invocationCallOrder[0]).toBeLessThan(mocks.scrollLidIntoView.mock.invocationCallOrder[0]);
    expect(app.findComponent({ name: 'BoardSourcePreview' }).exists()).toBe(false);
    expect((app.get('.board-workspace-overlay').element as HTMLElement).style.display).toBe('none');
    const reader = app.getComponent({ name: 'ReaderWorkspace' });
    expect(reader.props()).toMatchObject({ returnAvailable: true, returnLabel: '返回画板' });
    reader.vm.$emit('return'); await flushPromises();
    expect(app.get('.board-workspace-overlay').isVisible()).toBe(true);
    expect(board.element).toBe(boardElement);
    expect(board.props('pendingLocate')).toEqual({ board_id: 'board-A', item_id: 'answer-A' });
    expect(board.get('textarea').element).toHaveProperty('value', '未发出的追问');
  });
  it('opens the current temporary reading location and keeps it when retrying navigation', async () => {
    const app = await setup();
    app.getComponent({ name: 'BoardWorkspace' }).vm.$emit('source', request()); await flushPromises();
    const preview = app.getComponent({ name: 'BoardSourcePreview' });
    mocks.api.boardSourceOpen.mockRejectedValueOnce(new Error('打开失败，请重试'));
    preview.vm.$emit('open-reader', '2.3'); await flushPromises();
    expect(mocks.api.boardSourceOpen).toHaveBeenLastCalledWith({ ...request(), lid: '2.3' });
    preview.vm.$emit('retry'); await flushPromises();
    expect(mocks.api.boardSourceOpen).toHaveBeenLastCalledWith({ ...request(), lid: '2.3' });
    expect(app.findComponent({ name: 'BoardSourcePreview' }).exists()).toBe(false);
  });
  it('keeps the source preview retryable when loading fails and when reader navigation fails', async () => {
    mocks.api.boardSourceResolve.mockRejectedValueOnce(new Error('原文加载失败'));
    const app = await setup();
    app.getComponent({ name: 'BoardWorkspace' }).vm.$emit('source', request()); await flushPromises();
    const preview = app.getComponent({ name: 'BoardSourcePreview' });
    expect(preview.props()).toMatchObject({ source: null, loading: false, error: '原文加载失败' });
    preview.vm.$emit('retry'); await flushPromises();
    expect(preview.props()).toMatchObject({ source: source(), error: '' });
    mocks.api.boardSourceOpen.mockRejectedValueOnce(new Error('打开失败，请重试'));
    preview.vm.$emit('open-reader'); await flushPromises();
    expect(preview.props()).toMatchObject({ opening: false, error: '打开失败，请重试', source: source() });
    expect(app.get('.board-workspace-overlay').isVisible()).toBe(true);
    preview.vm.$emit('retry'); await flushPromises();
    expect(mocks.api.boardSourceOpen).toHaveBeenCalledTimes(2);
    expect(app.findComponent({ name: 'BoardSourcePreview' }).exists()).toBe(false);
  });
  it('ignores an older resolved source and a pending source closed by the learner', async () => {
    const first = deferred<SourcePopupView>(), third = deferred<SourcePopupView>();
    mocks.api.boardSourceResolve.mockReturnValueOnce(first.promise);
    const app = await setup(), board = app.getComponent({ name: 'BoardWorkspace' });
    board.vm.$emit('source', request()); await flushPromises();
    expect(app.getComponent({ name: 'BoardSourcePreview' }).props('loading')).toBe(true);
    board.vm.$emit('source', request('source-B')); await flushPromises();
    first.resolve(source()); await flushPromises();
    expect(app.getComponent({ name: 'BoardSourcePreview' }).props('source')).toEqual(source('source-B'));
    mocks.api.boardSourceResolve.mockReturnValueOnce(third.promise);
    board.vm.$emit('source', request('source-C')); await flushPromises();
    app.getComponent({ name: 'BoardSourcePreview' }).vm.$emit('close'); await flushPromises();
    third.resolve(source('source-C')); await flushPromises();
    expect(app.findComponent({ name: 'BoardSourcePreview' }).exists()).toBe(false);
    expect(mocks.api.boardSourceOpen).not.toHaveBeenCalled();
  });
  it('retains the board and preview for retry if loading the visible reader fails', async () => {
    const app = await setup();
    app.getComponent({ name: 'BoardWorkspace' }).vm.$emit('source', request()); await flushPromises();
    const preview = app.getComponent({ name: 'BoardSourcePreview' });
    mocks.api.state.mockRejectedValueOnce(new Error('阅读位置加载失败'));
    preview.vm.$emit('open-reader'); await flushPromises();
    expect(mocks.showReader).toHaveBeenCalledOnce();
    expect(preview.props()).toMatchObject({ source: source(), opening: false, error: '阅读位置加载失败' });
    expect((app.get('.board-workspace-overlay').element as HTMLElement).style.display).not.toBe('none');
    expect(app.getComponent({ name: 'ReaderWorkspace' }).props('returnAvailable')).toBe(false);
    preview.vm.$emit('retry'); await flushPromises();
    expect(app.findComponent({ name: 'BoardSourcePreview' }).exists()).toBe(false);
    expect((app.get('.board-workspace-overlay').element as HTMLElement).style.display).toBe('none');
  });
  it('does not offer or execute reader navigation for a retained stale source', async () => {
    mocks.api.boardSourceResolve.mockResolvedValue({ ...source(), stale: true, can_open_in_reader: false });
    const app = await setup();
    app.getComponent({ name: 'BoardWorkspace' }).vm.$emit('source', request()); await flushPromises();
    const preview = app.getComponent({ name: 'BoardSourcePreview' });
    expect(preview.props('canOpenInReader')).toBe(false);
    expect(preview.props('source').highlighted_quote).toBe(source().highlighted_quote);
    preview.vm.$emit('open-reader'); await flushPromises();
    expect(mocks.api.boardSourceOpen).not.toHaveBeenCalled();
    expect(app.get('.board-workspace-overlay').isVisible()).toBe(true);
  });
  it('does not leave the board when an older reader-open request completes after switching sources', async () => {
    const opening = deferred<{ source_ref_id: string; opened: boolean }>();
    mocks.api.boardSourceOpen.mockReturnValueOnce(opening.promise);
    const app = await setup(), board = app.getComponent({ name: 'BoardWorkspace' });
    board.vm.$emit('source', request()); await flushPromises();
    app.getComponent({ name: 'BoardSourcePreview' }).vm.$emit('open-reader'); await flushPromises();
    expect(app.getComponent({ name: 'BoardSourcePreview' }).props('opening')).toBe(true);
    board.vm.$emit('source', request('source-B')); await flushPromises();
    opening.resolve({ source_ref_id: 'source-A', opened: true }); await flushPromises();
    expect(app.getComponent({ name: 'BoardSourcePreview' }).props()).toMatchObject({ source: source('source-B'), opening: false });
    expect(mocks.showReader).not.toHaveBeenCalled();
    expect(app.get('.board-workspace-overlay').isVisible()).toBe(true);
  });
});
