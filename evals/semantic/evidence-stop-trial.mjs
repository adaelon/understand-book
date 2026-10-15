// Run the existing three-question product runner in ABBA blocks. A local
// streaming observer retains only original source-reading tool receipts.
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import http from 'node:http';
import https from 'node:https';
import assert from 'node:assert/strict';
import { loadCorpus } from './core.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const option = (key, fallback) => { const index = args.indexOf(key); return index < 0 ? fallback : args[index + 1]; };
const output = path.join(root, 'docs/performance/evidence-stop-20261015/results');
const logDirectory = path.join(root, 'tmp/evidence-stop-trial-20261015');
const phases = ['baseline-r1', 'candidate-r1', 'candidate-r2', 'baseline-r2'];
const questionIds = ['cross-01', 'cross-02', 'exact-02'];
const usageKeys = ['input_tokens', 'output_tokens', 'reasoning_output_tokens', 'cached_input_tokens', 'cache_creation_input_tokens', 'total_tokens'];
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const write = (file, value) => fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
const sumKnown = (rows, get) => rows.length && rows.every(row => Number.isFinite(get(row)))
  ? rows.reduce((sum, row) => sum + get(row), 0) : null;
const sumUsage = models => Object.fromEntries(usageKeys.map(key => [key, sumKnown(models, model => model.usage?.[key])]));
const retrievalTools = new Set(['book.text', 'book.text_batch', 'book.context', 'book.structure', 'book.search_text', 'book.query']);
const corpus = loadCorpus(path.join(root, '.understand-book/quantification-essence'));
const parse = text => { try { return JSON.parse(text); } catch { return null; } };
const sameArgs = (a, b) => JSON.stringify(parse(a)) === JSON.stringify(parse(b));

// Capture only source-reading tool receipts already sent to the answer model.
// Requests and streaming responses are forwarded unchanged; usage comes from history.
async function captureReads(providerBase, getCaseId, saveReceipts) {
  const receipts = [];
  const seen = new Set();
  let requests = 0;
  const server = http.createServer(async (req, res) => {
    try {
      const chunks = [];
      for await (const chunk of req) chunks.push(chunk);
      const body = Buffer.concat(chunks);
      const payload = parse(body.toString('utf8'));
      const caseId = getCaseId();
      const calls = new Map((payload?.messages ?? []).flatMap(message => message.tool_calls ?? [])
        .map(call => [call.id, call.function]));
      requests += 1;
      for (const message of payload?.messages ?? []) {
        const call = calls.get(message.tool_call_id);
        const tool = { book_text: 'book.text', book_text_batch: 'book.text_batch', 'book.text': 'book.text', 'book.text_batch': 'book.text_batch' }[call?.name];
        if (message.role !== 'tool' || !tool) continue;
        const identity = `${caseId}:${message.tool_call_id}`;
        if (seen.has(identity)) continue;
        seen.add(identity);
        receipts.push({ case_id: caseId, first_seen_request: requests, tool_call_id: message.tool_call_id,
          tool, arguments: call.arguments, content: message.content });
      }
      saveReceipts(receipts);
      const target = new URL(providerBase.replace(/\/$/, '') + req.url);
      const upstream = (target.protocol === 'https:' ? https : http).request(target,
        { method: req.method, headers: { ...req.headers, host: target.host } }, incoming => {
          res.writeHead(incoming.statusCode, incoming.headers);
          incoming.pipe(res);
        });
      upstream.on('error', () => {
        if (!res.headersSent) res.writeHead(502, { 'content-type': 'application/json' });
        res.end(JSON.stringify({ error: { message: 'Local source-receipt forwarding failed' } }));
      });
      upstream.end(body);
    } catch {
      if (!res.headersSent) res.writeHead(502, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ error: { message: 'Local source-receipt capture failed' } }));
    }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  return { url: `http://127.0.0.1:${server.address().port}`, receipts,
    stop: () => new Promise(resolve => { server.closeAllConnections(); server.close(resolve); }) };
}

