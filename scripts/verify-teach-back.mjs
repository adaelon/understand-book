// One ordinary Reader chat on a published sample. The model endpoint
// receives the same request bytes as in the Reader. No teaching reply is scripted.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { parseArgs } from 'node:util';
import { createInterface } from 'node:readline/promises';
import { DatabaseSync } from 'node:sqlite';
import { setTimeout as delay } from 'node:timers/promises';
import { startServer } from '../evals/semantic/server.mjs';
import { measuredUsage, startProviderRecorder } from '../evals/semantic/provider-recorder.mjs';
import { selectTrialProvider } from './model-trial-config.mjs';

const { values } = parseArgs({ options: {
  out: { type: 'string' }, 'origin-only': { type: 'boolean' }, 'from-origin': { type: 'string' },
  browser: { type: 'boolean' }, sample: { type: 'string', default: 'ai-infra' },
  retry: { type: 'string' }, 'budget-from': { type: 'string' },
} });
if (!values.out) throw new Error('Specify --out <new evidence directory>.');
const out = path.resolve(values.out);
const seed = values['from-origin'] && path.resolve(values['from-origin']);
const retry = values.retry && path.resolve(values.retry);
if (retry && (!seed || values.browser || values['origin-only'])) throw new Error('Retry requires --from-origin for an API comparison.');
const samples = {
  'ai-infra': {
    source: 'ai-infra-book-complete', lid: '3.6.3.14',
    question: '这 70 GB 权重已经装得下，换卡时看什么才知道它能不能读得更快？',
    expression: '容量决定装不装得下。装得下以后，容量翻倍，读取也会快一倍。',
  },
  'wealth': {
    source: 'justin-sun-z-library-sk-1lib-sk-z-lib-sk', lid: '40.2',
    question: '自由意志怎么理解',
    expression: '我的理解是：自由意志在于只要不侵犯他人的自由，那么两个人之间的任何互动都具有合法性。这个的前提是双方自愿非强迫。从而可以引申出来任何双方自愿的交易都具有合法性，应该被保护',
  },
};
const sample = samples[values.sample];
assert.ok(sample, 'Choose --sample ai-infra or wealth.');
const { question, expression, lid } = sample;
const book = path.resolve('.understand-book', sample.source);
const write = (name, value) => fs.writeFileSync(path.join(out, name), JSON.stringify(value, null, 2));
if (fs.existsSync('.env')) process.loadEnvFile('.env');
const selection = selectTrialProvider(process.env);
if (selection.direct) throw new Error('This input-isolation trial requires the recording proxy and legacy provider configuration.');
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.mkdirSync(out);
const state = path.join(out, 'reader');
if (seed) fs.cpSync(path.join(retry ?? seed, 'reader'), state, { recursive: true });
// A structurally valid but semantically wrong card is a completed turn. Replay
// the original expression from its seed and carry the reviewed attempt's budget.
const budgetFrom = retry ?? values['budget-from'] ?? seed;
const previousReport = budgetFrom && JSON.parse(fs.readFileSync(path.join(budgetFrom, 'run.json'), 'utf8'));
const previous = budgetFrom ? JSON.parse(fs.readFileSync(path.join(budgetFrom, 'provider.json'), 'utf8')) : [];
const used = previousReport?.cumulative_budget ?? {
  requests: previous.filter(r => !r.blocked).length,
  reported_tokens: previous.reduce((sum, r) => sum + (r.usage?.total_tokens ?? 0), 0),
};
const recorder = await startProviderRecorder(selection.base, {
  tokenLimit: 1200000 - used.reported_tokens,
  beforeSend: (_request, row) => used.requests + row.ordinal > 24 ? 'TRIAL_REQUEST_LIMIT' : null,
});
let server;
const binary = path.resolve(process.env.SEMANTIC_EVAL_SERVER ?? path.join('target', 'debug',
  process.platform === 'win32' ? 'reader.exe' : 'reader'));
const result = { started_at: new Date(), model: selection.model, book, original_sample: seed,
  reader_binary: binary, built_at: fs.statSync(binary).mtime, question, expression };
