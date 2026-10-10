// @vitest-environment happy-dom
import { mount, flushPromises } from "@vue/test-utils";
import { defineComponent, inject } from 'vue';
import { renderShare, shareHighlightKey, shareNoteKey, sharePresentationKey } from './reading-share';
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.vue";
import { network, installIdentity, installWorkspace, submittedRunDrafts } from './network-context';
import { writeReaderSurfacePreference } from './reader-surface';
import * as networkClient from './network-client';

const api = vi.hoisted(() => Object.fromEntries([
  "desktopStatus", "buildWorkbench", "manifest", "assetManifest", "sourceFingerprint",
  "sourceManifest", "state", "profileManifest", "text", "recall", "agentHistory",
  "profileMemory", "profileBackfill", "intentUsageEvent", "intentArtifacts", "bookLibrary", "paperMetadata",
  "openBook", "agentNew", "disposeEffect", "agentRunCreate", "agentRun", "agentRunRetrySave",
  "save", "replace", "delete", "agentHistorySelect", "agentSourceResolve", "tutorState", "tutorReadiness", "tutorMutate", "tutorStart",
].map(key => [key, vi.fn()])));
vi.mock("./api", async original => ({ ...await original<typeof import("./api")>(), api }));
vi.mock("./components/PdfReaderPane.vue", () => ({ default: { name: "PdfReaderPane", template: "<div />" } }));
vi.mock('./reading-share', async original => ({ ...await original<typeof import('./reading-share')>(), renderShare: vi.fn() }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const history = (book = "startup") => ({ active_session_id: `${book}-chat`, sessions: [],
  current: { id: `${book}-chat`, book_id: book, turns: [], goals: [] } });
let book = "startup";
const wrappers: ReturnType<typeof mount>[] = [];
function start() {
  const wrapper = mount(App, { shallow: true, global: { renderStubDefaultSlot: true } });
  wrappers.push(wrapper);
  return wrapper;
}
beforeEach(() => {
  vi.resetAllMocks();
  book = "startup";
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
  api.profileMemory.mockResolvedValue({});
  api.profileBackfill.mockResolvedValue({ jobs: [], sessions: [] });
  api.intentArtifacts.mockResolvedValue({ overlay: null });
  api.intentUsageEvent.mockResolvedValue({});
  api.bookLibrary.mockResolvedValue({ root: "books", books: [] });
  api.openBook.mockImplementation(async () => { book = "second"; return { ok: true, book_id: book }; });
});

it('sends the real start request with optional teaching assets unavailable', async () => {
  api.tutorState.mockResolvedValue({ control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} });
  api.tutorMutate.mockResolvedValue({ control: { enabled: true, revision: 1, current_tutor_session_id: null }, sessions: {} });
  api.tutorStart.mockResolvedValue({ started: true, message: '请从原文开始学习' });
  api.agentRunCreate.mockResolvedValue({ answer: '已开始', answer_view: { parts: [] }, effects: [] });
  const w = mount(App, { shallow: true, global: { stubs: { TopBar: defineComponent({ template: '<div><slot name="tutor-control" /></div>' }) } } });
  wrappers.push(w);
  await flushPromises();
  w.findComponent({ name: 'TutorControl' }).vm.$emit('toggle');
  await flushPromises();
  expect(api.tutorStart).toHaveBeenCalledOnce();
  expect(api.agentRunCreate).toHaveBeenCalledWith('请从原文开始学习', expect.anything());
});
afterEach(() => {
  wrappers.splice(0).forEach(wrapper => wrapper.unmount());
  network.value = { ...network.value, enabled: false, workspace: null, identity: null };
  localStorage.clear(); sessionStorage.clear();
  submittedRunDrafts.clear();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("workspace startup", () => {
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
    const wrapper = mount(App, { props: { chatDraft: { message: '待回答', quote: null, goalId: null } }, shallow: true, global: { renderStubDefaultSlot: true } });
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
    expect(api.agentRunCreate).toHaveBeenCalledTimes(1);
  });
  it('ADM8 restores a denied draft and quote, exposes the allowance action, and only explicitly continues the original goal', async () => {
    const { ApiError } = await import('./api');
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
    const wrapper = mount(App, { shallow: true, global: { renderStubDefaultSlot: true, stubs: { ReaderPane: false } } });
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
    expect(wrapper.get('.source-preview-table').element.tagName).toBe('DIV');
    expect(wrapper.findAll('.source-preview-table table tr')).toHaveLength(2);
    expect(wrapper.get('.source-preview-table td mark.source-focus-mark').text()).toBe('读者');
    expect(wrapper.find('.source-preview-table mark.hl-mark').exists()).toBe(false);
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
    expect(wrapper.emitted('update:chatDraft')?.at(-1)).toEqual([{ ...draft, message: '继续编辑' }]);
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


describe('RN2 presentation note draft', () => {
  const receipt = { session_id: 'startup-chat', turn_id: 'old-turn', reference: { presentation_id: 'page', revision: 2 }, state_revision: 4, saved_state_ref: 'state-4' };
  it('keeps the captured association across collapse, chat change and save failure', async () => {
    const wrapper = start(); await flushPromises();
    wrapper.findComponent({ name: 'RightRail' }).vm.$emit('record-note', receipt, 'Recorded example');
    await flushPromises();
    let editor = wrapper.findComponent({ name: 'NoteEditorPanel' });
    editor.vm.$emit('update:content', 'my thought'); editor.vm.$emit('collapse'); await flushPromises();
    expect(wrapper.findComponent({ name: 'NoteEditorPanel' }).exists()).toBe(false);
    api.agentNew.mockResolvedValue({ ok: true, history: { ...history(), active_session_id: 'new-chat', current: { ...history().current, id: 'new-chat' } } });
    wrapper.findComponent({ name: 'RightRail' }).vm.$emit('new-chat'); await flushPromises();
    await wrapper.get('.note-resume').trigger('click');
    editor = wrapper.findComponent({ name: 'NoteEditorPanel' });
    expect(editor.props('content')).toBe('my thought');
    api.save.mockRejectedValueOnce(new Error('write failed'));
    editor.vm.$emit('save'); await flushPromises();
    expect(editor.props('error')).toContain('write failed');
    expect(editor.props('content')).toBe('my thought');
    api.save.mockResolvedValue({ status: 'CREATED', record: { mem_id: 'saved' } });
    editor.vm.$emit('save'); await flushPromises();
    expect(api.save).toHaveBeenLastCalledWith(expect.objectContaining({ content: 'my thought', note: { association: { kind: 'presentation', receipt } } }));
    expect(wrapper.findComponent({ name: 'NoteEditorPanel' }).exists()).toBe(false);
    expect(wrapper.findComponent({ name: 'RightRail' }).props('activeChatSessionId')).toBe('new-chat');
  });
  it('edits a reactive saved note through replace and binds follow-up to the selected chat', async () => {
    const wrapper = start(); await flushPromises();
    const record = { mem_id: 'saved', type: 'note', book_id: 'startup', content: 'original', layer: 'long_term', anchor: {}, note: { association: { kind: 'presentation', receipt, title: 'Example' } } };
    wrapper.findComponent({ name: 'RightRail' }).vm.$emit('edit-note', record); await flushPromises();
    const editor = wrapper.findComponent({ name: 'NoteEditorPanel' });
    expect(editor.props('content')).toBe('original');
    editor.vm.$emit('update:content', 'edited');
    api.replace.mockResolvedValue({ ...record, mem_id: 'edited', content: 'edited' });
    editor.vm.$emit('save'); await flushPromises();
    expect(api.replace).toHaveBeenCalledWith({ mem_id: 'saved', content: 'edited', selection_context: undefined });
    api.agentRunCreate.mockResolvedValue({ answer: 'ok', answer_view: { parts: [] }, effects: [] });
    wrapper.findComponent({ name: 'RightRail' }).vm.$emit('note-follow-up', 'explain', receipt, 'edited'); await flushPromises();
    expect(api.agentRunCreate).toHaveBeenCalledWith('explain', expect.objectContaining({ note_mem_id: 'edited', presentation_follow_up: receipt }));
  });
});


describe('RN3 unified note draft', () => {
  const turn = { turnId: 'answer-1', pending: false, outcome: { answer: '__选中的回答__' } };
  const old = { mem_id: 'old', type: 'note', book_id: 'startup', layer: 'long_term', anchor: { lid: '1.1' }, content: '> 原来混排的内容\n\n用户编辑过的文字' };
  const editor = (w: ReturnType<typeof start>) => w.findComponent({ name: 'NoteEditorPanel' });
  async function choose(w: ReturnType<typeof start>, label: string) {
    await w.get('.note-transition').findAll('button').find(b => b.text() === label)!.trigger('click'); await flushPromises();
  }
  it('records answers directly with their fixed turn, preserving the composer and reading position', async () => {
    const w = start(); await flushPromises();
    const rail = w.findComponent({ name: 'RightRail' });
    rail.vm.$emit('update:agentInput', '我的问题');
    rail.vm.$emit('save-answer-selection', turn, '__选中的回答__'); await flushPromises();
    expect(editor(w).props('content')).toBe('');
    expect(editor(w).props('excerpt')).toEqual({ kind: 'assistant', text: '__选中的回答__' });
    api.agentNew.mockResolvedValue({ ok: true, history: { ...history(), active_session_id: 'other', current: { ...history().current, id: 'other' } } });
    rail.vm.$emit('new-chat'); await flushPromises();
    expect(editor(w).exists()).toBe(true);
    editor(w).vm.$emit('save'); await flushPromises();
    expect(api.save).toHaveBeenCalledWith(expect.objectContaining({ content: '', note: { association: { kind: 'answer', session_id: 'startup-chat', turn_id: 'answer-1' }, retained_excerpt: '__选中的回答__' } }));
    expect(rail.props('selectedLid')).toBe('1.1');
  });
  it('processes another note with continue, failure, save and discard without overwriting the draft', async () => {
    const w = start(); await flushPromises(); const rail = w.findComponent({ name: 'RightRail' });
    rail.vm.$emit('edit-note', old); await flushPromises();
    expect(editor(w).props('legacy')).toBe(true);
    expect(editor(w).props('content')).toBe(old.content);
    editor(w).vm.$emit('update:content', '第一次修改');
    rail.vm.$emit('save-answer-selection', turn, '__选中的回答__'); await flushPromises();
    await choose(w, '继续编辑'); expect(editor(w).props('content')).toBe('第一次修改');
    rail.vm.$emit('save-answer-selection', turn, '__选中的回答__'); await flushPromises();
    api.replace.mockRejectedValueOnce(new Error('写入失败'));
    await choose(w, '保存并继续');
    expect(w.find('.note-transition').exists()).toBe(true); expect(editor(w).props('content')).toBe('第一次修改');
    api.replace.mockResolvedValue({ ...old, mem_id: 'new-id', content: '第一次修改' });
    await choose(w, '保存并继续');
    expect(api.replace).toHaveBeenLastCalledWith({ mem_id: 'old', content: '第一次修改', selection_context: undefined });
    expect(editor(w).props('excerpt').kind).toBe('assistant');
    rail.vm.$emit('edit-note', old); await flushPromises(); await choose(w, '放弃并继续');
    expect(editor(w).props('content')).toBe(old.content);
  });
  it('keeps whitespace meaningful to Markdown when saving a legacy note', async () => {
    const w = start(); await flushPromises();
    const record = { ...old, content: '    const n = 1;\n\n' };
    w.findComponent({ name: 'RightRail' }).vm.$emit('edit-note', record); await flushPromises();
    editor(w).vm.$emit('save'); await flushPromises();
    expect(api.replace).toHaveBeenCalledWith({ mem_id: 'old', content: record.content, selection_context: undefined });
  });
  it('guards material switches and unload, and preserves failed text before leaving', async () => {
    const w = start(); await flushPromises();
    w.findComponent({ name: 'RightRail' }).vm.$emit('save-answer-selection', turn, '__选中的回答__'); await flushPromises();
    const event = new Event('beforeunload', { cancelable: true }); window.dispatchEvent(event); expect(event.defaultPrevented).toBe(true);
    const leave = (w.vm as unknown as { beforeNoteLeave: (label: string) => Promise<boolean> }).beforeNoteLeave('切换发布');
    await flushPromises(); await choose(w, '继续编辑'); expect(await leave).toBe(false);
    const exit = (w.vm as unknown as { beforeNoteLeave: (label: string) => Promise<boolean> }).beforeNoteLeave('退出登录');
    await flushPromises(); await choose(w, '放弃并继续'); expect(await exit).toBe(true);
    expect(editor(w).exists()).toBe(false);
    const clean = new Event('beforeunload', { cancelable: true }); window.dispatchEvent(clean); expect(clean.defaultPrevented).toBe(false);
  });
  it('clears private drafts on an identity change and ignores late save failures', async () => {
    installIdentity({ user_id: 'A', csrf_token: 'a' });
    installWorkspace({ workspace_id: 'w', generation: 1, revision: 1, selected_chat: 'startup-chat', published_book_ref: { book_id: 'startup', publication_id: 'p1' }, reader: { book_id: 'startup', viewport: { anchor_lid: '1.1', top_lid: '1.1', bottom_lid: '1.1', visible_lids: ['1.1'], width: 1 }, layout: { rev: 0, open_slots: [] } } } as never);
    const pending = deferred<unknown>(); api.save.mockReturnValue(pending.promise);
    const w = start(); await flushPromises();
    w.findComponent({ name: 'RightRail' }).vm.$emit('save-answer-selection', turn, '__选中的回答__'); await flushPromises();
    editor(w).vm.$emit('save'); await flushPromises();
    installIdentity({ user_id: 'B', csrf_token: 'b' }); await flushPromises();
    pending.reject(new Error('A 的保存错误')); await flushPromises();
    expect(editor(w).exists()).toBe(false); expect(w.text()).not.toContain('A 的保存错误');
  });
  it.each(['ReaderPane', 'RightRail'])('opens original notes from %s with read-only excerpts and atomic text editing', async name => {
    const w = start(); await flushPromises();
    const record = { ...old, content: '自己的话', note: { association: { kind: 'selection' }, retained_excerpt: { kind: 'original', text: '原文片段' } } };
    const surface = w.findComponent({ name });
    surface.vm.$emit('edit-note', record); await flushPromises();
    expect(editor(w).props('excerpt')).toEqual(record.note.retained_excerpt);
    editor(w).vm.$emit('update:content', ''); editor(w).vm.$emit('save'); await flushPromises();
    expect(api.replace).toHaveBeenCalledWith({ mem_id: 'old', content: '', selection_context: undefined });
  });
});


describe('RN4 note continuity', () => {
  const note = { mem_id: 'old', type: 'note', book_id: 'startup', layer: 'long_term', anchor: {}, content: '自己的想法', generated_at: '1700000000',
    note: { material: { book_id: 'startup', publication_id: null }, association: { kind: 'answer', session_id: 'original', turn_id: 'original-turn' }, retained_excerpt: { kind: 'assistant', text: '原回答摘录' }, source_bindings: [] } };
  it('uses the replacement identity immediately and keeps a failed delete visible', async () => {
    api.recall.mockResolvedValue([note]);
    const w = start(); await flushPromises(); const rail = w.getComponent({ name: 'RightRail' });
    rail.vm.$emit('edit-note', note); await flushPromises();
    const saved = { ...note, mem_id: 'edited', content: '修订文字' };
    api.replace.mockResolvedValue(saved); api.recall.mockResolvedValue([saved]);
    const editor = w.getComponent({ name: 'NoteEditorPanel' }); editor.vm.$emit('update:content', saved.content); editor.vm.$emit('save'); await flushPromises();
    expect(rail.props('contextNotes')).toEqual([saved]); expect(w.text()).toContain('笔记已保存');
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    api.delete.mockRejectedValueOnce(new Error('删除失败'));
    rail.vm.$emit('delete-note', saved); await flushPromises(); expect(rail.props('contextNotes')).toEqual([saved]);
    api.delete.mockResolvedValue({ ok: true }); api.recall.mockResolvedValue([]);
    rail.vm.$emit('delete-note', saved); await flushPromises(); expect(api.delete).toHaveBeenLastCalledWith('edited');
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
  expect(w.getComponent({ name: 'RightRail' }).props('contextHighlights')).toEqual([{ ...highlight, content: '前半段\n后半段搜索目标' }]);
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
    api.recall.mockResolvedValue([note]); api.paperMetadata.mockResolvedValue({ title: { value: '理解学习的方法' } });
    const w = startSharing(); await flushPromises();
    const rail = w.getComponent({ name: 'RightRail' }); rail.vm.$emit('edit-note', note); await flushPromises();
    w.getComponent({ name: 'NoteEditorPanel' }).vm.$emit('update:content', '未保存的修改'); await flushPromises();
    await rail.get('button').trigger('click'); await flushPromises();
    const share = w.getComponent({ name: 'ShareImagePanel' });
    expect(share.props('source')).toMatchObject({ parts: [{ label: '阅读笔记', text: note.content }], sources: ['《理解学习的方法》', '首章'] });
    share.vm.$emit('close'); await flushPromises();
    expect(w.getComponent({ name: 'NoteEditorPanel' }).props('content')).toBe('未保存的修改');
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
