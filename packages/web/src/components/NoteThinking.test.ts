// @vitest-environment happy-dom
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { installIdentity } from '../network-context';
import { pendingNoteSource } from '../personal-notes';
import { pendingUnderstandingAction, type NoteThinkingView, type UnderstandingSummary } from '../note-thinking';
import NoteThinking from './NoteThinking.vue';
let wrapper:VueWrapper;
const summary=():UnderstandingSummary=>({focus_ref:'focus-time',source_id:'book',source_revision:'v1',workspace_id:'scope',label:'全程时间包含停车时间',capability:'explanation',
  target:{learning_focus:'全程时间包含停车时间',expected_performance:'解释全程时间',capability:'explanation',source_bindings:[],object_refs:[]},
  demonstrated:[],difficulties:['把停止移动当成行程结束'],unconfirmed:['新情境中的独立判断'],evidence_ref:'e1',evidence_refs:['e1'],note_refs:[{note_id:'n',revision:1}],
  next_step:'比较停车前后的时间线',occurred_at:'2026-11-01T01:00:00Z',source_session_id:'chat',source_turn_id:'turn',assistance:'这次表达前没有记录到帮助',nature:'hypothesis',learner_quote:'停车时没有走距离，所以不用计时',correction:null});
const page=():NoteThinkingView=>({note_id:'n',current_revision:2,current_understanding:[summary()],timeline:[
  {id:'rev2',kind:'revision',reference:{note_id:'n',revision:2},text:'现在我知道，停车仍在这段行程里',based_on_turn:{session_id:'chat',turn_id:'turn'}},
  {id:'rev1',kind:'revision',reference:{note_id:'n',revision:1},text:'停车的时候没运动，这段时间应该不算吧？'},
],next_cursor:null,update_status:'ready',updates:[]});
beforeEach(()=>{installIdentity({user_id:'thinking-test',csrf_token:'token'});pendingNoteSource.value=null;pendingUnderstandingAction.value=null;});
afterEach(()=>{wrapper?.unmount();vi.unstubAllGlobals();});
const button=(label:string)=>wrapper.findAll('button').find(b=>b.text()===label)!;

it('keeps the original path and exact versions while a correction updates the shared judgment',async()=>{
  let data=page();const writes:any[]=[];
  vi.stubGlobal('fetch',vi.fn(async(url:string,init:RequestInit)=>{
    const body=JSON.parse(init.body as string);const action=url.split('/').at(-1);
    if(action==='thinking')return Response.json(data);
    if(action==='thinking_evidence')return Response.json({evidence_id:'e1',label:summary().label,learner_quote:summary().learner_quote,interpretation:'暂定理解',feedback_hidden:true,source_quotes:[{reason:'不应泄露的评分标准'}],source_session_id:'chat',source_turn_id:'turn'});
    if(action==='thinking_correct'){writes.push(body);data={...data,current_understanding:[{...summary(),evidence_ref:'e2',difficulties:[],unconfirmed:['依据补充重新确认'],correction:body.text}]};return Response.json({evidence_ref:'e2'});}
    throw new Error(action);
  }));
  wrapper=mount(NoteThinking,{props:{noteId:'n',revision:2}});await flushPromises();
  expect(wrapper.text()).toContain('把停止移动当成行程结束');expect(wrapper.text()).toContain('停车的时候没运动，这段时间应该不算吧？');
  await button('纠正判断').trigger('click');await flushPromises();
  expect(wrapper.text()).not.toContain('不应泄露的评分标准');
  await wrapper.get('textarea').setValue('我是在求证，并没有确定不算');await wrapper.get('form').trigger('submit');await flushPromises();
  expect(wrapper.text()).not.toContain('把停止移动当成行程结束');expect(wrapper.text()).toContain('依据补充重新确认');expect(wrapper.text()).toContain('停车的时候没运动，这段时间应该不算吧？');
  expect(writes[0]).toMatchObject({note_id:'n',workspace_id:'scope',evidence_ref:'e1',text:'我是在求证，并没有确定不算'});
  await wrapper.findAll('button').find(b=>b.text()==='阅读这一版 · 对照修改')!.trigger('click');
  expect(wrapper.emitted('revision')?.[0]).toEqual([2]);
});

it('carries the original scope, relationship and exact note version into a continuation',async()=>{
  vi.stubGlobal('fetch',vi.fn(async(url:string)=>Response.json(url.endsWith('/thinking')?page():{
    context:{book_id:'book',publication_id:'pub',workspace_id:'scope',session_id:'chat',turn_id:'turn'},note_ref:{note_id:'n',revision:1},source_revision:'v1',target:summary().target,
    understanding_ref:{note_id:'n',workspace_id:'scope',evidence_ref:'e1'},
  })));
  wrapper=mount(NoteThinking,{props:{noteId:'n',revision:2}});await flushPromises();await button('练一练').trigger('click');await flushPromises();
  expect(pendingUnderstandingAction.value).toMatchObject({mode:'practice',note_ref:{note_id:'n',revision:1},source_revision:'v1',understanding_ref:{evidence_ref:'e1'}});
  expect(pendingNoteSource.value).toMatchObject({book_id:'book',workspace_id:'scope',session_id:'chat',turn_id:'turn'});
});

it('saving or opening a note only reads retained understanding and explains an empty state',async()=>{
  const fetch=vi.fn(async()=>Response.json({...page(),current_understanding:[]}));vi.stubGlobal('fetch',fetch);
  wrapper=mount(NoteThinking,{props:{noteId:'n',revision:2,compact:true}});await flushPromises();
  expect(wrapper.text()).toContain('还没有相关理解观察');expect(wrapper.find('.thinking-path').exists()).toBe(false);
  expect(fetch.mock.calls).toHaveLength(1);expect(String((fetch.mock.calls[0] as unknown[])[0])).toBe('/api/me/notes/thinking');
});
