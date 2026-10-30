// Reproduce the recorded first-question teaching turn with the real Provider.
// Each invocation owns a fresh Reader, memory directory and private directory.
// --prepare-only exercises source/profile/control endpoints without submitting a turn.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { fileURLToPath } from 'node:url';
import { startServer } from './server.mjs';
import { startTeachingRecorder } from './teaching-replay-provider.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
if (args.includes('--help')) {
  console.log('node evals/semantic/teaching-revision-replay.mjs --server BINARY --out FRESH_DIR [--label baseline|candidate] [--history TRACE_JSON] [--book BOOK_DIR] [--scenario SCENARIO_JSON] [--background unavailable|enabled] [--prepare-only] [--turn-timeout-ms 900000]');
  process.exit(0);
}
const out = path.resolve(option('--out', 'tmp/teaching-revision-20261029/replay'));
const binary = path.resolve(option('--server', 'target/debug/server.exe'));
const historical = path.resolve(option('--history', 'tmp/latest-overview-failures-20261029/trace.private.json'));
const book = path.resolve(option('--book', '.understand-book/logic-of-failure'));
const turnTimeout = Number(option('--turn-timeout-ms', '900000'));
const prepareOnly = args.includes('--prepare-only');
const scenario = args.includes('--scenario') ? JSON.parse(fs.readFileSync(option('--scenario'), 'utf8')) : null;
assert(!fs.existsSync(out), 'Use a fresh output directory so every attempt remains available.');
assert(fs.existsSync(binary), 'Build or select the intended Server binary.');
assert(Number.isFinite(turnTimeout) && turnTimeout > 0, 'The turn timeout must be positive.');
if (fs.existsSync(path.join(root, '.env'))) process.loadEnvFile(path.join(root, '.env'));
assert(process.env.OPENCODE_API_KEY && process.env.OPENCODE_BASE_URL && process.env.FLUID_LLM_MODEL,
  'Use the existing real Provider configuration.');
