// Four real Provider/Reader turns. Learner messages are scripted simulations;
// browser receipts prove player state, never human hearing or understanding.
// Compile the intended Server/web snapshot and publish the music material first.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';
import { startServer } from '../semantic/server.mjs';
import { startProviderRecorder, measuredUsage, requestPurpose } from '../semantic/provider-recorder.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
if (args.includes('--help')) {
  console.log('Usage: node evals/music/live-listening.mjs --book PUBLISHED_MUSIC_DIR [--out FRESH_DIR] [--server BINARY] [--token-limit 1800000] [--turn-timeout-ms 480000]');
  process.exit(0);
}
const book = path.resolve(option('--book', '.understand-book/music-schubert'));
const output = path.resolve(option('--out', `evals/semantic/quality-results/music-listening-${new Date().toISOString().replace(/[:.]/g, '-')}`));
const binary = path.resolve(option('--server', process.env.SEMANTIC_EVAL_SERVER ?? `target/debug/server${process.platform === 'win32' ? '.exe' : ''}`));
const turnTimeout = Number(option('--turn-timeout-ms', '480000'));
const tokenLimit = Number(option('--token-limit', '1800000'));
assert(!fs.existsSync(output), 'Use a fresh output directory; never overwrite evidence.');
assert(fs.existsSync(binary), 'Build the intended Server snapshot first.');
const material = JSON.parse(fs.readFileSync(path.join(book, 'music_source_manifest.json'), 'utf8'));
const publication = JSON.parse(fs.readFileSync(path.join(book, 'publication.json'), 'utf8')).reference;
assert(publication?.book_id === material.book_id, 'Publish this exact music material before evaluation.');
assert(Number.isFinite(turnTimeout) && turnTimeout > 0 && Number.isFinite(tokenLimit) && tokenLimit > 0);
if (fs.existsSync(path.join(root, '.env'))) process.loadEnvFile(path.join(root, '.env'));
assert(['OPENCODE_BASE_URL', 'OPENCODE_API_KEY', 'FLUID_LLM_MODEL'].every(key => process.env[key]), 'Existing Provider configuration is required.');
process.env.SEMANTIC_EVAL_SERVER = binary;
fs.mkdirSync(output, { recursive: true });
const memory = path.join(output, 'memory');
const messages = [
  '我完全没学过乐理。钢琴两只手是不是就等于两件乐器？请从这首曲子开头很短的一段带我分清旋律和伴奏；先解释一个能听的区别，放一小段让我试着找，不要默认我已经听过或已经听懂。',
  '我还是听不出哪条是旋律，只觉得都在响。先别考我，也别讲新术语；请先放一小段谱面合成的上方线条示范，说明它和原录音的关系，再回到原声的同一小段，用这个短对照帮我找。',
  '临时追问：你说旋律在上面，是指乐谱位置高、实际音更高，还是它更响？这三个是一回事吗？就用刚才那一小段说明，先不用继续往后。',
  '这回我能用自己的话说：同一架钢琴也能有不同声音职责，较高的一条像在唱，下面的声音给它支撑；但我还不能稳定跟住每个音。我们继续听下一小段吧，先给我一个要留意的变化，再播放，让我自己辨认。',
];
const report = {
  protocol: 'music-listening-live.v1', status: 'running', started_at: new Date().toISOString(),
  model: process.env.FLUID_LLM_MODEL, book_directory: book, publication,
  recording: material.recording.source, coverage: material.coverage,
  executable: { path: binary, modified_at: fs.statSync(binary).mtime.toISOString() },
  web_dist: path.join(root, 'packages/web/dist'), memory_directory: memory,
  learner_origin: 'Four scripted simulated learner expressions. No human learning or listening claim.',
  messages, turns: [], checks: [], browser_errors: [], browser_music_responses: [],
  monetary_cost: { status: 'unknown', reason: 'Actual Provider token usage is recorded. This isolated local Host has no network account debit ledger or verified supplier price.' },
};
const save = (name, value) => fs.writeFileSync(path.join(output, name), JSON.stringify(value, null, 2) + '\n');
const checkpoint = () => save('run.json', report);
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const check = (name, passed, evidence) => { report.checks.push({ name, passed: Boolean(passed), evidence }); checkpoint(); };
const messageText = message => typeof message?.content === 'string' ? message.content
  : Array.isArray(message?.content) ? message.content.filter(part => part.type === 'text').map(part => part.text).join('\n') : '';
