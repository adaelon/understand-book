// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import CourseActivityTutor from './CourseActivityTutor.vue';
import type { CourseReview } from '../generated/CourseReview';
let intersect:(entries:unknown[])=>void;
const expression={fact_ref:'fact',job_ref:'job',activity:'reflection',response:'这个部分没懂',round_index:1,round_title:'找到升级收益的转折点'};
const review:CourseReview={expression,status:'ready',displayed:false,feedback:null,understanding:[],error:null};
beforeEach(()=>{
  vi.spyOn(document,'visibilityState','get').mockReturnValue('visible');
  vi.stubGlobal('IntersectionObserver',class{constructor(callback:typeof intersect){intersect=callback;}observe(){}disconnect(){}});
});
afterEach(()=>{vi.unstubAllGlobals();vi.restoreAllMocks();});
it('submits an adopted question as a question instead of claiming an answer',async()=>{
  const suggested='只缩短较快的一项，总时间会变吗？';
  const question={...expression,intent:'question' as const,response:'为什么要取较大值？'};
  const w=mount(CourseActivityTutor,{props:{expression:question,review:{...review,expression:question,displayed:true,feedback:{response_quote:'为什么',acknowledgement:'你可能认为较快项先完成就够了',guidance:'保留原问题',follow_up:'谁更晚完成？',question_guide:{suggested_question:suggested,why:'固定另一项进行比较',try_next:'只改变计算速度'}}},draft:'',disabled:false,presented:true}});
  expect(w.find('.acknowledgement').exists()).toBe(false);expect(w.get('blockquote').text()).toBe(question.response);
  await w.get('.question-guide button').trigger('click');expect(w.emitted('update:draft')?.at(-1)).toEqual([suggested]);
  await w.setProps({draft:suggested});await w.get('form').trigger('submit');
  expect(w.emitted('reply')).toEqual([['question',suggested]]);w.unmount();
});
it('opens a ready response only when the current activity is visible and uncovered',async()=>{
  const w=mount(CourseActivityTutor,{props:{expression,review,draft:'我的疑问',disabled:false,presented:false}});
  intersect([{isIntersecting:true}]);await flushPromises();expect(w.emitted('read')).toBeUndefined();
  await w.setProps({presented:true});expect(w.emitted('read')).toEqual([['job']]);
  await w.setProps({review:{...review,displayed:true,feedback:{response_quote:'没懂',acknowledgement:'先看两项时间。',guidance:'计算 5 ms，读取 8 ms，还得等待读取。',follow_up:'只加算力还能变快吗？'}}});
  expect(w.get('.guidance').text()).toContain('还得等待读取');expect(w.get('.next-question').text()).toContain('只加算力');
  expect(w.get('textarea').element.value).toBe('我的疑问');w.unmount();
});
it('keeps help and questions distinct and never offers completion from a feedback card',async()=>{
  const w=mount(CourseActivityTutor,{props:{expression,review:{...review,displayed:true},draft:'为什么是 8 ms？',disabled:false,presented:true}});
  await w.findAll('button').find(b=>b.text()==='这是我的疑问')!.trigger('click');
  await w.findAll('button').find(b=>b.text()==='先讲给我听')!.trigger('click');
  expect(w.emitted('reply')).toEqual([['question','为什么是 8 ms？'],['explain','我还没明白，请先把当前这一步讲清楚。']]);
  expect(w.text()).not.toContain('继续下一轮');expect(w.text()).not.toContain('完成这一课');w.unmount();
});
it('keeps the pending and failed response in the activity and allows retry',async()=>{
  const w=mount(CourseActivityTutor,{props:{expression,draft:'未提交的思路',disabled:false,presented:true}});
  expect(w.get('[role="status"]').text()).toContain('回应会直接出现在这里');
  await w.setProps({review:{...review,status:'failed'}});
  await w.findAll('button').find(b=>b.text()==='重试这次回应')!.trigger('click');
  expect(w.emitted('retry')).toEqual([['job']]);expect(w.get('textarea').element.value).toBe('未提交的思路');w.unmount();
});
it('ends an answered question without asking for a paraphrase, while leaving further questions optional',async()=>{
  const feedback={response_quote:'下界也是 10.45 ms',acknowledgement:'对，读取决定下界。',guidance:'它仍高于 9 ms 目标。',follow_up:null};
  const w=mount(CourseActivityTutor,{props:{expression,review:{...review,displayed:true,feedback},draft:'',disabled:false,presented:true,continuation:{label:'去图上验证 →',detail:'比较三种配置。'}}});
  expect(w.text()).toContain('这一步不用再回答了');expect(w.find('form').exists()).toBe(false);
  expect(w.text()).not.toContain('回应当前问题');expect(w.emitted('continue')).toBeUndefined();expect(w.emitted('reply')).toBeUndefined();
  await w.findAll('button').find(b=>b.text()==='去图上验证 →')!.trigger('click');
  expect(w.emitted('continue')).toEqual([[]]);expect(w.emitted('reply')).toBeUndefined();
  await w.findAll('button').find(b=>b.text()==='我还有疑问')!.trigger('click');
  await w.setProps({draft:'为什么下界不能当实测时间？'});await w.get('form').trigger('submit');
  expect(w.emitted('reply')).toEqual([['question','为什么下界不能当实测时间？']]);
  expect(w.text()).not.toContain('完成这一课');w.unmount();
});
it('keeps a real follow-up open and does not discard a draft when the response settles',async()=>{
  const feedback={response_quote:'没懂',acknowledgement:'先看读取。',guidance:'读取需 8 ms。',follow_up:'只加算力还能变快吗？'};
  const w=mount(CourseActivityTutor,{props:{expression,review:{...review,displayed:true,feedback},draft:'我还有个疑问',disabled:false,presented:true}});
  expect(w.text()).not.toContain('这一步不用再回答了');expect(w.get('textarea').element.value).toBe('我还有个疑问');
  await w.get('form').trigger('submit');expect(w.emitted('reply')).toEqual([['answer','我还有个疑问']]);
  await w.setProps({review:{...review,displayed:true,feedback:{...feedback,follow_up:null}}});
  expect(w.get('textarea').element.value).toBe('我还有个疑问');
  await w.get('form').trigger('submit');expect(w.emitted('reply')?.at(-1)).toEqual(['question','我还有个疑问']);w.unmount();
});

it('keeps the requested response primary and promotes continuation only once no follow-up is pending',async()=>{
  const feedback={response_quote:'没懂',acknowledgement:'先看读取。',guidance:'读取需 8 ms。',follow_up:'哪一项更晚结束？'};
  const w=mount(CourseActivityTutor,{props:{expression,review:{...review,displayed:true,feedback},draft:'读取',disabled:false,presented:true,continuation:{label:'继续下一段',detail:'接着下一个问题。'}}});
  expect(w.get('form .primary').text()).toBe('回应当前问题');
  expect(w.get('.activity-next button').classes()).not.toContain('primary');
  await w.setProps({draft:'',review:{...review,displayed:true,feedback:{...feedback,follow_up:null}}});
  expect(w.find('form').exists()).toBe(false);expect(w.get('.activity-next button').classes()).toContain('primary');
  w.unmount();
});
