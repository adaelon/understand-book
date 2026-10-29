import { useEffect, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Activity, ArrowDown, ArrowUp, HardDrive, MemoryStick, Pause, Play, RefreshCw } from 'lucide-react';
import { session } from '@/lib/api';
import { bandwidth, bytes, monitorTime, trendSegments, uptime, type Capacity, type MonitorReport, type MonitorSample } from '@/lib/monitor';
import { chartColors, Disclosure, Metric, Notice, StatusBadge } from '@/components/analytics';
import { Button } from '@/components/ui/button';
import './monitor.css';

type Series = { label: string; color: string; read: (sample: MonitorSample) => number | null | undefined };

function Trend({ title, samples, series, format, percent = false, integer = false }: {
  title: string; samples: MonitorSample[]; series: Series[]; format: (value: number) => string; percent?: boolean; integer?: boolean;
}) {
  const [selected, setSelected] = useState<number | null>(null);
  const start = samples[0]?.sampled_at ?? 0;
  const end = samples.at(-1)?.sampled_at ?? start;
  const extent = Math.max(5, end - start);
  const groups = series.map(item => trendSegments(samples, item.read));
  const peak = Math.max(1, ...groups.flat(2).map(point => point.value));
  const maximum = percent ? 100 : integer ? Math.ceil(peak * 1.1 / 2) * 2 : peak * 1.15;
  const x = (at: number) => (at - start) / extent * 600;
  const y = (value: number) => 138 - value / maximum * 126;
  const detail = selected == null ? samples.at(-1) : samples.find(sample => sample.sampled_at === selected) ?? samples.at(-1);
  return <section className="analytics-panel monitor-trend" aria-label={title}>
    <div className="flex flex-wrap items-center justify-between gap-2"><h2>{title}</h2><span className="text-xs text-muted-foreground">{detail ? monitorTime(detail.sampled_at) : '等待采样'}</span></div>
    <div className="flex flex-wrap gap-x-5 gap-y-2 text-xs">{series.map(item => <span key={item.label} className="inline-flex items-center gap-2"><span className="size-2 rounded-full" style={{ background: item.color }} />{item.label}<strong className="tabular-nums">{detail && item.read(detail) != null ? format(item.read(detail)!) : '—'}</strong></span>)}</div>
    <div className="grid grid-cols-[3.75rem_minmax(0,1fr)] gap-x-2 gap-y-2">
    <div className="flex flex-col justify-between py-3 text-right text-[11px] leading-none text-muted-foreground">{[maximum, maximum / 2, 0].map(value => <span key={value}>{format(value)}</span>)}</div>
    <svg viewBox="0 0 600 150" preserveAspectRatio="none" className="h-40 w-full overflow-visible" role="img" aria-label={`${title}趋势；下方可选择采样时间`} onMouseLeave={() => setSelected(null)} onMouseMove={event => {
      const rect = event.currentTarget.getBoundingClientRect();
      const at = start + Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width)) * extent;
      const nearest = samples.reduce<MonitorSample | undefined>((best, sample) => !best || Math.abs(sample.sampled_at - at) < Math.abs(best.sampled_at - at) ? sample : best, undefined);
      if (nearest) setSelected(nearest.sampled_at);
    }}>
      {[0, 0.5, 1].map(ratio => <line key={ratio} x1="0" x2="600" y1={y(maximum * ratio)} y2={y(maximum * ratio)} stroke="var(--border)" strokeDasharray="4 5" />)}
      {series.map((item, index) => <g key={item.label}>{groups[index].map((group, segment) => <g key={segment}>
        <polyline points={group.map(point => `${x(point.at)},${y(point.value)}`).join(' ')} fill="none" stroke={item.color} strokeWidth="2" vectorEffect="non-scaling-stroke" strokeLinejoin="round" strokeLinecap="round" />
        {group.length === 1 && <circle cx={x(group[0].at)} cy={y(group[0].value)} r="3" fill={item.color} />}
      </g>)}</g>)}
      {selected != null && detail && <line x1={x(detail.sampled_at)} x2={x(detail.sampled_at)} y1="12" y2="138" stroke="var(--muted-foreground)" strokeDasharray="3 3" />}
    </svg>
    <div className="col-start-2 flex justify-between gap-2 text-[11px] text-muted-foreground"><span>{monitorTime(start)}</span><span>{monitorTime(end)}</span></div>
    </div>
    {!groups.some(group => group.length) && <p className="text-center text-xs text-muted-foreground">等待有效采样</p>}
    <input type="range" className="w-full accent-primary" min="0" max={Math.max(0, samples.length - 1)} value={Math.max(0, samples.findIndex(sample => sample === detail))} disabled={samples.length < 2} aria-label={`${title}采样时间`} onChange={event => setSelected(samples[Number(event.target.value)]?.sampled_at ?? null)} />
  </section>;
}

