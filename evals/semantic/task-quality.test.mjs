import test from 'node:test';
import assert from 'node:assert/strict';
import { qaTasks, sourceTasks, restartTasks, version as legacyVersion } from './agent-dataset.mjs';
import { AGENT, CHUNK } from './agent-core.mjs';
import { productInput, taskSpec, taskSpecs, taskSpecVersion } from './task-spec.mjs';
import { prepareTaskBundle, productState, taskJudgeJobs, taskJudgeMessages, taskQualityReport,
  taskQualityVersionV2, validateTaskJudgment } from './task-quality.mjs';
import { allReadingTasks, readingTasks, readingMultiTurn, readingDatasetVersion, readingSpecs,
  readingProductInput, validateReadingDataset } from './reading-dataset.mjs';
import { approvedFor, annotateTaskCalibration, assessTaskCalibration, calibrationJobs, calibrationPacket } from './task-calibration.mjs';
import { compareV2WithPrior } from './task-quality-v2.mjs';

function fixture() {
  const source = [...qaTasks, ...sourceTasks].flatMap(t => t.slots.flatMap(s => s.evidence.map(g => g[0]))).join('\n');
  const corpus = { source, base: { book_id: 'test' }, leaves: [{ start: 0, end: source.length }] };
  const row = (task, system) => ({ id: task.id, system, category: task.category, answer: '结论。[[source:source_ref_private]]',
    sources: [{ id: 'source_ref_private', valid: true, text: source }], blocks: [], navigation_ok: true,
    outcome: { incomplete: false }, usage: { total_tokens: 10 } });
  return { corpus, run: { version: legacyVersion, corpus: { book_id: 'test', source_utf16: source.length },
    qa: qaTasks.flatMap(t => [CHUNK, AGENT].map(s => row(t, s))),
    navigation: sourceTasks.map(t => row(t, AGENT)),
    restart: restartTasks.map(t => ({ id: t.id, system: AGENT, setup_ok: true, persistence_ok: true,
      new_chat_empty: true, memory_observed_by_agent: true, success: true, usage: { total_tokens: 10 } })) } };
}
function absolute(job, values = {}) {
  const f = job.input.required_facts;
  return { evaluations: [{ candidate: 'A',
    facts: f.map(x => ({ key: x.key, value: values[x.key] ?? x.acceptable[0], answer_spans: ['a1'], support_ids: ['C1'], reason: '答案明确' })),
    refusal: { appropriate: !job.input.answerable, fabricated: false, answer_spans: ['a1'] },
    dimensions: Object.fromEntries(['accuracy', 'completeness', 'grounding', 'explanation'].map(name => [name, {
      verdict: name === 'explanation' && !job.input.explanation_required ? 'not_applicable' : 'met',
      answer_spans: ['a1'], evidence_ids: [], reason: '材料支持' }])), critical_errors: [] }],
    preference: 'not_applicable', reason: '单个答案' };
}

test('report preserves all samples when comparing named product revisions', () => {
  const { run, corpus } = fixture();
  const bundle = prepareTaskBundle(run, corpus);
  for (const row of bundle.audit) row.system = row.system === CHUNK ? 'CQ4 / LID entry' : 'CQ5 / quote entry';
  const report = taskQualityReport(bundle, []);
  for (const system of ['CQ4 / LID entry', 'CQ5 / quote entry']) {
    assert.equal(report.systems[system]?.total, bundle.audit.filter(row => row.system === system).length);
  }
  assert.equal(Object.values(report.systems).reduce((sum, row) => sum + row.total, 0), report.samples.length);
});

