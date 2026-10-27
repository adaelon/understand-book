import { useEffect, useRef, useState } from 'react';
import { DataState, DataTable, Pagination, useData } from '@/components/data';
import { Amount, chartColors, Disclosure, EmptyState, Metric, Notice, StatusBadge, ViewSwitch } from '@/components/analytics';
import { Button } from '@/components/ui/button';
import { date, money } from '@/lib/format';
import { percentage } from '@/lib/allowance';
import { costCategories, costChargesPath, type CostFilter, type CostReport, type CostSummary } from '@/lib/costs';
import type { Charge, Page } from '@/lib/types';
import { usageQuery } from '@/lib/usage';

const categoryTones = ['blue', 'green', 'violet', 'amber'] as const;
export function CostBreakdown({ value }: { value: CostSummary['cost_breakdown'] }) {
  const total = costCategories.reduce((sum, [kind]) => sum + value[kind].confirmed_cost_micro_cny, 0);
  const max = Math.max(0, ...costCategories.map(([kind]) => value[kind].confirmed_cost_micro_cny));
  return <figure className="space-y-4" aria-label="分类成本">
    <figcaption className="flex flex-wrap items-baseline justify-between gap-2"><h3 className="font-semibold">费用花在哪里</h3><span className="text-xs text-muted-foreground">按已确认成本比较 · 元</span></figcaption>
    <div className="space-y-4">{costCategories.map(([kind, label], index) => {
      const cost = value[kind];
      const share = total > 0 ? cost.confirmed_cost_micro_cny / total * 100 : 0;
      return <div key={kind} aria-label={`${label}成本`}>
        <div className="mb-2 flex flex-wrap items-center justify-between gap-2 text-sm">
          <span className="flex items-center gap-2"><span className="size-2 rounded-full" style={{ background: chartColors[categoryTones[index]] }} />{label}<span className="text-xs text-muted-foreground">{cost.request_count ? `${cost.request_count} 次调用` : '暂无调用'}</span></span>
          <span className="flex items-center gap-3"><span className="text-xs text-muted-foreground">{total > 0 ? `${share < 1 && share > 0 ? '<1' : share.toFixed(0)}%` : '—'}</span><Amount value={cost.confirmed_cost_micro_cny} /></span>
        </div>
        <div className="cost-bar-track" role="img" aria-label={`${label}已确认成本 ${money(cost.confirmed_cost_micro_cny)} 元，占已确认成本 ${share.toFixed(1)}%`}><span style={{ width: `${max > 0 ? cost.confirmed_cost_micro_cny / max * 100 : 0}%`, background: chartColors[categoryTones[index]] }} /></div>
        {(cost.unknown_cost_requests > 0 || cost.reconciliation_requests > 0 || cost.estimated_cost_requests > 0) && <div className="mt-2 flex flex-wrap gap-2">
          {cost.unknown_cost_requests > 0 && <StatusBadge tone="amber">费用未知 {cost.unknown_cost_requests} 次</StatusBadge>}
          {cost.reconciliation_requests > 0 && <StatusBadge tone="amber">待核对 {cost.reconciliation_requests} 次</StatusBadge>}
          {cost.estimated_cost_requests > 0 && <StatusBadge tone="violet">估计 {money(cost.estimated_cost_micro_cny)} 元</StatusBadge>}
        </div>}
        <details className="mt-1 text-xs text-muted-foreground"><summary className="w-fit cursor-pointer py-1">{label}精确金额与核算状态</summary><div className="space-y-1 py-2">
          <p>已确认 {money(cost.confirmed_cost_micro_cny)} 元 · 估计 {money(cost.estimated_cost_micro_cny)} 元</p>
          <p>{cost.request_count} 次调用 · 费用未知 {cost.unknown_cost_requests} 次 · 待核对 {cost.reconciliation_requests} 次</p>
          <p>完整成本：{cost.provider_cost_complete ? `${money(cost.provider_cost_micro_cny)} 元` : '尚未完整核算'}</p>
        </div></details>
      </div>;
    })}</div>
    {total === 0 && <p className="text-xs text-muted-foreground">{costCategories.some(([kind]) => value[kind].request_count > 0) ? '本期暂无已确认成本，请查看各项核算状态。' : '本期暂无调用，发生费用后会显示构成。'}</p>}
  </figure>;
}