const recorded = JSON.parse(fs.readFileSync(historical, 'utf8'));
const originalRun = recorded.runs.find(run => run.id === '01a1857e-ee20-7073-a7a2-096686388b6b');
assert(originalRun, 'The intended historical overview run is required.');
const originalModels = recorded.runs.filter(run => run.run_type === 'llm').sort((a, b) => a.start_time.localeCompare(b.start_time));
const originalFirstModel = originalModels.find(run => run.name === 'outer');
const originalSystem = originalFirstModel.inputs.system;
const contextLine = originalSystem.split('\n').find(line => line.startsWith('Tutor input context ('));
const originalContext = JSON.parse(contextLine.slice(contextLine.indexOf(': ') + 2));
assert.equal(process.env.FLUID_LLM_MODEL, originalFirstModel.inputs.model, 'Replay must use the historical model, not a faster substitute.');
assert.equal(originalRun.inputs.message, '这本书讲的是什么啊');
assert.equal(originalContext.binding.source_id, 'logic-of-failure');
assert.equal(originalContext.learning_purpose, null);
assert.equal(originalContext.learner_context.confirmed_background.length, 0);
assert.equal(originalContext.learner_context.constraints.length, 0);
process.env.SEMANTIC_EVAL_SERVER = binary;
fs.mkdirSync(out, { recursive: true });
const save = (name, value) => fs.writeFileSync(path.join(out, name), JSON.stringify(value, null, 2) + '\n');
const report = {
  protocol: 'teaching-revision-replay.v1', label: option('--label', 'candidate'),
  started_at: new Date().toISOString(), status: 'preparing', prepare_only: prepareOnly,
  binary, binary_modified_at: fs.statSync(binary).mtime.toISOString(), book,
  historical_run_id: originalRun.id, question: scenario?.question ?? originalRun.inputs.message,
  model: process.env.FLUID_LLM_MODEL, provider_mode: process.env.UNDERSTAND_BOOK_PROVIDER ?? 'native',
  provider_origin: new URL(process.env.OPENCODE_BASE_URL).origin,
  historical_model_settings: Object.fromEntries(['model', 'max_tokens', 'temperature', 'thinking', 'output_config', 'tool_choice'].map(key => [key, originalFirstModel.inputs[key]])),
  source_revision: originalContext.binding.source_revision,
  scenario: scenario?.id ?? 'overview',
  measurement_start: 'original_user_request_admission',
  background_control: option('--background', 'unavailable'),
  comparability: [
    'Same question, book source revision, confirmed explanation preferences, teaching enabled, no prior teaching stage, and configured model.',
    'Fresh local single-user process and private state; different generated IDs and profile/control revision counters.',
    'Historical run used a published multi-user Reader; local baseline and candidate use the same local host and source artifacts.',
    'The model generates all drafts, reads, reviews and corrections afresh. The historical answer is not supplied to it.',
    'One replay per binary is a case comparison, not a latency percentile or a guaranteed repeatable speedup.',
  ],
};
save('historical-context.private.json', { learner_context: originalContext.learner_context, teaching_assets: originalContext.teaching_assets });
fs.writeFileSync(path.join(out, 'historical-answer.md'), originalRun.outputs?.answer ?? '', 'utf8');
const checkpoint = () => save('run.json', report);
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
let server, activeTurn, eventsPromise, recorder;
const streamController = new AbortController();
const streamEvents = [];
function persistLearning() {
  const dbPath = path.join(out, 'memory', 'learning.db');
  if (!fs.existsSync(dbPath)) return;
  // This is the owned replay database, after its server stopped. SQLite may need
  // to recover a journal from shutdown before a SELECT can read the saved trace.
  const db = new DatabaseSync(dbPath);
  try {
    db.exec('PRAGMA busy_timeout=2000');
    const tables = new Set(db.prepare("SELECT name FROM sqlite_master WHERE type='table'").all().map(row => row.name));
    if (tables.has('teaching_trace')) save('teaching-events.private.json', db.prepare('SELECT seq,event FROM teaching_trace ORDER BY seq').all().map(row => ({ seq: row.seq, ...JSON.parse(row.event) })));
    if (tables.has('teaching_coach_jobs')) save('coach-jobs.private.json', db.prepare('SELECT job FROM teaching_coach_jobs ORDER BY seq').all().map(row => JSON.parse(row.job)));
  } finally { db.close(); }
}
async function consumeEvents(turnId) {
  const response = await fetch(new URL(`/api/agent/runs/${turnId}/events?after=0`, server.url), { signal: streamController.signal });
  assert(response.ok, `Run event stream: HTTP ${response.status}`);
  const decoder = new TextDecoder();
  let buffer = '';
  for await (const bytes of response.body) {
    buffer += decoder.decode(bytes, { stream: true });
    let end;
    while ((end = buffer.indexOf('\n\n')) >= 0) {
      const frame = buffer.slice(0, end); buffer = buffer.slice(end + 2);
      const data = frame.split('\n').filter(line => line.startsWith('data:')).map(line => line.slice(5).trimStart()).join('\n');
      if (!data) continue;
      const event = JSON.parse(data);
      streamEvents.push(event);
      fs.appendFileSync(path.join(out, 'events.private.jsonl'), JSON.stringify(event) + '\n');
      if (event.type === 'answer.patch' && event.payload?.view != null && report.first_answer_patch_ms == null) {
        report.first_answer_patch_ms = event.elapsed_ms; checkpoint();
      }
      if (event.type === 'model.finished' || (event.type === 'tool.finished' && ['failed', 'rejected'].includes(event.payload?.status))) {
        console.log(JSON.stringify({ elapsed_s: +(event.elapsed_ms / 1000).toFixed(1), name: event.payload.name, status: event.payload.status, error: event.payload.error_code }));
      }
    }
  }
}
function metrics(snapshot, turn) {
  const activities = snapshot.activities ?? [];
  const models = activities.filter(item => item.kind === 'model');
  const tools = activities.filter(item => item.kind === 'tool');
  const failed = tools.filter(item => ['failed', 'rejected'].includes(item.status));
  const byName = name => models.filter(item => item.name === name);
  const traces = turn?.outcome?.trace ?? [];
  const selects = traces.filter(item => item.tool === 'tutor.step' && /"operation"\s*:\s*"select"/.test(item.args));
  const reviewModels = models.filter(item => ['tutor_delivery_review', 'tutor_explanation_review'].includes(item.name));
  const selectionSteps = new Set(reviewModels.map(item => item.parent_step_id).filter(item => item != null));
  const selectedActivities = tools.filter(item => item.name === 'tutor.step' && selectionSteps.has(item.step_id));
  const argumentFailures = failed.filter(item => item.name === 'tutor.step' && item.error_code === 'TOOL_ARGUMENTS_INVALID');
  const terminal = streamEvents.find(event => ['run.completed', 'run.failed', 'run.cancelled', 'run.persistence_failed'].includes(event.type));
  return {
    model_calls: models.length,
    model_purposes: Object.fromEntries([...new Set(models.map(item => item.name))].map(name => [name, byName(name).length])),
    model_seconds: Object.fromEntries([...new Set(models.map(item => item.name))].map(name => [name, byName(name).reduce((sum, item) => sum + (item.duration_ms ?? 0), 0) / 1000])),
    tool_calls: tools.length, failed_or_rejected_tools: failed.length,
    plan_updates: tools.filter(item => item.name === 'goal.update').length,
    errors_by_code: Object.fromEntries([...new Set(failed.map(item => item.error_code))].map(code => [code, failed.filter(item => item.error_code === code).length])),
    reviewed_select_attempts: selectedActivities.length,
    rejected_reviewed_selects: selectedActivities.filter(item => ['failed', 'rejected'].includes(item.status)).length,
    successful_reviewed_selects: selectedActivities.filter(item => item.status === 'succeeded').length,
    tutor_argument_failures: argumentFailures.length,
    select_entries_in_outcome_trace: selects.length,
    first_answer_patch_ms: report.first_answer_patch_ms ?? null,
    first_useful_output_ms: streamEvents.find(event => event.type === 'board.committed' ||
      event.type === 'answer.patch' && event.payload?.view != null)?.elapsed_ms ?? null,
    run_finished_ms: terminal?.elapsed_ms ?? null,
    total_tokens: models.every(item => (item.usage?.total_tokens ?? item.usage_total_tokens) != null)
      ? models.reduce((sum, item) => sum + (item.usage?.total_tokens ?? item.usage_total_tokens), 0) : null,
    final_answer_chars: [...(turn?.outcome?.answer ?? '')].length,
    failed_tools: failed,
  };
}

