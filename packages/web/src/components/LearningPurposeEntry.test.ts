// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import LearningPurposeEntry from './LearningPurposeEntry.vue';

const base = { available: true, ready: true, enabled: false, busy: false };

it('starts one chosen purpose in the selected scope and explains enabling', async () => {
  const wrapper = mount(LearningPurposeEntry, { props: { ...base, scopeOptions: [
    { value: 'selection', label: '当前选中的原文' }, { value: 'chapter', label: '第一章' }, { value: 'material', label: '当前材料' },
  ] } });
  expect(wrapper.text()).toContain('开启连续教学并开始');
  await wrapper.get('[data-purpose=understand]').trigger('click');
  await wrapper.get('select').setValue('material');
  await wrapper.get('[data-purpose=exam]').trigger('click');
  expect(wrapper.emitted('choose')).toEqual([['understand', 'selection'], ['exam', 'material']]);
  expect(wrapper.emitted('custom')).toBeUndefined();
  wrapper.unmount();
});

it('allows an overview before tutoring is ready and keeps blocked teaching from firing', async () => {
  const wrapper = mount(LearningPurposeEntry, { props: { ...base, ready: false } });
  expect(wrapper.get('[data-purpose=discover]').attributes('disabled')).toBeUndefined();
  for (const purpose of ['understand', 'exam', 'explore']) {
    expect(wrapper.get(`[data-purpose=${purpose}]`).attributes('disabled')).toBeDefined();
    await wrapper.get(`[data-purpose=${purpose}]`).trigger('click');
  }
  await wrapper.get('[data-purpose=discover]').trigger('click');
  expect(wrapper.emitted('choose')).toEqual([['discover', undefined]]);
  await wrapper.setProps({ busy: true });
  await wrapper.get('[data-purpose=discover]').trigger('click');
  expect(wrapper.emitted('choose')).toHaveLength(1);
  wrapper.unmount();
});

it('uses a new selection by default while preserving an explicit scope choice', async () => {
  const wrapper = mount(LearningPurposeEntry, { props: { ...base, scopeOptions: [{ value: 'material', label: '当前材料' }] } });
  await wrapper.setProps({ scopeOptions: [{ value: 'selection', label: '当前选中原文' }, { value: 'material', label: '当前材料' }] });
  await wrapper.get('[data-purpose=explore]').trigger('click');
  expect(wrapper.emitted('choose')?.[0]).toEqual(['explore', 'selection']);
  await wrapper.get('select').setValue('material');
  await wrapper.setProps({ scopeOptions: [{ value: 'selection', label: '新的选区' }, { value: 'material', label: '当前材料' }] });
  await wrapper.get('[data-purpose=explore]').trigger('click');
  expect(wrapper.emitted('choose')?.[1]).toEqual(['explore', 'material']);
  wrapper.unmount();
});

it('offers a direct overview for an undecided reader and a separate free-input action', async () => {
  const wrapper = mount(LearningPurposeEntry, { props: base });
  const buttons = wrapper.findAll('.purpose-other button');
  await buttons[0].trigger('click');
  await buttons[1].trigger('click');
  expect(wrapper.emitted('choose')).toEqual([['discover', undefined]]);
  expect(wrapper.emitted('custom')).toEqual([[]]);
  wrapper.unmount();
});

it('uses listening language without changing the chosen purpose, scope or readiness gate', async () => {
  const wrapper = mount(LearningPurposeEntry, { props: { ...base, musicMode: true, scopeOptions: [{ value: 'chapter', label: '这首作品' }] } });
  expect(wrapper.get('[data-purpose=understand]').text()).toContain('带我听懂');
  expect(wrapper.text()).not.toContain('读懂');
  await wrapper.get('[data-purpose=understand]').trigger('click');
  expect(wrapper.emitted('choose')).toEqual([['understand', 'chapter']]);
  await wrapper.setProps({ ready: false });
  await wrapper.get('[data-purpose=understand]').trigger('click');
  expect(wrapper.emitted('choose')).toHaveLength(1);
  await wrapper.get('.purpose-other button').trigger('click');
  expect(wrapper.emitted('choose')?.[1]).toEqual(['discover', 'chapter']);
  wrapper.unmount();
});