test('all frozen old tasks have explicit requirements; level and interaction remain independent', () => {
  assert.equal(taskSpecs.length, 32);
  assert.equal(new Set(taskSpecs.map(s => s.case_id)).size, 32);
  assert.equal(taskSpec('cross-01').reading_level, 'L1', 'parallel chapter facts are not L3');
  assert.equal(taskSpec('cross-02').reading_level, 'L3');
  assert.equal(taskSpec('restart-01').reading_level, 'L1');
  assert.equal(taskSpec('restart-01').interaction_condition, 'cross_session');
  assert.equal(taskSpec('restart-01').recovery_mode, 'new_chat_after_restart');
  assert(!taskSpec('concept-01').requirements.some(r => r.kind === 'explanation'));
  assert(taskSpec('concept-04').requirements.some(r => r.kind === 'explanation'));
  const equivalent = { ...taskSpec('concept-01'), user_input: { question: '均值回归在本文属于哪一类？' } };
  assert.deepEqual(equivalent.requirements, taskSpec('concept-01').requirements);
  assert.equal(equivalent.reading_level, taskSpec('concept-01').reading_level);
});

test('product projection sends only current user material, never frozen gold or future restart step', () => {
  const spec = taskSpec('exact-01');
  const projection = JSON.stringify(productInput(spec));
  assert(!projection.includes('absolute_return'));
  assert(!projection.includes('L1'));
  assert(!projection.includes('evidence_alternatives'));
  const restart = taskSpec('restart-01');
  assert(!JSON.stringify(productInput(restart, 'setup')).includes(restart.user_input.resume));
  assert(!JSON.stringify(productInput(restart, 'resume')).includes(restart.user_input.setup));
  assert.equal(taskSpecVersion, 'quantification-essence-task-spec-v1');
});

test('final repaired delivery overrides incomplete only when actual repair is established', () => {
  const row = { answer: '修复后答案', outcome: { incomplete: true, delivery_diagnostics: { repair: { issues: [] } } } };
  assert.equal(productState(row).delivery, 'established');
  row.outcome.delivery_diagnostics.repair.issues.push({ code: 'SOURCE_NOT_OBSERVED' });
  assert.equal(productState(row).delivery, 'failed');
  delete row.outcome.delivery_diagnostics;
  assert.equal(productState(row).execution, 'incomplete');
});

test('explicit scoring keeps product, absolute and pair states separate', () => {
  const { run, corpus } = fixture();
  run.qa.find(r => r.id === 'contrast-04' && r.system === AGENT).sources = [];
  run.qa.find(r => r.id === 'concept-02' && r.system === AGENT).outcome.incomplete = true;
  const bundle = prepareTaskBundle(run, corpus, () => 0), jobs = taskJudgeJobs(bundle);
  assert.equal(bundle.cases.length, 32);
  assert.equal(bundle.audit.length, 56);
  const citationSample = bundle.audit.find(a => a.task_id === 'contrast-04' && a.system === AGENT);
  const job = jobs.find(j => j.kind === 'absolute' && j.mapping.A === citationSample.sample_id);
  const invalid = { id: job.id, error: 'invalid_judgment: JSON parse failed', raw_judgment: '{', usage: { total_tokens: 10 } };
  let report = taskQualityReport(bundle, [invalid]);
  assert.equal(report.samples.find(s => s.sample_id === citationSample.sample_id).task_result, 'unresolved');
  assert.equal(report.samples.find(s => s.sample_id === citationSample.sample_id).score_status, 'invalid_output');
  assert.equal(report.samples.find(s => s.task_id === 'concept-02' && s.system === AGENT).task_result, 'fail');
  const judgment = absolute(job);
  judgment.evaluations[0].facts[0].support_ids = [];
  judgment.evaluations[0].dimensions.grounding.verdict = 'failed';
  report = taskQualityReport(bundle, [{ id: job.id, judgment },
    { id: jobs.find(j => j.kind === 'preference' && j.case_id === job.case_id).id,
      error: 'judge_timeout' }]);
  const sample = report.samples.find(s => s.sample_id === citationSample.sample_id);
  assert.equal(sample.task_result, 'fail');
  assert.equal(sample.requirements.find(r => r.kind === 'content').verdict, 'met');
  assert.equal(sample.requirements.find(r => r.kind === 'citation').verdict, 'unmet');
  assert.equal(sample.score_status, 'valid');
  assert.equal(sample.preference, 'invalid');
  assert.equal(report.systems[AGENT].total, 32);
  assert.equal(report.systems[CHUNK].total, 24);
});

