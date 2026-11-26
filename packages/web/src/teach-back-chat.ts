import type { TeachBackReply } from './generated/TeachBackReply';
import type { AgentChatMeta, OuterOutcome } from './api';

export interface TeachBackAttempt {
  answerTurnId?: string;
  course?: { run_id: string; round_index: number };
  courseContext?: AgentChatMeta['course_context'];
  expressionId: string;
  operationId: string;
  reply?: TeachBackReply;
}

export interface ComparisonResponse {
  action: TeachBackReply['kind'];
  turnId: string;
  roundId: string;
  rowId: string;
  text: string;
}

export function comparisonPart(turn: { outcome: OuterOutcome | null }) {
  return turn.outcome?.answer_view?.parts.find(part => part.kind === 'teach_back_comparison');
}

export interface CourseTeachBackSubmission {
  text: string;
  reply?: TeachBackReply;
  sessionId?: string;
  retry?: boolean;
}

export interface CourseTeachBackState {
  expressionId: string;
  text: string;
  pending: boolean;
  status?: string;
  error?: string;
  canRetry: boolean;
  roundId?: string;
  ordinaryAnswer?: string;
}
