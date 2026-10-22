import { useState } from 'react';
import { DataState, DataTable, Pagination, useData } from '@/components/data';
import { Button } from '@/components/ui/button';
import { date, money } from '@/lib/format';
import { percentage } from '@/lib/allowance';
import { costCategories, costChargesPath, type CostFilter, type CostReport, type CostSummary } from '@/lib/costs';
import type { Charge, Page } from '@/lib/types';
import { usageQuery } from '@/lib/usage';

export function CostBreakdown({ value }: { value: CostSummary['cost_breakdown'] }) {
  return <dl className="grid gap-3 text-sm sm:grid-cols-2" aria-label="分类成本">{costCategories.map(([kind, label]) => {
    const cost = value[kind];
    return <div key={kind} aria-label={`${label}成本`}><dt className="font-medium">{label}</dt>
      <dd>已确认 {money(cost.confirmed_cost_micro_cny)} 元 · 估计 {money(cost.estimated_cost_micro_cny)} 元</dd>
      <dd>{cost.request_count} 次调用 · 费用未知 {cost.unknown_cost_requests} 次 · 待核对 {cost.reconciliation_requests} 次</dd>
      <dd>完整成本：{cost.provider_cost_complete ? `${money(cost.provider_cost_micro_cny)} 元` : '尚未完整核算'}</dd>
    </div>;
  })}</dl>;
}

export function CostMetrics({ value, platform = false }: { value: CostSummary; platform?: boolean }) {
  return <><dl className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
    <div><dt>{platform ? '平台任务已确认成本（元）' : '已确认成本（元）'}</dt><dd>{money(value.confirmed_cost_micro_cny)}</dd></div>
    <div><dt>成本估计（元）</dt><dd>{money(value.estimated_cost_micro_cny)} · {value.estimated_cost_requests} 次调用</dd></div>
    <div><dt>账号扣减（元）</dt><dd>{money(value.account_debit_micro_cny)}</dd></div>
    <div><dt>{platform ? '平台承担（元）' : '账号任务由平台承担（元）'}</dt><dd>{money(value.platform_covered_micro_cny)}</dd></div>
    <div><dt>调用数</dt><dd>{value.request_count}</dd></div>
    <div><dt>在途 / 待核算占用（元）</dt><dd>{money(value.active_reserved_micro_cny)} / {money(value.pending_micro_cny)}</dd></div>
    <div><dt>成本未知 / 待核对记录</dt><dd>{value.unknown_cost_requests} / {value.reconciliation_requests}</dd></div>
    <div><dt>完整供应商成本（元）</dt><dd>{value.provider_cost_complete ? money(value.provider_cost_micro_cny) : '尚未完整核算'}</dd></div>
  </dl><CostBreakdown value={value.cost_breakdown} />{value.period_groups.length > 0 && <details className="mt-3 text-sm"><summary className="cursor-pointer">原额度期百分比</summary>{value.period_groups.map(group => <p className="my-2 break-all" key={`${group.user_id}:${group.period_id}`}>{group.user_id} · {group.period_id} · 基准 {group.allowance_basis?.version ?? '待配置'}：已用 {percentage(group.percentages?.debited)}；预留 {percentage(group.percentages?.active_reserved)}；待核算 {percentage(group.percentages?.pending)}</p>)}</details>}</>;
}

