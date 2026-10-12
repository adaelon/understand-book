import { gradeEvaluation, validateJudgment } from './quality-core.mjs';
import { taskJudgeJobs, validateTaskJudgment, taskQualityVersion, taskQualityVersionV2 } from './task-quality.mjs';
import { v2Outcome, v2SourceVerdicts } from './task-quality-v2.mjs';
import { readingDatasetVersion } from './reading-dataset.mjs';
import { AGENT } from './agent-core.mjs';

const resultKinds = ['pass', 'fail', 'unknown'];
const preferences = ['A', 'B', 'tie', 'unresolved'];

export function calibrationPacket(bundle) {
  if (bundle.version === taskQualityVersionV2) return bundle.source_run_version === readingDatasetVersion
    ? v2CalibrationPacket(bundle) : v2GenericCalibrationPacket(bundle);
  if (bundle.source_run_version === readingDatasetVersion) return readingCalibrationPacket(bundle);
  const byId = id => {
    const found = bundle.cases.find(c => c.task_id === id);
    if (!found) throw new Error(`Missing calibration source: ${id}`);
    return found;
  };
  const answer = (text, citation = null) => ({ candidate: 'A', answer_spans: [{ id: 'a1', text }],
    citations: citation ? [{ id: 'C1', text: citation, valid: true }] : [] });
  const absolute = (id, task, text, { citation = null, critical = false } = {}) => {
    const c = byId(task);
    return { id, kind: 'absolute', task_id: task, family_id: c.family_id, reading_level: c.reading_level,
      interaction_condition: c.interaction_condition, critical,
      input: { question: c.question, answerable: c.answerable, explanation_required: c.explanation_required,
        required_facts: c.required_facts, reference_evidence: c.reference_evidence,
        candidates: [answer(text, citation)] }, human: null, basis: null };
  };
  const contrast = byId('contrast-03'), contrastEvidence = contrast.reference_evidence[0]?.text;
  const zeroOrder = byId('contrast-04'), zeroEvidence = zeroOrder.reference_evidence[0]?.text;
  const embargo = byId('concept-04'), embargoEvidence = embargo.reference_evidence[0]?.text;
  const cases = [
    absolute('negation-reversal', 'contrast-03', 'MWU 会全押领先者，落后因子没有权重。[[source:C1]]', { citation: contrastEvidence, critical: true }),
    absolute('unsupported-citation', 'contrast-04', '不是。零阶比一阶更费函数评估。[[source:C1]]', { citation: contrastEvidence, critical: true }),
    absolute('missing-condition', 'concept-04', 'embargo 是在测试段之后禁用一段训练样本。[[source:C1]]', { citation: embargoEvidence, critical: true }),
    absolute('short-sufficient', 'contrast-03', 'MWU 留给落后因子软权重；FTL 全押领先者。[[source:C1]]', { citation: contrastEvidence }),
    absolute('reasonable-refusal', 'refusal-01', '本书没有提供该随机种子，不能据此给出整数。'),
    absolute('over-refusal', 'contrast-03', '原文没有说明，无法回答。', { critical: true }),
    absolute('reference-not-citation', 'contrast-04', '不是。零阶比一阶更费函数评估。', { critical: true }),
    absolute('correct-with-source', 'contrast-04', '不是。零阶没有方向信息，需要更多函数评估，样本效率低于一阶。[[source:C1]]', { citation: zeroEvidence }),
  ];
  for (const id of ['contrast-03', 'contrast-04', 'cross-02']) {
    const c = byId(id);
    const sample = bundle.audit.find(a => a.task_id === id && a.system === AGENT);
    const job = taskJudgeJobs(bundle).find(j => j.kind === 'absolute' && j.mapping.A === sample?.sample_id);
    if (!job) throw new Error(`Missing actual calibration answer: ${id}`);
    cases.push({ id: `actual-${id}`, kind: 'absolute', task_id: id, family_id: c.family_id,
      reading_level: c.reading_level, interaction_condition: c.interaction_condition,
      critical: id === 'contrast-04', input: job.input, human: null, basis: null });
  }
  const pair = (id, task, a, b) => {
    const c = byId(task);
    return { id, kind: 'preference', task_id: task, family_id: c.family_id, reading_level: c.reading_level,
      interaction_condition: c.interaction_condition,
      critical: false, input: { question: c.question, candidates: [{ ...a, candidate: 'A' }, { ...b, candidate: 'B' }] },
      human: null, basis: null };
  };
  cases.push(pair('short-vs-wrong', 'contrast-03', answer('MWU 给落后因子留权重；FTL 全押。[[source:C1]]', contrastEvidence),
    answer('MWU 与 FTL 一样，都只押当前领先者。[[source:C1]]', contrastEvidence)));
  cases.push(pair('short-vs-long', 'contrast-04', answer('不是，零阶更费函数评估。[[source:C1]]', zeroEvidence),
    answer('零阶不需要梯度，但方向要靠大量函数评估采样，因此比一阶更费样本。[[source:C1]]', zeroEvidence)));
  return { version: taskQualityVersion, status: 'candidate_not_human_reviewed', reviewer: null, reviewed_at: null,
    judge_model: null, protocol: taskQualityVersion, rules: { absolute: null, preference: null }, cases };
}

