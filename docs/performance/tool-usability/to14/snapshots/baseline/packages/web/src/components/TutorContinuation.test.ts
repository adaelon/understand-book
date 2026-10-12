// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import TutorContinuation from './TutorContinuation.vue';
const api = vi.hoisted(() => ({ tutorActivities: vi.fn(), tutorContinuationDisplayed: vi.fn() }));
vi.mock('../api', () => ({ api }));
vi.mock('../network-context', () => ({ bindSceneApi: (value: unknown) => value }));
let intersect: IntersectionObserverCallback;
const view = { delivery_ref: 'delivery', origin_turn_id: 'origin', expected_chat_head: 'origin', target_summary: '总时间为什么在分母',
  availability: { status: 'eligible' }, continuation: { recommended_option_id: 'compare', options: [
    { option_id: 'stop', label: '加入一次停顿', purpose: '看时间增加的影响', move_kind: 'observe' },
    { option_id: 'compare', label: '比较两段等长行程', purpose: '分清时间与段数', move_kind: 'compare' },
  ] } };
function mountHost(extra = {}) { return mount(TutorContinuation, { props: { sessionId: 'chat', turnId: 'origin', head: 'origin', enabled: true, ...extra } }); }
beforeEach(() => {
  vi.clearAllMocks();
  vi.stubGlobal('IntersectionObserver', class { constructor(callback: IntersectionObserverCallback) { intersect = callback; } observe() {} disconnect() {} });
  api.tutorActivities.mockResolvedValue({ activities: [{ continuation: view }] });
  api.tutorContinuationDisplayed.mockResolvedValue({ event_id: 'seen' });
});
afterEach(() => vi.unstubAllGlobals());
it('records only visible options in displayed order and keeps choice usable after receipt failure', async () => {
  const wrapper = mountHost(); await flushPromises();
  expect(api.tutorContinuationDisplayed).not.toHaveBeenCalled();
  api.tutorContinuationDisplayed.mockRejectedValueOnce(new Error('offline'));
  const target = wrapper.get('[data-option-id="compare"]').element;
  intersect([{ target, isIntersecting: true } as IntersectionObserverEntry], {} as IntersectionObserver); await flushPromises();
  expect(api.tutorContinuationDisplayed).toHaveBeenCalledWith({ delivery_ref: 'delivery', option_ids: ['compare'], recommended_option_id: 'compare' });
  expect(wrapper.text()).toContain('仍可选择');
  await wrapper.get('.recommended').trigger('click'); await flushPromises();
  expect(wrapper.emitted('choose')?.[0]).toEqual([view, 'compare', undefined]); wrapper.unmount();
});
it('freezes the original presentation and ignores a double click while saving it', async () => {
  let finish!: (value: any) => void;
  const snapshot = vi.fn(() => new Promise<any>(resolve => { finish = resolve; }));
  const reference = { presentation_id: 'p', revision: 1 };
  const wrapper = mountHost({ reference, snapshot }); await flushPromises();
  await wrapper.get('.recommended').trigger('click'); await wrapper.get('.recommended').trigger('click');
  expect(snapshot).toHaveBeenCalledTimes(1);
  const receipt = { reference, session_id: 'chat', turn_id: 'origin', state_id: 'saved' };
  finish(receipt); await flushPromises();
  expect(wrapper.emitted('choose')).toEqual([[view, 'compare', receipt]]); wrapper.unmount();
});
it.each(['head', 'pause', 'version'])('does not submit a snapshot that finishes after %s changes', async change => {
  let finish!: (value: any) => void;
  const wrapper = mountHost({ reference: { presentation_id: 'p', revision: 1 }, snapshot: () => new Promise<any>(resolve => { finish = resolve; }) });
  await flushPromises(); await wrapper.get('.recommended').trigger('click');
  await wrapper.setProps(change === 'head' ? { head: 'new' } : change === 'pause' ? { enabled: false } : { reference: { presentation_id: 'p', revision: 2 } });
  finish({ state_id: 'old' }); await flushPromises(); expect(wrapper.emitted('choose')).toBeUndefined(); wrapper.unmount();
});
it('makes old sets read only and routes consumed sets to the accepted turn', async () => {
  const wrapper = mountHost({ head: 'new' }); await flushPromises();
  expect(wrapper.find('.recommended').exists()).toBe(false);
  api.tutorActivities.mockResolvedValue({ activities: [{ continuation: { ...view, availability: { status: 'consumed', accepted_turn_id: 'accepted' } } }] });
  window.dispatchEvent(new Event('tutor-state-changed')); await flushPromises();
  await wrapper.findAll('button').find(b => b.text() === '查看已接纳的任务')!.trigger('click');
  expect(wrapper.emitted('locate')).toEqual([['accepted']]); wrapper.unmount();
});
