import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCorpus } from './core.mjs';
import { absoluteInstruction, preferenceInstruction, prepareTaskBundle, taskJudgeJobs, taskJudgeMessages,
  taskQualityReport, taskQualityVersion, taskQualityVersionV2, parseTaskJudgment } from './task-quality.mjs';
import { v2Instruction, compareV2WithPrior } from './task-quality-v2.mjs';
import { approvedFor, annotateTaskCalibration, assessTaskCalibration, calibrationJobs, calibrationPacket } from './task-calibration.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2), command = args[0];
const option = (key, fallback) => { const i = args.indexOf(key); return i < 0 ? fallback : args[i + 1]; };
const selectedVersion = args.includes('--protocol') && option('--protocol', 'task-v1') === 'task-v2' ? taskQualityVersionV2 : taskQualityVersion;
const out = path.resolve(option('--out', path.join(root, `evals/semantic/quality-results/${selectedVersion === taskQualityVersionV2 ? 'task-v2' : 'task-v1'}`)));
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const writeNew = (file, data) => fs.writeFileSync(file, JSON.stringify(data, null, 2) + '\n', { flag: 'wx' });
const recordsAt = () => fs.existsSync(path.join(out, 'judgments')) ? fs.readdirSync(path.join(out, 'judgments'))
  .filter(f => f.endsWith('.json')).map(f => read(path.join(out, 'judgments', f))) : [];
const calibrationRecordsAt = dir => fs.existsSync(path.join(dir, 'calibration-judgments'))
  ? fs.readdirSync(path.join(dir, 'calibration-judgments')).filter(f => f.endsWith('.json'))
    .map(f => read(path.join(dir, 'calibration-judgments', f))) : [];
const protocol = { version: selectedVersion, absoluteInstruction: selectedVersion === taskQualityVersionV2 ? v2Instruction : absoluteInstruction, preferenceInstruction };
const safe = text => String(text ?? '').replace(/[|\r\n]/g, ' ');
function calibrationAt(dir, model) {
  const packet = read(path.join(dir, 'calibration.json'));
  const frozen = calibrationPacket(read(path.join(dir, 'bundle.json')));
  const contract = cases => cases.map(({ human, human_support, human_location, basis, ...rest }) => rest);
  if (JSON.stringify(contract(packet.cases)) !== JSON.stringify(contract(frozen.cases)))
    throw new Error('Calibration cases differ from frozen sources');
  if (JSON.stringify(calibrationJobs(packet)) !== JSON.stringify(read(path.join(dir, 'calibration-jobs.json'))))
    throw new Error('Calibration inputs differ from frozen jobs');
  return assessTaskCalibration(packet, calibrationRecordsAt(dir), { model, protocol: selectedVersion });
}
function writeCalibration() {
  const packet = calibrationPacket(read(path.join(out, 'bundle.json')));
  writeNew(path.join(out, 'calibration.json'), packet);
  writeNew(path.join(out, 'calibration-jobs.json'), calibrationJobs(packet));
  if (packet.version === taskQualityVersionV2) fs.writeFileSync(path.join(out, 'calibration-review.md'), v2ReviewMarkdown(packet), { flag: 'wx' });
}

function v2ReviewMarkdown(packet) {
  const lines = ['# task-v2 来源与语义校准复核', '',
    '判例答案与原文保留在本地。建议标签是候选；实际人审可在聊天中确认，由 Agent 记录回执及 human、basis。绝对判例另记 human_support、human_location；偏好判例的 human 为 A/B/tie/unresolved。',
    `审核状态：${packet.status}；审核人：${packet.reviewer ?? '待审'}。`, ''];
  for (const c of packet.cases) {
    lines.push(`## ${c.id}（${c.kind}，${c.family_id} / ${c.reading_level} / ${c.interaction_condition}）`, '',
      `题目：${c.input.question}`, '',
      `候选：整体 ${c.suggested_human ?? '待审'}；支持 ${c.suggested_support ?? '待审'}；定位 ${c.suggested_location ?? '待审'}。`, '');
    lines.push(`实际人审：整体 ${c.human ?? '待审'}；支持 ${c.human_support ?? '待审'}；定位 ${c.human_location ?? '待审'}。`,
      `依据：${c.basis ?? '尚未确认'}。`, '');
    for (const candidate of c.input.candidates) {
      lines.push(`### 答案 ${candidate.candidate}`, '');
      for (const span of candidate.answer_spans) lines.push(`**${span.id}**`, '', '```text', span.text, '```', '');
      for (const source of candidate.citations) {
        lines.push(`**${source.id} · ${source.valid ? '有效' : '无效'} · ${source.view_status ?? '旧视图'}**`, '');
        for (const [part, value] of [['前文', source.context_before], ['高亮', source.highlighted_quote], ['后文', source.context_after]])
          if (value) lines.push(`${part}：`, '', '```text', value, '```', '');
      }
    }
    if (c.kind === 'absolute') {
      lines.push('### 公共核验材料（不能补作交付来源）', '');
      for (const source of c.input.reference_evidence) lines.push(`**${source.id}**`, '', '```text', source.text, '```', '');
    }
  }
  return lines.join('\n') + '\n';
}

