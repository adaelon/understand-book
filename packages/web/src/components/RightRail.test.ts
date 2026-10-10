// @vitest-environment happy-dom
import { flushPromises, mount } from "@vue/test-utils";
import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import RightRail from "./RightRail.vue";
import { api } from '../api';
import { renderMarkdown } from '../md';
import { network, installIdentity, installWorkspace } from '../network-context';
import type { RecapTarget, SessionRecap } from '../session-recap';

const baseProps = {
  chat: [],
  chatSessions: [],
  activeChatSessionId: "chat-1",
  agentInput: "",
  sending: false,
  unquotedNotePlacementAvailable: false,
  askDraft: null,
  showTrace: {},
  latestTrace: [],
  selectedLid: "1.1",
  selectedFormula: null,
  contextNotes: [],
  contextHighlights: [],
  renderMarkdown: (source: string) => source,
  effLabel: () => "effect",
  effState: () => undefined,
  isGoto: () => false,
  showEffectPrimary: () => false,
  showEffectSecondary: () => false,
  effectPrimaryLabel: () => "keep",
  effectSecondaryLabel: () => "undo",
  gotoBack: () => "1.1",
};

describe("RightRail history visibility", () => {
  it('ADM8 exposes allowance from a stop while retaining partial answers and requiring an explicit click', async () => {
    installIdentity({ user_id: 'A', csrf_token: 'a' });
    const wrapper = mount(RightRail, { props: { ...baseProps, chat: [{ turnId: 'stopped', user: '问题', pending: false,
      error: 'raw error', spendStop: { code: 'ALLOWANCE_INSUFFICIENT', message: '本期额度不足。本次已完成内容已保存。' },
      questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
      outcome: { answer: '已完成的部分回答', effects: [], trace: [], memory_updates: [], turns: 1, tokens_spent: 0, incomplete: true,
        profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] } } as never,
    }] } });
    try {
      expect(wrapper.text()).toContain('已完成的部分回答');
      expect(wrapper.text()).not.toContain('raw error');
      expect(wrapper.emitted('continue-goal')).toBeUndefined();
      await wrapper.findAll('button').find(button => button.text() === '查看使用额度')!.trigger('click');
      expect(wrapper.emitted('show-allowance')).toHaveLength(1);
    } finally { wrapper.unmount(); network.value = { ...network.value, identity: null, enabled: false }; }
  });
  const emptySession = (id: string) => ({
    id, title: "New chat", created_at: "t0", updated_at: "t0", turn_count: 0, turns: [],
  });
  const savedSession = (id: string) => ({
    ...emptySession(id), title: "已有问题", turn_count: 1,
    turns: [{ user: "已有问题", question_source_label: null, question_quote: null }],
  });

  it.each([
    { name: "first-open empty session", sessions: [emptySession("chat-1")], count: 0 },
    { name: "repeated new empty sessions", sessions: [emptySession("chat-1"), emptySession("chat-2"), emptySession("chat-3")], count: 0 },
    { name: "saved conversation alongside empty sessions", sessions: [emptySession("chat-1"), savedSession("saved"), emptySession("old-empty")], count: 1 },
  ])("shows only conversations with questions: $name", async ({ sessions, count }) => {
    const wrapper = mount(RightRail, { props: { ...baseProps, chatSessions: sessions } });
    try {
      expect(wrapper.get(".chat-actions .history-button span").text()).toBe(String(count));
      if (count === 0) {
        expect(wrapper.get(".transcript .empty").text()).toContain("本书还没有保存的对话");
        expect(wrapper.find(".empty-history-button").exists()).toBe(false);
      } else {
        expect(wrapper.get(".transcript .empty").text()).toContain("本书有 1 段已保存的历史对话");
        expect(wrapper.get(".empty-history-button").text()).toContain("1 段历史对话");
      }
      await wrapper.get(".chat-actions .history-button").trigger("click");
      expect(document.body.querySelectorAll(".history-card")).toHaveLength(count);
      expect(document.body.querySelector(".history-card.active")).toBeNull();
      if (count === 0) {
        expect(document.body.querySelector(".history-list")?.textContent).toContain("暂无保存的对话历史");
      } else {
        expect(document.body.querySelector(".history-card h4")?.textContent).toBe("已有问题");
        (document.body.querySelector(".history-open") as HTMLButtonElement).click();
        expect(wrapper.emitted("select-chat")).toEqual([["saved"]]);
      }
    } finally { wrapper.unmount(); }
  });

  it.each(["pending", "failed", "cancelled"])("keeps the current conversation with a %s answer", async (status) => {
    const wrapper = mount(RightRail, { props: {
      ...baseProps, chatSessions: [savedSession("chat-1"), emptySession("old-empty")],
      chat: [{ turnId: "turn-1", user: "已有问题", pending: status === "pending", outcome: null,
        error: status === "failed" ? "回答失败" : undefined,
        runStatus: status === "cancelled" ? "已停止" : undefined,
        questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [] }],
    } });
    try {
      expect(wrapper.get(".chat-actions .history-button span").text()).toBe("1");
      expect(wrapper.find(".empty-history-button").exists()).toBe(false);
      await wrapper.get(".chat-actions .history-button").trigger("click");
      expect(document.body.querySelectorAll(".history-card")).toHaveLength(1);
      expect(document.body.querySelector(".history-card.active .active-badge")?.textContent).toBe("当前");
      expect((document.body.querySelector(".history-open") as HTMLButtonElement).disabled).toBe(true);
    } finally { wrapper.unmount(); }
  });

  it("updates the open history list when a question is saved and when the last conversation is deleted", async () => {
    const wrapper = mount(RightRail, { props: { ...baseProps, chatSessions: [emptySession("chat-1")] } });
    try {
      await wrapper.get(".chat-actions .history-button").trigger("click");
      expect(document.body.querySelectorAll(".history-card")).toHaveLength(0);
      await wrapper.setProps({ chatSessions: [savedSession("chat-1"), emptySession("chat-2")], activeChatSessionId: "chat-2" });
      expect(wrapper.get(".chat-actions .history-button span").text()).toBe("1");
      expect(wrapper.get(".empty-history-button").text()).toContain("1 段历史对话");
      expect(document.body.querySelectorAll(".history-card")).toHaveLength(1);
      await wrapper.setProps({ chatSessions: [emptySession("chat-2")] });
      expect(wrapper.get(".chat-actions .history-button span").text()).toBe("0");
      expect(wrapper.find(".empty-history-button").exists()).toBe(false);
      expect(document.body.querySelectorAll(".history-card")).toHaveLength(0);
      expect(document.body.querySelector(".history-list")?.textContent).toContain("暂无保存的对话历史");
    } finally { wrapper.unmount(); }
  });
});

