// One real Reader/browser conversation: purpose click, automatic coach, latest
// learner feedback, and the next turn's exact persisted guidance. No API mocks.
// Run only after compiling the intended Server and web snapshot:
// node evals/semantic/teaching-coach.mjs --out <fresh ignored evidence directory>
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';
import { startServer } from './server.mjs';
import { startProviderRecorder, measuredUsage, requestPurpose } from './provider-recorder.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const index = args.indexOf(name);
  return index < 0 ? fallback : args[index + 1];
};
const stamp = new Date().toISOString().replace(/[:.]/g, '-');
const output = path.resolve(option('--out', path.join(root, 'evals/semantic/quality-results', `teaching-coach-${stamp}`)));
const book = path.resolve(option('--book', path.join(root, '.understand-book/understandbook')));
const chapterTitle = option('--chapter', '第 1 章 一本书怎样成为可以交互的阅读环境');
const binary = option('--server', process.env.SEMANTIC_EVAL_SERVER ?? path.join(root, 'target/debug', process.platform === 'win32' ? 'server.exe' : 'server'));
const turnTimeout = Number(option('--turn-timeout-ms', '480000'));
const coachTimeout = Number(option('--coach-timeout-ms', '120000'));
const feedback = option('--feedback', '我能复述你刚才讲的结论，但还不明白它为什么成立。请先用一个很小的对照例子把这一步讲清楚，暂时不要让我做题，也不要展开下一节。');
if (args.includes('--help')) {
  console.log('Usage: node evals/semantic/teaching-coach.mjs [--out FRESH_DIR] [--book BOOK_DIR] [--chapter TITLE] [--server BINARY] [--feedback TEXT] [--turn-timeout-ms 480000] [--coach-timeout-ms 120000]');
  process.exit(0);
}
assert(!fs.existsSync(output), 'Choose a fresh output directory; existing evidence is never overwritten.');
assert(Number.isFinite(turnTimeout) && turnTimeout > 0 && Number.isFinite(coachTimeout) && coachTimeout > 0, 'Timeouts must be positive milliseconds.');
assert(feedback.trim(), 'The second turn needs a real submitted feedback string.');
assert(fs.existsSync(binary), 'Compile the intended Server binary before this run.');
assert(fs.existsSync(path.join(book, 'base.json')) && fs.existsSync(path.join(book, 'source.txt')), 'The selected real material is unavailable.');
if (fs.existsSync(path.join(root, '.env'))) process.loadEnvFile(path.join(root, '.env'));
assert(['OPENCODE_BASE_URL', 'OPENCODE_API_KEY', 'FLUID_LLM_MODEL'].every(key => process.env[key]), 'The existing Provider configuration is required.');
process.env.SEMANTIC_EVAL_SERVER = path.resolve(binary);
fs.mkdirSync(output, { recursive: true });
const memory = path.join(output, 'memory');
const privateDirectory = path.join(output, 'private');
const learningDb = path.join(memory, 'learning.db');
const pause = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const save = (name, value) => fs.writeFileSync(path.join(output, name), JSON.stringify(value, null, 2) + '\n');
const report = {
  protocol: 'teaching-coach-live.v1', started_at: new Date().toISOString(), status: 'running',
  model: process.env.FLUID_LLM_MODEL, book_directory: book, chapter_title: chapterTitle,
  executable: { path: path.resolve(binary), modified_at: fs.statSync(binary).mtime.toISOString() },
  web_dist: path.join(root, 'packages/web/dist'), memory_directory: memory, private_directory: privateDirectory,
  learner_origin: 'One scripted simulated learner expression; not human learning evidence.',
  feedback, turns: [], checks: [], browser_errors: [],
  monetary_cost: { status: 'unknown', reason: 'This isolated local Host records actual Provider usage. It has no network account charge ledger or verified supplier price; token counts are not converted to money.' },
};
const checkpoint = () => save('run.json', report);
function check(name, passed, evidence) {
  report.checks.push({ name, passed: Boolean(passed), evidence });
  checkpoint();
}
function readLearning() {
  if (!fs.existsSync(learningDb)) return { jobs: [], dependencies: [], events: [] };
  const db = new DatabaseSync(learningDb, { readOnly: true });
  try {
    db.exec('PRAGMA busy_timeout=2000');
    const tables = new Set(db.prepare("SELECT name FROM sqlite_master WHERE type='table'").all().map(row => row.name));
    return {
      jobs: tables.has('teaching_coach_jobs') ? db.prepare('SELECT job FROM teaching_coach_jobs ORDER BY seq').all().map(row => JSON.parse(row.job)) : [],
      dependencies: tables.has('teaching_coach_turns') ? db.prepare('SELECT context FROM teaching_coach_turns ORDER BY rowid').all().map(row => JSON.parse(row.context)) : [],
      events: tables.has('teaching_trace') ? db.prepare('SELECT seq,event FROM teaching_trace ORDER BY seq').all().map(row => ({ seq: row.seq, ...JSON.parse(row.event) })) : [],
    };
  } finally { db.close(); }
}
const messageText = message => typeof message?.content === 'string' ? message.content
  : Array.isArray(message?.content) ? message.content.filter(part => part.type === 'text').map(part => part.text).join('\n') : '';
