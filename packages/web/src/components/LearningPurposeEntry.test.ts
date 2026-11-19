// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { expect, it, vi } from 'vitest';
import { ref } from 'vue';
import { readingGuidanceKey, type ReadingGuidance, type GuidanceContext } from '../reading-guidance';
import LearningPurposeEntry from './LearningPurposeEntry.vue';

const base = { available: true, ready: true, enabled: false, busy: false };
it('starts the requested question format in the selected reading scope', async () => {
  const wrapper = mount(LearningPurposeEntry,{props:{...base, initialPurpose:'practice',scopeOptions:[{value:'section',label:'当前小节'}]}});
  try {
    await wrapper.get('.purpose-setup select').setValue('worked_solution');
    await wrapper.get('.purpose-submit').trigger('click');
    expect(wrapper.emitted('choose')?.[0]).toEqual(['practice','section',{message:'请在所选范围内出一道需要写出过程的计算题。'}]);
  } finally { wrapper.unmount(); }
});
const scopeOptions = [{ value: 'section' as const, label: '当前小节 · 引言' }, { value: 'material' as const, label: '整份材料' }];

const context = () => ref<GuidanceContext>({ key: 'reader:book', title: '趋势与套利',
  scopes: [{ ...scopeOptions[0], lid: '1.2' }, scopeOptions[1]], selection: null, intent: '', medium: 'text' });
const guide = { question: '这两种方法，哪一点让你犹豫？', draft: '我想弄清趋势和套利分别在预测什么。' };
const guided = (load = vi.fn(async () => guide)) => mount(LearningPurposeEntry, {
  props: { ...base, scopeOptions }, attachTo: document.body,
  global: { provide: { [readingGuidanceKey as symbol]: { context: context(), load } } },
});

it.each(['understand', 'explore'] as const)('keeps %s ghost text out of the draft until Tab and never sends it automatically', async purpose => {
  const load = vi.fn(async () => guide), wrapper = guided(load);
  await wrapper.get(`[data-purpose=${purpose}]`).trigger('click'); await flushPromises();
  const input = wrapper.get('textarea');
  expect(load).toHaveBeenCalledWith(expect.objectContaining({ purpose, scope: 'section', lid: '1.2' }));
  expect(input.element.value).toBe('');
  expect(input.attributes('placeholder')).toBe(guide.draft);
  expect(wrapper.get('.purpose-submit').attributes('disabled')).toBeDefined();
  await input.trigger('keydown', { key: 'Tab' });
  expect(input.element.value).toBe(guide.draft);
  expect(document.activeElement).toBe(input.element);
  expect(wrapper.find('.input-suggestion-hint').exists()).toBe(false);
  expect(wrapper.emitted('choose')).toBeUndefined();
  await input.setValue(guide.draft + ' 我更关心它们什么时候适用。');
  expect(wrapper.get('.purpose-setup label').text()).toBe(guide.question);
  await wrapper.get('.purpose-submit').trigger('click');
  expect(wrapper.emitted('choose')?.[0]).toEqual([purpose, 'section', { message: input.element.value }]);
  expect(load).toHaveBeenCalledTimes(1);
  wrapper.unmount();
});

it('keeps normal Tab navigation, typed text and Chinese composition independent of the suggestion', async () => {
  const wrapper = guided();
  await wrapper.get('[data-purpose=understand]').trigger('click'); await flushPromises();
  const input = wrapper.get('textarea');
  const reverse = new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, cancelable: true, bubbles: true });
  input.element.dispatchEvent(reverse); expect(reverse.defaultPrevented).toBe(false);
  await input.trigger('compositionstart');
  await input.trigger('keydown', { key: 'Tab' });
  await input.trigger('keydown', { key: 'Enter', ctrlKey: true });
  expect(input.element.value).toBe(''); expect(wrapper.emitted('choose')).toBeUndefined();
  await input.trigger('compositionend');
  await input.setValue('我自己的理解');
  const tab = new KeyboardEvent('keydown', { key: 'Tab', cancelable: true, bubbles: true });
  input.element.dispatchEvent(tab); expect(tab.defaultPrevented).toBe(false);
  expect(input.element.value).toBe('我自己的理解');
  expect(wrapper.find('.input-suggestion-hint').exists()).toBe(false);
  wrapper.unmount();
});