test('a confirmed necessary failure wins over another unknown; judge input has no historical identity', () => {
  const { run, corpus } = fixture(), bundle = prepareTaskBundle(run, corpus, () => 0);
  const job = taskJudgeJobs(bundle).find(j => j.kind === 'absolute' && j.case_id === 'T01');
  const message = JSON.stringify(taskJudgeMessages(job));
  for (const hidden of [CHUNK, AGENT, 'historical_success', 'source_ref_private', 'absolute_return'])
    if (hidden !== 'absolute_return') assert(!message.includes(hidden), hidden);
  const judgment = absolute(job, { purpose: 'derivative_pricing' });
  judgment.evaluations[0].dimensions.grounding.verdict = 'unknown';
  const report = taskQualityReport(bundle, [{ id: job.id, judgment }]);
  const sample = report.samples.find(s => s.sample_id === job.mapping.A);
  assert.equal(sample.task_result, 'fail');
  assert.equal(sample.requirements.find(r => r.kind === 'content').verdict, 'unmet');
  assert.equal(sample.requirements.find(r => r.kind === 'citation').verdict, 'unknown');
});

test('action observations and paired preference have their own outcomes', () => {
  const { run, corpus } = fixture();
  run.navigation[0].navigation_ok = false;
  run.restart[0].persistence_ok = false; run.restart[0].success = false;
  const bundle = prepareTaskBundle(run, corpus, () => 0), jobs = taskJudgeJobs(bundle);
  const nav = bundle.audit.find(a => a.task_id === 'source-01');
  const pair = jobs.filter(j => j.kind === 'preference' && j.case_id === 'T01');
  const records = [
    { id: pair[0].id, judgment: { preference: 'A', reason: '第一份较完整' } },
    { id: pair[1].id, judgment: { preference: 'B', reason: '第二份较完整' } },
  ];
  let report = taskQualityReport(bundle, records);
  assert.equal(report.samples.find(s => s.sample_id === nav.sample_id).requirements.find(r => r.kind === 'action').verdict, 'unmet');
  assert.equal(report.samples.find(s => s.task_id === 'restart-01').requirements.find(r => r.kind === 'persistence').verdict, 'unmet');
  assert.equal(report.preference.find(p => p.case_id === 'T01').status, 'valid');
  assert.equal(report.samples.find(s => s.task_id === 'exact-01').task_result, 'unresolved', 'preference does not pass absolute task');
  records[1].judgment.preference = 'A';
  report = taskQualityReport(bundle, records);
  assert.equal(report.preference.find(p => p.case_id === 'T01').status, 'judgment_disagreement');
});

function readingFixture() {
  const texts = allReadingTasks.flatMap(t => t.slots.flatMap(s => s.evidence.flat()));
  const source = texts.join('\n');
  let at = 0;
  const leaves = texts.map(text => { const leaf = { start: at, end: at + text.length }; at += text.length + 1; return leaf; });
  const corpus = { source, base: { book_id: 'reading-test' }, leaves };
  const row = (task, system) => ({ id: task.id, system, answer: '答案。', sources: [], outcome: { incomplete: false } });
  const run = { version: readingDatasetVersion, corpus: { book_id: corpus.base.book_id, source_utf16: source.length },
    qa: [...readingTasks.flatMap(t => [row(t, CHUNK), row(t, AGENT)]), ...readingMultiTurn.map(t => row(t, AGENT))],
    navigation: [], restart: [] };
  return { corpus, run };
}

