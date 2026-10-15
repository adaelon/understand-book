import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCorpus } from './core.mjs';
import { prepareBundle, judgeJobs, judgeMessages, validateJudgment, qualityReport, qualityVersion, assessCalibration, judgeInstruction } from './quality-core.mjs';

const selectedProtocol = process.argv.includes('--protocol') ? process.argv[process.argv.indexOf('--protocol') + 1] : 'historical-v2';
if (!['historical-v2', 'task-v1', 'task-v2'].includes(selectedProtocol)) throw new Error(`Unknown quality protocol: ${selectedProtocol}`);
if (selectedProtocol === 'task-v1' || selectedProtocol === 'task-v2') {
  await import('./task-quality-run.mjs');
  process.exit(0);
}

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2), command = args[0];
const option = (name, fallback) => { const i = args.indexOf(name); return i < 0 ? fallback : args[i + 1]; };
const out = path.resolve(option('--out', path.join(root, 'evals/semantic/quality-results/la7-v1')));
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const writeNew = (file, data) => fs.writeFileSync(file, JSON.stringify(data, null, 2) + '\n', { flag: 'wx' });
const safe = value => String(value ?? '').replace(/[|\r\n]/g, ' ');
const recordsAt = dir => fs.existsSync(path.join(dir, 'judgments')) ? fs.readdirSync(path.join(dir, 'judgments')).filter(f => f.endsWith('.json')).map(f => read(path.join(dir, 'judgments', f))) : [];

function calibrationAt(dir) {
  const packet = read(path.join(dir, 'calibration.json'));
  return assessCalibration(packet, judgeJobs(read(path.join(dir, 'bundle.json'))), recordsAt(dir));
}

function calibrationPacket(bundle, jobs) {
  const ids = ['exact-01', 'concept-01', 'concept-04', 'contrast-03', 'formula-01', 'cross-02'];
  const caseIds = [...new Set(bundle.audit.filter(a => ids.includes(a.task_id)).map(a => a.sample_id.split('-')[0]))];
  return { version: qualityVersion, status: 'draft_not_human_gold', reviewer: null, reviewed_at: null,
    instructions: '逐题核对原文和实际答句。填写各候选的四维 verdict、必要事实抽取及配对 preference；保持未审核为 null。审核完成后记录 reviewer/reviewed_at。人工意见与模型意见不同需记录具体依据。此文件尚不能证明评委校准通过。',
    cases: caseIds.map(id => ({ case_id: id, input: jobs.find(j => j.case_id === id).input,
      judgments: null, notes: '', checks: ['事实值来自实际答案而不是参考答案', '来源存在且支持结论', '正确简答不因篇幅短扣分', '历史摘句改写不影响本次按片段编号核验'] })) };
}

function reviewMarkdown(packet) {
  const lines = ['# 质量评委人工校准包', '', '状态：待人工审核，尚不是人工金标。', '',
    '此包包含六道代表题的真实回答。请对每份答案判断准确、完整、证据、解释四项为 met / partial / failed / unknown；无需解释的题目最后一项为 not_applicable。双答案另选 A / B / tie / unresolved。请指出关键理由。', '',
    '在 calibration.json 中填写 judgments 与 notes，并填写 reviewer、reviewed_at。未知就保留未知；不能因参考答案正确而替实际答案补答。', ''];
  for (const c of packet.cases) {
    lines.push(`## ${c.case_id}：${c.input.question}`, '', `需要解释：${c.input.explanation_required ? '是' : '否'}`, '', '参考事实：', '', ...c.input.required_facts.map(f => `- ${f.key}：${f.description}；可接受值 ${JSON.stringify(f.acceptable)}`), '');
    for (const r of c.input.reference_evidence) lines.push(`### 参考 ${r.id}`, '', ...r.text.split('\n').map(l => '> ' + l), '');
    for (const candidate of c.input.candidates) {
      lines.push(`### 答案 ${candidate.candidate}`, '');
      for (const s of candidate.answer_spans) lines.push(`**${s.id}**`, '', ...s.text.split('\n').map(l => '> ' + l), '');
      for (const s of candidate.citations) lines.push(`**${s.id}（${s.valid ? '有效来源' : '无有效绑定'}）**`, '', ...s.text.split('\n').map(l => '> ' + l), '');
    }
    lines.push('人工判定：待填写。', '');
  }
  return lines.join('\n');
}

