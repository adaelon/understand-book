// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, expect, it, vi } from 'vitest';
import { CostSummaryPanel } from './cost-summary';
const read=vi.hoisted(()=>vi.fn());
vi.mock('@/components/data',async(original)=>({...await original<typeof import('@/components/data')>(),useData:read}));
let root:Root|undefined;let host:HTMLDivElement;
afterEach(async()=>{if(root){await act(async()=>root!.unmount());root=undefined;host.remove();}read.mockReset();});
const summary={request_count:2,confirmed_cost_micro_cny:700000,account_debit_micro_cny:600000,platform_covered_micro_cny:100000,active_reserved_micro_cny:0,active_requests:0,pending_micro_cny:30000,pending_requests:1,unknown_cost_requests:1,reconciliation_requests:0,provider_cost_complete:false,provider_cost_micro_cny:null,period_groups:[]};
const task={...summary,cost_owner:'account',user_id:'A',parent_task_ref:'main / &',learning_workspace_ids:['w1'],related_learning_workspace_ids:['w1','w2'],purposes:['answer','memory']};
async function click(label:string){await act(async()=>[...host.querySelectorAll('button')].find(b=>b.textContent===label)!.click());}
it('separates platform cost, unknown total, and parent/workspace/account-level drill filters',async()=>{
  (globalThis as typeof globalThis & {IS_REACT_ACT_ENVIRONMENT:boolean}).IS_REACT_ACT_ENVIRONMENT=true;
  read.mockImplementation((path:string)=>({isPending:false,error:null,refetch:vi.fn(),data:path.startsWith('/admin/cost-summary')?{as_of:1792281600,account:summary,platform:{...summary,account_debit_micro_cny:0,confirmed_cost_micro_cny:200000,platform_covered_micro_cny:200000},accounts:[{...summary,user_id:'A'}],workspaces:[{...summary,user_id:'A',learning_workspace_id:'w1',workspace_goal:'考试准备',archived:false}],account_level:[{...summary,user_id:'A'}],tasks:{items:[task],total:1,limit:20,offset:0}}:{items:[],total:0,limit:20,offset:0}}));
  host=document.createElement('div');document.body.append(host);root=createRoot(host);await act(async()=>root!.render(<CostSummaryPanel from="2026-10-17" to="2026-10-18" userId=""/>));
  expect(host.textContent).toContain('尚未完整核算');expect(host.textContent).toContain('平台任务已确认成本');expect(host.textContent).toContain('账号任务由平台承担');
  await click('查看归集明细');let params=new URL(read.mock.calls.at(-1)![0],'https://reader.test').searchParams;expect(params.get('parent_task_ref')).toBe('main / &');expect(params.get('user_id')).toBe('A');
  await click('工作区');await click('查看工作区明细');params=new URL(read.mock.calls.at(-1)![0],'https://reader.test').searchParams;expect(params.get('learning_workspace_id')).toBe('w1');expect(params.has('parent_task_ref')).toBe(false);
  await click('账号级整理');await click('查看整理明细');params=new URL(read.mock.calls.at(-1)![0],'https://reader.test').searchParams;expect(params.get('account_level')).toBe('true');expect(params.get('from')).toBe('2026-10-17');
});
