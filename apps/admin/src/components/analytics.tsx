import type { ReactNode } from 'react';
import { AlertCircle, CheckCircle2, ChevronDown } from 'lucide-react';
import { cn } from '@/lib/utils';
import { money } from '@/lib/format';
import { percentage } from '@/lib/allowance';
import type { Percentage } from '@/lib/types';

export type Tone = 'blue' | 'green' | 'amber' | 'violet' | 'muted';
export const chartColors: Record<Tone, string> = {
  blue: 'var(--viz-blue)', green: 'var(--viz-green)', amber: 'var(--viz-amber)',
  violet: 'var(--viz-violet)', muted: 'var(--viz-muted)',
};

// Overview precision only. Ledgers and editable amounts always use money().
export function overviewMoney(value: number | null | undefined, digits = 6): string {
  if (value == null) return '未知';
  const amount = value / 10 ** digits;
  if (amount !== 0 && Math.abs(amount) < 0.01) return amount < 0 ? '> −0.01' : '< 0.01';
  return amount.toLocaleString('zh-CN', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

export function Amount({ value, digits = 6 }: { value: number | null | undefined; digits?: number }) {
  return <span className="tabular-nums" title={`精确金额：${money(value, digits)} 元`}>{value == null ? '未知' : `¥ ${overviewMoney(value, digits)}`}</span>;
}

export function overviewPercentage(value: Percentage): string {
  if (value.nonzero && value.below_tenth) return percentage(value);
  return `${Number(value.value).toLocaleString('zh-CN', { maximumFractionDigits: 2 })}%`;
}

export function AllowanceValue({ ratio, amount }: { ratio: Percentage | null | undefined; amount: number }) {
  return ratio ? <span className="tabular-nums" title={`精确比例：${percentage(ratio)}`}>{overviewPercentage(ratio)}</span> : <Amount value={amount} />;
}

export function Metric({ label, value, detail, tone = 'blue' }: { label: string; value: ReactNode; detail?: ReactNode; tone?: Tone }) {
  return <div className="metric-card" style={{ '--metric-color': chartColors[tone] } as React.CSSProperties}>
    <dt className="text-sm text-muted-foreground">{label}</dt>
    <dd className="mt-3 text-3xl font-semibold tracking-tight tabular-nums">{value}</dd>
    {detail && <dd className="mt-2 text-xs leading-relaxed text-muted-foreground">{detail}</dd>}
  </div>;
}

export function Notice({ children, warning = false }: { children: ReactNode; warning?: boolean }) {
  const Icon = warning ? AlertCircle : CheckCircle2;
  return <div className={cn('insight-notice', warning ? 'insight-warning' : 'insight-ok')}>
    <Icon className="mt-0.5 size-4 shrink-0" aria-hidden="true" /><div>{children}</div>
  </div>;
}

export function Disclosure({ title, children, open = false }: { title: string; children: ReactNode; open?: boolean }) {
  return <details className="detail-disclosure" open={open || undefined}>
    <summary><span>{title}</span><ChevronDown className="size-4 shrink-0" aria-hidden="true" /></summary>
    <div className="space-y-4 pt-4">{children}</div>
  </details>;
}

export function ViewSwitch<T extends string>({ label, value, options, onChange }: {
  label: string; value: T; options: readonly (readonly [T, string])[]; onChange: (value: T) => void;
}) {
  return <div className="view-switch" role="group" aria-label={label}>{options.map(([key, title]) =>
    <button key={key} type="button" aria-pressed={value === key} onClick={() => onChange(key)}>{title}</button>,
  )}</div>;
}

export interface BarSegment { label: string; value: number; tone: Tone; display?: string }
export function Distribution({ label, segments, total, note }: { label: string; segments: BarSegment[]; total?: number; note?: string }) {
  const sum = segments.reduce((value, segment) => value + Math.max(0, segment.value), 0);
  const scale = Math.max(sum, total ?? 0);
  return <figure className="space-y-3" aria-label={label}>
    <div className="distribution-track" role="img" aria-label={`${label}：${segments.map(s => `${s.label} ${s.display ?? s.value}`).join('，')}`}>
      {scale > 0 && segments.filter(s => s.value > 0).map(segment => <span key={segment.label} style={{ width: `${Math.max(0, segment.value) / scale * 100}%`, background: chartColors[segment.tone] }} />)}
    </div>
    <figcaption className="flex flex-wrap gap-x-5 gap-y-2 text-xs">{segments.map(segment => <span key={segment.label} className="inline-flex items-center gap-2">
      <span className="size-2 rounded-full" style={{ background: chartColors[segment.tone] }} aria-hidden="true" />
      <span className="text-muted-foreground">{segment.label}</span><strong className="font-medium tabular-nums">{segment.display ?? segment.value.toLocaleString('zh-CN')}</strong>
    </span>)}</figcaption>
    {note && <p className="text-xs leading-relaxed text-muted-foreground">{note}</p>}
  </figure>;
}

export function StatusBadge({ children, tone = 'muted' }: { children: ReactNode; tone?: Tone }) {
  return <span className="status-badge" style={{ '--status-color': chartColors[tone] } as React.CSSProperties}><span aria-hidden="true" />{children}</span>;
}

export function EmptyState({ children }: { children: ReactNode }) {
  return <p className="rounded-xl border border-dashed px-5 py-9 text-center text-sm text-muted-foreground">{children}</p>;
}
