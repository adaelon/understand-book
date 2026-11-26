// Start the actual local Reader at the first reflection of the published AI Infra
// sample. The operator submits the explanation and responds to what the model asks.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createInterface } from 'node:readline/promises';
import { parseArgs } from 'node:util';
import { startServer } from '../evals/semantic/server.mjs';
import { measuredUsage, startProviderRecorder } from '../evals/semantic/provider-recorder.mjs';
import { selectTrialProvider } from './model-trial-config.mjs';

const { values } = parseArgs({ options: { out: { type: 'string' }, from: { type: 'string' } } });
if (!values.out) throw new Error('Specify --out <new evidence directory>.');
if (fs.existsSync('.env')) process.loadEnvFile('.env');
const provider = selectTrialProvider(process.env);
if (provider.direct) throw new Error('Recording the two model inputs requires the configured recording proxy.');
const out = path.resolve(values.out);
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.mkdirSync(out);
const save = (name, value) => fs.writeFileSync(path.join(out, name), JSON.stringify(value, null, 2));
const seed = values.from && path.resolve(values.from);
const previous = seed && JSON.parse(fs.readFileSync(path.join(seed, 'session.json'), 'utf8'));
const priorReport = seed && JSON.parse(fs.readFileSync(path.join(seed, 'run.json'), 'utf8'));
const priorUsage = priorReport?.cumulative_usage ?? priorReport?.usage ?? { requests: 0, total_tokens: 0 };
if (priorUsage.total_tokens == null) throw new Error('Cannot resume until usage of the previous provider requests is known.');
const priorStages = !seed ? [] : priorReport.cumulative_stage_inputs ?? [{
  directory: seed,
  calls: stageInputs(JSON.parse(fs.readFileSync(path.join(seed, 'provider.json'), 'utf8'))),
}];
const original = path.resolve('.understand-book/ai-infra-book-complete');
const book = path.join(out, path.basename(original));
if (seed) {
  for (const name of [path.basename(book), 'memory', 'private']) {
    if (fs.existsSync(path.join(seed, name))) fs.cpSync(path.join(seed, name), path.join(out, name), { recursive: true });
  }
} else {
  fs.mkdirSync(book);
  for (const item of fs.readdirSync(original, { withFileTypes: true })) {
    if (item.isFile()) fs.copyFileSync(path.join(original, item.name), path.join(book, item.name));
  }
}
const recorder = await startProviderRecorder(provider.base, {
  tokenLimit: 1200000 - priorUsage.total_tokens,
  beforeSend: (_request, row) => priorUsage.requests + row.ordinal > 24 ? 'TRIAL_REQUEST_LIMIT' : null,
});
const binary = path.resolve(process.env.SEMANTIC_EVAL_SERVER ?? path.join('target', 'debug',
  process.platform === 'win32' ? 'reader.exe' : 'reader'));