test('layered reading tasks preserve family split and reveal multi-turn user input one step at a time', () => {
  const { corpus } = readingFixture();
  assert.equal(validateReadingDataset(corpus).tasks, 10);
  assert.equal(readingSpecs.filter(s => s.interaction_condition === 'multi_turn').length, 2);
  for (const family of new Set(readingSpecs.map(s => s.family_id)))
    assert.equal(new Set(readingSpecs.filter(s => s.family_id === family).map(s => s.split)).size, 1);
  for (const level of ['L1', 'L2', 'L3', 'L4'])
    assert.equal(readingTasks.filter(t => t.reading_level === level).length, 2);
  const spec = readingSpecs.find(s => s.case_id === 'holdout-multi');
  assert(!JSON.stringify(readingProductInput(spec, 0)).includes('反复拿来筛因子'));
  assert(JSON.stringify(readingProductInput(spec, 1)).includes('反复拿来筛因子'));
  assert(!JSON.stringify(readingProductInput(spec, 0)).includes('fresh_holdout_not_repaired_by_purge'));
  assert.throws(() => readingProductInput(spec, 2), /Unknown user step/);
});

test('reading protocol prepares eight paired single-turn tasks and two same-chat Agent tasks', () => {
  const { run, corpus } = readingFixture(), bundle = prepareTaskBundle(run, corpus, () => 0);
  assert.equal(bundle.cases.length, 10);
  assert.equal(bundle.audit.length, 18);
  assert.equal(bundle.cases.filter(c => c.interaction_condition === 'multi_turn').length, 2);
  assert.equal(taskJudgeJobs(bundle).length, 34);
  assert(bundle.cases.find(c => c.task_id === 'holdout-l3').reference_evidence.length >= 2);
  assert(bundle.cases.find(c => c.task_id === 'target-l3').requirements.some(r => r.kind === 'explanation'));
});

test('task-v2 keeps all 18 samples and separates semantic claims, support and citation location', () => {
  const { run, corpus } = readingFixture();
  const row = run.qa.find(r => r.id === 'target-l4' && r.system === AGENT);
  row.answer = '团队 B 应对齐 IC 或排序。[[source:source_ref_short]]';
  row.sources = [{ id: 'source_ref_short', valid: true, text: '排序', resolved: {
    highlighted_quote: '排序', context_before: '正确的做法是让损失函数直接对齐 IC /', context_after: '。',
  } }];
  const bundle = prepareTaskBundle(run, corpus, () => 0, taskQualityVersionV2);
  assert.equal(bundle.audit.length, 18);
  assert.equal(bundle.version, taskQualityVersionV2);
  const job = taskJudgeJobs(bundle).find(j => j.kind === 'absolute' && j.case_id === bundle.cases.find(c => c.task_id === 'target-l4').case_id && j.input.candidates[0].answer_spans[0].text.includes('团队 B'));
  assert(job);
  assert.deepEqual(job.input.candidates[0].citations[0], { id: 'C1', valid: true, highlighted_quote: '排序',
    context_before: '正确的做法是让损失函数直接对齐 IC /', context_after: '。', view_status: 'complete' });
  assert(job.input.requirements.some(r => r.kind === 'source_support'));
  assert(job.input.requirements.some(r => r.kind === 'source_location'));
  const judgment = { evaluations: [{ candidate: 'A',
    requirements: job.input.requirements.filter(r => !r.kind.startsWith('source_')).map(r => ({
      requirement_id: r.requirement_id, verdict: 'met', answer_span_ids: ['a1'], reference_ids: ['R1'], reason: '释义正确',
    })),
    claim_assessments: [{ claim_text: 'B 应对齐 IC 或排序', answer_span_ids: ['a1'], support: 'met', location: 'unmet',
      basis: [{ source_id: 'C1', part: 'before', start: 0, end: '正确的做法是让损失函数直接对齐 IC /'.length }], reason: '上下文支持，但高亮只有词' }],
  }], preference: 'not_applicable', reason: '单答案' };
  validateTaskJudgment(job, judgment);
  const report = taskQualityReport(bundle, [{ id: job.id, judgment }]);
  const sample = report.samples.find(s => s.absolute_job_id === job.id);
  assert.equal(sample.requirements.find(r => r.kind === 'source_support').verdict, 'met');
  assert.equal(sample.requirements.find(r => r.kind === 'source_location').verdict, 'unmet');
  assert.equal(sample.task_result, 'fail');
  const invalid = structuredClone(judgment);
  invalid.evaluations[0].claim_assessments[0].basis[0].end = 999;
  assert.throws(() => validateTaskJudgment(job, invalid), /Invalid source basis/);
  invalid.evaluations[0].claim_assessments[0].basis = [{ source_id: 'R1', part: 'highlight', start: 0, end: 1 }];
  assert.throws(() => validateTaskJudgment(job, invalid), /Invalid source basis/);
});

