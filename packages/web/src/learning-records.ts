import type { AgentChatTurn, SourcePopupView } from './api';
import type { PublishedBookRef, WorkspaceGoal } from './reader-context';
import type { SessionRecap } from './session-recap';

export interface LearningRecordSelection {
  source_learning_workspace_id: string | null;
  chat_session_id: string;
  turn_ids: string[];
  through_seq: number;
}
export interface LearningRecordSummary {
  session_id: string; title: string; book_id: string; updated_at: string;
  source_learning_workspace_id: string | null; source_goal: string | null;
  published_book_ref: PublishedBookRef | null; published_book_refs: PublishedBookRef[];
  turn_count: number; through_seq: number;
}
export interface LinkedLearningRecord {
  selection: LearningRecordSelection; linked_at: string;
}
export interface ImportedLearningRecord extends LinkedLearningRecord {
  through_seq: number;
  title: string | null; book_id: string | null; available: boolean;
  source_goal?: string | null;
}
export interface LearningRecordEvidence {
  evidence_id: string; label: string; nature: string; interpretation: string;
  learner_quote: string; teaching_implication: string; correction: string | null;
  supersedes: string | null; fact_refs: string[];
}
export interface LearningRecordTurn extends AgentChatTurn {
  workspace_goal: WorkspaceGoal | null;
  accepted_workspace_goal: WorkspaceGoal | null;
}
export interface LearningRecordDetail {
  session_id: string; title: string; learning_workspace_id: string | null;
  book_id: string; through_seq: number; turns: LearningRecordTurn[]; recap: SessionRecap;
  sources: { turn_id: string; source_ref_id: string; label: string; quote: string;
    published_book_ref: PublishedBookRef | null; unavailable_reason: string | null; view: SourcePopupView | null }[];
  learning: { selection: LearningRecordSelection; linked_at: string;
    facts: { event_id: string; kind: string; occurred_at: string; causal_refs: string[] }[];
    evidence: LearningRecordEvidence[]; assessments: unknown[] };
}
