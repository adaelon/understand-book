// @vitest-environment happy-dom
import { config, mount, flushPromises } from "@vue/test-utils";
import { defineComponent, inject } from 'vue';
import { renderShare, shareHighlightKey, shareNoteKey, sharePresentationKey } from '@understand-book/web/reader/reading-share';
import { afterAll, afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "@understand-book/web/reader/App.vue";
import PersonalNotes from '@understand-book/web/reader/components/PersonalNotes.vue';
import { discardNoteDraft, emptyNoteContent, emptyNoteContext, notesPanel, openThoughtCapture, readNoteDrafts } from '@understand-book/web/reader/personal-notes';
import { network, installIdentity, installWorkspace, submittedRunDrafts, readerKey } from './network-context';
import { writeReaderSurfacePreference } from '@understand-book/web/reader/reader-surface';
import './network-client';
import * as networkClient from '@understand-book/web/reader/reader-host';
import { inquiryCourse } from '@understand-book/web/reader/inquiry-course';
import type { ModelSettingsStatus } from '@understand-book/web/reader/desktop-provider';
import { invoke } from '@tauri-apps/api/core';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const api = vi.hoisted(() => Object.fromEntries([
  "videoPlayback", "videoContext", "musicContext", "adoptMusicPublication", "videoDraft", "adoptVideoPublication", "goto", "desktopStatus", "buildWorkbench", "manifest", "assetManifest", "sourceFingerprint",
  "sourceManifest", "state", "profileManifest", "text", "recall", "agentHistory",
  "profileMemory", "profileBackfill", "intentUsageEvent", "intentArtifacts", "bookLibrary", "paperMetadata",
  "openBook", "agentNew", "disposeEffect", "agentRunCreate", "agentRun", "agentRunRetrySave", "formulaSemantics",
  "save", "replace", "delete", "prepareNote", "agentHistorySelect", "agentSourceResolve", "tutorState", "tutorReadiness", "tutorMutate", "tutorStart",
  "closedBook", "learningTasks", "selectLearningTask",
].map(key => [key, vi.fn()])));
vi.mock("@understand-book/web/reader/api", async original => ({ ...await original<typeof import("@understand-book/web/reader/api")>(), api }));
vi.mock("@understand-book/web/reader/components/PdfReaderPane.vue", () => ({ default: { name: "PdfReaderPane", template: "<div />" } }));
vi.mock('@understand-book/web/reader/inquiry-course', () => ({ inquiryCourse: { catalog: vi.fn() } }));
vi.mock('@understand-book/web/reader/reading-share', async original => ({ ...await original<typeof import('@understand-book/web/reader/reading-share')>(), renderShare: vi.fn() }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

it('submits the selected presentation output once and resets it only after acceptance', async () => {
  const admission = deferred<unknown>();
  api.agentRunCreate.mockReturnValue(admission.promise);
  const wrapper = start(); await flushPromises();
  const rail = wrapper.findComponent({ name: 'RightRail' });
  rail.vm.$emit('update:agentInput', '解释一下这里');
  rail.vm.$emit('update:presentationRequested', true);
  await flushPromises();
  rail.vm.$emit('send-agent'); await flushPromises();
  expect(api.agentRunCreate).toHaveBeenCalledWith('解释一下这里', expect.objectContaining({ presentation_requested: true, display_user: '解释一下这里' }));
  expect(rail.props('presentationRequested')).toBe(true);
  admission.resolve({ book_id: 'startup', session_id: 'startup-chat', turn_id: 'page-request' });
  await flushPromises();
  expect(rail.props('presentationRequested')).toBe(false);
});

it('keeps presentation output with a rejected draft so a retry carries the same intent', async () => {
  const { ApiError } = await import('@understand-book/web/reader/api');
  api.agentRunCreate.mockRejectedValue(new ApiError(409, 'ALLOWANCE_INSUFFICIENT', 'model_spend', 'denied'));
  const wrapper = start(); await flushPromises();
  const rail = wrapper.findComponent({ name: 'RightRail' });
  rail.vm.$emit('update:agentInput', '解释一下这里');
  rail.vm.$emit('update:presentationRequested', true);
  await flushPromises();
  rail.vm.$emit('send-agent'); await flushPromises();
  expect(rail.props('presentationRequested')).toBe(true);
  expect(rail.props('agentInput')).toBe('解释一下这里');
  rail.vm.$emit('send-agent'); await flushPromises();
  expect(api.agentRunCreate).toHaveBeenCalledTimes(2);
  expect(api.agentRunCreate).toHaveBeenLastCalledWith('解释一下这里', expect.objectContaining({ presentation_requested: true }));
});

it('leaves ordinary messages unchanged when presentation output is switched off', async () => {
  api.agentRunCreate.mockResolvedValue({ answer: '解释内容', effects: [] });
  const wrapper = start(); await flushPromises();
  const rail = wrapper.findComponent({ name: 'RightRail' });
  rail.vm.$emit('update:agentInput', '解释一下这里');
  rail.vm.$emit('update:presentationRequested', true);
  rail.vm.$emit('update:presentationRequested', false);
  await flushPromises();
  rail.vm.$emit('send-agent'); await flushPromises();
  expect(api.agentRunCreate.mock.calls[0][0]).toBe('解释一下这里');
  expect(api.agentRunCreate.mock.calls[0][1]).not.toHaveProperty('presentation_requested');
});
const history = (book = "startup") => ({ active_session_id: `${book}-chat`, sessions: [],
  current: { id: `${book}-chat`, book_id: book, turns: [], goals: [] } });
let book = "startup";
const wrappers: ReturnType<typeof mount>[] = [];
const originalStubs = config.global.stubs;
const boardWorkspaceStub = defineComponent({ name: 'BoardWorkspace',
  props: ['boardId', 'pendingImport', 'run', 'pendingLocate', 'notesRevision'], template: '<div />',
  setup(_, { expose }) { expose({ flush: async () => true }); } });
afterAll(() => { config.global.stubs = originalStubs; });
function start(withNoteSurface = false) {
  const wrapper = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: withNoteSurface ? {
    ReaderPane: defineComponent({ name: 'ReaderPane', template: '<div />', setup(_, { expose }) {
      expose({ captureScrollAnchor: () => ({ lid: '1.1' }) });
    } }),
  } : {} } });
  wrappers.push(wrapper);
  return wrapper;
}

it('blocks reader interaction while its host is still restoring the learning task', async () => {
  const wrapper = start();
  await wrapper.setProps({ hostOpening: true });
  await flushPromises();
  expect(wrapper.get('.app').attributes('inert')).toBeDefined();
  expect(wrapper.get('.app').attributes('aria-busy')).toBe('true');
  await wrapper.setProps({ hostOpening: false });
  expect(wrapper.get('.app').attributes('inert')).toBeUndefined();
  expect(wrapper.get('.app').attributes('aria-busy')).toBeUndefined();
});

const learningWorkspaceStub = defineComponent({ template: '<div><slot /></div>',
  setup(_props, { expose }) { expose({ showAssistant: () => {} }); } });
function startPurpose(chatDraft?: { message: string; quote: { lid: string; quote: string } | null; goalId: string | null }, stubs: Record<string, ReturnType<typeof defineComponent>> = {}) {
  const wrapper = mount(App, { props: { chatDraft }, shallow: true,
    global: { renderStubDefaultSlot: true, stubs: { ReaderWorkspace: learningWorkspaceStub, ...stubs } } });
  wrappers.push(wrapper);
  return wrapper;
}
beforeEach(() => {
  vi.resetAllMocks();
  config.global.stubs = { ...originalStubs, BoardWorkspace: boardWorkspaceStub };
  book = "startup";
  vi.mocked(inquiryCourse.catalog).mockResolvedValue({ release_id: '', release_version: 0, course: null, progress: [] });
  api.videoContext.mockResolvedValue(null);
  api.musicContext.mockResolvedValue(null);
  api.videoPlayback.mockImplementation(async () => { const video = await api.videoContext(); return video ? { ...video, hls: false } : null; });
  api.videoDraft.mockResolvedValue({ draft: null });
  api.tutorState.mockResolvedValue({ control: { enabled: true, revision: 0, current_tutor_session_id: null }, sessions: {} });
  api.tutorReadiness.mockResolvedValue({ status: 'ready', source_id: 'startup', source_revision: 's1', limitations: [], reason: '可用',
    teaching_assets: { status: 'stale', teaching_map_revision: null, limitations: [], reason: '教学资料构建失败' } });
  api.desktopStatus.mockResolvedValue({ desktop_host: false, reader_only: true, active_book: true });
  api.buildWorkbench.mockImplementation(async () => ({ book_id: book, readiness: { route: "reader" },
    input: { manifest: null }, source_review: { unresolved: [] } }));
  api.manifest.mockResolvedValue({ tree: [
    { lid: "1", kind: "chapter", children: ["1.1"], span: { start: 0, end: 22 }, title: "# 首章" },
    { lid: "1.1", kind: "paragraph", children: [], span: { start: 5, end: 22 } },
    { lid: "2", kind: "chapter", children: ["2.1"], span: { start: 22, end: 40 }, title: "# 后章" },
  ], stats_by_lid: {} });
  api.assetManifest.mockResolvedValue({ images: [] });
  api.sourceFingerprint.mockImplementation(async () => ({ book_id: book, source_fingerprint: book }));
  api.sourceManifest.mockResolvedValue(null);
  api.state.mockImplementation(async () => ({ book_id: book, revision: 0,
    viewport: { anchor_lid: "1.1", top_lid: "1.1", bottom_lid: "1.1", width: 1, visible_lids: ["1.1"] },
    profile: { profile_id: "technical_learning", profile_version: "1" }, layout: { rev: 0, open_slots: [] } }));
  api.profileManifest.mockResolvedValue({ profile_id: "technical_learning", slots: [] });
  api.text.mockImplementation(async (lid: string) => ({ lid, text: lid === "1.1" ? "可立即阅读的正文" : "# 首章" }));
  api.recall.mockResolvedValue([]);
  api.agentHistory.mockImplementation(async () => history(book));
  api.closedBook.mockResolvedValue(null);
  api.learningTasks.mockResolvedValue({ tasks: [] });
  api.selectLearningTask.mockResolvedValue({});
  api.profileMemory.mockResolvedValue({});
  api.profileBackfill.mockResolvedValue({ jobs: [], sessions: [] });
  api.intentArtifacts.mockResolvedValue({ overlay: null });
  api.intentUsageEvent.mockResolvedValue({});
  api.bookLibrary.mockResolvedValue({ root: "books", books: [] });
  api.openBook.mockImplementation(async () => { book = "second"; return { ok: true, book_id: book }; });
});

it('submits the frozen board target without attaching the reader selection and only accepts after admission', async () => {
  const admission = deferred<unknown>();
  api.agentRunCreate.mockReturnValue(admission.promise);
  api.agentRun.mockReturnValue(new Promise(() => {}));
  const w = startPurpose(); await flushPromises();
  w.findComponent({ name: 'RightRail' }).vm.$emit('open-board-location', { board_id: 'board-A', item_id: 'point-A' });
  await flushPromises();
  const board = w.findComponent({ name: 'BoardWorkspace' });
  const context = { board_id: 'board-A', board_revision: 7, item_id: 'point-A', block_id: 'block-A',
    presentation: { session_id: 'old-chat', turn_id: 'old-turn', reference: { presentation_id: 'scene', revision: 2 }, state_revision: 3, saved_state_ref: 'saved-3' } };
  board.vm.$emit('follow-up', { client_request_id: 'board-question', message: '为什么慢段权重更大？', context });
  await flushPromises();
  expect(api.agentRunCreate).toHaveBeenCalledWith('为什么慢段权重更大？', expect.objectContaining({
    client_request_id: 'board-question', session_id: 'startup-chat', board_follow_up: context, question_anchor_lid: null, question_quote: null,
    expected_chat_head: null,
  }));
  const meta = api.agentRunCreate.mock.calls[0][1];
  expect(meta.presentation_follow_up).toBeUndefined(); expect(meta.music_question).toBeUndefined();
  expect(meta.video_draft_id).toBeUndefined();
  expect(board.props('run')).toMatchObject({ request_id: 'board-question', accepted: false, pending: true, item_id: 'point-A' });
  admission.resolve({ book_id: 'startup', session_id: 'startup-chat', turn_id: 'accepted-board-turn' });
  await flushPromises();
  expect(board.props('run')).toMatchObject({ accepted: true, turn_id: 'accepted-board-turn', board_id: 'board-A', item_id: 'point-A' });
});

it('recovers board delivery and exact navigation from persisted history', async () => {
  const context = { board_id: 'board-A', board_revision: 7, item_id: 'point-A' };
  const delivery = { board_id: 'board-A', before_revision: 8, after_revision: 9, item_ids: ['answer-A'], block_ids: ['block-answer-A'],
    origin_session_id: 'startup-chat', origin_turn_id: 'saved-turn', parent_item_id: 'point-A' };
  api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current, turns: [{
    turn_id: 'saved-turn', user: '之前的问题', status: 'completed', effect_labels: [], board_follow_up: context, board_delivery: delivery,
    outcome: { answer: '完整回答', effects: [] },
  }] } });
  const w = startPurpose(); await flushPromises();
  const rail = w.findComponent({ name: 'RightRail' });
  expect(rail.props('chat')[0]).toMatchObject({ boardFollowUp: context, boardDelivery: delivery });
  rail.vm.$emit('open-board-location', { board_id: 'board-A', item_id: 'answer-A', block_id: 'block-answer-A' });
  await flushPromises();
  expect(w.findComponent({ name: 'BoardWorkspace' }).props('pendingLocate')).toEqual({ board_id: 'board-A', item_id: 'answer-A', block_id: 'block-answer-A' });
  expect(w.findComponent({ name: 'BoardWorkspace' }).props('run')).toMatchObject({ accepted: true, pending: false, delivery });
  expect(new URL(location.href).searchParams.get('board_block_id')).toBe('block-answer-A');
  window.history.replaceState({}, '', '/');
});

it('submits a whole-board local question with the actual chat head and retries rejection inside the board', async () => {
  vi.stubGlobal('EventSource', class extends EventTarget { close() {} });
  const { ApiError } = await import('@understand-book/web/reader/api');
  const previous = { turn_id: 'actual-head', user: '之前的讨论', status: 'completed', effect_labels: [], outcome: { answer: '先前回答', effects: [] } };
  api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current, turns: [previous] } });
  api.agentRunCreate.mockRejectedValueOnce(new ApiError(409, 'CHAT_SELECTION_CHANGED', 'conflict', '讨论已更新，问题仍保留在画板。请重试以继续当前讨论。'));
  api.agentRunCreate.mockResolvedValueOnce({ book_id: 'startup', session_id: 'startup-chat', turn_id: 'retried-board-turn' });
  api.agentRun.mockReturnValue(new Promise(() => {}));
  const w = startPurpose(); await flushPromises();
  w.findComponent({ name: 'RightRail' }).vm.$emit('open-board-location', { board_id: 'board-A' }); await flushPromises();
  const board = w.findComponent({ name: 'BoardWorkspace' });
  const context = { board_id: 'board-A', board_revision: 1 };
  board.vm.$emit('follow-up', { client_request_id: 'whole-board-question', message: '帮我画出状态与上下文的关系', context }); await flushPromises();
  expect(api.agentRunCreate).toHaveBeenCalledWith('帮我画出状态与上下文的关系', expect.objectContaining({ expected_chat_head: 'actual-head', board_follow_up: context }));
  expect(board.props('run')).toMatchObject({ accepted: false, pending: false, can_retry: true, question: '帮我画出状态与上下文的关系' });
  board.vm.$emit('retry-follow-up', 'whole-board-question'); await flushPromises();
  expect(api.agentRunCreate).toHaveBeenCalledTimes(2);
  expect(board.props('run')).toMatchObject({ accepted: true, pending: true, turn_id: 'retried-board-turn' });
  expect(board.props('run').error).toBeUndefined();
  expect(board.props('run').discussion).toMatchObject({ sessionId: 'startup-chat', turnId: 'retried-board-turn', user: '帮我画出状态与上下文的关系' });
  window.history.replaceState({}, '', '/');
});

it('retries saving the original board answer without sending another model question', async () => {
  const context = { board_id: 'board-A', board_revision: 7, item_id: 'point-A' };
  const turn = { turn_id: 'saved-turn', user: '之前的问题', status: 'completed', effect_labels: [], board_follow_up: context,
    board_delivery_error: { error_code: 'IO_ERROR', category: 'internal', message: '暂时无法写入画板' }, outcome: { answer: '完整回答', effects: [] } };
  api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current, turns: [turn] } });
  const w = startPurpose(); await flushPromises();
  w.findComponent({ name: 'RightRail' }).vm.$emit('open-board-location', { board_id: 'board-A', item_id: 'point-A' });
  await flushPromises();
  const board = w.findComponent({ name: 'BoardWorkspace' });
  expect(board.props('run')).toMatchObject({ accepted: true, can_retry: true, error: '这次回答未能完成，请重试。' });
  const delivery = { board_id: 'board-A', before_revision: 7, after_revision: 8, item_ids: ['answer-A'], block_ids: ['block-answer-A'],
    origin_session_id: 'startup-chat', origin_turn_id: 'saved-turn', parent_item_id: 'point-A' };
  api.agentRunRetrySave.mockResolvedValue({ persistence_state: 'saved' });
  api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current, turns: [{ ...turn, board_delivery_error: null, board_delivery: delivery }] } });
  board.vm.$emit('retry-follow-up', board.props('run').request_id); await flushPromises();
  expect(api.agentRunRetrySave).toHaveBeenCalledWith('saved-turn');
  expect(api.agentRunCreate).not.toHaveBeenCalled();
  expect(board.props('run')).toMatchObject({ pending: false, delivery, can_retry: false });
  expect(board.props('run').error).toBeUndefined();
  window.history.replaceState({}, '', '/');
});