describe("RightRail history recovery", () => {
  it('shows oldest turns first while effect and trace actions retain their original turn indexes', async () => {
    const effect = { kind: 'Note' as const, mem_id: 'new-note', lid: '1.1', text: '笔记' };
    const turns = ['old', 'new'].map(turnId => ({ turnId, user: turnId, pending: false,
      questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
      outcome: { answer: turnId, effects: [effect], trace: [{}], memory_updates: [], turns: 1, tokens_spent: 0, incomplete: false,
        profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] } } as never }));
    const wrapper = mount(RightRail, { props: { ...baseProps, chat: turns, effState: () => '已保留' } });
    try {
      expect(wrapper.findAll('.turn').map(t => t.attributes('data-turn-id'))).toEqual(['old', 'new']);
      await wrapper.findAll('.turn')[1].get('.undo').trigger('click');
      expect(wrapper.emitted('undo-effect')).toEqual([[1, 0, effect]]);
      await wrapper.findAll('.turn')[1].get('.trace-toggle').trigger('click');
      expect(wrapper.emitted('toggle-trace')).toEqual([[1]]);
      expect(turns.map(t => t.turnId)).toEqual(['old', 'new']);
    } finally { wrapper.unmount(); }
  });
  it('follows new turns at the bottom, preserves older reading, and resets when switching chats', async () => {
    const turn = { turnId: 'old', user: '旧问题', pending: true, outcome: null,
      questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [] };
    const wrapper = mount(RightRail, { props: { ...baseProps, chat: [turn] } });
    try {
      const transcript = wrapper.get('.transcript');
      const el = transcript.element as HTMLElement;
      Object.defineProperty(el, 'scrollHeight', { value: 1000, configurable: true });
      Object.defineProperty(el, 'clientHeight', { value: 200, configurable: true });
      el.scrollTop = 790;
      await transcript.trigger('scroll');
      await wrapper.setProps({ chat: [turn, { ...turn, turnId: 'new' }] });
      await flushPromises();
      expect(el.scrollTop).toBe(1000);
      el.scrollTop = 200;
      await transcript.trigger('scroll');
      await wrapper.setProps({ chat: [turn, { ...turn, turnId: 'new' }, { ...turn, turnId: 'newer' }] });
      await flushPromises();
      expect(el.scrollTop).toBe(200);
      await wrapper.setProps({ activeChatSessionId: 'other', chat: [{ ...turn, turnId: 'other-turn' }] });
      await flushPromises();
      expect(el.scrollTop).toBe(1000);
    } finally { wrapper.unmount(); }
  });
  it("loads delivered presentations when history and chat identity arrive together", async () => {
    const wrapper = mount(RightRail, { props: { ...baseProps, activeChatSessionId: '' }, global: { stubs: { AgentPresentation: true } } });
    await wrapper.setProps({ activeChatSessionId: 'restored-chat', chat: [{ turnId: 'restored-turn', user: 'question', pending: false, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [], outcome: {
      answer: '', answer_view: { parts: [{ kind: 'presentation', presentation_id: 'p', revision: 1 }], sources: [] },
      incomplete: false, warning: null, turns: 1, tokens_spent: 0, effects: [], trace: [], memory_updates: [],
      profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] },
    } }] });
    expect(wrapper.find('agent-presentation-stub').exists()).toBe(true);
    wrapper.unmount();
  });
  it("keeps the draft editable, blocks chat changes until recovery, and offers retry", async () => {
    const wrapper = mount(RightRail, { props: { ...baseProps, agentInput: "保留草稿", historyLoading: true } });
    expect(wrapper.get('[role="status"]').text()).toContain("正在恢复对话");
    expect((wrapper.get(".new-chat").element as HTMLButtonElement).disabled).toBe(true);
    expect((wrapper.get(".agent-input textarea").element as HTMLTextAreaElement).disabled).toBe(false);
    expect((wrapper.get(".agent-input button:last-child").element as HTMLButtonElement).disabled).toBe(true);
    await wrapper.setProps({ historyLoading: false, historyError: "暂时无法连接" });
    await wrapper.get('[role="alert"] button').trigger("click");
    expect(wrapper.emitted("retry-history")).toHaveLength(1);
    await wrapper.setProps({ historyError: null });
    expect((wrapper.get(".agent-input button:last-child").element as HTMLButtonElement).disabled).toBe(false);
    wrapper.unmount();
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  network.value = { ...network.value, enabled: false, workspace: null, identity: null };
  document.body.replaceChildren();
  vi.unstubAllGlobals();
});

