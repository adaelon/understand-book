import test from 'node:test';
import assert from 'node:assert/strict';
import { candidatePrompt, observedMaterials, completionState, comparisonJobs, reportMarkdown } from './core.mjs';

test('original question stays exact and review answers do not leak to candidates', () => {
  const task = { question: '原题？', assessment: 'hidden criterion', reference_facts: ['hidden answer'] };
  assert.equal(candidatePrompt(task), '原题？');
  assert(!candidatePrompt(task, [{ content: 'source' }]).includes('hidden'));
});

test('same-material collection excludes candidate answers and deduplicates repeated tool history', () => {
  const messages = [{ role: 'assistant', content: 'candidate answer', tool_calls: [{ id: 't1', function: { name: 'music.read', arguments: '{"measure":"1"}' } }] },
    { role: 'tool', tool_call_id: 't1', content: 'actual observed source' }];
  assert.deepEqual(observedMaterials([{ request: { messages } }, { request: { messages } }]),
    [{ tool: 'music.read', arguments: '{"measure":"1"}', content: 'actual observed source' }]);
});

test('truncated and failed answers are not delivered even when text exists', () => {
  assert.equal(completionState({ answer: 'partial', finishReason: 'length' }), 'incomplete');
  assert.equal(completionState({ answer: 'partial', error: 'timeout' }), 'error');
  assert.equal(completionState({ answer: ' ', finishReason: 'stop' }), 'empty');
  assert.equal(completionState({ answer: 'done', finishReason: 'stop' }), 'delivered');
});

test('native Messages tool results are usable without passing assistant answers or thinking', () => {
  const records = [{ request: { messages: [
    { role: 'assistant', content: [{ type: 'text', text: 'do not pass' }, { type: 'tool_use', id: 'x', name: 'music_read', input: { measure: 1 } }] },
    { role: 'user', content: [{ type: 'tool_result', tool_use_id: 'x', content: [{ type: 'text', text: 'notes' }] }] },
  ] } }];
  assert.deepEqual(observedMaterials(records), [{ tool: 'music_read', arguments: '{"measure":1}', content: [{ type: 'text', text: 'notes' }] }]);
});

test('blind inputs retain question context and observed evidence, exchange both orders and keep failures in the report', () => {
  const run = { cases: [{ id: 'M1', question: 'Q', context: '原文限定条件', baseline_mode: 'knowledge', assessment: 'explain' }], samples: [
    { case_id: 'M1', arm: 'product', answer: 'one', status: 'delivered' },
    { case_id: 'M1', arm: 'astra', answer: 'two', status: 'delivered' },
    { case_id: 'M1', arm: 'same_model', answer: '', status: 'error', error: 'timeout' }] };
  const jobs = comparisonJobs(run, { M1: [{ content: '工具实际返回的原文' }] });
  assert.equal(jobs.length, 2);
  assert.deepEqual(jobs.map(job => job.mapping.A), ['product', 'astra']);
  assert(!JSON.stringify(jobs.map(job => job.input)).includes('astra'));
  assert.equal(jobs[0].input.context, '原文限定条件');
  assert.equal(jobs[1].input.observed_materials[0].content, '工具实际返回的原文');
  assert(reportMarkdown(run).includes('same_model'));
  assert(reportMarkdown(run).includes('timeout'));
});