export function calibrationJobs(packet) {
  if (![taskQualityVersion, taskQualityVersionV2].includes(packet.version)) throw new Error('Calibration protocol mismatch');
  return packet.cases.flatMap(c => c.kind === 'absolute'
    ? [{ id: `cal-${c.id}-absolute`, ...(packet.version === taskQualityVersionV2 ? { protocol: packet.version } : {}), kind: 'absolute', case_id: c.id, mapping: { A: c.id }, input: c.input }]
    : [false, true].map((reverse, i) => ({ id: `cal-${c.id}-preference-${i + 1}`, ...(packet.version === taskQualityVersionV2 ? { protocol: packet.version } : {}), kind: 'preference', case_id: c.id,
      mapping: reverse ? { A: `${c.id}-B`, B: `${c.id}-A` } : { A: `${c.id}-A`, B: `${c.id}-B` },
      input: { question: c.input.question, candidates: (reverse ? [...c.input.candidates].reverse() : c.input.candidates)
        .map((candidate, j) => ({ ...candidate, candidate: j ? 'B' : 'A' })) } })));
}

export function assessTaskCalibration(packet, records, { model, protocol } = {}) {
  if (![taskQualityVersion, taskQualityVersionV2].includes(packet.version)) throw new Error('Calibration protocol mismatch');
  const pending = reason => ({ status: 'pending_human_review', reason, absolute: { status: 'pending' }, preference: { status: 'pending' } });
  if (packet.status !== 'human_reviewed' || !packet.reviewer?.trim() || !packet.reviewed_at?.trim()) return pending('reviewer_or_status_missing');
  if (!packet.judge_model?.trim() || packet.judge_model !== model || packet.protocol !== protocol) return pending('judge_or_protocol_mismatch');
  if (!Array.isArray(packet.cases) || !packet.cases.length) return pending('empty_cases');
  const jobs = calibrationJobs(packet), findings = {};
  for (const kind of ['absolute', 'preference']) {
    const rule = packet.rules?.[kind];
    if (!rule || !Number.isInteger(rule.minimum_cases) || rule.minimum_cases < 1 ||
      !Number.isInteger(rule.allowed_disagreements) || rule.allowed_disagreements < 0 ||
      !Array.isArray(rule.scopes) || !rule.scopes.length)
      { findings[kind] = { status: 'pending', reason: 'human_rule_missing' }; continue; }
    const scopedCases = packet.cases.filter(c => c.kind === kind && rule.scopes.some(scope =>
      c.family_id === scope.family_id && c.reading_level === scope.reading_level &&
      c.interaction_condition === scope.interaction_condition));
    const selection = packet.version === taskQualityVersionV2 ? rule.case_ids : undefined;
    if (selection !== undefined && (!Array.isArray(selection) || !selection.length ||
      new Set(selection).size !== selection.length || selection.some(id => !scopedCases.some(c => c.id === id))))
      { findings[kind] = { status: 'pending', reason: 'invalid_case_selection' }; continue; }
    const cases = selection ? scopedCases.filter(c => selection.includes(c.id)) : scopedCases;
    if (cases.length < rule.minimum_cases) { findings[kind] = { status: 'pending', reason: 'insufficient_cases' }; continue; }
    if (rule.scopes.some(scope => !scope.family_id || !scope.reading_level || !scope.interaction_condition ||
      !cases.some(c => c.family_id === scope.family_id && c.reading_level === scope.reading_level &&
        c.interaction_condition === scope.interaction_condition)))
      { findings[kind] = { status: 'pending', reason: 'scope_not_represented' }; continue; }
    const discrepancies = [], failures = [];
    for (const c of cases) {
      if (!c.basis?.trim() || (kind === 'absolute' && !resultKinds.includes(c.human)) ||
        (packet.version === taskQualityVersionV2 && kind === 'absolute' &&
          (!['met', 'unmet', 'unknown'].includes(c.human_support) || !['met', 'unmet', 'unknown'].includes(c.human_location))) ||
        (kind === 'preference' && !preferences.includes(c.human))) {
        failures.push({ case_id: c.id, reason: 'human_label_missing' }); continue;
      }
      const caseJobs = jobs.filter(j => j.case_id === c.id), outcomes = [];
      for (const job of caseJobs) {
        const matches = records.filter(r => r.id === job.id);
        if (matches.length !== 1 || matches[0].error || !matches[0].judgment) {
          failures.push({ case_id: c.id, job_id: job.id, reason: matches[0]?.error ?? 'judgment_missing' }); continue;
        }
        try {
          const judgment = validateTaskJudgment(job, matches[0].judgment);
          if (kind === 'absolute') {
            outcomes.push(packet.version === taskQualityVersionV2
              ? v2Outcome(job, judgment) : gradeEvaluation(job.input, validateJudgment(job, judgment).evaluations[0]).status);
            if (packet.version === taskQualityVersionV2) {
              const source = v2SourceVerdicts(job, judgment);
              for (const field of ['support', 'location']) if (source[field] !== c[`human_${field}`])
                discrepancies.push({ case_id: c.id, dimension: field, expected: c[`human_${field}`], actual: source[field],
                  critical_false_admission: c.critical && c[`human_${field}`] === 'unmet' && source[field] === 'met' });
            }
          }
          else outcomes.push(['A', 'B'].includes(judgment.preference) ? job.mapping[judgment.preference] : judgment.preference);
        } catch (error) { failures.push({ case_id: c.id, job_id: job.id, reason: `invalid_output: ${error.message}` }); }
      }
      if (outcomes.length !== caseJobs.length) continue;
      const expected = kind === 'absolute' ? c.human === 'unknown' ? 'unscorable' : c.human
        : ['A', 'B'].includes(c.human) ? `${c.id}-${c.human}` : c.human;
      for (const actual of outcomes) if (actual !== expected)
        discrepancies.push({ case_id: c.id, expected, actual, critical_false_admission: kind === 'absolute' && c.critical && c.human === 'fail' && actual === 'pass' });
    }
    const severe = discrepancies.filter(d => d.critical_false_admission).length;
    findings[kind] = { status: failures.length ? 'invalid' : severe || discrepancies.length > rule.allowed_disagreements ? 'failed' : 'passed',
      cases: cases.length, critical_false_admissions: severe, discrepancies, failures,
      scope: rule.scopes };
    if (packet.version === taskQualityVersionV2) findings[kind].scope_results = rule.scopes.map(scope => {
      const ids = cases.filter(c => c.family_id === scope.family_id && c.reading_level === scope.reading_level &&
        c.interaction_condition === scope.interaction_condition).map(c => c.id);
      const scopedFailures = failures.filter(f => ids.includes(f.case_id));
      const scopedDiscrepancies = discrepancies.filter(d => ids.includes(d.case_id));
      return { ...scope, case_ids: ids,
        status: scopedFailures.length ? 'invalid' : scopedDiscrepancies.some(d => d.critical_false_admission) ||
          scopedDiscrepancies.length > rule.allowed_disagreements ? 'failed' : 'passed',
        failures: scopedFailures, discrepancies: scopedDiscrepancies };
    });
  }
  return { status: findings.absolute.status === 'passed' && findings.preference.status === 'passed' ? 'passed'
    : Object.values(findings).some(f => f.status === 'passed' || f.scope_results?.some(s => s.status === 'passed')) ? 'partial' : 'not_approved',
  reviewer: packet.reviewer, reviewed_at: packet.reviewed_at, judge_model: model, absolute: findings.absolute, preference: findings.preference };
}

