import type { AgentEffect } from './api';
import type { PresentationRef } from './generated/PresentationRef';
import type { PublishedBookRef } from './network-context';

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
  const date = /^\d+$/.test(value) ? new Date(Number(value) * 1000) : new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { hour12: false });
}
