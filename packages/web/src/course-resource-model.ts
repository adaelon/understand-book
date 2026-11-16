import type { CourseResourceState } from './generated/CourseResourceState';

export type ResourceObject = 'weights' | 'channel' | 'compute' | 'requests';
export function resourceModel(state: CourseResourceState) {
  const batch = state.batch_size ?? 1;
  const compute = state.work_gflops / state.compute_tflops;
  const read = state.read_gb / state.bandwidth_tb_s;
  const lower = Math.max(compute, read);
  return { batch, compute, read, lower, perOutput:lower / batch, throughput:1000 * batch / lower,
    workPerRequest:state.work_gflops / batch, switchBatch:read * state.compute_tflops / (state.work_gflops / batch) };
}
export function withBatch(state: CourseResourceState, batch: number): CourseResourceState {
  return { ...state, work_gflops:state.work_gflops / (state.batch_size ?? 1) * batch, batch_size:batch };
}