function readCoverage(row, trace, models, receipts) {
  const caseReceipts = receipts.filter(receipt => receipt.case_id === row.id);
  const unused = [...caseReceipts];
  const readCalls = trace.filter(call => ['book.text', 'book.text_batch'].includes(call.tool)).map(call => {
    const index = unused.findIndex(receipt => receipt.tool === call.tool && sameArgs(receipt.arguments, call.args));
    const receipt = index < 0 ? null : unused.splice(index, 1)[0];
    const content = typeof receipt?.content === 'string' ? receipt.content : null;
    const envelope = content === null ? null : parse(content);
    const value = envelope?.version === 'tool_result_envelope.v1' ? envelope.model_body : envelope;
    const items = call.tool === 'book.text_batch' ? (value?.items ?? []).map(item => item.result) : [value];
    const texts = items.filter(item => item && !item.error_code && typeof item.text === 'string' && item.text.trim())
      .map(item => item.text);
    const covered = (row.lids ?? []).filter(lid => {
      const node = corpus.base.lid_nodes.find(node => node.lid === lid);
      const original = node ? corpus.source.slice(node.span.start, node.span.end).trim() : '';
      return original.length > 0 && texts.some(text => text.includes(original));
    });
    return { model_tool_loop: call.model_tool_loop, tool: call.tool, arguments: call.args,
      receipt_available: !!receipt, receipt_parseable: value !== null, successful_nonempty_text_items: texts.length,
      covered_requested_lids: covered, error_code: value?.error_code ?? null };
  });
  const requested = (row.lids ?? []).map(lid => ({ lid,
    first_successful_text_round: readCalls.find(call => call.covered_requested_lids.includes(lid))?.model_tool_loop ?? null }));
  const complete = requested.length > 0 && requested.every(item => item.first_successful_text_round !== null);
  const readCompleteRound = complete ? Math.max(...requested.map(item => item.first_successful_text_round)) : null;
  const firstSourceRound = trace.find(call => call.tool === 'source.present')?.model_tool_loop ?? null;
  const outerModels = models.filter(model => model.name === 'outer');
  const afterModel = readCompleteRound === null ? null : outerModels[readCompleteRound];
  const citationModel = firstSourceRound === null ? null : outerModels[firstSourceRound - 1];
  const expansionModels = afterModel && citationModel ? models.filter(model => model.step_id >= afterModel.step_id && model.step_id < citationModel.step_id) : [];
  const unknown = !complete && readCalls.some(call => !call.receipt_available || !call.receipt_parseable);
  return { criterion: 'Actual captured tool receipt contains the full canonical text of each requested LID; arguments and activity success alone are insufficient.',
    requested_locations: requested, all_requested_locations_read: unknown ? null : complete,
    requested_reads_complete_round: readCompleteRound,
    read_calls: readCalls,
    initial_round_extra_location_calls: trace.filter(call => call.model_tool_loop === 1 && retrievalTools.has(call.tool) && !['book.text', 'book.text_batch'].includes(call.tool))
      .map(({ model_tool_loop, tool, args }) => ({ model_tool_loop, tool, args })),
    after_requested_reads_retrieval_calls: readCompleteRound === null ? null : trace
      .filter(call => call.model_tool_loop > readCompleteRound && retrievalTools.has(call.tool))
      .map(({ model_tool_loop, tool, args }) => ({ model_tool_loop, tool, args })),
    after_requested_reads_before_citation_models: complete && citationModel ? expansionModels.length : null,
    after_requested_reads_before_citation_usage: complete && citationModel
      ? (expansionModels.length ? sumUsage(expansionModels) : Object.fromEntries(usageKeys.map(key => [key, key === 'cache_creation_input_tokens' ? null : 0]))) : null,
    sufficiency: 'Reading completion is observed; whether those passages suffice for the requested scope is checked manually.',
  };
}

