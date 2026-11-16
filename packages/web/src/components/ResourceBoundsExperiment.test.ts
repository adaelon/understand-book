// @vitest-environment happy-dom
import {mount} from '@vue/test-utils';
import {it,expect} from 'vitest';
import ResourceBoundsExperiment from './ResourceBoundsExperiment.vue';

it('keeps batch work, resource times and a shared visible scale consistent',async()=>{
  const w=mount(ResourceBoundsExperiment,{props:{experiment:{kind:'resource_bounds',instruction:'比较批量',work_gflops:120,read_gb:16,compute_tflops:60,bandwidth_tb_s:2,batch_size:1},preset:'batch_one',observed:[],disabled:false}});
  expect(w.get('.givens').text()).toContain('120 GFLOPs');expect(w.get('.lower').text()).toContain('8 ms');
  await w.setProps({preset:'batch_eight'});
  expect(w.get('.givens').text()).toContain('960 GFLOPs');expect(w.get('.lower').text()).toContain('16 ms');
  expect(w.get('.bar.compute').attributes('style')).toContain('100%');expect(w.get('.bar.read').attributes('style')).toContain('50%');
  expect(w.get('.axis').text()).toContain('16 ms');w.unmount();
});