test('task-v2 calibration cannot inherit v1 approval or empty human review', () => {
  const { run, corpus } = readingFixture();
  const packet = calibrationPacket(prepareTaskBundle(run, corpus, () => 0, taskQualityVersionV2));
  assert.equal(packet.version, taskQualityVersionV2);
  assert.equal(packet.cases.length, 28);
  assert.equal(assessTaskCalibration(packet, [], { model: 'test', protocol: taskQualityVersionV2 }).status, 'pending_human_review');
  assert(calibrationJobs(packet).every(j => j.protocol === taskQualityVersionV2));
  const c = packet.cases.find(c => c.id === 'short-highlight-context');
  packet.cases = [c];
  const scope = { family_id: c.family_id, reading_level: c.reading_level, interaction_condition: c.interaction_condition };
  Object.assign(packet, { status: 'human_reviewed', reviewer: 'test reviewer', reviewed_at: '2026-09-26', judge_model: 'test',
    rules: { absolute: { minimum_cases: 1, allowed_disagreements: 0, scopes: [scope] }, preference: null } });
  Object.assign(c, { human: 'fail', human_support: 'met', human_location: 'unmet', basis: '前后文支持，高亮仅有词' });
  const job = calibrationJobs(packet).find(j => j.case_id === c.id);
  const judgment = { evaluations: [{ candidate: 'A', requirements: job.input.requirements
    .filter(r => !r.kind.startsWith('source_')).map(r => ({ requirement_id: r.requirement_id,
      verdict: 'unmet', answer_span_ids: ['a1'], reference_ids: [], reason: '测试内容失败' })),
    claim_assessments: [{ claim_text: '数据必须隔离', answer_span_ids: ['a1'], support: 'met', location: 'met',
      basis: [{ source_id: 'C1', part: 'after', start: 0, end: 2 }], reason: '故意错判定位' }] }],
    preference: 'not_applicable', reason: '单答案' };
  const result = assessTaskCalibration(packet, [{ id: job.id, judgment }], { model: 'test', protocol: taskQualityVersionV2 });
  assert.equal(result.absolute.status, 'failed', JSON.stringify(result.absolute));
  assert(result.absolute.discrepancies.some(d => d.dimension === 'location'));
});

