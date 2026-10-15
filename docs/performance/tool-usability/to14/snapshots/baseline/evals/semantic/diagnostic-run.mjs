import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { loadCorpus } from './core.mjs';
import { AGENT, CHUNK, citationText } from './agent-core.mjs';
import { anonymize } from './quality-core.mjs';
import { readingSpec } from './reading-dataset.mjs';
import { answerInstruction } from './agent-dataset.mjs';
import { startServer } from './server.mjs';
import { startProviderRecorder, measuredUsage } from './provider-recorder.mjs';
import { approvedFor } from './task-calibration.mjs';
import { productState, taskJudgeJobs, taskJudgeMessages, taskQualityReport, validateTaskJudgment,
  taskQualityVersion } from './task-quality.mjs';
import { actualCanonicalEvidence, commonAnswerMessages, commonAnswerSources, diagnosticCase,
  diagnosticJudgmentConflict, diagnosticQueries, evidenceIntervention } from './diagnostic-core.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const option = (key, fallback) => { const i = args.indexOf(key); return i < 0 ? fallback : args[i + 1]; };
const sourceRun = path.resolve(option('--run', path.join(root, 'evals/semantic/results/2026-09-25-ev4-current-v1/run.json')));
const formalDir = path.resolve(option('--formal', path.join(root, 'evals/semantic/quality-results/2026-09-25-ev4-current-v1-formal')));
const bookDir = path.resolve(option('--book', path.join(root, '.understand-book/quantification-essence')));
const out = path.resolve(option('--out', path.join(root, 'evals/semantic/quality-results', `2026-09-25-ev5-ev6-${Date.now()}`)));
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const save = (file, value) => fs.writeFileSync(path.join(out, file), JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
if (fs.existsSync(out)) throw new Error('Use a new diagnostic output directory');
if (fs.existsSync(path.join(root, '.env'))) process.loadEnvFile(path.join(root, '.env'));
const provider = { base: process.env.OPENCODE_BASE_URL, key: process.env.OPENCODE_API_KEY, model: process.env.FLUID_LLM_MODEL };
if (!provider.base || !provider.key || !provider.model) throw new Error('Configure the product provider');
const run = read(sourceRun), corpus = loadCorpus(bookDir), formal = read(path.join(formalDir, 'bundle.json'));
const approval = read(path.join(formalDir, 'run-mode.json')).calibration;
const originalCase = formal.cases.find(c => c.task_id === diagnosticCase);
const originalRows = [CHUNK, AGENT].map(system => run.qa.find(r => r.id === diagnosticCase && r.system === system));
if (!originalCase || originalRows.some(r => !r) || run.corpus.book_id !== corpus.base.book_id ||
  run.corpus.source_utf16 !== corpus.source.length) throw new Error('Diagnostic input does not match frozen run and book');
if (run.model !== provider.model) throw new Error('Diagnostic model differs from source run');
for (const query of diagnosticQueries) if (!corpus.source.includes(query)) throw new Error(`Missing diagnostic source query: ${query}`);
const config = read(path.join(formalDir, 'judge-config.json'));
const judgeEnv = config.provider === 'existing'
  ? ['OPENCODE_BASE_URL', 'OPENCODE_API_KEY', 'FLUID_LLM_MODEL']
  : ['QUALITY_JUDGE_BASE_URL', 'QUALITY_JUDGE_API_KEY', 'QUALITY_JUDGE_MODEL'];
const [judgeBase, judgeKey, judgeModel] = judgeEnv.map(name => process.env[name]);
if (!judgeBase || !judgeKey || !judgeModel || judgeModel !== config.model ||
  new URL(judgeBase).origin !== config.origin) throw new Error('Judge differs from approved calibration');

fs.mkdirSync(out, { recursive: true });
save('origin.json', { source_run: sourceRun, formal_dir: formalDir, book: bookDir,
  git_head: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8', windowsHide: true }).trim(),
  model: provider.model, provider_origin: new URL(provider.base).origin, judge_config: config,
  intervention: 'At business-loop samplings 1 and 2, scripted model tool choices locate fixed canonical phrases, then call real book.text. Later model calls use the provider.',
  diagnostic_case: diagnosticCase });

async function complete(base, key, body, timeoutMs) {
  const started = performance.now();
  try {
    const response = await fetch(base.replace(/\/$/, '') + '/chat/completions', { method: 'POST',
      headers: { authorization: `Bearer ${key}`, 'content-type': 'application/json' },
      body: JSON.stringify(body), signal: AbortSignal.timeout(timeoutMs) });
    const payload = await response.json();
    return { request: body, http_status: response.status, elapsed_ms: performance.now() - started,
      usage: payload.usage ?? null, finish_reason: payload.choices?.[0]?.finish_reason ?? null,
      answer: payload.choices?.[0]?.message?.content ?? '', error: response.ok ? null : `provider_http_${response.status}` };
  } catch (error) { return { request: body, elapsed_ms: performance.now() - started, usage: null,
    error: error.name === 'TimeoutError' ? 'provider_timeout' : 'provider_transport_or_response_error' }; }
}

async function runEv5() {
  const memory = fs.mkdtempSync(path.join(os.tmpdir(), 'understand-book-ev5-'));
  const recorder = await startProviderRecorder(provider.base, { intercept: evidenceIntervention });
  let server, row = { id: diagnosticCase, system: AGENT, answer: '', sources: [], blocks: [], memory_dir: memory };
  try {
    server = await startServer(bookDir, memory, root, { OPENCODE_BASE_URL: recorder.url,
      UNDERSTAND_BOOK_PRIVATE_DIR: path.join(memory, 'private') });
    const message = `${readingSpec(diagnosticCase).user_input.question} ${answerInstruction}`;
    const outcome = await server.api('agent/chat', { message }, 'POST', 300000);
    const history = await server.api('agent/history');
    row.outcome = outcome;
    row.answer = citationText(outcome);
    row.turn_id = history.current.turns.at(-1).turn_id;
    for (const source of outcome.answer_view?.sources ?? []) {
      try {
        const resolved = await server.api('agent/source.resolve',
          { turn_id: row.turn_id, source_ref_id: source.source_ref_id }, 'POST');
        row.sources.push({ id: source.source_ref_id, valid: resolved.can_open_in_reader === true &&
          resolved.stale === false && !!resolved.highlighted_quote && corpus.source.includes(resolved.highlighted_quote),
        text: resolved.highlighted_quote ?? '', resolved });
      } catch (error) { row.sources.push({ id: source.source_ref_id, valid: false, text: '', error: error.message }); }
    }
    try {
      const forged = await server.api('agent/source.resolve',
        { turn_id: row.turn_id, source_ref_id: 'source_ref_not_in_this_turn' }, 'POST');
      row.forged_source_rejected = forged.can_open_in_reader !== true;
    } catch { row.forged_source_rejected = true; }
  } catch (error) { row.error = error.message; }
  finally { if (server) await server.stop(); await recorder.stop(60000); }
  row.requests = recorder.records;
  row.usage = measuredUsage(recorder.records.filter(r => !r.scripted));
  const readRequest = recorder.records.find(r => r.request?.messages?.some(m =>
    m.role === 'tool' && m.tool_call_id === 'call_diag_read'));
  const readMessage = readRequest?.request.messages.find(m => m.role === 'tool' && m.tool_call_id === 'call_diag_read');
  const envelope = readMessage ? JSON.parse(readMessage.content) : null;
  const text = envelope?.model_body?.text ?? '';
  const accepted = envelope?.status === 'ok' && envelope.receipt?.tool === 'book.text' &&
    envelope.receipt.accepted_evidence?.length > 0 && corpus.source.includes(text) &&
    diagnosticQueries.every(query => text.includes(query));
  const result = { row, intervention: { scripted_samplings: recorder.records.filter(r => r.scripted).length,
    read_accepted: !!accepted, read_locator: envelope?.receipt?.locator_args ?? null,
    read_text_utf16: text.length, input_request_ordinal: readRequest?.ordinal ?? null,
    forged_source_rejected: row.forged_source_rejected ?? null,
    actual_provider_usage: row.usage } };
  save('ev5-run.json', result);
  if (!accepted) throw new Error('EV5 intervention did not enter the real model input as accepted canonical text');
  return result;
}

async function runEv6() {
  const evidenceSets = originalRows.map(row => actualCanonicalEvidence(row, corpus));
  const question = `${readingSpec(diagnosticCase).user_input.question} ${answerInstruction}`;
  const answers = [];
  for (const set of evidenceSets) {
    const call = await complete(provider.base, provider.key, { model: provider.model, temperature: 0,
      max_tokens: 4096, messages: commonAnswerMessages(question, set.evidence) }, 90000);
    answers.push({ id: diagnosticCase, system: set.source_system, answer: call.answer ?? '',
      sources: commonAnswerSources(call.answer ?? '', set.evidence), usage: measuredUsage([call]),
      error: call.error || (call.finish_reason !== 'stop' ? 'Answer did not finish normally' : null), call });
  }
  const result = { evidence_sets: evidenceSets, answers,
    model: provider.model, body_budget_utf16: evidenceSets[0].budget_utf16,
    answer_prompt: commonAnswerMessages(question, [])[0].content };
  save('ev6-run.json', result);
  return result;
}

function diagnosticBundle(rows, caseId) {
  const samples = rows.map((row, i) => {
    const state = productState(row);
    return { sample_id: `${caseId}-${i + 1}`, ...state,
      ...(state.delivery === 'established' ? anonymize(row, corpus.source) : {}) };
  });
  const audit = rows.map((row, i) => ({ sample_id: samples[i].sample_id, task_id: diagnosticCase,
    system: row.system, ...productState(row), usage: row.usage, navigation_ok: null,
    historical_success: null, historical_grade_error: null }));
  return { version: taskQualityVersion, source_run_version: formal.source_run_version,
    created_at: new Date().toISOString(), cases: [{ ...originalCase, case_id: caseId, samples }], audit };
}

async function score(bundle, label) {
  const jobs = taskJudgeJobs(bundle);
  if (jobs.some(job => !approvedFor(approval, job))) throw new Error('Diagnostic scope is not approved by calibration');
  const records = [];
  for (const job of jobs) {
    const call = await complete(judgeBase, judgeKey, { model: judgeModel, temperature: 0,
      max_tokens: config.max_tokens, response_format: { type: 'json_object' },
      messages: taskJudgeMessages(job) }, 120000);
    const record = { id: job.id, kind: job.kind, model: judgeModel, usage: call.usage,
      elapsed_ms: call.elapsed_ms, raw_judgment: call.answer, error: call.error };
    if (!record.error && call.finish_reason !== 'stop') record.error = 'judge_incomplete';
    if (!record.error) try {
      record.judgment = validateTaskJudgment(job, JSON.parse(call.answer));
      if (diagnosticJudgmentConflict(job, record.judgment))
        record.error = 'invalid_judgment: fact value conflicts with all-met dimensions';
    }
    catch (error) { record.error = `invalid_judgment: ${error.message}`; }
    records.push(record);
  }
  save(`${label}-judgments.json`, records);
  const report = taskQualityReport(bundle, records);
  report.status = 'diagnostic_calibrated_model_evaluation';
  report.calibration = { status: approval.status, source: formalDir };
  save(`${label}-report.json`, report);
  return report;
}

const ev5 = await runEv5();
if (args.includes('--ev5-only')) {
  const ev5Report = await score(diagnosticBundle([ev5.row], 'D05'), 'ev5');
  save('summary.json', { status: 'ev5_only', ev5: { intervention: ev5.intervention,
    result: ev5Report.samples[0] }, judge_usage: { calls: ev5Report.judge_calls,
    tokens: ev5Report.total_tokens } });
  console.log(JSON.stringify({ out, ev5: ev5Report.samples[0].task_result,
    score_status: ev5Report.samples[0].score_status }));
} else {
const ev6 = await runEv6();
const ev5Report = await score(diagnosticBundle([ev5.row], 'D05'), 'ev5');
const ev6Report = await score(diagnosticBundle(ev6.answers, 'D06'), 'ev6');
save('summary.json', { status: 'completed', source_product_result: read(path.join(formalDir, 'report.json')).samples
  .filter(s => s.task_id === diagnosticCase).map(s => ({ system: s.system, task_result: s.task_result,
    failed_requirements: s.requirements.filter(r => r.verdict === 'unmet').map(r => r.requirement_id) })),
  ev5: { intervention: ev5.intervention, result: ev5Report.samples[0] },
  ev6: { evidence_sets: ev6.evidence_sets.map(({ source_system, budget_utf16, available_utf16,
    selected_utf16, omitted_utf16 }) => ({ source_system, budget_utf16, available_utf16,
    selected_utf16, omitted_utf16 })), results: ev6Report.samples,
    preference: ev6Report.preference, usage: ev6.answers.map(a => ({ system: a.system, usage: a.usage })) },
  judge_usage: { ev5: { calls: ev5Report.judge_calls, tokens: ev5Report.total_tokens },
    ev6: { calls: ev6Report.judge_calls, tokens: ev6Report.total_tokens } } });
console.log(JSON.stringify({ out, ev5: ev5Report.samples[0].task_result,
  ev6: ev6Report.samples.map(s => [s.system, s.task_result]), judge_calls: ev5Report.judge_calls + ev6Report.judge_calls }));
}
