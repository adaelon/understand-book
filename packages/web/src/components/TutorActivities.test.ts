// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import TutorActivities from './TutorActivities.vue';

it('renders teaching Markdown, math and bound sources without exposing raw markers', async () => {
  api.tutorActivities.mockResolvedValue({ activities: [{ delivery_ref: 'lesson', status: 'active', attempted: false, help_seen: [],
    move: { move_id: 'explain', prompt: '## SmartNIC\n\n**卸载**\n\n> 原文\n\n$Q$ [[source:ref1]]\n\n[[source:missing]]', actions: [] } }] });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn', sources: [{ source_ref_id: 'ref1', label: '正文 · SmartNIC' }] } });
  try {
    await flushPromises();
    expect(wrapper.get('h2').text()).toBe('SmartNIC');
    expect(wrapper.get('strong').text()).toBe('卸载');
    expect(wrapper.get('blockquote').text()).toBe('原文');
    expect(wrapper.find('.katex').exists()).toBe(true);
    expect(wrapper.text()).not.toContain('[[source:');
    expect(wrapper.text()).toContain('来源不可用');
    const source = wrapper.get('button[data-answer-source-index]');
    await source.trigger('click');
    expect(wrapper.emitted('source')?.[0]).toEqual(['ref1', source.element]);
  } finally { wrapper.unmount(); }
});

it.each(['## 已显示的讲解', '## 已显示的讲解 [[source:ref1]]'])('uses the visible answer for duplicate actionless delivery %s and still records its exposure', async prompt => {
  const answer = document.createElement('div'); answer.textContent = '已显示的讲解'; document.body.append(answer);
  api.tutorActivities.mockResolvedValue({ activities: [{ delivery_ref: 'followup', status: 'active', attempted: false, help_seen: [],
    move: { move_id: 'followup', prompt, actions: [] } }] });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn', answerText: '## 已显示的讲解', answerTarget: () => answer, sources: [{ source_ref_id: 'ref1', label: '正文 · 讲解' }] } });
  try {
    await flushPromises();
    expect(wrapper.find('article').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('已显示的讲解');
    expect(api.tutorDisplay).not.toHaveBeenCalled();
    reveal(answer); await flushPromises();
    expect(api.tutorDisplay).toHaveBeenCalledWith('followup', undefined);
  } finally { wrapper.unmount(); answer.remove(); }
});

it.each([
  ['它持有的内容书里列得很具体：', '它持有的内容，书里列得很具体：'],
  ['各有一份**阅读现场** [[source:ref1]]', '**各有一份阅读现场**'],
  ['现场负责阅读位置。', '阅读位置由当前现场维护。'],
])('uses the final answer when an actionless teaching candidate is edited: %s', async (prompt, answerText) => {
  const answer = document.createElement('div'); answer.textContent = answerText; document.body.append(answer);
  api.tutorActivities.mockResolvedValue({ activities: [{ delivery_ref: 'explanation', status: 'active', attempted: false, help_seen: [],
    move: { move_id: 'explain-scene-workspace', prompt, actions: [] } }] });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn', answerText, answerTarget: () => answer,
    sources: [{ source_ref_id: 'ref1', label: '表格 · 阅读现场' }] } });
  try {
    await flushPromises();
    expect(wrapper.find('article').exists()).toBe(false);
    expect(api.tutorDisplay).not.toHaveBeenCalled();
    reveal(answer); await flushPromises();
    expect(api.tutorDisplay).toHaveBeenCalledExactlyOnceWith('explanation', undefined);
  } finally { wrapper.unmount(); answer.remove(); }
});

it('keeps an actionless teaching candidate visible when no final answer is available', async () => {
  api.tutorActivities.mockResolvedValue({ activities: [{ delivery_ref: 'explanation', status: 'active', attempted: false, help_seen: [],
    move: { move_id: 'explain', prompt: '仍需展示的讲解', actions: [] } }] });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn', answerText: '', answerTarget: () => undefined } });
  try {
    await flushPromises();
    const article = wrapper.get('article');
    expect(article.text()).toContain('仍需展示的讲解');
    expect(api.tutorDisplay).not.toHaveBeenCalled();
    reveal(article.element); await flushPromises();
    expect(api.tutorDisplay).toHaveBeenCalledExactlyOnceWith('explanation', undefined);
  } finally { wrapper.unmount(); }
});

