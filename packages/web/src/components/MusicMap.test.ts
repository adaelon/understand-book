// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { describe, it, expect, vi } from 'vitest';
import MusicMap from './MusicMap.vue';
import type { MusicSourceManifest } from '../../../core/src/music-contract';
const event = {event_id:'theme',title:'主题 · 长笛',start_ms:1000,end_ms:2000,listen_for:'注意长音',roles:[{part_id:'flute',role:'主题',theme_id:'a'}],concept_tags:[]};
const manifest = {recording:{duration_ms:10000},parts:[{part_id:'flute',name:'长笛'}],structure:[{section_id:'a',title:'呈示',parent_id:null,start_ms:0,end_ms:5000},{section_id:'b',title:'再现',parent_id:null,start_ms:5000,end_ms:10000}],events:[event],occurrences:[{measure:'1',start_ms:0,end_ms:5000},{measure:'2',start_ms:5000,end_ms:10000}]} as unknown as MusicSourceManifest;
describe('music map',()=>{
 it('follows actual recording time and changes section and participants without note alignment',async()=>{
  const w=mount(MusicMap,{props:{manifest,position:1500,paused:false}});
  expect(w.find('.map-now').text()).toContain('长笛');expect(w.find('.map-clock').text()).toContain('第 1 小节');
  await w.setProps({position:6000,paused:true});expect(w.find('summary strong').text()).toBe('再现');expect(w.find('.map-clock').text()).toContain('第 2 小节');expect(w.find('.map-empty').exists()).toBe(true);expect(w.find('.map-now').text()).not.toContain('长笛');await w.setProps({position:10000});expect(w.find('summary strong').text()).toBe('再现');w.unmount();
 });
 it('uses the selected source section and event for mouse and keyboard navigation',async()=>{
  const w=mount(MusicMap,{props:{manifest,position:1500,paused:true}});
  await w.findAll('.map-overview button')[1].trigger('click');expect(w.emitted('seek')?.[0]).toEqual([5000]);
  await w.find('.map-lane g').trigger('click');expect(w.emitted('listen')?.[0]).toEqual([event]);w.unmount();
 });
});

it('uses timed children as numbered dock sections and keeps the referenced exercise', async () => {
 const exercise={...event,event_id:'exercise',title:'听一次长音',explanation:'留意长音',evidence:[],relations:[],concept_tags:['导听练习']};
 const full={...manifest,structure:[{...manifest.structure[0],section_id:'whole',title:'全曲',parent_id:null,start_ms:0,end_ms:10000},...manifest.structure.map(s=>({...s,parent_id:'whole'}))],events:[exercise]};
 const w=mount(MusicMap,{props:{manifest:full,position:1500,paused:true,docked:true}});
 try {
  expect(w.find('details').exists()).toBe(false);
  expect(w.findAll('.dock-sections button').map(b=>b.text())).toEqual(['01','02']);
  await w.findAll('.dock-sections button')[1].trigger('pointerenter');
  expect(w.find('.dock-title').text()).toBe('再现');
  await w.findAll('.dock-sections button')[1].trigger('click');
  expect(w.emitted('seek')?.[0]).toEqual([5000]);
  await w.find('.dock-events button').trigger('click');
  expect(w.emitted('listen')?.[0]).toEqual([exercise]);
  await w.find('.dock-guide').trigger('click');
  expect(w.emitted('guide')).toHaveLength(1);
 } finally {w.unmount();}
});

it('starts compact readers on the score and returns to it after choosing a material', async () => {
 vi.stubGlobal('matchMedia', () => ({matches:true,addEventListener(){},removeEventListener(){}}));
 const w=mount(MusicMap,{props:{manifest,position:1500,paused:true}});
 try {
  const details=w.find('details');expect(details.element.open).toBe(false);
  details.element.open=true;await details.trigger('toggle');
  expect(w.find('.map-return').text()).toBe('返回乐谱');
  await w.find('.map-lane g').trigger('click');
  expect(w.emitted('listen')?.[0]).toEqual([event]);expect(details.element.open).toBe(false);
 } finally {w.unmount();vi.unstubAllGlobals();}
});