test('task-v2 calibrates only explicitly reviewed cases and isolates failed scopes', () => {
  const { run, corpus } = readingFixture();
  const packet = calibrationPacket(prepareTaskBundle(run, corpus, () => 0, taskQualityVersionV2));
  const reviewed = ['complete-necessary-passage', 'related-without-claim'].map(id => packet.cases.find(c => c.id === id));
  const scope = c => ({ family_id: c.family_id, reading_level: c.reading_level, interaction_condition: c.interaction_condition });
  Object.assign(packet, { status: 'human_reviewed', reviewer: 'test reviewer', reviewed_at: '2026-09-26', judge_model: 'test',
    rules: { absolute: { minimum_cases: 2, allowed_disagreements: 0,
      case_ids: reviewed.map(c => c.id), scopes: reviewed.map(scope) }, preference: null } });
  for (const c of reviewed) Object.assign(c, { human: 'pass', human_support: 'met', human_location: 'met', basis: 'test label' });
  Object.assign(reviewed[1], { human: 'fail', human_support: 'unmet', human_location: 'unmet' });
  const jobs = calibrationJobs(packet).filter(j => reviewed.some(c => c.id === j.case_id));
  const records = jobs.map(job => ({ id: job.id, judgment: { evaluations: [{ candidate: 'A',
    requirements: job.input.requirements.filter(r => !r.kind.startsWith('source_')).map(r => ({
      requirement_id: r.requirement_id, verdict: 'met', answer_span_ids: ['a1'], reference_ids: ['R1'], reason: 'test' })),
    claim_assessments: [{ claim_text: 'test claim', answer_span_ids: ['a1'], support: 'met', location: 'met',
      basis: [{ source_id: 'C1', part: 'highlight', start: 0, end: 2 }], reason: 'test' }] }],
    preference: 'not_applicable', reason: 'test' } }));
  const options = { model: 'test', protocol: taskQualityVersionV2 };
  const result = assessTaskCalibration(packet, records, options);
  assert.equal(result.status, 'partial');
  assert.equal(result.absolute.failures.length, 0);
  assert.equal(approvedFor(result, { kind: 'absolute', ...scope(reviewed[0]) }), true);
  assert.equal(approvedFor(result, { kind: 'absolute', ...scope(reviewed[1]) }), false);
  assert.equal(approvedFor(result, { kind: 'preference', ...scope(reviewed[0]) }), false);
  const bundle = prepareTaskBundle(run, corpus, () => 0, taskQualityVersionV2);
  const report = annotateTaskCalibration(taskQualityReport(bundle, []), bundle, result);
  assert(report.samples.filter(s => s.task_id === 'target-l2').every(s => s.score_reason === 'calibration_failed'));
  assert(report.samples.filter(s => s.task_id === 'holdout-l2').every(s => s.score_reason === 'calibration_not_reviewed'));
  assert(report.samples.filter(s => s.task_id === 'holdout-l1').every(s => s.score_reason === undefined));
  assert(report.preference.every(p => p.status === 'not_applicable' || p.reason === 'calibration_not_reviewed'));
  reviewed[0].human = null;
  assert.equal(approvedFor(assessTaskCalibration(packet, records, options), { kind: 'absolute', ...scope(reviewed[0]) }), false);
  packet.rules.absolute.case_ids.push('unknown-case');
  assert.equal(assessTaskCalibration(packet, records, options).absolute.reason, 'invalid_case_selection');
});

test('task-v2 exact numeric requirements are checked by the program', () => {
  const { run, corpus } = readingFixture();
  const row = run.qa.find(r => r.id === 'target-l1' && r.system === AGENT);
  row.answer = '方向标签约 7.7 bit。[[source:source_ref_numeric]]';
  row.sources = [{ id: 'source_ref_numeric', valid: true, text: 'H(Y)\\approx 1\\ \\text{bit}', resolved: {
    highlighted_quote: 'H(Y)\\approx 1\\ \\text{bit}', context_before: '', context_after: '',
  } }];
  const bundle = prepareTaskBundle(run, corpus, () => 0, taskQualityVersionV2);
  const job = taskJudgeJobs(bundle).find(j => j.kind === 'absolute' && j.input.question.includes('约为多少')
    && j.input.candidates[0].answer_spans[0].text.includes('7.7'));
  assert(job);
  const exact = job.input.requirements.find(r => r.evaluator === 'exact');
  assert.deepEqual(exact.acceptable, [1]);
  const judgment = { evaluations: [{ candidate: 'A', requirements: job.input.requirements
    .filter(r => !r.kind.startsWith('source_')).map(r => ({ requirement_id: r.requirement_id,
      ...(r.evaluator === 'exact' ? { observed_value: 7.7 } : { verdict: 'met' }),
      answer_span_ids: ['a1'], reference_ids: ['R1'], reason: '答案写出数值' })),
    claim_assessments: [{ claim_text: '方向标签约 7.7 bit', answer_span_ids: ['a1'], support: 'unmet', location: 'unmet',
      basis: [], reason: '来源给出 1 bit' }] }], preference: 'not_applicable', reason: '单答案' };
  const report = taskQualityReport(bundle, [{ id: job.id, judgment }]);
  const sample = report.samples.find(s => s.absolute_job_id === job.id);
  assert.equal(sample.requirements.find(r => r.requirement_id === exact.requirement_id).verdict, 'unmet');
  assert.equal(sample.task_result, 'fail');
});