function markdown(report, bundle) {
  const lines = ['# 历史答案质量复评', '', report.status.startsWith('calibrated') ? '**状态：已通过所选人工校准集的模型复评。** 所有旧运行只读。' : '**状态：探索性模型复评，待人工校准。** 本报告不构成架构胜出结论；所有旧运行只读。', '',
    `协议：${report.version}；实际评委调用 ${report.judge_calls}；实际总 Token：${report.total_tokens ?? '未知'}；缺失 usage ${report.missing_usage}。`, '',
    '## 交付与判分', '', '| 系统 | 全部任务 | 正常交付 | 新判通过 | 新判失败 | 待判定 | 产品未交付 |', '| --- | ---: | ---: | ---: | ---: | ---: | ---: |'];
  for (const [name, s] of Object.entries(report.systems)) lines.push(`| ${safe(name)} | ${s.total} | ${s.delivered} | ${s.pass} | ${s.fail} | ${s.unscorable} | ${s.product_failed} |`);
  lines.push('', '## 回答质量维度', '', '| 系统 | 维度 | 满足 | 部分满足 | 不满足 | 证据不足 | 不适用 | 判分失败或顺序分歧 |', '| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |');
  for (const [system, ds] of Object.entries(report.dimension_summary)) for (const [name, c] of Object.entries(ds)) lines.push(`| ${safe(system)} | ${name} | ${c.met} | ${c.partial} | ${c.failed} | ${c.unknown} | ${c.not_applicable} | ${c.unscorable} |`);
  lines.push('', '每个维度的分母为该系统正常交付的答案数；未交付数量见上表。两个顺序的同维度标签不一致时列为分歧。');
  lines.push('', '质量标准加入额外错误与解释要求，与旧事实槽判据不同；新旧差异不是产品性能变化。待判定保留在完整分母中。', '',
    '## 双方已交付子集的比较', '', '| 题目 | 两个顺序一致的结果 |', '| --- | --- |');
  for (const c of report.comparison) lines.push(`| ${c.case_id} | ${safe(c.system ?? c.winner)} |`);
  const paired = report.comparison.filter(c => c.winner !== 'not_paired');
  lines.push('', `双方可比较：${paired.length}/${bundle.cases.length} 题。未配对任务仍计入上表；位置分歧或评委失败标记 unresolved。`, '',
    '## 逐答案新旧判定', '', '| 题目 | 系统 | 交付 | 旧成功 | 新判 | 顺序分歧 |', '| --- | --- | --- | --- | --- | --- |');
  for (const s of report.samples) lines.push(`| ${s.task_id} | ${safe(s.system)} | ${s.execution} | ${s.historical_success} | ${s.quality_status} | ${s.order_disagreement} |`);
  lines.push('', '## 逐题判定依据', '');
  for (const s of report.samples.filter(s => s.execution === 'delivered')) {
    lines.push(`### ${s.task_id} / ${s.system}`, '');
    for (const j of s.judgments) {
      lines.push(`**${j.job_id}：${j.status}**`, '');
      if (j.error) { lines.push(`判分状态：${safe(j.error)}`, ''); continue; }
      for (const [name, d] of Object.entries(j.evaluation.dimensions)) lines.push(`- ${name}：${d.verdict}；${safe(d.reason)}；答句 ${d.answer_spans.join(', ') || '无'}；证据 ${d.evidence_ids.join(', ') || '无'}`);
      for (const f of j.evaluation.facts) lines.push(`- 事实 ${f.key}：${JSON.stringify(f.value)}；${safe(f.reason)}；答句 ${f.answer_spans.join(', ') || '无'}；引用 ${f.support_ids.join(', ') || '无'}`);
      for (const e of j.evaluation.critical_errors) lines.push(`- 实质性错误：${safe(e.reason)}；答句 ${e.answer_spans.join(', ')}；证据 ${e.evidence_ids.join(', ')}`);
      lines.push('');
    }
  }
  lines.push('## 限制', '', '历史单书开发集、非独立盲测；人工校准状态以报告顶部为准，小型校准集不代表全面可信。匿名处理移除系统标签和来源前缀，答案文风仍可能暴露实现特征。参考摘录不等于全书穷尽证明，unknown 需人工补证。此报告与样本包包含原书摘录及答案，仅供本地审核。');
  return lines.join('\n') + '\n';
}