it('uses the visible presentation for actionless delivery without rendering its teaching plan', async () => {
  const reference = { presentation_id: 'embedding-demo', revision: 2 };
  const frame = document.createElement('iframe'); document.body.append(frame);
  const scene = { session_id: 'chat', turn_id: 'turn', reference, state_revision: 1, saved_state_ref: 'scene' };
  const snapshot = vi.fn().mockResolvedValue(scene);
  api.tutorActivities.mockResolvedValue({ activities: [{ delivery_ref: 'explanation', status: 'active', attempted: false, help_seen: [],
    move: { move_id: 'explain-embedding-usage', prompt: '交付演示页并完整解释 embedding 的调用位置与用法。不另开提问或练习。', actions: [], presentation: reference } }] });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn', reference, snapshot, presentationTarget: () => frame } });
  try {
    await flushPromises();
    expect(wrapper.find('article').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('交付演示页');
    expect(wrapper.emitted('response-available')?.at(-1)).toEqual([false]);
    expect(api.tutorDisplay).not.toHaveBeenCalled();
    reveal(frame); await flushPromises();
    expect(snapshot).toHaveBeenCalledOnce();
    expect(api.tutorDisplay).toHaveBeenCalledExactlyOnceWith('explanation', scene);
  } finally { wrapper.unmount(); frame.remove(); }
});

it('retains an answer-matching interactive question and formats its help', async () => {
  const answer = document.createElement('div'); answer.textContent = '题目'; document.body.append(answer);
  api.tutorActivities.mockResolvedValue({ activities: [{ delivery_ref: 'question', status: 'active', attempted: false, help_seen: [],
    move: { move_id: 'question', prompt: '**题目**', actions: ['submit', 'request_hint'] } }] });
  api.tutorAction.mockResolvedValue({ help: { event_id: 'help', text: '## 提示\n\n**比较需求** [[source:ref1]]' } });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn', answerText: '**题目**', answerTarget: () => answer, sources: [{ source_ref_id: 'ref1', label: '正文 · 需求' }] } });
  try {
    await flushPromises();
    expect(wrapper.get('article strong').text()).toBe('题目');
    reveal(wrapper.get('article').element); await flushPromises();
    await wrapper.get('[data-help-action="request_hint"]').trigger('click'); await flushPromises();
    expect(wrapper.get('.help h2').text()).toBe('提示');
    expect(wrapper.get('.help strong').text()).toBe('比较需求');
    expect(wrapper.find('.help button[data-answer-source-index]').exists()).toBe(true);
  } finally { wrapper.unmount(); answer.remove(); }
});

const api = vi.hoisted(() => ({
  tutorActivities: vi.fn(), tutorDisplay: vi.fn(), tutorAction: vi.fn(),
  tutorHelpDisplayed: vi.fn(), tutorFeedbackDisplayed: vi.fn(),
  tutorPageHelp: vi.fn(),
}));
vi.mock('../api', () => ({ api, ApiError: class extends Error {} }));
vi.mock('../network-context', () => ({ bindSceneApi: (value: unknown) => value }));
const notes = vi.hoisted(() => ({ openPersonalNotes: vi.fn() }));
vi.mock('../personal-notes', () => notes);

it('shows the released solution after a correct answer, still replies, and flags a questionable problem', async () => {
  const practice = { delivery_ref: 'practice', status: 'active', attempted: false, help_seen: [], practice_check: 'questionable',
    move: { move_id: 'practice', prompt: 'A cyclist rests…', actions: ['submit', 'request_hint', 'reveal', 'skip'],
      practice: { entry: 'warmup', mode: 'recall', hint_count: 2 } } };
  api.tutorActivities.mockResolvedValue({ activities: [practice] });
  api.tutorAction.mockResolvedValueOnce({ event_id: 'a1', assessment: 'correct',
    help: { event_id: 'h1', text: 'All 60 minutes count.', reason: 'concluded' } });
  const replies: string[] = [];
  const listener = (event: Event) => replies.push((event as CustomEvent<{ text: string }>).detail.text);
  window.addEventListener('tutor-response', listener);
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn' }, attachTo: document.body });
  try {
    await flushPromises();
    reveal(wrapper.get('article').element); await flushPromises();
    expect(wrapper.get('.practice-check').text()).toContain('可能有误');
    await wrapper.get('textarea').setValue('["A","B"]');
    await wrapper.get('button.primary').trigger('click'); await flushPromises();
    expect(wrapper.get('.hint-place').text()).toBe('完整解答');
    expect(wrapper.get('.help').text()).toContain('All 60 minutes count.');
    expect(replies).toEqual(['["A","B"]']);
  } finally { wrapper.unmount(); window.removeEventListener('tutor-response', listener); }
});

it('assembles an answer from card picks instead of free text', async () => {
  api.tutorActivities.mockResolvedValue({ activities: [{ delivery_ref: 'cards', status: 'active', attempted: false, help_seen: [],
    move: { move_id: 'cards', prompt: 'Pick the cards…', actions: ['submit', 'request_hint', 'reveal', 'skip'],
      practice: { entry: 'board', mode: 'compose', hint_count: 1, cards: [{ id: 'A', text: '20 km' }, { id: 'B', text: '60 min' }, { id: 'C', text: '50 min' }] } } }] });
  api.tutorAction.mockResolvedValueOnce({ event_id: 'a1', assessment: 'correct', help: null });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn' }, attachTo: document.body });
  try {
    await flushPromises();
    reveal(wrapper.get('article').element); await flushPromises();
    expect(wrapper.find('textarea').exists()).toBe(false);
    const cards = wrapper.findAll('.practice-card');
    await cards[1].trigger('click'); await cards[0].trigger('click'); await cards[2].trigger('click'); await cards[2].trigger('click');
    expect(cards[2].attributes('aria-pressed')).toBe('false');
    await wrapper.get('button.primary').trigger('click'); await flushPromises();
    expect(api.tutorAction.mock.calls.at(-1)?.[0]).toMatchObject({ action: 'submit', response: '["B","A"]' });
  } finally { wrapper.unmount(); }
});

it('restores released practice help and concludes a newly assessed open answer once', async () => {
  const activity = { delivery_ref: 'open-practice', status: 'active', attempted: true, help_seen: [],
    assessment: { status: 'unassessed' },
    move: { move_id: 'practice', prompt: 'A cyclist rests ten minutes…', actions: ['revise', 'request_hint', 'reveal'],
      practice: { entry: 'warmup', mode: 'recall', hint_count: 2 } } };
  api.tutorActivities.mockResolvedValue({ activities: [activity] });
  const onConcluded = vi.fn(); window.addEventListener('practice-concluded', onConcluded);
  let wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn' } });
  try {
    await flushPromises();
    expect(wrapper.find('.help').exists()).toBe(false);
    api.tutorActivities.mockResolvedValue({ activities: [{ ...activity, assessment: { status: 'correct' },
      help: { event_id: 'solution', text: 'Count all 60 minutes.', reason: 'concluded' } }] });
    window.dispatchEvent(new Event('tutor-state-changed')); await flushPromises();
    expect(wrapper.get('.help').text()).toContain('完整解答');
    expect(wrapper.get('.help').text()).toContain('Count all 60 minutes.');
    expect(onConcluded).toHaveBeenCalledTimes(1);
    window.dispatchEvent(new Event('tutor-state-changed')); await flushPromises();
    expect(onConcluded).toHaveBeenCalledTimes(1);
    wrapper.unmount();
    wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn' } });
    await flushPromises();
    expect(wrapper.get('.help').text()).toContain('Count all 60 minutes.');
    expect(onConcluded).toHaveBeenCalledTimes(1);
  } finally { wrapper.unmount(); window.removeEventListener('practice-concluded', onConcluded); }
});

it('labels practice hints by where they point and opens the exact note or passage', async () => {
  const practice = { delivery_ref: 'practice', status: 'active', attempted: false,
    help_seen: [{ action: 'request_hint', hint_kind: 'in_problem', hint_index: 0, text: 'Look at the rest.' }],
    move: { move_id: 'practice', prompt: 'A cyclist rests ten minutes…', actions: ['submit', 'request_hint', 'reveal'],
      practice: { entry: 'warmup', mode: 'recall', hint_count: 3 } } };
  api.tutorActivities.mockResolvedValue({ activities: [practice] });
  const note = { note_id: 'n1', revision: 2 };
  api.tutorAction
    .mockResolvedValueOnce({ event_id: 'a1', assessment: 'unassessed', help: { event_id: 'h1', text: 'Your earlier idea.', hint_kind: 'own_note', hint_index: 1, note_ref: note } })
    .mockResolvedValueOnce({ event_id: 'a2', assessment: 'unassessed', help: { event_id: 'h2', text: 'Read the passage.', hint_kind: 'source', hint_index: 2, source_lids: ['1.2'] } });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn' } });
  try {
    await flushPromises();
    reveal(wrapper.get('article').element); await flushPromises();
    expect(wrapper.get('.activity-heading').text()).toContain('练一练');
    expect(wrapper.get('[data-help-action="reveal"]').text()).toBe('看解答');
    // One hint was already displayed before the reload.
    const hint = () => wrapper.get('[data-help-action="request_hint"]');
    expect(hint().text()).toBe('提示（还有 2 条）');
    await hint().trigger('click'); await flushPromises();
    expect(wrapper.get('.hint-place').text()).toBe('你自己的想法');
    await wrapper.get('.hint-link').trigger('click');
    expect(notes.openPersonalNotes).toHaveBeenCalledWith(note);
    await hint().trigger('click'); await flushPromises();
    expect(wrapper.get('.hint-place').text()).toBe('回到原文');
    await wrapper.get('.hint-link').trigger('click');
    expect(wrapper.emitted('goto')?.[0]).toEqual(['1.2']);
    expect(hint().text()).toBe('提示（还有 0 条）');
    expect(hint().attributes('disabled')).toBeDefined();
  } finally { wrapper.unmount(); }
});
let intersection: IntersectionObserverCallback;
let displayed: boolean;
function reveal(target: Element) {
  const bounds = target.getBoundingClientRect();
  intersection([{ target, isIntersecting: true, boundingClientRect: bounds,
    intersectionRatio: 1, intersectionRect: bounds, rootBounds: null, time: 0 }], {} as IntersectionObserver);
}

beforeEach(() => {
  vi.clearAllMocks();
  displayed = false;
  vi.stubGlobal('IntersectionObserver', class {
    constructor(callback: IntersectionObserverCallback) { intersection = callback; }
    observe() {}
    disconnect() {}
  });
  api.tutorActivities.mockResolvedValue({ activities: [{
    delivery_ref: 'question', status: 'active', attempted: false, help_seen: [],
    move: { prompt: 'What changed?', actions: ['submit', 'request_hint'] },
  }] });
  api.tutorDisplay.mockImplementation(async () => { displayed = true; });
  api.tutorAction.mockImplementation(async (request) => {
    if (!displayed) throw new Error('Invalid request');
    return { event_id: 'action', assessment: 'unassessed', help: request.action === 'request_hint'
      ? { event_id: 'help', text: 'Compare the demand.' } : null };
  });
});
afterEach(() => vi.unstubAllGlobals());

it('focuses the active response from the learning dock without replacing a draft or submitting it', async () => {
  const wrapper = mount(TutorActivities, { attachTo: document.body, props: { sessionId: 'chat', turnId: 'turn' } });
  try {
    await flushPromises();
    expect(wrapper.emitted('response-available')?.at(-1)).toEqual([true]);
    const input = wrapper.get('textarea');
    await input.setValue('还没写完的判断');
    const scroll = vi.fn(); input.element.scrollIntoView = scroll;
    wrapper.vm.focusResponse();
    expect(scroll).toHaveBeenCalledWith({ block: 'center' });
    expect(document.activeElement).toBe(input.element);
    expect(input.element.value).toBe('还没写完的判断');
    expect(api.tutorAction).not.toHaveBeenCalled();
    api.tutorActivities.mockResolvedValue({ activities: [{ delivery_ref: 'question', status: 'paused', attempted: false, help_seen: [],
      move: { prompt: 'What changed?', actions: ['submit', 'request_hint'] } }] });
    window.dispatchEvent(new Event('tutor-state-changed')); await flushPromises();
    expect(wrapper.emitted('response-available')?.at(-1)).toEqual([false]);
  } finally { wrapper.unmount(); }
});

it.each(['submit', 'request_hint'])('registers a visible ordinary chat question before %s without a ready prop', async action => {
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn' } });
  try {
    await flushPromises();
    expect(api.tutorDisplay).not.toHaveBeenCalled();
    reveal(wrapper.get('article').element);
    await flushPromises();
    expect(api.tutorDisplay).toHaveBeenCalledWith('question', undefined);
    await wrapper.get('textarea').setValue('The demand increased.');
    await wrapper.findAll('button').find(button => button.text() === (action === 'submit' ? '提交回答' : '提示'))!.trigger('click');
    await flushPromises();
    expect(api.tutorAction).toHaveBeenCalledWith(expect.objectContaining({ action, delivery_ref: 'question' }));
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    if (action === 'request_hint') expect(wrapper.text()).toContain('Compare the demand.');
  } finally { wrapper.unmount(); }
});

it('waits for an explicitly unready presentation before registering its visible question', async () => {
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn', ready: false } });
  try {
    await flushPromises();
    reveal(wrapper.get('article').element);
    await flushPromises();
    expect(api.tutorDisplay).not.toHaveBeenCalled();
    await wrapper.setProps({ ready: true });
    await flushPromises();
    expect(api.tutorDisplay).toHaveBeenCalledTimes(1);
  } finally { wrapper.unmount(); }
});

it('states when the original question has no hint or explanation and does not invent a help action', async () => {
  api.tutorActivities.mockResolvedValue({ activities: [{ delivery_ref:'question',status:'active',attempted:false,help_seen:[],move:{move_id:'q',prompt:'Explain',actions:['submit']} }] });
  const wrapper=mount(TutorActivities,{props:{sessionId:'chat',turnId:'turn'}}); await flushPromises();
  expect(wrapper.text()).toContain('当前题目未提供提示或直接讲解');
  expect(wrapper.find('[data-help-action]').exists()).toBe(false); expect(api.tutorAction).not.toHaveBeenCalled(); wrapper.unmount();
});

const invitation = (status = 'open') => ({ delivery_ref: 'invitation', status: 'active', attempted: true, help_seen: [],
  response_invitation_status: status, assessment: { status: 'unassessed' },
  move: { move_id: 'preserve-and-select', prompt: 'Agent准备的讲解，不应重复成为题面', actions: [],
    response_expectation: { public_prompt: '章节没有选乙，主题还能用乙吗？说说你的理由。', block_refs: ['preserve-vs-select'] } } });

it('offers an ordinary understanding invitation without a rubric and sends actual wording through chat', async () => {
  api.tutorActivities.mockResolvedValue({ activities: [invitation()] });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn' } });
  const received = vi.fn(); wrapper.element.addEventListener('tutor-response', received);
  try {
    await flushPromises();
    expect(wrapper.text()).toContain('章节没有选乙');
    expect(wrapper.text()).not.toContain('Agent准备');
    expect(wrapper.text()).not.toContain('尚未评分');
    expect(wrapper.text()).not.toContain('当前题目未提供');
    expect(wrapper.find('.interactive').exists()).toBe(false);
    await wrapper.get('textarea').setValue('我觉得乙还在，只是这次没选它。');
    await wrapper.get('button.primary').trigger('click'); await flushPromises();
    expect(api.tutorAction).not.toHaveBeenCalled();
    expect(received.mock.calls[0][0].detail).toEqual({ eventId: 'invitation', text: '我觉得乙还在，只是这次没选它。', scene: undefined, responseOrigin: { kind: 'user_text' } });
  } finally { wrapper.unmount(); }
});

it.each([
  ['exact wording', '换个假设：点丁仍在C中，原先既被某章accepted，也被某主题选中。现在只撤掉主题对丁的选择，章节accepted保持不变，其他条件不变。丁会从最终key_stops消失吗？用一句话说说依据。'],
  ['revised live wording', '换个小情境试着用一下这条关系：假设丁仍在 C 中，原先既被某章 accepted，也被某主题选中。现在只撤掉主题对丁的选择，章节 accepted 保持不变，其他条件也不变。丁会从最终 key_stops 消失吗？用一句话说说依据就行。'],
])('keeps the final answer as the sole ordinary invitation carrier: %s', async (_, deliveredPrompt) => {
  const prompt = '换个假设：点丁仍在C中，原先既被某章accepted，也被某主题选中。现在只撤掉主题对丁的选择，章节accepted保持不变，其他条件不变。丁会从最终key_stops消失吗？用一句话说说依据。';
  const answerText = `最终 key_stops = 所有章节 accepted 的点 ∪ 所有主题的成员点\n\n${deliveredPrompt}`;
  const answer = document.createElement('div');
  const opening = document.createElement('p'); opening.textContent = '最终 key_stops = 所有章节 accepted 的点 ∪ 所有主题的成员点';
  const invitationParagraph = document.createElement('p'); invitationParagraph.textContent = deliveredPrompt;
  answer.append(opening, invitationParagraph); document.body.append(answer);
  const activity = invitation(); activity.move.response_expectation.public_prompt = prompt;
  api.tutorActivities.mockResolvedValue({ activities: [activity] });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn', answerText, answerTarget: () => answer } });
  const received = vi.fn(); wrapper.element.addEventListener('tutor-response', received);
  try {
    await flushPromises();
    expect(wrapper.find('.activity-text').exists()).toBe(false);
    expect(answer.textContent).toBe(opening.textContent + deliveredPrompt);
    expect(wrapper.find('textarea').exists()).toBe(true);
    expect(wrapper.text()).toContain('帮我找个起点');
    expect(wrapper.text()).toContain('先完整讲给我');
    expect(wrapper.text()).toContain('暂缓这个问题');
    expect(wrapper.emitted('response-available')?.at(-1)).toEqual([true]);
    const boundary = answer.querySelector('.answer-delivery-end')!;
    expect(boundary).toBe(answer.lastElementChild);
    reveal(answer); reveal(opening); reveal(invitationParagraph); reveal(wrapper.get('article').element); await flushPromises();
    expect(api.tutorDisplay).not.toHaveBeenCalled();
    reveal(boundary); await flushPromises();
    expect(api.tutorDisplay).toHaveBeenCalledExactlyOnceWith('invitation', undefined);
    await wrapper.get('textarea').setValue('我想不会，因为章节还采用丁。');
    await wrapper.get('button.primary').trigger('click'); await flushPromises();
    expect(received.mock.calls[0][0].detail).toMatchObject({ eventId: 'invitation', text: '我想不会，因为章节还采用丁。' });
  } finally { wrapper.unmount(); answer.remove(); }
});

it.each(['missing-target', 'missing-answer', 'unrendered-answer', 'page-host', 'page-fallback'])('retains a usable invitation outside the rendered ordinary chat answer: %s', async scenario => {
  const activity = invitation();
  const prompt = activity.move.response_expectation.public_prompt;
  const answer = document.createElement('p');
  if (scenario !== 'unrendered-answer') answer.textContent = prompt;
  document.body.append(answer);
  const reference = { presentation_id: 'cards', revision: 1 };
  api.tutorActivities.mockResolvedValue({ activities: [{ ...activity,
    move: { ...activity.move, ...(scenario === 'page-fallback' ? { presentation: reference } : {}) } }] });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn',
    answerText: scenario === 'missing-answer' ? '' : prompt,
    answerTarget: () => scenario === 'missing-target' ? undefined : answer,
    ...(scenario === 'page-host' ? { reference } : {}) } });
  try {
    await flushPromises();
    expect(wrapper.text()).toContain(prompt);
    expect(wrapper.find('textarea').exists()).toBe(true);
    expect(answer.querySelector('.answer-delivery-end')).toBeNull();
    reveal(answer); await flushPromises();
    expect(api.tutorDisplay).not.toHaveBeenCalled();
    reveal(wrapper.get('article').element); await flushPromises();
    expect(api.tutorDisplay).toHaveBeenCalledExactlyOnceWith('invitation', undefined);
  } finally { wrapper.unmount(); answer.remove(); }
});