export function approvedFor(calibration, job) {
  const scope = calibration[job.kind];
  if (scope?.scope_results) return scope.scope_results.some(s => s.status === 'passed' &&
    s.family_id === job.family_id && s.reading_level === job.reading_level && s.interaction_condition === job.interaction_condition);
  return scope?.status === 'passed' && scope.scope?.some(s => s.family_id === job.family_id &&
    s.reading_level === job.reading_level && s.interaction_condition === job.interaction_condition);
}

export function annotateTaskCalibration(report, bundle, calibration) {
  const jobs = taskJudgeJobs(bundle);
  const reasonFor = job => {
    if (!job || approvedFor(calibration, job)) return null;
    const scope = calibration[job.kind]?.scope_results?.find(s => s.family_id === job.family_id &&
      s.reading_level === job.reading_level && s.interaction_condition === job.interaction_condition);
    return scope?.status === 'failed' ? 'calibration_failed' : scope?.status === 'invalid'
      ? 'calibration_invalid' : 'calibration_not_reviewed';
  };
  for (const sample of report.samples) {
    const reason = reasonFor(jobs.find(j => j.id === sample.absolute_job_id));
    if (!reason || sample.score_status !== 'not_run') continue;
    sample.score_reason = reason;
    for (const req of sample.requirements) if (req.reason_code === 'not_run') {
      req.reason_code = reason;
      req.reason = reason === 'calibration_failed' ? '该任务范围的模型校准与人审存在分歧。'
        : reason === 'calibration_invalid' ? '该任务范围的校准记录缺失或无效。' : '该任务范围尚未获得实际人审校准批准。';
    }
  }
  for (const pair of report.preference) {
    const reason = reasonFor(jobs.find(j => j.kind === 'preference' && j.case_id === pair.case_id));
    if (reason && pair.status !== 'valid') pair.reason = reason;
  }
  return report;
}

