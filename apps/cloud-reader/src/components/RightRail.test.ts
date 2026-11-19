// @vitest-environment happy-dom
import { flushPromises, mount, enableAutoUnmount } from "@vue/test-utils";
import { nextTick, reactive, ref } from 'vue';
import { readingGuidanceKey, type ReadingGuidanceRequest } from '@understand-book/web/reader/reading-guidance';
import appSource from '@understand-book/web/reader/App.vue?raw';
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import RightRail from "@understand-book/web/reader/components/RightRail.vue";
import YeyeMascot from '@understand-book/web/reader/components/YeyeMascot.vue';
import { yeyeAssets } from '@understand-book/web/reader/yeye-assets';
import { api } from '@understand-book/web/reader/api';
import { renderMarkdown } from '@understand-book/web/reader/md';
import mathAnswer from '@understand-book/web/reader/fixtures/agent-math-answer.md?raw';
import { network, installIdentity, installWorkspace } from '../network-context';
import type { RecapTarget, SessionRecap } from '@understand-book/web/reader/session-recap';

enableAutoUnmount(afterEach);
// Note placement has its own component suite. Its requests must not consume the
// source-popup HTTP fixtures in these conversation tests.
vi.mock('@understand-book/web/reader/components/MessageThoughts.vue', () => ({ default: { template: '<div><slot /></div>' } }));