const systemText = request => (request?.messages ?? []).filter(message => ['system', 'developer'].includes(message.role)).map(messageText).join('\n');
function purpose(record) {
  const system = systemText(record.request);
  if (system.startsWith('你是 Understand Book 的教学教练。')) return 'tutor_coach';
  if (system.startsWith('Teaching delivery acceptance review.')
    || system.includes('teaching_delivery_review') || system.includes('tutor_delivery_review')) return 'tutor_delivery_review';
  return requestPurpose(record.request);
}
function coachInput(record) {
  if (purpose(record) !== 'tutor_coach') return null;
  const user = record.request?.messages?.filter(message => message.role === 'user').at(-1);
  try { return JSON.parse(messageText(user)); } catch { return null; }
}
function toolNames(record) {
  return [
    ...(record.request?.tools ?? []).map(tool => tool.function?.name ?? tool.name),
    ...(record.response?.choices ?? []).flatMap(choice => (choice.message?.tool_calls ?? []).map(call => call.function?.name ?? call.name)),
  ].filter(Boolean);
}
function persistProvider() {
  if (!recorder) return;
  save('provider.json', recorder.records);
  report.usage = measuredUsage(recorder.records);
  report.usage_by_purpose = Object.fromEntries([...new Set(recorder.records.map(purpose))].map(name => [name, measuredUsage(recorder.records.filter(record => purpose(record) === name))]));
  report.provider_order = recorder.records.map(record => {
    const packet = coachInput(record);
    return { ordinal: record.ordinal, purpose: purpose(record), started_at: record.started_at,
      elapsed_ms: record.elapsed_ms ?? null, response_complete: record.response_complete ?? false,
      status: record.status ?? null, usage: record.usage ?? null,
      coach_trigger: packet?.trigger ?? null, source_turn_ref: packet?.source_turn_ref ?? null,
      tools: toolNames(record), error: record.error ?? record.provider_error ?? null };
  });
}
let server, browser, page, recorder, activeTurn;
let stage = 'startup';
let acceptedRequests = 0;
let observedProviderRequests = 0, observedProviderCompletions = 0;
function checkpointProviderProgress() {
  if (!recorder) return;
  const completions = recorder.records.filter(record => record.elapsed_ms != null).length;
  if (recorder.records.length !== observedProviderRequests || completions !== observedProviderCompletions) {
    observedProviderRequests = recorder.records.length;
    observedProviderCompletions = completions;
    persistProvider(); checkpoint();
  }
}
async function acceptFromBrowser(action) {
  const response = page.waitForResponse(result => result.request().method() === 'POST'
    && new URL(result.url()).pathname === '/api/agent/runs', { timeout: 30000 });
  const clickedAt = new Date().toISOString();
  await action();
  const received = await response;
  const value = await received.json();
  const request = received.request().postDataJSON();
  if (!received.ok() || !value.turn_id) {
    report.rejected_browser_action = { clicked_at: clickedAt, received_at: new Date().toISOString(),
      http_status: received.status(), request, response: value };
    checkpoint();
  }
  assert(received.ok() && value.turn_id,
    `Reader did not accept this browser action: ${received.status()} ${value.error_code ?? ''} ${value.message ?? ''}`);
  acceptedRequests++;
  const row = { index: acceptedRequests, clicked_at: clickedAt, accepted_at: new Date().toISOString(),
    request, descriptor: value, status: 'pending_assistant' };
  report.turns.push(row); activeTurn = value.turn_id; checkpoint();
  return row;
}
async function waitForTurn(row) {
  const deadline = Date.now() + turnTimeout;
  while (Date.now() < deadline) {
    checkpointProviderProgress();
    const history = await server.api('agent/history');
    const turn = history.current?.turns?.find(item => item.turn_id === row.descriptor.turn_id);
    if (turn && turn.status !== 'pending_assistant') {
      row.turn = turn; row.status = turn.status; row.completed_observed_at = new Date().toISOString();
      activeTurn = null;
      save(`turn-${row.index}.json`, row);
      fs.writeFileSync(path.join(output, `answer-${row.index}.md`), turn.outcome?.answer ?? '');
      checkpoint();
      assert.equal(turn.status, 'completed', `Turn ${row.index} failed: ${turn.error?.error_code ?? turn.status}`);
      assert(!turn.outcome?.incomplete && turn.outcome?.answer?.trim(), `Turn ${row.index} did not deliver a complete answer.`);
      return turn;
    }
    await pause(250);
  }
  throw new Error(`Turn ${row.index} exceeded its ${turnTimeout} ms deadline.`);
}
async function waitForAfterTurn(turnId) {
  const deadline = Date.now() + coachTimeout;
  while (Date.now() < deadline) {
    checkpointProviderProgress();
    const snapshot = readLearning();
    const job = snapshot.jobs.find(item => item.input.origin === 'after_turn' && item.input.source_ref === turnId);
    if (job && !['pending', 'running'].includes(job.status)) return { job, snapshot };
    await pause(250);
  }
  throw new Error(`Automatic after-turn coach did not reach a terminal state within ${coachTimeout} ms.`);
}
checkpoint();
try {
  recorder = await startProviderRecorder(process.env.OPENCODE_BASE_URL);
  server = await startServer(book, memory, root, { OPENCODE_BASE_URL: recorder.url,
    UNDERSTAND_BOOK_PRIVATE_DIR: privateDirectory });
  report.reader_url = server.url;
  browser = await chromium.launch({ headless: true });
  page = await browser.newPage({ viewport: { width: 1500, height: 1000 } });
  page.setDefaultTimeout(30000);
  page.on('pageerror', error => { report.browser_errors.push(error.message); checkpoint(); });
  await page.goto(server.url, { waitUntil: 'domcontentloaded' });
  const purposeButton = page.locator('#reader-panel-agent .learning-purpose-entry [data-purpose="understand"]');
  await purposeButton.waitFor({ state: 'visible' });
  if (chapterTitle) {
    stage = 'chapter_navigation';
    await page.locator('nav[aria-label="书籍目录"] .outline-item').filter({ has: page.locator('.outline-title', { hasText: chapterTitle }) }).click();
    await page.waitForFunction(title => {
      const selected = document.querySelector('#reader-panel-agent .purpose-scope option:checked');
      return selected?.textContent.includes(title);
    }, chapterTitle);
  }
  await page.screenshot({ path: path.join(output, '01-purpose-entry.png'), fullPage: true });
  stage = 'purpose_click';
  const first = await acceptFromBrowser(() => purposeButton.click());
  check('actual_understand_purpose_click', first.request.entry_action === 'learn_with_purpose'
    && first.request.learning_purpose === 'understand', first.request);
  stage = 'first_turn';
  await waitForTurn(first);

  // Do not wait for after-turn coaching or inspect the answer before submitting
  // the new expression. This is the supported overlap the real UI must handle.
  stage = 'immediate_feedback';
  await page.locator('.agent-input textarea').fill(feedback);
  const send = page.locator('.agent-input .send-agent');
  await send.waitFor({ state: 'visible' });
  report.coach_at_feedback_submission = readLearning().jobs.map(job => ({ job_ref: job.input.job_ref,
    origin: job.input.origin, source_ref: job.input.source_ref, status: job.status }));
  const second = await acceptFromBrowser(() => send.click());
  report.feedback_after_first_completion_ms = Date.parse(second.clicked_at) - Date.parse(first.completed_observed_at);
  check('latest_feedback_submitted_through_ui', second.request.message === feedback, { submitted: second.request.message });
  stage = 'second_turn';
  await waitForTurn(second);
  stage = 'second_after_turn_coach';
  const { job: finalCoach, snapshot } = await waitForAfterTurn(second.descriptor.turn_id);
  save('learning.json', snapshot);
  report.final_coach = finalCoach;
  const renderedSecond = page.locator(`.turn[data-turn-id="${second.descriptor.turn_id}"] .ans-text`);
  await renderedSecond.waitFor({ state: 'visible' });
  const renderedText = await renderedSecond.innerText();
  fs.writeFileSync(path.join(output, 'browser-second-answer.txt'), renderedText);
  check('second_answer_rendered_in_browser', renderedText.includes(second.turn.outcome.answer.split('\n')[0]),
    { turn_id: second.descriptor.turn_id, rendered_characters: renderedText.length });
  await renderedSecond.locator('p').first().scrollIntoViewIfNeeded();
  await page.screenshot({ path: path.join(output, '02-two-turn-conversation.png'), fullPage: true });
  persistProvider();

  const firstTurnId = first.descriptor.turn_id, secondTurnId = second.descriptor.turn_id;
  const initialJob = snapshot.jobs.find(job => job.input.origin === 'stage_start');
  const firstAfter = snapshot.jobs.filter(job => job.input.origin === 'after_turn' && job.input.source_ref === firstTurnId);
  const secondAfter = snapshot.jobs.filter(job => job.input.origin === 'after_turn' && job.input.source_ref === secondTurnId);
  const firstDependency = snapshot.dependencies.find(dependency => dependency.turn_id === firstTurnId);
  const secondDependency = snapshot.dependencies.find(dependency => dependency.turn_id === secondTurnId);
  check('one_bootstrap_and_one_job_per_completed_turn', snapshot.jobs.length === 3
    && initialJob && firstAfter.length === 1 && secondAfter.length === 1,
  snapshot.jobs.map(job => ({ job_ref: job.input.job_ref, origin: job.input.origin, source_ref: job.input.source_ref, status: job.status })));
  check('all_three_automatic_coach_jobs_ready', snapshot.jobs.length === 3 && snapshot.jobs.every(job => job.status === 'ready'),
    snapshot.jobs.map(job => ({ job_ref: job.input.job_ref, status: job.status, error: job.error })));
  check('first_turn_consumes_bootstrap', firstDependency?.job_ref === initialJob?.input.job_ref
    && firstDependency?.context?.status === 'ready' && firstDependency?.context?.coach_ref === initialJob?.guidance_ref, firstDependency);
  check('second_turn_consumes_exact_first_after_guidance', secondDependency?.job_ref === firstAfter[0]?.input.job_ref
    && secondDependency?.context?.status === 'ready' && secondDependency?.context?.coach_ref === firstAfter[0]?.guidance_ref, secondDependency);

  const coaches = recorder.records.filter(record => purpose(record) === 'tutor_coach');
  const secondMain = recorder.records.filter(record => purpose(record) === 'business_loop'
    && (record.request.messages ?? []).some(message => message.role === 'user' && messageText(message).includes(feedback)));
  const firstAfterCall = coaches.find(record => coachInput(record)?.source_turn_ref === firstTurnId);
  const secondAfterCall = coaches.find(record => coachInput(record)?.source_turn_ref === secondTurnId);
  const initialCall = coaches.find(record => coachInput(record)?.trigger === 'stage_start');
  const firstMain = recorder.records.find(record => purpose(record) === 'business_loop');
  check('host_schedules_bootstrap_then_after_each_turn', coaches.length === 3 && initialCall && firstAfterCall && secondAfterCall,
    coaches.map(record => ({ ordinal: record.ordinal, trigger: coachInput(record)?.trigger, source_turn_ref: coachInput(record)?.source_turn_ref })));
  check('bootstrap_precedes_first_main_sample', initialCall?.ordinal < firstMain?.ordinal
    && systemText(firstMain?.request).includes(initialJob?.guidance_ref ?? '__missing__'),
  { coach_ordinal: initialCall?.ordinal, main_ordinal: firstMain?.ordinal, coach_ref: initialJob?.guidance_ref });
  check('second_main_has_latest_input_and_exact_guidance', secondMain.length > 0
    && secondMain.every(record => systemText(record.request).includes(secondDependency?.context?.coach_ref ?? '__missing__')
      && systemText(record.request).includes(firstTurnId)),
  { model_request_ordinals: secondMain.map(record => record.ordinal), coach_ref: secondDependency?.context?.coach_ref, latest_input: feedback });
  check('first_after_coach_finishes_before_second_main', firstAfterCall && secondMain[0]
    && firstAfterCall.ordinal < secondMain[0].ordinal
    && Date.parse(firstAfterCall.started_at) + firstAfterCall.elapsed_ms <= Date.parse(secondMain[0].started_at),
  { coach_ordinal: firstAfterCall?.ordinal, main_ordinal: secondMain[0]?.ordinal, coach_elapsed_ms: firstAfterCall?.elapsed_ms });
  check('first_after_packet_does_not_claim_unseen_feedback', firstAfter[0]
    && !JSON.stringify(firstAfter[0].input.context).includes(feedback), { source_turn_ref: firstTurnId });
  check('second_after_packet_records_actual_feedback_and_consumed_guidance', secondAfter[0]
    && JSON.stringify(secondAfter[0].input.context).includes(feedback)
    && secondAfter[0].input.context.consumed_guidance?.coach_ref === firstAfter[0]?.guidance_ref,
  { source_turn_ref: secondTurnId, consumed_coach_ref: secondAfter[0]?.input.context.consumed_guidance?.coach_ref });
  const coachTools = recorder.records.flatMap(record => toolNames(record).filter(name => /(^|[._])coach($|[._])/i.test(name)).map(name => ({ ordinal: record.ordinal, name })));
  check('no_coach_tool_exposed_or_called', coachTools.length === 0, coachTools);
  const selected = snapshot.events.filter(event => event.binding?.turn_id === secondTurnId && event.payload?.move?.coach_ref);
  check('second_turn_records_adopted_guidance', selected.some(event => event.payload.move.coach_ref === firstAfter[0]?.guidance_ref),
    selected.map(event => ({ event_id: event.event_id, kind: event.kind, coach_ref: event.payload.move.coach_ref, adjustment: event.payload.move.coach_adjustment ?? null })));
  const selectedMove = selected.find(event => event.kind === 'move_selected')?.payload.move;
  const priorParticipation = firstAfter[0]?.guidance?.next_step?.participation?.choice;
  const actualParticipation = selectedMove?.participation?.choice;
  check('changed_participation_records_adjustment', !priorParticipation || !actualParticipation
    || priorParticipation === actualParticipation || selectedMove.coach_adjustment?.reason?.trim(),
  { recommended: priorParticipation, delivered: actualParticipation, adjustment: selectedMove?.coach_adjustment ?? null });
  check('coach_returns_are_tool_free', coaches.every(record => !(record.request.tools?.length)
    && !(record.response?.choices ?? []).some(choice => choice.message?.tool_calls?.length)), { coach_ordinals: coaches.map(record => record.ordinal) });
  report.semantic_review = { status: 'pending', evidence: ['answer-1.md', 'answer-2.md', 'learning.json', 'provider.json'],
    question: 'Did the second delivery use the actual request for a small contrast and direct help, adapting the prior guidance where necessary? Structural checks do not establish teaching quality or learning.' };
  report.status = report.checks.every(item => item.passed) ? 'passed_structural_checks_awaiting_semantic_review' : 'failed_checks';
} catch (error) {
  report.status = 'failed'; report.failed_stage = stage; report.error = error.message;
  try { if (page) await page.screenshot({ path: path.join(output, 'failure.png'), fullPage: true }); } catch { /* Main failure stays authoritative. */ }
} finally {
  if (activeTurn && server) {
    try { await server.api(`agent/runs/${encodeURIComponent(activeTurn)}/cancel`, {}, 'POST'); } catch (error) { report.cancel_error = error.message; }
  }
  try { if (browser) await browser.close(); } catch (error) { report.browser_close_error = error.message; }
  try { if (server) await server.stop(); } catch (error) { report.server_stop_error = error.message; }
  try { if (recorder) await recorder.stop(65000); } catch (error) { report.recorder_stop_error = error.message; }
  try { save('learning-final.json', readLearning()); } catch (error) { report.learning_read_error = error.message; }
  persistProvider();
  report.finished_at = new Date().toISOString(); checkpoint();
  console.log(JSON.stringify({ status: report.status, output, turns: report.turns.length,
    failed_checks: report.checks.filter(item => !item.passed).map(item => item.name), usage: report.usage, error: report.error }));
  if (!report.status.startsWith('passed_')) process.exitCode = 1;
}
