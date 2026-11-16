// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import CourseTutorControl from './CourseTutorControl.vue';
import type { CourseLessonView } from '../generated/CourseLessonView';

it('keeps course entry, pause, resume and ended states scoped to the course', async () => {
  const w=mount(CourseTutorControl,{props:{lesson:null}});
  expect(w.text()).toContain('选择课程'); expect(w.find('button').exists()).toBe(false);
  const lesson={progress:{status:'active'}} as CourseLessonView;
  await w.setProps({lesson}); await w.get('button').trigger('click');
  expect(w.emitted('action')).toEqual([[{kind:'pause'}]]);
  await w.setProps({lesson:{...lesson,progress:{...lesson.progress,status:'paused'}}});
  expect(w.text()).toContain('已暂停'); await w.get('button').trigger('click');
  expect(w.emitted('action')?.at(-1)).toEqual([{kind:'resume'}]);
  await w.setProps({goalChanged:true}); expect(w.get('button').attributes('disabled')).toBeDefined();
  await w.setProps({goalChanged:false,lesson:{...lesson,progress:{...lesson.progress,status:'completed'}}});
  expect(w.find('button').exists()).toBe(false); expect(w.text()).toContain('已完成');
  w.unmount();
});

it('shows an unconfirmed save and allows retry while normal actions are blocked', async () => {
  const w=mount(CourseTutorControl,{props:{lesson:{progress:{status:'active'}} as CourseLessonView,blocked:true,error:'保存结果待确认'}});
  expect(w.get('[role="alert"]').text()).toContain('保存结果待确认');
  expect(w.findAll('button')[0].attributes('disabled')).toBeDefined();
  await w.get('[role="alert"] button').trigger('click'); expect(w.emitted('retry')).toHaveLength(1);
  w.unmount();
});
