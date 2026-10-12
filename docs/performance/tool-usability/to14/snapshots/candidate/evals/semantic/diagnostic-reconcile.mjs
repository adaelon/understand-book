import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { AGENT, CHUNK } from './agent-core.mjs';
import { diagnosticJudgmentConflict } from './diagnostic-core.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const option = key => { const i = args.indexOf(key); return i < 0 ? null : args[i + 1]; };
const ev5Dir = path.resolve(option('--ev5') ?? '');
const ev6Dir = path.resolve(option('--ev6') ?? '');
const formalDir = path.resolve(option('--formal') ?? path.join(root,
  'evals/semantic/quality-results/2026-09-25-ev4-current-v1-formal'));
const out = path.resolve(option('--out') ?? '');
if (!option('--ev5') || !option('--ev6') || !option('--out'))
  throw new Error('Usage: diagnostic-reconcile.mjs --ev5 <dir> --ev6 <dir> --out <new file>');
if (fs.existsSync(out)) throw new Error('Use a new report file');
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const task = read(path.join(formalDir, 'bundle.json')).cases.find(c => c.task_id === 'target-l2');
if (!task) throw new Error('Missing frozen diagnostic task');

function reconcile(dir, label) {
  const report = read(path.join(dir, `${label}-report.json`));
  const judgments = read(path.join(dir, `${label}-judgments.json`));
  const conflicts = [];
  for (const sample of report.samples) {
    const record = judgments.find(r => r.id === sample.absolute_job_id);
    if (!record?.judgment) continue;
    const job = { kind: 'absolute', input: { required_facts: task.required_facts } };
    if (!diagnosticJudgmentConflict(job, record.judgment)) continue;
    conflicts.push({ sample_id: sample.sample_id, job_id: record.id,
      reason: 'Fact value is outside the frozen acceptable enum while accuracy, completeness and grounding are all met.' });
    sample.score_status = 'invalid_output';
    sample.task_result = sample.delivery === 'failed' ? 'fail' : 'unresolved';
    sample.quality = null;
    for (const requirement of sample.requirements) {
      requirement.verdict = 'unknown';
      requirement.assessment_status = 'invalid';
      requirement.reason_code = 'judgment_disagreement';
      requirement.reason = '评委事实槽与维度判定冲突；原始输出保留待人工复核。';
      requirement.answer_span_ids = [];
      requirement.reference_ids = [];
    }
  }
  report.systems = Object.fromEntries([CHUNK, AGENT].map(system => {
    const rows = report.samples.filter(s => s.system === system);
    return [system, { total: rows.length,
      pass: rows.filter(s => s.task_result === 'pass').length,
      fail: rows.filter(s => s.task_result === 'fail').length,
      unresolved: rows.filter(s => s.task_result === 'unresolved').length,
      product_failed: rows.filter(s => s.delivery === 'failed').length,
      delivered: rows.filter(s => s.delivery === 'established').length }];
  }));
  report.score_status_counts.valid -= conflicts.length;
  report.score_status_counts.invalid_output += conflicts.length;
  report.status = 'diagnostic_reconciled_no_new_judge_calls';
  return { source_dir: dir, report, conflicts };
}

const ev5 = reconcile(ev5Dir, 'ev5'), ev6 = reconcile(ev6Dir, 'ev6');
const value = { version: 'reading-diagnostic-ev5-ev6-v1', task_id: 'target-l2',
  status: 'reconciled_from_original_judgments', ev5, ev6,
  interpretation: {
    ev5: '必要原文经真实 book.text 接纳并进入模型请求；终答交付且原始评委判来源满足。事实槽与维度冲突，完整任务结果未决。',
    ev6: '共同回答器中 Resident 取证集通过；Chunk 取证集的事实槽与维度冲突而未决。双顺序偏好为平局，不能据此宣称一组证据更优。',
  } };
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ out, ev5: ev5.report.samples.map(s => s.task_result),
  ev6: ev6.report.samples.map(s => [s.system, s.task_result]),
  conflicts: ev5.conflicts.length + ev6.conflicts.length }));