export function CostMetrics({ value, platform = false }: { value: CostSummary; platform?: boolean }) {
  const attention = value.unknown_cost_requests > 0 || value.reconciliation_requests > 0 || value.pending_requests > 0;
  return <div className="space-y-5">
    <dl className="metric-grid">
      <Metric label={platform ? '平台任务已确认成本' : '已确认成本'} value={<Amount value={value.confirmed_cost_micro_cny} />} detail="供应商已确认金额" />
      <Metric label="账号扣减" value={<Amount value={value.account_debit_micro_cny} />} detail="记入账号使用额度" tone="green" />
      <Metric label={platform ? '平台承担' : '账号任务由平台承担'} value={<Amount value={value.platform_covered_micro_cny} />} detail="由平台支付的部分" tone="violet" />
      <Metric label="服务调用" value={value.request_count.toLocaleString('zh-CN')} detail="模型、搜索、网页与其他服务" tone="muted" />
    </dl>
    <div className="grid gap-6 lg:grid-cols-[1.25fr_1fr]">
      <CostBreakdown value={value.cost_breakdown} />
      <div className="space-y-4 rounded-xl bg-muted/20 p-4 sm:p-5">
        <h3 className="font-semibold">核算状态</h3>
        <Notice warning={attention}>{attention ? <>需关注：费用未知 {value.unknown_cost_requests} 次 · 待核对 {value.reconciliation_requests} 次 · 待核算 {value.pending_requests} 次</> : value.provider_cost_complete ? '费用已完整核算' : '完整成本尚待确认'}</Notice>
        <dl className="number-detail text-sm">
          <div><dt>成本估计</dt><dd><Amount value={value.estimated_cost_micro_cny} /> · {value.estimated_cost_requests} 次</dd></div>
          <div><dt>待核算占用</dt><dd><Amount value={value.pending_micro_cny} /></dd></div>
          <div><dt>在途预留</dt><dd><Amount value={value.active_reserved_micro_cny} /></dd></div>
          <div><dt>完整供应商成本</dt><dd>{value.provider_cost_complete ? <Amount value={value.provider_cost_micro_cny} /> : '尚未完整核算'}</dd></div>
        </dl>
        <p className="text-xs leading-relaxed text-muted-foreground">估计、预留与待核算占用分别列示，未确认费用不会作为零元计入完整成本。</p>
      </div>
    </div>
    <Disclosure title="精确金额与原额度期比例">
      <dl className="number-detail text-sm">
        <div><dt>已确认成本（元）</dt><dd>{money(value.confirmed_cost_micro_cny)}</dd></div>
        <div><dt>成本估计（元）</dt><dd>{money(value.estimated_cost_micro_cny)}</dd></div>
        <div><dt>账号扣减（元）</dt><dd>{money(value.account_debit_micro_cny)}</dd></div>
        <div><dt>平台承担（元）</dt><dd>{money(value.platform_covered_micro_cny)}</dd></div>
        <div><dt>在途预留（元）</dt><dd>{money(value.active_reserved_micro_cny)}</dd></div>
        <div><dt>待核算占用（元）</dt><dd>{money(value.pending_micro_cny)}</dd></div>
        <div><dt>完整供应商成本（元）</dt><dd>{value.provider_cost_complete ? money(value.provider_cost_micro_cny) : '尚未完整核算'}</dd></div>
      </dl>
      {value.period_groups.length > 0 && <><h4 className="font-medium">原额度期百分比</h4><DataTable rows={value.period_groups} columns={[
        {header:'账号 / 额度期',cell:({row})=><span className="block max-w-64 whitespace-normal break-all">{row.original.user_id}<small className="block text-muted-foreground">{row.original.period_id} · {row.original.allowance_basis?.version ?? '待配置'}</small></span>},
        {header:'已用',cell:({row})=>percentage(row.original.percentages?.debited)},
        {header:'预留',cell:({row})=>percentage(row.original.percentages?.active_reserved)},
        {header:'待核算',cell:({row})=>percentage(row.original.percentages?.pending)},
      ]}/></>}
    </Disclosure>
  </div>;
}