function inspectStages(records, comparison) {
  const calls = [];
  for (const { ordinal, elapsed_ms, request } of records) {
    const turns = request.messages.filter(message => !['system', 'developer'].includes(message.role));
    if (turns.length !== 1 || turns[0].role !== 'user') continue;
    const content = turns[0].content;
    const text = Array.isArray(content) ? content.map(block => block.text ?? '').join('') : content;
    let envelope;
    try { envelope = JSON.parse(text); } catch { continue; }
    const packet = envelope.input ?? envelope;
    if (packet.expression === expression && Array.isArray(packet.topic_terms)) {
      assert.deepEqual(Object.keys(packet).sort(), ['expression', 'original_question', 'topic_terms']);
      assert.equal(packet.original_question, question);
      assert.equal(request.tools?.length ?? 0, 0);
      calls.push({ stage: 'expression', ordinal, elapsed_ms, topic_terms: packet.topic_terms });
    } else if (packet.evidence && packet.frozen_student_graph) {
      assert.equal(packet.original_question, question);
      assert.deepEqual(packet.frozen_student_graph, comparison.student_graph);
      const { source_id, source_revision, passages } = packet.evidence;
      assert.deepEqual({ source_id, source_revision }, comparison.source);
      if (values.sample === 'wealth') {
        const anchor = passages.find(p => p.lid === lid);
        assert.equal(anchor?.text.length, 4447);
        assert.ok(anchor.text.includes('交易受保护的原则'));
      }
      calls.push({ stage: 'comparison', ordinal, elapsed_ms,
        read_ranges: passages.map(({ lid, source_range_utf16 }) => ({ lid, ...source_range_utf16 })) });
    }
  }
  assert.equal(calls[0]?.stage, 'expression', 'Missing blind first-stage provider input');
  assert.equal(calls.at(-1)?.stage, 'comparison', 'Missing source verification input');
  return calls;
}
function inspectStore(card, origin, displayed) {
  const database = new DatabaseSync(path.join(state, 'memory', 'learning.db'), { readOnly: true });
  try {
    const query = database.prepare('SELECT seq, event FROM teaching_trace WHERE event_id = ?');
    const delivery = query.get(card.round_id);
    const event = JSON.parse(delivery.event);
    assert.deepEqual(event.payload.comparison, card);
    assert.equal(event.payload.origin_turn_id, origin);
    const learner = JSON.parse(query.get(event.causal_refs[0]).event);
    assert.equal(learner.payload.response, expression);
    assert.ok(learner.payload.support_delivery_refs.some(id => id.endsWith(':assistant:' + origin)));
    assert.ok(!learner.payload.support_delivery_refs.includes(card.round_id));
    const receipt = query.get('display:' + card.round_id);
    assert.equal(!!receipt, displayed);
    if (displayed) assert.ok(receipt.seq > delivery.seq);
    return { delivery_ref: card.round_id, delivery_seq: delivery.seq, display_seq: receipt?.seq ?? null,
      expression_ref: learner.event_id, support_delivery_refs: learner.payload.support_delivery_refs };
  } finally { database.close(); }
}
async function submit(body, name) {
  const accepted = await server.api('agent/runs', body, 'POST');
  const id = accepted.turn_id ?? accepted.descriptor?.turn_id;
  assert.ok(id, JSON.stringify(accepted));
  return waitForTurn(id, name);
}
async function waitForTurn(id, name) {
  const start = Date.now();
  while (Date.now() - start < 180000) {
    const run = await server.api('agent/runs/' + id);
    if (['saved', 'failed', 'unsaved'].includes(run.persistence_state)) {
      write(name + '.json', run);
      result[name + '_activities'] = run.activities.filter(a => a.kind === 'teach_back')
        .map(({ name, duration_ms, status }) => ({ name, duration_ms, status }));
      assert.equal(run.execution_state, 'completed', JSON.stringify(run.final_view?.error ?? run.error));
      assert.equal(run.persistence_state, 'saved');
      if (!values.browser || name === 'origin') result[name + '_ms'] = Date.now() - start;
      return id;
    }
    await delay(300);
  }
  throw new Error(name + ' exceeded 180 seconds');
}
try {
  server = await startServer(book, path.join(state, 'memory'), process.cwd(), {
    UNDERSTAND_BOOK_PRIVATE_DIR: path.join(state, 'private'),
    OPENCODE_BASE_URL: recorder.url, FLUID_LLM_MODEL: selection.model,
    UB_OBSERVABILITY_MODE: 'off', LANGSMITH_TRACING: 'false',
  });
  const source = await server.api('book/text', { lid });
  assert.ok(source.text.includes(values.sample === 'wealth' ? '交易受保护的原则' : '10.45 ms'));
  const quote = source.text;
  write('anchor.json', { lid, text: quote });
  const origin = seed ? JSON.parse(fs.readFileSync(path.join(seed, 'origin.json'), 'utf8')).descriptor.turn_id :
    await submit({ message: question, question_anchor_lid: lid, response_language: 'zh-CN',
      question_quote: { lid, quote, raw_quote: quote, resolved_quote: quote, status: 'resolved',
        resolution_basis: 'exact', ranges: [{ lid, range: { start: 0, end: quote.length } }] } }, 'origin');
  const { current } = await server.api('agent/history');
  const head = current.turns.at(-1);
  if (retry) {
    assert.equal(head.status, 'failed');
    assert.equal(head.user, expression);
  } else assert.equal(head.turn_id, origin);
  result.origin_turn_id = origin;
  result.session_id = current.id;
  if (!values['origin-only']) {
    const { source_id, source_revision } = await server.api('tutor/readiness');
    const id = retry ? head.domain.teach_back_attempts[0].expression_id : crypto.randomUUID();
    let response;
    if (values.browser) {
      // A browser submission exercises selection, input, progress, rendering and
      // the display receipt on the same material and model sample.
      console.log('Reader ready:', server.url, '\nSubmit:', expression);
      const input = createInterface({ input: process.stdin, output: process.stdout });
      try { await input.question('After checking the displayed comparison, press Enter to save the trial.\n'); }
      finally { input.close(); }
      const turns = (await server.api('agent/history')).current.turns;
      const turn = turns.find(t => t.user === expression);
      assert.ok(turn, 'The browser did not submit the sample expression.');
      response = await waitForTurn(turn.turn_id, 'comparison');
    } else response = await submit({ message: expression, display_user: expression, entry_action: 'explain_back',
      session_id: current.id, expected_chat_head: head.turn_id, ...(retry ? { retry_of: head.turn_id } : {}),
      answer_ref: { turn_id: origin },
      source_id, source_revision, client_request_id: id, expression_id: id,
      operation_id: crypto.randomUUID(), response_language: 'zh-CN' }, 'comparison');
    const history = (await server.api('agent/history')).current;
    const comparison = history.turns.at(-1).outcome.answer_view.parts.find(p => p.kind === 'teach_back_comparison')?.comparison;
    assert.ok(comparison, 'Reader did not deliver a comparison');
    assert.equal(history.turns.filter(t => t.user === expression).length, 1);
    result.comparison_turn_id = response;
    write('card.json', comparison);
    result.stage_calls = inspectStages(recorder.records, comparison);
    result.receipts = inspectStore(comparison, origin, !!values.browser);
  }
  result.status = values['origin-only'] ? 'original_answer_saved' : 'delivered_pending_semantic_review';
} catch (error) {
  result.status = recorder.budgetRejections.length ? 'stopped_at_trial_budget' : 'failed';
  result.error = error.message;
  process.exitCode = 1;
} finally {
  await server?.stop();
  await recorder.stop(30000);
  write('provider.json', recorder.records);
  result.usage = measuredUsage(recorder.records);
  result.budget_stops = recorder.budgetRejections;
  result.cumulative_budget = {
    requests: used.requests + recorder.records.filter(r => !r.blocked).length,
    reported_tokens: used.reported_tokens + recorder.records.reduce((sum, r) => sum + (r.usage?.total_tokens ?? 0), 0),
  };
  if (retry) result.retried_from = retry;
  result.finished_at = new Date();
  write('run.json', result);
  console.log(result);
}
