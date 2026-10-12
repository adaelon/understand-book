// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import TutorControl from './TutorControl.vue';

it('only requests a toggle and waits for confirmed state', async () => {
  const w = mount(TutorControl,{props:{enabled:false,label:'围绕你的目标逐步学',busy:false,unavailable:false,error:''}});
  const toggle = w.get('[role=switch]');
  expect(w.text()).toContain('连续教学开关对所有书籍生效');
  await toggle.trigger('click');
  expect(w.emitted('toggle')).toEqual([[]]);
  expect(toggle.attributes('aria-checked')).toBe('false');
  await w.setProps({enabled:true});
  expect(toggle.attributes('aria-checked')).toBe('true');
  w.unmount();
});

it('blocks toggling pending state while keeping the retry action reachable', async () => {
  const w = mount(TutorControl,{props:{enabled:true,label:'本次学习已暂停',busy:false,unavailable:true,error:'设置未确认保存'}});
  expect(w.get('[role=switch]').attributes('disabled')).toBeDefined();
  await w.get('[role=alert] button').trigger('click');
  expect(w.emitted('retry')).toHaveLength(1);
  expect(w.emitted('toggle')).toBeUndefined();
  await w.setProps({busy:true});
  expect(w.get('[role=alert] button').attributes('disabled')).toBeDefined();
  w.unmount();
});
