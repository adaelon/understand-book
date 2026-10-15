import assert from 'node:assert/strict';
import test from 'node:test';
import { sumUsage, judgeRainwater, judgeResident, aggregate } from './report.mjs';
import { summarizeEvents } from './codex-client.mjs';

test('missing usage remains unknown and cached input is not billed twice', () => {
  const value = sumUsage([{ usage: { input_tokens: 100, cached_input_tokens: 70, output_tokens: 8 } }, {}]);
  assert.equal(value.total_tokens, 108); assert.equal(value.complete, false); assert.equal(value.actual_cost, null);
  assert.equal(summarizeEvents([{ type: 'turn.failed' }]).completed, false);
  assert.equal(summarizeEvents([]).total_tokens, null);
});

test('a recovered transport error does not discard a completed Codex turn', () => {
  const r = summarizeEvents([{ type: 'error', message: 'Reconnecting after 503' },
    { type: 'item.completed', item: { type: 'agent_message', text: '{"text":"done","tool_calls":[]}' } },
    { type: 'turn.completed', usage: { input_tokens: 10, output_tokens: 2 } }]);
  assert.equal(r.completed, true); assert.equal(r.total_tokens, 12);
  assert.deepEqual(r.recovered_errors, ['Reconnecting after 503']);
  assert.equal(summarizeEvents([{ type: 'turn.completed', usage: { input_tokens: 1, output_tokens: 1 } }, { type: 'turn.failed' }]).completed, false);
});

test('unstarted and failed tasks remain in the fixed denominator', () => {
  const r = aggregate([{ verdict: 'pass' }, { verdict: 'fail' }, { verdict: 'unresolved' }]);
  assert.equal(r.count, 3); assert.equal(r.pass, 1); assert.equal(r.fail, 1); assert.equal(r.unresolved, 1);
});

test('correct-looking rainwater answer without actual cited pages fails', () => {
  const verdict = judgeRainwater({ rows: [], metrics: { completed: true, final: '37 降到 19，减少 18；南池未参与 https://reader.example/?made-up=1' },
    evidence: { calls: [], source_text: '', reference: {}, usage: { free_remaining: 12 }, gift_units: 12 } });
  assert.equal(verdict.verdict, 'fail'); assert.equal(verdict.requirements.citations_read, false);
});

test('mentioning all three numbers does not prove the requested decrease', () => {
  const r = judgeRainwater({ rows: [], metrics: { completed: true, final: '37 变为 19，减少 19 升；18 是其他数据。南池参与情况不同。' },
    evidence: { calls: [], source_text: '', reference: {}, usage: { free_remaining: 12 }, gift_units: 12 } });
  assert.equal(r.requirements.numeric_answer, false); assert.equal(r.requirements.south_excluded, false);
});

test('saved goal or prose alone cannot replace a teaching delivery', () => {
  const result = { case: 'tutor-continue', http_status: 200, reply: { answer: 'Done', incomplete: false },
    deliveries: [], events: [{ kind: 'move_selected' }], reopened_records: [] };
  assert.equal(judgeResident(result, []).verdict, 'fail');
});

test('actual Runtime Tool role and duplicate request history count one rejection', () => {
  const message = { role: 'Tool', tool_call_id: 'one', content: JSON.stringify({ status: 'error', model_body: { error_code: 'TOOL_NOT_EXPOSED' } }) };
  const result = { case: 'save-note', http_status: 200, reply: { answer: 'done', incomplete: false }, reopened_records: [] };
  assert.equal(judgeResident(result, [{ input: [] }, { input: [message] }, { input: [message] }]).tool_rejections, 1);
});

test('discarded recovered client output is an invalid trial, not a product rejection', () => {
  const result = { case: 'tutor-continue', http_status: 502, reply: { error_code: 'PROVIDER_ERROR', message: 'CODEX_SAMPLE_FAILED' }, deliveries: [], events: [] };
  const r = judgeResident(result, [], [{ completed: false, exit_code: 0, timed_out: false, final: '{}', total_tokens: 12 }]);
  assert.equal(r.verdict, 'unresolved'); assert.equal(r.assessment_status, 'invalid_harness');
});

test('wrong note text and incomplete execution remain failures', () => {
  const result = { case: 'save-note', http_status: 200, reply: { answer: 'Saved', incomplete: true },
    reopened_records: [{ type: 'note', content: 'other', anchor: { lid: '1.1' }, note_placement: { lid: '1.1' } }] };
  assert.equal(judgeResident(result, []).verdict, 'fail');
});

test('a rejected write attempt still violates the explain-only tool boundary', () => {
  const result = { case: 'explain-only', http_status: 200, reply: { answer: 'explanation', incomplete: false,
    effects: [], answer_view: { sources: [{}] } }, reopened_records: [] };
  const r = judgeResident(result, [{ input: [] }, { input: [
    { role: 'Assistant', tool_calls: [{ id: 'write1', name: 'reader.note' }] },
    { role: 'Tool', tool_call_id: 'write1', content: JSON.stringify({ status: 'error' }) },
  ] }]);
  assert.equal(r.requirements.read_only, false);
  assert.deepEqual(r.tool_names, ['reader.note']);
});

test('explain-only sources must be the actually presented selected text', () => {
  const input = { source: 'selected text', input: { question_quote: {
    resolved_quote: 'selected text', ranges: [{ range: { start: 0, end: 13 } }] } } };
  const result = { case: 'explain-only', http_status: 200, reply: { answer: 'meaning', incomplete: false,
    effects: [], answer_view: { sources: [{ source_ref_id: 'source1' }] } }, reopened_records: [] };
  const requests = [{ input: [] }, { input: [{ role: 'Tool', tool_call_id: 'present1', content: JSON.stringify({
    status: 'ok', model_body: { source_ref_id: 'source1', preview: 'selected text' }, receipt: { tool: 'source.present' } }) }] }];
  assert.equal(judgeResident(result, requests, [], input).verdict, 'pass');
  result.reply.answer_view.sources[0].source_ref_id = 'unread';
  assert.equal(judgeResident(result, requests, [], input).requirements.selected_source, false);
});
