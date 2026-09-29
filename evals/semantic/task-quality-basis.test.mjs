import test from 'node:test';
import assert from 'node:assert/strict';
import * as quality from './task-quality.mjs';

const highlight = '原'.repeat(185);
function fixture(basis = [{ source_id: 'C1', part: 'after', quote: '$L=-corr(s,r)$' }]) {
  const job = { protocol: 'reading-task-quality-v2', kind: 'absolute', input: {
    answerable: true, requirements: [{ requirement_id: 'citation:support', kind: 'source_support' }],
    reference_evidence: [{ id: 'R1', text: '仅公共参考里的句子' }],
    candidates: [{ candidate: 'A', answer_spans: [{ id: 'a1', text: '公式与解释。[[source:C1]]' }],
      citations: [{ id: 'C1', valid: true, highlighted_quote: highlight,
        context_before: '此前给出定义。', context_after: '😀公式如下：$L=-corr(s,r)$。重复；重复。' }] }],
  } };
  const wire = { evaluations: [{ candidate: 'A', requirements: [], claim_assessments: [{
    claim_text: '公式与解释', answer_span_ids: ['a1'], support: 'met', location: 'met', basis,
    reason: '后文含公式',
  }] }], preference: 'not_applicable', reason: '单答案' };
  return { job, wire };
}

test('judge wire quotes resolve formula in after with original UTF-16 positions', () => {
  const { job, wire } = fixture(), raw = JSON.stringify(wire);
  const parsed = quality.parseTaskJudgment(job, raw);
  assert.deepEqual(parsed.evaluations[0].claim_assessments[0].basis,
    [{ source_id: 'C1', part: 'after', start: 7, end: 21 }]);
  assert.equal(JSON.stringify(wire), raw);
  quality.validateTaskJudgment(job, parsed);
});

test('185 character highlight cannot receive a model guessed end of 200', () => {
  const { job, wire } = fixture([{ source_id: 'C1', part: 'highlight', quote: highlight }]);
  const parsed = quality.parseTaskJudgment(job, JSON.stringify(wire));
  assert.equal(parsed.evaluations[0].claim_assessments[0].basis[0].end, 185);
  wire.evaluations[0].claim_assessments[0].basis[0] = { source_id: 'C1', part: 'highlight', start: 0, end: 200 };
  assert.throws(() => quality.parseTaskJudgment(job, JSON.stringify(wire)), /verbatim quote/);
});

test('quote resolution rejects missing, ambiguous, wrong-part and public-reference bases', () => {
  for (const basis of [
    { source_id: 'C1', part: 'after', quote: '重复' },
    { source_id: 'C1', part: 'after', quote: '$L=corr(s,r)$' },
    { source_id: 'C1', part: 'highlight', quote: '$L=-corr(s,r)$' },
    { source_id: 'R1', part: 'highlight', quote: '仅公共参考里的句子' },
    { source_id: 'C1', part: 'after', quote: '' },
    { source_id: 'C1', part: 'after', quote: '$L=-corr(s,r)$', start: 0, end: 200 },
  ]) {
    const { job, wire } = fixture([basis]);
    assert.throws(() => quality.parseTaskJudgment(job, JSON.stringify(wire)), /source basis/);
  }
});

test('complementary sources resolve separately and remain attached to the same claim', () => {
  const { job, wire } = fixture([
    { source_id: 'C1', part: 'before', quote: '此前给出定义。' },
    { source_id: 'C2', part: 'highlight', quote: '这里给出必要条件。' },
  ]);
  job.input.candidates[0].citations.push({ id: 'C2', valid: true, highlighted_quote: '这里给出必要条件。' });
  const parsed = quality.parseTaskJudgment(job, JSON.stringify(wire));
  assert.deepEqual(parsed.evaluations[0].claim_assessments[0].basis.map(b => b.source_id), ['C1', 'C2']);
});

test('historical canonical judgments stay readable without reinterpreting stored offsets', () => {
  const { job, wire } = fixture([{ source_id: 'C1', part: 'highlight', start: 0, end: 185 }]);
  assert.equal(quality.validateTaskJudgment(job, wire), wire);
});

test('quote ingestion does not turn a missing basis array into a valid unknown judgment', () => {
  const { job, wire } = fixture();
  const claim = wire.evaluations[0].claim_assessments[0];
  claim.support = 'unknown';
  delete claim.basis;
  assert.throws(() => quality.parseTaskJudgment(job, JSON.stringify(wire)), /Invalid claim/);
});
