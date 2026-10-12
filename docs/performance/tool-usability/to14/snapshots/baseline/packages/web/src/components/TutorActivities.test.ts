// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import TutorActivities from './TutorActivities.vue';

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