function v2CalibrationPacket(bundle) {
  const byId = id => {
    const c = bundle.cases.find(c => c.task_id === id);
    if (!c) throw new Error(`Missing v2 calibration source: ${id}`);
    return c;
  };
  const make = (id, taskId, answer, quoteMode, suggested_human, critical = false) => {
    const c = byId(taskId), source = (quoteMode === 'short'
      ? c.reference_evidence.find(r => r.text.includes('排序')) ?? c.reference_evidence[0]
      : c.reference_evidence[0])?.text ?? '';
    const highlighted_quote = quoteMode === 'short' ? (source.includes('排序') ? '排序' : source.slice(0, 2)) : source;
    const at = source.indexOf(highlighted_quote);
    const citation = { id: 'C1', valid: true, highlighted_quote,
      context_before: quoteMode === 'short' ? source.slice(0, at) : '',
      context_after: quoteMode === 'short' ? source.slice(at + highlighted_quote.length) : '', view_status: 'complete' };
    return { id, kind: 'absolute', task_id: taskId, family_id: c.family_id, reading_level: c.reading_level,
      interaction_condition: c.interaction_condition, critical, suggested_human,
      input: { question: c.question, answerable: c.answerable,
        requirements: c.requirements.map(r => ({ requirement_id: r.requirement_id, kind: r.kind,
          evaluator: r.evaluator, description: r.description, ...(r.evaluator === 'exact' ? { acceptable: r.acceptable } : {}) })),
        reference_evidence: c.reference_evidence,
        candidates: [{ candidate: 'A', answer_spans: [{ id: 'a1', text: `${answer} [[source:C1]]` }], citations: [citation] }] },
      human: null, human_support: null, human_location: null, basis: null };
  };
  const cases = [
    make('complete-necessary-passage', 'holdout-l1', '评判因子的数据必须和搜索因子的数据彻底隔离。', 'full', 'pass'),
    make('related-without-claim', 'target-l2', '方向标签凭空创造了更多互信息。', 'full', 'fail', true),
    make('omitted-necessary-condition', 'holdout-l3', 'purging 和 embargo 能修复反复筛选留出集造成的污染。', 'full', 'fail', true),
    make('short-highlight-context', 'holdout-l1', '评判因子的数据必须和搜索因子的数据彻底隔离。', 'short', 'fail'),
    make('broad-undirected-citation', 'holdout-l2', 'purging 处理标签重叠，embargo 处理测试后自相关。', 'full', null),
    make('chinese-semantic-equivalence', 'target-l2', '目标熵下降远大于互信息损失，所以可恢复比例提高。', 'full', 'fail'),
    make('semantic-reversal', 'target-l2', '目标熵上升，因而可恢复比例降低。', 'full', 'fail', true),
  ];
  const positive = make('chinese-semantic-equivalence-full-source', 'target-l2',
    '目标熵下降远大于互信息损失，所以可恢复比例提高。', 'full', 'pass');
  const target = byId('target-l2');
  const resident = target.samples.find(s => bundle.audit.find(a => a.sample_id === s.sample_id)?.system === AGENT);
  const full = resident?.citations?.find(s => s.valid && s.highlighted_quote.includes('分母直接塌掉') && s.highlighted_quote.includes('近五倍'));
  if (full) {
    positive.input.candidates[0].citations = [{ ...full, id: 'C1' }];
    cases.push(positive);
  }
  const suggested = {
    'complete-necessary-passage': ['met', 'met'],
    'related-without-claim': ['unmet', 'unmet'],
    'omitted-necessary-condition': ['unmet', null],
    'short-highlight-context': ['met', 'unmet'],
    'broad-undirected-citation': ['met', null],
    'chinese-semantic-equivalence': ['unmet', 'unmet'],
    'semantic-reversal': ['unmet', 'unmet'],
    'chinese-semantic-equivalence-full-source': ['met', 'met'],
  };
  for (const c of cases) [c.suggested_support, c.suggested_location] = suggested[c.id];
  const transfer = byId('target-l4');
  const transferSources = transfer.reference_evidence.map((r, i) => ({
    id: `C${i + 1}`, valid: true, highlighted_quote: r.text,
    context_before: '', context_after: '', view_status: 'complete',
  }));
  const transferAnswer = [
    { id: 'a1', text: 'A 的标签只看固定终点，和止盈止损的首达结果错位；应考虑三重屏障，按先触及止盈或止损的结果打标，使标签贴近实际交易结果，减少固定终点噪声。[[source:C1]][[source:C2]]' },
    { id: 'a2', text: 'B 只按相对排序交易，MSE 却要求拟合精确收益；应让损失对齐 IC 或排序，例如用预测分数与真实收益在当天截面上的相关系数取负作为 IC loss。[[source:C3]][[source:C4]]' },
  ];
  for (const mode of ['complete', 'reversed', 'keyword']) {
    const citations = structuredClone(transferSources);
    if (mode === 'keyword') for (const source of citations) {
      const text = source.highlighted_quote;
      const word = ['三重屏障', 'MSE', '排序', '标签'].find(word => text.includes(word)) ?? text.slice(0, 2);
      const at = text.indexOf(word);
      Object.assign(source, { highlighted_quote: word, context_before: text.slice(0, at), context_after: text.slice(at + word.length) });
    }
    cases.push({ id: `transfer-${mode}`, kind: 'absolute', task_id: transfer.task_id,
      family_id: transfer.family_id, reading_level: transfer.reading_level,
      interaction_condition: transfer.interaction_condition, critical: mode === 'reversed',
      suggested_human: mode === 'complete' ? 'pass' : 'fail',
      suggested_support: mode === 'reversed' ? 'unmet' : 'met',
      suggested_location: mode === 'complete' ? 'met' : 'unmet',
      input: { question: transfer.question, answerable: transfer.answerable,
        requirements: transfer.requirements.map(r => ({ requirement_id: r.requirement_id,
          kind: r.kind, evaluator: r.evaluator, description: r.description,
          ...(r.evaluator === 'exact' ? { acceptable: r.acceptable } : {}) })),
        reference_evidence: transfer.reference_evidence,
        candidates: [{ candidate: 'A', citations, answer_spans: mode === 'reversed' ? [
          { id: 'a1', text: 'A 应保留固定终点价格标签并改用 MSE，因为 MSE 能消除终点噪声；B 应用三重屏障替代排序损失，因为三重屏障会直接优化整个截面的排序。[[source:C1]][[source:C2]][[source:C3]][[source:C4]]' },
        ] : structuredClone(transferAnswer) }] },
      human: null, human_support: null, human_location: null, basis: null });

  }
  const actual = taskJudgeJobs(bundle).filter(j => j.kind === 'absolute');
  for (const c of bundle.cases.filter(c => c.category !== 'restart')) {
    const sample = c.samples.find(s => s.delivery === 'established' && bundle.audit.find(a => a.sample_id === s.sample_id)?.system === AGENT);
    const job = actual.find(j => j.mapping.A === sample?.sample_id);
    if (!job) throw new Error(`Missing v2 actual calibration answer: ${c.task_id}`);
    cases.push({ id: `actual-${c.task_id}`, kind: 'absolute', task_id: c.task_id, family_id: c.family_id,
      reading_level: c.reading_level, interaction_condition: c.interaction_condition,
      critical: false, suggested_human: null, input: job.input,
      human: null, human_support: null, human_location: null, basis: null });
  }
  for (const job of taskJudgeJobs(bundle).filter(j => j.kind === 'preference' && j.id.endsWith('-1'))) {
    const c = bundle.cases.find(c => c.case_id === job.case_id);
    cases.push({ id: `pair-${c.task_id}`, kind: 'preference', task_id: c.task_id,
      family_id: c.family_id, reading_level: c.reading_level, interaction_condition: c.interaction_condition,
      critical: false, suggested_human: null, input: job.input, human: null, basis: null });
  }
  return { version: taskQualityVersionV2, status: 'candidate_not_human_reviewed', reviewer: null,
    reviewed_at: null, judge_model: null, protocol: taskQualityVersionV2,
    rules: { absolute: null, preference: null }, cases };
}