it.each([false, true])('understanding conversation navigation preserves the draft and only scrolls after successful selection (failure=%s)', async failure => {
  const scroll = vi.fn().mockResolvedValue(true), onUnavailable = vi.fn();
  const w = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: {
    ReaderWorkspace: defineComponent({ setup(_, { expose }) { expose({ showAssistant: vi.fn() }); return {}; }, template: '<div><slot /><slot name="reader" /><slot name="assistant" /></div>' }),
    RightRail: defineComponent({ name: 'RightRail', props: ['agentInput'], setup(_, { expose }) { expose({ scrollToTurn: scroll }); return {}; }, template: '<div />' }),
  } } });
  wrappers.push(w); await flushPromises();
  const rail = w.getComponent({ name: 'RightRail' }); rail.vm.$emit('update:agentInput', '还没发送的问题'); await flushPromises();
  const location = { session_id: 'original-chat', turn_id: 'original-turn', reader_key: readerKey(), onUnavailable };
  window.dispatchEvent(new CustomEvent('understanding-open-chat', { detail: { ...location, reader_key: 'different-workspace' } }));
  await flushPromises(); expect(api.agentHistorySelect).not.toHaveBeenCalled();
  if (failure) api.agentHistorySelect.mockRejectedValueOnce(new Error('聊天已删除'));
  else api.agentHistorySelect.mockResolvedValueOnce({ ...history(), active_session_id: 'original-chat', current: { ...history().current, id: 'original-chat', turns: [{ turn_id: 'original-turn', user: '原问题', status: 'completed', effect_labels: [], outcome: null }] } });
  window.dispatchEvent(new CustomEvent('understanding-open-chat', { detail: location })); await flushPromises();
  expect(api.agentHistorySelect).toHaveBeenCalledWith('original-chat');
  expect(rail.props('agentInput')).toBe('还没发送的问题');
  if (failure) { expect(scroll).not.toHaveBeenCalled(); expect(onUnavailable).toHaveBeenCalledWith('对应对话暂时无法打开，请稍后重试。'); }
  else { expect(scroll).toHaveBeenCalledWith('original-turn'); expect(onUnavailable).not.toHaveBeenCalled(); }
});

it.each(['history', 'submission'])('shows the public failure message without internal error codes in %s', async source => {
  const message = '当前服务暂不可用，请稍后重试。';
  if (source === 'history') api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current,
    turns: [{ turn_id: 'failed', user: '原问题', status: 'failed', effect_labels: [],
      error: { category: 'conflict', error_code: 'SERVICE_UNAVAILABLE', message } }] } });
  else {
    const { ApiError } = await import('@understand-book/web/reader/api');
    api.agentRunCreate.mockRejectedValueOnce(new ApiError(409, 'SERVICE_UNAVAILABLE', 'conflict', message));
  }
  const wrapper = start(); await flushPromises();
  const rail = wrapper.getComponent({ name: 'RightRail' });
  if (source === 'submission') {
    rail.vm.$emit('update:agentInput', '原问题');
    rail.vm.$emit('send-agent'); await flushPromises();
  }
  expect(rail.props('chat')[0].error).toBe('这次回答未能完成，请重试。');
});

it('shows an actionable review failure after loading a failed teaching turn', async () => {
  api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current,
    turns: [{ turn_id: 'embedding-review', user: 'Embedding具体会被怎么用呢，给我一个演示页看看', status: 'failed', effect_labels: [],
      error: { category: 'provider', error_code: 'TEACHING_REVIEW_FAILED', message: 'PROVIDER_ERROR: finish_reason=length' },
      run_summary: { effects: [], activities: [], trace: [{ tool: 'presentation.author', args: '{"operation":"deliver"}', result_digest: '{"status":"version_saved"}' }] } }] } });
  const wrapper = start(); await flushPromises();
  expect(wrapper.getComponent({ name: 'RightRail' }).props('chat')[0].error).toBe('演示页已生成，但回答检查未完成。请重试。');
});

it('reads prose without formula explanations and keeps the dialog usable while explanations wait or fail', async () => {
  api.manifest.mockResolvedValue({ tree: [{ lid: '1.1', kind: 'formula', children: [], span: { start: 0, end: 5 } }], stats_by_lid: {} });
  api.text.mockResolvedValue({ lid: '1.1', text: '$x^2$' });
  const pending = deferred<unknown>(); api.formulaSemantics.mockReturnValue(pending.promise);
  const w = start(); await flushPromises();
  const reader = w.getComponent({ name: 'ReaderPane' });
  expect(reader.props('segments')[0].text).toBe('$x^2$');
  expect(api.formulaSemantics).not.toHaveBeenCalled();
  reader.vm.$emit('open-formula', reader.props('segments')[0]); await flushPromises();
  expect(w.get('.formula-dialog').text()).toContain('正在读取公式解释');
  pending.reject(new Error('解释暂不可用')); await flushPromises();
  expect(w.get('.formula-dialog').text()).toContain('解释暂不可用');
  api.formulaSemantics.mockResolvedValue(null);
  await w.get('.formula-dialog [role="alert"] button').trigger('click'); await flushPromises();
  expect(w.get('.formula-dialog').text()).toContain('暂时没有解释资料');
  expect(reader.props('segments')[0].text).toBe('$x^2$');
});

it('loads the selected formula panel on demand after body hydration', async () => {
  api.manifest.mockResolvedValue({ tree: [{ lid: '1.1', kind: 'formula', children: [], span: { start: 0, end: 5 } }], stats_by_lid: {} });
  api.text.mockResolvedValue({ lid: '1.1', text: '$x^2$' });
  const formula = { formula_lid: '1.1', composition: { meaning: '平方' }, parameters: [], context_links: [] };
  api.formulaSemantics.mockResolvedValue(formula);
  const w = start(); await flushPromises();
  expect(api.formulaSemantics).not.toHaveBeenCalled();
  const rail = w.getComponent({ name: 'RightRail' });
  rail.vm.$emit('request-formula'); await flushPromises();
  expect(api.formulaSemantics).toHaveBeenCalledWith('1.1');
  expect(rail.props('selectedFormula')).toEqual(formula);
});

it('shows learning preferences without the removed teaching switch or an automatic question', async () => {
  api.tutorState.mockResolvedValue({ control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} });
  const w = mount(App, { shallow: true, global: { renderStubDefaultSlot:true, stubs: { RightRail: defineComponent({ template: '<div><slot name="tutor-control" /></div>' }) } } });
  wrappers.push(w);
  await flushPromises();
  expect(w.findComponent({ name: 'TutorControl' }).exists()).toBe(false);
  expect(w.text()).toContain('阅读时提议练习');
  expect(w.text()).toContain('整理我的学习脉络');
  expect(api.tutorMutate).not.toHaveBeenCalled();
  expect(api.tutorStart).not.toHaveBeenCalled();
  expect(api.agentRunCreate).not.toHaveBeenCalled();
});

it('shows material titles while opening the original directory', async () => {
  api.bookLibrary.mockResolvedValue({ root: 'books', books: [
    { name: 'paper-slug', title: '材料的完整标题', book_id: 'paper-slug', dir: 'books/paper-slug', route: 'reader' },
    { name: 'paper-second-edition', title: '材料的完整标题', book_id: 'paper-second', dir: 'books/paper-second-edition', route: 'reader' },
  ] });
  const w = start(); await flushPromises();
  w.getComponent({ name: 'TopBar' }).vm.$emit('open-book'); await flushPromises();
  expect(w.get('.book-picker-card strong').text()).toBe('材料的完整标题');
  expect(w.get('.book-picker-location').attributes('open')).toBeUndefined();
  expect(w.findAll('.book-picker-variant').map(item => item.text())).toEqual(['paper-slug', 'paper-second-edition']);
  await w.findAll('.book-picker-card')[1].trigger('click');
  await w.get('.book-picker-actions .primary-action').trigger('click'); await flushPromises();
  expect(api.openBook).toHaveBeenCalledWith('books/paper-second-edition');
});

it('uses the native model settings commands without exposing saved credentials', async () => {
  api.desktopStatus.mockResolvedValue({ desktop_host: true, reader_only: false, active_book: true, library_root_available: true });
  vi.stubGlobal('__TAURI_INTERNALS__', { invoke: (command: string, args?: Record<string, unknown>) => invoke(command, args) });
  const provider: ModelSettingsStatus = { configured: true, source: 'settings', settings: {
    revision: 3,
    services: [{ id: 'service-a', name: 'Reading service', base_url: 'https://provider.example/v1', auth: 'bearer', api_key_configured: true, headers: {}, deployment: { kind: 'direct' } }],
    models: [{ id: 'model-a', name: 'Reading model', service_id: 'service-a', model_id: 'existing-model', api_protocol: 'openai_chat_completions', mode: 'native', capabilities: { native_tools: true }, generation: {} }],
    bindings: { default_model: 'model-a' },
  } };
  vi.mocked(invoke).mockImplementation(async command => command === 'codex_plugin_status' ? { state: 'installed_externally' } : provider as never);
  const w = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: { ModelProviderSettings: false } } });
  wrappers.push(w); await flushPromises();
  w.getComponent({ name: 'TopBar' }).vm.$emit('open-settings'); await flushPromises();
  await vi.waitFor(() => expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toContain('desktop_model_settings'));
  await flushPromises();
  expect(w.get('[data-testid=model-settings]').text()).toContain('AI 模型');
  expect(w.get('.model-advanced').attributes('open')).toBeUndefined();
  expect((w.get('[data-testid=service-key]').element as HTMLInputElement).value).toBe('');
  await w.get('[data-testid=service-url]').setValue('https://example.test/v1');
  await w.get('[data-testid=model-id]').setValue('selected-model');
  await w.get('[data-testid=model-protocol]').setValue('anthropic_messages');
  await w.get('select:has(option[value=react])').setValue('react');
  await w.get('[data-testid=save-model-settings]').trigger('click'); await flushPromises();
  expect(invoke).toHaveBeenCalledWith('save_desktop_model_settings', { input: expect.objectContaining({
    revision: 3,
    services: [expect.objectContaining({ id: 'service-a', api_key: '', base_url: 'https://example.test/v1' })],
    models: [expect.objectContaining({ id: 'model-a', model_id: 'selected-model', api_protocol: 'anthropic_messages', mode: 'react' })],
    bindings: { default_model: 'model-a' },
  }) });
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'test_desktop_model')).toBe(false);
});

it('shows provider settings while the native plugin query is still pending', async () => {
  api.desktopStatus.mockResolvedValue({ desktop_host: true, reader_only: false, active_book: true });
  vi.stubGlobal('__TAURI_INTERNALS__', { invoke: (command: string, args?: Record<string, unknown>) => invoke(command, args) });
  const plugin = deferred<unknown>();
  const provider: ModelSettingsStatus = { configured: false, source: 'unconfigured', settings: null };
  vi.mocked(invoke).mockImplementation(async command => command === 'codex_plugin_status' ? await plugin.promise : provider as never);
  const w = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: { ModelProviderSettings: false } } });
  wrappers.push(w); await flushPromises();
  w.getComponent({ name: 'TopBar' }).vm.$emit('open-settings'); await flushPromises();
  const settings = w.findAll('.desktop-settings-section');
  const connection = w.get('[data-testid=model-settings]');
  const codex = settings.find(section => section.text().includes('Codex 预构建插件'))!;
  await vi.waitFor(() => expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toContain('desktop_model_settings'));
  await flushPromises();
  expect(connection.text()).toContain('尚未配置');
  expect(connection.text()).not.toContain('正在读取');
  expect(codex.text()).toContain('正在检测');
  expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toContain('desktop_model_settings');
  plugin.resolve({ state: 'codex_not_found' }); await flushPromises();
  expect(codex.text()).toContain('请先安装 Codex');
});

it.each([
  ['pending_configuration', '需要安装插件', '在 Codex 的插件目录中安装 Understand Book，然后回到这里刷新。'],
  ['codex_not_found', '请先安装 Codex', '请先安装并启动 Codex，再回到这里刷新检测。'],
  ['error', '需要处理', 'Plugin inventory unavailable'],
])('guides external desktop setup and refreshes without installing when %s', async (state, label, message) => {
  api.desktopStatus.mockResolvedValue({ desktop_host: true, reader_only: false, active_book: true });
  vi.stubGlobal('__TAURI_INTERNALS__', {});
  const provider = { configured: false, source: 'unconfigured', mode: 'native', base_url: '', model: '', api_key_configured: false };
  vi.mocked(invoke).mockImplementation(async command => command === 'codex_plugin_status' ? { state, message: state === 'error' ? message : 'Internal deployment details' } : provider as never);
  const w = start(); await flushPromises();
  w.getComponent({ name: 'TopBar' }).vm.$emit('open-settings'); await flushPromises();
  const plugin = w.findAll('.desktop-settings-section').find(section => section.text().includes('Codex 预构建插件'))!;
  expect(plugin.text()).toContain(label);
  expect(plugin.text()).toContain(message);
  expect(plugin.text()).not.toContain('Internal deployment details');
  expect(plugin.text()).not.toContain('安装或重试');
  await plugin.get('button').trigger('click'); await flushPromises();
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === 'codex_plugin_status')).toHaveLength(2);
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'install_codex_plugin')).toBe(false);
});

it.each([false,true])('continues a saved goal only after its resume is confirmed (resume failure: %s)', async failResume => {
  const session = { id:'learning',revision:1,status:'paused',user_intent:'理解原文',explicit_constraints:[],
    current_focus:{interpretation:'',target_object_refs:[],capability_targets:[]},
    material_scope:[{source_id:'startup',scope_refs:[],role:'primary'}],default_teaching_intent:null,path_instance_ref:null,progress_ref:null };
  const state = {control:{enabled:false,revision:1,current_tutor_session_id:'learning'},sessions:{learning:session}};
  api.tutorState.mockImplementation(async()=>structuredClone(state));
  api.tutorMutate.mockImplementation(async request => {
    if (request.action.kind === 'resume') {
      if (failResume) throw new Error('继续学习暂时失败');
      state.sessions.learning.status = 'active';
    }
    state.control.revision++; return structuredClone(state);
  });
  api.tutorStart.mockResolvedValue({started:false,reason:'测试已记录继续操作'});
  const w = mount(App,{shallow:true,global:{renderStubDefaultSlot:true,stubs:{RightRail:defineComponent({template:'<div><slot name="learning-goal" /></div>'})}}});
  wrappers.push(w); await flushPromises();
  w.findComponent({name:'ConversationLearningGoal'}).vm.$emit('resume'); await flushPromises();
  expect(api.tutorMutate.mock.calls.map(([request])=>request.action.kind)).toEqual(['resume']);
  expect(api.tutorStart).toHaveBeenCalledTimes(failResume ? 0 : 1);
  expect(api.agentRunCreate).not.toHaveBeenCalled();
  expect(w.getComponent({name:'ConversationLearningGoal'}).props('session').status).toBe(failResume ? 'paused' : 'active');
  if (failResume) expect(w.getComponent({name:'ConversationLearningGoal'}).props('error')).toContain('继续学习暂时失败');
});

it('does not continue the previous book goal when its resume finishes after switching books', async () => {
  const session = (source: string) => ({ id: `${source}-learning`, revision: 1, status: 'paused', user_intent: `${source}的目标`,
    explicit_constraints: [], current_focus: { interpretation: '', target_object_refs: [], capability_targets: [] },
    material_scope: [{ source_id: source, scope_refs: [], role: 'primary' }], default_teaching_intent: null, path_instance_ref: null, progress_ref: null });
  const state = (source: string, enabled: boolean) => ({ control: { enabled, revision: 1, current_tutor_session_id: `${source}-learning` },
    sessions: { [`${source}-learning`]: session(source) } });
  api.tutorState.mockImplementation(async () => state(book, book === 'second'));
  api.tutorReadiness.mockImplementation(async () => ({ status: 'ready', source_id: book, source_revision: 's1', limitations: [], reason: '可用',
    teaching_assets: { status: 'ready', teaching_map_revision: 's1', limitations: [], reason: '可用' } }));
  const resuming = deferred<ReturnType<typeof state>>();
  api.tutorMutate.mockReturnValueOnce(resuming.promise).mockResolvedValue(state('second', true));
  const w = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: {
    RightRail: defineComponent({ template: '<div><slot name="learning-goal" /></div>' }),
  } } }); wrappers.push(w); await flushPromises();
  w.getComponent({ name: 'ConversationLearningGoal' }).vm.$emit('resume'); await flushPromises();
  expect(api.tutorMutate.mock.calls.map(([request]) => request.action.kind)).toEqual(['resume']);
  w.getComponent({ name: 'TopBar' }).vm.$emit('open-book'); await flushPromises();
  await w.get('input[placeholder=".understand-book/book-id"]').setValue('second');
  await w.get('.book-picker-actions .primary-action').trigger('click'); await flushPromises();
  expect(w.getComponent({ name: 'ConversationLearningGoal' }).props('session').id).toBe('second-learning');
  resuming.resolve(state('startup', true)); await flushPromises();
  expect(api.tutorMutate.mock.calls.map(([request]) => request.action.kind)).toEqual(['resume']);
  expect(api.tutorStart).not.toHaveBeenCalled();
  expect(api.agentRunCreate).not.toHaveBeenCalled();
});

