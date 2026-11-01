import { ref } from 'vue';
import { emptyNoteContext, noteRequest, pendingNoteSource, noteSourceError, type NoteContext, type NoteRef } from './personal-notes';
import type { TeachingTarget } from './generated/TeachingTarget';

export interface UnderstandingReference { note_id: string; workspace_id: string | null; evidence_ref: string }
export interface UnderstandingSummary {
  focus_ref: string; source_id: string; source_revision: string; workspace_id: string | null;
  label: string; capability: string; target: TeachingTarget | null;
  demonstrated: string[]; difficulties: string[]; unconfirmed: string[];
  evidence_ref: string; evidence_refs: string[]; note_refs: NoteRef[]; next_step: string;
  occurred_at: string | null; source_session_id: string | null; source_turn_id: string | null;
  source_session_title?: string | null;
  assistance: string; nature: 'hypothesis' | 'performance'; learner_quote: string; correction: string | null;
}
export interface ThinkingEntry {
  id: string; kind: 'revision' | 'discussion' | 'understanding' | 'correction' | 'activity' | 'action';
  occurred_at?: string | null; text?: string | null; reference?: NoteRef; note_refs?: NoteRef[];
  authorship?: string; change_note?: string; restored_from?: number;
  based_on_turn?: { session_id: string; turn_id: string } | null;
  source_session_id?: string | null; source_turn_id?: string | null; session_title?: string;
  workspace_id?: string | null; source_id?: string; source_revision?: string;
  label?: string; interpretation?: string; correction?: string; assisted?: boolean; attempt?: number;
  nature?: 'hypothesis' | 'performance'; superseded?: boolean; questionable?: boolean; action?: string;
}
export interface NoteThinkingView {
  note_id: string; current_revision: number; current_understanding: UnderstandingSummary[];
  timeline: ThinkingEntry[]; next_cursor: number | null; update_status: 'pending' | 'failed' | 'ready';
  updates: { job_ref: string; workspace_id: string | null; status: string }[];
}
export interface UnderstandingAction {
  context: NoteContext; note_ref: NoteRef; source_revision: string; target: TeachingTarget;
  understanding_ref: UnderstandingReference; mode: 'discuss' | 'practice';
}
export const pendingUnderstandingAction = ref<UnderstandingAction | null>(null);
export async function continueUnderstanding(item: UnderstandingSummary, noteId: string, mode: UnderstandingAction['mode']) {
  const result = await noteRequest<Omit<UnderstandingAction, 'mode'>>('thinking_start', {
    note_id: noteId, workspace_id: item.workspace_id, evidence_ref: item.evidence_ref,
  });
  noteSourceError.value = '';
  const context = { ...emptyNoteContext(), ...result.context };
  pendingUnderstandingAction.value = { ...result, context, mode };
  pendingNoteSource.value = context;
}
export function thinkingTime(value?: string | null) {
  if (!value) return '时间未记录';
  const date = /^\d+(\.\d+)?$/.test(value) ? new Date(Number(value) * (Number(value) < 1e11 ? 1000 : 1)) : new Date(value);
  return Number.isNaN(date.getTime()) ? '时间未记录' : date.toLocaleString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
}