function v2GenericCalibrationPacket(bundle) {
  const jobs = taskJudgeJobs(bundle).filter(j => j.kind === 'absolute');
  const cases = jobs.map(job => {
    const c = bundle.cases.find(c => c.case_id === job.case_id);
    return { id: `actual-${job.mapping.A}`, kind: 'absolute', task_id: c.task_id,
      family_id: c.family_id, reading_level: c.reading_level, interaction_condition: c.interaction_condition,
      critical: false, suggested_human: null, input: job.input,
      human: null, human_support: null, human_location: null, basis: null };
  });
  for (const job of taskJudgeJobs(bundle).filter(j => j.kind === 'preference' && j.id.endsWith('-1'))) {
    const c = bundle.cases.find(c => c.case_id === job.case_id);
    cases.push({ id: `pair-${c.task_id}`, kind: 'preference', task_id: c.task_id,
      family_id: c.family_id, reading_level: c.reading_level, interaction_condition: c.interaction_condition,
      critical: false, suggested_human: null, input: job.input, human: null, basis: null });
  }
  return { version: taskQualityVersionV2, status: 'candidate_not_human_reviewed', reviewer: null,
    reviewed_at: null, judge_model: null, protocol: taskQualityVersionV2,
    rules: { absolute: null, preference: null }, cases };
}

