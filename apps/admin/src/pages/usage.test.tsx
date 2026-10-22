// @vitest-environment happy-dom
import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, expect, it, vi } from 'vitest';
import { UsagePage } from './usage';
import { QueryClientProvider } from '@tanstack/react-query';
import { queryClient } from '@/lib/api';
import type { UsageMetrics } from '@/lib/types';

const read = vi.hoisted(() => vi.fn());
vi.mock('@/components/data', async (original) => ({
  ...await original<typeof import('@/components/data')>(),
  useData: read,
}));
vi.mock('@tanstack/react-router', () => ({
  Link: ({ children, params }: { children: ReactNode; params: { userId: string } }) => <a href={`/users/${params.userId}`}>{children}</a>,
}));

let root: Root | undefined;
let host: HTMLDivElement;
function costBreakdown(metrics: UsageMetrics) {
  const empty={request_count:0,confirmed_cost_micro_cny:0,estimated_cost_micro_cny:0,estimated_cost_requests:0,account_debit_micro_cny:0,platform_covered_micro_cny:0,active_reserved_micro_cny:0,active_requests:0,pending_micro_cny:0,pending_requests:0,unknown_cost_requests:0,reconciliation_requests:0,provider_cost_complete:true,provider_cost_micro_cny:0};
  return {model:{...empty,...metrics,provider_cost_micro_cny:metrics.confirmed_cost_micro_cny},service:empty,web_search:empty,web_fetch:empty};
}
afterEach(async () => {
  if (root) { await act(async () => root!.unmount()); root = undefined; host.remove(); }
  read.mockReset();
  queryClient.clear();
  vi.unstubAllGlobals();
});