if (args.includes('--self-test')) {
  let received;
  const stub = http.createServer(async (req, res) => {
    const chunks = []; for await (const chunk of req) chunks.push(chunk);
    received = { path: req.url, body: Buffer.concat(chunks).toString('utf8'), auth: req.headers.authorization };
    res.writeHead(200, { 'content-type': 'text/event-stream' });
    res.write('data: first\n\n'); setImmediate(() => res.end('data: [DONE]\n\n'));
  });
  await new Promise(resolve => stub.listen(0, '127.0.0.1', resolve));
  const node = corpus.base.lid_nodes.find(node => node.lid === '1.9.5.2');
  const text = corpus.source.slice(node.span.start, node.span.end);
  const request = JSON.stringify({ model: 'local-stub', stream: true, messages: [
    { role: 'system', content: 'never archive this' },
    { role: 'assistant', tool_calls: [{ id: 'read-1', type: 'function', function: { name: 'book_text', arguments: '{"lid":"1.9.5.2"}' } }] },
    { role: 'tool', tool_call_id: 'read-1', content: JSON.stringify({ version: 'tool_result_envelope.v1', status: 'ok', model_body: { lid: '1.9.5.2', text } }) }] });
  const proxy = await captureReads(`http://127.0.0.1:${stub.address().port}`, () => 'exact-02', () => {});
  try {
    const response = await fetch(`${proxy.url}/chat/completions?fixture=1`, { method: 'POST', headers: { 'content-type': 'application/json', authorization: 'Bearer local-fixture' }, body: request });
    assert.equal(await response.text(), 'data: first\n\ndata: [DONE]\n\n');
    assert.equal(received.body, request); assert.equal(received.path, '/chat/completions?fixture=1');
    assert.equal(received.auth, 'Bearer local-fixture');
    assert.equal(proxy.receipts.length, 1); assert(!JSON.stringify(proxy.receipts).includes('never archive'));
    assert(!JSON.stringify(proxy.receipts).includes('Bearer'));
    const trace = [{ tool: 'book.text', args: '{"lid":"1.9.5.2"}', model_tool_loop: 1 }, { tool: 'source.present', model_tool_loop: 2 }];
    const models = [{ name: 'outer', step_id: 1 }, { name: 'outer', step_id: 3 }];
    assert.equal(readCoverage({ id: 'exact-02', lids: ['1.9.5.2'] }, trace, models, proxy.receipts).all_requested_locations_read, true);
    assert.equal(readCoverage({ id: 'exact-02', lids: ['1.9.5.2'] }, trace, models, []).all_requested_locations_read, null);
    const failed = [{ ...proxy.receipts[0], content: '{"error_code":"LID_PROVENANCE_REQUIRED"}' }];
    assert.equal(readCoverage({ id: 'exact-02', lids: ['1.9.5.2'] }, trace, models, failed).all_requested_locations_read, false);
    const otherNode = corpus.base.lid_nodes.find(node => node.lid === '1.4.2');
    const batchArgs = '{"ranges":[{"lid":"1.9.5.2"},{"lid":"1.4.2"}]}';
    const batchTrace = [{ tool: 'book.text_batch', args: batchArgs, model_tool_loop: 1 }, trace[1]];
    const batch = [{ case_id: 'fixture', tool: 'book.text_batch', arguments: batchArgs,
      content: JSON.stringify({ version: 'tool_result_envelope.v1', status: 'ok', model_body: { items: [
        { result: { lid: '1.9.5.2', text } }, { result: { lid: '1.4.2', text: corpus.source.slice(otherNode.span.start, otherNode.span.end) } } ] } }) }];
    assert.equal(readCoverage({ id: 'fixture', lids: ['1.9.5.2', '1.4.2'] }, batchTrace, models, batch).requested_reads_complete_round, 1);
    console.log(JSON.stringify({ offline_checks: 'request bytes/parameters, streamed response bytes, minimal receipt archive, actual nonempty canonical body coverage, missing/rejected receipt exclusion', passed: true, provider_calls: 0 }));
  } finally { await proxy.stop(); await new Promise(resolve => stub.close(resolve)); }
  process.exit(0);
}