function readingCalibrationPacket(bundle) {
  const byId = id => {
    const c = bundle.cases.find(c => c.task_id === id);
    if (!c) throw new Error(`Missing reading calibration source: ${id}`);
    return c;
  };
  const candidate = (text, evidence) => ({ candidate: 'A', answer_spans: [{ id: 'a1', text }],
    citations: evidence.map((r, i) => ({ id: `C${i + 1}`, text: r.text, valid: true })) });
  const absolute = (id, task, text, critical = false, cited = true) => {
    const c = byId(task), evidence = cited ? c.reference_evidence : [];
    return { id, kind: 'absolute', task_id: task, family_id: c.family_id, reading_level: c.reading_level,
      interaction_condition: c.interaction_condition,
      critical, input: { question: c.question, answerable: true, explanation_required: c.explanation_required,
        required_facts: c.required_facts, requirement_rubric: c.requirements.map(r => ({ kind: r.kind, description: r.description })),
        reference_evidence: c.reference_evidence,
        candidates: [candidate(text + evidence.map((_, i) => ` [[source:C${i + 1}]]`).join(''), evidence)] },
      human: null, basis: null };
  };
  const cases = [
    absolute('l1-holdout-short', 'holdout-l1', '评判因子的数据和搜索因子的数据必须隔离。'),
    absolute('l2-holdout-conflation', 'holdout-l2', 'purging 与 embargo 都只是在测试段之前禁用训练样本。', true),
    absolute('l1-target-short', 'target-l1', '方向标签的 H(Y) 约 1 bit。'),
    absolute('l2-target-new-information', 'target-l2', '方向标签创造了更多互信息，所以可恢复信息比例提高。', true),
    absolute('l3-search-denial', 'holdout-l3', '反复使用同一留出集没有问题；purging 和 embargo 足以保证它仍是独立验证。', true),
    absolute('l4-missed-search', 'holdout-l4', '只需 purging 和 embargo；同一留出集反复筛选不影响验证。', true),
    absolute('l3-relation-swap', 'target-l3', '三重屏障直接优化股票排序损失，IC loss 则改变止盈止损标签。', true),
    absolute('l4-wrong-mapping', 'target-l4', '团队 A 继续用固定终点价格；团队 B 改用三重屏障作为排序损失。', true),
    absolute('multi-ignore-correction', 'holdout-multi', '仍只需 purging 与 embargo；重复筛选留出集不影响验证。', true),
    absolute('multi-ignore-goal', 'target-multi', '继续使用 MSE 精确拟合次日收益的小数值。', true),
    absolute('l3-short-sufficient', 'holdout-l3', '反复筛选使留出集参与搜索，必须另取未参与搜索的数据；purging 处理标签跨界，embargo 处理测试后自相关，二者不能修复搜索污染。'),
    absolute('l3-target-relation', 'target-l3', '三重屏障把固定期终点改为止盈止损首达，减少终点噪声；IC/rank loss 把指增优化从精确收益数值改为相对排序。两者都让目标对齐真实交易所需的结构。'),
  ];
  const pair = (id, task, first, second) => {
    const c = byId(task), evidence = c.reference_evidence;
    const withRefs = text => candidate(text + evidence.map((_, i) => ` [[source:C${i + 1}]]`).join(''), evidence);
    return { id, kind: 'preference', task_id: task, family_id: c.family_id, reading_level: c.reading_level,
      interaction_condition: c.interaction_condition,
      critical: false, input: { question: c.question, candidates: [withRefs(first), { ...withRefs(second), candidate: 'B' }] },
      human: null, basis: null };
  };
  cases.push(pair('l4-map-preference', 'target-l4',
    'A 改用首达结果的三重屏障标签；B 改用 IC 或 rank loss 优化相对排序。',
    'A 继续拟合固定期精确价格；B 用三重屏障直接代替排序损失。'));
  cases.push(pair('multi-correction-preference', 'holdout-multi',
    '补充信息改变了诊断：留出集已被反复筛选，应另取独立数据；purging 和 embargo 只处理时间泄漏。',
    '继续做 purging 和 embargo 就够了，反复筛选留出集不会改变验证性质。'));
  cases.push(pair('holdout-l1-preference', 'holdout-l1',
    '评判因子的数据必须与搜因子的数据隔离。', '可以在同一份数据上搜索并评判因子。'));
  cases.push(pair('holdout-l2-preference', 'holdout-l2',
    'purging 删掉标签区间跨入测试集的训练样本；embargo 禁用测试段后一小段训练样本以阻断自相关。',
    'purging 和 embargo 都是删除测试段之前的训练样本。'));
  cases.push(pair('holdout-l3-preference', 'holdout-l3',
    '反复筛选污染留出集，需另取独立数据；purging/embargo 只处理时间泄漏。',
    'purging/embargo 能让反复用于筛选的留出集重新独立。'));
  cases.push(pair('holdout-l4-preference', 'holdout-l4',
    '另取未参与搜索的验证数据，并分别对标签跨界做 purging、测试后自相关做 embargo。',
    '只有时间泄漏，不必更换反复使用的留出集。'));
  cases.push(pair('target-l1-preference', 'target-l1', '方向标签的 H(Y) 约 1 bit。', '方向标签的 H(Y) 约 7.7 bit。'));
  cases.push(pair('target-l2-preference', 'target-l2',
    '目标熵降幅大于互信息损失，所以可恢复比例提高；并未创造新信息。',
    '方向标签创造了更多互信息。'));
  cases.push(pair('target-l3-preference', 'target-l3',
    '三重屏障改变标签为首达结果，IC/rank loss 改变指增优化为排序；都对齐有用结构。',
    '三重屏障直接是排序损失，IC loss 用于设置止盈止损标签。'));
  cases.push(pair('target-multi-preference', 'target-multi',
    '新目标是截面排序，应改用 IC/rank loss；这与打标选择有用结构同理。',
    '继续用 MSE 拟合精确收益即可。'));
  return { version: taskQualityVersion, status: 'candidate_not_human_reviewed', reviewer: null, reviewed_at: null,
    judge_model: null, protocol: taskQualityVersion, rules: { absolute: null, preference: null }, cases };
}