const baseProps = {
  chat: [],
  chatSessions: [],
  activeChatSessionId: "chat-1",
  agentInput: "",
  sending: false,
  unquotedNotePlacementAvailable: false,
  askDraft: null,
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

it('offers an explicit presentation page switch without sending or rewriting the question', async () => {
  const wrapper = mount(RightRail, { props: { ...baseProps, agentInput: '解释一下这里' }, shallow: true });
  const toggle = wrapper.get('.presentation-request');
  expect(toggle.attributes('aria-checked')).toBe('false');
  await toggle.trigger('click');
  expect(wrapper.emitted('update:presentationRequested')).toEqual([[true]]);
  expect(wrapper.emitted('send-agent')).toBeUndefined();
  expect(wrapper.emitted('update:agentInput')).toBeUndefined();
  expect(wrapper.get('.presentation-request-hint').text()).toBe('本次回答制作演示页');
  await toggle.trigger('click');
  expect(toggle.attributes('aria-checked')).toBe('false');
  await wrapper.setProps({ sending: true });
  expect(toggle.attributes('disabled')).toBeDefined();
});

it('allows sending an image-only video question and locks sending while an upload is in progress', async () => {
  const video = { media_id: 'v', media_revision: '1' };
  const question = { draft_id: 'image-question', publication: { book_id: 'b', publication_id: 'p' },
    material: { book_id: 'b', video, transcript: { transcript_id: 't', transcript_revision: '1', video } },
    frame: { video, actual_pts_ms: 15466.6875, width: 1920, height: 1080 }, region: null, range: null,
    image: { png_base64: 'pixels', width: 100, height: 80, origin: 'upload' as const } };
  const wrapper = mount(RightRail, { props: { ...baseProps, videoImagesEnabled: true, videoQuestion: question }, shallow: true });
  expect(wrapper.get('.send-agent').attributes('disabled')).toBeUndefined();
  await wrapper.get('.send-agent').trigger('click'); expect(wrapper.emitted('send-agent')).toHaveLength(1);
  wrapper.findComponent({ name: 'QuestionImageInput' }).vm.$emit('loading', true); await nextTick();
  expect(wrapper.get('.send-agent').attributes('disabled')).toBeDefined();
  wrapper.unmount();
});

it('attaches music screenshots, allows image-only questions, and shows the image in history', async () => {
  const question = { draft_id: 'score-question', publication: { book_id: 'b', publication_id: 'p' },
    audio: { recording: { recording_id: 'r', revision: '1' }, start_ms: 0, end_ms: 1000 },
    score: null, occurrence_id: null, focused_part_ids: [],
    image: { png_base64: 'score-pixels', width: 640, height: 240, origin: 'selection' as const } };
  const wrapper = mount(RightRail, { props: { ...baseProps, musicMode: true, musicQuestion: question,
    chat: [{ turnId: 'music-turn', user: '这个符号是什么？', musicQuestion: question, effectLabels: [] }] as never }, shallow: true });
  expect(wrapper.get('.attach-image').attributes('aria-label')).toBe('替换图片');
  expect(wrapper.findComponent({ name: 'QuestionImageInput' }).props('image')).toEqual(question.image);
  expect(wrapper.get('.turn-question-image img').attributes('src')).toBe('data:image/png;base64,score-pixels');
  await wrapper.get('.send-agent').trigger('click'); expect(wrapper.emitted('send-agent')).toHaveLength(1);
  wrapper.findComponent({ name: 'QuestionImageInput' }).vm.$emit('loading', true); await nextTick();
  expect(wrapper.get('.send-agent').attributes('disabled')).toBeDefined();
  wrapper.findComponent({ name: 'QuestionImageInput' }).vm.$emit('loading', false);
  await wrapper.setProps({ musicQuestion: { ...question, image: null } });
  expect(wrapper.get('.send-agent').attributes('disabled')).toBeDefined();
  expect(wrapper.get('.attach-image').attributes('aria-label')).toBe('添加图片');
});

it('requests formula details when the panel opens, selection changes or a read is retried', async () => {
  const wrapper = mount(RightRail, { props: { ...baseProps, formulaSelected: true }, shallow: true });
  await wrapper.get('#reader-tab-formula').trigger('click');
  expect(wrapper.emitted('request-formula')).toHaveLength(1);
  await wrapper.setProps({ formulaLoading: true });
  expect(wrapper.get('#reader-panel-formula').text()).toContain('正在读取公式解释');
  await wrapper.setProps({ formulaLoading: false, formulaError: '读取失败' });
  await wrapper.get('#reader-panel-formula [role="alert"] button').trigger('click');
  expect(wrapper.emitted('request-formula')).toHaveLength(2);
  await wrapper.setProps({ selectedLid: '1.2', formulaError: null });
  expect(wrapper.emitted('request-formula')).toHaveLength(3);
  expect(wrapper.get('#reader-panel-formula').text()).toContain('暂时没有解释资料');
  wrapper.unmount();
});

it('shows first-visit ghost text and accepts it without sending', async () => {
  const draft = '我想先弄清这一节讨论的主要问题。';
  const load = vi.fn(async () => ({ question: '读这一节，你最想弄清什么？', draft }));
  const wrapper = mount(RightRail, { props: { ...baseProps, learningPurposeAvailable: true }, shallow: true,
    global: { provide: { [readingGuidanceKey as symbol]: { load, context: ref({ key: 'book:chat', title: '当前材料',
      scopes: [{ value: 'section', label: '当前小节', lid: '1.2' }], selection: null, intent: '', medium: 'text' }) } } } });
  await flushPromises();
  const input = wrapper.get<HTMLTextAreaElement>('.agent-input textarea');
  expect(input.element.value).toBe(''); expect(input.attributes('placeholder')).toBe(draft);
  expect(wrapper.get('.send-agent').attributes('disabled')).toBeDefined();
  await input.trigger('keydown', { key: 'Tab' });
  expect(wrapper.emitted('update:agentInput')).toEqual([[draft]]);
  expect(wrapper.emitted('send-agent')).toBeUndefined();
  await wrapper.setProps({ agentInput: '我有自己的问题' });
  await input.trigger('keydown', { key: 'Tab' });
  expect(wrapper.emitted('update:agentInput')).toHaveLength(1);
  expect(load).toHaveBeenCalledTimes(1);
});

it('puts the first-question learning entry in the conversation', () => {
  const wrapper = mount(RightRail, { props: baseProps, shallow: true,
    global: { stubs: { Teleport: false } },
    slots: { 'learning-goal': '<p>这次想弄懂什么？</p>' } });
  expect(wrapper.get('#reader-panel-agent').text()).toContain('这次想弄懂什么？');
  expect(wrapper.get('textarea[data-workspace-input="agent"]').attributes('placeholder')).toContain('学习目标');
  wrapper.unmount();
});

function guidanceChat(user = '我觉得只应该算行驶时间', answer = '平均速度要用总路程除以全过程的总时间。') {
  return { turnId: 'discussion-1', user, pending: false,
    outcome: { ...sourceOutcome(), answer, answer_view: { parts: [{ kind: 'markdown' as const, text: answer }], sources: [] } },
    questionAnchorLid: '1.1', questionQuote: { label: '原文', quote: '平均速度是总路程除以总时间。', status: 'resolved' as const },
    questionSelection: null, effectLabels: [] };
}
function discussionGuidance(load: ReturnType<typeof vi.fn>, props = {}) {
  return mount(RightRail, { props: { ...baseProps, learningPurposeAvailable: true, chat: [guidanceChat()], ...props }, shallow: true,
    global: { provide: { [readingGuidanceKey as symbol]: { load, context: ref({ key: 'book:chat', title: '平均速度',
      scopes: [{ value: 'selection', label: '所选原文' }, { value: 'material', label: '整份材料' }],
      selection: { lid: '1.2', quote: '停留时间也计入总时间。' }, intent: '', medium: 'text' }) } } } });
}

it('invites a reason for the actual user thought using the last two delivered exchanges', async () => {
  const guide = { question: '你原来只算行驶时间，是怎么考虑的？', draft: '我原来只算行驶时间，是因为……' };
  const load = vi.fn(async (_request: ReadingGuidanceRequest) => guide);
  const old = { ...guidanceChat('更早的问题', '更早的解释'), turnId: 'old' };
  const previous = { ...guidanceChat('请解释平均速度', '平均速度描述全程。'), turnId: 'previous' };
  const wrapper = discussionGuidance(load, { chat: [old, previous, guidanceChat()] });
  await flushPromises();
  expect(load.mock.calls[0][0]).toMatchObject({ purpose: 'followup', scope: 'material', recent_turns: [
    { turn_id: 'previous', user: '请解释平均速度', assistant: '平均速度描述全程。' },
    { turn_id: 'discussion-1', user: '我觉得只应该算行驶时间', assistant: '平均速度要用总路程除以全过程的总时间。' },
  ] });
  expect(wrapper.get('.reading-input-invitation').text()).toBe(guide.question);
  const input = wrapper.get<HTMLTextAreaElement>('.agent-input textarea');
  expect(input.element.value).toBe(''); expect(input.attributes('placeholder')).toBe(guide.draft);
  await input.trigger('keydown', { key: 'Tab' });
  expect(wrapper.emitted('update:agentInput')).toEqual([[guide.draft]]);
  expect(wrapper.emitted('send-agent')).toBeUndefined();
  await wrapper.setProps({ agentInput: '因为停留时没有走过路程，但我不确定。' });
  expect(wrapper.find('.reading-input-invitation').exists()).toBe(false);
  expect(load).toHaveBeenCalledTimes(1);
});

it('prioritizes a new source selection even when a conversation already exists', async () => {
  const load = vi.fn(async () => ({ question: '关于停留时间，你想追问什么？', draft: '关于这段话，我想说的是……' }));
  const wrapper = discussionGuidance(load, { askDraft: { lid: '1.2', quote: '停留时间也计入总时间。', status: 'resolved' } });
  await flushPromises();
  expect(load).toHaveBeenCalledWith(expect.objectContaining({ purpose: 'selection', scope: 'selection', lid: '1.2', quote: '停留时间也计入总时间。' }));
  expect(wrapper.get('.agent-input textarea').attributes('placeholder')).toBe('关于这段话，我想说的是……');
});

it('leaves manual input and normal Tab navigation available when the model chooses silence', async () => {
  const load = vi.fn(async () => null), wrapper = discussionGuidance(load);
  await flushPromises();
  expect(wrapper.find('.reading-input-invitation').exists()).toBe(false);
  expect(wrapper.findComponent({ name: 'InputSuggestionHint' }).exists()).toBe(false);
  const event = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
  wrapper.get('.agent-input textarea').element.dispatchEvent(event);
  expect(event.defaultPrevented).toBe(false);
  await wrapper.setProps({ agentInput: '我的新问题' });
  await wrapper.get('.send-agent').trigger('click');
  expect(wrapper.emitted('send-agent')).toHaveLength(1);
  expect(load).toHaveBeenCalledTimes(1);
});

it('waits for teaching activities and stays quiet while a response or choice is open', async () => {
  const load = vi.fn(async () => ({ question: '想继续讨论哪一点？', draft: '我想补充……' }));
  const wrapper = discussionGuidance(load, { chat: [{ ...guidanceChat(), hasTeaching: true }] });
  await flushPromises(); expect(load).not.toHaveBeenCalled();
  const activity = wrapper.getComponent({ name: 'TutorActivities' });
  activity.vm.$emit('response-available', true); activity.vm.$emit('guidance-available', false);
  await flushPromises(); expect(load).not.toHaveBeenCalled();
  activity.vm.$emit('response-available', false); activity.vm.$emit('guidance-available', true);
  await flushPromises(); expect(load).toHaveBeenCalledTimes(1);
  wrapper.getComponent({ name: 'TutorContinuation' }).vm.$emit('available', true);
  await flushPromises(); expect(wrapper.find('.reading-input-invitation').exists()).toBe(false);
});

it.each([
  { agentInput: '我已经有一个具体问题' }, { sending: true }, { canStop: true },
  { chat: [{ ...guidanceChat(), pending: true }] },
  { chat: [{ ...guidanceChat(), error: '失败' }] },
  { askDraft: { lid: '1.2', quote: '未定位完整的原文', status: 'partial' } },
])('does not request a new guide during an existing draft, run or unresolved state: %j', async props => {
  const load = vi.fn(async () => null), wrapper = discussionGuidance(load, props);
  await flushPromises(); expect(load).not.toHaveBeenCalled();
  expect(wrapper.find('.reading-input-invitation').exists()).toBe(false);
});

it('discards an old discussion result after a selection change or a newly typed draft', async () => {
  const pending: ((value: { question: string; draft: string }) => void)[] = [];
  const load = vi.fn(() => new Promise(resolve => pending.push(resolve)));
  const wrapper = discussionGuidance(load); await flushPromises();
  await wrapper.setProps({ askDraft: { lid: '1.2', quote: '停留时间也计入总时间。', status: 'resolved' } });
  pending[0]({ question: '旧问题', draft: '旧建议' }); await flushPromises();
  expect(wrapper.find('.reading-input-invitation').exists()).toBe(false);
  await wrapper.setProps({ agentInput: '我刚写出的想法' });
  pending[1]({ question: '晚到的问题', draft: '晚到的建议' }); await flushPromises();
  expect(wrapper.get<HTMLTextAreaElement>('.agent-input textarea').element.value).toBe('我刚写出的想法');
  expect(wrapper.find('.reading-input-invitation').exists()).toBe(false);
});

it('exchanges the welcome illustration for a compact identity without replacing the focused draft', async () => {
  const wrapper = mount(RightRail, { props: baseProps, attachTo: document.body });
  try {
    const input = wrapper.get<HTMLTextAreaElement>('.agent-input textarea');
    expect(wrapper.get('.conversation-welcome h3').text()).toBe('想从哪里开始？');
    expect(wrapper.get('.welcome-mascot img').attributes('src')).toBe(yeyeAssets.idle);
    expect(wrapper.get('.welcome-mascot').attributes('aria-hidden')).toBe('true');
    input.element.focus();
    expect(document.activeElement).toBe(input.element);
    expect(wrapper.emitted('send-agent')).toBeUndefined();
    await wrapper.setProps({ agentInput: '帮我梳理当前这一节的核心概念。' });
    await wrapper.get('.send-agent').trigger('click');
    expect(wrapper.emitted('send-agent')).toHaveLength(1);
    await wrapper.setProps({ sending: true, agentInput: '下一条问题的草稿', chat: [{
      turnId: 'first-question', user: '帮我梳理当前这一节的核心概念。', pending: true, outcome: null,
      questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
    }] });
    expect(wrapper.find('.conversation-welcome').exists()).toBe(false);
    expect(wrapper.get('.u-msg').text()).toBe('帮我梳理当前这一节的核心概念。');
    expect(wrapper.get('.agent-input textarea').element).toBe(input.element);
    expect(document.activeElement).toBe(input.element);
    expect(input.element.value).toBe('下一条问题的草稿');
    for (const selector of ['#reader-tab-agent', '.rail-title', '.mobile-learning-title']) {
      expect(wrapper.get(`${selector} .yeye-mascot img`).attributes('src')).toBe(yeyeAssets.avatar);
    }
    await wrapper.setProps({ sending: false, chat: [], activeChatSessionId: 'new-chat' });
    expect(wrapper.find('.conversation-welcome').exists()).toBe(true);
    expect(input.element.value).toBe('下一条问题的草稿');
  } finally { wrapper.unmount(); }
});

it.each([{ sending: true }, { canStop: true }])('keeps the welcome out of an active run: %j', async state => {
  const wrapper = mount(RightRail, { props: { ...baseProps, ...state }, shallow: true });
  try {
    expect(wrapper.find('.conversation-welcome').exists()).toBe(false);
    await wrapper.setProps({ sending: false, canStop: false });
    expect(wrapper.find('.conversation-welcome').exists()).toBe(true);
    await wrapper.setProps({ presentationWorkspace: { turnId: 'saved-turn', reference: { presentation_id: 'scene', revision: 1 } } });
    expect(wrapper.find('.conversation-welcome').exists()).toBe(false);
  } finally { wrapper.unmount(); }
});

it('passes host and tab visibility to the welcome without rebuilding the draft', async () => {
  const wrapper = mount(RightRail, { props: baseProps, shallow: true });
  try {
    const mascot = () => wrapper.findAllComponents(YeyeMascot).find(item => item.props('motion') === 'idle')!;
    expect(mascot().props('active')).toBe(true);
    await wrapper.setProps({ companionVisible: false }); expect(mascot().props('active')).toBe(false);
    await wrapper.setProps({ companionVisible: true, requestedTab: 'notes', requestedTabRevision: 1 });
    expect(mascot().props('active')).toBe(false);
    await wrapper.setProps({ requestedTab: 'agent', requestedTabRevision: 2 }); expect(mascot().props('active')).toBe(true);
  } finally { wrapper.unmount(); }
});

it('uses a full reading pose only for the current pending wait in a wide conversation', async () => {
  const match = vi.spyOn(window, 'matchMedia').mockImplementation(query => ({
    matches: query.includes('min-width'), addEventListener() {}, removeEventListener() {},
  } as unknown as MediaQueryList));
  const pending = { turnId: 'wait', user: '解释这一步', pending: true, outcome: null,
    questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [], runStatus: '正在等待模型资源' };
  const wrapper = mount(RightRail, { props: { ...baseProps, sending: true, canStop: true, chat: [pending], waitingTurnId: 'wait' }, shallow: true });
  try {
    expect(wrapper.find('.waiting-mascot').exists()).toBe(false);
    await wrapper.setProps({ fullscreen: true }); expect(wrapper.find('.waiting-mascot').exists()).toBe(true);
    expect(wrapper.get('.run-status').text()).toBe('正在等待模型资源');
    await wrapper.setProps({ companionVisible: false });
    expect(wrapper.findAllComponents(YeyeMascot).find(item => item.props('motion') === 'waiting')!.props('active')).toBe(false);
    await wrapper.setProps({ waitingTurnId: null }); expect(wrapper.find('.waiting-mascot').exists()).toBe(false);
    await wrapper.setProps({ waitingTurnId: 'wait', chat: [{ ...pending, draft: { message_id: 0, revision: 1, operation: 'replace', view: { parts: [{ kind: 'markdown', text: '正在出现的回答' }], sources: [] } } }] });
    expect(wrapper.find('.waiting-mascot').exists()).toBe(false);
  } finally { wrapper.unmount(); match.mockRestore(); }
});

it('keeps learning records and practice options beside the composer without a teaching switch', async () => {
  const wrapper = mount(RightRail, { props:baseProps, shallow:true, global: { stubs: { Teleport: false } }, slots:{
    'learning-actions':'<button>我的理解</button><button>学习详情</button>',
    'tutor-control':'<button>练习设置</button>',
  } });
  expect(wrapper.get('#reader-panel-agent .conversation-learning-actions').text()).toContain('我的理解');
  expect(wrapper.findAll('[role=switch]')).toHaveLength(1);
  expect(wrapper.text()).not.toContain('连续教学');
  expect(wrapper.get('.agent-input-tools button').text()).toBe('练习设置');
  expect(wrapper.findComponent({ name: 'LearningPurposeEntry' }).exists()).toBe(false);
  expect(wrapper.emitted('update:agentInput')).toBeUndefined();
  expect(wrapper.emitted('send-agent')).toBeUndefined();
  await wrapper.setProps({agentInput:'已经写下的问题',fullscreen:true});
  expect(wrapper.find('.question-example').exists()).toBe(false);
  expect(wrapper.findComponent({ name: 'LearningPurposeEntry' }).exists()).toBe(false);
  expect(wrapper.findAll('button').filter(control => control.text() === '练习设置')).toHaveLength(1);
  wrapper.unmount();
});

it('keeps new video conversations focused on input without starter buttons', async () => {
  const wrapper = mount(RightRail, { props: { ...baseProps, videoImagesEnabled: true, learningPurposeAvailable: true, learningPurposeReady: true }, shallow: true });
  expect(wrapper.findComponent({ name: 'LearningPurposeEntry' }).exists()).toBe(false);
  expect(wrapper.find('.question-example').exists()).toBe(false);
  expect(wrapper.get('.presentation-request').isVisible()).toBe(true);
  await wrapper.setProps({ learningTitle: '能解释视频中的资源核算' });
  expect(wrapper.find('.question-example').exists()).toBe(false);
  expect(wrapper.get('textarea').isVisible()).toBe(true);
});

it('keeps help bound to the latest delivered answer and focuses the existing draft without sending it', async () => {
  const turn = { turnId: 'current-answer', user: '解释这一步', pending: false,
    questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
    outcome: { answer: '一段较长的讲解', warning: null, effects: [], trace: [], memory_updates: [], turns: 1, tokens_spent: 0, incomplete: false,
      profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] } } };
  const wrapper = mount(RightRail, { attachTo: document.body, shallow: true,
    global: { stubs: { Teleport: false } },
    props: { ...baseProps, agentInput: '尚未发送的想法', chat: [turn] } });
  try {
    await wrapper.get('.learning-help').trigger('click');
    expect(wrapper.emitted('clarify-answer')).toEqual([['current-answer']]);
    await wrapper.get('.learning-respond').trigger('click');
    await nextTick();
    expect(document.activeElement).toBe(wrapper.get('.agent-input textarea').element);
    expect((document.activeElement as HTMLTextAreaElement).value).toBe('尚未发送的想法');
    expect(wrapper.emitted('send-agent')).toBeUndefined();
    expect(wrapper.emitted('update:agentInput')).toBeUndefined();
    await wrapper.setProps({ sending: true });
    expect(wrapper.get('.learning-help').attributes('disabled')).toBeDefined();
    await wrapper.setProps({ sending: false, chat: [turn, { ...turn, turnId: 'new-turn', outcome: null, pending: true }] });
    expect(wrapper.find('.learning-dock').exists()).toBe(false);
    await wrapper.setProps({ chat: [], activeChatSessionId: 'another-chat' });
    expect(wrapper.find('.learning-dock').exists()).toBe(false);
  } finally { wrapper.unmount(); }
});