function markdown(report) {
  const historical = report.samples.some(s => s.historical_success !== null);
  const lines = ['# 分层阅读任务复评', '', report.status.startsWith('calibrated')
    ? '状态：按已审核用途与任务范围的模型复评。'
    : '状态：探索性，待 EV3 人工校准。', '',
    `协议：${report.version}；评分调用 ${report.judge_calls}/${report.expected_judge_calls}；评分 Token ${report.total_tokens ?? '未知'}；缺失 usage ${report.missing_usage}。`, '',
    '## 产品与任务结果', '', '| 系统 | 完整分母 | 通过 | 失败 | 未决 | 实际交付 | 产品交付失败 |',
    '| --- | ---: | ---: | ---: | ---: | ---: | ---: |'];
  for (const [system, s] of Object.entries(report.systems)) lines.push(`| ${safe(system)} | ${s.total} | ${s.pass} | ${s.fail} | ${s.unresolved} | ${s.delivered} | ${s.product_failed} |`);
  lines.push('', '通过、失败、未决均留在完整任务分母。`incomplete` 保留原生终态；只有实际修复交付有可观察依据时才按最终交付核验。', '',
    '## 评分有效性', '', '| 有效 | 未运行 | 传输错误 | 无效输出 |', '| ---: | ---: | ---: | ---: |',
    `| ${report.score_status_counts.valid} | ${report.score_status_counts.not_run} | ${report.score_status_counts.transport_error} | ${report.score_status_counts.invalid_output} |`, '',
    '绝对判定与配对偏好分别调用。单次偏好失败不抹掉绝对判定。', '',
    historical ? '## 逐答案新旧差异' : '## 逐答案结果', '', '| 任务 | 系统 | 执行 | 交付 | 旧成功 | 新任务结果 | 绝对评分 | 配对状态 |',
    '| --- | --- | --- | --- | --- | --- | --- | --- |');
  for (const s of report.samples) lines.push(`| ${s.task_id} | ${safe(s.system)} | ${s.execution} | ${s.delivery} | ${s.historical_success} | ${s.task_result} | ${s.score_status} | ${s.preference} |`);
  lines.push('', '## 逐要求结果', '', '| 任务 | 系统 | 要求 | 类型 | 判定 | 评分状态 | 原因 |',
    '| --- | --- | --- | --- | --- | --- | --- |');
  for (const s of report.samples) for (const r of s.requirements)
    lines.push(`| ${s.task_id} | ${safe(s.system)} | ${r.requirement_id} | ${r.kind} | ${r.verdict} | ${r.assessment_status} | ${safe(r.reason_code)} |`);
  lines.push('', '## 限制', '', historical
    ? '历史样本只用于协议和故障形状复核，不代表当前版本成绩；旧评分错误不直接改判为产品成功。原文、答案、匿名映射与评分原始记录只留在本地忽略目录。'
    : '当前版本结果只覆盖本题集与已获校准批准的评分范围；未获批准的答案保持未评。原文、答案、匿名映射与评分原始记录只留在本地忽略目录。');
  return lines.join('\n') + '\n';
}

function rebuildReport() {
  const bundle = read(path.join(out, 'bundle.json'));
  const report = taskQualityReport(bundle, recordsAt());
  const modeFile = path.join(out, 'run-mode.json');
  if (fs.existsSync(modeFile)) {
    const mode = read(modeFile);
    report.status = report.judge_calls === report.expected_judge_calls ? 'calibrated_model_evaluation' : 'calibrated_partial_evaluation';
    report.calibration = mode.calibration;
    annotateTaskCalibration(report, bundle, mode.calibration);
  }
  fs.writeFileSync(path.join(out, 'report.json'), JSON.stringify(report, null, 2) + '\n');
  fs.writeFileSync(path.join(out, 'report.md'), markdown(report));
  console.log(JSON.stringify({ out, systems: report.systems, judge_calls: report.judge_calls, status: report.status }));
}