try {
  checkpoint();
  recorder = await startTeachingRecorder(process.env.OPENCODE_BASE_URL, process.env.FLUID_LLM_MODEL,
    process.env.OPENCODE_API_KEY, value => save('provider-requests.json', value), report.background_control);
  server = await startServer(book, path.join(out, 'memory'), root, {
    UNDERSTAND_BOOK_PRIVATE_DIR: path.join(out, 'private'), UB_OBSERVABILITY_MODE: 'off',
    OPENCODE_BASE_URL: recorder.url,
    UNDERSTAND_BOOK_PREVIEW_BROWSER: option('--browser', 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe'),
  });
  report.reader_url = server.url;
  const readiness = await server.api('tutor/readiness');
  save('source-readiness.json', readiness);
  assert.equal(readiness.source_id, originalContext.binding.source_id);
  assert.equal(readiness.source_revision, originalContext.binding.source_revision, 'Historical and local book sources differ.');
  let profile = await server.api('profile/memory');
  for (const item of originalContext.learner_context.confirmed_preferences) {
    const preference = item.preference;
    await server.api('profile/memory/apply', {
      expected_document_revision: profile.status.document_revision,
      action: { kind: 'remember', operation_id: `replay-${item.fact_id}`, evidence_text: preference.claim.value,
        fact: { scope_kind: 'global', applicability_kind: 'any', applicability_value: null,
          payload_kind: preference.kind, payload_key: preference.claim.key, payload_value: preference.claim.value,
          sensitivity: 'normal', valid_until: null } },
    }, 'POST');
    profile = await server.api('profile/memory');
    const fact = profile.facts.find(fact => fact.payload_key === preference.claim.key && fact.payload_value === preference.claim.value);
    assert(fact?.status === 'confirmed', 'The replay preference must be confirmed before the question.');
  }
  save('profile.private.json', profile);
  let tutor = await server.api('tutor/state');
  if (!tutor.control.enabled) tutor = await server.api('tutor/mutate', {
    operation_id: 'replay-enable-teaching', expected_revision: tutor.control.revision,
    action: { kind: 'set_enabled', enabled: true },
  }, 'POST');
  assert(tutor.control.enabled && tutor.control.current_tutor_session_id == null);
  save('tutor-before.private.json', tutor);
  const history = await server.api('agent/history');
  const sessionId = history.current?.id ?? history.active_session_id;
  assert(sessionId && (history.current?.turns?.length ?? 0) === 0, 'The replay needs a new empty conversation.');
  report.request = { message: report.question, expected_chat_head: null, session_id: sessionId,
    client_request_id: randomUUID(), response_language: 'zh-CN' };
  if (scenario?.board) {
    const board = await server.api('boards/create', { title: scenario.title }, 'POST');
    report.board_id = board.board_id;
    report.request.board_follow_up = { board_id: board.board_id, board_revision: board.revision };
  }
  report.status = prepareOnly ? 'prepared_without_model_calls' : 'running'; checkpoint();
  if (!prepareOnly) {
    const started = performance.now();
    report.descriptor = await server.api('agent/runs', report.request, 'POST');
    activeTurn = report.descriptor.turn_id; checkpoint();
    eventsPromise = consumeEvents(activeTurn).catch(error => {
      if (error.name !== 'AbortError') { report.event_stream_error = error.message; checkpoint(); }
    });
    const deadline = Date.now() + turnTimeout;
    while (Date.now() < deadline) {
      const view = await server.api(`agent/runs/${activeTurn}`);
      report.snapshot = view.snapshot ?? view;
      report.metrics = metrics(report.snapshot, report.turn);
      const history = await server.api('agent/history');
      const turn = history.current?.turns?.find(item => item.turn_id === activeTurn);
      if (turn && turn.status !== 'pending_assistant') {
        report.turn = turn; report.status = turn.status;
        report.observed_total_ms = performance.now() - started;
        report.completed_at = new Date().toISOString();
        await pause(100);
        report.metrics = metrics(report.snapshot, turn);
        fs.writeFileSync(path.join(out, 'answer.md'), turn.outcome?.answer ?? '', 'utf8');
        save('activities.json', report.snapshot.activities ?? []);
        if (report.board_id) save('board.json', await server.api('boards/read', { board_id: report.board_id }, 'POST'));
        activeTurn = null; checkpoint();
        assert.equal(turn.status, 'completed', `The replay ended ${turn.status}: ${turn.error?.error_code ?? ''}`);
        assert(turn.outcome?.answer?.trim() && !turn.outcome.incomplete, 'The replay must deliver a complete answer.');
        break;
      }
      checkpoint();
      await pause(1000);
    }
    assert(!activeTurn, 'Replay exceeded its deadline; this incomplete attempt remains in the report.');
  }
} catch (error) {
  report.status = 'failed'; report.error = error.message;
  console.error(error.message); process.exitCode = 1;
} finally {
  if (activeTurn && server) {
    try { await server.api(`agent/runs/${activeTurn}/cancel`, {}, 'POST'); } catch {}
  }
  streamController.abort();
  if (eventsPromise) await eventsPromise;
  if (server) await server.stop();
  if (recorder) await recorder.stop();
  persistLearning();
  report.finished_at = new Date().toISOString(); checkpoint();
  console.log(JSON.stringify({ status: report.status, output: out, error: report.error, metrics: report.metrics }));
}