it('accepts by touch and leaves the result editable without starting learning', async () => {
  const wrapper = guided();
  await wrapper.get('[data-purpose=explore]').trigger('click'); await flushPromises();
  await wrapper.get('.input-suggestion-hint button').trigger('click');
  expect(wrapper.get('textarea').element.value).toBe(guide.draft);
  expect(wrapper.emitted('choose')).toBeUndefined();
  wrapper.unmount();
});

it('preserves a composed reading question through both adjacent start actions', async () => {
  const wrapper = guided();
  await wrapper.get('[data-purpose=understand]').trigger('click');
  const input = wrapper.get('textarea');
  await input.setValue('这里的海运');
  await input.trigger('compositionstart');
  input.element.value += '指什么';
  await input.trigger('input');
  expect(wrapper.get('.purpose-submit').attributes('disabled')).toBeDefined();
  expect(wrapper.get('.purpose-fallback').attributes('disabled')).toBeDefined();
  await wrapper.get('.purpose-submit').trigger('click');
  await input.trigger('keydown', { key: 'Enter', ctrlKey: true });
  expect(wrapper.emitted('choose')).toBeUndefined();
  await input.trigger('compositionend');
  await wrapper.get('.purpose-fallback').trigger('click');
  expect(wrapper.emitted('choose')).toBeUndefined();
  await wrapper.get('.purpose-submit').trigger('click');
  expect(wrapper.emitted('choose')).toEqual([['understand', 'section', { message: '这里的海运指什么' }]]);
  await input.setValue('');
  expect(wrapper.get('.purpose-fallback').attributes('disabled')).toBeUndefined();
  wrapper.unmount();
});

it('ignores a late result from a different purpose or after the reader has started typing', async () => {
  const pending: ((value: ReadingGuidance) => void)[] = [];
  const wrapper = guided(vi.fn(() => new Promise<ReadingGuidance>(resolve => pending.push(resolve))));
  await wrapper.get('[data-purpose=understand]').trigger('click');
  await wrapper.get('.purpose-back').trigger('click');
  await wrapper.get('[data-purpose=explore]').trigger('click');
  pending[0](guide); await flushPromises();
  expect(wrapper.find('.input-suggestion-hint').exists()).toBe(false);
  await wrapper.get('textarea').setValue('我想问自己的问题');
  pending[1]({ question: '晚到的问题', draft: '晚到的文字' }); await flushPromises();
  expect(wrapper.get('textarea').element.value).toBe('我想问自己的问题');
  expect(wrapper.get('.purpose-setup label').text()).toBe('你想探索什么问题？');
  wrapper.unmount();
});

it('retains the ordinary input when generation fails', async () => {
  const wrapper = guided(vi.fn(async () => { throw new Error('offline'); }));
  await wrapper.get('[data-purpose=understand]').trigger('click'); await flushPromises();
  expect(wrapper.find('.input-suggestion-hint').exists()).toBe(false);
  await wrapper.get('textarea').setValue('我想弄清一个问题');
  await wrapper.get('.purpose-submit').trigger('click');
  expect(wrapper.emitted('choose')?.[0]).toEqual(['understand', 'section', { message: '我想弄清一个问题' }]);
  wrapper.unmount();
});

it.each(['understand', 'exam', 'explore', 'practice'] as const)('collects a real %s request before starting and preserves it while busy', async purpose => {
  const wrapper = mount(LearningPurposeEntry, { props: { ...base, scopeOptions }, attachTo: document.body });
  await wrapper.get('[data-purpose=' + purpose + ']').trigger('click');
  expect(wrapper.emitted('choose')).toBeUndefined();
  expect(document.activeElement).toBe(wrapper.get('textarea').element);
  expect(wrapper.get('.purpose-submit').attributes('disabled')).toBeDefined();
  await wrapper.get('textarea').setValue('  想弄清趋势与套利的区别  ');
  await wrapper.get('select').setValue('material');
  await wrapper.get('.purpose-submit').trigger('click');
  expect(wrapper.emitted('choose')).toEqual([[purpose, 'material', { message: '想弄清趋势与套利的区别' }]]);
  await wrapper.setProps({ busy: true });
  await wrapper.get('.purpose-submit').trigger('click');
  await wrapper.get('.purpose-fallback').trigger('click');
  expect(wrapper.emitted('choose')).toHaveLength(1);
  expect(wrapper.get('textarea').element.value).toContain('想弄清趋势与套利的区别');
  wrapper.unmount();
});