it.each(['off', 'active', 'paused', 'other'])(
  'submits one canonical chapter purpose request independently of the saved legacy teaching state (%s)', async mode => {
  const session = { id: 'learning', revision: 1, status: mode === 'paused' ? 'paused' : 'active', user_intent: '学习', explicit_constraints: [],
    current_focus: { interpretation: '', target_object_refs: [], capability_targets: [] },
    material_scope: [{ source_id: 'startup', scope_refs: [mode === 'other' ? '2' : '1'], role: 'primary' }],
    default_teaching_intent: null, path_instance_ref: null, progress_ref: null };
  const hasSession = ['active', 'paused', 'other'].includes(mode);
  const state = { control: { enabled: mode !== 'off', revision: 1, current_tutor_session_id: hasSession ? 'learning' : null },
    sessions: hasSession ? { learning: session } : {} };
  api.tutorState.mockImplementation(async () => structuredClone(state));
  api.agentRunCreate.mockResolvedValue({ answer: '章节导读', answer_view: { parts: [] }, effects: [] });
  const w = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: {
    ReaderWorkspace: defineComponent({ template: '<div><slot /></div>', setup(_props, { expose }) { expose({ showAssistant: () => {} }); } }),
  } } }); wrappers.push(w); await flushPromises();
  const rail = w.getComponent({ name: 'RightRail' });
  expect(rail.props('learningScopeOptions')).toContainEqual({ value: 'chapter', lid: '1', label: '当前章节 · 首章' });
  expect(api.agentRunCreate).not.toHaveBeenCalled();
  expect(api.tutorMutate).not.toHaveBeenCalled();
  rail.vm.$emit('learning-purpose', 'understand', 'chapter', { message: '我认识术语，但不知道用途' });
  rail.vm.$emit('learning-purpose', 'understand', 'chapter', { message: '我认识术语，但不知道用途' });
  await flushPromises();
  expect(api.tutorMutate).not.toHaveBeenCalled();
  expect(api.tutorStart).not.toHaveBeenCalled();
  expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
  expect(api.agentRunCreate).toHaveBeenCalledWith('我认识术语，但不知道用途', expect.objectContaining({
    entry_action: 'learn_with_purpose', learning_purpose: 'understand', chapter_lid: '1', source_id: 'startup', source_revision: 'startup', session_id: 'startup-chat',
    display_user: '带我读懂 · 当前章节 · 首章\n\n我认识术语，但不知道用途', question_quote: null,
  }));
});

it('preserves the current scope, purpose, teaching mode and constraints when editing a stage goal', async () => {
  const session = { id: 'learning', revision: 2, status: 'active', user_intent: '理解本章条件',
    explicit_constraints: ['先讲必要前提', '每次只做一道题'],
    current_focus: { interpretation: '', target_object_refs: [], capability_targets: [] },
    material_scope: [{ source_id: 'startup', scope_refs: ['1'], role: 'primary' }],
    learning_purpose: 'exam', default_teaching_intent: 'guided_inquiry', path_instance_ref: null, progress_ref: null };
  const state = { control: { enabled: true, revision: 2, current_tutor_session_id: 'learning' }, sessions: { learning: session } };
  api.tutorState.mockResolvedValue(state);
  api.tutorMutate.mockResolvedValue(state);
  const w = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: {
    RightRail: defineComponent({ template: '<div><slot name="learning-goal" /></div>' }),
  } } }); wrappers.push(w); await flushPromises();
  w.getComponent({ name: 'ConversationLearningGoal' }).vm.$emit('goal', '能解释条件并应用到新题');
  await flushPromises();
  expect(api.tutorMutate).toHaveBeenCalledTimes(1);
  expect(api.tutorMutate.mock.calls[0][0].action).toEqual({
    kind: 'start', user_intent: '能解释条件并应用到新题', explicit_constraints: session.explicit_constraints,
    material_scope: session.material_scope, learning_purpose: 'exam', default_teaching_intent: 'guided_inquiry',
  });
  expect(api.agentRunCreate).not.toHaveBeenCalled();
});

it.each(['discover', 'understand', 'exam', 'explore', 'practice'] as const)(
  'starts %s from the conversation with a captured chapter and one typed request', async purpose => {
  api.agentRunCreate.mockResolvedValue({ answer: '开始学习', answer_view: { parts: [] }, effects: [] });
  const w = startPurpose(); await flushPromises();
  w.getComponent({ name: 'RightRail' }).vm.$emit('learning-purpose', purpose);
  await flushPromises();
  expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
  expect(api.agentRunCreate).toHaveBeenCalledWith('', expect.objectContaining({
    entry_action: 'learn_with_purpose', learning_purpose: purpose, chapter_lid: '1',
    source_id: 'startup', source_revision: 'startup', session_id: 'startup-chat',
    client_request_id: expect.any(String), expected_chat_head: null, question_quote: null,
  }));
  expect(JSON.parse(JSON.stringify(api.agentRunCreate.mock.calls[0][1]))).not.toHaveProperty('question_anchor_lid');
  expect(api.tutorMutate).not.toHaveBeenCalled();
  expect(api.tutorStart).not.toHaveBeenCalled();
});

it.each(['understand', 'exam', 'explore', 'practice'] as const)('passes the actual %s input and subsection through the full App without consuming the chat draft', async purpose => {
  api.manifest.mockResolvedValue({ stats_by_lid: {}, tree: [
    { lid: '1', kind: 'chapter', title: '材料总标题', children: ['1.1', '1.2'], span: { start: 0, end: 100 } },
    { lid: '1.1', kind: 'section', title: '引言', children: ['1.1.1'], span: { start: 8, end: 60 } },
    { lid: '1.1.1', kind: 'paragraph', children: [], span: { start: 12, end: 60 } },
  ] });
  api.agentRunCreate.mockResolvedValue({ answer: '开始学习', effects: [] });
  const w = startPurpose({ message: '未发送的聊天草稿', quote: null, goalId: null }); await flushPromises();
  const rail = w.getComponent({ name: 'RightRail' });
  expect(rail.props('learningScopeOptions')).toEqual([
    { value: 'section', lid: '1.1', label: '当前小节 · 引言' }, { value: 'material', label: '整份材料' },
  ]);
  rail.vm.$emit('learning-purpose', purpose, 'section', { message: '趋势与套利有什么区别？' });
  await flushPromises();
  expect(api.agentRunCreate).toHaveBeenCalledWith('趋势与套利有什么区别？', expect.objectContaining({
    entry_action: 'learn_with_purpose', learning_purpose: purpose, chapter_lid: '1.1',
    display_user: expect.stringContaining('趋势与套利有什么区别？'), question_quote: null,
  }));
  expect(rail.props('agentInput')).toBe('未发送的聊天草稿');
});

it.each([undefined, 'material'] as const)(
  'captures the selected or explicit material scope without consuming the draft (scope=%s)', async scope => {
  const draft = { message: '还在编辑的问题', quote: { lid: '1.1', quote: '可立即阅读的正文' }, goalId: null };
  api.agentRunCreate.mockResolvedValue({ answer: '开始学习', answer_view: { parts: [] }, effects: [] });
  const w = startPurpose(draft); await flushPromises();
  const rail = w.getComponent({ name: 'RightRail' });
  rail.vm.$emit('learning-purpose', 'understand', scope); await flushPromises();
  expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
  const meta = api.agentRunCreate.mock.calls[0][1];
  expect(meta.chapter_lid).toBeUndefined();
  expect(meta.question_quote).toEqual(scope === 'material' ? null : draft.quote);
  if (scope === 'material') expect(JSON.parse(JSON.stringify(meta))).not.toHaveProperty('question_anchor_lid');
  else expect(meta.question_anchor_lid).toBe(draft.quote.lid);
  expect(meta).toMatchObject({ source_id: 'startup', source_revision: 'startup', learning_purpose: 'understand' });
  expect(rail.props('agentInput')).toBe(draft.message);
  expect(rail.props('askDraft')).toEqual(draft.quote);
});

it('lets discovery start while Tutor readiness is unavailable without enabling teaching', async () => {
  api.tutorState.mockResolvedValue({ control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} });
  api.tutorReadiness.mockResolvedValue({ status: 'unavailable', reason: '教学资料尚未就绪', limitations: [] });
  api.agentRunCreate.mockResolvedValue({ answer: '材料概览', answer_view: { parts: [] }, effects: [] });
  const w = startPurpose(); await flushPromises();
  w.getComponent({ name: 'RightRail' }).vm.$emit('learning-purpose', 'discover'); await flushPromises();
  expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
  expect(api.agentRunCreate).toHaveBeenCalledWith('', expect.objectContaining({ learning_purpose: 'discover' }));
  expect(api.tutorMutate).not.toHaveBeenCalled();
  expect(api.tutorStart).not.toHaveBeenCalled();
});

it('retains the question draft and blocks a learning purpose when readiness cannot be read', async () => {
  const draft = { message: '尚未发送的问题', quote: { lid: '1.1', quote: '可立即阅读的正文' }, goalId: null };
  api.tutorState.mockResolvedValue({ control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} });
  api.tutorReadiness.mockRejectedValue(new Error('当前材料状态暂不可读取'));
  const w = mount(App, { props: { chatDraft: draft }, shallow: true, global: { renderStubDefaultSlot: true, stubs: {
    ReaderWorkspace: learningWorkspaceStub,
    RightRail: defineComponent({ name: 'RightRail', props: ['agentInput', 'askDraft', 'learningPurposeReady', 'learningPurposeReason'], template: '<div><slot name="learning-goal" /></div>' }),
  } } });
  wrappers.push(w); await flushPromises();
  const rail = w.getComponent({ name: 'RightRail' });
  rail.vm.$emit('learning-purpose', 'exam'); await flushPromises();
  expect(api.tutorMutate).not.toHaveBeenCalled();
  expect(api.agentRunCreate).not.toHaveBeenCalled();
  expect(rail.props('agentInput')).toBe(draft.message);
  expect(rail.props('askDraft')).toEqual(draft.quote);
  expect(rail.props('learningPurposeReady')).toBe(false);
  expect(rail.props('learningPurposeReason')).toBe('学习状态暂时无法读取，请重试。');
  expect(w.getComponent({ name: 'ConversationLearningGoal' }).props('ready')).toBe(false);
});

it('locks purpose submission while pending and accepts one changed purpose after completion', async () => {
  const admission = deferred<unknown>();
  api.agentRunCreate.mockReturnValueOnce(admission.promise);
  const w = startPurpose(); await flushPromises();
  const rail = w.getComponent({ name: 'RightRail' });
  rail.vm.$emit('learning-purpose', 'understand'); await flushPromises();
  rail.vm.$emit('learning-purpose', 'understand');
  rail.vm.$emit('learning-purpose', 'exam'); await flushPromises();
  expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
  admission.resolve({ answer: '已开始读懂', answer_view: { parts: [] }, effects: [] }); await flushPromises();
  api.agentRunCreate.mockResolvedValueOnce({ answer: '改为备考', answer_view: { parts: [] }, effects: [] });
  rail.vm.$emit('learning-purpose', 'exam'); await flushPromises();
  expect(api.agentRunCreate).toHaveBeenCalledTimes(2);
  expect(api.agentRunCreate.mock.calls.map(([, meta]) => meta.learning_purpose)).toEqual(['understand', 'exam']);
  expect(api.agentRunCreate.mock.calls[0][1].client_request_id).not.toBe(api.agentRunCreate.mock.calls[1][1].client_request_id);
});

function installLearningMedia(kind: 'video' | 'music') {
  const publication = { book_id: book, publication_id: `${kind}-publication` };
  const video = { media_id: 'lesson', media_revision: '1' };
  const videoQuestion = { draft_id: 'selected-video', publication,
    material: { book_id: book, video, transcript: { transcript_id: 'transcript', transcript_revision: '1', video } },
    frame: { video, actual_pts_ms: 35000, width: 1920, height: 1080 }, region: null,
    range: { video, start_ms: 30000, end_ms: 40000 } };
  const musicQuestion = { draft_id: 'selected-music', publication,
    audio: { recording: { recording_id: 'recording', revision: '1' }, start_ms: 12000, end_ms: 18000 },
    score: null, occurrence_id: 'occurrence-2', focused_part_ids: ['violin'] };
  if (kind === 'video') api.videoContext.mockResolvedValue({ time_base: [1, 1000], publication, material: videoQuestion.material,
    source: { video, original_path: 'lesson.mp4', duration_ms: 90000, width: 1920, height: 1080 }, map: { segments: [] } });
  else api.musicContext.mockResolvedValue({ publication, manifest: { title: 'Music', coverage: [{ status: 'accepted' }] }, audio_url: '/music.wav' });
  const captureQuestion = vi.fn(() => kind === 'video' ? videoQuestion : musicQuestion);
  const paneName = kind === 'video' ? 'VideoReaderPane' : 'MusicReaderPane';
  const pane = defineComponent({ name: paneName, template: '<div />', setup(_props, { expose }) { expose({ captureQuestion }); } });
  return { videoQuestion, musicQuestion, captureQuestion, paneName, stubs: { [paneName]: pane } };
}

it.each(['video', 'music'] as const)('starts %s learning at its real selected position and preserves the unsubmitted question', async kind => {
  const fixture = installLearningMedia(kind);
  const state = { control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} };
  api.tutorState.mockImplementation(async () => structuredClone(state));
  api.agentRunCreate.mockResolvedValue({ answer: '媒体教学', answer_view: { parts: [] }, effects: [] });
  const w = startPurpose({ message: '尚未发送的提问', quote: null, goalId: null }, fixture.stubs); await flushPromises();
  const pane = w.getComponent({ name: fixture.paneName });
  if (kind === 'video') pane.vm.$emit('select-range', fixture.videoQuestion);
  else pane.vm.$emit('question', fixture.musicQuestion);
  await flushPromises();
  const rail = w.getComponent({ name: 'RightRail' });
  expect(rail.props('learningPurposeAvailable')).toBe(true);
  expect(rail.props('learningPurposeReady')).toBe(true);
  expect(rail.props('learningScopeOptions').map((scope: { value: string }) => scope.value)).toEqual(['media', 'material']);
  rail.vm.$emit('learning-purpose', 'understand', 'media'); await flushPromises();
  expect(api.tutorMutate).not.toHaveBeenCalled();
  expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
  const meta = api.agentRunCreate.mock.calls[0][1];
  expect(meta).toMatchObject({ entry_action: 'learn_with_purpose', learning_purpose: 'understand', source_id: book, source_revision: book });
  expect(meta.chapter_lid).toBeUndefined();
  expect(meta.question_quote).toBeNull();
  if (kind === 'video') {
    const saved = api.videoDraft.mock.calls.map(([request]) => request).find(request => request.window_id.includes(':learning:'));
    expect(saved.draft.context).toMatchObject({ frame: fixture.videoQuestion.frame, range: fixture.videoQuestion.range });
    expect(saved.draft.context.draft_id).not.toBe(fixture.videoQuestion.draft_id);
    expect(meta).toMatchObject({ video_window_id: saved.window_id, video_draft_id: saved.draft.context.draft_id });
    expect(rail.props('videoQuestion').draft_id).toBe(fixture.videoQuestion.draft_id);
  } else {
    expect(meta.music_question).toMatchObject({ audio: fixture.musicQuestion.audio, occurrence_id: 'occurrence-2', focused_part_ids: ['violin'] });
    expect(rail.props('musicQuestion').draft_id).toBe(fixture.musicQuestion.draft_id);
  }
  expect(rail.props('agentInput')).toBe('尚未发送的提问');
});

it.each(['video', 'music'] as const)('allows a whole-material %s overview while teaching prepares, without attaching the selected fragment', async kind => {
  const fixture = installLearningMedia(kind);
  api.tutorReadiness.mockResolvedValue({ status: 'preparing', source_id: book, source_revision: book, reason: '字幕或谱面仍在准备', limitations: [] });
  api.agentRunCreate.mockResolvedValue({ answer: '整体概览', answer_view: { parts: [] }, effects: [] });
  const w = startPurpose(undefined, fixture.stubs); await flushPromises();
  const pane = w.getComponent({ name: fixture.paneName });
  pane.vm.$emit(kind === 'video' ? 'select-range' : 'question', kind === 'video' ? fixture.videoQuestion : fixture.musicQuestion);
  await flushPromises(); api.videoDraft.mockClear();
  const rail = w.getComponent({ name: 'RightRail' });
  expect(rail.props('learningPurposeAvailable')).toBe(true);
  expect(rail.props('learningPurposeReady')).toBe(false);
  expect(rail.props('learningPurposeReason')).toBe('字幕或谱面仍在准备');
  rail.vm.$emit('learning-purpose', 'understand', 'material'); await flushPromises();
  expect(api.agentRunCreate).not.toHaveBeenCalled();
  rail.vm.$emit('learning-purpose', 'discover', 'material'); await flushPromises();
  expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
  const meta = api.agentRunCreate.mock.calls[0][1];
  for (const field of ['chapter_lid', 'video_draft_id', 'video_window_id', 'music_question', 'question_anchor_lid']) expect(meta[field]).toBeUndefined();
  expect(meta.learning_purpose).toBe('discover');
  expect(api.tutorMutate).not.toHaveBeenCalled();
  expect(api.videoDraft).not.toHaveBeenCalled();
  expect(fixture.captureQuestion).not.toHaveBeenCalled();
});