describe('JL9 recap jumps', () => {
  it('offers undo of a retained note and removes that action after its receipt', async () => {
    const effect = { kind: 'Note' as const, mem_id: 'note', lid: '1.1', text: '笔记' };
    const wrapper = mount(RightRail, { props: { ...baseProps, effState: () => '已保留', chat: [{
      turnId: 'turn', user: '保留笔记', pending: false, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
      outcome: { answer: '笔记', effects: [effect], trace: [], memory_updates: [], turns: 1, tokens_spent: 0, incomplete: false,
        profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] } } as never,
    }] } });
    try {
      await wrapper.findAll('button').find(b => b.text() === '撤销保留')!.trigger('click');
      expect(wrapper.emitted('undo-effect')).toEqual([[0, 0, effect]]);
      await wrapper.setProps({ effState: () => '已撤销' });
      expect(wrapper.findAll('button').some(b => b.text() === '撤销保留')).toBe(false);
    } finally { wrapper.unmount(); }
  });
  const evidence = [{ turn_id: 'turn-1', event_seq: 3 }];
  const source = { source_ref_id: 'source-1', label: '原文', quote: '引文', published_book_ref: null, evidence, unavailable_reason: null };
  const note = { effect_id: 'memory:original', label: '笔记', status: 'kept', effect: { kind: 'reader' as const, effect: { kind: 'Note' as const, mem_id: 'original', lid: '1.1', text: '笔记' } }, object_id: 'kept-object', published_book_ref: null, evidence, unavailable_reason: null };
  const presentation = { ...note, effect_id: 'presentation:p:2', object_id: null, effect: { kind: 'presentation' as const, reference: { presentation_id: 'p', revision: 2 } } };
  const recap: SessionRecap = { session_id: 'chat-1', through_seq: 10, through_at: 'now', generated_at: 'now', questions: [{ text: '原问题', status: 'answered', evidence }], sources: [source], effects: [note, presentation], continuations: [] };
  const chat = [{ turnId: 'turn-1', user: '原问题', pending: false, outcome: null, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [] }];
  const base = { session_id: 'chat-1', through_seq: 10, turn_id: 'turn-1' };
  it('opens without navigation, then locates the turn, bound source, kept object and exact presentation version', async () => {
    vi.spyOn(api, 'sessionRecap').mockResolvedValue(recap);
    const resolve = vi.spyOn(api, 'agentSourceResolve').mockResolvedValue({ source_ref_id: 'source-1', label: '原文', highlighted_quote: '引文', context_before: '', context_after: '', stale: false, can_open_in_reader: true });
    const read = vi.spyOn(api, 'presentationRead').mockResolvedValue({} as never);
    vi.spyOn(api, 'recall').mockResolvedValue([{ mem_id: 'kept-object', anchor: { lid: '1.2' }, content: '保留后的笔记' }] as never);
    const scroll = vi.fn(); vi.stubGlobal('HTMLElement', HTMLElement);
    const prior = HTMLElement.prototype.scrollIntoView;
    HTMLElement.prototype.scrollIntoView = scroll;
    const wrapper = mount(RightRail, { props: { ...baseProps, chat }, global: { stubs: { AgentPresentation: true } } });
    try {
      await wrapper.get('.recap-button').trigger('click'); await flushPromises();
      expect(document.body.querySelector('.session-recap')).not.toBeNull();
      expect(resolve).not.toHaveBeenCalled(); expect(read).not.toHaveBeenCalled(); expect(scroll).not.toHaveBeenCalled();
      const rail = wrapper.vm as unknown as { openRecapTarget: (t: RecapTarget) => Promise<void> };
      await rail.openRecapTarget({ ...base, kind: 'turn' }); expect(scroll).toHaveBeenCalled();
      await rail.openRecapTarget({ ...base, kind: 'source', source });
      expect(resolve).toHaveBeenCalledWith('turn-1', 'source-1');
      await wrapper.setProps({ chat: [{ ...chat[0], pending: true, runStatus: '新活动' }] });
      expect(document.body.querySelector('.agent-source-popup')).not.toBeNull();
      await rail.openRecapTarget({ ...base, kind: 'effect', effect: note });
      expect(wrapper.emitted('focus-source')?.[0]).toEqual([{ lid: '1.2', quote: null, memId: 'kept-object' }]);
      await rail.openRecapTarget({ ...base, kind: 'effect', effect: presentation });
      expect(read).toHaveBeenCalledWith('chat-1', 'turn-1', { presentation_id: 'p', revision: 2 });
      expect(wrapper.getComponent({ name: 'AgentPresentation' }).props('reference')).toEqual({ presentation_id: 'p', revision: 2 });
      await wrapper.setProps({ activeChatSessionId: 'chat-2', chat: [] });
      expect(document.body.querySelector('.session-recap')).toBeNull();
      expect(wrapper.find('agent-presentation-stub').exists()).toBe(false);
    } finally { wrapper.unmount(); HTMLElement.prototype.scrollIntoView = prior; }
  });
  it('requests the original publication before any source navigation in another publication', async () => {
    installIdentity({ user_id: 'A', csrf_token: 'csrf' });
    installWorkspace({ workspace_id: 'w', generation: 1, revision: 1, selected_chat: 'chat-1', published_book_ref: { book_id: 'book', publication_id: 'new' }, reader: {} } as never);
    const original = { ...source, published_book_ref: { book_id: 'book', publication_id: 'original' } };
    vi.spyOn(api, 'sessionRecap').mockResolvedValue({ ...recap, sources: [original] });
    const resolve = vi.spyOn(api, 'agentSourceResolve');
    const wrapper = mount(RightRail, { props: { ...baseProps, chat } });
    try {
      const target: RecapTarget = { ...base, kind: 'source', source: original };
      await (wrapper.vm as unknown as { openRecapTarget: (t: RecapTarget) => Promise<void> }).openRecapTarget(target);
      expect(wrapper.emitted('recap-publication')).toEqual([[target]]);
      expect(resolve).not.toHaveBeenCalled();
    } finally { wrapper.unmount(); }
  });
});

describe("RightRail fullscreen", () => {
  it("keeps the live answer and draft mounted across fullscreen changes, and exits with Escape", async () => {
    const wrapper = mount(RightRail, {
      attachTo: document.body,
      props: { ...baseProps, agentInput: "未发送的问题", chat: [{ turnId: "turn-1", user: "问题", outcome: null, pending: true, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [], runStatus: "正在运行" }] },
    });
    const transcript = wrapper.get(".transcript").element;
    const input = wrapper.get(".agent-input textarea").element;
    await wrapper.get(".fullscreen-button").trigger("click");
    expect(wrapper.emitted("toggle-fullscreen")).toHaveLength(1);
    await wrapper.setProps({ fullscreen: true });
    expect(wrapper.classes()).toContain("fullscreen");
    expect(wrapper.find(".context-tabs").exists()).toBe(false);
    expect(wrapper.get(".transcript").element).toBe(transcript);
    expect(wrapper.get(".agent-input textarea").element).toBe(input);
    expect((input as HTMLTextAreaElement).value).toBe("未发送的问题");
    expect(wrapper.text()).toContain("正在运行");

    await wrapper.get(".history-button").trigger("click");
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await flushPromises();
    expect(document.body.querySelector(".history-backdrop")).toBeNull();
    expect(wrapper.emitted("toggle-fullscreen")).toHaveLength(1);

    const presentation = document.createElement("div");
    presentation.className = "agent-presentation expanded";
    document.body.append(presentation);
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(wrapper.emitted("toggle-fullscreen")).toHaveLength(1);
    presentation.remove();

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(wrapper.emitted("toggle-fullscreen")).toHaveLength(2);
    await wrapper.setProps({ fullscreen: false });
    expect(wrapper.classes()).not.toContain("fullscreen");
    expect(wrapper.get(".transcript").element).toBe(transcript);
    wrapper.unmount();
  });
});

