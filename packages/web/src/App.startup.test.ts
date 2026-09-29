// @vitest-environment happy-dom
import { mount, flushPromises } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.vue";

const api = vi.hoisted(() => Object.fromEntries([
  "desktopStatus", "buildWorkbench", "manifest", "assetManifest", "sourceFingerprint",
  "sourceManifest", "state", "profileManifest", "text", "recall", "agentHistory",
  "profileMemory", "profileBackfill", "intentUsageEvent", "intentArtifacts", "bookLibrary",
  "openBook", "agentNew",
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
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); });

describe("workspace startup", () => {
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
