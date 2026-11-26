import { describe, expect, it } from "vitest";
import { setLocale } from './i18n';
import { agentFailureMessage, completedTeachingStep, initialRun, interruptRun, reduceRun, runStatusText, type RunActivity, type RunEvent } from "./agent-run-state";
const descriptor = { book_id: "book", session_id: "session", turn_id: "turn" };

it.each(['HTTP_413', 'REQUEST_TOO_LARGE'])('explains an oversized question without hiding the recovery action (%s)', error_code => {
  expect(agentFailureMessage({ error_code, category: 'validation', message: 'Request Entity Too Large' }))
    .toBe('图片或附件过大，请缩小后重新发送。');
});

it('explains that invalid video context rejected the question before answering', () => {
  const error = { error_code: 'VIDEO_DRAFT_INVALID', category: 'conflict', message: 'A video question needs its input-time draft' };
  expect(agentFailureMessage(error)).toBe('问题未提交：视频提问内容无效，请检查所选画面或附件后重新发送。');
  setLocale('en');
  try {
    expect(agentFailureMessage(error)).toBe('Question not submitted: the video context is invalid. Check the selected frame or attachment, then send again.');
  } finally { setLocale('zh-CN'); }
});

it('keeps raw tool diagnostics out of both known and unknown terminal failures', () => {
  const message = '$.move must satisfy anyOf; $.move.delivery_text is required';
  for (const error_code of ['TOOL_ARGUMENTS_INVALID', 'AGENT_NO_PROGRESS', 'FUTURE_INTERNAL_ERROR']) {
    const error = { error_code, category: 'validation', message };
    expect(agentFailureMessage(error)).toBe('这次回答未能完成，请重试。');
  }
});
it('localizes the public failure without translating or exposing its diagnostic', () => {
  setLocale('en');
  try {
    expect(agentFailureMessage({ error_code: 'FUTURE_INTERNAL_ERROR', category: 'validation', message: '$.move is required' }))
      .toBe('This answer could not be completed. Please try again.');
  } finally { setLocale('zh-CN'); }
});
const activity: RunActivity = { step_id: 1, parent_step_id: null, kind: "tool", name: "book.text", label: "读取原文", status: "running", started_ms: 5, duration_ms: null, error_code: null, result_count: null, usage_total_tokens: null, usage: null, model_first_text_ms: null, model_name: null, model_name_source: null, accepted_evidence_count: null, evidence_refs: [] };
const event = (seq: number, type: string, payload: unknown): RunEvent => ({ turn_id: "turn", seq, type, payload, elapsed_ms: seq });

it('invalidates a submitted answer on a new successful tutor step while the run is still active', () => {
  const assessing = { ...activity, name: 'tutor.step' };
  const saved = { ...assessing, status: 'succeeded' as const };
  expect(completedTeachingStep([], [assessing])).toBe(false);
  expect(completedTeachingStep([assessing], [saved])).toBe(true);
  expect(completedTeachingStep(undefined, [saved])).toBe(true);
  expect(completedTeachingStep([saved], [saved, { ...activity, step_id: 2 }])).toBe(false);
});

it('shows the current work phase while preserving connection and terminal status', () => {
  const state = { ...initialRun(descriptor), execution_state: 'running' as const, activities: [activity] };
  expect(runStatusText(state, 'connected')).toBe('读取原文');
  expect(runStatusText(state, 'reconnecting')).toBe('连接中断，正在恢复活动');
  expect(runStatusText({ ...state, execution_state: 'completed' }, 'connected')).toBe('运行已结束');
});

it('reports the actual teach-back stage ahead of nested model activity', () => {
  const extract = { ...activity, kind: 'teach_back', name: 'organize_expression', label: '正在整理你的表达' };
  const compare = { ...extract, step_id: 3, name: 'compare_passages', label: '正在核对书中原文与对照' };
  let state = reduceRun(initialRun(descriptor), event(1, 'teach_back.started', extract));
  state = reduceRun(state, event(2, 'model.started', { ...activity, step_id: 2, kind: 'model' }));
  expect(runStatusText(state, 'connected')).toBe(extract.label);
  state = reduceRun(state, event(3, 'teach_back.finished', { ...extract, status: 'succeeded' }));
  state = reduceRun(state, event(4, 'teach_back.started', compare));
  expect(runStatusText(state, 'connected')).toBe(compare.label);
  expect(state.draft).toBeUndefined();
});