it.each(['video', 'music'] as const)('saves a custom %s goal, then explicitly continues it with the actual media context', async kind => {
  const fixture = installLearningMedia(kind);
  const goal = kind === 'video' ? '我想搞懂 FP16 的显存核算，能解释为什么参数、梯度和优化器状态占用不同空间。'
    : '我想听懂这段音乐的主题与声部，能指出旋律如何从小提琴交给钢琴。';
  const continuation = '请继续本次学习，结合当前阅读位置和已有回应，选择一个有依据的教学动作。';
  const other = { id: 'other', revision: 1, status: 'paused', user_intent: '另一本书的目标', explicit_constraints: [],
    current_focus: { interpretation: '', target_object_refs: [], capability_targets: [] },
    material_scope: [{ source_id: 'other-book', scope_refs: [], role: 'primary' }], default_teaching_intent: null, path_instance_ref: null, progress_ref: null };
  const state = { control: { enabled: false, revision: 1, current_tutor_session_id: null as string | null }, sessions: { other } as Record<string, typeof other> };
  api.tutorState.mockImplementation(async () => structuredClone(state));
  api.tutorMutate.mockImplementation(async request => {
    const action = request.action;
    if (action.kind === 'start') {
      state.sessions.media = { ...other, id: 'media', status: 'active', user_intent: action.user_intent, material_scope: action.material_scope };
      state.control.current_tutor_session_id = 'media';
    } else if (action.kind === 'resume') state.sessions.media.revision++;
    state.control.revision++;
    return structuredClone(state);
  });
  api.tutorStart.mockResolvedValue({ started: true, message: continuation });
  api.agentRunCreate.mockResolvedValue({ answer: '第一步教学', answer_view: { parts: [] }, effects: [] });
  const w = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: {
    ...fixture.stubs, ReaderWorkspace: learningWorkspaceStub, ConversationLearningGoal: false,
    RightRail: defineComponent({ name: 'RightRail', template: '<div><slot name="learning-goal" /></div>' }),
  } } }); wrappers.push(w); await flushPromises();
  expect(w.get('.goal-empty').text()).toContain('还没有学习目标');
  expect(w.get('.conversation-learning-goal').text()).not.toContain(other.user_intent);
  await w.get('.goal-empty button').trigger('click');
  await w.get('[data-workspace-input="learning-goal"]').setValue(goal);
  await w.get('form[aria-label="调整目标"]').trigger('submit'); await flushPromises();
  expect(w.get('.goal-resume').text()).toContain('学习目标已保存');
  expect(api.agentRunCreate).not.toHaveBeenCalled();
  await w.get('.goal-resume button').trigger('click'); await flushPromises();
  expect(api.tutorMutate.mock.calls.map(([request]) => request.action.kind)).toEqual(['start', 'resume']);
  expect(api.tutorStart).toHaveBeenCalledTimes(1);
  expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
  expect(api.agentRunCreate.mock.calls[0][0]).toBe(`${continuation}\n\n本次学习目标：\n${goal}`);
  expect(api.agentRunCreate.mock.calls[0][0]).not.toContain(other.user_intent);
  expect(fixture.captureQuestion).toHaveBeenCalledTimes(1);
  const meta = api.agentRunCreate.mock.calls[0][1];
  if (kind === 'video') expect(meta.video_draft_id).toEqual(expect.any(String));
  else expect(meta.music_question).toMatchObject({ audio: fixture.musicQuestion.audio, focused_part_ids: ['violin'] });
  expect(meta.question_anchor_lid).toBeNull();
  expect(w.find('.goal-resume').exists()).toBe(false);
  const followup = '刚才第二步没听懂，请换一个例子。';
  const rail = w.getComponent({ name: 'RightRail' });
  rail.vm.$emit('update:agentInput', followup); await flushPromises();
  rail.vm.$emit('send-agent'); await flushPromises();
  expect(api.agentRunCreate).toHaveBeenCalledTimes(2);
  expect(api.agentRunCreate.mock.calls[1][0]).toBe(followup);
});

it('opens understanding directly from the chat without changing teaching or the draft', async () => {
  const w = mount(App,{props:{chatDraft:{message:'尚未发送',quote:null,goalId:null}},shallow:true,global:{renderStubDefaultSlot:true,
    stubs:{RightRail:defineComponent({name:'RightRail',props:['agentInput'],template:'<div><slot name="learning-actions" /></div>'})}}});
  wrappers.push(w); await flushPromises();
  await w.findAll('button').find(button=>button.text()==='我的理解')!.trigger('click');
  expect(w.findComponent({name:'UnderstandingMap'}).exists()).toBe(true);
  w.getComponent({name:'UnderstandingMap'}).vm.$emit('close'); await flushPromises();
  expect(w.getComponent({name:'RightRail'}).props('agentInput')).toBe('尚未发送');
  expect(api.tutorMutate).not.toHaveBeenCalled();
  expect(api.agentRunCreate).not.toHaveBeenCalled();
});
afterEach(() => {
  wrappers.splice(0).forEach(wrapper => wrapper.unmount());
  readNoteDrafts().forEach(discardNoteDraft);
  Object.assign(notesPanel, { open: false, view: 'library', draftId: null, reference: null });
  network.value = { ...network.value, enabled: false, workspace: null, identity: null };
  localStorage.clear(); sessionStorage.clear();
  submittedRunDrafts.clear();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("workspace startup", () => {
  it('opens a contiguous first window with one range read and preserves paragraph text', async () => {
    api.manifest.mockResolvedValue({ tree: [
      { lid: '1', kind: 'chapter', children: ['1.1', '1.2'], span: { start: 0, end: 8 }, title: '# 首章' },
      { lid: '1.1', kind: 'paragraph', children: [], span: { start: 0, end: 3 } },
      { lid: '1.2', kind: 'paragraph', children: [], span: { start: 5, end: 8 } },
    ], stats_by_lid: {} });
    const state = await api.state();
    api.state.mockResolvedValue({ ...state, viewport: { ...state.viewport, visible_lids: ['1.1', '1.2'], bottom_lid: '1.2' } });
    const text = deferred<{ lid: string; text: string }>();
    api.text.mockReturnValue(text.promise);
    const wrapper = start(); await flushPromises();
    expect(wrapper.find('.app-loading').exists()).toBe(true);
    expect(api.text.mock.calls).toEqual([['1.1', '1.2']]);
    text.resolve({ lid: '1.1', text: '甲😀\n\n乙丙丁' });
    await flushPromises();
    expect(wrapper.find('.app-loading').exists()).toBe(false);
    expect(wrapper.getComponent({ name: 'ReaderPane' }).props('segments').map((segment: { lid: string; text: string }) => ({ lid: segment.lid, text: segment.text }))).toEqual([
      { lid: '1.1', text: '甲😀' }, { lid: '1.2', text: '乙丙丁' },
    ]);
  });

  it.each(['new-chat', 'select-chat'])('unlocks %s as soon as history is ready, before profile refresh', async action => {
    installIdentity({ user_id: 'A', csrf_token: 'csrf' });
    installWorkspace({ workspace_id: 'w', generation: 1, revision: 1, selected_chat: 'startup-chat',
      published_book_ref: { book_id: book, publication_id: 'p' }, reader: await api.state() });
    const wrapper = start(); await flushPromises();
    const rail = wrapper.getComponent({ name: 'RightRail' });
    const response = deferred<ReturnType<typeof history>>();
    const profile = deferred<object>();
    api.agentNew.mockImplementation(async () => ({ ok: true, history: await response.promise }));
    api.agentHistorySelect.mockReturnValue(response.promise);
    api.profileMemory.mockReturnValueOnce(profile.promise);
    rail.vm.$emit(action, 'next-chat'); await flushPromises();
    expect(rail.props('historyLoading')).toBe(true);
    response.resolve({ ...history(), active_session_id: 'next-chat', current: { ...history().current, id: 'next-chat' } });
    await flushPromises();
    try {
      expect(rail.props('activeChatSessionId')).toBe('next-chat');
      expect(rail.props('historyLoading')).toBe(false);
      expect(rail.props('profileMemoryLoading')).toBe(true);
      expect(wrapper.getComponent({ name: 'TopBar' }).props('chatUnavailable')).toBe(false);
    } finally { profile.resolve({}); await flushPromises(); }
    expect(rail.props('profileMemoryLoading')).toBe(false);
    expect(rail.props('historyError')).toBeNull();
  });

  it("shows the text-free page animation until the first reading window is ready", async () => {
    const text = deferred<{ lid: string; text: string }>();
    api.text.mockReturnValue(text.promise);
    const wrapper = mount(App, { shallow: true, global: {
      renderStubDefaultSlot: true, stubs: { LoadingAnimation: false },
    } });
    wrappers.push(wrapper);
    await flushPromises();
    const loading = wrapper.get(".app-loading");
    expect(loading.text()).toBe("");
    expect(loading.get('[role="status"]').attributes("aria-label")).toBe("正在加载");
    expect(loading.find(".book-loading-turn").exists()).toBe(true);
    expect(wrapper.findComponent({ name: "ReaderPane" }).exists()).toBe(false);
    text.resolve({ lid: "1.1", text: "可立即阅读的正文" });
    await flushPromises();
    expect(wrapper.find(".app-loading").exists()).toBe(false);
    expect(wrapper.findComponent({ name: "ReaderPane" }).exists()).toBe(true);
  });

  it("removes the loading animation and exposes the error when material loading fails", async () => {
    api.manifest.mockRejectedValueOnce(new Error("material load failed"));
    const wrapper = start();
    await flushPromises();
    expect(wrapper.find(".app-loading").exists()).toBe(false);
    expect(wrapper.get(".banner").text()).toContain("material load failed");
  });

  it.each(['videoPlayback', 'manifest', 'assetManifest'])('automatically recovers a failed %s read without reopening or submitting a question', async failedRead => {
    installIdentity({ user_id: 'A', csrf_token: 'csrf' });
    installWorkspace({ workspace_id: 'w', generation: 2, revision: 3, selected_chat: 'startup-chat',
      published_book_ref: { book_id: book, publication_id: 'p' }, reader: await api.state() });
    api[failedRead].mockRejectedValueOnce(new TypeError('temporary connection failure'));
    api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current, turns: [{
      turn_id: 'saved', user: '之前的问题', status: 'completed', effect_labels: [], outcome: { answer: '已保存的回答', effects: [] },
    }] } });
    const requestRecovery = vi.fn();
    window.addEventListener('workspace-recovery-needed', requestRecovery);
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    try {
      const wrapper = start(); await flushPromises();
      const rail = wrapper.getComponent({ name: 'RightRail' });
      expect(rail.props('historyLoading')).toBe(false);
      expect(rail.props('historyError')).toContain('temporary connection failure');
      expect(requestRecovery).toHaveBeenCalledTimes(1);
      window.dispatchEvent(new Event('workspace-recovered'));
      await vi.advanceTimersByTimeAsync(1); await flushPromises();
      const restoredRail = wrapper.getComponent({ name: 'RightRail' });
      expect(restoredRail.props('historyLoading')).toBe(false);
      expect(restoredRail.props('historyError')).toBeNull();
      expect(restoredRail.props('chat')).toMatchObject([{ user: '之前的问题' }]);
      expect(wrapper.getComponent({ name: 'ReaderPane' }).props('segments')).toMatchObject([{ text: '可立即阅读的正文' }]);
      expect(wrapper.find('.banner').exists()).toBe(false);
      expect(api.agentRunCreate).not.toHaveBeenCalled();
    } finally {
      window.removeEventListener('workspace-recovery-needed', requestRecovery);
      vi.useRealTimers();
    }
  });

  it('requests automatic recovery when loading saved chat history fails transiently', async () => {
    installIdentity({ user_id: 'A', csrf_token: 'csrf' });
    installWorkspace({ workspace_id: 'w', generation: 2, revision: 3, selected_chat: 'startup-chat',
      published_book_ref: { book_id: book, publication_id: 'p' }, reader: await api.state() });
    api.agentHistory.mockRejectedValueOnce(new TypeError('history unavailable'));
    const requestRecovery = vi.fn();
    window.addEventListener('workspace-recovery-needed', requestRecovery);
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    try {
      const wrapper = start(); await flushPromises();
      expect(requestRecovery).toHaveBeenCalledTimes(1);
      expect(wrapper.getComponent({ name: 'ReaderPane' }).props('segments')).toHaveLength(1);
      window.dispatchEvent(new Event('workspace-recovered'));
      await vi.advanceTimersByTimeAsync(1); await flushPromises();
      expect(wrapper.getComponent({ name: 'RightRail' }).props('historyError')).toBeNull();
      expect(api.agentHistory).toHaveBeenCalledTimes(2);
      expect(api.manifest).toHaveBeenCalledTimes(1);
      expect(api.agentRunCreate).not.toHaveBeenCalled();
    } finally {
      window.removeEventListener('workspace-recovery-needed', requestRecovery);
      vi.useRealTimers();
    }
  });

  it('ADM8 keeps a submitted draft when first-chat admission remounts App before returning', async () => {
    const admission = deferred<unknown>();
    api.agentRunCreate.mockReturnValue(admission.promise);
    const old = mount(App, { props: { chatDraft: { message: '首次提问', quote: { lid: '1.1', quote: '原文' }, goalId: null } }, shallow: true, global: { renderStubDefaultSlot: true } });
    await flushPromises();
    old.findComponent({ name: 'RightRail' }).vm.$emit('send-agent'); await flushPromises();
    old.unmount();
    api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current, turns: [{
      turn_id: 'first-turn', user: '首次提问', status: 'failed', effect_labels: [],
      error: { category: 'model_spend', error_code: 'ALLOWANCE_INSUFFICIENT', message: 'server stop' },
    }] } });
    const current = start(); await flushPromises();
    admission.resolve({ book_id: 'startup', session_id: 'startup-chat', turn_id: 'first-turn' }); await flushPromises();
    const rail = current.findComponent({ name: 'RightRail' });
    expect(rail.props('agentInput')).toBe('首次提问');
    expect(rail.props('askDraft')).toEqual({ lid: '1.1', quote: '原文' });
    expect(submittedRunDrafts.size).toBe(0);
    expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
  });
  it.each(['saved', 'failed', 'new-draft'] as const)('ADM8 projects a real terminal snapshot (%s) and preserves the right draft', async state => {
    vi.stubGlobal('EventSource', class extends EventTarget { close() {} });
    const snapshot = deferred<unknown>();
    const descriptor = { book_id: 'startup', session_id: 'startup-chat', turn_id: 'stopped' };
    const spendError = { category: 'model_spend', error_code: 'ALLOWANCE_INSUFFICIENT', message: 'server stop' };
    const savedTurn = { turn_id: 'stopped', user: '待回答', status: 'failed', effect_labels: [], error: spendError };
    api.agentRunCreate.mockResolvedValue(descriptor);
    api.agentRun.mockReturnValue(snapshot.promise);
    const wrapper = mount(App, { props: { chatDraft: { message: '待回答', quote: null, goalId: null, presentationRequested: true } }, shallow: true, global: { renderStubDefaultSlot: true } });
    wrappers.push(wrapper); await flushPromises();
    api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current, turns: [savedTurn] } });
    const rail = wrapper.findComponent({ name: 'RightRail' });
    rail.vm.$emit('send-agent'); await flushPromises();
    expect(rail.props('agentInput')).toBe('');
    if (state === 'new-draft') rail.vm.$emit('update:agentInput', '等待期间写的新问题');
    snapshot.resolve({ descriptor, last_seq: 1, execution_state: 'failed', activities: [],
      persistence_state: state === 'failed' ? 'failed' : 'saved',
      final_view: state === 'failed' ? null : savedTurn,
      error: state === 'failed' ? { error_code: 'TURN_UNSAVED', message: 'save failed', execution_error: spendError } : null });
    await flushPromises();
    expect(rail.props('chat')[0].spendStop.message).toContain(state === 'failed' ? '尚未保存' : '已保存');
    expect(rail.props('agentInput')).toBe(state === 'new-draft' ? '等待期间写的新问题' : '待回答');
    expect(rail.props('presentationRequested')).toBe(state !== 'new-draft');
    expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
  });
  it('ADM8 restores a denied draft and quote, exposes the allowance action, and only explicitly continues the original goal', async () => {
    const { ApiError } = await import('@understand-book/web/reader/api');
    const draft = { message: '尚未回答的问题', quote: { lid: '1.1', quote: '原文' }, goalId: 'original-goal' };
    api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current,
      goals: [{ id: 'original-goal', status: 'open' }] } });
    api.agentRunCreate.mockRejectedValueOnce(new ApiError(409, 'ALLOWANCE_INSUFFICIENT', 'model_spend', 'denied'));
    const wrapper = mount(App, { props: { chatDraft: draft }, shallow: true, global: { renderStubDefaultSlot: true } });
    wrappers.push(wrapper); await flushPromises();
    const rail = wrapper.findComponent({ name: 'RightRail' });
    rail.vm.$emit('send-agent'); await flushPromises();
    expect(rail.props('agentInput')).toBe(draft.message);
    expect(rail.props('askDraft')).toEqual(draft.quote);
    expect(rail.props('chat')[0].spendStop.message).toContain('问题未提交');
    rail.vm.$emit('show-allowance'); await flushPromises();
    expect(wrapper.emitted('show-allowance')).toHaveLength(1);
    expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
    api.agentRunCreate.mockResolvedValueOnce({ answer: '继续结果', effects: [] });
    rail.vm.$emit('continue-goal', 'original-goal'); await flushPromises();
    expect(api.agentRunCreate).toHaveBeenLastCalledWith('继续这个任务', expect.objectContaining({ goal_id: 'original-goal' }));
    expect(rail.props('agentInput')).toBe(draft.message);
  });

  it('ADM8 recovers a saved spend stop from history using its code without claiming task completion', async () => {
    api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current, turns: [{
      turn_id: 'stopped', user: '问题', status: 'failed', effect_labels: [],
      error: { category: 'model_spend', error_code: 'ALLOWANCE_EXPIRED', message: 'server text' },
    }] } });
    const wrapper = start(); await flushPromises();
    const turn = wrapper.findComponent({ name: 'RightRail' }).props('chat')[0];
    expect(turn.spendStop.message).toContain('没有有效');
    expect(turn.spendStop.message).toContain('已保存');
    expect(turn.pending).toBe(false);
    expect(api.agentRunCreate).not.toHaveBeenCalled();
  });

  it("passes the table renderer and canonical focus marks to the reader", async () => {
    const text = "| 状态 | 归属 |\n| 笔记 | 读者 |";
    api.manifest.mockResolvedValue({ tree: [
      { lid: "1", kind: "chapter", children: ["1.1"], span: { start: 0, end: text.length }, title: "首章" },
      { lid: "1.1", kind: "table", children: [], span: { start: 0, end: text.length } },
    ], stats_by_lid: {} });
    api.text.mockImplementation(async (lid: string) => ({ lid, text }));
    api.recall.mockResolvedValue([{ mem_id: "table-highlight", type: "highlight", layer: "long_term", book_id: book,
      anchor: { lid: "1.1" }, content: "读者", range: { start: text.indexOf("读者"), end: text.indexOf("读者") + 2 } }]);
    const wrapper = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: {
      ReaderPane: false, BoardSourcePreview: false, BoardSourceReader: false, BoardSourceSegment: false, Teleport: true,
    } } });
    wrappers.push(wrapper); await flushPromises();
    const pane = wrapper.findComponent({ name: "ReaderPane" });
    const table = pane.props("segments").find((seg: { kind: string }) => seg.kind === "table");
    const root = document.createElement("div");
    root.innerHTML = pane.props("renderSeg")(table);
    expect(root.querySelectorAll("table tr")).toHaveLength(2);
    expect(root.querySelector("td mark.hl-mark")?.textContent).toBe("读者");
    expect(table.text).toBe(text);
    pane.vm.$emit('focus-source-local', { lid: "1.1", quote: "读者" });
    await flushPromises();
    expect(wrapper.findAll('.source-preview-dialog table tr')).toHaveLength(2);
    expect(wrapper.get('.source-preview-dialog td mark').text()).toBe('读者');
    expect(wrapper.find('.source-preview-dialog mark.hl-mark').exists()).toBe(false);
  });

  it('restores the unsent message and quote after the network scene remounts', async () => {
    const draft = { message: '尚未发送', quote: { lid: '1.1', quote: '原文引用' }, goalId: null };
    const wrapper = mount(App, { props: { chatDraft: draft }, shallow: true, global: { renderStubDefaultSlot: true } });
    wrappers.push(wrapper); await flushPromises();
    const rail = wrapper.findComponent({ name: 'RightRail' });
    expect(rail.props('agentInput')).toBe(draft.message);
    expect(rail.props('askDraft')).toEqual(draft.quote);
    rail.vm.$emit('update:agentInput', '继续编辑');
    await flushPromises();
    expect(wrapper.emitted('update:chatDraft')?.at(-1)).toEqual([expect.objectContaining({ ...draft, message: '继续编辑' })]);
  });
  it('rebinds the network workspace before retrying a failed history read', async () => {
    installIdentity({ user_id: 'A', csrf_token: 'csrf' });
    installWorkspace({ workspace_id: 'w', generation: 1, revision: 2, selected_chat: null,
      published_book_ref: { book_id: book, publication_id: 'p' }, reader: await api.state() });
    const binding = deferred<void>();
    const restore = vi.spyOn(networkClient, 'recoverWorkspaceBinding').mockReturnValue(binding.promise);
    api.agentHistory.mockRejectedValueOnce(new Error('WORKSPACE_STALE'));
    const wrapper = start(); await flushPromises();
    wrapper.findComponent({ name: 'RightRail' }).vm.$emit('retry-history');
    await flushPromises();
    expect(restore).toHaveBeenCalledOnce();
    expect(api.agentHistory).toHaveBeenCalledOnce();
    binding.resolve(); await flushPromises();
    expect(api.agentHistory).toHaveBeenCalledTimes(2);
    expect(wrapper.findComponent({ name: 'RightRail' }).props('historyError')).toBeNull();
  });
  it('JL9 resumes a recap target after the original chat has loaded', async () => {
    const open = vi.fn(async () => {});
    const target = { kind: 'turn' as const, session_id: 'startup-chat', turn_id: 'original-turn', through_seq: 11 };
    const saved = deferred<ReturnType<typeof history>>();
    api.agentHistory.mockReturnValueOnce(saved.promise);
    const wrapper = mount(App, { shallow: true, props: { recapTarget: target }, global: {
      renderStubDefaultSlot: true,
      stubs: {
        RightRail: defineComponent({ name: 'RightRail', template: '<div />', setup(_props, { expose }) { expose({ openRecapTarget: open }); } }),
        ReaderWorkspace: defineComponent({ template: '<div><slot /></div>', setup(_props, { expose }) { expose({ showAssistant: () => {} }); } }),
      },
    } });
    wrappers.push(wrapper); await flushPromises();
    expect(open).not.toHaveBeenCalled();
    saved.resolve(history()); await flushPromises();
    expect(open).toHaveBeenCalledExactlyOnceWith(target);
    expect(wrapper.emitted('recap-consumed')).toHaveLength(1);
  });
  it("uses the attached network reading position without another state round trip", async () => {
    installIdentity({ user_id: 'A', csrf_token: 'csrf' });
    const reader = await api.state(); api.state.mockClear();
    installWorkspace({ workspace_id: 'w', generation: 1, revision: 2, selected_chat: null,
      published_book_ref: { book_id: book, publication_id: 'p' }, reader });
    const wrapper = start();
    await flushPromises();
    expect(wrapper.find(".app-loading").exists()).toBe(false);
    expect(api.state).not.toHaveBeenCalled();
    expect(api.text).toHaveBeenCalledWith('1.1');
  });

  it("does not wait for PDF metadata when the saved surface is text", async () => {
    writeReaderSurfacePreference(localStorage, { bookId: book, sourceFingerprint: book }, 'markdown');
    const pdf = deferred<null>();
    api.sourceManifest.mockReturnValue(pdf.promise);
    const wrapper = start();
    await flushPromises();
    expect(wrapper.find(".app-loading").exists()).toBe(false);
    expect(wrapper.findComponent({ name: "ReaderPane" }).exists()).toBe(true);
    pdf.resolve(null); await flushPromises();
  });
  it("shows text without waiting for annotations or profile configuration", async () => {
    const notes = deferred<unknown[]>();
    const profile = deferred<object>();
    api.recall.mockReturnValue(notes.promise);
    api.profileManifest.mockReturnValue(profile.promise);
    const wrapper = start();
    await flushPromises();
    expect(wrapper.find(".app-loading").exists()).toBe(false);
    expect(wrapper.findComponent({ name: "ReaderPane" }).exists()).toBe(true);
    notes.resolve([]);
    profile.resolve({ profile_id: "technical_learning", slots: [] });
    await flushPromises();
  });

  it("starts independent material requests together", async () => {
    const manifest = deferred<object>();
    api.manifest.mockReturnValue(manifest.promise);
    start();
    await flushPromises();
    expect(api.assetManifest).toHaveBeenCalled();
    expect(api.sourceFingerprint).toHaveBeenCalled();
    expect(api.sourceManifest).toHaveBeenCalled();
    manifest.resolve({ tree: [], stats_by_lid: {} });
    await flushPromises();
  });
  it("shows the reader while history and profile are still pending", async () => {
    const pending = deferred<ReturnType<typeof history>>();
    const profile = deferred<object>();
    api.agentHistory.mockReturnValue(pending.promise);
    api.profileMemory.mockReturnValue(profile.promise);
    const wrapper = start();
    await flushPromises();
    expect(wrapper.find(".app-loading").exists()).toBe(false);
    expect(wrapper.findComponent({ name: "ReaderPane" }).exists()).toBe(true);
    expect(api.text.mock.calls.map(([lid]) => lid)).not.toContain("2");
    expect(wrapper.findComponent({ name: "RightRail" }).props("historyLoading")).toBe(true);
    const context = wrapper.findComponent({ name: "ReaderWorkspace" }).props("logical").contextKey;
    const clearSelection = vi.spyOn(window.getSelection()!, "removeAllRanges");
    pending.resolve(history());
    profile.resolve({});
    await flushPromises();
    expect(wrapper.findComponent({ name: "RightRail" }).props("historyError")).toBe(null);
    expect(wrapper.findComponent({ name: "RightRail" }).props("activeChatSessionId")).toBe("startup-chat");
    expect(wrapper.findComponent({ name: "ReaderWorkspace" }).props("logical").contextKey).toBe(context);
    expect(clearSelection).not.toHaveBeenCalled();
    clearSelection.mockRestore();
  });

  it("keeps the reader and unsent draft available when history fails, and supports retry", async () => {
    api.agentHistory.mockRejectedValueOnce(new Error("temporary history failure"));
    const wrapper = start();
    await flushPromises();
    const rail = wrapper.findComponent({ name: "RightRail" });
    expect(wrapper.find(".app-loading").exists()).toBe(false);
    expect(rail.props("historyError")).toContain("temporary history failure");
    rail.vm.$emit("update:agentInput", "还没发送的问题");
    rail.vm.$emit("send-agent");
    await flushPromises();
    expect(rail.props("agentInput")).toBe("还没发送的问题");
    rail.vm.$emit("retry-history");
    await flushPromises();
    expect(rail.props("historyError")).toBe(null);
    expect(rail.props("activeChatSessionId")).toBe("startup-chat");
    expect(rail.props("agentInput")).toBe("还没发送的问题");
  });

  it("ignores history from the previous book after a book switch", async () => {
    const pending = deferred<ReturnType<typeof history>>();
    api.agentHistory.mockReturnValueOnce(pending.promise);
    const wrapper = start();
    await flushPromises();
    wrapper.findComponent({ name: "TopBar" }).vm.$emit("open-book");
    await flushPromises();
    await wrapper.get('input[placeholder=".understand-book/book-id"]').setValue("second");
    await wrapper.get(".book-picker-actions .primary-action").trigger("click");
    await flushPromises();
    expect(wrapper.findComponent({ name: "RightRail" }).props("activeChatSessionId")).toBe("second-chat");
    pending.resolve(history("startup"));
    await flushPromises();
    expect(wrapper.findComponent({ name: "RightRail" }).props("activeChatSessionId")).toBe("second-chat");
    expect(wrapper.findComponent({ name: "RightRail" }).props("historyError")).toBe(null);
  });
});

