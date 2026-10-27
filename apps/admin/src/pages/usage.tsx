import { useEffect, useRef, useState } from 'react';
import { Link } from '@tanstack/react-router';
import { DataState, DataTable, Pagination, useData } from '@/components/data';
import { Field } from '@/components/operation';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { date, money } from '@/lib/format';
import { hongKongDay, usageQuery, taskChargesPath } from '@/lib/usage';
import type { Activity, Charge, Page, UsageReport, UsageSummary, UsageTask } from '@/lib/types';
import { CostSummaryPanel } from './cost-summary';
import { queryClient } from '@/lib/api';
import { Amount, Disclosure, Distribution, Metric, StatusBadge, ViewSwitch } from '@/components/analytics';

export function ActivitySummary({ activity }: { activity: Activity }) {
  return <section className="analytics-panel">
    <h2>使用与回访</h2>
    <dl className="grid gap-3 sm:grid-cols-3">
      <Metric label="活跃天数" value={activity.active_days} detail="有阅读或提问记录的日期" />
      <Metric label="回访天数" value={activity.return_days} detail="跨日再次回来使用" tone="green" />
      <Metric label="已完成任务" value={activity.completed_runs} detail="成功保存并完成交付" tone="violet" />
    </dl>
    <dl className="number-detail text-sm"><div><dt>最近阅读（香港）</dt><dd>{activity.last_read_at == null ? '暂无记录' : date(activity.last_read_at)}</dd></div><div><dt>最近提问（香港）</dt><dd>{activity.last_question_at == null ? '暂无记录' : date(activity.last_question_at)}</dd></div></dl>
    <p className="text-xs text-muted-foreground">自记录启用起统计，日期按香港时间。</p>
  </section>;
}

function Summary({ title, value }: { title: string; value: UsageSummary }) {
  return <section className="analytics-panel" aria-label={title}>
    <h2>{title}</h2>
    <dl className="metric-grid">
      <Metric label="活跃账号" value={value.active_users} detail="期间内有阅读或提问" />
      <Metric label="回访账号" value={value.returning_users} detail="此前另一日已使用过" tone="green" />
      <Metric label="已完成任务" value={value.completed_runs} detail="按成功保存时间统计" tone="violet" />
      <Metric label="收款净额" value={<Amount value={value.receipt_net_fen} digits={2} />} detail={`原额 ${money(value.receipt_original_fen, 2)} 元 · 更正 ${money(value.receipt_correction_fen, 2)} 元`} tone="muted" />
    </dl>
    <Distribution label="活跃账号构成" segments={[
      {label:'回访账号',value:value.returning_users,tone:'green'},
      {label:'首次活跃账号',value:Math.max(0,value.active_users-value.returning_users),tone:'blue'},
    ]} note={value.active_users ? '按所选期间内的活跃账号统计。' : '所选期间暂无活跃记录。'} />
    <Disclosure title="模型调用账目与精确金额"><dl className="number-detail">
      <div><dt>已确认模型成本（元）</dt><dd>{money(value.confirmed_cost_micro_cny)}</dd></div>
      <div><dt>账号扣减（元）</dt><dd>{money(value.account_debit_micro_cny)}</dd></div>
      <div><dt>在途预留（元）</dt><dd>{money(value.active_reserved_micro_cny)} · {value.active_requests} 条</dd></div>
      <div><dt>待核算占用（元）</dt><dd>{money(value.pending_micro_cny)} · {value.pending_requests} 条</dd></div>
      <div><dt>调用记录</dt><dd>{value.request_count}</dd></div><div><dt>成本尚未确认</dt><dd>{value.unknown_cost_requests} 条</dd></div>
    </dl></Disclosure>
  </section>;
}

