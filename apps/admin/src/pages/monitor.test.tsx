// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, expect, it, vi } from 'vitest';
import { session } from '@/lib/api';
import type { MonitorReport } from '@/lib/monitor';
import { MonitorPage } from './monitor';

// These cases exercise the monitor sampler; the operations panel has its own query.
vi.mock('./operations-panel', () => ({ OperationsPanel: () => null }));

let root: Root | undefined, host: HTMLDivElement;
let client: QueryClient;
afterEach(async () => {
  if (root) { await act(async () => root!.unmount()); root = undefined; host.remove(); }
  client?.clear(); vi.restoreAllMocks(); vi.useRealTimers();
});

function fixture(): MonitorReport {
  const current = {
    sampled_at: Math.floor(Date.now() / 1000), service_uptime_seconds: 90000,
    memory:{total_bytes:8*1024**3,used_bytes:4*1024**3,available_bytes:4*1024**3,used_percent:50},
    disk:{total_bytes:100*1024**3,used_bytes:20*1024**3,available_bytes:75*1024**3,used_percent:20},
    network:{interfaces:['eth0'],receive_bytes_per_second:125000,transmit_bytes_per_second:0},
    activity:{online_users:38,active_runs:4,queued_runs:2,active_run_limit:12,today_turns:128,today_completed_turns:119},
    issues:[],
  };
  return {current,history:[current],interval_seconds:5,history_seconds:600,online_window_seconds:90,timezone:'Asia/Hong_Kong',platform:'linux'};
}
async function settle(ms = 25) { await act(async () => { await vi.advanceTimersByTimeAsync(ms); }); }
async function mount() {
  (globalThis as typeof globalThis & {IS_REACT_ACT_ENVIRONMENT:boolean}).IS_REACT_ACT_ENVIRONMENT = true;
  host=document.createElement('div');document.body.append(host);root=createRoot(host);
  client=new QueryClient({defaultOptions:{queries:{retry:false}}});
  await act(async () => root!.render(<QueryClientProvider client={client}><MonitorPage /></QueryClientProvider>));
  await settle();
}
async function click(text: string) { await act(async () => [...host.querySelectorAll('button')].find(button => button.textContent === text)!.click()); await settle(); }

it('refreshes every five seconds, pauses, manually refreshes and resumes', async () => {
  vi.useFakeTimers();
  const read=vi.spyOn(session,'request').mockImplementation(async () => fixture());
  await mount();
  expect(host.textContent).toContain('38');
  expect(host.textContent).toContain('排队 2 轮');
  expect(host.textContent).toContain('1 Mbps');
  expect(read).toHaveBeenCalledTimes(1);
  await settle(5000); expect(read).toHaveBeenCalledTimes(2);
  await click('暂停更新');
  await settle(16000); expect(read).toHaveBeenCalledTimes(2);
  expect(host.textContent).toContain('自动更新已暂停');
  await click('立即刷新'); expect(read).toHaveBeenCalledTimes(3);
  await click('恢复更新'); expect(read).toHaveBeenCalledTimes(4);
  await settle(5000); expect(read).toHaveBeenCalledTimes(5);
});

it('keeps the last sample visible on failure and recovers without showing missing values as zero', async () => {
  vi.useFakeTimers();
  const data=fixture(); data.current.memory=null; data.current.network!.receive_bytes_per_second=null;
  data.current.issues=['暂时无法读取服务器内存。'];
  const read=vi.spyOn(session,'request').mockResolvedValue(data);
  await mount();
  expect(host.querySelector('[aria-label="服务器内存"]')!.textContent).toContain('暂不可用');
  expect(host.querySelector('[aria-label="实时带宽"]')!.textContent).toContain('待采样');
  read.mockRejectedValueOnce(new Error('连接不可用'));
  await settle(5025);
  expect(host.querySelector('[role="alert"]')!.textContent).toContain('保留上次采样');
  expect(host.textContent).toContain('38');
  await settle(5025);
  expect(host.querySelector('[role="alert"]')).toBeNull();
  expect(host.textContent).toContain('实时更新');
});
