import { describe, expect, it } from 'vitest';
import { initialRun, reduceRun, runStatusText } from './agent-run-state';
const descriptor = { book_id: 'book', session_id: 'chat', turn_id: 'turn' };
describe('MU8 observation recovery', () => {
  it('replaces old live data when a restarted server sends a lower sequence snapshot', () => {
    const state = { ...initialRun(descriptor), last_seq: 90, observation_epoch: 'old', draft: { message_id: 1, revision: 1, operation: 'replace', view: { parts: [], sources: [] } } };
    const fresh = { ...initialRun(descriptor), last_seq: 0 };
    const next = reduceRun(state, { turn_id: 'turn', seq: 0, elapsed_ms: 0, type: 'run.snapshot', payload: fresh, observation_epoch: 'new', live_buffer_reset: true });
    expect(next.last_seq).toBe(0); expect(next.draft).toBeUndefined();
    expect(next.observation_epoch).toBe('new');
    expect(reduceRun(next, { turn_id: 'turn', seq: 91, elapsed_ms: 0, type: 'answer.patch', payload: state.draft, observation_epoch: 'old' })).toBe(next);
  });
  it('does not accept a snapshot for another chat and retains local sequence ordering', () => {
    const state = { ...initialRun(descriptor), last_seq: 5 };
    expect(reduceRun(state, { turn_id: 'turn', seq: 0, elapsed_ms: 0, type: 'run.snapshot', payload: initialRun(descriptor) })).toBe(state);
    expect(reduceRun(state, { turn_id: 'turn', seq: 0, elapsed_ms: 0, type: 'run.snapshot', observation_epoch: 'new', live_buffer_reset: true, payload: initialRun({ ...descriptor, session_id: 'other' }) })).toBe(state);
  });
  it('explains expired confirmations and queue waiting', () => {
    expect(runStatusText({ ...initialRun(descriptor), error: { error_code: 'SENSITIVE_CONFIRMATION_EXPIRED', category: 'conflict', message: '' } }, 'closed')).toContain('重新确认');
    const waiting = reduceRun(initialRun(descriptor), { turn_id: 'turn', seq: 1, elapsed_ms: 0, type: 'run.resource', payload: { execution_state: 'waiting_model' } });
    expect(runStatusText(waiting, 'connected')).toContain('等待模型资源');
    expect(reduceRun(waiting, { turn_id: 'turn', seq: 2, elapsed_ms: 0, type: 'run.resource', payload: { execution_state: 'running' } }).execution_state).toBe('running');
    expect(runStatusText({ ...initialRun(descriptor), execution_state: 'interrupted', persistence_state: 'failed', error: { error_code: 'OBJECT_NOT_FOUND', category: 'not_found', message: '' } }, 'closed')).toContain('对话已不可用');
  });
});