function derive(directory) {
  const run = read(path.join(directory, 'run.json'));
  const receiptPath = path.join(directory, 'read-receipts.json');
  const receipts = fs.existsSync(receiptPath) ? read(receiptPath) : [];
  return { variant: run.variant, status: run.status, started_at: run.started_at, finished_at: run.finished_at,
    binary: run.binary, model: run.model, cases: run.cases.map(row => {
      const outcomePath = path.join(directory, `${row.id}.outcome.json`);
      const historyPath = path.join(directory, `${row.id}.history.json`);
      if (!fs.existsSync(outcomePath) || !fs.existsSync(historyPath)) {
        return { id: row.id, status: row.status, error: row.error ?? 'Saved outcome/history unavailable', statistics: row.statistics ?? null };
      }
      const outcome = read(outcomePath);
      const history = read(historyPath);
      const activities = history.current.turns.at(-1).run_summary?.activities ?? [];
      const models = activities.filter(activity => activity.kind === 'model');
      const outerModels = models.filter(activity => activity.name === 'outer');
      const sourceActivities = activities.filter(activity => activity.kind === 'tool' && activity.name === 'source.present');
      const trace = outcome.trace ?? [];
      const sourceTrace = trace.filter(call => call.tool === 'source.present');
      const activityMappingComplete = sourceActivities.length === sourceTrace.length;
      const groups = new Map();
      for (const [index, call] of sourceTrace.entries()) {
        let input;
        try { input = JSON.parse(call.args); } catch { input = null; }
        const activity = activityMappingComplete ? sourceActivities[index] : null;
        if (!groups.has(call.model_tool_loop)) groups.set(call.model_tool_loop, []);
        groups.get(call.model_tool_loop).push({ source_call: index + 1, quote: input?.quote ?? null,
          arguments: call.args, status: activity?.status ?? null, error_code: activity?.error_code ?? null,
          step_id: activity?.step_id ?? null, result_digest: call.result_digest });
      }
      const sourceRounds = [...groups.keys()].sort((a, b) => a - b);
      const firstRound = sourceRounds[0] ?? null;
      const firstModel = firstRound === null ? null : outerModels[firstRound - 1];
      const citationModels = firstModel ? models.filter(model => model.step_id >= firstModel.step_id) : [];
      const precedingModels = firstModel ? models.filter(model => model.step_id < firstModel.step_id) : [];
      const toolRounds = [...new Set(trace.map(call => call.model_tool_loop))].sort((a, b) => a - b);
      return { id: row.id, status: row.status, message: row.message, answer: row.answer,
        elapsed_ms: row.elapsed_ms, statistics: row.statistics,
        model_activities_by_purpose: Object.fromEntries([...new Set(models.map(model => model.name))]
          .map(name => [name, models.filter(model => model.name === name).length])),
        read_coverage: readCoverage(row, trace, models, receipts),
        outer_tool_rounds: toolRounds.map(loop => ({ model_tool_loop: loop,
          tools: trace.filter(call => call.model_tool_loop === loop).map(call => call.tool) })),
        outer_rounds_without_tools: Array.from({ length: outerModels.length }, (_, index) => index + 1)
          .filter(loop => !toolRounds.includes(loop)),
        non_outer_model_activities: models.filter(model => model.name !== 'outer')
          .map(({ step_id, name, status, usage }) => ({ step_id, name, status, usage })),
        before_citation: {
          model_activity_count: firstModel ? precedingModels.length : null,
          usage: sumUsage(precedingModels),
          model_duration_ms: sumKnown(precedingModels, model => model.duration_ms),
          activities: precedingModels.map(({ step_id, name, status, usage }) => ({ step_id, name, status, usage })),
        },
        citation: {
          source_call_count: sourceTrace.length,
          source_submission_round_count: sourceRounds.length,
          source_submission_rounds: sourceRounds,
          first_source_round: firstRound,
          citation_phase_model_activity_count: firstModel ? citationModels.length : null,
          citation_phase_usage: sumUsage(citationModels),
          citation_phase_model_duration_ms: sumKnown(citationModels, model => model.duration_ms),
          calls_after_first_source_round: sourceTrace.filter(call => call.model_tool_loop > firstRound).length,
          rejected_source_count: activityMappingComplete ? sourceActivities.filter(activity => activity.status === 'rejected').length : null,
          source_activity_mapping_complete: activityMappingComplete,
          retrieval_tools_at_or_after_first_source_round: firstRound === null ? [] : trace
            .filter(call => call.model_tool_loop >= firstRound && retrievalTools.has(call.tool))
            .map(({ model_tool_loop, tool, args }) => ({ model_tool_loop, tool, args })),
          non_source_tools_after_first_source_round: firstRound === null ? [] : trace
            .filter(call => call.model_tool_loop > firstRound && call.tool !== 'source.present')
            .map(({ model_tool_loop, tool, args }) => ({ model_tool_loop, tool, args })),
          groups: sourceRounds.map(loop => ({ model_tool_loop: loop, calls: groups.get(loop) })),
        } };
    }) };
}