describe("RightRail Note placement actions", () => {
  it("offers Markdown move for body Notes and placement for legacy, but never for selection or PDF Notes", async () => {
    const currentSource = "a".repeat(64);
    const note = (memId: string, overrides: Record<string, unknown> = {}) => ({
      mem_id: memId,
      type: "note",
      layer: "long_term",
      book_id: "book-a",
      anchor: { lid: "1.1" },
      content: `${memId} body`,
      ...overrides,
    });
    const body = note("body", {
      note_placement: { kind: "lid_block", source_fingerprint: currentSource, lid: "1.1" },
    });
    const stale = note("stale", {
      note_placement: { kind: "lid_block", source_fingerprint: "b".repeat(64), lid: "1.1" },
    });
    const legacy = note("legacy");
    const selected = note("selected", {
      selection_context: { status: "resolved", raw_quote: "x", resolved_quote: "x", ranges: [] },
    });
    const pdf = note("pdf", {
      note_placement: {
        kind: "pdf_region",
        source_fingerprint: currentSource,
        lid: "1.1",
        source_map_version: "pdf_source_map.v1",
        source_map_config_hash: "cfg",
        page_index: 0,
        region_id: "r1",
      },
    });
    const wrapper = mount(RightRail, {
      props: {
        ...baseProps,
        unquotedNotePlacementAvailable: true,
        noteSourceFingerprint: currentSource,
        contextNotes: [body, stale, legacy, selected, pdf],
      },
    });

    await wrapper.findAll("button.tab").find((button) => button.text().includes("笔记"))!.trigger("click");
    const actions = wrapper.findAll("[data-note-placement-action]");
    expect(actions.map((button) => button.attributes("data-mem-id"))).toEqual(["body", "stale", "legacy"]);
    expect(actions.map((button) => button.text())).toEqual(["移动", "重新放置", "放置到正文"]);

    await actions[1]!.trigger("click");
    expect(wrapper.emitted("place-note")?.[0]).toEqual([stale]);
    expect(wrapper.find('[data-mem-id="legacy"] .note-source-button').exists()).toBe(false);
    expect(wrapper.find('[data-mem-id="stale"] .note-source-button').exists()).toBe(false);
    expect(wrapper.find('[data-mem-id="body"] .note-source-button').exists()).toBe(true);
    expect(wrapper.find('[data-mem-id="selected"] .note-source-button').exists()).toBe(true);
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

function sourceOutcome(stale = false) {
  return {
    answer: "First claim. Next claim.",
    answer_view: {
      parts: [
        { kind: "markdown" as const, text: "First claim." },
        { kind: "sources" as const, source_ref_ids: ["source_ref_a"] },
        { kind: "markdown" as const, text: " Next claim." },
        { kind: "sources" as const, source_ref_ids: ["source_ref_a", "source_ref_b"] },
      ],
      sources: [
        { source_ref_id: "source_ref_a", label: "正文 · Methods" },
        { source_ref_id: "source_ref_b", label: "正文 · Results" },
      ],
    },
    incomplete: false,
    warning: null,
    turns: 3,
    tokens_spent: 12,
    effects: [{ kind: "Goto" as const, before_anchor: "1.1", after_anchor: "1.2" }],
    trace: [],
    profile_usage: {
      snapshot_revision: 0,
      injected_fact_ids: [],
      claimed_used_fact_ids: [],
      influences: [],
    },
    memory_updates: [],
    stale,
  };
}

describe("RightRail agent sources", () => {
  it.each(["completed", "draft"])("keeps a %s table intact when sources occur inside cells", async (state) => {
    const view = {
      parts: [
        { kind: "markdown" as const, text: "| 字段 | 说明 | 依据 |\n| --- | --- | --- |\n| msg | 问题 | 书中解释 " },
        { kind: "sources" as const, source_ref_ids: ["source_ref_a"] },
        { kind: "markdown" as const, text: " |\n| quote | 选区 | 另一处解释 " },
        { kind: "sources" as const, source_ref_ids: ["source_ref_a", "source_ref_b"] },
        { kind: "markdown" as const, text: " |\n| user | 身份 | 见下方说明 |" },
      ],
      sources: [
        { source_ref_id: "source_ref_a", label: "正文 · Methods" },
        { source_ref_id: "source_ref_b", label: "正文 · Results" },
      ],
    };
    const outcome = { ...sourceOutcome(), answer_view: view };
    const turn = {
      turnId: "turn-table", user: "question", pending: state === "draft",
      outcome: state === "completed" ? outcome : null,
      draft: state === "draft" ? { message_id: 1, revision: 1, operation: "replace", view } : null,
      questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
    };
    const wrapper = mount(RightRail, { props: { ...baseProps, renderMarkdown, chat: [turn] } });
    try {
      const answer = wrapper.get(state === "draft" ? ".answer-draft" : ".a-msg .ans-text");
      expect(answer.findAll("tbody tr")).toHaveLength(3);
      expect(answer.findAll("tbody td")).toHaveLength(9);
      expect(answer.findAll("tbody td .agent-source-button")).toHaveLength(2);
      expect(answer.findAll("p").some(p => p.text().includes("| quote |"))).toBe(false);
      expect(answer.findAll("tbody td .agent-source-button")[1].text()).toContain("2 个来源");
      if (state === "draft") {
        const updatedView = {
          ...view,
          parts: [...view.parts.slice(0, -1), { kind: "markdown" as const, text: " |\n| user | 身份 | 见下方说明 |\n| goal | 目标 | 已保存 |" }],
        };
        await wrapper.setProps({ chat: [{ ...turn, draft: { message_id: 1, revision: 2, operation: "replace", view: updatedView } }] });
        expect(wrapper.get(".answer-draft").findAll("tbody tr")).toHaveLength(4);
      }
      const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const body = JSON.parse(String(init?.body ?? "{}"));
        return new Response(JSON.stringify({
          source_ref_id: body.source_ref_id, label: "正文 · Methods", highlighted_quote: "evidence",
          context_before: "", context_after: "", stale: false, can_open_in_reader: true,
        }), { status: 200 });
      });
      vi.stubGlobal("fetch", fetchMock);
      await wrapper.findAll("tbody td .agent-source-button")[1].trigger("click");
      await flushPromises();
      expect(fetchMock.mock.calls.map(([, init]) => JSON.parse(String(init?.body)).source_ref_id)).toEqual([
        "source_ref_a", "source_ref_b",
      ]);
    } finally { wrapper.unmount(); }
  });

  it.each(["after the closing pipe", "on the next line"])("keeps a table source $0 inside its preceding row", (position) => {
    const parts = [
      { kind: "markdown" as const, text: `| 字段 | 依据 |\n| --- | --- |\n| msg | 问题 |${position === "on the next line" ? "\n" : ""}` },
      { kind: "sources" as const, source_ref_ids: ["source_ref_a"] },
      { kind: "markdown" as const, text: "\n| quote | 选区 |" },
    ];
    const wrapper = mount(RightRail, { props: {
      ...baseProps, renderMarkdown,
      chat: [{
        turnId: "turn-table", user: "question", pending: false,
        outcome: { ...sourceOutcome(), answer_view: { parts, sources: [{ source_ref_id: "source_ref_a", label: "正文 · Methods" }] } },
        questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
      }],
    } });
    try {
      const answer = wrapper.get(".a-msg .ans-text");
      expect(answer.findAll("tbody tr")).toHaveLength(2);
      expect(answer.findAll("tbody td .agent-source-button")).toHaveLength(1);
    } finally { wrapper.unmount(); }
  });

  it("shows context shortage only for an actual context-budget warning", () => {
    const deliveryFailure = {
      ...sourceOutcome(),
      answer: "这次回答生成失败，请重试。",
      answer_view: undefined,
      incomplete: true,
      warning: null,
    };
    const contextBudget = {
      ...sourceOutcome(),
      answer: "Partial answer.",
      answer_view: undefined,
      incomplete: true,
      warning: "CONTEXT_BUDGET_EXCEEDED",
    };
    const wrapper = mount(RightRail, {
      attachTo: document.body,
      props: {
        ...baseProps,
        askDraft: null,
        chat: [
          {
            turnId: "turn-delivery",
            user: "question one",
            outcome: deliveryFailure,
            pending: false,
            questionAnchorLid: null,
            questionQuote: null,
            questionSelection: null,
            effectLabels: [],
          },
          {
            turnId: "turn-budget",
            user: "question two",
            outcome: contextBudget,
            pending: false,
            questionAnchorLid: null,
            questionQuote: null,
            questionSelection: null,
            effectLabels: [],
          },
        ],
      },
    });

    expect(wrapper.text()).toContain("这次回答生成失败，请重试。");
    const notices = wrapper.findAll(".incomplete");
    expect(notices).toHaveLength(1);
    expect(notices[0].text()).toBe("未完成: 上下文不足");
  });

  it("distinguishes compaction, physical capacity, and tool-turn stop reasons", () => {
    const warnings = [
      ["COMPACTION_FAILED", "上下文整理失败，请重试"],
      ["ACTIVE_CONTEXT_EXHAUSTED", "当前内容超过模型可处理范围"],
      ["TURN_LIMIT_EXCEEDED", "本轮模型—工具循环次数已达上限"],
    ] as const;
    const wrapper = mount(RightRail, {
      attachTo: document.body,
      props: {
        ...baseProps,
        askDraft: null,
        chat: warnings.map(([warning], index) => ({
          turnId: `turn-warning-${index}`,
          user: `question ${index}`,
          outcome: {
            ...sourceOutcome(),
            answer_view: undefined,
            incomplete: true,
            warning,
          },
          pending: false,
          questionAnchorLid: null,
          questionQuote: null,
          questionSelection: null,
          effectLabels: [],
        })),
      },
    });

    expect(wrapper.findAll(".incomplete").map((notice) => notice.text())).toEqual(
      warnings.map(([, label]) => `未完成: ${label}`),
    );
    expect(wrapper.text()).not.toContain("上下文不足");
  });

  it("renders inline single and grouped source buttons without visible LIDs", () => {
    const wrapper = mount(RightRail, {
      attachTo: document.body,
      props: {
        ...baseProps,
        askDraft: null,
        chat: [{
          turnId: "turn-a",
          user: "question",
          outcome: sourceOutcome(),
          pending: false,
          questionAnchorLid: null,
          questionQuote: { label: "正文 · Introduction", quote: "quoted text", status: "resolved" },
          questionSelection: null,
          effectLabels: ["跳转 · 正文 · Results"],
        }],
      },
    });

    const buttons = wrapper.findAll(".agent-source-button");
    expect(buttons).toHaveLength(2);
    expect(buttons[0].text()).toContain("Methods [1]");
    expect(buttons[1].text()).toContain("2 个来源");
    expect(wrapper.get(".turn-quote-head").text()).toContain("正文 · Introduction");
    expect(wrapper.get(".proposal .prop-label").text()).toBe("跳转 · 正文 · Results");
    expect(wrapper.get(".agent-panel").text()).not.toContain("1.1");
    expect(wrapper.get(".agent-panel").text()).not.toContain("1.2");
  });

  it("resolves on first click and opens the reader only from the secondary action", async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      const body = JSON.parse(String(init?.body ?? "{}"));
      if (url.endsWith("/agent/source.resolve")) {
        return new Response(JSON.stringify({
          source_ref_id: body.source_ref_id,
          label: "正文 · Methods",
          highlighted_quote: "exact evidence",
          context_before: "substantial context before",
          context_after: "substantial context after",
          stale: false,
          can_open_in_reader: true,
        }), { status: 200 });
      }
      return new Response(JSON.stringify({ source_ref_id: body.source_ref_id, opened: true }), { status: 200 });
    });
    vi.stubGlobal("fetch", fetchMock);
    const wrapper = mount(RightRail, {
      attachTo: document.body,
      props: {
        ...baseProps,
        askDraft: null,
        chat: [{
          turnId: "turn-a",
          user: "question",
          outcome: sourceOutcome(),
          pending: false,
          questionAnchorLid: null,
          questionQuote: null,
          questionSelection: null,
          effectLabels: [],
        }],
      },
    });

    await wrapper.findAll(".agent-source-button")[0].trigger("click");
    await flushPromises();

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(String(fetchMock.mock.calls[0][0])).toContain("/agent/source.resolve");
    expect(JSON.parse(String(fetchMock.mock.calls[0][1]?.body))).toEqual({
      turn_id: "turn-a",
      source_ref_id: "source_ref_a",
    });
    expect(document.body.querySelector(".agent-source-popup")?.textContent).toContain("exact evidence");
    expect(document.body.querySelector(".source-excerpt")?.textContent).toContain("substantial context before");
    expect(wrapper.emitted("agent-source-opened")).toBeUndefined();

    const open = document.body.querySelector<HTMLButtonElement>(".source-open-reader")!;
    open.click();
    await flushPromises();
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(String(fetchMock.mock.calls[1][0])).toContain("/agent/source.open");
    expect(wrapper.emitted("agent-source-will-open")).toEqual([[
      { turnId: "turn-a", sourceRefId: "source_ref_a" },
    ]]);
    expect(wrapper.emitted("agent-source-opened")).toHaveLength(1);
  });

  it("shows stale snapshots and disables reader navigation", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({
      source_ref_id: "source_ref_a",
      label: "正文 · Methods",
      highlighted_quote: "saved preview",
      context_before: "",
      context_after: "",
      stale: true,
      can_open_in_reader: false,
    }), { status: 200 })));
    const wrapper = mount(RightRail, {
      attachTo: document.body,
      props: {
        ...baseProps,
        askDraft: null,
        chat: [{
          turnId: "turn-a",
          user: "question",
          outcome: sourceOutcome(true),
          pending: false,
          questionAnchorLid: null,
          questionQuote: null,
          questionSelection: null,
          effectLabels: [],
        }],
      },
    });

    await wrapper.findAll(".agent-source-button")[0].trigger("click");
    await flushPromises();

    expect(document.body.querySelector(".source-stale")?.textContent).toContain("已失效");
    expect(document.body.querySelector<HTMLButtonElement>(".source-open-reader")?.disabled).toBe(true);
  });

  it("keeps the newest source click and ignores a superseded late resolve", async () => {
    const first = deferred<Response>();
    const second = deferred<Response>();
    const responses = [first, second];
    vi.stubGlobal("fetch", vi.fn(() => responses.shift()!.promise));
    const wrapper = mount(RightRail, {
      attachTo: document.body,
      props: {
        ...baseProps,
        askDraft: null,
        chat: [{
          turnId: "turn-a",
          user: "question",
          outcome: sourceOutcome(),
          pending: false,
          questionAnchorLid: null,
          questionQuote: null,
          questionSelection: null,
          effectLabels: [],
        }],
      },
    });

    const source = wrapper.findAll(".agent-source-button")[0];
    await source.trigger("click");
    await source.trigger("click");
    second.resolve(new Response(JSON.stringify({
      source_ref_id: "source_ref_a",
      label: "new source",
      highlighted_quote: "new evidence",
      context_before: "new before",
      context_after: "new after",
      stale: false,
      can_open_in_reader: true,
    }), { status: 200 }));
    await flushPromises();
    expect(document.body.querySelector(".agent-source-popup")?.textContent).toContain("new evidence");

    first.resolve(new Response(JSON.stringify({
      source_ref_id: "source_ref_a",
      label: "old source",
      highlighted_quote: "old evidence",
      context_before: "old before",
      context_after: "old after",
      stale: false,
      can_open_in_reader: true,
    }), { status: 200 }));
    await flushPromises();
    expect(document.body.querySelector(".agent-source-popup")?.textContent).toContain("new evidence");
    expect(document.body.querySelector(".agent-source-popup")?.textContent).not.toContain("old evidence");
  });

  it("does not restore a closed popup or emit navigation after a late open", async () => {
    const pendingOpen = deferred<Response>();
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
      if (String(input).endsWith("/agent/source.open")) return pendingOpen.promise;
      return new Response(JSON.stringify({
        source_ref_id: "source_ref_a",
        label: "source",
        highlighted_quote: "evidence",
        context_before: "before",
        context_after: "after",
        stale: false,
        can_open_in_reader: true,
      }), { status: 200 });
    }));
    const wrapper = mount(RightRail, {
      attachTo: document.body,
      props: {
        ...baseProps,
        askDraft: null,
        chat: [{
          turnId: "turn-a",
          user: "question",
          outcome: sourceOutcome(),
          pending: false,
          questionAnchorLid: null,
          questionQuote: null,
          questionSelection: null,
          effectLabels: [],
        }],
      },
    });

    await wrapper.findAll(".agent-source-button")[0].trigger("click");
    await flushPromises();
    document.body.querySelector<HTMLButtonElement>(".source-open-reader")!.click();
    await flushPromises();
    document.body.querySelector<HTMLButtonElement>('.source-popup-head button[aria-label="关闭来源"]')!.click();
    await flushPromises();
    pendingOpen.resolve(new Response(JSON.stringify({ source_ref_id: "source_ref_a", opened: true }), { status: 200 }));
    await flushPromises();

    expect(document.body.querySelector(".agent-source-popup")).toBeNull();
    expect(wrapper.emitted("agent-source-opened")).toBeUndefined();
  });
});

