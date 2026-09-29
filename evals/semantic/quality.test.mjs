import test from 'node:test';
import assert from 'node:assert/strict';
import { qaTasks, version } from './agent-dataset.mjs';
import { CHUNK, AGENT } from './agent-core.mjs';
import { execution, anonymize, prepareBundle, judgeJobs, judgeMessages, validateJudgment, gradeEvaluation, qualityReport, assessCalibration, qualityVersion } from './quality-core.mjs';

function fixture() {
  const source = qaTasks.flatMap(t => t.slots.flatMap(s => s.evidence.map(g => g[0]))).join('\n');
  const corpus = { source, base: { book_id: 'test' }, leaves: [{ start: 0, end: source.length }] };
  const qa = qaTasks.flatMap(t => [CHUNK, AGENT].map(system => ({ id: t.id, category: t.category, system,
    answer: '结论正确。[[source:source_ref_original]]', sources: [{ id: 'source_ref_original', valid: true, text: source }], blocks: [],
    outcome: { incomplete: false }, score: { success: false }, requests: [{ credential: 'must-never-leak' }], usage: { total_tokens: 100 } })));
  return { corpus, run: { version, corpus: { book_id: 'test', source_utf16: source.length }, qa } };
}
function judgment(job, preference = 'tie') {
  return { evaluations: job.input.candidates.map(c => ({ candidate: c.candidate,
    facts: job.input.required_facts.map(f => ({ key: f.key, value: f.acceptable[0], answer_spans: ['a1'], support_ids: ['C1'], reason: '匹配该事实' })),
    refusal: { appropriate: !job.input.answerable, fabricated: false, answer_spans: ['a1'] },
    dimensions: Object.fromEntries(['accuracy', 'completeness', 'grounding', 'explanation'].map(name => [name, {
      verdict: name === 'explanation' && !job.input.explanation_required ? 'not_applicable' : 'met',
      answer_spans: ['a1'], evidence_ids: job.input.reference_evidence.length ? ['R1'] : [], reason: '原文支持' }])), critical_errors: [] })),
    preference: job.input.candidates.length === 2 ? preference : 'not_applicable', reason: '依据相同' };
}
test('preparation preserves every task and hides system, cost, old scores, credentials and original citation prefixes', () => {
  const { run, corpus } = fixture(); const original = JSON.stringify(run);
  const bundle = prepareBundle(run, corpus, () => 0), jobs = judgeJobs(bundle);
  assert.equal(bundle.audit.length, 48); assert.equal(jobs.length, 48);
  for (const job of jobs) {
    const text = JSON.stringify(judgeMessages(job));
    for (const secret of [CHUNK, AGENT, 'historical_success', 'must-never-leak', 'source_ref_original', 'total_tokens', 'sample_id']) assert(!text.includes(secret), secret);
  }
  assert.equal(JSON.stringify(run), original, 'source records must be immutable');
});
test('missing/duplicate task and mismatched source stop preparation', () => {
  const { run, corpus } = fixture();
  assert.throws(() => prepareBundle({ ...run, qa: run.qa.slice(1) }, corpus), /complete/);
  const duplicate = structuredClone(run); duplicate.qa[0] = duplicate.qa[1];
  assert.throws(() => prepareBundle(duplicate, corpus), /Missing or duplicate/);
  assert.throws(() => prepareBundle(run, { ...corpus, source: corpus.source + 'changed' }), /Corpus mismatch/);
});
test('citation renaming handles overlapping names and never upgrades bare IDs, invalid or noncanonical text', () => {
  const row = { answer: '[[source:chunk-1]] [[source:chunk-10]] source_ref_bare [[source:fake]]', sources: [
    { id: 'chunk-1', valid: true, text: '原文' }, { id: 'chunk-10', valid: true, text: '其他原文' },
    { id: 'source_ref_bare', valid: true, text: '原文' }, { id: 'fake', valid: true, text: '伪造' }] };
  const anon = anonymize(row, '原文，其他原文');
  assert.equal(anon.answer_spans[0].text, '[[source:C1]] [[source:C2]] C4 [[source:C3]]');
  assert.deepEqual(anon.citations.map(c => c.valid), [true, true, false, false]);
});
test('new source view preserves saved highlight boundaries and never fills missing historical context', () => {
  const row = { answer: '结论 [[source:source_ref_short]] [[source:chunk-2]] [[source:source_ref_old]]', sources: [
    { id: 'source_ref_short', valid: true, text: '排序', resolved: { highlighted_quote: '排序',
      context_before: '正确的做法是对齐 IC /', context_after: '。最直接的是 IC loss。' } },
    { id: 'chunk-2', valid: true, text: '整块正文', resolved: { highlighted_quote: '整块正文', context_before: '', context_after: '' } },
    { id: 'source_ref_old', valid: true, text: '旧片段' },
  ] };
  const source = '正确的做法是对齐 IC /排序。最直接的是 IC loss。整块正文旧片段';
  const modern = anonymize(row, source, { sourceView: true });
  assert.deepEqual(modern.citations, [
    { id: 'C1', valid: true, highlighted_quote: '排序', context_before: '正确的做法是对齐 IC /', context_after: '。最直接的是 IC loss。', view_status: 'complete' },
    { id: 'C2', valid: true, highlighted_quote: '整块正文', context_before: '', context_after: '', view_status: 'complete' },
    { id: 'C3', valid: true, highlighted_quote: '旧片段', context_before: '', context_after: '', view_status: 'partial' },
  ]);
  assert.equal(anonymize(row, source).citations[0].text, '排序', 'historical scoring keeps its original projection');
  const chunk = anonymize({ system: CHUNK, answer: '[[source:chunk-2]]', sources: [{ id: 'chunk-2', valid: true, text: '整块正文' }] }, source, { sourceView: true });
  assert.equal(chunk.citations[0].view_status, 'complete');
  assert.equal(chunk.citations[0].highlighted_quote, '整块正文');
  const mismatched = anonymize({ answer: '[[source:source_ref_short]]', sources: [{ id: 'source_ref_short', valid: true,
    text: '排序', resolved: { source_ref_id: 'someone-else', highlighted_quote: '排序',
      context_before: '错误前文', context_after: '错误后文' } }] }, source, { sourceView: true });
  assert.equal(mismatched.citations[0].view_status, 'partial');
  assert.equal(mismatched.citations[0].context_after, '');
});
test('one-sided delivery creates one job; product failures and judge failures have different counts', () => {
  const { run, corpus } = fixture(); run.qa[0].outcome.incomplete = true; run.qa[2].error = 'transport';
  run.qa[4].answer = ''; run.qa[5].answer = '';
  assert.equal(execution(run.qa[4]), 'empty');
  const bundle = prepareBundle(run, corpus, () => 0), report = qualityReport(bundle, []);
  assert.equal(judgeJobs(bundle).length, 44);
  assert.equal(report.systems[CHUNK].total, 24);
  assert.equal(report.systems[CHUNK].product_failed, 3);
  assert.equal(report.systems[CHUNK].unscorable, 21);
  assert.equal(report.comparison.filter(p => p.winner === 'not_paired').length, 3);
});
test('fake spans, borrowed reference evidence, invalid applicability and missing facts are grader failures', () => {
  const { run, corpus } = fixture(); const bundle = prepareBundle(run, corpus, () => 0), job = judgeJobs(bundle)[0];
  const good = judgment(job); assert.equal(validateJudgment(job, good), good);
  for (const mutate of [j => j.evaluations[0].facts[0].answer_spans = ['invented'],
    j => j.evaluations[0].facts[0].support_ids = ['R1'], j => j.evaluations.pop(),
    j => j.evaluations[0].facts = [], j => j.evaluations[0].dimensions.explanation.verdict = 'met']) {
    const bad = structuredClone(good); mutate(bad); assert.throws(() => validateJudgment(job, bad));
    assert.equal(qualityReport(bundle, [{ id: job.id, judgment: bad }]).samples[0].quality_status, 'unscorable');
  }
});
test('correct facts do not override an extra false statement or insufficient explanation', () => {
  const { run, corpus } = fixture(); const job = judgeJobs(prepareBundle(run, corpus, () => 0)).find(j => j.input.explanation_required);
  const e = judgment(job).evaluations[0]; assert.equal(gradeEvaluation(job.input, e).status, 'pass');
  e.dimensions.explanation.verdict = 'partial'; assert.equal(gradeEvaluation(job.input, e).status, 'fail');
  e.dimensions.explanation.verdict = 'met'; e.critical_errors.push({ reason: '附带错误', answer_spans: ['a1'], evidence_ids: ['R1'] });
  assert.equal(gradeEvaluation(job.input, e).status, 'fail');
  e.critical_errors = []; e.dimensions.accuracy.verdict = 'unknown';
  assert.equal(gradeEvaluation(job.input, e).status, 'unscorable');
});
test('reversed positions are mapped to the same underlying answer and position bias becomes unresolved', () => {
  const { run, corpus } = fixture(); const bundle = prepareBundle(run, corpus, () => 0), [a, b] = judgeJobs(bundle);
  const records = [{ id: a.id, judgment: judgment(a, 'A'), usage: { total_tokens: 10 } },
    { id: b.id, judgment: judgment(b, 'B'), usage: { total_tokens: 20 } }];
  const report = qualityReport(bundle, records);
  assert.equal(report.comparison[0].system, CHUNK); assert.equal(report.total_tokens, 30);
  records[1].judgment.preference = 'A';
  assert.equal(qualityReport(bundle, records).comparison[0].winner, 'unresolved');
  records[1].usage = null; assert.equal(qualityReport(bundle, records).total_tokens, null);
  assert.throws(() => qualityReport(bundle, [records[0], records[0]]), /Duplicate/);
});
test('conflicting quality judgments are not averaged into a success', () => {
  const { run, corpus } = fixture(); const bundle = prepareBundle(run, corpus, () => 0), [a, b] = judgeJobs(bundle);
  const second = judgment(b); second.evaluations.find(e => e.candidate === 'B').dimensions.accuracy.verdict = 'failed';
  const report = qualityReport(bundle, [{ id: a.id, judgment: judgment(a) }, { id: b.id, judgment: second }]);
  assert.equal(report.samples[0].quality_status, 'unscorable'); assert(report.samples[0].order_disagreement);
});
test('refusal is scored without requiring a citation but must be explicit and not fabricated', () => {
  const { run, corpus } = fixture(); const job = judgeJobs(prepareBundle(run, corpus, () => 0)).find(j => !j.input.answerable);
  const e = judgment(job).evaluations[0]; assert.equal(gradeEvaluation(job.input, e).status, 'pass');
  e.refusal.fabricated = true; assert.equal(gradeEvaluation(job.input, e).status, 'fail');
});