test('task-v2 distinguishes product delivery failure from content and judge failures', () => {
  const { run, corpus } = readingFixture();
  run.qa.find(r => r.id === 'target-l4' && r.system === CHUNK).outcome.incomplete = true;
  const bundle = prepareTaskBundle(run, corpus, () => 0, taskQualityVersionV2);
  const job = taskJudgeJobs(bundle).find(j => j.kind === 'absolute');
  const report = taskQualityReport(bundle, [{ id: job.id, error: 'judge_timeout' }]);
  const failed = report.samples.find(s => s.task_id === 'target-l4' && s.system === CHUNK);
  assert.equal(failed.task_result, 'fail');
  assert.deepEqual(failed.failure_categories, ['product_delivery']);
  const timeout = report.samples.find(s => s.absolute_job_id === job.id);
  assert.equal(timeout.task_result, 'unresolved');
  assert.equal(timeout.score_reason, 'judge_timeout');
  assert.deepEqual(timeout.failure_categories, ['scoring_unresolved']);
});

test('task-v2 comparison keeps the complete old and new denominator', () => {
  const { run, corpus } = readingFixture();
  const oldReport = taskQualityReport(prepareTaskBundle(run, corpus, () => 0), []);
  const newReport = taskQualityReport(prepareTaskBundle(run, corpus, () => 0, taskQualityVersionV2), []);
  const comparison = compareV2WithPrior(newReport, oldReport);
  assert.equal(comparison.total, 18);
  assert.equal(comparison.samples.filter(s => s.new_task_result === 'unresolved').length, 18);
  assert.throws(() => compareV2WithPrior({ ...newReport, samples: newReport.samples.slice(1) }, oldReport), /denominator/);
});

test('calibration requires human rules and blocks a critical false admission without blocking preference', () => {
  const { run, corpus } = readingFixture(), bundle = prepareTaskBundle(run, corpus, () => 0);
  const packet = calibrationPacket(bundle), jobs = calibrationJobs(packet);
  assert.equal(packet.cases.length, 22);
  assert.equal(assessTaskCalibration(packet, [], { model: 'test-model', protocol: packet.protocol }).status, 'pending_human_review');
  const absoluteCase = packet.cases.find(c => c.id === 'l4-missed-search');
  const pairCase = packet.cases.find(c => c.id === 'l4-map-preference');
  const scope = c => ({ family_id: c.family_id, reading_level: c.reading_level, interaction_condition: c.interaction_condition });
  Object.assign(packet, { status: 'human_reviewed', reviewer: 'test reviewer', reviewed_at: '2026-09-25', judge_model: 'test-model',
    rules: { absolute: { minimum_cases: 1, allowed_disagreements: 1, scopes: [scope(absoluteCase)] },
      preference: { minimum_cases: 1, allowed_disagreements: 0, scopes: [scope(pairCase)] } } });
  absoluteCase.human = 'fail'; absoluteCase.basis = '原文明确说重复筛选污染留出集';
  pairCase.human = 'A'; pairCase.basis = 'A 分别匹配两个目标';
  const absoluteJob = jobs.find(j => j.case_id === absoluteCase.id);
  const records = [{ id: absoluteJob.id, judgment: absolute(absoluteJob) },
    ...jobs.filter(j => j.case_id === pairCase.id).map((j, i) => ({ id: j.id,
      judgment: { preference: i ? 'B' : 'A', reason: '同一份答案较好' } }))];
  const result = assessTaskCalibration(packet, records, { model: 'test-model', protocol: packet.protocol });
  assert.equal(result.absolute.status, 'failed');
  assert.equal(result.absolute.critical_false_admissions, 1);
  assert.equal(result.preference.status, 'passed');
  const pairJob = taskJudgeJobs(bundle).find(j => j.kind === 'preference' && j.family_id === pairCase.family_id && j.reading_level === pairCase.reading_level);
  assert.equal(approvedFor(result, pairJob), true);
  assert.equal(approvedFor(result, { ...pairJob, interaction_condition: 'multi_turn' }), false);
  assert.equal(approvedFor(result, taskJudgeJobs(bundle).find(j => j.kind === 'absolute')), false);
  pairCase.basis = null;
  assert.equal(assessTaskCalibration(packet, records, { model: 'test-model', protocol: packet.protocol }).preference.status, 'invalid');
});