describe('LE3/LE4 learning actions in the current conversation', () => {
  const readyProps = { learningPurposeAvailable: true, learningPurposeReady: true, closedBookReady: true };
  const mountLearning = (props = {}) => mount(RightRail, {
    props: { ...baseProps, ...readyProps, ...props }, shallow: true, attachTo: document.body,
    global: { stubs: { Teleport: false } },
  });

  it('starts from the current selection without requiring a new goal or changing the draft', async () => {
    const wrapper = mountLearning({ agentInput: '还没发出的想法', learningScopeOptions: [
      { value: 'selection', label: '所选原文', lid: '2.1' }, { value: 'chapter', label: '第二章', lid: '2' },
    ] });
    expect(wrapper.emitted('learning-purpose')).toBeUndefined();
    await wrapper.get('.start-current-learning').trigger('click');
    expect(wrapper.emitted('learning-purpose')).toEqual([['understand', 'selection', {
      display: '带我学这一部分', message: expect.stringContaining('一个值得理解的重点'),
    }]]);
    expect(wrapper.get<HTMLTextAreaElement>('.agent-input textarea').element.value).toBe('还没发出的想法');
    expect(wrapper.emitted('update:agentInput')).toBeUndefined();
    expect(wrapper.emitted('send-agent')).toBeUndefined();
  });

  it('waits for a usable learning scope and preparation before starting', async () => {
    const wrapper = mountLearning();
    expect(wrapper.get('.start-current-learning').attributes('disabled')).toBeDefined();
    await wrapper.setProps({ learningScopeOptions: [{ value: 'section', label: '当前小节', lid: '2.1' }], learningPurposeReady: false });
    expect(wrapper.get('.start-current-learning').attributes('disabled')).toBeDefined();
    await wrapper.get('.start-current-learning').trigger('click');
    expect(wrapper.emitted('learning-purpose')).toBeUndefined();
    await wrapper.setProps({ learningPurposeReady: true });
    await wrapper.get('.start-current-learning').trigger('click');
    expect(wrapper.emitted('learning-purpose')?.[0].slice(0, 2)).toEqual(['understand', 'section']);
  });

  it('offers explanation and application for the exact delivered answer with an explicit closed-book notice', async () => {
    const wrapper = mountLearning({ chat: [guidanceChat()], agentInput: '保留这份草稿' });
    await nextTick();
    expect(wrapper.get('.learning-practice-note').text()).toContain('收起资料');
    expect(wrapper.get('.learning-practice-note').text()).toContain('可随时暂停');
    expect(wrapper.emitted('explain-answer')).toBeUndefined();
    expect(wrapper.emitted('practice-answer')).toBeUndefined();
    await wrapper.get('.learning-explain').trigger('click');
    await wrapper.get('.learning-apply').trigger('click');
    expect(wrapper.emitted('explain-answer')).toEqual([['discussion-1']]);
    expect(wrapper.emitted('practice-answer')).toEqual([['discussion-1']]);
    expect(wrapper.emitted('update:agentInput')).toBeUndefined();
    expect(wrapper.emitted('send-agent')).toBeUndefined();
  });

  it('keeps application available while closed-book state loads and disables both actions during a run', async () => {
    const wrapper = mountLearning({ chat: [guidanceChat()], closedBookReady: false });
    await nextTick();
    expect(wrapper.get('.learning-explain').attributes('disabled')).toBeDefined();
    expect(wrapper.get('.learning-apply').attributes('disabled')).toBeUndefined();
    await wrapper.get('.learning-explain').trigger('click');
    expect(wrapper.emitted('explain-answer')).toBeUndefined();
    await wrapper.setProps({ closedBookReady: true, canStop: true });
    expect(wrapper.get('.learning-explain').attributes('disabled')).toBeDefined();
    expect(wrapper.get('.learning-apply').attributes('disabled')).toBeDefined();
    await wrapper.get('.learning-apply').trigger('click');
    expect(wrapper.emitted('practice-answer')).toBeUndefined();
  });

  it('keeps the existing exercise as the only task until its activity and continuation have ended', async () => {
    const wrapper = mountLearning({ chat: [{ ...guidanceChat(), hasTeaching: true }] });
    await nextTick();
    expect(wrapper.find('.answer-practice-actions').exists()).toBe(false);
    const activity = wrapper.getComponent({ name: 'TutorActivities' });
    activity.vm.$emit('response-available', true);
    activity.vm.$emit('guidance-available', false);
    await nextTick();
    expect(wrapper.get('.learning-respond').text()).toContain('回答问题');
    expect(wrapper.find('.answer-practice-actions').exists()).toBe(false);
    activity.vm.$emit('response-available', false);
    activity.vm.$emit('guidance-available', true);
    await nextTick();
    expect(wrapper.find('.answer-practice-actions').exists()).toBe(true);
    wrapper.getComponent({ name: 'TutorContinuation' }).vm.$emit('available', true);
    await nextTick();
    expect(wrapper.find('.answer-practice-actions').exists()).toBe(false);
    expect(wrapper.get('.learning-respond').text()).toContain('查看下一步');
  });

  it('keeps a delivered Socratic question ready for its own answer', async () => {
    const turn = guidanceChat();
    const wrapper = mountLearning({ chat: [{ ...turn, outcome: { ...turn.outcome,
      answer_view: { parts: [{ kind: 'socratic_question', text: '为什么停留时间也算在内？' }], sources: [] },
    } }] });
    await nextTick();
    expect(wrapper.find('.answer-practice-actions').exists()).toBe(false);
    expect(wrapper.get('.learning-respond').text()).toContain('回答问题');
  });

  it.each(['pending', 'error', 'incomplete', 'history'] as const)('does not start a new practice from an unresolved %s answer', async state => {
    const turn = guidanceChat();
    const wrapper = mountLearning({ chat: [{ ...turn, pending: state === 'pending', error: state === 'error' ? '失败' : undefined,
      outcome: { ...turn.outcome, incomplete: state === 'incomplete' },
    }], historyLoading: state === 'history' });
    await nextTick();
    expect(wrapper.find('.answer-practice-actions').exists()).toBe(false);
  });

  it('leaves course learning actions in the course and retains local help in the conversation', async () => {
    const wrapper = mountLearning({ chat: [guidanceChat()], courseActive: true });
    await nextTick();
    expect(wrapper.find('.answer-practice-actions').exists()).toBe(false);
    expect(wrapper.find('.learning-respond').exists()).toBe(false);
    await wrapper.get('.learning-help').trigger('click');
    expect(wrapper.emitted('clarify-answer')).toEqual([['discussion-1']]);
  });
});

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
  it.each(['TEACHING_INVALID', 'MODEL_PROVIDER_ERROR'])('keeps the fallback answer visible without exposing %s', warning => {
    const wrapper = mount(RightRail, { props: { ...baseProps, chat: [{ turnId: 'fallback', user: 'Embedding 用在哪里？给我一个演示页。', pending: false,
      questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
      outcome: { answer: 'Embedding 把文本表示为向量，检索时通过向量比较找到相关片段。演示页尚未完成。', warning,
        effects: [], trace: [], memory_updates: [], turns: 1, tokens_spent: 0, incomplete: true,
        profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] } },
    }] }, shallow: true, global: { renderStubDefaultSlot: true } });
    try {
      expect(wrapper.get('.a-msg .ans-text').text()).toContain('Embedding 把文本表示为向量');
      expect(wrapper.get('.a-msg .ans-text').text()).toContain('演示页尚未完成');
      expect(wrapper.get('.incomplete').text()).toContain('已提供当前可用内容');
      expect(wrapper.text()).not.toContain(warning);
      expect(wrapper.text()).not.toContain('请重试');
    } finally { wrapper.unmount(); }
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
        expect(wrapper.get(".transcript .empty").text()).toContain("想从哪里开始？");
        expect(wrapper.find(".empty-history-button").exists()).toBe(false);
      } else {
        expect(wrapper.get(".transcript .empty").text()).toContain("1 段历史对话");
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
  it('shows oldest turns first while effect actions retain their original turn indexes', async () => {
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
      expect(wrapper.find('.trace-toggle').exists()).toBe(false);
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
    expect(wrapper.find('.conversation-welcome').exists()).toBe(false);
    expect((wrapper.get(".new-chat").element as HTMLButtonElement).disabled).toBe(true);
    expect((wrapper.get(".agent-input textarea").element as HTMLTextAreaElement).disabled).toBe(false);
    expect((wrapper.get(".agent-input .send-agent").element as HTMLButtonElement).disabled).toBe(true);
    await wrapper.setProps({ historyLoading: false, historyError: "暂时无法连接" });
    expect(wrapper.find('.conversation-welcome').exists()).toBe(false);
    expect(wrapper.get('[role="alert"]').text()).toContain('暂时无法连接');
    await wrapper.get('[role="alert"] button').trigger("click");
    expect(wrapper.emitted("retry-history")).toHaveLength(1);
    await wrapper.setProps({ historyError: null });
    expect(wrapper.find('.conversation-welcome').exists()).toBe(true);
    expect((wrapper.get(".agent-input button:last-child").element as HTMLButtonElement).disabled).toBe(false);
    wrapper.unmount();
  });
});