if (command === 'prepare') {
  if (fs.existsSync(out)) throw new Error('Use a new output directory; prepared inputs are immutable');
  const runFile = path.resolve(option('--run', path.join(root, 'evals/semantic/results/2026-09-08-la7/run.json')));
  const book = path.resolve(option('--book', path.join(root, '.understand-book/quantification-essence')));
  const bundle = prepareTaskBundle(read(runFile), loadCorpus(book), undefined, selectedVersion), jobs = taskJudgeJobs(bundle);
  fs.mkdirSync(out, { recursive: true });
  writeNew(path.join(out, 'bundle.json'), bundle);
  writeNew(path.join(out, 'jobs.json'), jobs);
  writeNew(path.join(out, 'protocol.json'), protocol);
  writeNew(path.join(out, 'origin.json'), { historical_run: runFile, book });
  writeCalibration();
  rebuildReport();
  console.log(JSON.stringify({ prepared: bundle.cases.length, jobs: jobs.length }));
} else if (command === 'calibration-prepare') {
  writeCalibration();
  console.log(JSON.stringify({ out, calibration_cases: read(path.join(out, 'calibration.json')).cases.length }));
} else if (command === 'calibration-review') {
  const packet = read(path.join(out, 'calibration.json'));
  if (packet.version !== taskQualityVersionV2) throw new Error('Review view requires task-v2');
  fs.writeFileSync(path.join(out, 'calibration-review.md'), v2ReviewMarkdown(packet));
  console.log(JSON.stringify({ out, review_cases: packet.cases.length }));
} else if (command === 'report') rebuildReport();
else if (command === 'compare') {
  if (selectedVersion !== taskQualityVersionV2) throw new Error('Comparison requires task-v2');
  const oldReport = option('--old-report', null);
  if (!oldReport) throw new Error('Provide --old-report');
  const comparison = compareV2WithPrior(read(path.join(out, 'report.json')), read(path.resolve(oldReport)));
  fs.writeFileSync(path.join(out, 'comparison.json'), JSON.stringify(comparison, null, 2) + '\n');
  const lines = ['# 同批答案 task-v1 / task-v2 复评差异', '',
    `样本：${comparison.total}；task-v2 状态：${read(path.join(out, 'report.json')).status}。`, '',
    '| 任务 | 系统 | 旧结果 | 新结果 | 分类 | 评分未决原因 |', '| --- | --- | --- | --- | --- | --- |'];
  for (const s of comparison.samples) lines.push(`| ${safe(s.task_id)} | ${safe(s.system)} | ${s.old_task_result} | ${s.new_task_result} | ${s.failure_categories.join(', ') || '无'} | ${safe(s.new_score_reason ?? '—')} |`);
  lines.push('', '逐要求依据与论断片段见 `comparison.json`；未获有效新判定的答案保持未决。', '');
  fs.writeFileSync(path.join(out, 'comparison.md'), lines.join('\n'));
  console.log(JSON.stringify({ out, compared: comparison.total }));
}
else if (command === 'calibrate') {
  const configFile = path.join(out, 'judge-config.json');
  const result = calibrationAt(out, fs.existsSync(configFile) ? read(configFile).model : null);
  fs.writeFileSync(path.join(out, 'calibration-result.json'), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result));
} else if (command === 'judge') {
  const calibrationMode = args.includes('--calibration');
  const exploratory = args.includes('--exploratory');
  const calibrationFrom = option('--calibration-from', null);
  if (!calibrationMode && !exploratory && !calibrationFrom)
    throw new Error('Provide --exploratory or --calibration-from');
  if (calibrationMode && calibrationFrom) throw new Error('Calibration jobs cannot use --calibration-from');
  if (JSON.stringify(read(path.join(out, 'protocol.json'))) !== JSON.stringify(protocol)) throw new Error('Prepared protocol differs');
  if (fs.existsSync(path.join(root, '.env'))) process.loadEnvFile(path.join(root, '.env'));
  const provider = option('--provider', 'quality');
  if (!['quality', 'existing'].includes(provider)) throw new Error('Unknown provider configuration');
  const names = provider === 'quality' ? ['QUALITY_JUDGE_BASE_URL', 'QUALITY_JUDGE_API_KEY', 'QUALITY_JUDGE_MODEL']
    : ['OPENCODE_BASE_URL', 'OPENCODE_API_KEY', 'FLUID_LLM_MODEL'];
  const [base, key, model] = names.map(name => process.env[name]);
  if (!base || !key || !model) throw new Error(`Set ${names.join(', ')}`);
  const max_tokens = Number(option('--max-output-tokens', '12000'));
  const limit = Number(option('--limit', '1000'));
  if (!Number.isInteger(max_tokens) || max_tokens < 1 || !Number.isInteger(limit) || limit < 1) throw new Error('Invalid output budget or limit');
  const config = { version: selectedVersion, model, provider, origin: new URL(base).origin, max_tokens, temperature: 0 };
  const configFile = path.join(out, 'judge-config.json');
  if (fs.existsSync(configFile) && JSON.stringify(read(configFile)) !== JSON.stringify(config)) throw new Error('Judge configuration changed');
  if (!fs.existsSync(configFile)) writeNew(configFile, config);
  const dir = path.join(out, calibrationMode ? 'calibration-judgments' : 'judgments'); fs.mkdirSync(dir, { recursive: true });
  const all = calibrationMode ? read(path.join(out, 'calibration-jobs.json')) : taskJudgeJobs(read(path.join(out, 'bundle.json')));
  let approval = null;
  if (calibrationFrom) {
    const calibrationDir = path.resolve(calibrationFrom);
    if (JSON.stringify(read(path.join(calibrationDir, 'judge-config.json'))) !== JSON.stringify(config))
      throw new Error('Calibration judge configuration differs');
    if (JSON.stringify(read(path.join(calibrationDir, 'protocol.json'))) !== JSON.stringify(protocol))
      throw new Error('Calibration protocol differs');
    approval = calibrationAt(calibrationDir, model);
  }
  const only = option('--only', '').split(',').filter(Boolean);
  if (only.some(id => !all.some(j => j.id === id || j.case_id === id))) throw new Error('Unknown case or job');
  const requested = all.filter(j => !only.length || only.includes(j.id) || only.includes(j.case_id));
  if (approval && only.length && requested.some(j => !approvedFor(approval, j)))
    throw new Error('Calibration does not approve all selected jobs and purposes');
  const selected = approval ? requested.filter(j => approvedFor(approval, j)) : requested;
  const modeFile = path.join(out, 'run-mode.json');
  if (approval && !fs.existsSync(modeFile)) {
    if (recordsAt().length) throw new Error('Formal evaluation requires a fresh output directory');
    writeNew(modeFile, { calibration_dir: calibrationFrom, calibration: approval });
  } else if (approval && JSON.stringify(read(modeFile).calibration) !== JSON.stringify(approval))
    throw new Error('Calibration approval changed during this run');
  else if (!approval && !calibrationMode && fs.existsSync(modeFile))
    throw new Error('Cannot mix exploratory judgments into a formal run');
  const jobs = selected.filter(j => !fs.existsSync(path.join(dir, `${j.id}.json`))).slice(0, limit);
  for (const job of jobs) {
    const record = { id: job.id, kind: job.kind, started_at: new Date().toISOString(), model, usage: null };
    const started = performance.now();
    try {
      const response = await fetch(base.replace(/\/$/, '') + '/chat/completions', { method: 'POST',
        headers: { authorization: `Bearer ${key}`, 'content-type': 'application/json' },
        body: JSON.stringify({ model, messages: taskJudgeMessages(job), temperature: 0, max_tokens,
          response_format: { type: 'json_object' } }), signal: AbortSignal.timeout(120000) });
      record.http_status = response.status;
      if (!response.ok) record.error = `provider_http_${response.status}`;
      else {
        const result = await response.json(); record.usage = result.usage ?? null;
        record.finish_reason = result.choices?.[0]?.finish_reason;
        record.raw_judgment = result.choices?.[0]?.message?.content ?? '';
        if (record.finish_reason !== 'stop') record.error = 'judge_incomplete';
        else try { record.judgment = parseTaskJudgment(job, record.raw_judgment); }
        catch (error) { record.error = `invalid_judgment: ${error instanceof SyntaxError ? 'JSON parse failed' : error.message}`; }
      }
    } catch (error) { record.error = error.name === 'TimeoutError' ? 'judge_timeout' : 'judge_transport_or_response_error'; }
    record.elapsed_ms = performance.now() - started;
    writeNew(path.join(dir, `${job.id}.json`), record);
    console.log(`${job.id}: ${record.error ?? 'judged'}`);
  }
  if (!calibrationMode) rebuildReport();
} else throw new Error('Usage: quality-run.mjs prepare|calibration-prepare|judge|calibrate|report|compare --protocol task-v1|task-v2 --out <directory>');