export function UsagePage({ userId }: { userId?: string }) {
  const [filter, setFilter] = useState(() => ({ from: hongKongDay(), to: hongKongDay(), user: userId ?? '' }));
  const [offset, setOffset] = useState(0), [error, setError] = useState('');
  const [task, setTask] = useState<UsageTask | null>(null);
  const [view, setView] = useState<'costs' | 'activity' | 'ledger'>('costs');
  const path = userId ? `/admin/users/${encodeURIComponent(userId)}/usage` : '/admin/usage';
  const query = useData<UsageReport>(`${path}?${usageQuery(filter.from, filter.to, offset, userId ? '' : filter.user)}`);
  const result = query.data;
  function refreshReports() {
    setTask(null);
    void queryClient.invalidateQueries({ predicate: ({ queryKey }) => typeof queryKey[1] === 'string' &&
      (queryKey[1].startsWith(`${path}?`) || queryKey[1].startsWith('/admin/cost-summary?') || queryKey[1].startsWith('/admin/charges?')) });
  }
  return <section className="space-y-5">
    <div className="flex flex-wrap items-center justify-between gap-3"><h1>{userId ? '账号用量汇总' : '全站用量'}</h1><Button variant="outline" onClick={refreshReports}>刷新汇总</Button></div>
    <p className="text-sm text-muted-foreground">查看费用构成、账号活跃与收款，按任务追踪每笔调用。</p>
    <form className="filter-bar usage-filter" key={`${filter.from}-${filter.to}-${filter.user}`} onSubmit={(e) => {
      e.preventDefault(); const data = new FormData(e.currentTarget);
      const next = { from: String(data.get('from')), to: String(data.get('to')), user: userId ?? String(data.get('user') ?? '').trim() };
      try { usageQuery(next.from, next.to); setFilter(next); setOffset(0); setTask(null); setError(''); } catch (err) { setError((err as Error).message); }
    }}>
      <Field label="开始日期"><Input type="date" name="from" defaultValue={filter.from} required /></Field>
      <Field label="结束日期"><Input type="date" name="to" defaultValue={filter.to} required /></Field>
      {!userId && <Field label="账号（留空为全部）"><Input name="user" defaultValue={filter.user} /></Field>}
      <Button type="submit" variant="outline" className="mb-2">查询期间</Button>
      <div className="mb-2 flex flex-wrap gap-1">{([[1,'今天'],[7,'近 7 天'],[30,'近 30 天']] as const).map(([days,label]) => <Button key={days} type="button" variant="ghost" onClick={() => {setFilter({from:hongKongDay(Date.now()-(days-1)*86400000),to:hongKongDay(),user:filter.user});setOffset(0);setTask(null);setError('');}}>{label}</Button>)}</div>
    </form>
    {error && <p role="alert">{error}</p>}
    <div className="flex flex-wrap items-center justify-between gap-3"><ViewSwitch label="用量视图" value={view} options={[["costs","费用概览"],["activity","使用与收款"],["ledger","任务账单"]]} onChange={setView}/><span className="text-xs text-muted-foreground">香港时间 · 包含起止两日</span></div>
    <div hidden={view !== 'costs'}><CostSummaryPanel key={`costs-${filter.from}-${filter.to}-${filter.user}`} from={filter.from} to={filter.to} userId={filter.user} onRefresh={refreshReports} /></div>
    {view !== 'costs' && <DataState query={query} />}
    {result && <>
      <div hidden={view !== 'activity'} className="space-y-4">
      <Summary title={`所选期间 · ${result.from} 至 ${result.to}${filter.user ? ` · ${filter.user}` : ''}`} value={result.summary} />
      <Disclosure title="查看今日快照"><Summary title={`今日 · ${result.today_date}${filter.user ? ` · ${filter.user}` : ''}`} value={result.today} /></Disclosure>
      <p className="text-xs text-muted-foreground">截至 {date(result.as_of)}（香港）。费用按调用创建日归组，收款按付款日归组并包含后续更正。回访账号此前另一香港自然日已有使用记录。</p>
      </div>
      <div hidden={view !== 'ledger'} className="space-y-4">
      <section className="analytics-panel"><h2>任务费用 · 按已确认成本排序</h2>
        <p className="text-sm text-muted-foreground">每个任务可包含多次调用；点击调用明细核对各笔费用。</p>
        <DataTable rows={result.tasks.items} columns={[
          { header: '账号', cell: ({row}) => row.original.cost_owner === 'platform' ? '平台' : <Link to="/users/$userId" params={{userId:row.original.user_id}} className="underline">{row.original.user_id}</Link> },
          { header: '任务', cell: ({row}) => <span className="block min-w-48 max-w-64 break-all whitespace-normal">{row.original.reference_kind === 'run' ? '运行' : row.original.reference_kind === 'operator_task' ? '平台任务' : '后台任务'} · {row.original.reference}</span> },
          { header: '调用数', accessorKey: 'request_count' },
          { header: '已确认成本（元）', cell: ({row}) => money(row.original.confirmed_cost_micro_cny) },
          { header: '核算状态', cell: ({row}) => <div className="space-y-2"><StatusBadge tone={row.original.unknown_cost_requests || row.original.pending_requests ? 'amber' : 'green'}>{row.original.unknown_cost_requests || row.original.pending_requests ? `未知 ${row.original.unknown_cost_requests} · 待核算 ${row.original.pending_requests}` : '已核算'}</StatusBadge><details><summary>金额明细</summary><dl className="space-y-2 py-2 text-xs"><div><dt>账号扣减（元）</dt><dd>{money(row.original.account_debit_micro_cny)}</dd></div><div><dt>在途预留（元）</dt><dd>{money(row.original.active_reserved_micro_cny)}</dd></div><div><dt>待核算占用（元）</dt><dd>{money(row.original.pending_micro_cny)}</dd></div></dl></details></div> },
          { header: '核对', cell: ({row}) => <Button variant="ghost" onClick={() => setTask(row.original)}>调用明细</Button> },
        ]} />
        <Pagination offset={offset} total={result.tasks.total} change={(v) => {setOffset(v);setTask(null);}} />
      </section>
      {task && <TaskCharges key={`${task.user_id}-${task.reference_kind}-${task.reference}-${result.from}-${result.to}`} task={task} from={result.from} to={result.to} close={() => setTask(null)} />}
      </div>
    </>}
  </section>;
}
function TaskCharges({ task, from, to, close }: { task: UsageTask; from: string; to: string; close: () => void }) {
  const [offset,setOffset]=useState(0);
  const query=useData<Page<Charge>>(taskChargesPath(task,from,to,offset));
  const anchor=useRef<HTMLElement>(null);
  useEffect(()=>{if(query.data){anchor.current?.focus({preventScroll:true});anchor.current?.scrollIntoView?.({block:'start'});}},[!!query.data]);
  return <section ref={anchor} tabIndex={-1} className="analytics-panel outline-none" aria-label="任务调用明细">
    <div className="flex items-center justify-between gap-3"><h2 className="break-all">调用明细 · {task.reference}</h2><Button variant="ghost" onClick={close}>关闭明细</Button></div>
    <DataState query={query} />
    {query.data && <><DataTable rows={query.data.items} columns={[
      { header:'调用',cell:({row})=><span className="block min-w-48 max-w-64 break-all whitespace-normal">{row.original.call_id}</span> },
      { header:'时间（香港）',cell:({row})=>date(row.original.created_at) },
      { header:'模型 / 目的',cell:({row})=>`${row.original.model} / ${row.original.purpose}` },
      { header:'状态',cell:({row})=>({reserved:'已预占',sent:'发送中',pending:'待核算',settled:'已结算',released:'已释放'}[row.original.state] ?? row.original.state) },
      { header:'模型成本（元）',cell:({row})=>money(row.original.provider_cost_micro_cny) },
      { header:'账号扣减（元）',cell:({row})=>money(row.original.account_debit_micro_cny) },
      { header:'当前占用（元）',cell:({row})=>money(['reserved','sent','pending'].includes(row.original.state)?row.original.reserved_micro_cny:0) },
    ]} /><Pagination offset={offset} total={query.data.total} change={setOffset} /></>}
    {task.cost_owner === 'account' && <Link to="/users/$userId" params={{userId:task.user_id}} className="text-sm underline">前往账号详情核算费用</Link>}
  </section>;
}