it('offers a grounded overview directly without enabling teaching', async () => {
  const wrapper = mount(LearningPurposeEntry, { props: { ...base, ready: false, scopeOptions } });
  for (const purpose of ['understand', 'exam', 'explore', 'practice']) {
    expect(wrapper.get('[data-purpose=' + purpose + ']').attributes('disabled')).toBeDefined();
  }
  await wrapper.get('[data-purpose=discover]').trigger('click');
  expect(wrapper.emitted('choose')).toEqual([['discover', 'section']]);
  expect(wrapper.find('textarea').exists()).toBe(false);
  await wrapper.setProps({ busy: true });
  await wrapper.get('[data-purpose=discover]').trigger('click');
  expect(wrapper.emitted('choose')).toHaveLength(1);
  wrapper.unmount();
});

it.each([
  ['understand', '选出一个值得理解的重点'], ['exam', '我没有具体考试目标'], ['explore', '等我选一个后再展开'],
] as const)('makes the undecided %s path an explicit, scoped request', async (purpose, text) => {
  const wrapper = mount(LearningPurposeEntry, { props: { ...base, scopeOptions } });
  await wrapper.get('[data-purpose=' + purpose + ']').trigger('click');
  await wrapper.get('.purpose-fallback').trigger('click');
  expect(wrapper.emitted('choose')?.[0]).toEqual([purpose, 'section', { message: expect.stringContaining(text) }]);
  wrapper.unmount();
});

it('takes a beginner to guided reading without inventing a practice topic or discarding a practice draft', async () => {
  const wrapper = mount(LearningPurposeEntry, { props: { ...base, scopeOptions } });
  await wrapper.get('[data-purpose=practice]').trigger('click');
  await wrapper.get('textarea').setValue('我还没学过套利');
  await wrapper.get('.purpose-fallback').trigger('click');
  expect(wrapper.emitted('choose')).toBeUndefined();
  expect(wrapper.get('.purpose-setup label').text()).toBe('想读懂哪里？');
  expect(wrapper.get('textarea').element.value).toBe('');
  await wrapper.get('.purpose-back').trigger('click');
  await wrapper.get('[data-purpose=practice]').trigger('click');
  expect(wrapper.get('textarea').element.value).toBe('我还没学过套利');
  wrapper.unmount();
});

it('retains an explicit scope while new selected text becomes available', async () => {
  const wrapper = mount(LearningPurposeEntry, { props: { ...base, scopeOptions } });
  await wrapper.get('select').setValue('material');
  await wrapper.setProps({ scopeOptions: [{ value: 'selection', label: '所选原文' }, ...scopeOptions] });
  await wrapper.get('[data-purpose=discover]').trigger('click');
  expect(wrapper.emitted('choose')?.[0]).toEqual(['discover', 'material']);
  wrapper.unmount();
});

it('offers a direct overview and a separate free-input action', async () => {
  const wrapper = mount(LearningPurposeEntry, { props: base });
  const buttons = wrapper.findAll('.purpose-other button');
  await buttons[0].trigger('click');
  await buttons[1].trigger('click');
  expect(wrapper.emitted('choose')).toEqual([['discover', undefined]]);
  expect(wrapper.emitted('custom')).toEqual([[]]);
  wrapper.unmount();
});

it.each([{ musicMode: true, question: '想听懂哪里？' }, { videoMode: true, question: '想看懂哪里？' }])('uses the right medium and respects readiness at submission ($question)', async mode => {
  const wrapper = mount(LearningPurposeEntry, { props: { ...base, ...mode, scopeOptions } });
  await wrapper.get('[data-purpose=understand]').trigger('click');
  expect(wrapper.get('.purpose-setup label').text()).toBe(mode.question);
  await wrapper.get('textarea').setValue('这里的变化');
  await wrapper.setProps({ ready: false, reason: '完整内容仍在准备。' });
  await wrapper.get('.purpose-submit').trigger('click');
  expect(wrapper.emitted('choose')).toBeUndefined();
  expect(wrapper.get('.purpose-notice').text()).toContain('完整内容仍在准备');
  await wrapper.setProps({ available: false, reason: '读取失败，请重试。' });
  expect(wrapper.get('.purpose-notice').text()).toBe('读取失败，请重试。');
  wrapper.unmount();
});
