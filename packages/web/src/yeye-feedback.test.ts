import { describe, expect, it } from 'vitest';
import type { AgentHistoryResponse, ResidentGoal } from './api';
import { initialRun, type RunSnapshot } from './agent-run-state';
import { createYeyeFeedback } from './yeye-feedback';

const open: ResidentGoal = { id: 'goal', revision: 1, status: 'open', interpretation: '整理缓存笔记', requirements: [],
  working: { focus: '', open_questions: [], next_move: '', items: [] }, result_refs: [] };
const completed: ResidentGoal = { ...open, revision: 2, status: 'completed', result_refs: ['note:cache'] };
function history(goals: ResidentGoal[], status: 'completed' | 'failed' | 'cancelled' | 'pending_assistant' = 'completed'): AgentHistoryResponse {
  return { active_session_id: 'chat', sessions: [], current: { id: 'chat', book_id: 'book', title: '', created_at: '', updated_at: '', goals,
    turns: [{ turn_id: 'turn', user: '整理缓存笔记', user_turn_ordinal: 0, status, question_source_label: null, question_quote: null, effect_labels: [] }] } };
}
const saved: RunSnapshot = { ...initialRun({ book_id: 'book', session_id: 'chat', turn_id: 'turn' }), execution_state: 'completed', persistence_state: 'saved' };

describe('current saved goal feedback', () => {
  it('consumes an observed open→completed result once, including repeated reads and older snapshots', () => {
    const feedback = createYeyeFeedback();
    expect(feedback.observe('scene', history([open]))).toBe(false);
    expect(feedback.observe('scene', history([completed]), saved)).toBe(true);
    expect(feedback.observe('scene', history([completed]), saved)).toBe(false);
    expect(feedback.observe('scene', history([open]))).toBe(false);
    expect(feedback.observe('scene', history([completed]), saved)).toBe(false);
  });
  it.each([
    ['ordinary answer', open], ['no delivered result', { ...completed, result_refs: [] }],
    ['same revision', { ...completed, revision: 1 }], ['cancelled goal', { ...completed, status: 'cancelled' }],
    ['superseded goal', { ...completed, status: 'superseded' }], ['created and completed this turn', { ...completed, id: 'new' }],
  ] satisfies [string, ResidentGoal][])('stays static for %s', (_name, goal) => {
    const feedback = createYeyeFeedback(); feedback.observe('scene', history([open]));
    expect(feedback.observe('scene', history([goal]), saved)).toBe(false);
  });
  it.each([
    ['still running', { execution_state: 'running', persistence_state: 'pending' }],
    ['saving', { execution_state: 'finalizing', persistence_state: 'pending' }],
    ['unsaved', { persistence_state: 'failed' }], ['failed run', { execution_state: 'failed' }],
    ['cancelled run', { execution_state: 'cancelled' }], ['error', { error: { error_code: 'FAILED', category: 'runtime', message: 'failed' } }],
  ] satisfies [string, Partial<RunSnapshot>][])('consumes without playing for %s', (_name, change) => {
    const feedback = createYeyeFeedback(); feedback.observe('scene', history([open]));
    expect(feedback.observe('scene', history([completed]), { ...saved, ...change })).toBe(false);
    expect(feedback.observe('scene', history([completed]), saved)).toBe(false);
  });
  it.each(['failed', 'cancelled', 'pending_assistant'] as const)('requires a completed persisted turn, not %s', status => {
    const feedback = createYeyeFeedback(); feedback.observe('scene', history([open]));
    expect(feedback.observe('scene', history([completed], status), saved)).toBe(false);
  });
  it('uses history, a switched chat and a remounted reader only as baselines', () => {
    const feedback = createYeyeFeedback();
    expect(feedback.observe('scene', history([completed]), saved)).toBe(false);
    feedback.observe('scene', history([{ ...open, revision: 3 }]));
    expect(feedback.observe('another-chat', history([{ ...completed, revision: 4 }]), saved)).toBe(false);
    feedback.reset();
    expect(feedback.observe('scene', history([{ ...completed, revision: 4 }]), saved)).toBe(false);
    expect(createYeyeFeedback().observe('scene', history([completed]), saved)).toBe(false);
  });
  it('does not replay a completion consumed by a plain history read or while the host is hidden', () => {
    const feedback = createYeyeFeedback(); feedback.observe('scene', history([open]));
    expect(feedback.observe('scene', history([completed]))).toBe(false);
    expect(feedback.observe('scene', history([completed]), saved)).toBe(false);
  });
  it.each(['book_id', 'session_id', 'turn_id'] as const)('rejects a saved run from another %s', field => {
    const feedback = createYeyeFeedback(); feedback.observe('scene', history([open]));
    expect(feedback.observe('scene', history([completed]), { ...saved, descriptor: { ...saved.descriptor, [field]: 'other' } })).toBe(false);
  });
  it('coalesces multiple completed goals into one feedback and permits a later observed completion', () => {
    const feedback = createYeyeFeedback(); feedback.observe('scene', history([open, { ...open, id: 'second' }]));
    expect(feedback.observe('scene', history([completed, { ...completed, id: 'second' }]), saved)).toBe(true);
    feedback.observe('scene', history([{ ...open, id: 'third' }]));
    expect(feedback.observe('scene', history([{ ...completed, id: 'third' }]), saved)).toBe(true);
  });
});