it.each([
  ['帮我找个起点', '我不知道从哪里开始想，请帮我找一个起点。'],
  ['先完整讲给我', '请先完整讲解这一步。'],
  ['暂缓这个问题', '这次先暂缓这个问题，我想继续阅读。'],
])('keeps the %s request distinct from the unsent learner draft', async (label, message) => {
  api.tutorActivities.mockResolvedValue({ activities: [invitation()] });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn' } });
  const received = vi.fn(); wrapper.element.addEventListener('tutor-response', received);
  try {
    await flushPromises();
    await wrapper.get('textarea').setValue('这段话还没写完');
    await wrapper.findAll('button').find(button => button.text() === label)!.trigger('click'); await flushPromises();
    expect(received.mock.calls[0][0].detail).toMatchObject({ eventId: 'invitation', text: message, responseOrigin: { kind: 'interface_request' } });
    expect(wrapper.get('textarea').element.value).toBe('这段话还没写完');
    expect(api.tutorAction).not.toHaveBeenCalled();
  } finally { wrapper.unmount(); }
});

it('carries the exact page scene and original prefill even after the user edits it', async () => {
  const reference = { presentation_id: 'cards', revision: 2 };
  const scene = { session_id: 'chat', turn_id: 'turn', reference, state_revision: 3, saved_state_ref: 'scene-before-reply' };
  const snapshot = vi.fn().mockResolvedValue(scene);
  api.tutorActivities.mockResolvedValue({ activities: [invitation()] });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn', reference, snapshot } });
  const received = vi.fn(); wrapper.element.addEventListener('tutor-response', received);
  try {
    await flushPromises();
    wrapper.element.scrollIntoView = vi.fn(); wrapper.get('textarea').element.scrollIntoView = vi.fn();
    wrapper.vm.focusActivity('preserve-and-select', '乙仍然存在。'); await flushPromises();
    expect(received).not.toHaveBeenCalled();
    await wrapper.get('textarea').setValue('乙仍然存在，但我不确定主题为什么能找到它。');
    await wrapper.get('button.primary').trigger('click'); await flushPromises();
    expect(snapshot).toHaveBeenCalledTimes(1);
    expect(received.mock.calls[0][0].detail).toMatchObject({ eventId: 'invitation', scene,
      text: '乙仍然存在，但我不确定主题为什么能找到它。',
      responseOrigin: { kind: 'presentation_prefill', prefill_text: '乙仍然存在。', presentation: reference } });
  } finally { wrapper.unmount(); }
});