describe("JL6 effect receipts in chat", () => {
  it("submits the stable effect identity and restores handled state after reload", async () => {
    const effect = { kind: "Highlight", mem_id: "original", lid: "1.1" };
    const disposition = { started: { disposition_id: "dispose-1", effect_id: "memory:original", action: "keep" },
      receipt: { disposition_id: "dispose-1", original_object_id: "original", result_object_id: "retained", error: null } };
    const stored = { ...history(), current: { ...history().current, turns: [{ turn_id: "turn-1", user: "Keep it", status: "completed",
      outcome: { answer: "Saved", effects: [effect], trace: [], memory_updates: [], turns: 1, tokens_spent: 0, incomplete: false },
      effect_labels: ["Highlight"], domain: { effects: [{ effect_id: "memory:original", effect: { kind: "reader", effect }, disposition: null as typeof disposition | null }] } }] } };
    api.agentHistory.mockResolvedValue(stored);
    api.disposeEffect.mockResolvedValue(disposition);
    const wrapper = start(); await flushPromises();
    const rail = wrapper.findComponent({ name: "RightRail" });
    rail.vm.$emit("keep-effect", 0, 0, effect); await flushPromises();
    expect(api.disposeEffect).toHaveBeenCalledWith({ session_id: "startup-chat", turn_id: "turn-1", effect_id: "memory:original", action: "keep" });
    expect(rail.props("effState")(0, 0)).toBe("已保留");
    stored.current.turns[0].domain.effects[0].disposition = disposition;
    wrapper.unmount();
    const reloaded = start(); await flushPromises();
    expect(reloaded.findComponent({ name: "RightRail" }).props("effState")(0, 0)).toBe("已保留");
    expect(api.disposeEffect).toHaveBeenCalledTimes(1);
  });
});

it.each([false, true])('acknowledges video send before draft persistence and prevents duplicate sends (save fails=%s)', async failed => {
  const video = { media_id: 'v', media_revision: '1' };
  const publication = { book_id: book, publication_id: 'original' };
  const material = { book_id: book, video, transcript: { transcript_id: 't', transcript_revision: '1', video } };
  const draft = { message: '原草稿', context: { draft_id: 'd', publication, material,
    frame: { video, actual_pts_ms: 274000, width: 1920, height: 1080 }, region: null, range: null } };
  api.videoContext.mockResolvedValue({ time_base: [1, 1000], publication, material,
    source: { video, original_path: 'lesson.mp4', duration_ms: 510000, width: 1920, height: 1080 }, map: { segments: [] } });
  const save = deferred<{ draft: typeof draft }>(), admission = deferred<unknown>();
  api.videoDraft.mockImplementation(input => input.action === 'get' ? Promise.resolve({ draft }) : save.promise);
  api.agentRunCreate.mockReturnValue(admission.promise);
  const wrapper = start(); await flushPromises();
  const rail = wrapper.getComponent({ name: 'RightRail' });
  rail.vm.$emit('update:agentInput', '这里的资源管理和书中一样吗？');
  rail.vm.$emit('send-agent'); await flushPromises();
  expect(rail.props('sending')).toBe(true);
  expect(rail.props('questionInputBlocked')).toBe(true);
  expect(rail.props('chat')).toHaveLength(1);
  expect(rail.props('chat')[0].runStatus).toBe('正在保存视频提问…');
  expect(api.agentRunCreate).not.toHaveBeenCalled();
  rail.vm.$emit('send-agent'); await flushPromises();
  if (failed) save.reject(new Error('保存暂不可用'));
  else save.resolve({ draft });
  await flushPromises();
  if (failed) {
    expect(api.agentRunCreate).not.toHaveBeenCalled();
    expect(rail.props('sending')).toBe(false);
    expect(rail.props('agentInput')).toBe('这里的资源管理和书中一样吗？');
    expect(rail.props('chat')[0]).toMatchObject({ pending: false, error: '保存暂不可用' });
    expect(rail.props('chat')[0].submissionUnknown).toBeUndefined();
    api.videoDraft.mockResolvedValue({ draft });
    rail.vm.$emit('send-agent'); await flushPromises();
    expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
    admission.resolve({ answer: '重试后的回答', effects: [] }); await flushPromises();
    expect(rail.props('sending')).toBe(false);
  } else {
    expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
    expect(rail.props('chat')).toHaveLength(1);
    expect(rail.props('chat')[0].runStatus).toBe('正在提交问题…');
    admission.resolve({ answer: '回答', effects: [] }); await flushPromises();
    expect(rail.props('sending')).toBe(false);
    expect(rail.props('chat')[0].runStatus).toBeUndefined();
  }
});

it('shows book submission immediately and removes an activity-help-only placeholder', async () => {
  const admission = deferred<unknown>(); api.agentRunCreate.mockReturnValue(admission.promise);
  const wrapper = start(); await flushPromises();
  const rail = wrapper.getComponent({ name: 'RightRail' });
  (rail.vm as unknown as { focusActivityHelp: () => void }).focusActivityHelp = vi.fn();
  rail.vm.$emit('update:agentInput', '解释书中的这段内容');
  rail.vm.$emit('send-agent'); await flushPromises();
  expect(rail.props('sending')).toBe(true);
  expect(rail.props('chat')[0].runStatus).toBe('正在提交问题…');
  expect(api.videoDraft).not.toHaveBeenCalled();
  admission.resolve({ kind: 'activity_help', delivery_refs: [] }); await flushPromises();
  expect(rail.props('chat')).toEqual([]);
  expect(rail.props('sending')).toBe(false);
});

it('mounts the video before transcript and draft responses without replacing its component', async () => {
  const video = { media_id: 'v', media_revision: '1' };
  const publication = { book_id: book, publication_id: 'original' };
  const head = { hls: true, time_base: [1, 1000], publication, material: { book_id: book, video },
    source: { video, original_path: 'lesson.mp4', duration_ms: 510000, width: 1920, height: 1080 } };
  const details = deferred<unknown>(), draft = deferred<unknown>();
  api.videoPlayback.mockResolvedValue(head); api.videoContext.mockReturnValue(details.promise); api.videoDraft.mockReturnValue(draft.promise);
  const wrapper = start(); await flushPromises();
  const pane = wrapper.getComponent({ name: 'VideoReaderPane' });
  expect(pane.props('playback')).toEqual(head);
  expect(pane.props('detailsLoading')).toBe(true);
  expect(wrapper.getComponent({ name: 'RightRail' }).props('questionInputBlocked')).toBe(true);
  details.resolve({ ...head, map: { segments: [] } }); await flushPromises();
  expect(wrapper.getComponent({ name: 'VideoReaderPane' }).vm).toBe(pane.vm);
  expect(pane.props('detailsLoading')).toBe(true);
  draft.resolve({ draft: null }); await flushPromises();
  expect(pane.props('detailsLoading')).toBe(false);
  expect(wrapper.getComponent({ name: 'VideoReaderPane' }).vm).toBe(pane.vm);
});

