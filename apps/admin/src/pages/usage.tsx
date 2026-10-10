import { useState } from 'react';
import { Link } from '@tanstack/react-router';
import { DataState, DataTable, Pagination, useData } from '@/components/data';
import { Field } from '@/components/operation';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { date, money } from '@/lib/format';
import { hongKongDay, usageQuery, taskChargesPath } from '@/lib/usage';
import type { Activity, Charge, Page, UsageReport, UsageSummary, UsageTask } from '@/lib/types';

export function ActivitySummary({ activity }: { activity: Activity }) {
  return <section className="space-y-3 rounded-lg border p-5">
    <h2>使用与回访</h2>
    <dl className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      <div><dt>最近阅读（香港）</dt><dd>{activity.last_read_at == null ? '暂无记录' : date(activity.last_read_at)}</dd></div>
      <div><dt>最近提问（香港）</dt><dd>{activity.last_question_at == null ? '暂无记录' : date(activity.last_question_at)}</dd></div>
      <div><dt>活跃 / 回访天数</dt><dd>{activity.active_days} / {activity.return_days}</dd></div>
      <div><dt>已完成任务</dt><dd>{activity.completed_runs}</dd></div>
    </dl>
    <p className="text-sm text-muted-foreground">自记录启用起统计。跨香港自然日再次阅读或提问计为回访；完成任务按保存成功并满足交付判定的运行计数。</p>
  </section>;
}

function Summary({ title, value }: { title: string; value: UsageSummary }) {
  return <section className="space-y-4 rounded-lg border p-5" aria-label={title}>
    <h2>{title}</h2>
    <dl className="grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
      <Metric label="已确认模型成本（元）" value={money(value.confirmed_cost_micro_cny)} />
      <Metric label="账号扣减（元）" value={money(value.account_debit_micro_cny)} />
      <Metric label="在途预留（元）" value={money(value.active_reserved_micro_cny)} detail={`${value.active_requests} 条在途调用`} />
      <Metric label="待核算占用（元）" value={money(value.pending_micro_cny)} detail={`${value.pending_requests} 条待核算；含免扣但成本未知及冲突记录`} />
      <Metric label="调用记录" value={String(value.request_count)} detail={`${value.unknown_cost_requests} 条成本尚未确认`} />
      <Metric label="收款净额（元）" value={money(value.receipt_net_fen, 2)} detail={`原额 ${money(value.receipt_original_fen, 2)}，更正 ${money(value.receipt_correction_fen, 2)}`} />
      <Metric label="活跃 / 回访账号" value={`${value.active_users} / ${value.returning_users}`} />
      <Metric label="已完成任务" value={String(value.completed_runs)} />
    </dl>
  </section>;
}
function Metric({ label, value, detail }: { label: string; value: string; detail?: string }) {
  return <div><dt className="text-sm text-muted-foreground">{label}</dt><dd className="mt-1 text-xl font-semibold tabular-nums">{value}</dd>{detail && <p className="mt-1 text-xs text-muted-foreground">{detail}</p>}</div>;
}

