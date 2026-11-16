import { useQuery } from '@tanstack/react-query';
import { useEffect } from 'react';
import { session } from '@/lib/api';
import { Disclosure, Metric, Notice, StatusBadge } from '@/components/analytics';

interface Operations {
  model: { status: string; can_answer: boolean; code: string; message: string; coverage_until: number | null; model: string | null };
  outcomes_today: { outcome: string; is_probe: boolean; count: number; users: number }[];
  recent_outcomes: { user_id: string; email: string | null; request_id: string; turn_id: string | null; book_id: string | null; finished_at: number; outcome: string; stage: string; error_code: string | null; is_probe: boolean }[];
  last_probe: { finished_at: number; outcome: string; error_code: string | null } | null;
  watchdog: { recipient: string; last_check_at: number; last_email_at: number | null; mail_error: string | null; pending_emails: number; incidents: Record<string, { title: string; detail: string }> } | null;
}
const names: Record<string,string> = { delivered:'成功交付', failed:'失败', unsaved:'保存失败', partial:'未完整交付', cancelled:'用户取消', account_blocked:'账号额度受限', rejected:'未受理' };
const stages: Record<string,string> = { admission:'受理', preflight:'调用前检查', model:'模型调用', execution:'执行', delivery:'交付', save:'保存' };
const date = (at: number | null | undefined) => at ? new Date(at*1000).toLocaleString('zh-CN', { timeZone:'Asia/Hong_Kong', hour12:false }) : '尚无记录';