it('refreshes both report snapshots from either refresh action after a late settlement', async () => {
  (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  const actual = await vi.importActual<typeof import('@/components/data')>('@/components/data');
  read.mockImplementation(actual.useData);
  let confirmed = 85000;
  const fetcher = vi.fn(async (url: string) => {
    const metrics = { request_count:1,confirmed_cost_micro_cny:confirmed,account_debit_micro_cny:confirmed,active_reserved_micro_cny:0,active_requests:0,pending_micro_cny:0,pending_requests:0,unknown_cost_requests:0 };
    const tasks = {items:[],total:0,limit:20,offset:0};
    if (url.startsWith('/api/admin/cost-summary?')) return Response.json({as_of:1792281600,account:{...metrics,estimated_cost_micro_cny:0,estimated_cost_requests:0,platform_covered_micro_cny:0,reconciliation_requests:0,provider_cost_complete:true,provider_cost_micro_cny:confirmed,period_groups:[],cost_breakdown:costBreakdown(metrics)},platform:null,accounts:[],workspaces:[],account_level:[],tasks:{...tasks,total:1,items:[{...metrics,platform_covered_micro_cny:0,reconciliation_requests:0,cost_breakdown:costBreakdown(metrics),cost_owner:'account',user_id:'A',parent_task_ref:'lesson',learning_workspace_ids:['workspace'],related_learning_workspace_ids:[],purposes:['answer']}]}});
    if (url.startsWith('/api/admin/charges?')) return Response.json({items:[{call_id:'late-call',purpose:'answer',state:'settled',provider_cost_micro_cny:confirmed,account_debit_micro_cny:confirmed,created_at:1792281600}],total:1,limit:20,offset:0});
    if (url.startsWith('/api/admin/usage?')) {
      const summary={...metrics,receipt_original_fen:0,receipt_correction_fen:0,receipt_net_fen:0,active_users:0,returning_users:0,completed_runs:0};
      return Response.json({from:'2026-10-18',to:'2026-10-18',today_date:'2026-10-18',as_of:1792281600,today:summary,summary,tasks});
    }
    throw new Error(`Unexpected request ${url}`);
  });
  vi.stubGlobal('fetch',fetcher);
  host=document.createElement('div');document.body.append(host);root=createRoot(host);
  await act(async()=>root!.render(<QueryClientProvider client={queryClient}><UsagePage/></QueryClientProvider>));
  const settle=()=>act(async()=>{await new Promise(resolve=>setTimeout(resolve,25));});
  await settle();
  const amounts=()=>[
    host.querySelector('[aria-label="账号费用"]')?.textContent,
    host.querySelector('[aria-label^="今日"]')?.textContent,
    host.querySelector('[aria-label^="所选期间"]')?.textContent,
  ];
  expect(amounts().every(value=>value?.includes('0.085'))).toBe(true);
  for (const [label,cost,display] of [['刷新费用归集',92000,'0.092'],['刷新汇总',99000,'0.099']] as const) {
    if (label === '刷新汇总') {
      await act(async()=>[...host.querySelectorAll('button')].find(button=>button.textContent==='查看归集明细')!.click());
      await settle();
      expect(host.querySelector('[aria-label="归集调用明细"]')?.textContent).toContain('0.092');
    }
    confirmed=cost;
    await act(async()=>[...host.querySelectorAll('button')].find(button=>button.textContent===label)!.click());
    await settle();
    expect(amounts().every(value=>value?.includes(display))).toBe(true);
    expect(amounts().some(value=>value?.includes('0.085'))).toBe(false);
    if (label === '刷新汇总') {
      expect(host.querySelector('[aria-label="归集调用明细"]')?.textContent).toContain('0.099');
      expect(host.querySelector('[aria-label="归集调用明细"]')?.textContent).not.toContain('0.092');
    }
  }
  expect(fetcher.mock.calls.filter(([url])=>url.startsWith('/api/admin/usage?'))).toHaveLength(3);
  expect(fetcher.mock.calls.filter(([url])=>url.startsWith('/api/admin/cost-summary?'))).toHaveLength(3);
});

it('renders a platform task and opens its charges without an account link', async () => {
  (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  const metrics = { request_count:1,confirmed_cost_micro_cny:5000,account_debit_micro_cny:0,active_reserved_micro_cny:0,active_requests:0,pending_micro_cny:0,pending_requests:0,unknown_cost_requests:0 };
  const summary = { ...metrics,receipt_original_fen:0,receipt_correction_fen:0,receipt_net_fen:0,active_users:0,returning_users:0,completed_runs:0 };
  read.mockImplementation((path: string) => ({
    isPending:false,error:null,refetch:vi.fn(),
    data:path.startsWith('/admin/cost-summary?') ? {as_of:1792281600,account:{...metrics,estimated_cost_micro_cny:0,estimated_cost_requests:0,platform_covered_micro_cny:0,reconciliation_requests:0,provider_cost_complete:true,provider_cost_micro_cny:5000,period_groups:[],cost_breakdown:costBreakdown(metrics)},platform:null,accounts:[],workspaces:[],account_level:[],tasks:{items:[],total:0,limit:20,offset:0}} : path.startsWith('/admin/usage?') ? {
      from:'2026-10-18',to:'2026-10-18',today_date:'2026-10-18',as_of:1792281600,today:summary,summary,
      tasks:{items:[{...metrics,user_id:null,cost_owner:'platform',reference_kind:'operator_task',reference:'public-build'}],total:1,limit:20,offset:0},
    } : {items:[{call_id:'platform-call',state:'settled',model:'fixture',purpose:'prebuild',created_at:1792281600,provider_cost_micro_cny:5000,account_debit_micro_cny:0}],total:1,limit:20,offset:0},
  }));
  host = document.createElement('div'); document.body.append(host); root = createRoot(host);
  await act(async () => root!.render(<UsagePage />));
  expect(host.textContent).toContain('平台任务');
  expect(host.querySelectorAll('a')).toHaveLength(0);
  const details = [...host.querySelectorAll('button')].find(button => button.textContent === '调用明细');
  expect(details).toBeDefined();
  await act(async () => details!.click());
  expect(host.textContent).toContain('platform-call');
  expect(host.textContent).not.toContain('前往账号详情核算费用');
  expect(host.querySelectorAll('a')).toHaveLength(0);
  const request = read.mock.calls.map(([path]) => String(path)).find(path => path.startsWith('/admin/charges?'));
  expect(request).toBeDefined();
  const params = new URL(request!, 'https://reader.example').searchParams;
  expect(params.get('operator_task_ref')).toBe('public-build');
  expect(params.has('user_id')).toBe(false);
});
