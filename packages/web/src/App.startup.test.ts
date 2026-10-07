// @vitest-environment happy-dom
import { mount, flushPromises } from "@vue/test-utils";
import { defineComponent } from 'vue';
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.vue";
import { network, installIdentity, installWorkspace } from './network-context';
import { writeReaderSurfacePreference } from './reader-surface';
import * as networkClient from './network-client';

const api = vi.hoisted(() => Object.fromEntries([
  "desktopStatus", "buildWorkbench", "manifest", "assetManifest", "sourceFingerprint",
  "sourceManifest", "state", "profileManifest", "text", "recall", "agentHistory",
  "profileMemory", "profileBackfill", "intentUsageEvent", "intentArtifacts", "bookLibrary",
  "openBook", "agentNew", "disposeEffect",
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
afterEach(() => {
  wrappers.splice(0).forEach(wrapper => wrapper.unmount());
  network.value = { ...network.value, enabled: false, workspace: null, identity: null };
  localStorage.clear(); sessionStorage.clear();
  vi.restoreAllMocks();
});

describe("workspace startup", () => {
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