export function OperationsPanel({ paused, clock, refresh }: { paused: boolean; clock: number; refresh: number }) {
  const query = useQuery({
    queryKey: [session.snapshot().epoch, '/admin/operations'],
    queryFn: ({ signal }) => session.request<Operations>('/admin/operations', 'GET', undefined, AbortSignal.any([signal, AbortSignal.timeout(12_000)])),
    refetchInterval: paused ? false : 5000, refetchOnWindowFocus: !paused,
  });
  useEffect(() => { if (refresh) void query.refetch(); }, [refresh, query.refetch]);
  const data = query.data;
  if (!data) return query.error ? <Notice warning>业务监测读取失败：{query.error.message}。当前可用性未知。</Notice> : <section className="analytics-panel animate-pulse" role="status">正在读取业务可用性与邮件告警状态…</section>;
  const { model, watchdog, last_probe: probe } = data;
  const stale = !!query.error || clock-query.dataUpdatedAt>15000;
  const mailStale = !watchdog || clock/1000-watchdog.last_check_at>180;
  const mailProblem = mailStale || !!watchdog?.mail_error || (watchdog?.pending_emails ?? 0)>0;
  const count = (...keys: string[]) => data.outcomes_today.filter(row=>!row.is_probe && keys.includes(row.outcome)).reduce((n,row)=>n+row.count,0);
  const probes = data.outcomes_today.filter(row=>row.is_probe).reduce((n,row)=>n+row.count,0);
  const incidents = Object.values(watchdog?.incidents ?? {});
  return <div className="space-y-4" aria-label="业务可用性与邮件告警">
    <div className="flex flex-wrap items-center justify-between gap-2"><h2>业务可用性与告警</h2><span className="text-xs text-muted-foreground">独立巡检每分钟运行 · 关闭网页后继续检测</span></div>
    {stale && <Notice warning>业务数据{paused ? '已暂停更新' : '刷新中断'}，以下为上次结果，当前可用性待确认。</Notice>}
    <div className="grid gap-4 lg:grid-cols-3">
      <section className="analytics-panel min-w-0">
        <div className="flex flex-wrap items-center justify-between gap-2"><h3 className="font-medium">问答配置</h3><StatusBadge tone={!stale && model.status==='ready' ? 'green' : 'amber'}>{stale ? '待确认' : model.can_answer ? model.status==='ready' ? '配置可用' : '即将到期' : '无法调用'}</StatusBadge></div>
        <p className="text-sm leading-relaxed">{model.message}</p>
        <dl className="space-y-2 text-xs text-muted-foreground"><div>当前模型：<span className="break-all text-foreground">{model.model ?? '未配置'}</span></div><div>连续费率覆盖至：<span className="text-foreground">{date(model.coverage_until)}</span></div></dl>
        <p className="text-xs text-muted-foreground">提前 7 天预警，剩余 24 小时升级告警。</p>
      </section>
      <section className="analytics-panel min-w-0">
        <div className="flex flex-wrap items-center justify-between gap-2"><h3 className="font-medium">最近真实交付验证</h3><StatusBadge tone={probe?.outcome==='delivered' ? 'green' : 'amber'}>{probe ? names[probe.outcome] ?? probe.outcome : '尚未验证'}</StatusBadge></div>
        <p className="text-sm">{date(probe?.finished_at)}</p>
        <p className="text-xs leading-relaxed text-muted-foreground">通过实际模型回答、结果保存和交付记录验证。此处展示最近一次结果，不代表持续向模型发送测试。</p>
        {probe?.error_code && <code className="break-all text-xs">{probe.error_code}</code>}
      </section>
      <section className="analytics-panel min-w-0">
        <div className="flex flex-wrap items-center justify-between gap-2"><h3 className="font-medium">自动邮件通知</h3><StatusBadge tone={mailProblem ? 'amber' : 'green'}>{mailStale ? '巡检待确认' : watchdog?.mail_error ? '发送失败，待重试' : watchdog?.pending_emails ? '等待发送' : '已启用'}</StatusBadge></div>
        <p className="break-all text-sm">{watchdog?.recipient ?? '尚未配置通知邮箱'}</p>
        <dl className="space-y-2 text-xs text-muted-foreground"><div>最近巡检：{date(watchdog?.last_check_at)}</div><div>最近发信：{date(watchdog?.last_email_at)}</div></dl>
        <p className="text-xs leading-relaxed text-muted-foreground">首次异常、影响升级和恢复时通知。监测运行于本机，整机离线需外部监测。</p>
        {watchdog?.mail_error && <code className="break-all text-xs">{watchdog.mail_error}</code>}
      </section>
    </div>
    {incidents.length>0 && <Notice warning><strong>待处理 / 待验证恢复：{incidents.length} 项</strong><ul className="mt-2 space-y-1">{incidents.map((item,i)=><li className="break-words" key={i}>{item.title} · {item.detail}</li>)}</ul></Notice>}
    <dl className="metric-grid">
      <Metric label="今日失败 / 保存失败" value={count('failed','unsaved')} detail="已记录的用户请求结果" tone="amber" />
      <Metric label="今日未完整交付" value={count('partial')} detail="回答未达到完整交付条件" tone="amber" />
      <Metric label="今日额度受限" value={count('account_blocked')} detail="账号额度或有效期阻止调用" tone="violet" />
      <Metric label="今日取消 / 未受理" value={count('cancelled','rejected')} detail={`运维测试另计：今日 ${probes} 次`} tone="muted" />
    </dl>
    <Disclosure title={`最近 24 小时未交付明细 · ${data.recent_outcomes.length} 条（最多 50 条）`} open>
      {data.recent_outcomes.length===0 ? <p className="text-sm text-muted-foreground">最近 24 小时没有记录到未交付请求。</p> : <div className="space-y-3">{data.recent_outcomes.map(row=><article key={`${row.user_id}:${row.request_id}`} className="rounded-lg border p-3">
        <div className="flex flex-wrap items-start justify-between gap-2"><div className="min-w-0"><strong className="break-all text-sm">{row.email ?? row.user_id}</strong><span className="ml-2 text-xs text-muted-foreground">{row.is_probe ? '运维测试' : '用户请求'}</span></div><StatusBadge tone={row.outcome==='cancelled' || row.outcome==='rejected' ? 'muted' : 'amber'}>{names[row.outcome] ?? row.outcome}</StatusBadge></div>
        <div className="mt-2 flex flex-wrap gap-x-5 gap-y-1 text-xs text-muted-foreground"><span>{date(row.finished_at)}</span><span>{stages[row.stage] ?? row.stage}</span><code className="break-all text-foreground">{row.error_code ?? '交付未完成'}</code></div>
        <details className="mt-2 text-xs text-muted-foreground"><summary className="cursor-pointer">查看追踪标识</summary><dl className="mt-2 space-y-1 break-all"><div>账号：{row.user_id}</div><div>请求：{row.request_id}</div><div>回合：{row.turn_id ?? '尚未创建'}</div><div>书籍：{row.book_id ?? '—'}</div></dl></details>
      </article>)}</div>}
    </Disclosure>
  </div>;
}