export function UsagePage({ userId }: { userId?: string }) {
  const [filter, setFilter] = useState(() => ({ from: hongKongDay(), to: hongKongDay(), user: userId ?? '' }));
  const [offset, setOffset] = useState(0), [error, setError] = useState('');
  const [task, setTask] = useState<UsageTask | null>(null);
  const path = userId ? `/admin/users/${encodeURIComponent(userId)}/usage` : '/admin/usage';
  const query = useData<UsageReport>(`${path}?${usageQuery(filter.from, filter.to, offset, userId ? '' : filter.user)}`);
  const result = query.data;
  return <section className="space-y-5">
    <div className="flex flex-wrap items-center justify-between gap-3"><h1>{userId ? '账号用量汇总' : '全站用量'}</h1><Button variant="outline" onClick={() => { setTask(null); void query.refetch(); }}>刷新汇总</Button></div>
    <p className="text-sm text-muted-foreground">日期按香港时间，包含起止两日。金额分别记账；预留与待核算占用不是最终费用。</p>
    <form className="flex flex-wrap items-end gap-3" key={`${filter.from}-${filter.to}-${filter.user}`} onSubmit={(e) => {
      e.preventDefault(); const data = new FormData(e.currentTarget);
      const next = { from: String(data.get('from')), to: String(data.get('to')), user: userId ?? String(data.get('user') ?? '').trim() };
      try { usageQuery(next.from, next.to); setFilter(next); setOffset(0); setTask(null); setError(''); } catch (err) { setError((err as Error).message); }
    }}>
      <Field label="开始日期"><Input type="date" name="from" defaultValue={filter.from} required /></Field>
      <Field label="结束日期"><Input type="date" name="to" defaultValue={filter.to} required /></Field>
      {!userId && <Field label="账号（留空为全部）"><Input name="user" defaultValue={filter.user} /></Field>}
      <Button type="submit" variant="outline" className="mb-2">查询期间</Button>
      <Button type="button" variant="ghost" className="mb-2" onClick={() => {setFilter({from:hongKongDay(),to:hongKongDay(),user:filter.user});setOffset(0);setTask(null);setError('');}}>今天</Button>
    </form>
    {error && <p role="alert">{error}</p>}
    <DataState query={query} />
    {result && <>
      <Summary title={`今日 · ${result.today_date}${filter.user ? ` · ${filter.user}` : ''}`} value={result.today} />
      <Summary title={`所选期间 · ${result.from} 至 ${result.to}${filter.user ? ` · ${filter.user}` : ''}`} value={result.summary} />
      <p className="text-sm text-muted-foreground">截至 {date(result.as_of)}（香港）。费用按调用创建日归组并展示当前核算结果；收款按付款日归组并包含后续更正。旧活跃记录不补造。回访账号在期间内使用，且此前另一香港自然日已有使用记录。完成数按成功保存时刻归日。</p>
      <section className="space-y-3 rounded-lg border p-5"><h2>任务费用 · 按已确认成本排序</h2>
        <p className="text-sm text-muted-foreground">每个任务可包含多次调用；点击调用明细核对各笔费用。</p>
        <DataTable rows={result.tasks.items} columns={[
          { header: '账号', cell: ({row}) => <Link to="/users/$userId" params={{userId:row.original.user_id}} className="underline">{row.original.user_id}</Link> },
          { header: '任务', cell: ({row}) => <span className="block min-w-48 max-w-64 break-all whitespace-normal">{row.original.reference_kind === 'run' ? '运行' : '后台任务'} · {row.original.reference}</span> },
          { header: '调用数', accessorKey: 'request_count' },
          { header: '已确认成本（元）', cell: ({row}) => money(row.original.confirmed_cost_micro_cny) },
          { header: '账号扣减（元）', cell: ({row}) => money(row.original.account_debit_micro_cny) },
          { header: '在途 / 待核算占用（元）', cell: ({row}) => `${money(row.original.active_reserved_micro_cny)} / ${money(row.original.pending_micro_cny)}` },
          { header: '成本未知 / 待核算记录', cell: ({row}) => `${row.original.unknown_cost_requests} / ${row.original.pending_requests}` },
          { header: '核对', cell: ({row}) => <Button variant="ghost" onClick={() => setTask(row.original)}>调用明细</Button> },
        ]} />
        <Pagination offset={offset} total={result.tasks.total} change={(v) => {setOffset(v);setTask(null);}} />
      </section>
      {task && <TaskCharges key={`${task.user_id}-${task.reference_kind}-${task.reference}-${result.from}-${result.to}`} task={task} from={result.from} to={result.to} close={() => setTask(null)} />}
    </>}
  </section>;
}
function TaskCharges({ task, from, to, close }: { task: UsageTask; from: string; to: string; close: () => void }) {
  const [offset,setOffset]=useState(0);
  const query=useData<Page<Charge>>(taskChargesPath(task,from,to,offset));
  return <section className="space-y-3 rounded-lg border p-5" aria-label="任务调用明细">
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
    <Link to="/users/$userId" params={{userId:task.user_id}} className="text-sm underline">前往账号详情核算费用</Link>
  </section>;
}
