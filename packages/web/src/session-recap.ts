import type { AgentEffect } from './api';
import type { PresentationRef } from './generated/PresentationRef';
import type { PublishedBookRef } from './network-context';
import type { SharePart, ShareSource } from './reading-share';

export interface RecapEvidence { turn_id: string; event_seq: number }
export interface RecapQuestion { text: string; status: string; evidence: RecapEvidence[] }
export interface RecapSource {
  source_ref_id: string; label: string; quote: string; published_book_ref: PublishedBookRef | null;
  evidence: RecapEvidence[]; unavailable_reason: string | null;
}
export interface RecapEffect {
  effect_id: string; label: string; status: string;
  effect: { kind: 'reader'; effect: AgentEffect } | { kind: 'presentation'; reference: PresentationRef };
  object_id: string | null; published_book_ref: PublishedBookRef | null;
  evidence: RecapEvidence[]; unavailable_reason: string | null;
}
export interface SessionRecap {
  session_id: string; through_seq: number; through_at: string; generated_at: string;
  questions: RecapQuestion[]; sources: RecapSource[]; effects: RecapEffect[];
  continuations: (RecapQuestion & { goal_id: string | null })[];
}
export type RecapTarget = { session_id: string; through_seq: number; turn_id: string } & (
  { kind: 'turn' } | { kind: 'source'; source: RecapSource } | { kind: 'effect'; effect: RecapEffect }
);
export function targetPublication(target: RecapTarget): PublishedBookRef | null {
  return target.kind === 'source' ? target.source.published_book_ref
    : target.kind === 'effect' ? target.effect.published_book_ref : null;
}
export function recapStatus(status: string): string {
  return ({ running: '进行中', answered: '已回答', interrupted: '中断', cancelled: '已停止', incomplete: '未完成',
    open: '待继续', delivered: '已交付', pending: '待处理', unconfirmed: '待核对', failed: '处理失败',
    kept: '已保留', undone: '已撤销', dismissed: '已忽略', applied: '已应用' } as Record<string, string>)[status] ?? status;
}
export function recapTime(value: string): string {
  // Native hosts write milliseconds; older records and network responses may use seconds.
  const date = /^\d+$/.test(value) ? new Date(Number(value) * (value.length <= 10 ? 1000 : 1)) : new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { hour12: false });
}

export const recapGroups = [
  { kind: 'questions', label: '讨论过的问题', empty: '还没有提问。' },
  { kind: 'sources', label: '引用的原文', empty: '还没有已绑定的原文来源。' },
  { kind: 'effects', label: '留下的成果', empty: '还没有阅读成果。' },
  { kind: 'continuations', label: '待继续事项', empty: '没有明确待继续的任务或运行。' },
] as const;
export type RecapEntry = {
  id: `recap:${string}`; group: typeof recapGroups[number]['kind']; text: string;
  label: string; status: string; unavailable: string | null; target: RecapTarget; action: string; turnAction: string;
};

/** Display and selection share the same factual projection; navigation keeps the original objects. */
export function recapEntries(recap: SessionRecap): RecapEntry[] {
  const base = (evidence: RecapEvidence[]) => ({ session_id: recap.session_id, through_seq: recap.through_seq, turn_id: evidence[0].turn_id });
  const id = (group: RecapEntry['group'], evidence: RecapEvidence[], object = ''): RecapEntry['id'] =>
    `recap:${JSON.stringify([group, evidence[0].turn_id, object])}`;
  return [
    ...recap.questions.map(item => ({ id: id('questions', item.evidence), group: 'questions' as const,
      text: item.text, label: '', status: recapStatus(item.status), unavailable: null,
      target: { ...base(item.evidence), kind: 'turn' as const }, action: '回到问题', turnAction: '' })),
    ...recap.sources.map(item => ({ id: id('sources', item.evidence, item.source_ref_id), group: 'sources' as const,
      text: item.quote, label: item.label, status: '', unavailable: item.unavailable_reason,
      target: { ...base(item.evidence), kind: 'source' as const, source: item }, action: '查看原文', turnAction: '关联问题' })),
    ...recap.effects.map(item => ({ id: id('effects', item.evidence, item.effect_id), group: 'effects' as const,
      text: item.label, label: '', status: recapStatus(item.status), unavailable: item.unavailable_reason,
      target: { ...base(item.evidence), kind: 'effect' as const, effect: item },
      action: item.effect.kind === 'presentation' ? '查看原版本' : item.object_id ? '定位成果' : '查看操作记录', turnAction: '关联回合' })),
    ...recap.continuations.map(item => ({ id: id('continuations', item.evidence, item.goal_id ?? ''), group: 'continuations' as const,
      text: item.text, label: '', status: recapStatus(item.status), unavailable: null,
      target: { ...base(item.evidence), kind: 'turn' as const }, action: `回到${item.goal_id ? '任务' : '回合'}`, turnAction: '' })),
  ];
}

/** Copy only explicitly chosen facts. Refreshing the recap cannot mutate this draft. */
export function recapShareSource(recap: SessionRecap, selected: string[], sessionTitle = ''): ShareSource {
  const parts: SharePart[] = recapEntries(recap).filter(item => selected.includes(item.id)).map(item => ({
    id: item.id,
    label: ['阅读回顾', recapGroups.find(group => group.kind === item.group)!.label, item.status].filter(Boolean).join(' · '),
    text: [item.label, item.text, item.unavailable].filter(Boolean).join('\n\n'), format: 'plain',
  }));
  return { parts, sources: [`从对话开始 · 截至 ${recapTime(recap.through_at)}`],
    association: sessionTitle.trim() ? `对话「${sessionTitle.trim()}」` : '当前对话',
    recap: { sessionId: recap.session_id, throughSeq: recap.through_seq, throughAt: recap.through_at } };
}