describe("RightRail AskQuote", () => {
  it("shows recovered resolved provenance as an ordinary citation", async () => {
    const recovered = {
      lid: "1.1",
      quote: "feed-forward",
      status: "resolved" as const,
      resolution_basis: "recovered" as const,
      raw_quote: "feed-forward",
      resolved_quote: "feed-forward",
      ranges: [{ lid: "1.1", range: { start: 0, end: 12 } }],
    };
    const wrapper = mount(RightRail, {
      attachTo: document.body,
      props: { ...baseProps, askDraft: recovered },
    });

    await flushPromises();
    expect(wrapper.get(".ask-draft").text()).not.toContain("部分定位");
    expect(wrapper.get(".ask-draft blockquote").text()).toBe("feed-forward");
  });

  it("shows partial provenance while preserving the user-visible raw quote", async () => {
    const partial = {
      lid: "1.1",
      quote: "raw visible quote",
      status: "partial" as const,
      raw_quote: "raw visible quote",
      resolved_quote: "resolved quote",
      ranges: [{ lid: "1.1", range: { start: 1, end: 3 } }],
    };
    const wrapper = mount(RightRail, {
      attachTo: document.body,
      props: { ...baseProps, askDraft: null },
    });
    await wrapper.setProps({ askDraft: partial });
    await flushPromises();
    expect(wrapper.get(".ask-draft").text()).toContain("部分定位");
    expect(wrapper.get(".ask-draft blockquote").text()).toBe("raw visible quote");
    expect(document.activeElement).toBe(wrapper.get("textarea").element);

    await wrapper.setProps({
      askDraft: null,
      chat: [{
        turnId: null,
        user: "question",
        outcome: null,
        pending: false,
        questionAnchorLid: "1.1",
        questionQuote: { label: "部分定位引用", quote: partial.quote, status: partial.status },
        questionSelection: partial,
        effectLabels: [],
      }],
    });
    expect(wrapper.get(".turn-quote").text()).toContain("部分定位");
    expect(wrapper.get(".turn-quote blockquote").text()).toBe("raw visible quote");
  });

  it("keeps legacy lid/quote drafts and sends only on explicit non-empty input", async () => {
    const wrapper = mount(RightRail, {
      attachTo: document.body,
      props: { ...baseProps, askDraft: { lid: "1.1", quote: "legacy quote" } },
    });
    expect(wrapper.get(".ask-draft").text()).toContain("引用来源");
    expect(wrapper.get(".ask-draft").text()).not.toContain("部分定位");
    expect(wrapper.get(".agent-compose-row > button").attributes()).toHaveProperty("disabled");
    await wrapper.get("textarea").setValue("question");
    await wrapper.setProps({ agentInput: "question" });
    await wrapper.get(".agent-compose-row > button").trigger("click");
    expect(wrapper.emitted("send-agent")).toHaveLength(1);

    const app = readFileSync("src/App.vue", "utf8");
    expect(app).toContain("await api.agentRunCreate(msg, {");
    expect(app).not.toContain("const outbound = draft");
    expect(app).toContain("question_quote: draft ? { ...draft } : null");
  });

  it("exposes every auxiliary surface as a labelled keyboard-operable tab", async () => {
    const wrapper = mount(RightRail, { attachTo: document.body, props: baseProps });
    const tabs = wrapper.findAll('.context-tabs [role="tab"]');
    expect(tabs.map((tab) => tab.text().trim())).toEqual(["问答", "成果", "画像", "轨迹", "公式", "笔记"]);
    expect(tabs[0].attributes("aria-selected")).toBe("true");
    expect(wrapper.get('.secondary-tabs').isVisible()).toBe(false);
    await wrapper.get('[aria-label="阅读工具"]').trigger('click');
    expect(wrapper.get('.secondary-tabs').isVisible()).toBe(true);
    await tabs[4].trigger("click");
    expect(tabs[4].attributes("aria-selected")).toBe("true");
    expect(wrapper.get("#reader-panel-formula").attributes("role")).toBe("tabpanel");
    wrapper.unmount();
  });

  it("keeps profile updates quiet, undoable, and exposes usage only on demand", async () => {
    const profileMemory = {
      current_book_id: "book-a",
      status: {
        document_revision: 2,
        projection_revision: 2,
        profile_status: "current",
        pending_sensitive_confirmation: false,
        pending_review_jobs: 0,
        review_error: null,
      },
      snapshot: {
        source_revision: 2,
        profile_status: "current",
        global_core: [],
        applicable_global: [],
        book_state_core: [],
        profile_projection: [],
        pending_context: [],
      },
      facts: [],
      pending_candidates: [{
        fact_id: "fact-pending",
        scope_kind: "global",
        scope_value: null,
        applicability_kind: "any",
        applicability_value: null,
        payload_kind: "goal",
        payload_key: "reading",
        payload_value: "understand",
        source: "agent_inferred",
        capture: "current_interaction",
        status: "pending",
        sensitivity: "normal",
        evidence_ids: [],
        created_at: "t0",
        updated_at: "t0",
        valid_until: null,
        supersedes: [],
      }],
      evidence: [],
      collection_rules: [],
    };
    const memoryUpdate = {
      kind: "remembered" as const,
      operation_id: "op-1",
      fact_ids: ["fact-1"],
      message: null,
    };
    const wrapper = mount(RightRail, {
      attachTo: document.body,
      props: {
        ...baseProps,
        askDraft: null,
        profileMemory,
        chat: [{
          turnId: null,
          user: "remember this",
          pending: false,
          error: undefined,
          questionAnchorLid: "1.1",
          questionQuote: null,
          questionSelection: null,
          effectLabels: [],
          outcome: {
            answer: "done",
            incomplete: false,
            warning: null,
            turns: 1,
            tokens_spent: 3,
            effects: [],
            trace: [],
            profile_usage: {
              snapshot_revision: 2,
              injected_fact_ids: ["fact-1"],
              claimed_used_fact_ids: ["fact-1"],
              influences: ["explanation_depth"],
            },
            memory_updates: [memoryUpdate],
          },
        }],
      },
    });

    expect(wrapper.get(".memory-update-row").text()).toContain("画像已记住");
    expect(wrapper.get(".profile-usage").attributes()).not.toHaveProperty("open");
    expect(wrapper.get(".profile-usage summary").text()).toContain("画像依据 · 1");
    await wrapper.get('.memory-update-row button[aria-label="撤销画像更新"]').trigger("click");
    expect(wrapper.emitted("undo-profile-update")?.[0]).toEqual([0, 0, memoryUpdate]);

    const profileTab = wrapper.findAll(".tab").find((tab) => tab.text().includes("画像"));
    expect(profileTab?.text()).toContain("1");
    await profileTab!.trigger("click");
    expect(wrapper.emitted("refresh-profile")).toHaveLength(1);
    expect(wrapper.get(".profile-memory-panel").isVisible()).toBe(true);
  });

  it("renders persisted QueryAudit without placing it in the result digest", async () => {
    const audit = {
      budget_version: "referent-first-v1",
      model_calls: 2,
      request: {
        query: "command 是什么",
        intent: "definition" as const,
        targets: ["command"],
        obligations: [{ requirement: "给出定义" }],
        anchor_lid: "1.1",
      },
      plan_gate: { valid: true, missing_requirements: [], target_issues: [] },
      candidate_rounds: [{
        round: 0,
        targets: [{
          target_index: 0,
          target: "command",
          candidates: [{
            candidate_id: "entity:command",
            kind: "entity" as const,
            sources: ["graph" as const],
            labels: ["command"],
            aliases: [],
            recall_strength: "direct" as const,
            match_reasons: ["exact label"],
            occurrence_count: 1,
            excerpts: [{ lid: "1.1", text: "command source" }],
            hint_only: null,
          }],
        }],
      }],
      candidate_fits: [{
        round: 0,
        target_index: 0,
        candidate_id: "entity:command",
        fit: "direct_match",
        reason: "fixture",
      }],
      probes: [],
      bindings: [{
        target: "command",
        candidate_id: "entity:command",
        kind: "entity" as const,
        canonical_label: "command",
        source_lids: ["1.1"],
      }],
      selected_bindings: [{
        target_index: 0,
        candidate_id: "entity:command",
        round: 0,
        rank: 1,
      }],
      evidence: {
        seed_lids: ["1.1"],
        expansion_lids: [],
        expansion_rounds: 0,
        skipped_lids: [],
        chars_used: 14,
        mandatory_overflow_used: 0,
        mandatory_overflow_reasons: [],
      },
      assessments: [{
        obligation_index: 0,
        verdict: "supported" as const,
        citation_lids: ["1.1"],
        support_note: "source support",
      }],
      structural_gate: {
        bindings_complete: true,
        assessments_complete: true,
        citations_valid: true,
        all_obligations_supported: true,
      },
      outcome_status: "complete",
    };
    const wrapper = mount(RightRail, {
      props: {
        ...baseProps,
        askDraft: null,
        latestTrace: [{
          tool: "book.query",
          args: "typed request",
          result_digest: "complete response only",
          model_tool_loop: 2,
          query_audit: audit,
        }],
      },
    });
    const traceTab = wrapper.findAll(".tab").find((tab) => tab.text().includes("轨迹"));
    await traceTab!.trigger("click");
    expect(wrapper.get(".panel-head h3").text()).toBe("2 个模型—工具循环 · 1 次工具调用");
    expect(wrapper.get(".trace-loop").text()).toBe("循环 2");
    expect(wrapper.get(".trace-result").text()).toBe("complete response only");
    const panel = wrapper.get(".query-audit");
    expect(panel.text()).toContain("QueryAudit");
    await panel.get("summary").trigger("click");
    expect(panel.text()).toContain("entity:command");
    expect(panel.text()).toContain("seed: 1.1");
    expect(panel.text()).toContain("citations=true");
  });

  it("opens the active target artifacts without coupling their refresh or evidence navigation to chat", async () => {
    const wrapper = mount(RightRail, {
      props: {
        ...baseProps,
        askDraft: null,
        intentArtifacts: {
          version: "intent_artifact_overlay.v1" as const,
          book_id: "book-private",
          intent_id: "intent-private",
          plan_id: "plan-private",
          plan_digest: "f".repeat(64),
          artifacts: [{
            artifact_id: "timeline-1",
            artifact_type: "timeline" as const,
            state: "accepted" as const,
            payload_digest: "e".repeat(64),
            accepted_at: "2026-07-26T02:00:00.000Z",
            payload: {
              items: [{ id: "event-1", label: "First event", evidence_lids: ["4.2"] }],
            },
          }],
        },
      },
    });

    const artifactsTab = wrapper.findAll(".tab").find((tab) => tab.text().includes("成果"));
    expect(artifactsTab?.text()).toContain("1");
    await artifactsTab!.trigger("click");

    expect(wrapper.emitted("open-artifacts")).toHaveLength(1);
    expect(wrapper.emitted("refresh-artifacts")).toBeUndefined();
    expect(wrapper.get(".intent-artifact-panel").isVisible()).toBe(true);
    await wrapper.get('button[data-lid="4.2"]').trigger("click");
    expect(wrapper.emitted("goto")?.at(-1)).toEqual(["4.2"]);
    expect(wrapper.emitted("artifact-cited")?.at(-1)).toEqual(["timeline-1"]);
    expect(wrapper.get(".artifact-panel").text()).not.toContain("book-private");
    expect(wrapper.get(".artifact-panel").text()).not.toContain("plan-private");
  });
});

