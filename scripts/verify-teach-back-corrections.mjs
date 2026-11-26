/* Exercise each learner action against an isolated copy of the real Reader history.
 * --from-origin and --from-displayed are evidence directories produced by
 * verify-teach-back.mjs. The displayed sample must include its browser receipt.
 */
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { setTimeout as delay } from 'node:timers/promises';
import { parseArgs } from 'node:util';
import { startServer } from '../evals/semantic/server.mjs';
import { measuredUsage, startProviderRecorder } from '../evals/semantic/provider-recorder.mjs';
import { selectTrialProvider } from './model-trial-config.mjs';

const samples = [
  { name: 'revise', kind: 'revise_understanding', status: 'divergent',
    text: '我把容量和带宽弄混了。能装下同样的 70 GB 之后，只加大容量不会影响读取下界；提高 HBM 带宽才会缩短它。' },
  { name: 'dispute', kind: 'dispute_interpretation', status: 'consistent',
    text: '第一句“容量决定装不装得下”只在判断能否容纳，第二句才是我对速度的判断。请分开看，不要说我第一句已经区分了容量和读取速度。' },
  { name: 'new-reason', kind: 'dispute_interpretation', status: 'divergent',
    text: '我其实已经说过要用读取量除以 HBM 带宽，带宽翻倍下界才会减半。' },
  { name: 'analogy',
    text: '仓库要先够大才装得下这批货。货量固定又卡在搬运上时，再扩仓也不会搬快；换卡应该看每秒能搬多少，也就是 HBM 带宽，带宽翻倍，搬完的最短时间才减半。' },
  { name: 'conclusion', text: '换卡我会看 HBM 带宽。' },
  { name: 'insufficient',
    text: '我会按生产环境里连续跑半小时的实测 HBM 带宽选卡。我认为我机房的甲卡能稳定跑到每秒四 TB，因此搬完同样的权重会比乙卡快。' },
  { name: 'follow-up', kind: 'follow_up',
    text: '我会看 HBM 带宽。若带宽和权重都不变，单独增加容量不会把读取时间下界缩短。' },
];
const { values } = parseArgs({ options: {
  out: { type: 'string' }, 'from-origin': { type: 'string' }, 'from-displayed': { type: 'string' },
  case: { type: 'string', multiple: true }, 'retry-failed': { type: 'string' },
} });
if (!values.out || !values['from-origin'] || !values['from-displayed']) {
  throw new Error('Specify --out <new directory> --from-origin <saved original> --from-displayed <saved displayed comparison>.');
}
const selected = values.case ? samples.filter(sample => values.case.includes(sample.name)) : samples;
assert.equal(selected.length, values.case?.length ?? samples.length, 'Unknown or duplicate case');
if (values['retry-failed']) assert.equal(selected.length, 1, 'Select the single case to retry');
const out = path.resolve(values.out);
const load = (dir, file) => JSON.parse(fs.readFileSync(path.join(dir, file), 'utf8'));
const save = (dir, file, value) => fs.writeFileSync(path.join(dir, file), JSON.stringify(value, null, 2));
const originDir = path.resolve(values['from-origin']);
const displayedDir = path.resolve(values['from-displayed']);
const origin = load(originDir, 'run.json');
const previous = load(displayedDir, 'run.json');
const firstCard = load(displayedDir, 'card.json');
assert.equal(previous.origin_turn_id, origin.origin_turn_id);
assert.ok(previous.receipts.display_seq > previous.receipts.delivery_seq);
if (fs.existsSync('.env')) process.loadEnvFile('.env');
const provider = selectTrialProvider(process.env);
if (provider.direct) throw new Error('The input-isolation trial requires the recording proxy and legacy provider configuration.');
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.mkdirSync(out);
const recorder = await startProviderRecorder(provider.base, { tokenLimit: 1200000,
  beforeSend: (_request, row) => row.ordinal > 24 ? 'TRIAL_REQUEST_LIMIT' : null });
const binary = path.resolve(process.env.SEMANTIC_EVAL_SERVER ?? path.join('target', 'debug',
  process.platform === 'win32' ? 'reader.exe' : 'reader'));
const report = { reader: binary, built_at: fs.statSync(binary).mtime, model: provider.model,
  origin: originDir, displayed: displayedDir, started_at: new Date(), cases: [] };
const readCard = turn => turn.outcome?.answer_view?.parts.find(part => part.kind === 'teach_back_comparison')?.comparison;
async function savedRun(server, id) {
  const deadline = Date.now() + 180000;
  while (Date.now() < deadline) {
    const run = await server.api('agent/runs/' + id);
    if (['saved', 'failed', 'unsaved'].includes(run.persistence_state)) return run;
    await delay(300);
  }
  throw new Error('Learning turn exceeded 180 seconds');
}