test('task-v2 transfer calibration preserves source text and never supplies human approval', () => {
  const { run, corpus } = readingFixture();
  const bundle = prepareTaskBundle(run, corpus, () => 0, taskQualityVersionV2);
  const packet = calibrationPacket(bundle);
  const cases = packet.cases.filter(c => c.id.startsWith('transfer-'));
  assert.equal(cases.length, 3);
  const full = cases.find(c => c.id === 'transfer-complete').input.candidates[0];
  const short = cases.find(c => c.id === 'transfer-keyword').input.candidates[0];
  assert.deepEqual(full.answer_spans, short.answer_spans);
  for (const c of cases) {
    assert.equal(c.reading_level, 'L4');
    assert.equal(c.human, null);
    assert.equal(c.human_support, null);
    assert.equal(c.human_location, null);
    assert(c.input.requirements.every(r => r.requirement_id.startsWith('target-l4:')));
    for (const span of c.input.candidates[0].answer_spans) for (const match of span.text.matchAll(/\[\[source:(C\d+)\]\]/g))
      assert(c.input.candidates[0].citations.some(source => source.id === match[1]));
  }
  for (let i = 0; i < full.citations.length; i++) {
    const source = short.citations[i];
    assert.equal(source.context_before + source.highlighted_quote + source.context_after, full.citations[i].highlighted_quote);
  }
});

test('task-v2 incomplete supporting evidence stays unknown without making accurate content false', () => {
  const { run, corpus } = readingFixture();
  const bundle = prepareTaskBundle(run, corpus, () => 0, taskQualityVersionV2);
  const job = taskJudgeJobs(bundle).find(j => j.kind === 'absolute' && j.family_id === 'target-loss-alignment' && j.reading_level === 'L2');
  const judgment = { evaluations: [{ candidate: 'A', requirements: job.input.requirements
    .filter(r => !r.kind.startsWith('source_')).map(r => ({ requirement_id: r.requirement_id,
      verdict: 'met', answer_span_ids: ['a1'], reference_ids: [], reason: '内容正确，来源另判' })),
    claim_assessments: [{ claim_text: '比值提高', answer_span_ids: ['a1'], support: 'unknown', location: 'unknown',
      basis: [], reason: '交付片段没有互信息损失幅度，不能判断是否支持比值提高' }] }],
    preference: 'not_applicable', reason: '单答案' };
  const result = taskQualityReport(bundle, [{ id: job.id, judgment }]).samples.find(s => s.absolute_job_id === job.id);
  assert.equal(result.task_result, 'unresolved');
  assert.equal(result.requirements.find(r => r.kind === 'accuracy').verdict, 'met');
  assert.equal(result.requirements.find(r => r.kind === 'source_support').verdict, 'unknown');
  assert.equal(result.requirements.find(r => r.kind === 'source_location').verdict, 'unknown');
});
