// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import CourseResourceObjects from './CourseResourceObjects.vue';

const scene=vi.hoisted(()=>({draw:vi.fn(),pick:vi.fn(),dispose:vi.fn()}));
vi.mock('../course-resource-scene',()=>({createResourceScene:vi.fn(async()=>scene)}));
let intersect:(entries:{isIntersecting:boolean}[])=>void;
let resized:()=>void;
let now=0,id=0;
const frames=new Map<number,FrameRequestCallback>();
const mounted:ReturnType<typeof mount>[]=[];
async function frame(){const callbacks=[...frames.values()];frames.clear();now+=16;for(const callback of callbacks)callback(now);await nextTick();}
async function open(props:Record<string,unknown>={presented:true}){
  const wrapper=mount(CourseResourceObjects,{props});mounted.push(wrapper);
  intersect([{isIntersecting:true}]);await flushPromises();await frame();scene.draw.mockClear();return wrapper;
}
beforeEach(()=>{
  vi.clearAllMocks();frames.clear();now=0;id=0;sessionStorage.clear();
  vi.spyOn(performance,'now').mockImplementation(()=>now);
  vi.spyOn(document,'hidden','get').mockReturnValue(false);
  vi.stubGlobal('requestAnimationFrame',(cb:FrameRequestCallback)=>{frames.set(++id,cb);return id;});
  vi.stubGlobal('cancelAnimationFrame',(key:number)=>frames.delete(key));
  vi.stubGlobal('IntersectionObserver',class{constructor(cb:typeof intersect){intersect=cb;}observe(){}disconnect(){}});
  vi.stubGlobal('ResizeObserver',class{constructor(cb:typeof resized){resized=cb;}observe(){}disconnect(){}});
});
afterEach(()=>{for(const wrapper of mounted.splice(0))wrapper.unmount();vi.restoreAllMocks();vi.unstubAllGlobals();});

describe('resource scene frame scheduling',()=>{
  it('coalesces rapid input and resize into one frame with the latest view',async()=>{
    const w=await open();
    for(let i=0;i<6;i++)await w.get('canvas').trigger('keydown',{key:'ArrowRight'});
    await w.get('.view-controls input').setValue('.85');resized();resized();
    expect(scene.draw).not.toHaveBeenCalled();
    await frame();expect(scene.draw).toHaveBeenCalledTimes(1);
    expect(scene.draw.mock.calls[0][0]).toMatchObject({yaw:expect.closeTo(.9),explode:.85});
    await frame();expect(scene.draw).toHaveBeenCalledTimes(1);
  });
  it('does no rendering while hidden and refreshes the latest state when presented again',async()=>{
    const w=await open();
    await w.get('canvas').trigger('keydown',{key:'ArrowRight'});
    await w.setProps({presented:false});await frame();expect(scene.draw).not.toHaveBeenCalled();
    await w.get('.view-controls input').setValue('.7');resized();await frame();expect(scene.draw).not.toHaveBeenCalled();
    await w.setProps({presented:true});await nextTick();await frame();
    expect(scene.draw).toHaveBeenCalledTimes(1);expect(scene.draw.mock.calls[0][0]).toMatchObject({explode:.7,yaw:.3});
    scene.draw.mockClear();intersect([{isIntersecting:false}]);resized();await frame();expect(scene.draw).not.toHaveBeenCalled();
    intersect([{isIntersecting:true}]);await frame();expect(scene.draw).toHaveBeenCalledTimes(1);
  });
  it('renders a visible history model without an explicit presented prop',async()=>{
    await open({});resized();await frame();expect(scene.draw).toHaveBeenCalledTimes(1);
  });
  it('renders playback progress on every display frame',async()=>{
    const w=await open({presented:true,state:{work_gflops:960,read_gb:16,compute_tflops:60,bandwidth_tb_s:2,target_ms:9,batch_size:8}});
    await w.get('.object-time button').trigger('click');
    for(let i=0;i<6;i++){
      await frame();expect(scene.draw).toHaveBeenCalledTimes(i+1);
      expect(scene.draw.mock.calls[i][0].computeProgress).toBeCloseTo((i+1)*16/5000);
    }
  });
  it('pauses playback when backgrounded and cancels queued work on unmount',async()=>{
    const w=await open({presented:true,sceneKey:'test',state:{work_gflops:960,read_gb:16,compute_tflops:60,bandwidth_tb_s:2,target_ms:9,batch_size:8}});
    await w.get('.object-time button').trigger('click');await frame();
    vi.spyOn(document,'hidden','get').mockReturnValue(true);document.dispatchEvent(new Event('visibilitychange'));await frame();
    expect(w.get('.object-time button').text()).toContain('播放');const count=scene.draw.mock.calls.length;
    await frame();expect(scene.draw).toHaveBeenCalledTimes(count);
    vi.spyOn(document,'hidden','get').mockReturnValue(false);document.dispatchEvent(new Event('visibilitychange'));await frame();
    expect(scene.draw).toHaveBeenCalledTimes(count+1);
    await w.get('canvas').trigger('keydown',{key:'ArrowLeft'});w.unmount();
    expect(scene.dispose).toHaveBeenCalledTimes(1);expect(frames.size).toBe(0);
    expect(JSON.parse(sessionStorage.getItem('understand-book:course-objects:test')!).yaw).toBeCloseTo(.06);
  });
});