function ResourceCard({ title, value, icon, detail }: { title: string; value: Capacity | null; icon: React.ReactNode; detail: string }) {
  return <section className="analytics-panel" aria-label={title}>
    <div className="flex items-center justify-between gap-3"><h2 className="text-base">{title}</h2><span className="text-muted-foreground">{icon}</span></div>
    <div className="flex flex-wrap items-baseline gap-2"><strong className="text-3xl font-semibold tabular-nums">{value ? `${value.used_percent.toFixed(1)}%` : '—'}</strong><span className="text-xs text-muted-foreground">{value ? '已使用' : '暂不可用'}</span></div>
    <div className="h-2 overflow-hidden rounded-full bg-muted" role="meter" aria-label={`${title}使用率`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={value?.used_percent} aria-valuetext={value ? `${value.used_percent.toFixed(1)}%` : '暂不可用'}><div className="h-full rounded-full" style={{ width: `${value?.used_percent ?? 0}%`, background: (value?.used_percent ?? 0) >= 90 ? chartColors.amber : chartColors.blue }} /></div>
    <dl className="space-y-1 text-xs"><div className="flex justify-between gap-2"><dt className="text-muted-foreground">已用 / 总量</dt><dd className="tabular-nums">{value ? `${bytes(value.used_bytes)} / ${bytes(value.total_bytes)}` : '—'}</dd></div><div className="flex justify-between gap-2"><dt className="text-muted-foreground">可用</dt><dd className="tabular-nums">{value ? bytes(value.available_bytes) : '—'}</dd></div></dl>
    <p className="text-xs leading-relaxed text-muted-foreground">{detail}</p>
  </section>;
}

export function MonitorPage() {
  const [paused, setPaused] = useState(false);
  const [clock, setClock] = useState(Date.now());
  useEffect(() => { const timer = setInterval(() => setClock(Date.now()), 1000); return () => clearInterval(timer); }, []);
  const query = useQuery({
    queryKey: [session.snapshot().epoch, '/admin/monitor'],
    queryFn: ({ signal }) => session.request<MonitorReport>('/admin/monitor', 'GET', undefined, AbortSignal.any([signal, AbortSignal.timeout(12_000)])),
    refetchInterval: paused ? false : 5000,
    refetchIntervalInBackground: false,
    refetchOnWindowFocus: !paused,
  });
  const report = query.data, current = report?.current;
  const stale = !!current && clock - query.dataUpdatedAt > 15_000;
  const status = paused ? '已暂停' : query.error ? '连接中断' : stale ? '数据已过期' : current ? '实时更新' : '正在连接';
  const pausedOrStale = paused || !!query.error || stale;
  return <section className="space-y-6">
    <div className="flex flex-wrap items-start justify-between gap-4">
      <div><div className="flex flex-wrap items-center gap-3"><h1>实时监控</h1><StatusBadge tone={pausedOrStale ? 'amber' : 'green'}>{status}</StatusBadge></div><p className="mt-2 text-sm text-muted-foreground">服务器资源与全站对话，一眼了解当前运行情况。</p></div>
      <div className="flex flex-wrap gap-2"><Button variant="outline" onClick={() => { setPaused(!paused); if (paused) void query.refetch(); }}>{paused ? <Play className="size-4" /> : <Pause className="size-4" />}{paused ? '恢复更新' : '暂停更新'}</Button><Button variant="outline" disabled={query.isFetching} onClick={() => void query.refetch()}><RefreshCw className={`size-4 ${query.isFetching ? 'animate-spin' : ''}`} />立即刷新</Button></div>
    </div>
    <div className="flex flex-wrap items-center gap-x-5 gap-y-2 text-xs text-muted-foreground"><span className="inline-flex items-center gap-2"><Activity className="size-3.5" />每 5 秒刷新 · 隐藏页面时暂停</span><span>香港时间 {current ? monitorTime(current.sampled_at) : '—'}</span>{current && <span>服务已运行 {uptime(current.service_uptime_seconds)}</span>}</div>
    {query.error && <div role="alert"><Notice warning>刷新失败：{query.error.message}。{current ? '下方保留上次采样，连接恢复后自动更新。' : '连接恢复后自动重试，也可以点击立即刷新。'}</Notice></div>}
    {stale && !query.error && <Notice warning>{paused ? '自动更新已暂停，当前显示暂停前的数据。' : '尚未收到最新采样，当前数值可能已变化。'}</Notice>}
    {query.isPending && !query.error && <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4" role="status" aria-label="正在读取监控数据">{[0,1,2,3].map(item => <div key={item} className="h-32 animate-pulse rounded-xl bg-muted" />)}</div>}
    {current && report && <>
      {current.issues.length > 0 && <Notice warning>{current.issues.join(' ')}</Notice>}
      <dl className="metric-grid">
        <Metric label="在线人数" value={current.activity.online_users.toLocaleString()} detail={`最近 ${report.online_window_seconds} 秒内有可见页面心跳的账号`} tone="green" />
        <Metric label="正在进行的 AI 对话" value={current.activity.active_runs.toLocaleString()} detail={`排队 ${current.activity.queued_runs} 轮 · 并发上限 ${current.activity.active_run_limit}`} tone="violet" />
        <Metric label="今日对话轮次" value={current.activity.today_turns.toLocaleString()} detail="香港时间今日受理的提问与继续" />
        <Metric label="今日完成交付" value={current.activity.today_completed_turns.toLocaleString()} detail="今日成功保存并交付的对话轮次" tone="muted" />
      </dl>
      <div className="grid gap-4 lg:grid-cols-3">
        <ResourceCard title="服务器内存" value={current.memory} icon={<MemoryStick className="size-5" />} detail="整台服务器的内存；可回收缓存计入可用。" />
        <ResourceCard title="服务数据磁盘" value={current.disk} icon={<HardDrive className="size-5" />} detail="服务数据所在文件系统；可用空间扣除系统预留。" />
        <section className="analytics-panel" aria-label="实时带宽"><div className="flex items-center justify-between gap-3"><h2 className="text-base">实时带宽</h2><Activity className="size-5 text-muted-foreground" /></div>
          <dl className="space-y-5"><div><dt className="flex items-center gap-2 text-xs text-muted-foreground"><ArrowDown className="size-3.5" />入站 / 下载</dt><dd className="mt-1 text-2xl font-semibold tabular-nums" style={{ color: chartColors.blue }}>{current.network ? bandwidth(current.network.receive_bytes_per_second) : '暂不可用'}</dd></div><div><dt className="flex items-center gap-2 text-xs text-muted-foreground"><ArrowUp className="size-3.5" />出站 / 上传</dt><dd className="mt-1 text-2xl font-semibold tabular-nums" style={{ color: chartColors.green }}>{current.network ? bandwidth(current.network.transmit_bytes_per_second) : '暂不可用'}</dd></div></dl>
          <p className="break-words text-xs text-muted-foreground">{current.network ? `网卡 ${current.network.interfaces.join('、')} · 默认路由网卡的实际流量` : '等待服务器网卡数据'}</p>
        </section>
      </div>
      <div className="flex flex-wrap items-baseline justify-between gap-2"><h2>近期趋势</h2><span className="text-xs text-muted-foreground">最多保留 10 分钟 · 仅在监控页打开时采样 · 拖动滑块查看</span></div>
      <div className="grid gap-4 xl:grid-cols-2">
        <Trend title="内存与磁盘使用率" samples={report.history} series={[{label:'内存',color:chartColors.blue,read:s=>s.memory?.used_percent},{label:'磁盘',color:chartColors.violet,read:s=>s.disk?.used_percent}]} percent format={v=>`${v.toFixed(0)}%`} />
        <Trend title="网络吞吐" samples={report.history} series={[{label:'入站',color:chartColors.blue,read:s=>s.network?.receive_bytes_per_second},{label:'出站',color:chartColors.green,read:s=>s.network?.transmit_bytes_per_second}]} format={bandwidth} />
        <Trend title="在线人数" samples={report.history} series={[{label:'在线账号',color:chartColors.green,read:s=>s.activity.online_users}]} integer format={v=>`${v} 人`} />
        <Trend title="AI 对话并发" samples={report.history} series={[{label:'进行中',color:chartColors.violet,read:s=>s.activity.active_runs},{label:'排队',color:chartColors.amber,read:s=>s.activity.queued_runs}]} integer format={v=>`${v} 轮`} />
      </div>
      <Disclosure title="统计口径">
        <dl className="space-y-3 text-sm text-muted-foreground"><div><dt className="font-medium text-foreground">在线人数</dt><dd>已登录的阅读器或后台页面可见时，每 30 秒发送一次心跳；按账号去重，关闭或隐藏全部页面后最长 90 秒移出在线统计。退出、停用与会话失效会在下一次采样时剔除。</dd></div><div><dt className="font-medium text-foreground">AI 对话数</dt><dd>一次被受理的提问或继续算一轮，包括等待中的请求；一次回答内部的多次模型调用不会重复计数。今日受理和今日完成分别按各自发生日期统计，因此跨日完成可能大于今日受理。后台整理与资料构建不计入对话并发。</dd></div><div><dt className="font-medium text-foreground">资源与趋势</dt><dd>系统资源来自运行当前服务的 Linux 服务器。带宽为默认路由网卡的实际传输速率，包含服务器其他进程的流量；首次采样或长时间暂停后，需要等待下一次采样计算速率。趋势保存在服务内存中，服务重启后重新积累。</dd></div></dl>
      </Disclosure>
    </>}
  </section>;
}
