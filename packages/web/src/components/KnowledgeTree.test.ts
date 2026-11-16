// @vitest-environment happy-dom
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import KnowledgeTree from './KnowledgeTree.vue';
import { api } from '../api';
import type { KnowledgeTreeView, KnowledgeNodeView } from '../knowledge-tree';
vi.mock('../network-context',async importOriginal=>({...await importOriginal<typeof import('../network-context')>(),bindSceneApi:(value:unknown)=>value,readerKey:()=> 'test-reader'}));
const node=(id:string,label:string,patch:Partial<KnowledgeNodeView>={}):KnowledgeNodeView=>({id,label,parent_id:'root',kind:'claim',summary:'',
  reference:{source_id:'book',source_revision:'v1',publication_id:null,artifact:'pass1',entry_id:id},sources:[{kind:'text',lid:'1.1'}],relations:[],child_count:0,fruit_count:0,...patch});
const root=node('root','运动',{parent_id:null,kind:'material',child_count:2,fruit_count:1});
const first=node('time','总时间包含停车',{fruit_count:1}),second=node('distance','总路程包含往返');
const view=(n=root):KnowledgeTreeView=>({node:n,children:n.id==='root'?[first,second]:[],breadcrumbs:[{id:'root',label:'运动'},...(n.id==='root'?[]:[{id:n.id,label:n.label}])],
  next:null,total:n.id==='root'?2:0,fruits:[],current_ids:[],modality:'text'});
const wrappers:VueWrapper[]=[];
function mountTree(props:Record<string,unknown>={}){const w=mount(KnowledgeTree,{props:{sourceId:'book',...props},global:{stubs:{UnderstandingSpace:true}}});wrappers.push(w);return w;}
afterEach(()=>{wrappers.splice(0).forEach(w=>w.unmount());localStorage.clear();vi.restoreAllMocks();});
it('opens a precise branch, emits its actual source and keeps an unrelated claim without fruit',async()=>{
  const read=vi.spyOn(api,'knowledgeTree').mockImplementation(async r=>view(r?.node_id==='time'?first:root));
  const listener=vi.fn();window.addEventListener('knowledge-open-source',listener);
  const w=mountTree();await flushPromises();
  expect(w.findAll('.understanding-fruit')).toHaveLength(1);
  expect(w.findAll('.branch-content')[1].text()).toContain('总路程包含往返');
  await w.get('.branch-content').trigger('click');await flushPromises();
  expect(read.mock.lastCall?.[0]).toMatchObject({node_id:'time'});
  await w.get('.knowledge-actions .source-action').trigger('click');
  expect(listener.mock.calls[0][0].detail.source).toEqual({kind:'text',lid:'1.1'});
  window.removeEventListener('knowledge-open-source',listener);
});
it('keeps the current branch after a failed read and lets the same request retry',async()=>{
  const read=vi.spyOn(api,'knowledgeTree').mockResolvedValueOnce(view()).mockRejectedValueOnce(new Error('连接暂时断开')).mockResolvedValueOnce(view(first));
  const w=mountTree();await flushPromises();await w.get('.branch-content').trigger('click');await flushPromises();
  expect(w.get('[role="alert"]').text()).toContain('连接暂时断开');expect(w.findAll('.branch-content')).toHaveLength(2);
  await w.get('[role="alert"] button').trigger('click');await flushPromises();
  expect(read.mock.lastCall?.[0]?.node_id).toBe('time');expect(w.get('.detail-title h3').text()).toBe(first.label);
});
it('ignores a delayed response from the branch already left',async()=>{
  let finish:(value:KnowledgeTreeView)=>void=()=>{};
  vi.spyOn(api,'knowledgeTree').mockResolvedValueOnce(view()).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;})).mockResolvedValueOnce(view(second));
  const w=mountTree();await flushPromises();
  await w.findAll('.branch-content')[0].trigger('click');await w.findAll('.branch-content')[1].trigger('click');await flushPromises();
  finish(view(first));await flushPromises();expect(w.get('.detail-title h3').text()).toBe(second.label);
});
it('uses the current playback position when locating and preserves the exact repeated music source',async()=>{
  const source={kind:'music' as const,target:{publication:{book_id:'book',publication_id:'performance-a'},audio:{recording:{recording_id:'original',revision:'v1'},start_ms:2000,end_ms:3000},score:{score:{score_id:'score',revision:'v1'},source_ids:['note-a']},occurrence_ids:['second-entrance'],event_ids:['return'],provenance_ids:['listening']}};
  const musical=node('return','主题再次出现',{sources:[source],kind:'music_event'});
  const read=vi.spyOn(api,'knowledgeTree').mockImplementation(async r=>({...view(r?.node_id==='return'||!r?.node_id&&r?.position_ms===2500?musical:root),modality:'music',current_ids:['root',r?.position_ms===2500?'return':'opening']}));
  const listener=vi.fn();window.addEventListener('knowledge-open-source',listener);
  const w=mountTree({positionMs:500});await flushPromises();await w.setProps({positionMs:2500});await flushPromises();
  expect(read).toHaveBeenCalledTimes(1);
  await w.findAll('button').find(b=>b.text().includes('我在这里'))!.trigger('click');await flushPromises();
  expect(read.mock.lastCall?.[0]).toMatchObject({position_ms:2500});
  await w.get('.knowledge-actions .source-action').trigger('click');expect(listener.mock.calls[0][0].detail.source).toEqual(source);
  window.removeEventListener('knowledge-open-source',listener);
});

it('opens at the reading location and pins that branch for later refreshes',async()=>{
  localStorage.setItem('knowledge-tree:local:test-reader:book','old-unrelated-location');
  const read=vi.spyOn(api,'knowledgeTree').mockResolvedValue(view(first));
  const w=mountTree({currentLid:'1.1'});await flushPromises();
  expect(read.mock.calls[0][0]).toMatchObject({current_lid:'1.1'});
  expect(read.mock.calls[0][0]?.node_id).toBeUndefined();
  await w.setProps({currentLid:'2.1'});
  window.dispatchEvent(new Event('tutor-state-changed'));await flushPromises();
  expect(read.mock.lastCall?.[0]).toMatchObject({node_id:'time',current_lid:'2.1'});
  expect(w.find('.living-tree').exists()).toBe(false);
});
it('shows an existing introduction and one explicit start before the branches',async()=>{
  const read=vi.spyOn(api,'knowledgeTree').mockResolvedValueOnce({...view(),entry_point:first,introduction:{text:'解释总时间的范围。',is_opening:true}}).mockResolvedValue(view(first));
  const w=mountTree();await flushPromises();
  expect(w.get('.knowledge-heading').text()).toContain('开篇线索');
  expect(w.get('.knowledge-summary').text()).toBe('解释总时间的范围。');
  expect(w.get('.starting-point').text()).toContain(first.label);
  expect(w.find('.tree-search').exists()).toBe(false);
  await w.get('.starting-point button').trigger('click');await flushPromises();
  expect(read.mock.lastCall?.[0]?.node_id).toBe('time');
  expect(w.find('.living-tree').exists()).toBe(false);
  expect(w.get('.knowledge-actions .source-action').text()).toContain('打开原文');
});