function inspectSamples(records, sample, card) {
  const stages = [];
  for (const { request, ordinal, elapsed_ms } of records) {
    const messages = request.messages?.filter(m => !['system', 'developer'].includes(m.role));
    if (messages?.length !== 1 || messages[0].role !== 'user') continue;
    const content = messages[0].content;
    const text = Array.isArray(content) ? content.map(b => b.text ?? '').join('') : content;
    let envelope;
    try { envelope = JSON.parse(text); } catch { continue; }
    const packet = envelope.input ?? envelope;
    if (packet.original_question !== origin.question) continue;
    if (Array.isArray(packet.topic_terms)) {
      assert.equal(request.tools?.length ?? 0, 0);
      assert.equal(packet.expression, sample.kind === 'dispute_interpretation' ? previous.expression : sample.text);
      assert.equal(packet.evidence, undefined);
      assert.equal(packet.selected_prior_row, undefined);
      stages.push({ stage: 'expression', ordinal, elapsed_ms, fields: Object.keys(packet) });
    } else if (packet.evidence?.passages && packet.frozen_student_graph) {
      if (card) assert.deepEqual(packet.frozen_student_graph, card.student_graph);
      stages.push({ stage: 'comparison', ordinal, elapsed_ms, lids: packet.evidence.passages.map(p => p.lid) });
    }
  }
  assert.equal(stages[0]?.stage, 'expression', 'Missing blind first-stage input');
  if (card) assert.equal(stages.at(-1)?.stage, 'comparison', 'Missing verification input');
  return stages;
}

function inspectLearning(state, turn, card, sample) {
  const db = new DatabaseSync(path.join(state, 'memory', 'learning.db'), { readOnly: true });
  try {
    const evidence = id => JSON.parse(db.prepare('SELECT evidence FROM learning_evidence WHERE evidence_id=?').get(id).evidence);
    const event = id => {
      const row = db.prepare('SELECT event FROM teaching_trace WHERE event_id=?').get(id);
      return row && JSON.parse(row.event);
    };
    const submitted = event('teach-back-expression:' + turn.turn_id);
    if (!card) {
      assert.equal(sample.name, 'new-reason', 'Expected a delivered comparison');
      assert.equal(submitted, undefined);
      const round = 'tb:' + turn.turn_id + ':r1';
      assert.equal(event(round), undefined);
      assert.equal(db.prepare('SELECT 1 FROM learning_evidence WHERE evidence_id=?')
        .get('interpretation:' + round), undefined);
      assert.match(turn.outcome.answer ?? '', /改一下我的想法/);
      return;
    }
    assert.notEqual(sample.name, 'new-reason', 'New reasoning was credited to an earlier expression');
    assert.deepEqual(event(card.round_id).payload.comparison, card);
    const result = evidence('interpretation:' + card.round_id);
    const support = result.support_delivery_refs;
    let quoteText = sample.text;
    let quoteTurn = turn.turn_id;
    if (sample.kind) {
      assert.equal(card.continuation?.kind, sample.kind);
      assert.equal(card.continuation?.parent_round_id, firstCard.round_id);
      assert.deepEqual(event(firstCard.round_id).payload.comparison, firstCard);
    }
    if (sample.kind === 'dispute_interpretation') {
      assert.equal(submitted, undefined);
      const prior = evidence('interpretation:' + firstCard.round_id);
      assert.equal(result.supersedes, prior.evidence_id);
      assert.equal(result.action_ref, prior.action_ref);
      assert.deepEqual(support, prior.support_delivery_refs);
      quoteText = previous.expression;
      quoteTurn = previous.comparison_turn_id;
    } else {
      assert.equal(submitted.payload.response, sample.text);
      assert.deepEqual(submitted.payload.support_delivery_refs, support);
      assert.equal(support.includes(firstCard.round_id), Boolean(sample.kind));
    }
    assert.equal(result.learner_quote, quoteText);
    assert.deepEqual(card.support_delivery_refs, support);
    for (const item of [...card.student_graph.nodes, ...card.student_graph.edges]) {
      for (const quote of item.quotes ?? []) {
        assert.equal(quote.turn_id, quoteTurn);
        assert.ok(quoteText.includes(quote.text));
      }
    }
    return { action_ref: result.action_ref, supersedes: result.supersedes, support_delivery_refs: support };
  } finally { db.close(); }
}

