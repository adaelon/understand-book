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

it('retains an answer-matching interactive question and formats its help', async () => {
  api.tutorActivities.mockResolvedValue({ activities: [{ delivery_ref: 'question', status: 'active', attempted: false, help_seen: [],
    move: { move_id: 'question', prompt: '**题目**', actions: ['submit', 'request_hint'] } }] });
  api.tutorAction.mockResolvedValue({ help: { event_id: 'help', text: '## 提示\n\n**比较需求** [[source:ref1]]' } });
  const wrapper = mount(TutorActivities, { props: { sessionId: 'chat', turnId: 'turn', answerText: '**题目**', sources: [{ source_ref_id: 'ref1', label: '正文 · 需求' }] } });
  try {
    await flushPromises();
    expect(wrapper.get('article strong').text()).toBe('题目');
    reveal(wrapper.get('article').element); await flushPromises();
    await wrapper.get('[data-help-action="request_hint"]').trigger('click'); await flushPromises();
    expect(wrapper.get('.help h2').text()).toBe('提示');
    expect(wrapper.get('.help strong').text()).toBe('比较需求');
    expect(wrapper.find('.help button[data-answer-source-index]').exists()).toBe(true);
  } finally { wrapper.unmount(); }
});

const api = vi.hoisted(() => ({
  tutorActivities: vi.fn(), tutorDisplay: vi.fn(), tutorAction: vi.fn(),
  tutorHelpDisplayed: vi.fn(), tutorFeedbackDisplayed: vi.fn(),
}));
vi.mock('../api', () => ({ api, ApiError: class extends Error {} }));
vi.mock('../network-context', () => ({ bindSceneApi: (value: unknown) => value }));
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