export function CostSummaryPanel({ from, to, userId, onRefresh }: { from: string; to: string; userId: string; onRefresh?: () => void }) {
  const [offset, setOffset] = useState(0), [mode, setMode] = useState<'tasks' | 'workspaces' | 'accounts' | 'memory'>('tasks');
  const [selected, setSelected] = useState<{ title: string; filter: CostFilter } | null>(null);
  const query = useData<CostReport>(`/admin/cost-summary?${usageQuery(from, to, offset, userId)}`);
  const report = query.data;
  const groups = !report || mode === 'tasks' ? [] : mode === 'workspaces' ? report.workspaces.map(ws => ({ value: ws, title: ws.workspace_goal || ws.learning_workspace_id, subtitle: `${ws.user_id}${ws.archived ? ' · 已归档' : ''}`, action: '查看工作区明细', filter: { cost_owner: 'account', user_id: ws.user_id, learning_workspace_id: ws.learning_workspace_id } as CostFilter })) :
    (mode === 'accounts' ? report.accounts : report.account_level).map(account => ({ value: account, title: account.user_id, subtitle: mode === 'memory' ? '账号级整理' : '账号费用', action: mode === 'memory' ? '查看整理明细' : '查看账号明细', filter: { cost_owner: 'account', user_id: account.user_id, ...(mode === 'memory' ? { account_level: true } : {}) } as CostFilter }));
  const taskMax = Math.max(0, ...(report?.tasks.items.map(task => task.confirmed_cost_micro_cny) ?? []));
  return <section className="space-y-5" aria-label="统一成本汇总">
    <DataState query={query} />
    {report && <>
      <section className="analytics-panel" aria-label="账号费用">
        <div className="flex flex-wrap items-center justify-between gap-3"><div><h2>账号费用</h2><p className="mt-1 text-xs text-muted-foreground">{from === to ? from : `${from} 至 ${to}`} · {userId || '全部账号'}</p></div><Button variant="outline" onClick={() => { if (onRefresh) onRefresh(); else void query.refetch(); }}>刷新费用归集</Button></div>
        <CostMetrics value={report.account} />
      </section>
      {report.platform && <section className="analytics-panel" aria-label="平台准备成本">{(report.platform.unknown_cost_requests>0||report.platform.reconciliation_requests>0)&&<Notice warning>平台费用需关注：未知 {report.platform.unknown_cost_requests} 次 · 待核对 {report.platform.reconciliation_requests} 次</Notice>}<Disclosure title={`平台准备成本 · 已确认 ¥ ${money(report.platform.confirmed_cost_micro_cny)} · ${report.platform.request_count} 次调用`}><CostMetrics value={report.platform} platform /></Disclosure></section>}
      <section className="analytics-panel">
        <div className="flex flex-wrap items-end justify-between gap-3"><div><h2>费用归集</h2><p className="mt-1 text-xs text-muted-foreground">按父任务归集回答与后台整理，账号级整理只计一次。</p></div><ViewSwitch label="费用归集分组" value={mode} options={[["tasks","父任务"],["workspaces","工作区"],["accounts","账号"],["memory","账号级整理"]]} onChange={key => { setMode(key); setSelected(null); }} /></div>
        {selected && <CostCharges key={JSON.stringify(selected.filter)} {...selected} from={from} to={to} close={() => setSelected(null)} />}
        {mode === 'tasks' ? <><p className="text-xs text-muted-foreground">条形长度比较本页任务的已确认成本</p><DataTable rows={report.tasks.items} columns={[
          { header: '任务 / 账号', cell: ({row}) => <div className="max-w-64 whitespace-normal"><span className="block break-all font-medium">{row.original.parent_task_ref}</span><span className="text-xs text-muted-foreground">{row.original.cost_owner === 'platform' ? '平台任务' : row.original.user_id}</span></div> },
          { header: '已确认成本（元）', cell: ({row}) => <div className="w-36 max-w-full space-y-2"><strong className="font-semibold">{money(row.original.confirmed_cost_micro_cny)}</strong><div className="cost-bar-track" aria-hidden="true"><span style={{width:`${taskMax > 0 ? row.original.confirmed_cost_micro_cny/taskMax*100 : 0}%`,background:chartColors.blue}} /></div></div> },
          { header: '核算状态', cell: ({row}) => <CostStatus value={row.original} /> },
          { header: '归集信息', cell: ({row}) => <details><summary>费用与工作区</summary><dl className="my-3 space-y-2 text-xs"><div><dt>账号扣减 / 平台承担（元）</dt><dd>{money(row.original.account_debit_micro_cny)} / {money(row.original.platform_covered_micro_cny)}</dd></div><div><dt>工作区</dt><dd className="break-all">{row.original.learning_workspace_ids.join('、') || '账号级 / 平台'}</dd></div>{row.original.related_learning_workspace_ids.length > 0 && <div><dt>关联来源</dt><dd className="break-all">{row.original.related_learning_workspace_ids.join('、')}</dd></div>}</dl><CostBreakdown value={row.original.cost_breakdown} /></details> },
          { header: '核对', cell: ({row}) => <Button variant="outline" onClick={() => setSelected({ title: row.original.parent_task_ref, filter: { cost_owner: row.original.cost_owner, user_id: row.original.user_id ?? undefined, parent_task_ref: row.original.parent_task_ref } })}>查看归集明细</Button> },
        ]}/><Pagination offset={offset} total={report.tasks.total} change={value => { setOffset(value); setSelected(null); }} /></> : groups.length > 0 ? <DataTable rows={groups} columns={[
          { header: mode === 'workspaces' ? '工作区 / 账号' : '账号', cell: ({row}) => <div className="max-w-72 whitespace-normal break-words"><strong>{row.original.title}</strong><p className="text-xs text-muted-foreground">{row.original.subtitle}</p></div> },
          { header: '已确认成本（元）', cell: ({row}) => money(row.original.value.confirmed_cost_micro_cny) },
          { header: '调用数', cell: ({row}) => row.original.value.request_count },
          { header: '核算状态', cell: ({row}) => <CostStatus value={row.original.value} /> },
          { header: '费用构成', cell: ({row}) => <GroupCosts value={row.original.value} /> },
          { header: '核对', cell: ({row}) => <Button variant="outline" onClick={() => setSelected({title:row.original.title,filter:row.original.filter})}>{row.original.action}</Button> },
        ]}/> : <EmptyState>所选期间暂无{mode === 'workspaces' ? '工作区费用' : mode === 'accounts' ? '账号费用' : '账号级整理费用'}。</EmptyState>}
      </section>
      <p className="text-xs text-muted-foreground">更新于 {date(report.as_of)}（香港）。概览金额保留两位小数，展开可核对精确金额；原额度期比例分别计算。</p>
    </>}
  </section>;
}