async function execute(sample, result) {
  const retry = values['retry-failed'] && path.resolve(values['retry-failed']);
  const seed = retry ?? (sample.kind ? displayedDir : originDir);
  const dir = path.join(out, sample.name);
  fs.mkdirSync(dir);
  const state = path.join(dir, 'reader');
  fs.cpSync(path.join(seed, 'reader'), state, { recursive: true });
  const server = await startServer(origin.book, path.join(state, 'memory'), process.cwd(), {
    UNDERSTAND_BOOK_PRIVATE_DIR: path.join(state, 'private'), OPENCODE_BASE_URL: recorder.url,
    FLUID_LLM_MODEL: provider.model, UB_OBSERVABILITY_MODE: 'off', LANGSMITH_TRACING: 'false',
  });
  try {
    const { current } = await server.api('agent/history');
    const head = current.turns.at(-1);
    const { source_id, source_revision } = await server.api('tutor/readiness');
    const id = crypto.randomUUID();
    let body = { entry_action: 'explain_back', message: sample.text, display_user: sample.text,
      expression_id: id, operation_id: crypto.randomUUID(), client_request_id: id, source_id, source_revision,
      session_id: current.id, expected_chat_head: head.turn_id, answer_ref: { turn_id: origin.origin_turn_id }, response_language: 'zh-CN' };
    if (retry) {
      const failedRequest = load(retry, 'request.json');
      assert.equal(head.status, 'failed');
      assert.equal(head.user, sample.text);
      assert.equal(failedRequest.message, sample.text);
      assert.equal(failedRequest.teach_back_reply?.kind, sample.kind);
      body = { ...failedRequest, retry_of: head.turn_id, expected_chat_head: head.turn_id, operation_id: crypto.randomUUID() };
      result.retry_of = head.turn_id;
    } else if (sample.kind) {
      const parent = readCard(head);
      if (firstCard.student_graph) assert.deepEqual(parent, firstCard);
      else {
        // The trace keeps the historical JSON; the Reader projects its card into graphs.
        assert.equal(parent.round_id, firstCard.round_id);
        assert.deepEqual(parent.source, firstCard.source);
        assert.deepEqual(parent.rows.map(r => [r.row_id, r.status, r.summary]),
          firstCard.rows.map(r => [r.row_id, r.status, r.summary]));
      }
      const rowId = sample.status ? firstCard.rows.find(row => row.status === sample.status)?.row_id : firstCard.follow_up?.row_id;
      assert.ok(rowId, 'The saved sample does not have the needed row');
      body.teach_back_reply = { kind: sample.kind, round_id: firstCard.round_id, row_id: rowId };
    }
    save(dir, 'request.json', body);
    const accepted = await server.api('agent/runs', body, 'POST');
    const run = await savedRun(server, accepted.turn_id ?? accepted.descriptor.turn_id);
    save(dir, 'run.json', run);
    assert.equal(run.execution_state, 'completed', JSON.stringify(run.final_view?.error ?? run.error));
    assert.equal(run.persistence_state, 'saved');
    const history = (await server.api('agent/history')).current.turns;
    assert.equal(history.length, current.turns.length + (retry ? 0 : 1));
    const turn = history.at(-1);
    if (retry) assert.equal(turn.turn_id, head.turn_id);
    save(dir, 'answer.json', turn.outcome);
    const card = readCard(turn);
    if (card) save(dir, 'card.json', card);
    result.evidence = inspectLearning(state, turn, card, sample);
    result.statuses = card?.rows.map(row => row.status);
    result.model_calls = inspectSamples(recorder.records.slice(result.first_request_ordinal - 1), sample, card);
    result.stages = run.activities.filter(action => action.kind === 'teach_back')
      .map(({ name, duration_ms }) => ({ name, duration_ms }));
  } finally { await server.stop(); }
}

try {
  for (const sample of selected) {
    const start = Date.now();
    const result = { name: sample.name, first_request_ordinal: recorder.records.length + 1 };
    report.cases.push(result);
    try {
      await execute(sample, result);
      result.status = 'execution_passed_semantic_review_pending';
    } catch (error) {
      result.status = recorder.budgetRejections.length ? 'stopped_at_trial_budget' : 'failed';
      result.error = error.message;
      process.exitCode = 1;
    }
    result.elapsed_ms = Date.now() - start;
    result.last_request_ordinal = recorder.records.length;
    save(out, 'provider.json', recorder.records);
    save(out, 'run.json', report);
    console.log(result.name, result.status, result.error ?? '', result.elapsed_ms);
    if (recorder.budgetRejections.length) break;
  }
} finally {
  await recorder.stop(30000);
  report.usage = measuredUsage(recorder.records);
  report.budget_stops = recorder.budgetRejections;
  report.finished_at = new Date();
  save(out, 'provider.json', recorder.records);
  save(out, 'run.json', report);
}