it('explains the two recorded Embedding failures without rewriting their diagnostic data', () => {
  const legacy = { error_code: 'TEACHING_INVALID', category: 'validation', message: '教学设计或动作尚未就绪，无法将预算终答标为已交付' };
  const author = { effects: [], trace: [{ model_tool_loop: 12, tool: 'presentation.author', args: '{"operation":null}', result_digest: '{"error_code":"PRESENTATION_ARGUMENTS_INVALID","message":"$.operation is required"}' }] };
  expect(agentFailureMessage(legacy, author)).toBe('演示页制作未完成。请重试。');
  const reviewed = { effects: [], trace: [{ model_tool_loop: 18, tool: 'presentation.author', args: '{"operation":"deliver"}', result_digest: '{"reference":{"presentation_id":"saved","revision":1},"status":"version_saved"}' }],
    activities: [{ ...activity, kind: 'model' as const, name: 'tutor_delivery_review', status: 'failed' as const, error_code: 'PROVIDER_ERROR' }] };
  expect(agentFailureMessage(legacy, reviewed)).toBe('演示页已生成，但回答检查未完成。请重试。');
  expect(agentFailureMessage(legacy, { ...reviewed, activities: [] })).toBe('演示页已生成，但回答检查未完成。请重试。');
  const failure = { error_code: 'TEACHING_REVIEW_FAILED', category: 'provider', message: 'PROVIDER_ERROR: finish_reason=length' };
  expect(agentFailureMessage(failure, reviewed)).toBe(agentFailureMessage(legacy, reviewed));
  expect(failure.message).toBe('PROVIDER_ERROR: finish_reason=length');
  expect(agentFailureMessage({ error_code: 'UNAVAILABLE', category: 'provider', message: '连接暂时不可用' })).toBe('服务暂时不可用，请稍后重试。');
  expect(agentFailureMessage({ error_code: 'MODEL_OUTPUT_LIMIT', category: 'provider', message: 'finish_reason=length' })).toBe('本次生成达到单次输出上限。');
});
describe("Resident run reducer", () => {
  it("keeps commentary between tool steps through replay, cancellation and snapshot recovery", () => {
    const message = { ...activity, step_id: 2, kind: "commentary", name: "agent.commentary", label: "已读完这一节，接下来制作演示。", status: "succeeded" as const, started_ms: null };
    let state = reduceRun(initialRun(descriptor), event(1, "tool.finished", { ...activity, status: "succeeded" }));
    state = reduceRun(state, event(2, "commentary.finished", message));
    state = reduceRun(state, event(3, "tool.started", { ...activity, step_id: 3, name: "presentation.author" }));
    expect(state.activities.map(item => item.step_id)).toEqual([1, 2, 3]);
    expect(reduceRun(state, event(2, "commentary.finished", message))).toBe(state);
    const stopped = reduceRun(state, event(4, "run.cancelled", { ...state, execution_state: "cancelled", persistence_state: "saved" }));
    const restored = reduceRun(initialRun(descriptor), event(4, "run.snapshot", stopped));
    expect(restored.activities[1]).toEqual(message);
    expect(restored.execution_state).toBe("cancelled");
  });
  it("merges live and snapshot activities without duplicate steps or stale replacement", () => {
    const started = reduceRun(initialRun(descriptor), event(2, "tool.started", activity));
    const finished = reduceRun(started, event(3, "tool.finished", { ...activity, status: "succeeded", duration_ms: 10 }));
    expect(finished.activities).toHaveLength(1);
    expect(reduceRun(finished, event(2, "tool.started", activity))).toBe(finished);
    const restored = reduceRun(initialRun(descriptor), event(3, "run.snapshot", finished));
    expect(restored).toEqual(finished);
    expect(reduceRun(finished, { ...event(4, "tool.started", activity), turn_id: "other" })).toBe(finished);
    expect(reduceRun(finished, event(4, "run.snapshot", { ...finished, descriptor: { ...descriptor, book_id: "other" } }))).toBe(finished);
  });
  it("keeps failure, cancellation and unsaved outcomes distinct with their activity tree", () => {
    for (const [type, execution, persistence] of [["run.failed", "failed", "saved"], ["run.cancelled", "cancelled", "saved"], ["run.persistence_failed", "ended", "failed"]]) {
      const initial = initialRun(descriptor);
      const stopped = reduceRun(initial, event(9, type, { ...initial, execution_state: execution, persistence_state: persistence, activities: [{ ...activity, status: "cancelled" }] }));
      expect(stopped.persistence_state).toBe(persistence);
      expect(stopped.execution_state).toBe(execution);
      expect(stopped.activities[0].status).toBe("cancelled");
    }
  });
});

it("replaces repair revisions, discards tool candidates and restores draft snapshots", () => {
  const patch = (revision: number, text: string) => ({ message_id: 1, revision, operation: "replace", view: { parts: [{ kind: "markdown", text }], sources: [] } });
  const draft = reduceRun(initialRun(descriptor), event(1, "answer.patch", patch(0, "原草稿。")));
  expect(reduceRun(initialRun(descriptor), event(1, "run.snapshot", draft))).toEqual(draft);
  const repair = reduceRun(draft, event(2, "answer.patch", patch(1, "修复。")));
  expect(repair.draft?.view?.parts).toEqual([{ kind: "markdown", text: "修复。" }]);
  expect(reduceRun(repair, event(3, "answer.patch", patch(0, "旧草稿。"))).draft).toEqual(repair.draft);
  const discarded = reduceRun(repair, event(4, "answer.patch", { message_id: 1, revision: 1, operation: "discard", view: null }));
  expect(discarded.draft?.view).toBeNull();
  const saved = reduceRun(discarded, event(5, "run.completed", { ...discarded, draft: null, execution_state: "completed", persistence_state: "saved" }));
  expect(saved.draft).toBeNull();
});