function CostStatus({value}:{value:CostSummary}) {
  return value.unknown_cost_requests > 0 || value.reconciliation_requests > 0 ? <div className="space-y-1"><StatusBadge tone="amber">需核对</StatusBadge><p className="text-xs text-muted-foreground">未知 {value.unknown_cost_requests} · 待核对 {value.reconciliation_requests}</p></div> : value.estimated_cost_requests > 0 ? <StatusBadge tone="violet">含估计费用</StatusBadge> : value.active_requests > 0 || value.pending_requests > 0 ? <StatusBadge tone="blue">核算中</StatusBadge> : <StatusBadge tone="green">已核算</StatusBadge>;
}

function GroupCosts({value}:{value:CostSummary}) {
  return <details><summary>展开费用概况</summary><div className="min-w-48 space-y-4 py-4"><dl className="space-y-2 text-xs">
    <div><dt>账号扣减 / 平台承担（元）</dt><dd>{money(value.account_debit_micro_cny)} / {money(value.platform_covered_micro_cny)}</dd></div>
    <div><dt>在途预留 / 待核算占用（元）</dt><dd>{money(value.active_reserved_micro_cny)} / {money(value.pending_micro_cny)}</dd></div>
    <div><dt>成本估计（元）</dt><dd>{money(value.estimated_cost_micro_cny)}</dd></div>
    <div><dt>完整供应商成本（元）</dt><dd>{value.provider_cost_complete ? money(value.provider_cost_micro_cny) : '尚未完整核算'}</dd></div>
  </dl><CostBreakdown value={value.cost_breakdown}/>{value.period_groups.map(group=><p className="break-all text-xs text-muted-foreground" key={`${group.user_id}:${group.period_id}`}>{group.user_id} · {group.period_id} · 基准 {group.allowance_basis?.version??'待配置'}：已用 {percentage(group.percentages?.debited)}；预留 {percentage(group.percentages?.active_reserved)}；待核算 {percentage(group.percentages?.pending)}</p>)}</div></details>;
}

function CostCharges({ title, filter, from, to, close }: { title: string; filter: CostFilter; from: string; to: string; close: () => void }) {
  const [offset,setOffset] = useState(0);
  const anchor = useRef<HTMLElement>(null);
  const query = useData<Page<Charge>>(costChargesPath(filter,from,to,offset));
  useEffect(() => { if(query.data){anchor.current?.focus({preventScroll:true}); anchor.current?.scrollIntoView?.({block:'start'});} }, [!!query.data]);
  return <section ref={anchor} tabIndex={-1} className="space-y-3 rounded-xl border border-blue-500/30 bg-muted/20 p-4 outline-none" aria-label="归集调用明细"><div className="flex flex-wrap justify-between gap-2"><h3 className="break-all font-semibold">{title} · 调用明细</h3><Button variant="ghost" onClick={close}>关闭归集明细</Button></div><DataState query={query}/>{query.data&&<><DataTable rows={query.data.items} columns={[
    {header:'调用',cell:({row})=><span className="block max-w-64 break-all whitespace-normal">{row.original.call_id}</span>},
    {header:'类别 / 用途',cell:({row})=><><span>{costCategories.find(([kind])=>kind===(row.original.entry_kind??'model'))?.[1]}</span><p className="text-xs text-muted-foreground">{row.original.purpose}</p></>},
    {header:'状态',cell:({row})=>({reserved:'已预占',sent:'发送中',pending:'待核算',settled:'已结算',released:'已释放'}[row.original.state]??row.original.state)},
    {header:'成本（元）',cell:({row})=>`${money(row.original.provider_cost_micro_cny)}${row.original.provider_cost_status==='estimated'?'（估计）':''}`},
    {header:'账号扣减（元）',cell:({row})=>money(row.original.account_debit_micro_cny)},
    {header:'创建时间',cell:({row})=>date(row.original.created_at)},
  ]}/><Pagination offset={offset} total={query.data.total} change={setOffset}/></>}</section>;
}