describe("Resident activities", () => {
  it("shares live steps with trace, emits stop and preserves upward scrolling", async () => {
    const activity = { step_id: 1, parent_step_id: null, kind: "model", name: "outer", label: "生成回答", status: "running" as const, started_ms: 0, duration_ms: null, result_count: null, error_code: null, usage_total_tokens: null, usage: null, model_first_text_ms: null, model_name: null, model_name_source: null, accepted_evidence_count: null, evidence_refs: [] };
    const turn = { turnId: "turn", user: "问题", outcome: null, pending: true, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [], activities: [activity], runStatus: "正在运行" };
    const wrapper = mount(RightRail, { props: { ...baseProps, chat: [turn], sending: true, canStop: true } });
    expect(wrapper.find(".transcript .agent-activity").attributes("data-status")).toBe("running");
    await wrapper.find(".stop-agent").trigger("click");
    expect(wrapper.emitted("stop-agent")).toHaveLength(1);
    await wrapper.findAll("button.tab").find(button => button.text().includes("轨迹"))!.trigger("click");
    expect(wrapper.findAll('.agent-activity[data-step-id="1"]')).toHaveLength(2);
    const transcript = wrapper.find(".transcript");
    const el = transcript.element as HTMLElement;
    Object.defineProperty(el, "scrollHeight", { value: 1000, configurable: true });
    Object.defineProperty(el, "clientHeight", { value: 200, configurable: true });
    el.scrollTop = 50;
    await transcript.trigger("scroll");
    await wrapper.setProps({ chat: [{ ...turn, pending: false, runStatus: "已停止", activities: [{ ...activity, status: "cancelled" as const, duration_ms: 100 }] }], sending: false, canStop: false });
    expect(el.scrollTop).toBe(50);
    expect(wrapper.find(".pending").exists()).toBe(false);
    expect(wrapper.find(".run-status").text()).toBe("已停止");
    expect(wrapper.findAll('.agent-activity[data-status="cancelled"]')).toHaveLength(2);
    wrapper.unmount();
  });
});

