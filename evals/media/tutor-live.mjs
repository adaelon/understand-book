// Real UI + production Reader + real Provider verification for media tutoring.
// Use a freshly built server/web snapshot. All private data stays in --out.
// Scripted learner messages test software behavior, not human learning outcomes.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { DatabaseSync } from 'node:sqlite';
import { startServer } from '../semantic/server.mjs';
import { startProviderRecorder, measuredUsage, requestPurpose } from '../semantic/provider-recorder.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const require = createRequire(new URL('../../packages/web/package.json', import.meta.url));
const { chromium, expect } = require('@playwright/test');
const args = process.argv.slice(2);
const option = (key, fallback) => args.includes(key) ? args[args.indexOf(key) + 1] : fallback;
if (args.includes('--help')) {
  console.log('Usage: node evals/media/tutor-live.mjs --out FRESH_DIR [--only video|music] [--server BINARY] [--web-dist DIRECTORY] [--video-book DIRECTORY] [--music-book DIRECTORY] [--token-limit 1800000] [--turn-timeout-ms 480000] [--headed]');
  process.exit(0);
}
assert(option('--out'), 'Pass a fresh --out directory. This run calls the real configured Provider.');
const output = path.resolve(option('--out'));
const binary = path.resolve(option('--server', process.env.SEMANTIC_EVAL_SERVER ?? path.join(root, 'target/debug/server.exe')));
const webDist = path.resolve(option('--web-dist', path.join(root, 'packages/web/dist')));
const tokenLimit = Number(option('--token-limit', '1800000'));
const turnTimeout = Number(option('--turn-timeout-ms', '480000'));
const only = option('--only');
assert(!only || ['video', 'music'].includes(only), '--only must be video or music.');
assert(!fs.existsSync(output), 'Use a fresh output directory; existing evidence must not be overwritten.');
assert(fs.existsSync(binary) && fs.existsSync(path.join(webDist, 'index.html')), 'Build the intended server and web snapshot first.');
assert(Number.isFinite(tokenLimit) && tokenLimit > 0 && Number.isFinite(turnTimeout) && turnTimeout > 0);
const cases = [
  {
    kind: 'video',
    book: path.resolve(option('--video-book', 'D:/codex-build/understand-book-vi18/production/service/library/published/19ac880d-a788-43f4-9725-6c56f4980189/01a129cf-c1b7-72c2-bac7-40f9740fda6f')),
    selector: '.video-reader video', seconds: 561.7,
    goal: '结合这段 CS336 视频约 9:00 到 10:10 的原字幕和画面，带我弄懂 FP16 的指数位为什么会限制数值范围，以及它与 BF16 的区别。每次只解释一个关系，给出可回看的来源。',
    followup: '我理解成指数位决定能表示多大或多小，尾数位影响数值有多精细。请接着核对这段视频，解释为什么同为16位的BF16能缓解FP16的范围问题，但不代表它在所有数值上都更精确；仍只讲这一处并保留来源。',
  },
  {
    kind: 'music',
    book: path.resolve(option('--music-book', path.join(root, '.understand-book/music-acceptance-service/library/published/music-schubert-d780-3-vetter/01a166d8-fabf-73d2-9502-4bf3d55da1b7'))),
    selector: '.music-reader > audio', seconds: 0,
    goal: '我是乐理初学者。请用这首舒伯特作品开头约12秒，带我分清旋律进入和下方伴奏的关系。先讲一个能留意的区别，实际播放一小段原录音，并给出能回看的乐谱或录音来源。',
    followup: '我现在知道同一架钢琴也能有旋律与伴奏，但还不能稳定听出旋律何时进入。请继续用开头同一小段，先说一个要留意的变化再播放原录音，并结合乐谱解释；先不要讲新术语。',
  },
].filter(item => !only || item.kind === only);
for (const item of cases) {
  item.publication = JSON.parse(fs.readFileSync(path.join(item.book, 'publication.json'), 'utf8')).reference;
  item.manifest = JSON.parse(fs.readFileSync(path.join(item.book, `${item.kind}_source_manifest.json`), 'utf8'));
}
if (fs.existsSync(path.join(root, '.env'))) process.loadEnvFile(path.join(root, '.env'));
assert(['OPENCODE_BASE_URL', 'OPENCODE_API_KEY', 'FLUID_LLM_MODEL'].every(key => process.env[key]), 'Existing Provider configuration is required.');
process.env.SEMANTIC_EVAL_SERVER = binary;
fs.mkdirSync(output, { recursive: true });
const report = {
  protocol: 'media-tutor-live.v1', status: 'running', started_at: new Date().toISOString(),
  server: { path: binary, modified_at: fs.statSync(binary).mtime.toISOString() }, web_dist: webDist,
  model: process.env.FLUID_LLM_MODEL, token_start_limit: tokenLimit, cases: [],
  learner_origin: 'Scripted learner messages; browser and persistence evidence does not establish human learning.',
};
const save = (file, value) => fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n');
const checkpoint = () => save(path.join(output, 'report.json'), report);
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
let recorder, browser, server, page, activeCase;
const currentSession = state => state.sessions[state.control.current_tutor_session_id];
const purpose = record => {
  const system = (record.request?.messages ?? []).filter(m => ['system', 'developer'].includes(m.role)).map(m => typeof m.content === 'string' ? m.content : '').join('\n');
  return system.startsWith('你是 Understand Book 的教学教练。') ? 'tutor_coach' : requestPurpose(record.request);
};
function readLearning(memory) {
  const db = new DatabaseSync(path.join(memory, 'learning.db'), { readOnly: true });
  try {
    db.exec('PRAGMA busy_timeout=2000');
    return {
      events: db.prepare('SELECT seq,event FROM teaching_trace ORDER BY seq').all().map(row => ({ seq: row.seq, ...JSON.parse(row.event) })),
      jobs: db.prepare('SELECT job FROM teaching_coach_jobs ORDER BY seq').all().map(row => JSON.parse(row.job)),
      dependencies: db.prepare('SELECT context FROM teaching_coach_turns ORDER BY rowid').all().map(row => JSON.parse(row.context)),
    };
  } finally { db.close(); }
}
async function until(read, predicate, label, timeout = turnTimeout) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const value = await read();
    if (predicate(value)) return value;
    await sleep(350);
  }
  throw new Error(`Timed out waiting for ${label}.`);
}
function check(name, passed, evidence) {
  activeCase.checks.push({ name, passed: Boolean(passed), evidence }); checkpoint();
  assert(passed, name);
}
async function mutate(action) {
  const state = await server.api('tutor/state');
  return server.api('tutor/mutate', { operation_id: randomUUID(), expected_revision: state.control.revision, action }, 'POST');
}
async function playerSnapshot(item) {
  return page.locator(item.selector).evaluate(media => ({ current_seconds: media.currentTime, duration_seconds: media.duration,
    paused: media.paused, ready_state: media.readyState, playback_rate: media.playbackRate, error: media.error?.message ?? null,
    video_width: media.videoWidth ?? null, video_height: media.videoHeight ?? null, current_source: media.currentSrc }));
}
async function preparePlayer(item) {
  await page.locator(item.selector).waitFor({ state: 'attached' });
  await page.waitForFunction(selector => document.querySelector(selector)?.readyState >= 2, item.selector, { timeout: 90000 });
  await page.locator(item.selector).evaluate((media, seconds) => { media.pause(); media.currentTime = seconds; }, item.seconds);
  await page.waitForFunction(({ selector, seconds }) => {
    const media = document.querySelector(selector);
    return media && !media.seeking && media.readyState >= 2 && Math.abs(media.currentTime - seconds) < .25;
  }, { selector: item.selector, seconds: item.seconds }, { timeout: 30000 });
  activeCase.player_before = await playerSnapshot(item);
  check('original_player_loaded_at_selected_segment', activeCase.player_before.error === null
    && activeCase.player_before.duration_seconds > (item.kind === 'video' ? 4600 : 90), activeCase.player_before);
  if (item.kind === 'video') {
    await page.locator(item.selector).evaluate(async media => { media.muted = true; await media.play(); });
    await page.waitForFunction(({ selector, seconds }) => {
      const media = document.querySelector(selector);
      return !media.paused && media.currentTime > seconds + .3;
    }, { selector: item.selector, seconds: item.seconds });
    activeCase.actual_video_playback = await playerSnapshot(item);
    await page.locator(item.selector).evaluate(media => media.pause());
    check('original_video_actually_advances', !activeCase.actual_video_playback.paused
      && activeCase.actual_video_playback.current_seconds > item.seconds + .3, activeCase.actual_video_playback);
  }
}
async function captureTurn(item, index, click) {
  // Capture each turn's actual player position, including ordinary follow-ups
  // after model-directed playback has moved the music player.
  const playerAtSubmit = await playerSnapshot(item);
  const accepted = page.waitForResponse(response => response.request().method() === 'POST' && new URL(response.url()).pathname === '/api/agent/runs', { timeout: 60000 });
  await click();
  const response = await accepted, descriptor = await response.json();
  check(`turn_${index}_accepted`, response.ok() && !!descriptor.turn_id, { status: response.status(), descriptor });
  const row = { index, descriptor, request: response.request().postDataJSON(), player_at_submit: playerAtSubmit, submitted_at: new Date().toISOString() };
  activeCase.turns.push(row); checkpoint();
  const coachSnapshot = await until(() => readLearning(activeCase.memory), snapshot => snapshot.dependencies.some(dependency =>
    dependency.turn_id === descriptor.turn_id && ['ready', 'unavailable'].includes(dependency.context?.status)),
  `${item.kind} turn ${index} frozen coach dependency`, Math.min(turnTimeout, 120000));
  const dependency = coachSnapshot.dependencies.find(candidate => candidate.turn_id === descriptor.turn_id);
  const coachJob = coachSnapshot.jobs.find(job => job.input.job_ref === dependency.job_ref);
  row.coaching = {
    dependency: { chat_session_id: dependency.chat_session_id, turn_id: dependency.turn_id, job_ref: dependency.job_ref,
      created_at: dependency.created_at, frozen_at: dependency.frozen_at, status: dependency.context.status,
      coach_job_ref: dependency.context.coach_job_ref, source_turn_ref: dependency.context.source_turn_ref,
      job_status: dependency.context.job_status, reason: dependency.context.reason },
    job: coachJob ? { job_ref: coachJob.input.job_ref, origin: coachJob.input.origin, source_ref: coachJob.input.source_ref,
      binding: coachJob.input.binding, status: coachJob.status, guidance_ref: coachJob.guidance_ref,
      error: coachJob.error ? { category: coachJob.error.category, error_code: coachJob.error.error_code,
        message: coachJob.error.message?.split(';原文=')[0] } : null } : null,
  };
  save(path.join(activeCase.directory, `turn-${index}.json`), row);
  save(path.join(activeCase.directory, `coach-turn-${index}.json`), row.coaching);
  check(`turn_${index}_coach_ready_before_teaching`, dependency.context.status === 'ready' && coachJob?.status === 'ready', row.coaching);
  row.turn = await until(async () => (await server.api('agent/history')).current?.turns?.find(turn => turn.turn_id === descriptor.turn_id),
    turn => turn && turn.status !== 'pending_assistant', `${item.kind} turn ${index} completion`);
  row.completed_at = new Date().toISOString();
  save(path.join(activeCase.directory, `turn-${index}.json`), row);
  fs.writeFileSync(path.join(activeCase.directory, `answer-${index}.md`), (row.turn.outcome?.answer ?? '') + '\n');
  save(path.join(activeCase.directory, `learning-after-turn-${index}.json`), readLearning(activeCase.memory));
  // Video submission names a persisted draft; admission resolves its immutable
  // context into history.video_question. Music submits its context inline.
  const question = row.turn[`${item.kind}_question`];
  const draftId = item.kind === 'video' ? row.request.video_draft_id : row.request.music_question?.draft_id;
  check(`turn_${index}_freezes_media_context`, !!draftId && question?.draft_id === draftId
    && question.publication?.book_id === item.publication.book_id
    && question.publication?.publication_id === item.publication.publication_id,
  { submitted_draft_id: draftId, frozen_question: question });
  const playerMs = playerAtSubmit.current_seconds * 1000;
  const frozenStart = item.kind === 'video' ? question.frame?.actual_pts_ms : question.audio?.start_ms;
  const frozenEnd = item.kind === 'video' ? frozenStart : question.audio?.end_ms;
  // All scripted submissions pause the player. A decoded video frame may differ
  // by a few frame intervals; music uses the score occurrence containing now.
  const toleranceMs = item.kind === 'video' ? 250 : 50;
  check(`turn_${index}_frozen_context_matches_current_player`, playerAtSubmit.paused
    && Number.isFinite(frozenStart) && Number.isFinite(frozenEnd)
    && playerMs >= frozenStart - toleranceMs && playerMs <= frozenEnd + toleranceMs,
  { player_at_submit: playerAtSubmit, frozen_start_ms: frozenStart, frozen_end_ms: frozenEnd, tolerance_ms: toleranceMs });
  if (item.kind === 'music') {
    const occurrence = item.manifest.occurrences.find(row => row.occurrence_id === question.occurrence_id);
    check(`turn_${index}_music_range_matches_published_occurrence`, occurrence
      && question.audio.start_ms === occurrence.start_ms && question.audio.end_ms === occurrence.end_ms
      && question.audio.recording.recording_id === item.manifest.recording.source.recording_id
      && question.audio.recording.revision === item.manifest.recording.source.revision
      && question.score?.score?.score_id === item.manifest.score.source.score_id
      && question.score?.score?.revision === item.manifest.score.source.revision
      && question.score.source_ids.every(id => occurrence.score_object_ids.includes(id)),
    { occurrence, frozen_question: question });
  }
  check(`turn_${index}_complete_answer`, row.turn.status === 'completed' && !row.turn.outcome?.incomplete && !!row.turn.outcome?.answer?.trim(),
    { status: row.turn.status, error: row.turn.error, incomplete: row.turn.outcome?.incomplete });
  row.sources = [];
  for (const source of row.turn.outcome.answer_view?.sources ?? []) {
    row.sources.push(await server.api('agent/source.resolve', { turn_id: descriptor.turn_id, source_ref_id: source.source_ref_id }, 'POST'));
  }
  check(`turn_${index}_sources_resolve`, row.sources.length > 0 && row.sources.every(source => source.stale === false && source.can_open_in_reader === true),
    row.sources.map(source => ({ source_ref_id: source.source_ref_id, stale: source.stale, can_open_in_reader: source.can_open_in_reader })));
  check(`turn_${index}_sources_keep_publication`, row.sources.every(source => {
    const publication = source.music?.target?.publication ?? source.media?.publication;
    return publication?.book_id === item.publication.book_id && publication?.publication_id === item.publication.publication_id;
  }), row.sources.map(source => source.music?.target?.publication ?? source.media?.publication));
  const learning = await until(() => readLearning(activeCase.memory), snapshot => snapshot.events.some(event => event.kind === 'message_delivered'
    && event.binding.turn_id === descriptor.turn_id && event.payload.move?.target), `turn ${index} persisted teaching delivery`, 30000);
  row.deliveries = learning.events.filter(event => event.kind === 'message_delivered' && event.binding.turn_id === descriptor.turn_id);
  check(`turn_${index}_target_bound_to_media_and_session`, row.deliveries.some(event => {
    const target = event.payload.move?.target;
    const bindings = item.kind === 'music' ? target?.music_bindings : [...(target?.media_bindings ?? []), ...(target?.source_bindings ?? [])];
    return event.binding.tutor_session_id === activeCase.session_id && event.binding.source_id === item.publication.book_id
      && target.learning_focus?.trim() && bindings?.length > 0 && bindings.every(binding => binding.source_id === item.publication.book_id);
  }), row.deliveries.map(event => ({ event_id: event.event_id, binding: event.binding, target: event.payload.move?.target })));
  row.player_after = await playerSnapshot(item);
  const rendered = page.locator(`.turn[data-turn-id="${descriptor.turn_id}"] .ans-text`);
  await rendered.waitFor({ state: 'visible', timeout: 30000 });
  await rendered.scrollIntoViewIfNeeded();
  fs.writeFileSync(path.join(activeCase.directory, `browser-answer-${index}.txt`), await rendered.innerText());
  await page.screenshot({ path: path.join(activeCase.directory, `0${index + 1}-teaching.png`), fullPage: true });
  save(path.join(activeCase.directory, `turn-${index}.json`), row); checkpoint();
}
async function runCase(item) {
  const directory = path.join(output, item.kind), memory = path.join(directory, 'memory'), privateDir = path.join(directory, 'private');
  fs.mkdirSync(directory, { recursive: true });
  activeCase = { kind: item.kind, status: 'running', directory, memory, publication: item.publication, book: item.book,
    goal: item.goal, followup: item.followup, checks: [], turns: [], browser_errors: [], music_responses: [], control_requests: [] };
  report.cases.push(activeCase); checkpoint();
  const callStart = recorder.records.length;
  const environment = { OPENCODE_BASE_URL: recorder.url, UNDERSTAND_BOOK_PRIVATE_DIR: privateDir,
    UNDERSTAND_BOOK_LIBRARY_ROOT: path.join(directory, 'library'), UNDERSTAND_BOOK_WEB_DIST: webDist };
  server = await startServer(item.book, memory, root, environment);
  activeCase.reader_url = server.url;
  const readiness = await server.api('tutor/readiness');
  check('actual_publication_ready_for_tutor', readiness.status === 'ready' && readiness.source_id === item.publication.book_id, readiness);
  // Reproduce the user's other-material goal, using only the supported control API.
  await mutate({ kind: 'set_enabled', enabled: false });
  const seeded = await mutate({ kind: 'start', user_intent: '保留此前材料的学习目标，切换材料后再开始新目标。', explicit_constraints: [],
    material_scope: [{ source_id: `previous-material-${item.kind}`, scope_refs: [], role: 'primary' }], default_teaching_intent: null });
  activeCase.previous_session_id = seeded.control.current_tutor_session_id;
  page = await browser.newPage({ viewport: { width: 1500, height: 1000 } });
  page.setDefaultTimeout(30000);
  page.on('pageerror', error => { activeCase.browser_errors.push(error.message); checkpoint(); });
  page.on('response', async response => {
    if (response.request().method() !== 'POST') return;
    const pathname = new URL(response.url()).pathname;
    if (pathname === '/api/tutor/mutate' || pathname === '/api/tutor/start') {
      activeCase.control_requests.push({ pathname, request: response.request().postDataJSON(), status: response.status() });
    }
    if (pathname === '/api/agent/music/respond') {
      try { activeCase.music_responses.push({ request: response.request().postDataJSON(), status: response.status(), response: await response.json() }); checkpoint(); } catch { /* Final report keeps completed responses. */ }
    }
  });
  await page.goto(server.url, { waitUntil: 'domcontentloaded' });
  await preparePlayer(item);
  await page.getByRole('button', { name: item.kind === 'video' ? '为这段视频设定目标' : '为这首作品设定目标', exact: true }).click();
  const editor = page.getByRole('dialog', { name: '调整目标', exact: true });
  await editor.locator('input[data-workspace-input="learning-goal"]').fill(item.goal);
  await editor.getByRole('button', { name: '保存目标', exact: true }).click();
  await expect(editor).toBeHidden();
  await expect(page.locator('.conversation-learning-goal .goal-text')).toHaveText(item.goal);
  const saved = await server.api('tutor/state');
  activeCase.session_id = saved.control.current_tutor_session_id;
  check('custom_goal_saved_for_current_material', currentSession(saved)?.user_intent === item.goal
    && currentSession(saved).material_scope.some(material => material.source_id === item.publication.book_id), saved);
  check('previous_material_goal_preserved', !!saved.sessions[activeCase.previous_session_id] && activeCase.session_id !== activeCase.previous_session_id);
  check('saving_goal_does_not_start_model_or_enable_tutor', !saved.control.enabled && recorder.records.length === callStart);
  await page.screenshot({ path: path.join(directory, '01-goal-saved.png'), fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  const goalCta = page.locator('.goal-resume').getByRole('button', { name: '开启并开始这次学习', exact: false });
  // Mobile moves the detailed goal into the existing Learning/Conversation
  // sheet. The visible welcome CTA is the mobile entry to the same resume flow.
  await expect(page.locator('.right-rail.conversation-mobile')).toBeVisible();
  const mobileCta = page.locator('.conversation-welcome').getByRole('button', { name: '开启并继续上次学习', exact: false });
  await expect(mobileCta).toBeInViewport({ ratio: 1 });
  activeCase.mobile_goal_layout = await mobileCta.evaluate(button => {
    const bounds = button.getBoundingClientRect();
    return { width: innerWidth, height: innerHeight, document_width: document.documentElement.scrollWidth,
      cta: { left: bounds.left, right: bounds.right, top: bounds.top, bottom: bounds.bottom, width: bounds.width, height: bounds.height } };
  });
  check('mobile_saved_goal_cta_fits_viewport', activeCase.mobile_goal_layout.document_width <= 390
    && activeCase.mobile_goal_layout.cta.left >= 0 && activeCase.mobile_goal_layout.cta.right <= 390
    && activeCase.mobile_goal_layout.cta.top >= 0 && activeCase.mobile_goal_layout.cta.bottom <= 844, activeCase.mobile_goal_layout);
  await page.screenshot({ path: path.join(directory, '01-goal-saved-mobile.png') });
  await page.getByRole('button', { name: '学习与会话', exact: true }).click();
  const mobileSheet = page.getByRole('dialog', { name: '学习与会话', exact: true });
  await expect(mobileSheet.locator('.goal-text')).toHaveText(item.goal);
  await page.screenshot({ path: path.join(directory, '01-goal-sheet-mobile.png') });
  await mobileSheet.getByRole('button', { name: '调整目标', exact: true }).click();
  const mobileEditor = page.getByRole('dialog', { name: '调整目标', exact: true });
  await expect(mobileEditor.locator('input[data-workspace-input="learning-goal"]')).toHaveValue(item.goal);
  check('mobile_goal_editor_has_no_horizontal_overflow', await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  await page.screenshot({ path: path.join(directory, '01-goal-editor-mobile.png') });
  await mobileEditor.getByRole('button', { name: '取消', exact: true }).click();
  await expect(mobileEditor).toBeHidden();
  await mobileSheet.getByRole('button', { name: '关闭学习与会话', exact: true }).click();
  await expect(mobileSheet).toBeHidden();
  await page.setViewportSize({ width: 1500, height: 1000 });
  await expect(goalCta).toBeInViewport({ ratio: 1 });
  check('mobile_goal_inspection_does_not_start_model', recorder.records.length === callStart);
  await captureTurn(item, 1, () => page.locator('.goal-resume').getByRole('button', { name: '开启并开始这次学习', exact: false }).click());
  const started = await server.api('tutor/state');
  check('start_button_enables_and_resumes_current_session', started.control.enabled && currentSession(started)?.id === activeCase.session_id && currentSession(started).status === 'active');
  // Ordinary follow-up uses the same visible input and frozen media selection.
  if (item.kind === 'music') await page.locator('.music-ask').click();
  await page.locator('.agent-input textarea').fill(item.followup);
  await captureTurn(item, 2, () => page.locator('.agent-input .send-agent').click());
  const learning = await until(() => readLearning(memory), snapshot => snapshot.jobs.some(job => job.input.origin === 'after_turn'
    && job.input.source_ref === activeCase.turns[1].descriptor.turn_id && !['pending', 'running'].includes(job.status)), 'final coaching result', 120000);
  activeCase.learning = learning;
  check('final_turn_prepares_ready_continuation', learning.jobs.some(job => job.input.origin === 'after_turn'
    && job.input.source_ref === activeCase.turns[1].descriptor.turn_id && job.status === 'ready'), learning.jobs);
  check('coaching_continues_from_actual_prior_turn', activeCase.turns.every((turn, index) => {
    const job = learning.jobs.find(candidate => index === 0 ? candidate.input.origin === 'stage_start'
      : candidate.input.origin === 'after_turn' && candidate.input.source_ref === activeCase.turns[index - 1].descriptor.turn_id);
    const dependency = learning.dependencies.find(candidate => candidate.turn_id === turn.descriptor.turn_id);
    return job?.status === 'ready' && dependency?.job_ref === job.input.job_ref && dependency?.context?.status === 'ready';
  }), { jobs: learning.jobs, dependencies: learning.dependencies });
  if (item.kind === 'music') {
    check('real_original_music_player_applied_model_action', activeCase.music_responses.some(row => row.response?.accepted
      && row.response.receipt?.status === 'applied' && row.response.receipt.paused === false && !row.response.receipt.demonstration_id), activeCase.music_responses);
  }
  const beforeRestart = await server.api('tutor/state');
  await server.stop(); server = null;
  server = await startServer(item.book, memory, root, environment);
  activeCase.restarted_reader_url = server.url;
  await page.goto(server.url, { waitUntil: 'domcontentloaded' });
  await page.locator(item.selector).waitFor({ state: 'attached' });
  await expect(page.locator('.conversation-learning-goal .goal-text')).toHaveText(item.goal);
  const afterRestart = await server.api('tutor/state'), recovered = readLearning(memory), history = await server.api('agent/history');
  check('goal_and_enabled_session_survive_server_restart', JSON.stringify(afterRestart) === JSON.stringify(beforeRestart), afterRestart);
  const delivered = activeCase.turns.flatMap(turn => turn.deliveries);
  check('teaching_targets_survive_server_restart', delivered.every(event => JSON.stringify(recovered.events.find(saved => saved.event_id === event.event_id)) === JSON.stringify(event)));
  check('both_answers_survive_server_restart', activeCase.turns.every(row => history.current?.turns?.some(turn => turn.turn_id === row.descriptor.turn_id && turn.status === 'completed')));
  for (const row of activeCase.turns) for (const source of row.sources) {
    const resolved = await server.api('agent/source.resolve', { turn_id: row.descriptor.turn_id, source_ref_id: source.source_ref_id }, 'POST');
    check(`source_${row.index}_${source.source_ref_id}_survives_restart`, resolved.stale === false && resolved.can_open_in_reader === true);
  }
  await page.screenshot({ path: path.join(directory, '04-restarted.png'), fullPage: true });
  check('browser_has_no_script_errors', activeCase.browser_errors.length === 0, activeCase.browser_errors);
  const records = recorder.records.slice(callStart);
  activeCase.usage = measuredUsage(records);
  activeCase.provider_calls = records.map(record => ({ ordinal: record.ordinal, purpose: purpose(record), status: record.status, usage: record.usage, elapsed_ms: record.elapsed_ms }));
  activeCase.semantic_review = { status: 'pending', answers: ['answer-1.md', 'answer-2.md'], questions: [
    'Does the teaching use the selected real material and the custom goal?', 'Does the follow-up address the actual learner expression and preserve uncertainty?',
    'Are the cited source claims supported and the next step understandable?',
  ] };
  activeCase.status = 'passed_structural_checks_awaiting_semantic_review';
  save(path.join(directory, 'learning.json'), learning); checkpoint();
  await page.close(); page = null;
  await server.stop(); server = null;
}
const progress = setInterval(() => {
  report.usage = recorder ? measuredUsage(recorder.records) : null; checkpoint();
  console.log(JSON.stringify({ status: report.status, material: activeCase?.kind, completed_turns: activeCase?.turns.filter(turn => turn.completed_at).length, usage: report.usage }));
}, 15000);
try {
  recorder = await startProviderRecorder(process.env.OPENCODE_BASE_URL, { tokenLimit });
  browser = await chromium.launch({ channel: option('--channel', 'msedge'), headless: !args.includes('--headed') });
  for (const item of cases) await runCase(item);
  report.status = 'passed_structural_checks_awaiting_semantic_review';
} catch (error) {
  report.status = 'failed'; report.error = error.stack ?? error.message; process.exitCode = 1;
  if (activeCase) { activeCase.status = 'failed'; activeCase.error = error.message; }
  if (page) {
    try { await page.screenshot({ path: path.join(activeCase.directory, 'failure.png'), fullPage: true });
      fs.writeFileSync(path.join(activeCase.directory, 'failure.txt'), await page.locator('body').innerText()); } catch { /* Preserve the primary failure. */ }
  }
} finally {
  clearInterval(progress);
  await browser?.close();
  if (server) await server.stop();
  if (recorder) {
    await recorder.stop(1000); save(path.join(output, 'provider.json'), recorder.records);
    report.usage = measuredUsage(recorder.records); report.budget_rejections = recorder.budgetRejections;
  }
  report.finished_at = new Date().toISOString(); checkpoint();
  console.log(JSON.stringify({ status: report.status, output, usage: report.usage, error: report.error }));
}