it.each([false, true])('loads the current music calibration while respecting a shared course selection (%s)', async shared => {
  installIdentity({ user_id: 'music-learner', csrf_token: 'csrf' });
  const publication = { book_id: book, publication_id: 'pending-music' };
  installWorkspace({ workspace_id: 'music-scene', learning_workspace_id: shared ? 'music-course' : undefined,
    generation: 1, revision: 1, selected_chat: `${book}-chat`, published_book_ref: publication, reader: await api.state() } as never);
  const old = { publication, manifest: { title: 'Schubert', coverage: [{ status: 'pending' }] }, audio_url: '/old-audio' };
  const revised = { ...old, publication: { ...publication, publication_id: 'calibrated-music' },
    manifest: { title: 'Schubert', coverage: [{ status: 'accepted' }] }, audio_url: '/current-audio' };
  api.musicContext.mockResolvedValue(old);
  api.adoptMusicPublication.mockResolvedValue({ status: 'adopted', music: revised });
  const wrapper = start(); await flushPromises();
  expect(wrapper.getComponent({ name: 'MusicReaderPane' }).props('context')).toEqual(shared ? old : revised);
  if (shared) expect(api.adoptMusicPublication).not.toHaveBeenCalled();
  else expect(api.adoptMusicPublication).toHaveBeenCalledWith('pending-music');
});

it.each(['selection', 'upload'] as const)('sends a music %s as pixels, retains a rejected draft, and clears it after acceptance', async origin => {
  const { ApiError } = await import('@understand-book/web/reader/api');
  const publication = { book_id: book, publication_id: 'music-publication' };
  api.musicContext.mockResolvedValue({ publication, manifest: { title: 'Music', coverage: [{ status: 'accepted' }] }, audio_url: '/music.wav' });
  const context = { draft_id: 'music-image', publication, score: null, occurrence_id: null, focused_part_ids: [],
    audio: { recording: { recording_id: 'recording', revision: '1' }, start_ms: 1200, end_ms: 2000 } };
  const image = { png_base64: 'music-pixels', width: 640, height: 240, origin };
  const wrapper = mount(App, { shallow: true, global: { renderStubDefaultSlot: true,
    stubs: { ReaderWorkspace: learningWorkspaceStub, MusicReaderPane: { name: 'MusicReaderPane', props: ['context', 'question', 'selectionDisabled'], methods: { captureQuestion: () => context }, template: '<div />' } } } });
  wrappers.push(wrapper); await flushPromises();
  const rail = wrapper.getComponent({ name: 'RightRail' }), pane = wrapper.getComponent({ name: 'MusicReaderPane' });
  if (origin === 'selection') pane.vm.$emit('question', { ...context, image });
  else {
    rail.vm.$emit('question-image-loading', true); await flushPromises();
    rail.vm.$emit('send-agent'); await flushPromises();
    expect(api.agentRunCreate).not.toHaveBeenCalled();
    rail.vm.$emit('question-image', image); rail.vm.$emit('question-image-loading', false);
  }
  await flushPromises();
  expect(rail.props('musicQuestion').image).toEqual(image);
  expect(rail.props('agentInput')).toBe('');
  api.agentRunCreate.mockRejectedValueOnce(new ApiError(409, 'MUSIC_QUESTION_INVALID', 'conflict', '请重试'));
  rail.vm.$emit('send-agent'); await flushPromises();
  expect(api.agentRunCreate.mock.calls[0][0]).toContain('我是初学者');
  expect(api.agentRunCreate.mock.calls[0][1].music_question).toEqual({ ...context, image });
  expect(rail.props('musicQuestion').image).toEqual(image);
  rail.vm.$emit('update:agentInput', '这五条线是什么？'); await flushPromises();
  const admission = deferred<unknown>(); api.agentRunCreate.mockReturnValueOnce(admission.promise);
  rail.vm.$emit('send-agent'); await flushPromises();
  expect(rail.props('agentInput')).toBe('这五条线是什么？');
  expect(rail.props('questionInputBlocked')).toBe(true);
  expect(api.agentRunCreate.mock.calls[1][1].music_question.audio.start_ms).toBe(1200);
  admission.resolve({ book_id: book, session_id: `${book}-chat`, turn_id: 'music-accepted' }); await flushPromises();
  expect(rail.props('musicQuestion')).toBeNull();
  expect(rail.props('agentInput')).toBe('');
});

it('clears a music image draft when history confirms admission after a lost creation response', async () => {
  const publication = { book_id: book, publication_id: 'music-publication' };
  api.musicContext.mockResolvedValue({ publication, manifest: { title: 'Music', coverage: [{ status: 'accepted' }] }, audio_url: '/music.wav' });
  const question = { draft_id: 'accepted-music-image', publication, score: null, occurrence_id: null, focused_part_ids: [],
    audio: { recording: { recording_id: 'recording', revision: '1' }, start_ms: 1200, end_ms: 2000 },
    image: { png_base64: 'pixels', width: 100, height: 80, origin: 'selection' } };
  const wrapper = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: { ReaderWorkspace: learningWorkspaceStub } } });
  wrappers.push(wrapper); await flushPromises();
  const rail = wrapper.getComponent({ name: 'RightRail' });
  wrapper.getComponent({ name: 'MusicReaderPane' }).vm.$emit('question', question); await flushPromises();
  api.agentRunCreate.mockRejectedValueOnce(new Error('creation response lost'));
  api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current, turns: [{
    turn_id: 'accepted-image-turn', user: '原提问', status: 'failed', effect_labels: [], music_question: question,
    error: { category: 'provider', error_code: 'PROVIDER_ERROR', message: '回答未完成' },
  }] } });
  rail.vm.$emit('send-agent'); await flushPromises();
  expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
  expect(rail.props('musicQuestion')).toBeNull();
  expect(rail.props('agentInput')).toBe('');
  expect(rail.props('chat')[0].musicQuestion.image).toEqual(question.image);
});

it('VI6 opens a local published video without interpreting preparation as a build workbench', async () => {
  api.desktopStatus.mockResolvedValue({ desktop_host:false, reader_only:false, active_book:true });
  const video = { media_id:'v',media_revision:'1' };
  const publication = { book_id:book,publication_id:'original' };
  const material = { book_id:book,video,transcript:{transcript_id:'t',transcript_revision:'1',video} };
  api.videoContext.mockResolvedValue({ time_base:[1,1000], publication, material, source:{video,original_path:"lesson.mp4",duration_ms:510000,width:1920,height:1080},map:{segments:[]} });
  api.videoDraft.mockResolvedValue({ draft:{message:'恢复草稿',context:{draft_id:'d',publication,material,frame:{video,actual_pts_ms:492000,width:1920,height:1080},region:null,range:null}} });
  const wrapper=start(); await flushPromises();
  expect(api.buildWorkbench).not.toHaveBeenCalled();
  expect(wrapper.findComponent({name:'VideoReaderPane'}).exists()).toBe(true);
  expect(wrapper.findComponent({name:'ReaderPane'}).exists()).toBe(false);
  const rail=wrapper.findComponent({name:'RightRail'});
  expect(rail.props('agentInput')).toBe('恢复草稿');
  expect(rail.props('videoQuestion').frame.actual_pts_ms).toBe(492000);
  expect(rail.props('questionSendBlocked')).toBeUndefined();
});


it.each([false, true])('keeps a plain workspace draft in video, binds it on send and never auto-adopts another publication (range=%s)', async selectedRange => {
  vi.useFakeTimers();
  try {
    installIdentity({ user_id: 'video-learner', csrf_token: 'csrf' });
    const publication = { book_id: book, publication_id: 'video-v1' };
    installWorkspace({ workspace_id: 'scene', learning_workspace_id: 'learning-video', generation: 1, revision: 1,
      selected_chat: `${book}-chat`, published_book_ref: publication, reader: await api.state() } as never);
    api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current, learning_workspace_id: 'learning-video' } });
    const video = { media_id: 'v', media_revision: '1' };
    const material = { book_id: book, video, transcript: { transcript_id: 't', transcript_revision: '1', video } };
    api.videoContext.mockResolvedValue({ time_base: [1, 1000], publication, material, source: { video, original_path: 'lesson.mp4', duration_ms: 510000, width: 1920, height: 1080 }, map: { segments: [] } });
    const captureQuestion = vi.fn((draft_id: string) => ({ draft_id, publication, material, frame: { video, actual_pts_ms: 95000, width: 1920, height: 1080 }, region: null, range: null }));
    api.agentRunCreate.mockResolvedValue({ answer: '关联资料回答', effects: [] });
    const wrapper = mount(App, { props: { chatDraft: { message: '联系书中的内容解释视频', quote: null, goalId: null } }, shallow: true,
      global: { renderStubDefaultSlot: true, stubs: { VideoReaderPane: { name: 'VideoReaderPane', props: ['owner'], methods: { captureQuestion }, template: '<div />' } } } });
    wrappers.push(wrapper); await flushPromises();
    const rail = wrapper.getComponent({ name: 'RightRail' });
    expect(rail.props('agentInput')).toBe('联系书中的内容解释视频');
    expect(wrapper.getComponent({ name: 'VideoReaderPane' }).props('owner')).toBe('video-learner:learning-video');
    await vi.advanceTimersByTimeAsync(6500); await flushPromises();
    expect(api.adoptVideoPublication).not.toHaveBeenCalled();
    if (selectedRange) {
      wrapper.getComponent({ name: 'VideoReaderPane' }).vm.$emit('select-range', { ...captureQuestion('range-draft'), range: { video, start_ms: 1000, end_ms: 2000 } });
      await flushPromises();
    }
    rail.vm.$emit('send-agent'); await flushPromises();
    expect(captureQuestion).toHaveBeenCalledTimes(1);
    expect(api.agentRunCreate).toHaveBeenCalledWith('联系书中的内容解释视频', expect.objectContaining({ video_draft_id: expect.any(String), video_window_id: expect.any(String) }));
    expect(api.videoDraft).toHaveBeenCalledWith(expect.objectContaining({ action: 'save', draft: expect.objectContaining({ message: '联系书中的内容解释视频', context: expect.objectContaining({ publication }) }) }));
    if (selectedRange) expect(api.videoDraft.mock.calls.at(-1)?.[0].draft.context.range).toMatchObject({ start_ms: 1000, end_ms: 2000 });
  } finally { vi.useRealTimers(); }
});


// Exercise the reader-to-global-notes boundary with the current versioned note service.
function noteSurface(records: any[] = []) {
  const view = (record: any, revision = 1) => ({ note_id: record.mem_id, created_at: '1700000000', current_revision: revision,
    value: { revision, saved_at: '1700000000', title: null, authorship: 'user', record,
      context: { ...emptyNoteContext(), book_id: record.book_id }, source_refs: [], change_note: null, based_on_turn: null, restored_from: null } });
  const saved = new Map(records.map(record => [record.mem_id, view(record)]));
  const write = vi.fn(async (_action: string, input: any) => {
    const id = input.note_id ?? 'saved', previous = saved.get(id);
    const result = view({ ...(previous?.value.record ?? { type: 'note', book_id: 'startup', layer: 'long_term', anchor: {} }),
      mem_id: id, content: input.value.content, note: input.value.source }, (previous?.current_revision ?? 0) + 1);
    result.value.context = input.value.context; saved.set(id, result); return result;
  });
  vi.stubGlobal('fetch', vi.fn(async (url: unknown, init?: RequestInit) => {
    const action = String(url).split('/').at(-1), input = JSON.parse(String(init?.body ?? '{}'));
    if (action === 'create' || action === 'revise') return Response.json(await write(action, input));
    if (action === 'read') return Response.json(saved.get(input.note_id));
    if (action === 'history') return Response.json({ revisions: [], discussions: [] });
    if (action === 'links') return Response.json({ links: [] });
    if (action === 'attachments' || action === 'board_locations') return Response.json({ items: [] });
    if (action === 'list') return Response.json({ notes: [...saved.values()], next_cursor: null });
    throw new Error(`Unexpected note fixture request: ${url}`);
  }));
  api.prepareNote.mockImplementation(async (input: any) => ({ ...emptyNoteContent({ book_id: 'startup',
    session_id: input.note?.association.session_id ?? input.note?.association.receipt?.session_id ?? null,
    turn_id: input.note?.association.turn_id ?? input.note?.association.receipt?.turn_id ?? null }, input.content),
    source: input.note ? { material: { book_id: 'startup', publication_id: null }, association: input.note.association,
      retained_excerpt: input.note.retained_excerpt ? { kind: input.note.association.kind === 'answer' ? 'assistant' : 'original', text: input.note.retained_excerpt } : null, source_bindings: [] } : null }));
  const notes = mount(PersonalNotes); wrappers.push(notes as never);
  const click = async (text: string) => { await notes.findAll('button').find(button => button.text() === text)!.trigger('click'); await flushPromises(); };
  const body = () => notes.get('textarea[aria-label="想法正文"]');
  const save = async () => { await notes.get('.thought-actions .primary').trigger('click'); await flushPromises(); };
  return { notes, write, saved, click, body, save };
}

describe('RN2 presentation note draft', () => {
  const receipt = { session_id: 'startup-chat', turn_id: 'old-turn', reference: { presentation_id: 'page', revision: 2 }, state_revision: 4, saved_state_ref: 'state-4' };
  it('keeps the captured association across collapse, chat change and save failure', async () => {
    const n = noteSurface(), w = start(true); await flushPromises();
    w.getComponent({ name: 'RightRail' }).vm.$emit('record-note', receipt, 'Recorded example'); await flushPromises();
    await n.body().setValue('my thought'); await n.notes.get('[aria-label="收起我的想法"]').trigger('click');
    api.agentNew.mockResolvedValue({ ok: true, history: { ...history(), active_session_id: 'new-chat', current: { ...history().current, id: 'new-chat' } } });
    w.getComponent({ name: 'RightRail' }).vm.$emit('new-chat'); await flushPromises();
    await n.notes.get('.thought-entry').trigger('click'); await flushPromises();
    expect((n.body().element as HTMLTextAreaElement).value).toBe('my thought');
    n.write.mockRejectedValueOnce(new Error('write failed')); await n.save();
    expect(n.notes.get('[role="alert"]').text()).toContain('write failed');
    expect(readNoteDrafts()[0].value.content).toBe('my thought');
    await n.save();
    expect(n.write).toHaveBeenLastCalledWith('create', expect.objectContaining({ value: expect.objectContaining({ content: 'my thought',
      source: expect.objectContaining({ association: { kind: 'presentation', receipt } }) }) }));
    expect(notesPanel.open).toBe(false); expect(readNoteDrafts()).toEqual([]);
    expect(w.getComponent({ name: 'RightRail' }).props('activeChatSessionId')).toBe('new-chat');
  });
  it('revises a saved note under its stable identity and binds follow-up to that note', async () => {
    const record = { mem_id: 'saved', type: 'note', book_id: 'startup', content: 'original', layer: 'long_term', anchor: {}, note: { association: { kind: 'presentation', receipt, title: 'Example' } } };
    const n = noteSurface([record]), w = start(true); await flushPromises();
    w.getComponent({ name: 'RightRail' }).vm.$emit('edit-note', record); await flushPromises();
    await n.click('继续写'); await n.body().setValue('edited'); await n.save();
    expect(n.write).toHaveBeenCalledWith('revise', expect.objectContaining({ note_id: 'saved', base_revision: 1, value: expect.objectContaining({ content: 'edited' }) }));
    expect(n.saved.get('saved')?.current_revision).toBe(2);
    api.agentRunCreate.mockResolvedValue({ answer: 'ok', answer_view: { parts: [] }, effects: [] });
    w.getComponent({ name: 'RightRail' }).vm.$emit('note-follow-up', 'explain', receipt, 'saved'); await flushPromises();
    expect(api.agentRunCreate).toHaveBeenCalledWith('explain', expect.objectContaining({ note_mem_id: 'saved', presentation_follow_up: receipt }));
  });
});