it("materializes same-revision appends without mutating the earlier snapshot", () => {
  const view = (text: string) => ({ parts: [{ kind: "markdown", text }], sources: [] });
  const original = reduceRun(initialRun(descriptor), event(1, "answer.patch", { message_id: 1, revision: 0, operation: "replace", view: view("第一句。") }));
  const appended = reduceRun(original, event(2, "answer.patch", { message_id: 1, revision: 0, operation: "append", view: view("第二句。") }));
  expect(appended.draft?.view).toEqual(view("第一句。第二句。"));
  expect(original.draft?.view).toEqual(view("第一句。"));
  expect(reduceRun(appended, event(3, "answer.patch", { message_id: 1, revision: 1, operation: "append", view: view("wrong revision") })).draft).toEqual(appended.draft);
});

it("deduplicates effects and rejects older or cross-book Reader revisions", () => {
  const effect = { effect_id: "turn:2", effect: { kind: "Note", mem_id: "note", lid: "1.1", text: "saved" } };
  let state = reduceRun(initialRun(descriptor), event(1, "effect.created", effect));
  state = reduceRun(state, event(2, "effect.created", effect));
  expect(state.effects).toEqual([effect]);
  state = reduceRun(state, event(3, "reader.changed", { book_id: "book", revision: 4 }));
  state = reduceRun(state, event(4, "reader.changed", { book_id: "book", revision: 3 }));
  state = reduceRun(state, event(5, "reader.changed", { book_id: "other", revision: 9 }));
  expect(state.reader_state).toEqual({ book_id: "book", revision: 4 });
  expect(reduceRun(initialRun(descriptor), event(5, "run.snapshot", state))).toEqual(state);
});

it("marks an unverifiable persisted run interrupted without changing its identity or sequence", () => {
  const running = { ...initialRun(descriptor), last_seq: 7 };
  const interrupted = interruptRun(running, {
    error_code: "AGENT_RUN_NOT_FOUND",
    category: "not_found",
    message: "Run does not exist",
  });
  expect(interrupted.descriptor).toEqual(descriptor);
  expect(interrupted.last_seq).toBe(7);
  expect(interrupted.execution_state).toBe("interrupted");
  expect(interrupted.persistence_state).toBe("failed");
  expect(interrupted.error?.error_code).toBe("AGENT_RUN_NOT_FOUND");
});

it('retains committed board parts across draft clearing, replay, epoch reset and an incomplete terminal result', () => {
  const first = { board_id: 'board', commit_id: 'part-one', before_revision: 1, after_revision: 2, item_ids: ['answer-one'], block_ids: ['block-one'], origin_session_id: 'session', origin_turn_id: 'turn', parent_item_id: 'point' };
  const second = { ...first, commit_id: 'part-two', before_revision: 2, after_revision: 3, item_ids: ['answer-two'], block_ids: ['block-two'] };
  let state = reduceRun(initialRun(descriptor), event(1, 'board.committed', { delivery: first }));
  state = reduceRun(state, event(2, 'board.committed', { delivery: first }));
  state = reduceRun(state, event(3, 'answer.patch', { message_id: 1, revision: 1, operation: 'discard', view: null }));
  expect(state.board_commits).toEqual([first]);
  state = reduceRun(state, event(3, 'run.snapshot', { ...initialRun(descriptor), board_commits: [second] }));
  expect(state.board_commits).toEqual([first, second]);
  expect(state.last_seq).toBe(3);
  state = reduceRun(state, { ...event(0, 'run.snapshot', { ...initialRun(descriptor), board_commits: [second] }), observation_epoch: 'new-buffer', live_buffer_reset: true });
  expect(state.board_commits).toEqual([first, second]);
  state = reduceRun(state, { ...event(1, 'run.cancelled', { ...initialRun(descriptor), execution_state: 'cancelled', persistence_state: 'saved', board_commits: [first] }), observation_epoch: 'new-buffer' });
  expect(state.board_commits).toEqual([first, second]);
  expect(state.execution_state).toBe('cancelled');
});

it('treats explicit user cancellation as a normal stop while retaining real failures', () => {
  expect(agentFailureMessage({ error_code: 'AGENT_RUN_CANCELLED', category: 'cancelled', message: 'Agent run was cancelled' })).toBeUndefined();
  expect(agentFailureMessage({ error_code: 'PROVIDER_ERROR', category: 'provider', message: '服务暂时失败' })).toBe('服务暂时不可用，请稍后重试。');
  expect(agentFailureMessage({ error_code: 'TURN_UNSAVED', category: 'storage', message: '结果保存失败' })).toBe('结果保存失败，请重试保存。');
});