const systemText = request => (request?.messages ?? []).filter(message => ['system', 'developer'].includes(message.role)).map(messageText).join('\n');
function purpose(record) {
  const text = systemText(record.request);
  if (text.startsWith('你是 Understand Book 的教学教练。')) return 'tutor_coach';
  if (text.startsWith('Teaching delivery acceptance review.') || /teaching_delivery_review|tutor_delivery_review/.test(text)) return 'tutor_delivery_review';
  if (text.startsWith('available_answer.v2')) return 'available_answer';
  return requestPurpose(record.request);
}
function coachInput(record) {
  if (purpose(record) !== 'tutor_coach') return null;
  try { return JSON.parse(messageText(record.request.messages.filter(message => message.role === 'user').at(-1))); } catch { return null; }
}
function readLearning() {
  const file = path.join(memory, 'learning.db');
  if (!fs.existsSync(file)) return { jobs: [], dependencies: [], events: [] };
  const db = new DatabaseSync(file, { readOnly: true });
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
let recorder, server, browser, page, activeTurn;
let stage = 'startup';
function persistProvider() {
  if (!recorder) return;
  save('provider.json', recorder.records);
  report.usage = measuredUsage(recorder.records);
  report.usage_by_purpose = Object.fromEntries([...new Set(recorder.records.map(purpose))].map(name => [name, measuredUsage(recorder.records.filter(record => purpose(record) === name))]));
  report.provider_order = recorder.records.map(record => ({ ordinal: record.ordinal, purpose: purpose(record),
    started_at: record.started_at, elapsed_ms: record.elapsed_ms ?? null, status: record.status,
    usage: record.usage ?? null, coach_trigger: coachInput(record)?.trigger ?? null,
    source_turn_ref: coachInput(record)?.source_turn_ref ?? null,
    tools_called: (record.response?.choices ?? []).flatMap(choice => (choice.message?.tool_calls ?? []).map(call => call.function?.name ?? call.name)),
    error: record.error ?? record.provider_error ?? null }));
}
let lastProviderProgress = '';
function progress() {
  const key = `${recorder?.records.length}:${recorder?.records.filter(record => record.elapsed_ms != null).length}`;
  if (key !== lastProviderProgress) { lastProviderProgress = key; persistProvider(); checkpoint(); }
}
async function until(read, predicate, timeout = turnTimeout) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    progress();
    const value = await read();
    if (predicate(value)) return value;
    await pause(300);
  }
  throw new Error(`Timed out at ${stage} after ${timeout} ms.`);
}
checkpoint();
try {
  recorder = await startProviderRecorder(process.env.OPENCODE_BASE_URL, { tokenLimit });
  server = await startServer(book, memory, root, { OPENCODE_BASE_URL: recorder.url, UNDERSTAND_BOOK_PRIVATE_DIR: path.join(output, 'private') });
  report.reader_url = server.url;
  // These are real production control operations, not model/tool mocks.
  const readiness = await server.api('tutor/readiness');
  assert.equal(readiness.status, 'ready', 'Publish complete teaching assets before the live run.');
  let state = await server.api('tutor/state');
  for (const action of [{ kind: 'set_enabled', enabled: true }, { kind: 'start',
    user_intent: messages[0], explicit_constraints: ['零基础，每次只解释一个能听到的关系'],
    material_scope: [{ source_id: material.book_id, scope_refs: [], role: 'primary' }],
    default_teaching_intent: null, learning_purpose: 'understand' }]) {
    state = await server.api('tutor/mutate', { operation_id: randomUUID(), expected_revision: state.control.revision, action }, 'POST');
  }
  report.initial_teaching_state = state;
  browser = await chromium.launch({ headless: true });
  page = await browser.newPage({ viewport: { width: 1500, height: 1000 } });
  page.setDefaultTimeout(30000);
  page.on('pageerror', error => { report.browser_errors.push(error.message); checkpoint(); });
  page.on('response', async response => {
    if (response.request().method() === 'POST' && new URL(response.url()).pathname === '/api/agent/music/respond') {
      try { report.browser_music_responses.push({ at: new Date().toISOString(), request: response.request().postDataJSON(), status: response.status(), response: await response.json() }); checkpoint(); } catch { /* Final report retains completed responses. */ }
    }
  });
  await page.goto(server.url, { waitUntil: 'domcontentloaded' });
  await page.locator('.music-reader').waitFor({ state: 'visible' });
  await page.waitForFunction(() => document.querySelector('.music-reader > audio')?.readyState > 0);
  await page.screenshot({ path: path.join(output, '00-music-reader.png'), fullPage: true });
  for (const [index, text] of messages.entries()) {
    stage = `turn_${index + 1}_submit`;
    // Capture through the real question control. Playback may later advance;
    // the accepted input must keep the original publication/time/occurrence.
    await page.locator('.music-ask').click();
    await page.locator('.agent-input textarea').fill(text);
    const received = page.waitForResponse(response => response.request().method() === 'POST' && new URL(response.url()).pathname === '/api/agent/runs');
    await page.locator('.agent-input .send-agent').click();
    const response = await received;
    const descriptor = await response.json();
    assert(response.ok() && descriptor.turn_id, `Reader rejected turn ${index + 1}: ${response.status()} ${descriptor.error_code ?? ''}`);
    activeTurn = descriptor.turn_id;
    const row = { index: index + 1, request: response.request().postDataJSON(), descriptor, submitted_at: new Date().toISOString() };
    report.turns.push(row); checkpoint();
    check(`turn_${index + 1}_actual_input_and_frozen_music`, row.request.message === text
      && row.request.music_question?.publication?.publication_id === publication.publication_id, row.request.music_question);
    stage = `turn_${index + 1}_response`;
    row.turn = await until(async () => (await server.api('agent/history')).current?.turns?.find(turn => turn.turn_id === descriptor.turn_id), turn => turn && turn.status !== 'pending_assistant');
    row.completed_at = new Date().toISOString(); activeTurn = null;
    save(`turn-${index + 1}.json`, row);
    fs.writeFileSync(path.join(output, `answer-${index + 1}.md`), row.turn.outcome?.answer ?? '');
    assert.equal(row.turn.status, 'completed', `Turn ${index + 1} failed: ${row.turn.error?.error_code ?? row.turn.status}`);
    assert(!row.turn.outcome?.incomplete && row.turn.outcome?.answer?.trim(), `Turn ${index + 1} did not deliver a complete answer.`);
    // History exposes public source cards, not private persisted bindings. Open
    // each actual card through the same source resolver used by the Reader.
    row.source_resolutions = [];
    for (const source of row.turn.outcome?.answer_view?.sources ?? []) {
      row.source_resolutions.push({ source_ref_id: source.source_ref_id,
        response: await server.api('agent/source.resolve', { turn_id: descriptor.turn_id, source_ref_id: source.source_ref_id }, 'POST') });
    }
    save(`turn-${index + 1}.json`, row); checkpoint();
    const rendered = page.locator(`.turn[data-turn-id="${descriptor.turn_id}"] .ans-text`);
    await rendered.waitFor({ state: 'visible' });
    fs.writeFileSync(path.join(output, `browser-answer-${index + 1}.txt`), await rendered.innerText());
    await rendered.scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.join(output, `0${index + 1}-conversation.png`), fullPage: true });
    // Submit the next expression without waiting for its predecessor's coach.
  }
  stage = 'final_after_turn_coach';
  const learning = await until(readLearning, snapshot => snapshot.jobs.some(job => job.input.origin === 'after_turn'
    && job.input.source_ref === report.turns.at(-1).descriptor.turn_id && !['pending', 'running'].includes(job.status)), 120000);
  save('learning.json', learning); persistProvider();
  const coaches = recorder.records.filter(record => purpose(record) === 'tutor_coach');
  check('one_bootstrap_and_one_after_each_turn', learning.jobs.length === messages.length + 1
    && learning.jobs.filter(job => job.input.origin === 'stage_start').length === 1
    && report.turns.every(row => learning.jobs.filter(job => job.input.origin === 'after_turn' && job.input.source_ref === row.descriptor.turn_id).length === 1),
  learning.jobs.map(job => ({ job_ref: job.input.job_ref, origin: job.input.origin, source_ref: job.input.source_ref, status: job.status })));
  check('all_automatic_coach_jobs_ready', learning.jobs.every(job => job.status === 'ready'), learning.jobs.map(job => ({ job_ref: job.input.job_ref, status: job.status, error: job.error })));
  for (const [index, row] of report.turns.entries()) {
    const expected = learning.jobs.find(job => index === 0 ? job.input.origin === 'stage_start' : job.input.origin === 'after_turn' && job.input.source_ref === report.turns[index - 1].descriptor.turn_id);
    const dependency = learning.dependencies.find(item => item.turn_id === row.descriptor.turn_id);
    check(`turn_${index + 1}_consumes_exact_prior_guidance`, dependency?.job_ref === expected?.input.job_ref
      && dependency?.context?.status === 'ready' && dependency?.context?.coach_ref === expected?.guidance_ref, dependency);
    const after = learning.jobs.find(job => job.input.origin === 'after_turn' && job.input.source_ref === row.descriptor.turn_id);
    check(`turn_${index + 1}_coach_uses_actual_expression`, JSON.stringify(after?.input.context).includes(messages[index]), { source_turn_ref: row.descriptor.turn_id });
    const main = recorder.records.filter(record => purpose(record) === 'business_loop'
      && Date.parse(record.started_at) >= Date.parse(row.submitted_at)
      && Date.parse(record.started_at) <= Date.parse(row.completed_at));
    check(`turn_${index + 1}_model_receives_exact_guidance`, main.length > 0
      && main.every(record => systemText(record.request).includes(expected?.guidance_ref ?? '__missing__')),
    { provider_ordinals: main.map(record => record.ordinal), coach_ref: expected?.guidance_ref });
    row.actual_provider_usage = measuredUsage(main);
  }
  check('real_provider_coach_calls_match_jobs', coaches.length === messages.length + 1, coaches.map(record => ({ ordinal: record.ordinal, trigger: coachInput(record)?.trigger, source_turn_ref: coachInput(record)?.source_turn_ref })));
  const exposedCoach = recorder.records.flatMap(record => (record.request?.tools ?? []).map(tool => tool.function?.name ?? tool.name)).filter(name => /(^|[._])coach($|[._])/i.test(name));
  check('coach_is_automatic_not_resident_tool', !exposedCoach.length && coaches.every(record => !record.request.tools?.length), exposedCoach);
  const called = report.provider_order.flatMap(record => record.tools_called);
  check('real_music_read_and_source_citation', called.includes('music_read') || called.includes('music.read'), called);
  check('source_present_called', called.includes('source_present') || called.includes('source.present'), called);
  const musicBindings = report.turns.flatMap(row => row.source_resolutions ?? []).map(source => ({ source_ref_id: source.source_ref_id, ...source.response })).filter(binding => binding.music);
  check('delivered_sources_keep_original_publication_and_recording', musicBindings.length > 0
    && musicBindings.every(binding => binding.music.target.publication.publication_id === publication.publication_id
      && binding.music.target.audio.recording.recording_id === material.recording.source.recording_id
      && binding.music.target.audio.recording.revision === material.recording.source.revision),
  musicBindings.map(binding => ({ source_ref_id: binding.source_ref_id, target: binding.music.target })));
  check('browser_acknowledged_music_action', report.browser_music_responses.some(row => row.response?.accepted && row.response.receipt?.status === 'applied'),
    report.browser_music_responses.filter(row => row.response?.accepted).map(row => row.response.receipt));
  check('browser_confirmed_original_recording_playing', report.browser_music_responses.some(row => row.response?.accepted
    && row.response.receipt?.status === 'applied' && row.response.receipt.paused === false
    && !row.response.receipt.demonstration_id), { meaning: 'The browser reported the original player running; this is not evidence of a human hearing it.' });
  check('browser_acknowledged_prepared_cue_before_play', report.browser_music_responses.some(row => row.response?.accepted
    && row.response.receipt?.status === 'applied' && row.response.receipt.paused === false
    && material.events.some(event => event.event_id === row.response.receipt.displayed_cue?.event_id
      && event.listen_for === row.response.receipt.displayed_cue?.text)),
    { meaning: 'The browser receipt identifies the published cue shown by the tested cue-before-play path; it does not prove that the learner read it.' });
  check('browser_confirmed_score_demo_with_its_own_player_state', report.browser_music_responses.some(row => row.response?.accepted
    && row.response.receipt?.status === 'applied' && row.response.receipt.demonstration_id
    && row.response.receipt.paused === true && row.response.receipt.demonstration_state?.paused === false
    && Number.isFinite(row.response.receipt.demonstration_state.current_ms)),
    { meaning: 'Original recording paused while a separately identified score-synthesis player ran; neither state proves human hearing.' });
  report.semantic_review = { status: 'pending', evidence: messages.map((_, index) => `answer-${index + 1}.md`),
    questions: ['Does the first delivery correct two hands versus two instruments using one audible relationship?', 'Does still-cannot-hear get a shorter concrete contrast without forced testing?', 'Does the interruption distinguish score position, pitch and loudness with source support?', 'Does continue-listening preserve uncertainty rather than marking mastery?'],
    limitation: 'Structural checks alone do not establish good teaching or human learning.' };
  report.status = report.checks.every(item => item.passed) ? 'passed_structural_checks_awaiting_semantic_review' : 'failed_checks';
} catch (error) {
  report.status = 'failed'; report.failed_stage = stage; report.error = error.message;
  try { if (page) await page.screenshot({ path: path.join(output, 'failure.png'), fullPage: true }); } catch { /* Keep primary error. */ }
} finally {
  if (activeTurn && server) try { await server.api(`agent/runs/${encodeURIComponent(activeTurn)}/cancel`, {}, 'POST'); } catch (error) { report.cancel_error = error.message; }
  try { if (browser) await browser.close(); } catch (error) { report.browser_close_error = error.message; }
  try { if (server) await server.stop(); } catch (error) { report.server_stop_error = error.message; }
  try { if (recorder) await recorder.stop(65000); } catch (error) { report.recorder_stop_error = error.message; }
  try { save('learning-final.json', readLearning()); } catch (error) { report.learning_read_error = error.message; }
  persistProvider(); report.finished_at = new Date().toISOString(); checkpoint();
  console.log(JSON.stringify({ status: report.status, output, turns: report.turns.length,
    failed_checks: report.checks.filter(item => !item.passed).map(item => item.name), usage: report.usage, error: report.error }));
  if (!report.status.startsWith('passed_')) process.exitCode = 1;
}