// Pure reconstruction: inspect previous artifacts without starting a Reader.
if (args.includes('--derive')) {
  const directory = option('--records');
  if (!directory) throw new Error('Use --derive --records DIRECTORY');
  console.log(JSON.stringify(derive(path.resolve(directory)), null, 2));
  process.exit(0);
}

// Rebuild a comparison exclusively from the twelve saved question outcomes.
if (args.includes('--summarize')) {
  const protocol = read(path.join(output, 'protocol.json'));
  if (!protocol.phases.every(phase => phase.status === 'completed')) throw new Error('All four ABBA phases must finish before summarizing');
  const runs = phases.map(phase => {
    const directory = path.join(output, phase);
    const metrics = derive(directory);
    write(path.join(directory, 'citation-metrics.json'), metrics);
    return { phase, ...metrics };
  });
  const manualPath = path.join(output, 'manual-review.json');
  const manual = fs.existsSync(manualPath) ? read(manualPath) : null;
  const samples = runs.flatMap(run => run.cases.map(row => ({ phase: run.phase, variant: run.variant,
    id: row.id, category: row.id.startsWith('cross') ? 'complex' : 'single_passage',
    status: row.status, message: row.message, usage: row.statistics.usage,
    citation_delivery_ok: row.statistics.source_count > 0 && row.statistics.valid_source_count === row.statistics.source_count,
    manual_review: manual?.phases.find(phase => phase.phase === run.phase)?.cases.find(item => item.id === row.id) ?? null,
    core_and_citation_checks_pass: manual?.phases.find(phase => phase.phase === run.phase)?.cases
      .find(item => item.id === row.id)?.core_and_citation_checks_pass ?? null,
    before_citation: row.before_citation, citation: row.citation, read_coverage: row.read_coverage,
    metrics: {
      ...row.statistics.usage,
      first_input_tokens: row.statistics.model_calls[0]?.usage?.input_tokens ?? null,
      first_cached_input_tokens: row.statistics.model_calls[0]?.usage?.cached_input_tokens ?? null,
      model_activities: row.statistics.model_activity_count, outcome_turns: row.statistics.outcome_turns,
      outer_model_activities: row.model_activities_by_purpose.outer ?? 0,
      before_citation_models: row.before_citation.model_activity_count,
      before_citation_total_tokens: row.before_citation.usage.total_tokens,
      before_citation_input_tokens: row.before_citation.usage.input_tokens,
      before_citation_output_tokens: row.before_citation.usage.output_tokens,
      before_citation_cached_input_tokens: row.before_citation.usage.cached_input_tokens,
      requested_reads_complete_round: row.read_coverage.requested_reads_complete_round,
      initial_round_extra_location_calls: row.read_coverage.initial_round_extra_location_calls.length,
      after_requested_reads_retrieval_calls: row.read_coverage.after_requested_reads_retrieval_calls?.length ?? null,
      after_requested_reads_before_citation_models: row.read_coverage.after_requested_reads_before_citation_models,
      after_requested_reads_before_citation_total_tokens: row.read_coverage.after_requested_reads_before_citation_usage?.total_tokens ?? null,
      after_requested_reads_before_citation_input_tokens: row.read_coverage.after_requested_reads_before_citation_usage?.input_tokens ?? null,
      after_requested_reads_before_citation_output_tokens: row.read_coverage.after_requested_reads_before_citation_usage?.output_tokens ?? null,
      after_requested_reads_before_citation_cached_input_tokens: row.read_coverage.after_requested_reads_before_citation_usage?.cached_input_tokens ?? null,
      citation_phase_models: row.citation.citation_phase_model_activity_count,
      citation_phase_total_tokens: row.citation.citation_phase_usage.total_tokens,
      citation_phase_input_tokens: row.citation.citation_phase_usage.input_tokens,
      citation_phase_output_tokens: row.citation.citation_phase_usage.output_tokens,
      citation_phase_cached_input_tokens: row.citation.citation_phase_usage.cached_input_tokens,
      source_submission_rounds: row.citation.source_submission_round_count,
      retrieval_calls_at_or_after_first_source_round: row.citation.retrieval_tools_at_or_after_first_source_round.length,
      source_calls: row.citation.source_call_count, rejected_sources: row.citation.rejected_source_count,
      source_count: row.statistics.source_count, valid_source_count: row.statistics.valid_source_count,
      elapsed_ms: row.elapsed_ms,
    },
    model_activities_by_purpose: row.model_activities_by_purpose,
    outer_tool_rounds: row.outer_tool_rounds, outer_rounds_without_tools: row.outer_rounds_without_tools,
    non_outer_model_activities: row.non_outer_model_activities,
  })));
  const summarizeRows = rows => ({ samples: rows.length,
    measurements: Object.fromEntries(Object.keys(rows[0]?.metrics ?? {}).map(key => {
      const values = rows.map(row => row.metrics[key]);
      const complete = values.length > 0 && values.every(Number.isFinite);
      return [key, { values, observed: values.filter(Number.isFinite).length,
        mean: complete ? values.reduce((sum, value) => sum + value, 0) / values.length : null,
        min: complete ? Math.min(...values) : null, max: complete ? Math.max(...values) : null }];
    })),
    usage_totals: Object.fromEntries(usageKeys.map(key => [key, sumKnown(rows, row => row.usage[key])])),
    before_citation_usage_totals: Object.fromEntries(usageKeys.map(key => [key, sumKnown(rows, row => row.before_citation.usage[key])])),
    citation_phase_usage_totals: Object.fromEntries(usageKeys.map(key => [key, sumKnown(rows, row => row.citation.citation_phase_usage[key])])),
    count_totals: Object.fromEntries(['model_activities', 'outer_model_activities', 'before_citation_models',
      'citation_phase_models', 'source_submission_rounds', 'source_calls', 'rejected_sources', 'source_count', 'valid_source_count']
      .map(key => [key, sumKnown(rows, row => row.metrics[key])])),
    citation_delivery_successes: rows.filter(row => row.citation_delivery_ok).length,
    citation_delivery_failures: rows.filter(row => !row.citation_delivery_ok).map(row => ({ phase: row.phase, id: row.id })),
  });
  const comparison = { version: 'evidence-stop-comparison.v1', order: phases, product_questions: samples.length,
    rebuilt_at: new Date().toISOString(), samples,
    by_variant: ['baseline', 'candidate'].map(variant => ({ variant,
      ...summarizeRows(samples.filter(row => row.variant === variant)) })),
    by_question: questionIds.map(id => {
      const rows = samples.filter(row => row.id === id);
      const baseline = summarizeRows(rows.filter(row => row.variant === 'baseline'));
      const candidate = summarizeRows(rows.filter(row => row.variant === 'candidate'));
      const a = baseline.measurements.total_tokens.mean, b = candidate.measurements.total_tokens.mean;
      const checksPass = rows.every(row => row.core_and_citation_checks_pass === true);
      const change = Number.isFinite(a) && Number.isFinite(b) && a > 0 ? (b - a) / a * 100 : null;
      return { id, category: rows[0].category, identical_user_messages: new Set(rows.map(row => row.message)).size === 1,
        baseline, candidate, all_samples_core_and_citation_checks_pass: checksPass,
        observed_mean_total_token_change_percent: change,
        core_and_citation_checked_change_percent: checksPass ? change : null };
    }),
    by_category: ['complex', 'single_passage'].map(category => ({ category,
      baseline: summarizeRows(samples.filter(row => row.category === category && row.variant === 'baseline')),
      candidate: summarizeRows(samples.filter(row => row.category === category && row.variant === 'candidate')) })),
    limitations: [
      'Two samples per question and variant; no significance or population-level saving is inferred.',
      'Autonomous retrieval, nested query calls, answer scope, and cache state may differ. Total usage changes alone do not establish prompt causality.',
      'Reasoning tokens are included in output; cache tokens are included in input. Missing provider fields remain null.',
      'Zero delivered citations is a citation failure for these tasks. Actual usage is retained; core_and_citation_checks_pass does not claim overall answer equivalence.',
      'Given-location reading completion requires actual returned canonical source text; absent receipt evidence stays unknown and is not inferred from arguments.',
      'A1 source receipts were not captured because its observer matched internal names instead of native provider names. Name matching was corrected from candidate-r1 request 1. A1 reading-completion and post-completion measures remain unknown, never zero.',
    ],
  };
  write(path.join(output, 'comparison.json'), comparison);
  console.log(JSON.stringify({ product_questions: samples.length, questions: comparison.by_question.map(question => ({
    id: question.id, identical_user_messages: question.identical_user_messages,
    baseline_tokens: question.baseline.measurements.total_tokens,
    candidate_tokens: question.candidate.measurements.total_tokens,
    baseline_source_rounds: question.baseline.measurements.source_submission_rounds.values,
    candidate_source_rounds: question.candidate.measurements.source_submission_rounds.values,
  })) }, null, 2));
  process.exit(0);
}

