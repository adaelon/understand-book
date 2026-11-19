import { expect, it } from 'vitest';
import { resourceModel, withBatch } from './course-resource-model';
it('separates the shared read, batch completion, amortized time and throughput',()=>{
  const one={work_gflops:140,read_gb:70,compute_tflops:989.4,bandwidth_tb_s:3.35,target_ms:9,batch_size:1};
  const eight=withBatch(one,8),model=resourceModel(eight);
  expect(eight.read_gb).toBe(70);expect(eight.work_gflops).toBe(1120);
  expect(model.read).toBeCloseTo(20.8955224);expect(model.compute).toBeCloseTo(1.132);
  expect(model.lower).toBe(model.read);expect(model.perOutput).toBeCloseTo(2.61194);expect(model.throughput).toBeCloseTo(382.857);
  expect(withBatch(eight,1)).toEqual(one);
});
it('shows the same throughput but different batch time on both sides of a compute limit',()=>{
  const one={work_gflops:120,read_gb:16,compute_tflops:60,bandwidth_tb_s:2,target_ms:9,batch_size:1};
  const four=resourceModel(withBatch(one,4)),eight=resourceModel(withBatch(one,8));
  expect(four.switchBatch).toBe(4);expect(four.lower).toBe(8);expect(eight.lower).toBe(16);
  expect(four.throughput).toBe(500);expect(eight.throughput).toBe(500);expect(eight.perOutput).toBe(2);
});
