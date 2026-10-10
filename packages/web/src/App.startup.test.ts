// @vitest-environment happy-dom
import { mount, flushPromises } from "@vue/test-utils";
import { defineComponent } from 'vue';
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.vue";
import { network, installIdentity, installWorkspace, submittedRunDrafts } from './network-context';
import { writeReaderSurfacePreference } from './reader-surface';
import * as networkClient from './network-client';

const api = vi.hoisted(() => Object.fromEntries([
  "desktopStatus", "buildWorkbench", "manifest", "assetManifest", "sourceFingerprint",
  "sourceManifest", "state", "profileManifest", "text", "recall", "agentHistory",
  "profileMemory", "profileBackfill", "intentUsageEvent", "intentArtifacts", "bookLibrary",
  "openBook", "agentNew", "disposeEffect", "agentRunCreate", "agentRun", "agentRunRetrySave",
  "tutorState", "tutorReadiness", "tutorMutate", "tutorStart",
].map(key => [key, vi.fn()])));
vi.mock("./api", async original => ({ ...await original<typeof import("./api")>(), api }));
vi.mock("./components/PdfReaderPane.vue", () => ({ default: { name: "PdfReaderPane", template: "<div />" } }));

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

it('enables continuous teaching without inventing a goal or sending a synthetic question', async () => {
  api.tutorState.mockResolvedValue({ control: { enabled: false, revision: 0, current_tutor_session_id: null }, sessions: {} });
  api.tutorMutate.mockResolvedValue({ control: { enabled: true, revision: 1, current_tutor_session_id: null }, sessions: {} });
  api.tutorStart.mockResolvedValue({ started: true, message: '请从原文开始学习' });
  api.agentRunCreate.mockResolvedValue({ answer: '已开始', answer_view: { parts: [] }, effects: [] });
  const w = mount(App, { shallow: true, global: { stubs: { TopBar: defineComponent({ template: '<div><slot name="tutor-control" /></div>' }) } } });
  wrappers.push(w);
  await flushPromises();
  w.findComponent({ name: 'TutorControl' }).vm.$emit('toggle');
  await flushPromises();
  expect(api.tutorStart).not.toHaveBeenCalled();
  expect(api.agentRunCreate).not.toHaveBeenCalled();
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