describe('RN3 global note draft', () => {
  const turn = { turnId: 'answer-1', pending: false, outcome: { answer: '__选中的回答__' },
    questionSelection: { lid: '1.1', quote: '提问时选择的原文', status: 'resolved', raw_quote: '提问时选择的原文', resolved_quote: '提问时选择的原文', ranges: [{ lid: '1.1', range: { start: 0, end: 9 } }] } };
  const old = { mem_id: 'old', type: 'note', book_id: 'startup', layer: 'long_term', anchor: { lid: '1.1' }, content: '> 原来混排的内容\n\n用户编辑过的文字' };
  it('records an answer with its fixed turn without changing the composer or reading position', async () => {
    const n = noteSurface(), w = start(true); await flushPromises(); const rail = w.getComponent({ name: 'RightRail' });
    rail.vm.$emit('update:agentInput', '我的问题'); rail.vm.$emit('save-answer-selection', turn, '__选中的回答__'); await flushPromises();
    expect((n.body().element as HTMLTextAreaElement).value).toBe('');
    expect(n.notes.get('.thought-excerpt').text()).toContain('选中的回答');
    expect(rail.props('agentInput')).toBe('我的问题'); expect(rail.props('selectedLid')).toBe('1.1');
    api.agentNew.mockResolvedValue({ ok: true, history: { ...history(), active_session_id: 'other', current: { ...history().current, id: 'other' } } });
    rail.vm.$emit('new-chat'); await flushPromises(); await n.save();
    expect(api.prepareNote).toHaveBeenCalledWith(expect.objectContaining({ note: { association: { kind: 'answer', session_id: 'startup-chat', turn_id: 'answer-1' }, retained_excerpt: '__选中的回答__' } }));
    expect(n.write.mock.calls[0][1].value.context).toMatchObject({ session_id: 'startup-chat', turn_id: 'answer-1' });
  });
  it('keeps independent drafts when another note opens and retains failed revisions for retry', async () => {
    const n = noteSurface([old]), w = start(true); await flushPromises(); const rail = w.getComponent({ name: 'RightRail' });
    rail.vm.$emit('edit-note', old); await flushPromises(); await n.click('继续写'); await n.body().setValue('第一次修改');
    const original = readNoteDrafts()[0];
    rail.vm.$emit('save-answer-selection', turn, '__选中的回答__'); await flushPromises();
    expect(readNoteDrafts()).toHaveLength(2); expect(original.value.content).toBe('第一次修改');
    rail.vm.$emit('edit-note', old); await flushPromises();
    expect((n.body().element as HTMLTextAreaElement).value).toBe('第一次修改');
    n.write.mockRejectedValueOnce(new Error('写入失败')); await n.save();
    expect(n.notes.get('[role="alert"]').text()).toContain('写入失败'); expect(original.value.content).toBe('第一次修改');
    await n.save(); expect(n.saved.get('old')?.value.record.content).toBe('第一次修改');
    expect(readNoteDrafts()).toHaveLength(1); openThoughtCapture(); await flushPromises();
    await n.notes.get('[aria-label="放弃这份草稿"]').trigger('click'); expect(readNoteDrafts()).toEqual([]);
  });
  it('preserves Markdown whitespace when revising an old note', async () => {
    const record = { ...old, content: '    const n = 1;\n\n' }, n = noteSurface([record]), w = start(true); await flushPromises();
    w.getComponent({ name: 'RightRail' }).vm.$emit('edit-note', record); await flushPromises(); await n.click('继续写');
    await n.body().setValue(record.content + '    const m = 2;\n'); await n.save();
    expect(n.write.mock.calls[0][1].value.content).toBe(record.content + '    const m = 2;\n');
  });
  it('keeps device-persisted drafts across material switches without a blocking leave prompt', async () => {
    const n = noteSurface(), w = start(true); await flushPromises();
    w.getComponent({ name: 'RightRail' }).vm.$emit('save-answer-selection', turn, '__选中的回答__'); await flushPromises();
    await n.body().setValue('跨材料继续写');
    expect(await (w.vm as unknown as { beforeNoteLeave: (label: string) => Promise<boolean> }).beforeNoteLeave('切换发布')).toBe(true);
    const draft = readNoteDrafts()[0]; expect(draft.value.content).toBe('跨材料继续写');
    expect(Object.values(localStorage).some(value => value.includes(draft.draft_id))).toBe(true);
    const event = new Event('beforeunload', { cancelable: true }); window.dispatchEvent(event); expect(event.defaultPrevented).toBe(false);
    expect(n.write).not.toHaveBeenCalled();
  });
  it('hides another account’s drafts and ignores its late save failure while retaining its device copy', async () => {
    installIdentity({ user_id: 'A', csrf_token: 'a' });
    installWorkspace({ workspace_id: 'w', generation: 1, revision: 1, selected_chat: 'startup-chat', published_book_ref: { book_id: 'startup', publication_id: 'p1' }, reader: { book_id: 'startup', viewport: { anchor_lid: '1.1', top_lid: '1.1', bottom_lid: '1.1', visible_lids: ['1.1'], width: 1 }, layout: { rev: 0, open_slots: [] } } } as never);
    const n = noteSurface(), w = start(true); await flushPromises();
    w.getComponent({ name: 'RightRail' }).vm.$emit('save-answer-selection', turn, '__选中的回答__'); await flushPromises();
    await n.body().setValue('A 的想法'); const pending = deferred<any>(); n.write.mockReturnValueOnce(pending.promise);
    await n.notes.get('.thought-actions .primary').trigger('click'); await flushPromises();
    installIdentity({ user_id: 'B', csrf_token: 'b' }); await flushPromises(); pending.reject(new Error('A 的保存错误')); await flushPromises();
    expect(readNoteDrafts()).toEqual([]); expect(notesPanel.open).toBe(false); expect(n.notes.text()).not.toContain('A 的保存错误');
    installIdentity({ user_id: 'A', csrf_token: 'a' }); await flushPromises(); expect(readNoteDrafts()[0].value.content).toBe('A 的想法');
  });
  it.each(['ReaderPane', 'RightRail'])('opens original notes from %s with a read-only excerpt and versioned body editing', async name => {
    const record = { ...old, content: '自己的话', note: { association: { kind: 'selection' }, retained_excerpt: { kind: 'original', text: '原文片段' } } };
    const n = noteSurface([record]), w = start(true); await flushPromises();
    w.getComponent({ name }).vm.$emit('edit-note', record); await flushPromises(); expect(n.notes.get('.thought-excerpt').text()).toContain('原文片段');
    await n.click('继续写'); await n.body().setValue(''); await n.save();
    expect(n.write).toHaveBeenCalledWith('revise', expect.objectContaining({ note_id: 'old', base_revision: 1,
      value: expect.objectContaining({ content: '', source: record.note }) }));
  });
});


describe('RN4 note continuity', () => {
  const note = { mem_id: 'old', type: 'note', book_id: 'startup', layer: 'long_term', anchor: {}, content: '自己的想法', generated_at: '1700000000',
    note: { material: { book_id: 'startup', publication_id: null }, association: { kind: 'answer', session_id: 'original', turn_id: 'original-turn' }, retained_excerpt: { kind: 'assistant', text: '原回答摘录' }, source_bindings: [] } };
  it('refreshes the same note after a revision and keeps a failed delete visible', async () => {
    const n = noteSurface([note]);
    api.recall.mockResolvedValue([note]);
    const w = start(); await flushPromises(); const rail = w.getComponent({ name: 'RightRail' });
    rail.vm.$emit('edit-note', note); await flushPromises();
    const saved = { ...note, content: '修订文字' };
    api.recall.mockResolvedValue([saved]);
    await n.click('继续写'); await n.body().setValue(saved.content); await n.save();
    expect(rail.props('contextNotes')).toEqual([saved]); expect(n.notes.text()).toContain('已保存');
    expect(n.saved.get('old')?.current_revision).toBe(2);
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    api.delete.mockRejectedValueOnce(new Error('删除失败'));
    rail.vm.$emit('delete-note', saved); await flushPromises(); expect(rail.props('contextNotes')).toEqual([saved]);
    api.delete.mockResolvedValue({ ok: true }); api.recall.mockResolvedValue([]);
    rail.vm.$emit('delete-note', saved); await flushPromises(); expect(api.delete).toHaveBeenLastCalledWith('old');
    expect(rail.props('contextNotes')).toEqual([]); expect(w.text()).toContain('笔记已删除');
  });
  it('opens the original answer and reports a deleted chat without changing notes', async () => {
    const scroll = vi.fn().mockResolvedValue(true);
    const w = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: { ReaderPane: defineComponent({ setup(_, { expose }) { expose({ captureScrollAnchor: vi.fn() }); return {}; }, template: '<div />' }), ReaderWorkspace: defineComponent({ setup(_, { expose }) { expose({ showAssistant: vi.fn() }); return {}; }, template: '<div><slot /><slot name="reader" /><slot name="assistant" /></div>' }), RightRail: defineComponent({ name: 'RightRail', setup(_, { expose }) { expose({ scrollToTurn: scroll }); return {}; }, template: '<div />' }) } } }); wrappers.push(w);
    await flushPromises(); const rail = w.getComponent({ name: 'RightRail' });
    const original = { ...history(), active_session_id: 'original', current: { ...history().current, id: 'original', turns: [{ turn_id: 'original-turn', user: '原问题', status: 'completed', effect_labels: [], outcome: null }] } };
    api.agentHistorySelect.mockResolvedValue(original);
    rail.vm.$emit('open-note-answer', note); await flushPromises();
    expect(api.agentHistorySelect).toHaveBeenCalledWith('original'); expect(scroll).toHaveBeenCalledWith('original-turn');
    api.agentHistorySelect.mockRejectedValueOnce(new Error('聊天已删除'));
    rail.vm.$emit('open-note-answer', { ...note, note: { ...note.note, association: { ...note.note.association, session_id: 'deleted' } } }); await flushPromises();
    expect(w.text()).toContain('原回答当前不可用'); expect(scroll).toHaveBeenCalledTimes(1);
  });
  it.each(['publication', 'identity'])('does not apply a late recall after changing %s', async change => {
    installIdentity({ user_id: 'A', csrf_token: 'a' });
    const workspace = { workspace_id: 'w', generation: 1, revision: 1, selected_chat: 'startup-chat', published_book_ref: { book_id: 'startup', publication_id: 'p1' }, reader: { book_id: 'startup', viewport: { anchor_lid: '1.1', top_lid: '1.1', bottom_lid: '1.1', visible_lids: ['1.1'], width: 1 }, layout: { rev: 0, open_slots: [] } } };
    installWorkspace(workspace as never);
    const wait = deferred<unknown[]>(); api.recall.mockReturnValue(wait.promise);
    const w = start(); await flushPromises();
    expect(api.recall).toHaveBeenCalled();
    if (change === 'identity') installIdentity({ user_id: 'B', csrf_token: 'b' });
    else installWorkspace({ ...workspace, generation: 2, published_book_ref: { book_id: 'startup', publication_id: 'p2' } } as never);
    await flushPromises(); wait.resolve([{ ...note, note: { ...note.note, material: { book_id: 'startup', publication_id: 'p1' } } }]); await flushPromises();
    expect(w.getComponent({ name: 'RightRail' }).props('contextNotes')).toEqual([]);
  });
  it('only projects the current publication for structured notes while retaining unknown legacy records', async () => {
    installIdentity({ user_id: 'A', csrf_token: 'a' });
    installWorkspace({ workspace_id: 'w', generation: 1, revision: 1, selected_chat: 'startup-chat', published_book_ref: { book_id: 'startup', publication_id: 'p2' }, reader: { book_id: 'startup', viewport: { anchor_lid: '1.1', top_lid: '1.1', bottom_lid: '1.1', visible_lids: ['1.1'], width: 1 }, layout: { rev: 0, open_slots: [] } } } as never);
    const current = { ...note, mem_id: 'p2', note: { ...note.note, material: { book_id: 'startup', publication_id: 'p2' } } };
    const old = { ...note, mem_id: 'legacy', note: undefined };
    api.recall.mockResolvedValue([note, current, old]); const w = start(); await flushPromises();
    expect(w.getComponent({ name: 'RightRail' }).props('contextNotes').map((n: any) => n.mem_id).sort()).toEqual(['legacy', 'p2']);
  });
});


it('RN4 searches and displays all parts of a grouped highlight without changing its deletion identity', async () => {
  const highlight = { type: 'highlight', layer: 'long_term', book_id: 'startup', source_session_id: 'highlight-group:g', content: '前半段', mem_id: 'h1', anchor: { lid: '1.1' } };
  api.recall.mockResolvedValue([highlight, { ...highlight, mem_id: 'h2', anchor: { lid: '2.1' }, content: '后半段搜索目标' }]);
  const w = start(); await flushPromises();
  expect(w.getComponent({ name: 'RightRail' }).props('contextHighlights')).toEqual([{ ...highlight, content: '前半段\n后半段搜索目标',
    selection_context: { status: 'resolved', resolution_basis: 'exact', raw_quote: '前半段\n后半段搜索目标',
      resolved_quote: '前半段\n后半段搜索目标', ranges: [] },
  }]);
});

describe('RS1 shared note selection and scope', () => {
  const note = { mem_id: 'share-note', type: 'note', book_id: 'startup', layer: 'long_term', anchor: { lid: '1.1' }, content: '已保存的中文想法' };
  function startSharing(realPanel = false) {
    const w = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: {
      ...(realPanel ? { ShareImagePanel: false } : {}),
      RightRail: defineComponent({ name: 'RightRail', props: ['contextNotes'], setup() { return { share: inject(shareNoteKey)! }; },
        template: '<div><button v-for="note in contextNotes" :key="note.mem_id" @click="share(note)">分享</button></div>' }),
    } } }); wrappers.push(w); return w;
  }
  it('keeps saved text and an independent editing draft untouched while using the real book title', async () => {
    const n = noteSurface([note]);
    api.recall.mockResolvedValue([note]); api.paperMetadata.mockResolvedValue({ title: { value: '理解学习的方法' } });
    const w = startSharing(); await flushPromises();
    const rail = w.getComponent({ name: 'RightRail' }); rail.vm.$emit('edit-note', note); await flushPromises();
    await n.click('继续写'); await n.body().setValue('未保存的修改'); await flushPromises();
    await rail.get('button').trigger('click'); await flushPromises();
    const share = w.getComponent({ name: 'ShareImagePanel' });
    expect(share.props('source')).toMatchObject({ parts: [{ label: '阅读笔记', text: note.content }], sources: ['《理解学习的方法》', '首章'] });
    share.vm.$emit('close'); await flushPromises();
    expect((n.body().element as HTMLTextAreaElement).value).toBe('未保存的修改');
    expect(n.write).not.toHaveBeenCalled();
    expect(rail.props('contextNotes')[0].content).toBe(note.content);
    expect(api.replace).not.toHaveBeenCalled(); expect(api.save).not.toHaveBeenCalled(); expect(api.agentRunCreate).not.toHaveBeenCalled();
  });
  it.each(['chat', 'identity', 'publication', 'material'])('clears a pending source read on %s change', async change => {
    if (change === 'publication') {
      installIdentity({ user_id: 'A', csrf_token: 'a' });
      installWorkspace({ workspace_id: 'w', generation: 1, revision: 1, selected_chat: 'startup-chat', published_book_ref: { book_id: 'startup', publication_id: 'p1' }, reader: { book_id: 'startup', viewport: { anchor_lid: '1.1', top_lid: '1.1', bottom_lid: '1.1', visible_lids: ['1.1'], width: 1 }, layout: { rev: 0, open_slots: [] } } } as never);
    }
    const pending = deferred<any>(); api.paperMetadata.mockReturnValue(pending.promise); api.recall.mockResolvedValue([note]);
    const w = startSharing(); await flushPromises(); const rail = w.getComponent({ name: 'RightRail' });
    await rail.get('button').trigger('click'); await flushPromises(); expect(w.findComponent({ name: 'ShareImagePanel' }).exists()).toBe(true);
    if (change === 'chat') { api.agentHistorySelect.mockResolvedValue({ ...history(), active_session_id: 'other', current: { ...history().current, id: 'other' } }); rail.vm.$emit('select-chat', 'other'); }
    else if (change === 'identity') installIdentity({ user_id: 'B', csrf_token: 'b' });
    else if (change === 'material') {
      w.getComponent({ name: 'TopBar' }).vm.$emit('open-book'); await flushPromises();
      await w.get('input[placeholder=".understand-book/book-id"]').setValue('second');
      await w.get('.book-picker-actions .primary-action').trigger('click');
    }
    else installWorkspace({ ...network.value.workspace!, generation: 2, published_book_ref: { book_id: 'startup', publication_id: 'p2' } });
    await flushPromises(); pending.resolve({ title: { value: '旧来源晚到' } }); await flushPromises();
    expect(w.findComponent({ name: 'ShareImagePanel' }).exists()).toBe(false);
  });
  it('does not manufacture a source for a legacy unplaced note or after metadata failure', async () => {
    api.recall.mockResolvedValue([{ ...note, anchor: {} }, { ...note, mem_id: 'source-note' }]);
    api.paperMetadata.mockRejectedValue(new Error('不可用'));
    const w = startSharing(); await flushPromises(); const rail = w.getComponent({ name: 'RightRail' });
    await rail.findAll('button')[1].trigger('click'); await flushPromises();
    expect(w.getComponent({ name: 'ShareImagePanel' }).props('source').sources).toEqual(['无已记录出处']);
    expect(api.paperMetadata).not.toHaveBeenCalled();
    w.getComponent({ name: 'ShareImagePanel' }).vm.$emit('close'); await flushPromises();
    await rail.findAll('button')[0].trigger('click'); await flushPromises();
    expect(w.getComponent({ name: 'ShareImagePanel' }).props('source').sources).toEqual(['材料名称暂不可用', '首章']);
  });
  it('loads the existing material name even when the book picker has never been opened', async () => {
    api.recall.mockResolvedValue([note]); api.paperMetadata.mockResolvedValue({ available: false });
    api.bookLibrary.mockResolvedValue({ root: 'books', books: [{ book_id: 'startup', name: '学习与理解', dir: 'books/startup', route: 'reader' }] });
    const w = startSharing(); await flushPromises();
    await w.getComponent({ name: 'RightRail' }).get('button').trigger('click'); await flushPromises();
    expect(w.getComponent({ name: 'ShareImagePanel' }).props('source').sources).toEqual(['《学习与理解》', '首章']);
  });
  it('ignores a late library title after the share is closed', async () => {
    api.recall.mockResolvedValue([note]); api.paperMetadata.mockResolvedValue({ available: false });
    const pending = deferred<any>(); api.bookLibrary.mockReturnValue(pending.promise);
    const w = startSharing(); await flushPromises();
    await w.getComponent({ name: 'RightRail' }).get('button').trigger('click'); await flushPromises();
    expect(w.getComponent({ name: 'ShareImagePanel' }).props('loading')).toBe(true);
    w.getComponent({ name: 'ShareImagePanel' }).vm.$emit('close'); await flushPromises();
    pending.resolve({ books: [{ book_id: 'startup', name: '迟到书名' }] }); await flushPromises();
    expect(w.findComponent({ name: 'ShareImagePanel' }).exists()).toBe(false);
  });
  it.each(['identity', 'material', 'publication', 'chat'])('discards late PNG output after %s changes the reader scene', async change => {
    vi.spyOn(HTMLDialogElement.prototype, 'showModal').mockImplementation(() => {});
    vi.spyOn(HTMLDialogElement.prototype, 'close').mockImplementation(() => {});
    const create = vi.spyOn(URL, 'createObjectURL');
    installIdentity({ user_id: 'A', csrf_token: 'a' });
    installWorkspace({ workspace_id: 'w', generation: 1, revision: 1, selected_chat: 'startup-chat',
      published_book_ref: { book_id: 'startup', publication_id: 'p1' }, reader: { book_id: 'startup', viewport: { anchor_lid: '1.1', top_lid: '1.1', bottom_lid: '1.1', visible_lids: ['1.1'], width: 1 }, layout: { rev: 0, open_slots: [] } } } as never);
    api.recall.mockResolvedValue([note]); api.paperMetadata.mockResolvedValue({ title: { value: '原材料' } });
    const pending = deferred<Blob[]>(); vi.mocked(renderShare).mockReturnValue(pending.promise);
    const w = startSharing(true); await flushPromises();
    await w.getComponent({ name: 'RightRail' }).get('button').trigger('click'); await flushPromises();
    const share = w.getComponent({ name: 'ShareImagePanel' });
    await share.findAll('button').find(button => button.text() === '预览图片')!.trigger('click');
    expect(renderShare).toHaveBeenCalledOnce();
    if (change === 'identity') installIdentity({ user_id: 'B', csrf_token: 'b' });
    else if (change === 'chat') installWorkspace({ ...network.value.workspace!, generation: 2, selected_chat: 'other-chat' }, undefined, false, 'chat');
    else installWorkspace({ ...network.value.workspace!, generation: 2,
      published_book_ref: { book_id: change === 'material' ? 'second' : 'startup', publication_id: 'p2' } });
    await flushPromises();
    expect(w.findComponent({ name: 'ShareImagePanel' }).exists()).toBe(false);
    pending.resolve([new Blob(['old private output'])]); await flushPromises();
    expect(create).not.toHaveBeenCalled();
  });
});

