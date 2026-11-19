// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import CourseDecisionTutor from './CourseDecisionTutor.vue';
import type { CourseDecisionView } from '../generated/CourseDecisionView';
import type { CourseReview } from '../generated/CourseReview';
let visible:(entries:unknown[])=>void;
const decision:CourseDecisionView={phase:'decision',latest_job:'job',question:'比较两种升级，并说明为什么。',can_continue:false,focus_title:null,support:null,attention:null};
const review:CourseReview={expression:{fact_ref:'fact',job_ref:'job',activity:'task',response:'还是要实测啊'},status:'ready',displayed:false,feedback:null,understanding:[],error:null};
beforeEach(()=>{
  vi.spyOn(document,'visibilityState','get').mockReturnValue('visible');
  vi.stubGlobal('IntersectionObserver',class { constructor(callback:typeof visible){visible=callback;} observe(){} disconnect(){} });
});
afterEach(()=>{vi.unstubAllGlobals();vi.restoreAllMocks();});
it('keeps an adopted question out of decision-answer evidence',async()=>{
  const suggested='下界低于目标时，还缺什么依据？';
  const w=mount(CourseDecisionTutor,{props:{decision,reviews:[{...review,expression:{...review.expression,intent:'question'},displayed:true,feedback:{response_quote:'实测',acknowledgement:'你可能把下界当成实际耗时',guidance:'接着这个疑问探索',follow_up:null,question_guide:{suggested_question:suggested,why:'明确还需要验证的部分',try_next:'对比一次实际运行'}}}],draft:'',disabled:false}});
  expect(w.find('.acknowledgement').exists()).toBe(false);expect(w.get('.guidance').text()).toBe('接着这个疑问探索');
  await w.get('.question-guide button').trigger('click');await w.setProps({draft:suggested});await w.get('form').trigger('submit');
  expect(w.emitted('reply')).toEqual([['question',suggested]]);w.unmount();
});
it('delivers ready feedback when its current teaching card becomes visible',async()=>{
  const w=mount(CourseDecisionTutor,{props:{decision,reviews:[review],draft:'我的草稿',disabled:false}});
  expect(w.emitted('read')).toBeUndefined();
  visible([{isIntersecting:true}]);await flushPromises();
  expect(w.emitted('read')).toEqual([['job']]);
  expect((w.get('textarea').element as HTMLTextAreaElement).value).toBe('我的草稿');
  await w.setProps({reviews:[{...review,displayed:true,feedback:{response_quote:'实测',acknowledgement:'这是必要的一点。',guidance:'现在比较两项时间。',follow_up:'读取缩短后，还要等多久？'}}]});
  expect(w.get('.next-question').text()).toContain('还要等多久');
  expect(w.text()).not.toContain('完成这一课');w.unmount();
});
it('keeps a learner question distinct from an answer and provides a direct explanation request',async()=>{
  const w=mount(CourseDecisionTutor,{props:{decision,reviews:[{...review,displayed:true}],draft:'为什么取 max？',disabled:false}});
  await w.findAll('button').find(b=>b.text()==='这是我的疑问')!.trigger('click');
  await w.findAll('button').find(b=>b.text()==='先讲给我听')!.trigger('click');
  expect(w.emitted('reply')).toEqual([['question','为什么取 max？'],['explain','我还没明白，请先把当前这一步讲清楚。']]);w.unmount();
});
it('opens a new application before completion and does not present a second completion action in history',async()=>{
  const w=mount(CourseDecisionTutor,{props:{decision:{...decision,can_continue:true},reviews:[{...review,displayed:true}],draft:'',disabled:false}});
  expect(w.text()).toContain('换个情境试试');expect(w.text()).not.toContain('完成这一课');
  await w.setProps({decision:{...decision,phase:'transfer',can_continue:true}});
  expect(w.text()).toContain('完成这一课');
  await w.setProps({finished:true});expect(w.text()).not.toContain('完成这一课 ✓');expect(w.find('textarea').exists()).toBe(false);w.unmount();
});

it('shows why reused practice still needs course work and follows the original continuation decision',async()=>{
  const reused={...review,expression:{...review.expression,closed_book_result_ref:'closed-result'},displayed:true,feedback:{response_quote:'实测',acknowledgement:'你区分了下界与实测。',guidance:'还需要作出具体决策。',follow_up:'你会选哪种升级？',course_reuse:{accepted:false,reason:'仍需按本题预算作出选择。'}}};
  const w=mount(CourseDecisionTutor,{props:{decision,reviews:[reused],draft:'',disabled:false}});
  expect(w.get('.reuse-result').text()).toContain('仍需按本题预算作出选择。');expect(w.find('form').exists()).toBe(true);
  expect(w.text()).not.toContain('换个情境试试 →');
  await w.setProps({decision:{...decision,can_continue:true},reviews:[{...reused,feedback:{...reused.feedback,follow_up:null,course_reuse:{accepted:true,reason:'本题要求已满足。'}}}]});
  expect(w.find('form').exists()).toBe(false);expect(w.get('.ready').text()).toContain('换个情境试试');
  w.unmount();
});