export function CostSummaryPanel({ from, to, userId, onRefresh }: { from: string; to: string; userId: string; onRefresh?: () => void }) {
  const [offset, setOffset] = useState(0), [mode, setMode] = useState<'tasks' | 'workspaces' | 'accounts' | 'memory'>('tasks');
  const [selected, setSelected] = useState<{ title: string; filter: CostFilter } | null>(null);
  const query = useData<CostReport>(`/admin/cost-summary?${usageQuery(from, to, offset, userId)}`);
  const report = query.data;
  return <section className="space-y-5" aria-label="统一成本汇总">
    <div className="flex flex-wrap items-center justify-between gap-3"><h2>任务、工作区与平台成本</h2><Button variant="outline" onClick={() => { setSelected(null); if (onRefresh) onRefresh(); else void query.refetch(); }}>刷新费用归集</Button></div>
    <p className="text-sm text-muted-foreground">前台回答和后台整理按原父任务归集。关联多个工作区的账号级整理只计一次。未确认与待核对的费用不作为零元计入完整成本。</p>
    <DataState query={query} />
    {report && <>
      <section className="space-y-4 rounded-lg border p-5" aria-label="账号费用"><h3>账号费用</h3><CostMetrics value={report.account} /></section>
      {report.platform && <section className="space-y-4 rounded-lg border p-5" aria-label="平台准备成本"><h3>平台准备成本</h3><CostMetrics value={report.platform} platform /></section>}
      <div className="flex flex-wrap gap-2" aria-label="费用归集分组">{([['tasks','父任务'],['workspaces','工作区'],['accounts','账号'],['memory','账号级整理']] as const).map(([key,label]) => <Button key={key} variant={mode===key?'default':'outline'} aria-pressed={mode===key} onClick={() => { setMode(key); setSelected(null); }}>{label}</Button>)}</div>
      {mode === 'tasks' && <><DataTable rows={report.tasks.items} columns={[
        { header:'承担方 / 账号',cell:({row})=>row.original.cost_owner==='platform'?'平台':row.original.user_id },
        { header:'父任务',cell:({row})=><span className="block max-w-64 break-all whitespace-normal">{row.original.parent_task_ref}</span> },
        { header:'工作区 / 关联来源',cell:({row})=><span className="block max-w-64 break-all whitespace-normal">{row.original.learning_workspace_ids.join('、') || '账号级 / 平台'}{row.original.related_learning_workspace_ids.length>0 && <><br />关联：{row.original.related_learning_workspace_ids.join('、')}</>}</span> },
        { header:'已确认 / 账号扣减 / 平台承担（元）',cell:({row})=>`${money(row.original.confirmed_cost_micro_cny)} / ${money(row.original.account_debit_micro_cny)} / ${money(row.original.platform_covered_micro_cny)}` },
        { header:'费用分类',cell:({row})=><details><summary className="cursor-pointer">模型 / 搜索 / 网页读取</summary><CostBreakdown value={row.original.cost_breakdown} /></details> },
        { header:'未知 / 待核对',cell:({row})=>`${row.original.unknown_cost_requests} / ${row.original.reconciliation_requests}` },
        { header:'核对',cell:({row})=><Button variant="ghost" onClick={()=>setSelected({title:row.original.parent_task_ref,filter:{cost_owner:row.original.cost_owner,user_id:row.original.user_id??undefined,parent_task_ref:row.original.parent_task_ref}})}>查看归集明细</Button> },
      ]} /><Pagination offset={offset} total={report.tasks.total} change={value=>{setOffset(value);setSelected(null);}} /></>}
      {mode === 'workspaces' && report.workspaces.map(ws=><section key={`${ws.user_id}:${ws.learning_workspace_id}`} className="space-y-3 rounded-lg border p-5"><h3 className="break-all">{ws.workspace_goal || ws.learning_workspace_id}{ws.archived?'（已归档）':''}</h3><p className="break-all text-sm">{ws.user_id} · {ws.learning_workspace_id}</p><CostMetrics value={ws}/><Button variant="outline" onClick={()=>setSelected({title:ws.workspace_goal||ws.learning_workspace_id,filter:{cost_owner:'account',user_id:ws.user_id,learning_workspace_id:ws.learning_workspace_id}})}>查看工作区明细</Button></section>)}
      {mode === 'accounts' && report.accounts.map(account=><section key={account.user_id} className="space-y-3 rounded-lg border p-5"><h3>账号 {account.user_id}</h3><CostMetrics value={account}/><Button variant="outline" onClick={()=>setSelected({title:account.user_id,filter:{cost_owner:'account',user_id:account.user_id}})}>查看账号明细</Button></section>)}
      {mode === 'memory' && report.account_level.map(account=><section key={account.user_id} className="space-y-3 rounded-lg border p-5"><h3>账号级整理 · {account.user_id}</h3><CostMetrics value={account}/><Button variant="outline" onClick={()=>setSelected({title:`账号级整理 ${account.user_id}`,filter:{cost_owner:'account',user_id:account.user_id,account_level:true}})}>查看整理明细</Button></section>)}
      <p className="text-sm text-muted-foreground">截至 {date(report.as_of)}（香港）。百分比按各原额度期分别显示；精确成本可跨期汇总。</p>
      {selected && <CostCharges key={JSON.stringify(selected.filter)} {...selected} from={from} to={to} close={()=>setSelected(null)}/ >}
    </>}
  </section>;
}

function CostCharges({ title,filter,from,to,close }: {title:string;filter:CostFilter;from:string;to:string;close:()=>void}) {
  const [offset,setOffset]=useState(0);
  const query=useData<Page<Charge>>(costChargesPath(filter,from,to,offset));
  return <section className="space-y-3 rounded-lg border p-5" aria-label="归集调用明细"><div className="flex flex-wrap justify-between gap-2"><h3 className="break-all">{title} · 调用明细</h3><Button variant="ghost" onClick={close}>关闭归集明细</Button></div><DataState query={query}/>{query.data&&<><DataTable rows={query.data.items} columns={[
    {header:'调用',cell:({row})=><span className="block max-w-64 break-all whitespace-normal">{row.original.call_id}</span>},
    {header:'类别',cell:({row})=>costCategories.find(([kind])=>kind===(row.original.entry_kind??'model'))?.[1]},
    {header:'用途 / 状态',cell:({row})=>`${row.original.purpose} / ${row.original.state}`},
    {header:'成本（元）',cell:({row})=>`${money(row.original.provider_cost_micro_cny)}${row.original.provider_cost_status === 'estimated' ? '（估计）' : ''}`},
    {header:'账号扣减（元）',cell:({row})=>money(row.original.account_debit_micro_cny)},
    {header:'创建时间',cell:({row})=>date(row.original.created_at)},
  ]}/><Pagination offset={offset} total={query.data.total} change={setOffset}/></>}</section>;
}