function rebuildReport() {
  const bundle = read(path.join(out, 'bundle.json'));
  const records = recordsAt(out);
  const report = qualityReport(bundle, records);
  const modeFile = path.join(out, 'evaluation-mode.json');
  if (fs.existsSync(modeFile)) {
    const mode = read(modeFile);
    if (mode.mode === 'calibrated') report.status = records.length === report.expected_judge_calls ? 'calibrated_model_evaluation' : 'calibrated_partial_evaluation';
    report.calibration = mode.calibration ?? null;
  }
  fs.writeFileSync(path.join(out, 'report.json'), JSON.stringify(report, null, 2) + '\n');
  fs.writeFileSync(path.join(out, 'report.md'), markdown(report, bundle));
  console.log(JSON.stringify({ out, judge_calls: report.judge_calls, systems: report.systems, status: report.status }));
}

if (command === 'prepare') {
  if (fs.existsSync(out)) throw new Error('Use a new output directory; prepared inputs are immutable');
  const runFile = path.resolve(option('--run', path.join(root, 'evals/semantic/results/2026-09-08-la7/run.json')));
  const corpus = loadCorpus(path.resolve(option('--book', path.join(root, '.understand-book/quantification-essence'))));
  const bundle = prepareBundle(read(runFile), corpus), jobs = judgeJobs(bundle);
  fs.mkdirSync(out, { recursive: true });
  writeNew(path.join(out, 'bundle.json'), bundle);
  writeNew(path.join(out, 'jobs.json'), jobs);
  writeNew(path.join(out, 'protocol.json'), { version: qualityVersion, instruction: judgeInstruction });
  writeNew(path.join(out, 'origin.json'), { historical_run: runFile });
  const packet = calibrationPacket(bundle, jobs);
  writeNew(path.join(out, 'calibration.json'), packet);
  fs.writeFileSync(path.join(out, 'calibration-review.md'), reviewMarkdown(packet), { flag: 'wx' });
  console.log(JSON.stringify({ prepared: bundle.cases.length, jobs: jobs.length, calibration_cases: packet.cases.length, out }));
  rebuildReport();
} else if (command === 'judge') {
  const exploratory = args.includes('--exploratory');
  const calibrationDir = option('--calibration-from', null);
  if (!exploratory && !calibrationDir) throw new Error('Provide a reviewed --calibration-from directory, or --exploratory for provisional evaluation');
  if (fs.existsSync(path.join(root, '.env'))) process.loadEnvFile(path.join(root, '.env'));
  const provider = option('--provider', 'quality');
  if (!['quality', 'existing'].includes(provider)) throw new Error('Unknown provider configuration');
  const prefix = provider === 'quality' ? ['QUALITY_JUDGE_BASE_URL', 'QUALITY_JUDGE_API_KEY', 'QUALITY_JUDGE_MODEL'] : ['OPENCODE_BASE_URL', 'OPENCODE_API_KEY', 'FLUID_LLM_MODEL'];
  const [base, key, model] = prefix.map(k => process.env[k]);
  if (!base || !key || !model) throw new Error(`Set ${prefix.join(', ')}`);
  const maxOutputTokens = Number(option('--max-output-tokens', '12000'));
  if (!Number.isInteger(maxOutputTokens) || maxOutputTokens < 1) throw new Error('Invalid judge output budget');
  const metadata = { version: qualityVersion, model, origin: new URL(base).origin, temperature: 0, max_output_tokens: maxOutputTokens, provider_configuration: provider };
  const protocol = { version: qualityVersion, instruction: judgeInstruction };
  const protocolFile = path.join(out, 'protocol.json');
  if (!fs.existsSync(protocolFile) || JSON.stringify(read(protocolFile)) !== JSON.stringify(protocol)) throw new Error('Prepared judge protocol differs; prepare a new directory');
  const mode = { mode: exploratory ? 'exploratory' : 'calibrated' };
  if (!exploratory) {
    mode.calibration = calibrationAt(path.resolve(calibrationDir));
    if (mode.calibration.status !== 'passed') throw new Error(`Calibration is not passed: ${mode.calibration.status}`);
    if (JSON.stringify(read(path.join(calibrationDir, 'judge-config.json'))) !== JSON.stringify(metadata)) throw new Error('Calibration judge configuration differs');
    if (JSON.stringify(read(path.join(calibrationDir, 'protocol.json'))) !== JSON.stringify(protocol)) throw new Error('Calibration prompt differs');
  }
  const modeFile = path.join(out, 'evaluation-mode.json');
  if (fs.existsSync(modeFile)) {
    if (JSON.stringify(read(modeFile)) !== JSON.stringify(mode)) throw new Error('Evaluation mode changed; use a new prepared directory');
  } else {
    if (!exploratory && recordsAt(out).length) throw new Error('Do not relabel exploratory results; prepare a new directory');
    writeNew(modeFile, mode);
  }
  const configFile = path.join(out, 'judge-config.json');
  if (fs.existsSync(configFile)) {
    if (JSON.stringify(read(configFile)) !== JSON.stringify(metadata)) throw new Error('Judge configuration changed; use a new prepared directory');
  } else writeNew(configFile, metadata);
  const allJobs = judgeJobs(read(path.join(out, 'bundle.json')));
  const selected = option('--only', '').split(',').filter(Boolean);
  if (selected.some(id => !allJobs.some(j => j.id === id || j.case_id === id))) throw new Error('Unknown case or job');
  const limit = Number(option('--limit', String(allJobs.length)));
  const concurrency = Number(option('--concurrency', '1'));
  if (!Number.isInteger(limit) || limit < 1 || ![1, 2].includes(concurrency)) throw new Error('Invalid limit/concurrency');
  const dir = path.join(out, 'judgments'); fs.mkdirSync(dir, { recursive: true });
  const jobs = allJobs.filter(j => (!selected.length || selected.includes(j.id) || selected.includes(j.case_id)) && !fs.existsSync(path.join(dir, `${j.id}.json`))).slice(0, limit);
  let next = 0;
  async function worker() {
    while (next < jobs.length) {
      const job = jobs[next++], record = { id: job.id, started_at: new Date().toISOString(), model, usage: null };
      const started = performance.now();
      try {
        const response = await fetch(base.replace(/\/$/, '') + '/chat/completions', {
          method: 'POST', headers: { authorization: `Bearer ${key}`, 'content-type': 'application/json' },
          body: JSON.stringify({ model, messages: judgeMessages(job), temperature: 0, max_tokens: maxOutputTokens, response_format: { type: 'json_object' } }),
          signal: AbortSignal.timeout(120000),
        });
        record.http_status = response.status;
        if (!response.ok) record.error = `provider_http_${response.status}`;
        else {
          const result = await response.json(); record.usage = result.usage ?? null;
          record.finish_reason = result.choices?.[0]?.finish_reason;
          record.raw_judgment = result.choices?.[0]?.message?.content ?? '';
          if (record.finish_reason !== 'stop') record.error = 'judge_incomplete';
          else try { record.judgment = validateJudgment(job, JSON.parse(record.raw_judgment)); }
          catch (e) { record.error = `invalid_judgment: ${e instanceof SyntaxError ? 'JSON parse failed' : e.message}`; }
        }
      } catch (e) { record.error = e.name === 'TimeoutError' ? 'judge_timeout' : 'judge_transport_or_response_error'; }
      record.elapsed_ms = performance.now() - started;
      writeNew(path.join(dir, `${job.id}.json`), record);
      console.log(`${job.id}: ${record.error ?? 'judged'} (${Math.round(record.elapsed_ms / 1000)}s)`);
    }
  }
  await Promise.all(Array.from({ length: concurrency }, worker));
  rebuildReport();
} else if (command === 'report') rebuildReport();
else if (command === 'calibrate') {
  const result = calibrationAt(out);
  fs.writeFileSync(path.join(out, 'calibration-result.json'), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result));
  if (result.status !== 'passed') process.exitCode = 2;
} else throw new Error('Usage: quality-run.mjs prepare|judge|report|calibrate --out <directory>');
