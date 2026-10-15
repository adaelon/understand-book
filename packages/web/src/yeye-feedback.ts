import type { AgentHistoryResponse, ResidentGoal } from './api';
import type { RunSnapshot } from './agent-run-state';

/** A reading scene's observed facts, consumed on every read, including hidden reads. */
export function createYeyeFeedback() {
  let scope: string | undefined;
  const goals = new Map<string, Pick<ResidentGoal, 'revision' | 'status'>>();
  function reset() { scope = undefined; goals.clear(); }
  function observe(context: string, history: AgentHistoryResponse, run?: RunSnapshot): boolean {
    const sameScope = scope === context;
    if (!sameScope) { goals.clear(); scope = context; }
    const turn = history.current.turns.find(item => item.turn_id === run?.descriptor.turn_id);
    const savedCompletion = sameScope && run?.persistence_state === 'saved'
      && run.execution_state === 'completed' && !run.error && !run.final_view?.error
      && run.descriptor.book_id === history.current.book_id
      && run.descriptor.session_id === history.current.id
      && history.active_session_id === history.current.id
      && turn?.status === 'completed' && !turn.error;
    let completed = false;
    for (const goal of history.current.goals ?? []) {
      const previous = goals.get(goal.id);
      // A late earlier read must not reopen a consumed completion.
      if (previous && goal.revision <= previous.revision) continue;
      if (savedCompletion && previous?.status === 'open' && goal.status === 'completed'
        && goal.result_refs.length > 0) completed = true;
      goals.set(goal.id, { revision: goal.revision, status: goal.status });
    }
    return completed;
  }
  return { observe, reset };
}