const phase = option('--phase');
const phaseIndex = phases.indexOf(phase);
if (phaseIndex < 0) throw new Error(`Choose --phase ${phases.join('|')}`);
const variant = phase.startsWith('baseline') ? 'baseline' : 'candidate';
const binary = path.resolve(option('--binary', path.join(logDirectory, `${variant}-server.exe`)));
if (!fs.existsSync(binary)) throw new Error('Frozen Reader binary not found');
const phaseOutput = path.join(output, phase);
const protocolPath = path.join(output, 'protocol.json');
fs.mkdirSync(output, { recursive: true });
const protocol = fs.existsSync(protocolPath) ? read(protocolPath) : {
  version: 'evidence-stop-trial.v1', created_at: new Date().toISOString(),
  intent: 'Compare the citation-delivery-fixed runtime with and without scoped guidance to finish once requested evidence is sufficient.',
  order: 'ABBA blocks; each block runs cross-01, cross-02, exact-02 in that order.',
  maximum_product_questions: 12, grading_provider_calls: 0, retries: 0,
  isolation: 'Each product question starts a new local Reader with isolated memory/private directories; external telemetry is off.',
  inputs: 'The old runner supplies identical questions and explicit LID positions. No source body or gold labels are put in user messages.',
  measurement: {
    usage: 'Final provider-reported activity fields; missing values stay null. Reasoning is included in output, cached input in input.',
    source_submission_round_count: 'Distinct outcome.trace model_tool_loop values containing source.present.',
    citation_phase: 'All model activities from the first outer source.present submission through final delivery; later non-source tools are separately listed.',
    validation: 'Resolve every final citation to canonical source text, then manually check core facts without a paid judge.',
    returned_source_observation: 'Both variants use the same local streaming pass-through. Only necessary original source-reading tool receipt fields/body are retained; no headers, credentials, environment, reasoning, or other request messages are stored.',
    reading_completion: 'All requested LIDs must appear in actual non-error returned text covering their full canonical source. Arguments or a successful tool activity alone do not establish this.',
    observation_limit: 'A1 ran before matching native provider book_text/book_text_batch names; its missing receipts remain unknown. Matching is corrected from candidate-r1 request 1, without rerunning any question.',
    limitations: 'Two samples per case/variant; provider cache and autonomous extra reading can differ. No guaranteed saving is assumed.',
  },
  phases: phases.map((name, index) => ({ name, order: index + 1, status: 'pending', question_ids: questionIds })),
};
if (protocol.phases[phaseIndex].status !== 'pending') throw new Error('This phase was already started; do not retry or overwrite it');
if (phaseIndex && protocol.phases[phaseIndex - 1].status !== 'completed') throw new Error('The previous ABBA phase must finish first');
if (fs.existsSync(path.join(phaseOutput, 'run.json'))) throw new Error('Phase output already exists');
Object.assign(protocol.phases[phaseIndex], { status: 'running', started_at: new Date().toISOString(), binary });
write(protocolPath, protocol);