describe("RightRail Resident task controls", () => {
  it("keeps running tasks collapsed and exposes only the stop action beside the draft", async () => {
    const wrapper = mount(RightRail, { props: { ...baseProps, sending: true, canStop: true, agentInput: "下个问题",
      chatGoals: [{ id: "goal-running", revision: 1, interpretation: "制作全书主线演示",
        requirements: [{ id: "page", description: "交付演示", basis_turn_id: "t1", verification: "presentation_delivery" as const }],
        working: { focus: "制作页面", open_questions: [], next_move: "交付", items: [] },
        result_refs: [], status: "open" as const, last_stop_reason: null }] } });
    try {
      expect(wrapper.findAll(".agent-compose-row > button")).toHaveLength(1);
      expect(wrapper.get(".agent-compose-row > button").text()).toBe("停止");
      expect(wrapper.find(".goal-card").exists()).toBe(false);
      expect(wrapper.get(".task-summary-toggle").text()).toContain("正在执行");
      expect(wrapper.text()).not.toContain("等待继续");
      expect(wrapper.text()).not.toContain("页面交付待确认");
      await wrapper.get(".task-summary-toggle").trigger("click");
      expect(wrapper.get(".goal-card").text()).toContain("交付演示");
      expect(wrapper.get(".goal-card").text()).toContain("正在执行");
    } finally { wrapper.unmount(); }
  });

  it("preserves drafts and older reading while task details open and close", async () => {
    const wrapper = mount(RightRail, { props: { ...baseProps, agentInput: "保留这份草稿", chatGoals: [{
      id: "goal-saved", revision: 1, interpretation: "解释关系", requirements: [],
      working: { focus: "", open_questions: [], next_move: "", items: [] },
      result_refs: [], status: "open" as const, last_stop_reason: null }] } });
    try {
      const transcript = wrapper.get(".transcript").element as HTMLElement;
      transcript.scrollTop = 80;
      await wrapper.get(".task-summary-toggle").trigger("click");
      expect(wrapper.get(".goal-card").text()).toContain("尚未完成");
      await wrapper.get('[aria-label="关闭任务详情"]').trigger("click");
      expect(wrapper.find(".goal-card").exists()).toBe(false);
      expect((wrapper.get("textarea").element as HTMLTextAreaElement).value).toBe("保留这份草稿");
      expect(transcript.scrollTop).toBe(80);
    } finally { wrapper.unmount(); }
  });

  it("does not send another turn through Ctrl+Enter while a run is active", async () => {
    const wrapper = mount(RightRail, { props: { ...baseProps, sending: true, canStop: true, agentInput: "先写草稿" } });
    try {
      await wrapper.get("textarea").trigger("keydown", { key: "Enter", ctrlKey: true });
      expect(wrapper.emitted("send-agent")).toBeUndefined();
      await wrapper.get(".stop-agent").trigger("click");
      expect(wrapper.emitted("stop-agent")).toHaveLength(1);
      expect((wrapper.get("textarea").element as HTMLTextAreaElement).value).toBe("先写草稿");
    } finally { wrapper.unmount(); }
  });

  it("does not leave a completed task notice in the input area", () => {
    const wrapper = mount(RightRail, { props: { ...baseProps, chatGoals: [{
      id: "goal-1", revision: 2, interpretation: "解释术语", requirements: [],
      working: { focus: "", open_questions: [], next_move: "", items: [] },
      result_refs: ["answer:t1"], status: "completed" as const, last_stop_reason: null,
    }] } });
    expect(wrapper.find(".goal-card").exists()).toBe(false);
    expect(wrapper.find(".agent-input").text()).not.toContain("最近任务已完成");
  });
  it("shows a saved open task and sends explicit continue and cancel targets", async () => {
    const wrapper = mount(RightRail, { props: { ...baseProps, chatGoals: [{
      id: "goal-1", revision: 2, interpretation: "把这一章做成演示页",
      requirements: [{ id: "page", description: "交付演示", basis_turn_id: "t1", verification: "presentation_delivery" as const }],
      working: { focus: "核对交付", open_questions: [], next_move: "交付页面", items: [{ id: "page", description: "制作页面", status: "completed" as const }] },
      result_refs: ["answer:t1"], status: "open" as const, last_stop_reason: "TURN_LIMIT_EXCEEDED",
    }] } });
    await wrapper.get('.task-summary-toggle').trigger('click');
    expect(wrapper.find(".goal-card").text()).toContain("交付要求：交付演示");
    expect(wrapper.find(".goal-card").text()).toContain("本次达到运行上限");
    await wrapper.findAll(".goal-actions button")[0]!.trigger("click");
    await wrapper.get('.task-summary-toggle').trigger('click');
    await wrapper.findAll(".goal-actions button")[1]!.trigger("click");
    await wrapper.get('.task-summary-toggle').trigger('click');
    await wrapper.findAll(".goal-actions button")[2]!.trigger("click");
    expect(wrapper.emitted("continue-goal")?.[0]).toEqual(["goal-1"]);
    expect(wrapper.emitted("target-goal")?.[0]).toEqual(["goal-1"]);
    expect(wrapper.emitted("cancel-goal")?.[0]).toEqual(["goal-1"]);
    await wrapper.setProps({ targetGoalId: "goal-1" });
    expect(wrapper.find(".goal-target").text()).toContain("补充任务：把这一章做成演示页");
    await wrapper.get('[aria-label="取消补充任务"]').trigger("click");
    expect(wrapper.emitted("clear-goal-target")).toHaveLength(1);
  });
});

describe("mobile input continuity", () => {
  it("does not submit a composing IME value and keeps Ctrl+Enter for an explicit submit", async () => {
    const wrapper = mount(RightRail, { props: { ...baseProps, agentInput: "草稿" } });
    const input = wrapper.get(".agent-input textarea");
    await input.trigger("keydown", { key: "Enter", ctrlKey: true, isComposing: true });
    expect(wrapper.emitted("send-agent")).toBeUndefined();
    await input.trigger("keydown", { key: "Enter", ctrlKey: true, isComposing: false });
    expect(wrapper.emitted("send-agent")).toHaveLength(1);
  });
});