it('RS2 shares all actual highlight group members and their chapter labels without trusting the UI preview text', async () => {
  const first = { mem_id: 'h1', type: 'highlight', book_id: 'startup', layer: 'long_term', anchor: { lid: '1.1' }, range: { start: 0, end: 3 }, content: '前半段', source_session_id: 'highlight-group:one' };
  const second = { ...first, mem_id: 'h2', anchor: { lid: '2.1' }, content: '后半段' };
  api.recall.mockResolvedValue([second, first]); api.paperMetadata.mockResolvedValue({ title: { value: '原材料' } });
  const w = mount(App, { shallow:true, global: { renderStubDefaultSlot:true, stubs: {
    RightRail: defineComponent({ name:'RightRail', setup() { return { share: inject(shareHighlightKey)! }; },
      template: '<button @click="share({mem_id: \'h1\', content: \'伪造预览\'})">分享高亮</button>' }),
  } } }); wrappers.push(w); await flushPromises();
  await w.getComponent({name:'RightRail'}).get('button').trigger('click'); await flushPromises();
  expect(w.getComponent({name:'ShareImagePanel'}).props('source')).toMatchObject({ parts: [{label:'原文摘录',text:'前半段\n后半段'}],sources:['《原材料》','首章','后章'] });
  expect(api.save).not.toHaveBeenCalled(); expect(api.agentRunCreate).not.toHaveBeenCalled();
});

describe('RS6 delivered answer sharing', () => {
  function setupAnswer(options: { pending?: boolean; incomplete?: boolean; sources?: boolean } = {}) {
    const outcome = { answer: '旧回答原文', incomplete: !!options.incomplete, effects: [], trace: [], memory_updates: [],
      answer_view: { parts: [{ kind: 'markdown', text: '仅当条件成立时，结论才成立。' },
        ...(options.sources === false ? [] : [{ kind: 'sources', source_ref_ids: ['old-a', 'other-b'] }])],
        sources: [{ source_ref_id: 'old-a', label: '旧材料 · 第一章' }, { source_ref_id: 'other-b', label: '另一材料 · 第 9 页' }] } };
    api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current, turns: [
      { turn_id: 'old-turn', user: '旧问题', status: 'completed', outcome, effect_labels: [] },
      { turn_id: 'other-turn', user: '其他问题', status: 'completed', outcome: { ...outcome, answer_view: null, answer: '不要分享的另一回答' }, effect_labels: [] },
    ] } });
    return outcome;
  }
  it('freezes the selected historical answer and resolves multiple original materials without current metadata', async () => {
    setupAnswer();
    api.agentSourceResolve.mockImplementation(async (_turn, id) => ({ label: id === 'old-a' ? '旧材料 · 第一章' : '另一材料 · 第 9 页', stale: id === 'other-b' }));
    const w = start(); await flushPromises(); const rail = w.getComponent({ name: 'RightRail' });
    const beforeMetadata = api.paperMetadata.mock.calls.length;
    rail.vm.$emit('share-answer', rail.props('chat')[0], '结论才成立。'); await flushPromises();
    const panel = w.getComponent({ name: 'ShareImagePanel' });
    expect(panel.props('source')).toMatchObject({ parts: [{ label: '助手解释', text: '结论才成立。' }],
      sources: ['旧材料 · 第一章', '另一材料 · 第 9 页（来源暂不可用）'], answer: { sessionId: 'startup-chat', turnId: 'old-turn' } });
    expect(api.agentSourceResolve.mock.calls).toEqual([['old-turn', 'old-a'], ['old-turn', 'other-b']]);
    expect(api.paperMetadata.mock.calls.length).toBe(beforeMetadata);
    expect(api.save).not.toHaveBeenCalled(); expect(api.agentRunCreate).not.toHaveBeenCalled();
    expect(rail.props('chat')[0].outcome.answer).toBe('旧回答原文');
  });
  it('keeps an unavailable original label and shares unsourced answers without inventing an attribution', async () => {
    setupAnswer(); api.agentSourceResolve.mockRejectedValue(new Error('unavailable'));
    const w = start(); await flushPromises(); const rail = w.getComponent({ name: 'RightRail' });
    rail.vm.$emit('share-answer', rail.props('chat')[0]); await flushPromises();
    expect(w.getComponent({ name: 'ShareImagePanel' }).props('source').sources).toEqual(['旧材料 · 第一章（来源暂不可用）', '另一材料 · 第 9 页（来源暂不可用）']);
    rail.vm.$emit('share-answer', rail.props('chat')[1]); await flushPromises();
    expect(w.getComponent({ name: 'ShareImagePanel' }).props('source')).toMatchObject({ parts: [{ text: '不要分享的另一回答' }], sources: ['无已记录出处'] });
    expect(api.agentSourceResolve).toHaveBeenCalledTimes(2);
  });
  it('rejects incomplete answers and text outside the delivered answer', async () => {
    setupAnswer({ incomplete: true }); const w = start(); await flushPromises(); const rail = w.getComponent({ name: 'RightRail' });
    rail.vm.$emit('share-answer', rail.props('chat')[0]); await flushPromises();
    expect(w.findComponent({ name: 'ShareImagePanel' }).exists()).toBe(false);
    rail.props('chat')[0].outcome.incomplete = false;
    rail.vm.$emit('share-answer', rail.props('chat')[0], '另一回答'); await flushPromises();
    expect(w.findComponent({ name: 'ShareImagePanel' }).exists()).toBe(false);
    rail.props('chat')[0].pending = true;
    rail.vm.$emit('share-answer', rail.props('chat')[0]); await flushPromises();
    expect(w.findComponent({ name: 'ShareImagePanel' }).exists()).toBe(false);
  });
  it.each(['close', 'chat', 'identity', 'publication'])('ignores late original sources after %s', async change => {
    if (change === 'publication') {
      installIdentity({ user_id: 'A', csrf_token: 'a' });
      installWorkspace({ workspace_id: 'w', generation: 1, revision: 1, selected_chat: 'startup-chat', published_book_ref: { book_id: 'startup', publication_id: 'p1' }, reader: { book_id: 'startup', viewport: { anchor_lid: '1.1', top_lid: '1.1', bottom_lid: '1.1', width: 1, visible_lids: ['1.1'] }, layout: { rev: 0, open_slots: [] } } } as never);
    }
    setupAnswer(); const pending = deferred<any>(); api.agentSourceResolve.mockReturnValue(pending.promise);
    const w = start(); await flushPromises(); const rail = w.getComponent({ name: 'RightRail' });
    rail.vm.$emit('share-answer', rail.props('chat')[0]); await flushPromises();
    expect(w.getComponent({ name: 'ShareImagePanel' }).props('loading')).toBe(true);
    if (change === 'close') w.getComponent({ name: 'ShareImagePanel' }).vm.$emit('close');
    if (change === 'chat') { api.agentHistorySelect.mockResolvedValue({ ...history(), active_session_id: 'other', current: { ...history().current, id: 'other' } }); rail.vm.$emit('select-chat', 'other'); }
    if (change === 'identity') installIdentity({ user_id: 'B', csrf_token: 'b' });
    if (change === 'publication') installWorkspace({ ...network.value.workspace!, generation: 2, published_book_ref: { book_id: 'startup', publication_id: 'p2' } });
    await flushPromises(); pending.resolve({ label: '迟到来源', stale: false }); await flushPromises();
    expect(w.findComponent({ name: 'ShareImagePanel' }).exists()).toBe(false);
  });
});


describe('RS7 shared diagram lifetime', () => {
  it.each(['success', 'chat', 'identity', 'new-share'])('handles %s while a diagram is being prepared', async change => {
    let share!: (load: () => Promise<any>) => Promise<void>;
    const w = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: {
      RightRail: defineComponent({ name: 'RightRail', setup() { share = inject(sharePresentationKey)!; return () => null; } }),
    } } }); wrappers.push(w); await flushPromises();
    const source = { parts: [{ id: 'body', label: '图解现场', text: '记录的参数' }], sources: ['旧来源'], association: '版本 2', diagram: { png: 'data:image/png;base64,AA==', width: 640, height: 360, title: '旧图解' } };
    const pending = deferred<any>(); const operation = share(() => pending.promise);
    if (change === 'chat') { api.agentHistorySelect.mockResolvedValue({ ...history(), active_session_id: 'other', current: { ...history().current, id: 'other' } }); w.getComponent({ name: 'RightRail' }).vm.$emit('select-chat', 'other'); }
    if (change === 'identity') installIdentity({ user_id: 'B', csrf_token: 'b' });
    if (change === 'new-share') await share(async () => ({ ...source, association: '版本 3' }));
    await flushPromises(); pending.resolve(source); await operation; await flushPromises();
    if (change === 'success') {
      expect(w.getComponent({ name: 'ShareImagePanel' }).props('source')).toEqual(source);
      w.getComponent({ name: 'ShareImagePanel' }).vm.$emit('close'); await flushPromises();
      expect(w.findComponent({ name: 'ShareImagePanel' }).exists()).toBe(false);
    } else if (change === 'new-share') expect(w.getComponent({ name: 'ShareImagePanel' }).props('source').association).toBe('版本 3');
    else expect(w.findComponent({ name: 'ShareImagePanel' }).exists()).toBe(false);
    expect(api.save).not.toHaveBeenCalled(); expect(api.agentRunCreate).not.toHaveBeenCalled();
  });
});


it('MCP7 opens exact UTF-16 evidence in canonical text without saving a highlight', async () => {
  installIdentity({ user_id: 'alice', csrf_token: 'csrf' });
  const reader = await api.state();
  installWorkspace({ workspace_id: 'mcp-source', generation: 1, revision: 0, selected_chat: null,
    published_book_ref: { book_id: 'startup', publication_id: 'original' }, reader });
  api.goto.mockResolvedValue({ ok: true, viewport: reader.viewport });
  const scroll = vi.fn(async () => true);
  const wrapper = mount(App, { shallow: true, props: { mcpSource: {
    published_book_ref: { book_id: 'startup', publication_id: 'original' },
    start_lid: '1', end_lid: '1', start_utf16: 7, end_utf16: 11,
  } }, global: { renderStubDefaultSlot: true, stubs: { ReaderWorkspace: { methods: { showReader() {} }, template: '<div><slot /></div>' }, ReaderPane: {
    name: 'ReaderPane', props: ['segments', 'renderSeg'], methods: { cancelTypographyRestore() {}, captureScrollAnchor: () => null, scrollLidIntoView: scroll, scrollSourceIntoView: scroll },
    template: '<div><div v-for="seg in segments" :key="seg.lid" v-html="renderSeg(seg)" /></div>',
  } } } });
  wrappers.push(wrapper); await flushPromises();
  expect(wrapper.emitted('mcp-source-error')).toBeUndefined();
  expect(api.goto).toHaveBeenCalledWith('1.1', false);
  expect(wrapper.get('.source-focus-mark').text()).toBe('即阅读的');
  expect(wrapper.emitted('mcp-source-consumed')).toHaveLength(1);
  expect(scroll).toHaveBeenCalledWith('1.1');
  expect(api.save).not.toHaveBeenCalled();
});


it.each(['publication', 'range'])('MCP7 reports an invalid %s instead of navigating to unrelated text', async kind => {
  installIdentity({ user_id: 'alice', csrf_token: 'csrf' });
  installWorkspace({ workspace_id: 'source-invalid', generation: 1, revision: 0, selected_chat: null,
    published_book_ref: { book_id: 'startup', publication_id: 'original' }, reader: await api.state() });
  const wrapper = mount(App, { shallow: true, props: { mcpSource: {
    published_book_ref: { book_id: 'startup', publication_id: kind === 'publication' ? 'wrong' : 'original' },
    start_lid: '1', end_lid: '1', start_utf16: 7, end_utf16: kind === 'range' ? 99 : 11,
  } }, global: { renderStubDefaultSlot: true } });
  wrappers.push(wrapper); await flushPromises();
  expect(wrapper.emitted('mcp-source-error')).toHaveLength(1);
  expect(wrapper.emitted('mcp-source-consumed')).toBeUndefined(); expect(api.goto).not.toHaveBeenCalled();
});

it('projects a saved board part while running, keeps it after cancellation, and restores it from history on reload', async () => {
  const { initialRun } = await import('@understand-book/web/reader/agent-run-state');
  const sources: EventTarget[] = [];
  vi.stubGlobal('EventSource', class extends EventTarget { constructor() { super(); sources.push(this); } close() {} });
  const descriptor = { book_id: 'startup', session_id: 'startup-chat', turn_id: 'board-running' };
  const context = { board_id: 'board-A', board_revision: 7, item_id: 'point-A' };
  const delivery = { board_id: 'board-A', commit_id: 'part-one', before_revision: 7, after_revision: 8, item_ids: ['part-answer'], block_ids: ['part-block'], origin_session_id: 'startup-chat', origin_turn_id: 'board-running', parent_item_id: 'point-A' };
  const pending = { turn_id: 'board-running', user: '接着讲', status: 'pending_assistant', effect_labels: [], board_follow_up: context, board_commits: [] };
  api.agentRun.mockReturnValue(new Promise(() => {}));
  api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current, turns: [pending] } });
  const wrapper = startPurpose(); await flushPromises();
  const rail = wrapper.getComponent({ name: 'RightRail' });
  rail.vm.$emit('open-board-location', { board_id: 'board-A', item_id: 'point-A' }); await flushPromises();
  sources[0].dispatchEvent(new MessageEvent('board.committed', { data: JSON.stringify({ type: 'board.committed', turn_id: descriptor.turn_id, seq: 1, elapsed_ms: 1, payload: { delivery } }) }));
  await flushPromises();
  expect(rail.props('chat')[0]).toMatchObject({ pending: true, boardCommits: [delivery] });
  expect(wrapper.getComponent({ name: 'BoardWorkspace' }).props('run')).toMatchObject({ pending: true, commits: [delivery], status_text: '已保存第1部分，后续仍在讲解' });
  const cancellation = { error_code: 'AGENT_RUN_CANCELLED', category: 'cancelled', message: 'Agent run was cancelled' };
  const stopped = { ...pending, status: 'cancelled', board_commits: [delivery], error: cancellation };
  api.agentHistory.mockResolvedValue({ ...history(), current: { ...history().current, turns: [stopped] } });
  sources[0].dispatchEvent(new MessageEvent('run.cancelled', { data: JSON.stringify({ type: 'run.cancelled', turn_id: descriptor.turn_id, seq: 2, elapsed_ms: 2,
    payload: { ...initialRun(descriptor), execution_state: 'cancelled', persistence_state: 'saved', final_view: stopped, board_commits: [delivery], error: cancellation } }) }));
  await flushPromises();
  expect(rail.props('chat')[0]).toMatchObject({ pending: false, boardIncomplete: true, boardCommits: [delivery], runStatus: '已停止' });
  expect(rail.props('chat')[0].error).toBeUndefined();
  expect(wrapper.getComponent({ name: 'BoardWorkspace' }).props('run')).toMatchObject({ pending: false, incomplete: true, commits: [delivery], status_text: '已停止' });
  expect(wrapper.getComponent({ name: 'BoardWorkspace' }).props('run').error).toBeUndefined();
  wrapper.unmount();
  const reopened = startPurpose(); await flushPromises();
  expect(reopened.getComponent({ name: 'RightRail' }).props('chat')[0]).toMatchObject({ pending: false, boardIncomplete: true, boardCommits: [delivery] });
  expect(reopened.getComponent({ name: 'BoardWorkspace' }).props('run')).toMatchObject({ commits: [delivery], incomplete: true, status_text: '已停止' });
  expect(reopened.getComponent({ name: 'BoardWorkspace' }).props('run').error).toBeUndefined();
  expect(api.agentRunCreate).not.toHaveBeenCalled();
  window.history.replaceState({}, '', '/');
});
