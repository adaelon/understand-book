// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import AccountCostSummary from './AccountCostSummary.vue';
import { installIdentity, network } from '../network-context';

const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => { wrappers.splice(0).forEach(w => w.unmount()); vi.unstubAllGlobals(); network.value = { ...network.value, identity: null, enabled: false }; });
const pct=(value:string)=>({value,nonzero:value!=='0',below_tenth:false});
const periods=[{period_id:'old-period',allowance_basis:{version:'old-basis',basis_micro_cny:1000000},percentages:{debited:pct('2.3'),active_reserved:pct('0'),pending:pct('0')},unknown_provider_cost_count:0},{period_id:'new-period',allowance_basis:{version:'new-basis',basis_micro_cny:2000000},percentages:{debited:pct('5'),active_reserved:pct('1'),pending:pct('2')},unknown_provider_cost_count:1}];
const summary={request_count:3,unknown_cost_requests:1,pending_requests:1,period_groups:periods};
const report={as_of:1792281600,account:summary,platform:{provider_cost_micro_cny:999999},workspaces:[{...summary,learning_workspace_id:'ws / &',workspace_goal:'期末复习'}],account_level:[{...summary,request_count:1}],tasks:{items:[{...summary,cost_owner:'account',user_id:'A',parent_task_ref:'task / &',latest_at:1792281600,learning_workspace_ids:['ws / &'],related_learning_workspace_ids:['w1','w2'],purposes:['answer','memory']}],total:21,limit:20,offset:0}};
const button=(w:ReturnType<typeof mount>,text:string)=>w.findAll('button').find(b=>b.text()===text)!;
function start() { installIdentity({user_id:'A',csrf_token:'a'});const w=mount(AccountCostSummary);wrappers.push(w);return w; }
it('keeps original period percentages separate and drills parent task, workspace and account-level scope',async()=>{
  const fetcher=vi.fn(async(url:string)=>Response.json(url.includes('cost-summary')?report:{items:[{call_id:'background-call',purpose:'memory',state:'settled',account_debit_micro_cny:23000,period_id:'old-period',allowance_basis:periods[0].allowance_basis,allowance_percentages:{debited:pct('2.3')},created_at:1792281600}],total:1,limit:20,offset:0}));
  vi.stubGlobal('fetch',fetcher);const w=start();await flushPromises();
  expect(w.text()).toContain('2.3%');expect(w.text()).toContain('5%');expect(w.text()).not.toContain('7.3%');expect(w.text()).not.toContain('999999');expect(w.text()).toContain('1 次费用尚未确认');
  await button(w,'按任务').trigger('click');await button(w,'查看调用明细').trigger('click');await flushPromises();
  expect(w.text()).toContain('background-call');expect(w.text()).toContain('关联仅说明来源，不追加扣减');
  let params=new URL(fetcher.mock.calls.at(-1)![0],'https://reader.test').searchParams;expect(params.get('parent_task_ref')).toBe('task / &');
  await button(w,'按工作区').trigger('click');expect(w.text()).toContain('期末复习');await button(w,'查看调用明细').trigger('click');await flushPromises();
  params=new URL(fetcher.mock.calls.at(-1)![0],'https://reader.test').searchParams;expect(params.get('learning_workspace_id')).toBe('ws / &');expect(params.has('parent_task_ref')).toBe(false);
  await button(w,'账号级整理').trigger('click');await button(w,'查看调用明细').trigger('click');await flushPromises();
  params=new URL(fetcher.mock.calls.at(-1)![0],'https://reader.test').searchParams;expect(params.get('account_level')).toBe('true');
  await button(w,'按任务').trigger('click');await button(w,'下一页任务').trigger('click');await flushPromises();expect(fetcher.mock.calls.at(-1)![0]).toContain('offset=20');
});
it('clears old account data and ignores an earlier account response',async()=>{
  const pending:((response:Response)=>void)[]=[];vi.stubGlobal('fetch',vi.fn(()=>new Promise<Response>(resolve=>pending.push(resolve))));
  const w=start();installIdentity({user_id:'B',csrf_token:'b'});pending[0](Response.json(report));await flushPromises();expect(w.text()).not.toContain('2.3%');
  pending[1](Response.json({...report,account:{...summary,request_count:0,period_groups:[]},tasks:{items:[],total:0,limit:20,offset:0},workspaces:[],account_level:[]}));await flushPromises();expect(w.text()).toContain('暂无用量记录');expect(w.text()).not.toContain('old-period');
});