let exitCode = null;
let observer;
let phaseLog;
try {
  if (fs.existsSync(path.join(root, '.env'))) process.loadEnvFile(path.join(root, '.env'));
  const providerBase = process.env.OPENCODE_BASE_URL;
  if (!providerBase || !process.env.OPENCODE_API_KEY || !process.env.FLUID_LLM_MODEL) throw new Error('Existing provider configuration is missing');
  fs.mkdirSync(phaseOutput, { recursive: true });
  fs.mkdirSync(logDirectory, { recursive: true });
  let caseId = questionIds[0];
  observer = await captureReads(providerBase, () => {
    const runFile = path.join(phaseOutput, 'run.json');
    if (fs.existsSync(runFile)) caseId = parse(fs.readFileSync(runFile, 'utf8'))?.cases?.at(-1)?.id ?? caseId;
    return caseId;
  }, receipts => write(path.join(phaseOutput, 'read-receipts.json'), receipts));
  phaseLog = fs.createWriteStream(path.join(logDirectory, `${phase}.log`), { flags: 'wx' });
  exitCode = await new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [path.join(root, 'evals/semantic/book-text-batch-trial.mjs'),
      '--variant', variant, '--binary', binary, '--out', phaseOutput],
    { cwd: root, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env: { ...process.env, OPENCODE_BASE_URL: observer.url } });
    child.stdout.on('data', chunk => { process.stdout.write(chunk); phaseLog.write(chunk); });
    child.stderr.on('data', chunk => { process.stderr.write(chunk); phaseLog.write(chunk); });
    child.once('error', reject);
    child.once('exit', code => resolve(code));
  });
  if (fs.existsSync(path.join(phaseOutput, 'run.json'))) {
    const metrics = derive(phaseOutput);
    write(path.join(phaseOutput, 'citation-metrics.json'), metrics);
    protocol.phases[phaseIndex].status = exitCode === 0 && metrics.status === 'completed' ? 'completed' : 'failed';
    protocol.phases[phaseIndex].question_count = metrics.cases.length;
    console.log(JSON.stringify({ phase, status: protocol.phases[phaseIndex].status,
      cases: metrics.cases.map(row => ({ id: row.id, total_tokens: row.statistics?.usage.total_tokens,
        model_activities: row.statistics?.model_activity_count, source_rounds: row.citation?.source_submission_round_count,
        source_calls: row.citation?.source_call_count, rejected_sources: row.citation?.rejected_source_count,
        requested_reads_complete_round: row.read_coverage?.requested_reads_complete_round,
        after_reads_retrieval_calls: row.read_coverage?.after_requested_reads_retrieval_calls?.length ?? null })) }));
  } else protocol.phases[phaseIndex].status = 'failed';
} catch (error) {
  protocol.phases[phaseIndex].status = 'failed';
  protocol.phases[phaseIndex].error = error.message;
  process.exitCode = 1;
} finally {
  if (observer) { await observer.stop(); write(path.join(phaseOutput, 'read-receipts.json'), observer.receipts); }
  if (phaseLog) await new Promise(resolve => phaseLog.end(resolve));
  protocol.phases[phaseIndex].finished_at = new Date().toISOString();
  protocol.phases[phaseIndex].runner_exit_code = exitCode;
  write(protocolPath, protocol);
  if (exitCode !== 0) process.exitCode = 1;
}