beforeEach(() => {
  vi.spyOn(api, 'agentSourceRead').mockResolvedValue({
    source_ref_id: '', source_lid: '1.1', focus_lid: '1.1', segments: [], previous_lid: null, next_lid: null,
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
    vi.spyOn(api, 'sessionRecap').mockResolvedValue({ ...recap, effects: [{ ...note, object_id: 'edited-object' }, presentation] });
    const resolve = vi.spyOn(api, 'agentSourceResolve').mockResolvedValue({ source_ref_id: 'source-1', label: '原文', highlighted_quote: '引文', context_before: '', context_after: '', stale: false, can_open_in_reader: true });
    const read = vi.spyOn(api, 'presentationRead').mockResolvedValue({} as never);
    vi.spyOn(api, 'recall').mockResolvedValue([{ mem_id: 'edited-object', anchor: { lid: '1.2' }, content: '保留后的笔记' }] as never);
    const scroll = vi.fn(); vi.stubGlobal('HTMLElement', HTMLElement);
    const prior = HTMLElement.prototype.scrollIntoView;
    HTMLElement.prototype.scrollIntoView = scroll;
    const wrapper = mount(RightRail, { props: { ...baseProps, chat, contextNotes: [{ mem_id: 'edited-object', type: 'note', book_id: 'book', layer: 'long_term', anchor: {}, content: '保留后的笔记' }] }, global: { stubs: { AgentPresentation: true } } });
    try {
      await wrapper.get('.recap-button').trigger('click'); await flushPromises();
      expect(document.body.querySelector('.session-recap')).not.toBeNull();
      expect(resolve).not.toHaveBeenCalled(); expect(read).not.toHaveBeenCalled(); expect(scroll).not.toHaveBeenCalled();
      const rail = wrapper.vm as unknown as { openRecapTarget: (t: RecapTarget) => Promise<void> };
      await rail.openRecapTarget({ ...base, kind: 'turn' }); expect(scroll).toHaveBeenCalled();
      await rail.openRecapTarget({ ...base, kind: 'source', source });
      expect(resolve).toHaveBeenCalledWith('turn-1', 'source-1', undefined);
      await wrapper.setProps({ chat: [{ ...chat[0], pending: true, runStatus: '新活动' }] });
      expect(document.body.querySelector('.agent-source-popup')).not.toBeNull();
      await rail.openRecapTarget({ ...base, kind: 'effect', effect: note });
      expect(wrapper.get('[data-mem-id="edited-object"]').attributes('open')).toBeDefined();
      expect(wrapper.emitted('focus-source')).toBeUndefined();
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
  it('offers a reference-book link in a new tab without navigating the current reader', async () => {
    vi.spyOn(api, 'sessionRecap').mockResolvedValue(recap);
    const sourceUrl = '/?book_id=rust&publication_id=edition-1&start_lid=1.1&end_lid=1.1&start_utf16=0&end_utf16=20';
    vi.spyOn(api, 'agentSourceResolve').mockResolvedValue({ source_ref_id: 'source-1', label: 'Rust · 借用',
      material_title: 'Rust', highlighted_quote: '共享借用', context_before: '', context_after: '', stale: false,
      can_open_in_reader: true, source_url: sourceUrl });
    const open = vi.spyOn(api, 'agentSourceOpen');
    const wrapper = mount(RightRail, { props: { ...baseProps, chat }, global: { stubs: { AgentPresentation: true } } });
    try {
      await (wrapper.vm as unknown as { openRecapTarget: (t: RecapTarget) => Promise<void> }).openRecapTarget({ ...base, kind: 'source', source });
      await flushPromises();
      const link = document.body.querySelector<HTMLAnchorElement>('a.source-open-reader');
      expect(link?.getAttribute('href')).toBe(sourceUrl);
      expect(link?.target).toBe('_blank');
      expect(link?.textContent).toContain('查看参考书原文');
      expect(open).not.toHaveBeenCalled();
      expect(wrapper.emitted('agent-source-will-open')).toBeUndefined();
      expect(wrapper.emitted('agent-source-opened')).toBeUndefined();
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

describe('WB1 board entry from conversation', () => {
  it('imports the complete delivered answer by its saved owner and excludes drafts or incomplete answers', async () => {
    const complete = { turnId: 'answer-one', user: '解释这个概念', pending: false, outcome: sourceOutcome(),
      questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [] };
    const wrapper = mount(RightRail, { shallow: true, props: { ...baseProps, chat: [complete,
      { ...complete, turnId: 'draft', pending: true },
      { ...complete, turnId: 'partial', outcome: { ...sourceOutcome(), incomplete: true } },
      { ...complete, turnId: 'stopped', runStatus: '已停止' },
    ] } });
    expect(wrapper.findAll('.answer-add-board')).toHaveLength(1);
    await wrapper.get('.answer-add-board').trigger('click');
    expect(wrapper.emitted('add-to-board')).toEqual([[{ kind: 'answer', session_id: 'chat-1', turn_id: 'answer-one' }]]);
  });

  it('forwards the exact saved scene receipt with its original presentation owner', () => {
    const reference = { presentation_id: 'demo-one', revision: 2 };
    const receipt = { session_id: 'chat-1', turn_id: 'answer-one', reference, state_revision: 4, saved_state_ref: 'saved-four' };
    const outcome = sourceOutcome();
    const wrapper = mount(RightRail, { shallow: true, global: { renderStubDefaultSlot: true }, props: { ...baseProps, chat: [{
      turnId: receipt.turn_id, user: '解释这个概念', pending: false,
      outcome: { ...outcome, answer_view: { ...outcome.answer_view, parts: [{ kind: 'presentation', ...reference }] } },
      questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
    }] } });
    wrapper.getComponent({ name: 'AgentPresentation' }).vm.$emit('add-to-board', receipt);
    expect(wrapper.emitted('add-to-board')).toEqual([[{ kind: 'presentation', session_id: 'chat-1', turn_id: 'answer-one', reference, receipt }]]);
  });
});

describe('WB2 saved board locations in conversation history', () => {
  const followUp = { board_id: 'original-board', board_revision: 4, item_id: 'question-item', block_id: 'question-block' };
  const delivery = { board_id: 'saved-board', before_revision: 4, after_revision: 5,
    item_ids: ['saved-answer-item'], block_ids: ['saved-answer-block'],
    origin_session_id: 'historical-session', origin_turn_id: 'historical-answer', parent_item_id: 'question-item' };
  const turn = () => ({ turnId: 'historical-answer', user: '为什么这个条件必不可少？', pending: false,
    outcome: sourceOutcome(), questionAnchorLid: null, questionQuote: null, questionSelection: null,
    effectLabels: [], boardFollowUp: followUp, boardDelivery: delivery });

  it('opens the delivered answer and original question at their exact saved board locations', async () => {
    const wrapper = mount(RightRail, { shallow: true,
      props: { ...baseProps, activeChatSessionId: 'current-session', chat: [turn()] } });

    expect(wrapper.get('.board-delivery-link').text()).toBe('在画板中查看');
    await wrapper.get('.board-delivery-link').trigger('click');
    await wrapper.get('.board-question-link').trigger('click');
    expect(wrapper.emitted('open-board-location')).toEqual([
      [{ board_id: 'saved-board', item_id: 'saved-answer-item', block_id: 'saved-answer-block' }],
      [{ board_id: 'original-board', item_id: 'question-item', block_id: 'question-block' }],
    ]);
    expect(wrapper.emitted('add-to-board')).toBeUndefined();
  });

  it('retains the answer and original target when delivery fails, without claiming it was saved', async () => {
    const wrapper = mount(RightRail, { shallow: true, global: { renderStubDefaultSlot: true },
      props: { ...baseProps, chat: [{ ...turn(), boardDeliveryError: '画板暂时无法保存' }] } });

    expect(wrapper.get('.a-msg').text()).toContain('First claim.');
    expect(wrapper.get('.a-msg').text()).toContain('Next claim.');
    expect(wrapper.get('.board-delivery-error').text()).toBe('内容没有加入画板：画板暂时无法保存');
    expect(wrapper.find('.board-delivery-link').exists()).toBe(false);
    await wrapper.get('.board-question-link').trigger('click');
    expect(wrapper.emitted('open-board-location')).toEqual([
      [{ board_id: 'original-board', item_id: 'question-item', block_id: 'question-block' }],
    ]);
  });
});

describe('answer math formatting', () => {
  it.each(['completed', 'draft'])('renders screenshot variables and equations in a %s answer', (state) => {
    const view = { parts: [{ kind: 'markdown' as const, text: mathAnswer }], sources: [] };
    const turn = {
      turnId: 'math-answer', user: '解释状态更新与读出', pending: state === 'draft',
      outcome: state === 'completed' ? { ...sourceOutcome(), answer: mathAnswer, answer_view: view } : null,
      draft: state === 'draft' ? { message_id: 1, revision: 1, operation: 'replace', view } : null,
      questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
    };
    const wrapper = mount(RightRail, { props: { ...baseProps, renderMarkdown, chat: [turn] } });
    const answer = wrapper.get(state === 'draft' ? '.answer-draft' : '.a-msg .ans-text');
    expect(answer.findAll('.katex-display')).toHaveLength(2);
    expect(answer.findAll('p > .katex')).toHaveLength(10);
    expect(answer.findAll('.katex-error, pre, code')).toHaveLength(0);
    expect(answer.findAll('.katex-display annotation').map(node => node.text())).toEqual([
      'S_t = S_{t-1} + k_t v_t^{T}', 'o_t = q_t^{T} S_t',
    ]);
    expect(answer.findAll('p annotation').map(node => node.text())).toContain('d_k \\cdot d_v');
    expect(answer.find('math msub').exists()).toBe(true);
    expect(answer.find('math msubsup').exists()).toBe(true);
  });
});

describe('answer rendering on local UI changes', () => {
  it('reuses unchanged answers across menus, history, input and hidden tabs', async () => {
    const render = vi.fn(renderMarkdown);
    const chat = reactive(Array.from({ length: 10 }, (_, index) => ({
      turnId: `cached-${index}`, user: '问题', pending: false, outcome: sourceOutcome(),
      questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
    })));
    const wrapper = mount(RightRail, { props: { ...baseProps, chat, renderMarkdown: render } });
    try {
      const initial = render.mock.calls.length;
      expect(initial).toBe(10);
      await wrapper.get('.context-menu-toggle').trigger('click');
      await wrapper.get('.context-menu-toggle').trigger('click');
      await wrapper.get('.history-button').trigger('click');
      (document.querySelector('.history-close') as HTMLButtonElement).click();
      await nextTick();
      await wrapper.setProps({ agentInput: '新问题' });
      await wrapper.get('#reader-tab-notes').trigger('click');
      await wrapper.setProps({ agentInput: '继续输入' });
      await wrapper.get('#reader-tab-agent').trigger('click');
      expect(render).toHaveBeenCalledTimes(initial);

      chat[0].outcome.answer_view.parts[0] = { kind: 'markdown', text: '已经修订的回答。' };
      await nextTick();
      expect(render).toHaveBeenCalledTimes(initial + 1);
      expect(wrapper.get('.answer-markdown').text()).toContain('已经修订');
      chat[0].outcome.answer_view.sources[0].label = '更新后的来源';
      await nextTick();
      expect(render).toHaveBeenCalledTimes(initial + 2);
      expect(wrapper.get('.agent-source-button').text()).toContain('更新后的来源');
    } finally { wrapper.unmount(); }
  });

  it('updates only the growing draft and invalidates when the renderer changes', async () => {
    const render = vi.fn(renderMarkdown);
    const saved = { turnId: 'saved', user: '旧问题', pending: false, outcome: sourceOutcome(),
      questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [] };
    const draft = reactive({ ...saved, turnId: 'draft', pending: true, outcome: null,
      draft: { message_id: 1, revision: 1, operation: 'replace', view: { parts: [{ kind: 'markdown', text: '第一段草稿' }], sources: [] } } });
    const wrapper = mount(RightRail, { props: { ...baseProps, chat: [saved, draft] as never, renderMarkdown: render } });
    try {
      render.mockClear();
      draft.draft.revision++;
      draft.draft.view.parts[0].text = '第一段草稿，继续解释';
      await nextTick();
      expect(render.mock.calls.map(([text]) => text)).toEqual(['第一段草稿，继续解释']);
      expect(wrapper.get('.answer-draft').text()).toContain('继续解释');
      const replacement = vi.fn(renderMarkdown);
      await wrapper.setProps({ renderMarkdown: replacement });
      expect(replacement).toHaveBeenCalledTimes(2);
    } finally { wrapper.unmount(); }
  });
});

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

  it("explains long content only for an actual context-budget warning", () => {
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
    expect(notices[0].text()).toBe("这次内容较长，先提供当前可用的回答；其余部分可以继续补充。");
  });

  it("explains incomplete delivery without exposing internal stop reasons", () => {
    const warnings = [
      ["COMPACTION_FAILED", "已提供当前可用的回答；剩余内容暂未完成，可以继续补充。"],
      ["ACTIVE_CONTEXT_EXHAUSTED", "这次内容较长，先提供当前可用的回答；其余部分可以继续补充。"],
      ["TURN_LIMIT_EXCEEDED", "已提供当前可用内容；未完成的讲解、演示或其他操作可以接着完善。"],
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
      warnings.map(([, label]) => label),
    );
    for (const [warning] of warnings) expect(wrapper.text()).not.toContain(warning);
    expect(wrapper.text()).not.toContain("请重试");
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

  it("reads context on first click and opens the main reader only from the secondary action", async () => {
    vi.mocked(api.agentSourceRead).mockRestore();
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
      if (url.endsWith("/agent/source.read")) {
        return Response.json({
          source_ref_id: body.source.source_ref_id, source_lid: '1.2', focus_lid: '1.2', previous_lid: null, next_lid: null,
          segments: [
            { lid: '1.1', kind: 'paragraph', text: 'substantial context before', highlights: [] },
            { lid: '1.2', kind: 'paragraph', text: 'exact evidence', highlights: [{ start: 0, end: 14 }] },
          ],
        });
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

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(String(fetchMock.mock.calls[0][0])).toContain("/agent/source.resolve");
    expect(JSON.parse(String(fetchMock.mock.calls[0][1]?.body))).toEqual({
      turn_id: "turn-a",
      source_ref_id: "source_ref_a",
    });
    expect(String(fetchMock.mock.calls[1][0])).toContain("/agent/source.read");
    expect(document.body.querySelector(".agent-source-popup mark")?.textContent).toBe("exact evidence");
    expect(document.body.querySelector(".source-reader-page")?.textContent).toContain("substantial context before");
    expect(wrapper.emitted("agent-source-opened")).toBeUndefined();

    const open = document.body.querySelector<HTMLButtonElement>(".source-open-reader")!;
    open.click();
    await flushPromises();
    expect(fetchMock).toHaveBeenCalledTimes(3);
    expect(String(fetchMock.mock.calls[2][0])).toContain("/agent/source.open");
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

    expect(document.body.querySelector(".board-source-snapshot")?.textContent).toContain("引用时的快照");
    expect(document.body.querySelector<HTMLButtonElement>(".source-open-reader")?.disabled).toBe(true);
  });

  it("keeps the newest source click and ignores a superseded late resolve", async () => {
    const first = deferred<Response>();
    const second = deferred<Response>();
    const responses = [first, second];
    vi.stubGlobal("fetch", vi.fn((input: RequestInfo | URL) => String(input).endsWith('/agent/source.resolve')
      ? responses.shift()!.promise : Promise.resolve(Response.json({ items: [] }))));
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
    document.body.querySelector<HTMLButtonElement>('.board-source-heading button[aria-label="关闭来源"]')!.click();
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

    const app = appSource;
    expect(app).toContain("await api.agentRunCreate(msg, {");
    expect(app).not.toContain("const outbound = draft");
    expect(app).toContain("question_quote: draft ? { ...draft } : null");
  });

  it("exposes every auxiliary surface as a labelled keyboard-operable tab", async () => {
    const wrapper = mount(RightRail, { attachTo: document.body, props: baseProps });
    const tabs = wrapper.findAll('.context-tabs [role="tab"]');
    expect(tabs.map((tab) => tab.text().trim())).toEqual(["问答", "画像", "公式", "笔记"]);
    expect(tabs[0].attributes("aria-selected")).toBe("true");
    expect(wrapper.get('.secondary-tabs').isVisible()).toBe(false);
    await wrapper.get('[aria-label="阅读工具"]').trigger('click');
    expect(wrapper.get('.secondary-tabs').isVisible()).toBe(true);
    await tabs[2].trigger("click");
    expect(tabs[2].attributes("aria-selected")).toBe("true");
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
    expect(wrapper.get(".profile-usage summary").text()).toContain("学习偏好与背景 · 1 条");
    expect(wrapper.get('.profile-usage').text()).not.toContain('快照');
    expect(wrapper.get('.profile-usage').text()).not.toContain('fact-1');
    await wrapper.get('.memory-update-row button[aria-label="撤销画像更新"]').trigger("click");
    expect(wrapper.emitted("undo-profile-update")?.[0]).toEqual([0, 0, memoryUpdate]);

    const profileTab = wrapper.findAll(".tab").find((tab) => tab.text().includes("画像"));
    expect(profileTab?.text()).toContain("1");
    await profileTab!.trigger("click");
    expect(wrapper.emitted("refresh-profile")).toHaveLength(1);
    expect(wrapper.get(".profile-memory-panel").isVisible()).toBe(true);
  });

  it("keeps saved and live tool diagnostics out of the reader while retaining the answer", async () => {
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
        chat: [{
          turnId: 'saved-trace', user: '问题', pending: false,
          questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
          outcome: { answer: '给读者的解释', effects: [], memory_updates: [], turns: 2, tokens_spent: 0, incomplete: false, warning: null,
            profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] },
            trace: [{
              tool: "book.query",
              args: "typed request",
              result_digest: "complete response only",
              model_tool_loop: 2,
              query_audit: audit,
            }],
          },
        }, {
          turnId: 'live-trace', user: '继续解释', pending: true, outcome: null,
          questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
          runTrace: [{ tool: 'book.text', args: 'live request', result_digest: 'live diagnostic' }],
        }],
      },
    });
    try {
      expect(wrapper.get('.answer-markdown').text()).toContain('给读者的解释');
      await wrapper.get('[aria-label="阅读工具"]').trigger('click');
      expect(wrapper.find('#reader-tab-trace').exists()).toBe(false);
      expect(wrapper.find('#reader-panel-trace').exists()).toBe(false);
      expect(wrapper.find('.trace-toggle').exists()).toBe(false);
      expect(wrapper.find('.query-audit').exists()).toBe(false);
      for (const detail of ['模型—工具循环', '工具调用', 'book.query', 'typed request', 'complete response only', 'entity:command', 'live request', 'live diagnostic']) {
        expect(wrapper.text()).not.toContain(detail);
      }
    } finally { wrapper.unmount(); }
  });

  it("opens the profile directly beside chat and keeps tools limited to formula and notes", async () => {
    const wrapper = mount(RightRail, { attachTo: document.body, props: baseProps });
    expect(wrapper.findAll('.context-tabs > .tab').map(tab => tab.text().trim())).toEqual(['问答', '画像']);
    expect(wrapper.find('#reader-tab-artifacts').exists()).toBe(false);
    expect(wrapper.find('#reader-panel-artifacts').exists()).toBe(false);
    await wrapper.get('#reader-tab-profile').trigger('click');
    expect(wrapper.emitted('refresh-profile')).toHaveLength(1);
    expect(wrapper.get('#reader-panel-profile').isVisible()).toBe(true);
    expect(wrapper.get('[aria-label="阅读工具"]').classes()).not.toContain('active');
    expect(wrapper.get('[aria-label="阅读工具"]').text()).toBe('工具');
    await wrapper.get('[aria-label="阅读工具"]').trigger('click');
    expect(wrapper.findAll('.secondary-tabs .tab').map(tab => tab.text().trim())).toEqual(['公式', '笔记']);
    await wrapper.get('#reader-tab-agent').trigger('click');
    expect(wrapper.get('#reader-panel-agent').isVisible()).toBe(true);
  });
});

describe("Resident activities", () => {
  it("retains live progress, emits stop and preserves upward scrolling without diagnostics", async () => {
    const activity = { step_id: 1, parent_step_id: null, kind: "tool", name: "book.read", label: "读取原文", status: "running" as const, started_ms: 0, duration_ms: null, result_count: null, error_code: null, usage_total_tokens: null, usage: null, model_first_text_ms: null, model_name: null, model_name_source: null, accepted_evidence_count: null, evidence_refs: [] };
    const turn = { turnId: "turn", user: "问题", outcome: null, pending: true, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [], activities: [activity], runStatus: "正在运行" };
    const wrapper = mount(RightRail, { props: { ...baseProps, chat: [turn], sending: true, canStop: true } });
    expect(wrapper.find(".transcript .agent-activity").attributes("data-status")).toBe("running");
    await wrapper.find(".stop-agent").trigger("click");
    expect(wrapper.emitted("stop-agent")).toHaveLength(1);
    expect(wrapper.find('#reader-panel-trace').exists()).toBe(false);
    expect(wrapper.findAll('.agent-activity[data-step-id="1"]')).toHaveLength(1);
    expect(wrapper.find('.activity-group.diagnostic').exists()).toBe(false);
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
    expect(wrapper.get('.activity-summary').text()).toBe('查看工作过程 · 1 项操作');
    await wrapper.get('.activity-summary').trigger('click');
    expect(wrapper.findAll('.agent-activity[data-status="cancelled"]')).toHaveLength(1);
    wrapper.unmount();
  });
  it("keeps stop available outside the composer, disables it while stopping and preserves the input", async () => {
    const turn = { turnId: 'running', user: '解释架构', outcome: null, pending: true, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [], runStatus: '正在运行' };
    const wrapper = mount(RightRail, { props: { ...baseProps, chat: [turn], canStop: true, sending: true, agentInput: '还想补充的要求' } });
    const stop = wrapper.get('.run-progress-footer .stop-work-button');
    expect(stop.text()).toBe('停止');
    await stop.trigger('click');
    expect(wrapper.emitted('stop-agent')).toHaveLength(1);
    await wrapper.setProps({ stopping: true, canStop: false });
    expect(stop.text()).toBe('正在停止');
    expect(stop.attributes('disabled')).toBeDefined();
    expect(wrapper.get('.stop-agent').attributes('disabled')).toBeDefined();
    await stop.trigger('click');
    await wrapper.get('.stop-agent').trigger('click');
    expect(wrapper.emitted('stop-agent')).toHaveLength(1);
    expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('还想补充的要求');
    await wrapper.setProps({ stopping: false, canStop: true });
    await stop.trigger('click');
    expect(wrapper.emitted('stop-agent')).toHaveLength(2);
    wrapper.unmount();
  });
  it("keeps stop reachable when the mobile composer or presentation discussion is collapsed", async () => {
    const match = vi.spyOn(window, 'matchMedia').mockImplementation(query => ({
      matches: query.includes('max-width'), addEventListener() {}, removeEventListener() {},
    } as unknown as MediaQueryList));
    const turn = { turnId: 'running', user: '解释架构', outcome: null, pending: true, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [], runStatus: '正在运行' };
    const previous = { ...turn, turnId: 'saved', pending: false, hasTeaching: true, outcome: {
      answer: '已经完成的解释', effects: [], trace: [], memory_updates: [], incomplete: false,
      profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] },
    } };
    const wrapper = mount(RightRail, { shallow: true, attachTo: document.body, props: { ...baseProps, chat: [previous as never, turn], canStop: true, sending: true, agentInput: '保留草稿' } });
    try {
      await flushPromises();
      expect(wrapper.classes()).toContain('conversation-mobile');
      wrapper.getComponent({ name: 'TutorActivities' }).vm.$emit('editing-response', true);
      await nextTick();
      expect(wrapper.get('.agent-input').classes()).toContain('mobile-answering');
      expect(wrapper.get('.agent-compose-row').isVisible()).toBe(false);
      expect(wrapper.get('.run-progress-footer .stop-work-button').isVisible()).toBe(true);
      await wrapper.get('.run-progress-footer .stop-work-button').trigger('click');
      expect(wrapper.emitted('stop-agent')).toHaveLength(1);
      await wrapper.setProps({ presentationWorkspace: { turnId: 'saved', reference: { presentation_id: 'scene', revision: 1 } } });
      expect(wrapper.get('.discussion-toggle').attributes('aria-expanded')).toBe('false');
      expect(wrapper.get('.workspace-tools .stop-work-button').isVisible()).toBe(true);
      await wrapper.get('.workspace-tools .stop-work-button').trigger('click');
      expect(wrapper.emitted('stop-agent')).toHaveLength(2);
      expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('保留草稿');
    } finally { wrapper.unmount(); match.mockRestore(); }
  });
  it("follows same-revision streamed draft appends but respects upward scrolling", async () => {
    const turn = reactive({ turnId: 'streaming', user: '解释架构', outcome: null, pending: true, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
      draft: { message_id: 1, revision: 1, operation: 'replace', view: { parts: [{ kind: 'markdown' as const, text: '第一段' }], sources: [] } },
      activities: [] as import('@understand-book/web/reader/agent-run-state').RunActivity[] });
    const wrapper = mount(RightRail, { props: { ...baseProps, chat: [turn] } });
    const transcript = wrapper.get('.transcript');
    const el = transcript.element as HTMLElement;
    Object.defineProperty(el, 'scrollHeight', { value: 1000, configurable: true });
    Object.defineProperty(el, 'clientHeight', { value: 200, configurable: true });
    el.scrollTop = 790;
    await transcript.trigger('scroll');
    turn.draft.view.parts[0]!.text += '，继续解释';
    await flushPromises();
    expect(el.scrollTop).toBe(1000);
    el.scrollTop = 200;
    await transcript.trigger('scroll');
    turn.draft.view.parts[0]!.text += '，最后一段';
    await flushPromises();
    expect(el.scrollTop).toBe(200);
    expect(wrapper.get('.answer-draft').text()).toContain('最后一段');
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
      expect(wrapper.get(".agent-compose-row > button").attributes('aria-label')).toBe("停止");
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


it('opens media beside the answer and enters the main video only after the explicit action', async () => {
  const media = { publication: { book_id: 'book', publication_id: 'old' }, duration_ms: 600000,
    target: { kind: 'visual' as const, publication: { book_id: 'book', publication_id: 'old' }, frames: [{ video: { media_id: 'v', media_revision: '1' }, actual_pts_ms: 492000, width: 1920, height: 1080 }], regions: [] }, transcript: [], translations: [] };
  vi.spyOn(api, 'agentSourceResolve').mockResolvedValue({ source_ref_id: 'source_ref_a', label: '视频画面', highlighted_quote: '', context_before: '', context_after: '', stale: false, can_open_in_reader: true, media, media_url: '/original/media' });
  const open = vi.spyOn(api, 'agentSourceOpen');
  const wrapper = mount(RightRail, { props: { ...baseProps, chat: [{ turnId: 'turn-a', user: 'question', outcome: sourceOutcome(), pending: false, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [] }] } });
  await wrapper.findAll('.agent-source-button')[0].trigger('click'); await flushPromises();
  expect(document.body.querySelector('.media-source-preview video')).not.toBeNull();
  expect(wrapper.emitted('media-source-enter')).toBeUndefined(); expect(open).not.toHaveBeenCalled();
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' })); await flushPromises();
  expect(document.body.querySelector('.media-source-preview')).toBeNull();
  await wrapper.findAll('.agent-source-button')[0].trigger('click'); await flushPromises();
  (document.body.querySelector('.source-open-reader') as HTMLButtonElement).click(); await flushPromises();
  expect(wrapper.emitted('media-source-enter')).toEqual([[media]]); expect(open).not.toHaveBeenCalled(); wrapper.unmount();
});


describe('RN4 note retrieval', () => {
  const note = (id: string, content: string, more = {}) => ({ mem_id: id, content, type: 'note', layer: 'long_term', book_id: 'book', anchor: {}, ...more });
  it('searches excerpts and titles, mixes highlights by real time, clears filters for a targeted record and follows replacements', async () => {
    const notes = [note('legacy', '> 混排旧正文'), note('answer', '自己的想法', { generated_at: '1700000000', note: {
      association: { kind: 'answer', session_id: 'original-chat', turn_id: 'answer-turn' }, retained_excerpt: { kind: 'assistant', text: '助手独有片段' }, source_bindings: [],
    } })];
    const highlight = note('highlight', '原文高亮', { type: 'highlight', generated_at: '2025-01-01T00:00:00Z' });
    const w = mount(RightRail, { props: { ...baseProps, contextNotes: notes as never, contextHighlights: [highlight], noteBookOrder: ['legacy', 'highlight', 'answer'],
      chatSessions: [{ id: 'original-chat', title: '梯度下降的讨论', turn_count: 1, turns: [], created_at: '', updated_at: '' }] } });
    const ids = () => w.findAll('.memory-card').map(c => c.attributes('data-mem-id'));
    const rail = w.vm as unknown as { showMemory: (id: string) => Promise<boolean>; replaceMemory: (id: string, record: any) => void };
    const scroll = vi.spyOn(HTMLElement.prototype, 'scrollIntoView').mockImplementation(() => {});
    try {
      expect(ids()).toEqual(['highlight', 'answer', 'legacy']);
      expect(w.get('[data-mem-id="legacy"]').text()).toContain('保存时间未知');
      await w.get('[aria-label="搜索笔记"]').setValue('助手独有'); expect(ids()).toEqual(['answer']);
      await w.get('[aria-label="搜索笔记"]').setValue('梯度下降'); expect(ids()).toEqual(['answer']);
      await w.get('[aria-label="笔记类型"]').setValue('highlight'); expect(ids()).toEqual([]);
      expect(await rail.showMemory('answer')).toBe(true); await flushPromises();
      await w.get('[data-mem-id="answer"]').findAll('button').find(b => b.text() === '返回原回答')!.trigger('click');
      expect(w.emitted('open-note-answer')?.[0]?.[0]).toMatchObject({ mem_id: 'answer' });
      const edited = { ...notes[1], mem_id: 'edited', content: '修订后的想法' };
      rail.replaceMemory('answer', edited); await w.setProps({ contextNotes: [notes[0], edited] as never });
      expect(w.get('[data-mem-id="edited"]').attributes('open')).toBeDefined();
      expect(w.get('[data-mem-id="edited"] .note-detail').text()).toContain('修订后的想法');
      await w.get('[aria-label="笔记排序"]').setValue('original');
      await w.setProps({ noteBookOrder: ['legacy', 'highlight', 'edited'] }); expect(ids()).toEqual(['legacy', 'highlight', 'edited']);
      await w.get('[aria-label="搜索笔记"]').setValue('私人查询');
      await w.setProps({ noteScopeKey: 'another-publication', contextNotes: [], contextHighlights: [] });
      await w.setProps({ contextNotes: [notes[0]] as never });
      expect((w.get('[aria-label="搜索笔记"]').element as HTMLInputElement).value).toBe('');
    } finally { w.unmount(); scroll.mockRestore(); }
  });
});

it.each(['completed', 'pending', 'incomplete', 'failed', 'cancelled'])('RS6 exposes an answer menu only for a delivered complete answer: %s', async status => {
  const turn = { turnId: 'rs6-turn', user: '原问题', pending: status === 'pending', error: status === 'failed' ? 'failed' : undefined,
    runStatus: status === 'cancelled' ? '已停止' : undefined, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [],
    outcome: { answer: '仅当条件成立时，结论成立。', incomplete: status === 'incomplete', effects: [], trace: [], memory_updates: [],
      profile_usage: { injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] } } as any };
  const wrapper = mount(RightRail, { props: { ...baseProps, renderMarkdown, chat: [turn] } });
  try {
    expect(wrapper.find('.answer-actions').exists()).toBe(status === 'completed');
    if (status === 'completed') {
      await wrapper.findAll('.answer-actions button').find(button => button.text() === '生成理解卡')!.trigger('click');
      expect(wrapper.emitted('share-answer')?.[0]).toEqual([turn]);
    }
  } finally { wrapper.unmount(); }
});

it('keeps each saved board part navigable during generation and after cancellation without a final answer', async () => {
  const first = { board_id: 'original-board', commit_id: 'part-one', before_revision: 1, after_revision: 2, item_ids: ['answer-one'], block_ids: ['block-one'], origin_session_id: 'session', origin_turn_id: 'turn', parent_item_id: 'point' };
  const second = { ...first, commit_id: 'part-two', before_revision: 2, after_revision: 3, item_ids: ['answer-two'], block_ids: ['block-two'] };
  const turn = { turnId: 'turn', user: '继续解释', pending: true, outcome: null, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [], boardCommits: [first, second, first] };
  const wrapper = mount(RightRail, { shallow: true, props: { ...baseProps, chat: [turn] } });
  expect(wrapper.findAll('.board-part-link')).toHaveLength(2);
  expect(wrapper.get('.board-parts-progress').text()).toContain('后续仍在讲解');
  await wrapper.findAll('.board-part-link')[1].trigger('click');
  expect(wrapper.emitted('open-board-location')).toEqual([[{ board_id: 'original-board', item_id: 'answer-two', block_id: 'block-two' }]]);
  await wrapper.setProps({ chat: [{ ...turn, pending: false, boardIncomplete: true, runStatus: '已停止' }] });
  expect(wrapper.findAll('.board-part-link')).toHaveLength(2);
  expect(wrapper.get('.board-parts-progress').text()).toContain('讲解尚未全部完成');
  expect(wrapper.find('.a-msg').exists()).toBe(false);
});