test('human calibration is never inferred, and mismatched facts or changed evidence cannot pass it', () => {
  const { run, corpus } = fixture(); const jobs = judgeJobs(prepareBundle(run, corpus, () => 0));
  const selected = jobs.slice(0, 2), first = judgment(selected[0]);
  const labels = Object.fromEntries(first.evaluations.map(e => [e.candidate, {
    facts: Object.fromEntries(e.facts.map(f => [f.key, f.value])),
    dimensions: Object.fromEntries(Object.entries(e.dimensions).map(([k, d]) => [k, d.verdict])) }]));
  labels.preference = 'tie';
  const packet = { version: qualityVersion, status: 'draft_not_human_gold', reviewer: null, reviewed_at: null,
    cases: [{ case_id: selected[0].case_id, input: selected[0].input, judgments: labels }] };
  const records = selected.map(j => ({ id: j.id, judgment: judgment(j) }));
  assert.equal(assessCalibration(packet, jobs, records).status, 'pending_human_review');
  Object.assign(packet, { status: 'human_reviewed', reviewer: 'test reviewer', reviewed_at: '2026-09-16' });
  assert.equal(assessCalibration(packet, jobs, records).status, 'passed');
  packet.cases[0].judgments.A.facts.purpose = 'wrong';
  assert.equal(assessCalibration(packet, jobs, records).status, 'calibration_disagreement');
  const changed = structuredClone(packet); changed.cases[0].input.question = 'another question';
  assert.throws(() => assessCalibration(changed, jobs, records), /differs/);
});
