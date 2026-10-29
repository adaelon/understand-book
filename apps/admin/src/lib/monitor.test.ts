import { expect, it } from 'vitest';
import { bandwidth, bytes, trendSegments, type MonitorSample } from './monitor';

it('distinguishes missing bandwidth from idle traffic and converts bytes to bits', () => {
  expect(bandwidth(null)).toBe('待采样');
  expect(bandwidth(0)).toBe('0 bps');
  expect(bandwidth(125000)).toBe('1 Mbps');
  expect(bytes(0)).toBe('0 B');
  expect(bytes(8 * 1024 ** 3)).toBe('8 GiB');
});

it('breaks trends at missing samples and a dashboard that was closed', () => {
  const samples = [
    {sampled_at:10, memory:{used_percent:50}}, {sampled_at:15, memory:null},
    {sampled_at:20, memory:{used_percent:55}}, {sampled_at:60, memory:{used_percent:0}},
  ] as MonitorSample[];
  expect(trendSegments(samples, sample => sample.memory?.used_percent)).toEqual([
    [{at:10,value:50}], [{at:20,value:55}], [{at:60,value:0}],
  ]);
});