it('keeps one invitation identity when switching from a chat fallback to its page host', async () => {
  const reference = { presentation_id: 'cards', revision: 2 };
  const activity = invitation();
  api.tutorActivities.mockResolvedValue({ activities: [{ ...activity, move: { ...activity.move, presentation: reference } }] });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn' } });
  try {
    await flushPromises();
    expect(wrapper.get('article').attributes('data-delivery-ref')).toBe('invitation');
    await wrapper.get('.scene-link').trigger('click');
    expect(wrapper.emitted('locate-presentation')).toEqual([[reference]]);
    await wrapper.setProps({ hostedPresentations: [reference] });
    expect(wrapper.find('article').exists()).toBe(false);
    await wrapper.setProps({ hostedPresentations: [] });
    expect(wrapper.get('article').attributes('data-delivery-ref')).toBe('invitation');
  } finally { wrapper.unmount(); }
});

it.each(['addressed', 'deferred'])('retains an %s invitation without presenting it as an unanswered test', async status => {
  api.tutorActivities.mockResolvedValue({ activities: [invitation(status)] });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn' } });
  try {
    await flushPromises();
    expect(wrapper.text()).toContain('章节没有选乙');
    expect(wrapper.find('textarea').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('尚未评分');
    expect(wrapper.emitted('response-available')?.at(-1)).toEqual([false]);
  } finally { wrapper.unmount(); }
});

it('records displayed page help with the saved scene, without submitting a learner response', async () => {
  const reference = { presentation_id: 'cards', revision: 2 };
  const scene = { session_id: 'chat', turn_id: 'turn', reference, state_revision: 3, saved_state_ref: 'help-scene' };
  const snapshot = vi.fn().mockResolvedValue(scene);
  api.tutorActivities.mockResolvedValue({ activities: [invitation()] });
  api.tutorPageHelp.mockResolvedValue({ event_id: 'help-shown' });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn', reference, snapshot } });
  const received = vi.fn(); wrapper.element.addEventListener('tutor-response', received);
  try {
    await flushPromises();
    await wrapper.vm.reportPageHelp('preserve-and-select', 'hint', '比较乙在两个集合中的位置。');
    expect(api.tutorPageHelp).toHaveBeenCalledWith({ delivery_ref: 'invitation', kind: 'hint', text: '比较乙在两个集合中的位置。', presentation_follow_up: scene });
    expect(received).not.toHaveBeenCalled();
    expect(api.tutorAction).not.toHaveBeenCalled();
  } finally { wrapper.unmount(); }
});

it('does not let a learner response overtake a page help receipt, and retries that same saved help scene', async () => {
  const reference = { presentation_id: 'cards', revision: 2 };
  const helpScene = { session_id: 'chat', turn_id: 'turn', reference, state_revision: 3, saved_state_ref: 'help-scene' };
  const replyScene = { ...helpScene, state_revision: 4, saved_state_ref: 'reply-scene' };
  const snapshot = vi.fn().mockResolvedValueOnce(helpScene).mockResolvedValue(replyScene);
  api.tutorActivities.mockResolvedValue({ activities: [invitation()] });
  let finish!: () => void;
  api.tutorPageHelp.mockImplementationOnce(() => new Promise(resolve => { finish = () => resolve({ event_id: 'help-shown' }); }));
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn', reference, snapshot } });
  const received = vi.fn(); wrapper.element.addEventListener('tutor-response', received);
  try {
    await flushPromises();
    const help = wrapper.vm.reportPageHelp('preserve-and-select', 'hint', '比较乙的位置。');
    await flushPromises();
    await wrapper.get('textarea').setValue('乙还留在候选里。');
    await wrapper.get('button.primary').trigger('click'); await flushPromises();
    expect(received).not.toHaveBeenCalled();
    expect(snapshot).toHaveBeenCalledTimes(1);
    finish(); await help; await flushPromises();
    expect(received.mock.calls[0][0].detail.scene).toEqual(replyScene);

    api.tutorPageHelp.mockRejectedValueOnce(new Error('未写入')).mockResolvedValue({ event_id: 'help-retried' });
    await expect(wrapper.vm.reportPageHelp('preserve-and-select', 'explanation', '候选没有被删除。')).rejects.toThrow('未写入');
    const captures = snapshot.mock.calls.length;
    await wrapper.vm.waitForHelp();
    expect(snapshot).toHaveBeenCalledTimes(captures);
    expect(api.tutorPageHelp.mock.calls.at(-1)).toEqual(api.tutorPageHelp.mock.calls.at(-2));
  } finally { wrapper.unmount(); }
});
