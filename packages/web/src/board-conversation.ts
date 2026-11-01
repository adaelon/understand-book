import type { BoardConversationSession, OuterOutcome, PendingBoardEdit } from './api';
import type { AnswerDraft, RunActivity } from './agent-run-state';
import type { BoardDelivery } from './generated/BoardDelivery';
import { mergeBoardDeliveries } from './board-workspace';
import { t as uiText } from './i18n';

/** The same answer and draft objects rendered by the reading conversation. */
export interface BoardDiscussionTurn {
  sessionId: string;
  turnId: string | null;
  requestId?: string;
  user: string;
  outcome: OuterOutcome | null;
  pending: boolean;
  draft?: AnswerDraft | null;
  runStatus?: string;
  activities?: RunActivity[];
  error?: string | null;
  commits?: BoardDelivery[];
  pendingBoardEdit?: PendingBoardEdit | null;
}
export function discussionTurnKey(turn: BoardDiscussionTurn) {
  return `${turn.sessionId}:${turn.turnId ?? `request:${turn.requestId}`}`;
}
export function mergeBoardConversation(sessions: BoardConversationSession[], live: BoardDiscussionTurn[]): BoardDiscussionTurn[] {
  const turns = new Map<string, BoardDiscussionTurn>();
  for (const session of sessions) for (const turn of session.turns) {
    const view: BoardDiscussionTurn = {
      sessionId: session.session_id, turnId: turn.turn_id, user: turn.user,
      outcome: turn.outcome ?? null, pending: turn.status === 'pending_assistant',
      error: turn.board_delivery_error?.message ?? turn.error?.message,
      runStatus: turn.status === 'cancelled' ? uiText('已停止') : undefined,
      activities: turn.run_summary?.activities ?? undefined,
      commits: mergeBoardDeliveries(turn.board_commits, turn.board_delivery ? [turn.board_delivery] : []),
      pendingBoardEdit: turn.pending_board_edit,
    };
    turns.set(discussionTurnKey(view), view);
  }
  for (const turn of live) {
    const key = discussionTurnKey(turn), saved = turns.get(key);
    // A completed stored result supersedes a retained in-flight view after a
    // chat switch. Keep an interrupted draft when no final answer exists.
    const completed = saved && !saved.pending && turn.pending;
    turns.set(key, { ...saved, ...turn,
      outcome: completed ? saved.outcome ?? turn.outcome : turn.outcome ?? saved?.outcome ?? null,
      pending: completed ? false : turn.pending,
      error: completed ? saved.error : turn.error ?? saved?.error,
      runStatus: completed ? saved.runStatus : turn.runStatus ?? saved?.runStatus,
      activities: completed ? saved.activities ?? turn.activities : turn.activities ?? saved?.activities,
      commits: mergeBoardDeliveries(saved?.commits, turn.commits),
    });
  }
  return [...turns.values()];
}