const report = { model: provider.model, binary, built_at: fs.statSync(binary).mtime, started_at: new Date(), resumed_from: seed };
function inspectRounds(history, courseView) {
  const rounds = history.current.turns.flatMap(turn => {
    const card = turn.outcome?.answer_view?.parts.find(part => part.kind === 'teach_back_comparison')?.comparison;
    if (!card) return [];
    const expression = courseView.expressions.find(item => item.teach_back?.round_id === card.round_id);
    assert.equal(expression?.teach_back?.turn_id, turn.turn_id, 'Course feedback lost its original chat turn');
    assert.equal(expression.teach_back.session_id, history.current.id);
    assert.equal(expression.response, turn.user);
    for (const item of [...card.student_graph.nodes, ...card.student_graph.edges]) {
      for (const quote of item.quotes ?? []) {
        assert.equal(quote.turn_id, turn.turn_id);
        assert.ok(expression.response.includes(quote.text));
      }
    }
    assert.ok(courseView.reviews.find(review => review.expression.job_ref === expression.job_ref)?.displayed,
      'No course display receipt for ' + card.round_id);
    return [card];
  });
  assert.ok(rounds.length >= 2, 'Submit the first explanation and answer its follow-up before ending the trial');
  assert.equal(rounds[1].continuation?.parent_round_id, rounds[0].round_id);
  assert.ok(rounds[1].support_delivery_refs.includes(rounds[0].round_id));
  return rounds;
}
function stageInputs(records) {
  return records.filter(record => !record.blocked).flatMap(({ request, ordinal, elapsed_ms }) => {
    const messages = request?.messages?.filter(message => !['system', 'developer'].includes(message.role));
    if (messages?.length !== 1 || messages[0].role !== 'user') return [];
    const content = messages[0].content;
    const text = Array.isArray(content) ? content.map(block => block.text).join('') : content;
    let envelope;
    try { envelope = JSON.parse(text); } catch { return []; }
    const packet = envelope.input ?? envelope;
    if (Array.isArray(packet.topic_terms) && typeof packet.expression === 'string') {
      assert.deepEqual(Object.keys(packet).sort(), ['expression', 'original_question', 'topic_terms']);
      assert.equal(request.tools?.length ?? 0, 0);
      return [{ ordinal, stage: 'organize_expression', elapsed_ms, topic_terms: packet.topic_terms }];
    }
    if (packet.evidence?.passages && packet.frozen_student_graph) {
      return [{ ordinal, stage: 'compare_passages', elapsed_ms, read_lids: packet.evidence.passages.map(p => p.lid) }];
    }
    return [];
  });
}
let server;
try {
  server = await startServer(book, path.join(out, 'memory'), process.cwd(), {
    UNDERSTAND_BOOK_PRIVATE_DIR: path.join(out, 'private'), OPENCODE_BASE_URL: recorder.url,
    FLUID_LLM_MODEL: provider.model, UB_OBSERVABILITY_MODE: 'off', LANGSMITH_TRACING: 'false',
  });
  const { source_id, source_revision } = await server.api('tutor/readiness');
  const artifact = 'docs/examples/inquiry-course/ai-infra-resource-bounds.course.json';
  const course = JSON.parse(fs.readFileSync(artifact, 'utf8'));
  for (const { summary } of course.lessons) {
    Object.assign(summary.source, { source_revision, publication: { book_id: source_id, publication_id: 'local' } });
  }
  fs.writeFileSync(path.join(book, 'inquiry_course.json'), JSON.stringify({
    version: 'course_artifact.v1', source_id, source_revision, course,
  }));
  const endpoint = 'book/course/' + encodeURIComponent(source_id);
  let lesson;
  async function act(action) {
    lesson = await server.api(endpoint, { operation_id: crypto.randomUUID(), run_id: lesson?.progress.run_id ?? null,
      expected_revision: lesson?.progress.revision ?? 0, release_version: 1, action }, 'POST');
  }
  if (previous) {
    assert.equal(source_revision, previous.source_revision);
    lesson = await server.api(endpoint, { run_id: previous.run_id });
  } else {
    await server.api('agent/new', {}, 'POST');
    await act({ kind: 'start', lesson_id: course.lessons[0].summary.id });
    await act({ kind: 'advance' });
    await act({ kind: 'predict', response: { choice: 'compute', reason: '' } });
    for (const preset of ['baseline', 'double_compute', 'double_bandwidth']) await act({ kind: 'observe', preset });
    await act({ kind: 'explore', state: { ...lesson.lab, read_gb: 35 } });
    await act({ kind: 'advance' });
  }
  assert.equal(lesson.progress.stage, 'explanation');
  save('before.json', lesson);
  const chat = (await server.api('agent/history')).current.id;
  Object.assign(report, { url: server.url, source_id, source_revision, artifact,
    run_id: lesson.progress.run_id, session_id: chat });
  save('session.json', report);
  console.log('Open the course in the Reader:', server.url, 'Run:', report.run_id);
  const input = createInterface({ input: process.stdin, output: process.stdout });
  try { await input.question('After the course, clarification and closed-book trial, press Enter to retain the evidence.\n'); }
  finally { input.close(); }
  const after = await server.api(endpoint, { run_id: report.run_id });
  const history = await server.api('agent/history');
  const practice = await server.api('tutor/closed-book', { action: 'state', chat_session_id: history.current.id }, 'POST');
  save('after.json', after);
  save('history.json', history);
  save('practice.json', practice);
  report.stage_inputs = stageInputs(recorder.records);
  report.cumulative_stage_inputs = [...priorStages, { directory: out, calls: report.stage_inputs }];
  const rounds = inspectRounds(history, after);
  save('cards.json', rounds);
  assert.equal(practice.owner_id, after.progress.run_id);
  assert.equal(practice.course_origin.round_index, lesson.round_index);
  for (const card of rounds) assert.ok(practice.support_delivery_refs?.includes(card.round_id));
  assert.ok(practice.transfer_condition?.trim(), 'No changed condition was delivered for the practice');
  assert.equal(practice.status, 'ended');
  const allCalls = report.cumulative_stage_inputs.flatMap(attempt => attempt.calls);
  for (const stage of ['organize_expression', 'compare_passages']) {
    assert.ok(allCalls.filter(call => call.stage === stage).length >= rounds.length);
  }
  report.status = 'execution_passed_semantic_review_pending';
} catch (error) {
  report.status = 'failed';
  report.error = error.message;
  process.exitCode = 1;
} finally {
  await server?.stop();
  await recorder.stop(30000);
  save('provider.json', recorder.records);
  report.usage = measuredUsage(recorder.records.filter(record => !record.blocked));
  report.cumulative_usage = { requests: priorUsage.requests + report.usage.requests,
    total_tokens: report.usage.total_tokens == null ? null : priorUsage.total_tokens + report.usage.total_tokens };
  report.budget_stops = recorder.budgetRejections;
  report.finished_at = new Date();
  save('run.json', report);
  console.log(report);
}
