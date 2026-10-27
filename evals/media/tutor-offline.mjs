// Recover already captured live-media evidence without contacting a model.
// Original evidence remains read-only; Reader restarts use copied private data.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual } from 'node:util';
import { DatabaseSync } from 'node:sqlite';
import { startServer } from '../semantic/server.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const option = key => args.includes(key) ? args[args.indexOf(key) + 1] : undefined;
if (args.includes('--help')) {
  console.log('Usage: node evals/media/tutor-offline.mjs --out FRESH_DIR --video LIVE_VIDEO_DIR --music LIVE_MUSIC_DIR');
  process.exit(0);
}
assert(option('--out') && (option('--video') || option('--music')), 'Provide output and live evidence directories.');
const output = path.resolve(option('--out'));
assert(!fs.existsSync(output), 'Use a fresh output directory.');
fs.mkdirSync(output, { recursive: true });
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const save = (file, value) => fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n');
const report = { protocol: 'media-tutor-offline.v1', started_at: new Date().toISOString(), status: 'running',
  live_acceptance: 'not_passed', model_calls: 0, blocked_local_provider_attempts: [], cases: [] };
const checkpoint = () => save(path.join(output, 'report.json'), report);
function learning(memory) {
  const db = new DatabaseSync(path.join(memory, 'learning.db'), { readOnly: true });
  try {
    const state = JSON.parse(db.prepare("SELECT state FROM tutor_projection WHERE learning_workspace_id = ''").get().state);
    const events = db.prepare('SELECT seq,event FROM teaching_trace ORDER BY seq').all().map(row => ({ seq: row.seq, ...JSON.parse(row.event) }));
    const jobs = db.prepare('SELECT job FROM teaching_coach_jobs ORDER BY seq').all().map(row => JSON.parse(row.job));
    return { state, events, jobs };
  } finally { db.close(); }
}
function check(result, name, passed, evidence) {
  result.checks.push({ name, passed: Boolean(passed), evidence }); checkpoint();
  assert(passed, `${result.kind}: ${name}`);
}
function publicationMatches(actual, expected) {
  return actual?.book_id === expected.book_id && actual?.publication_id === expected.publication_id;
}
function scopeEvidence(item, baseline) {
  const manifest = read(path.join(item.book, `${item.kind}_source_manifest.json`));
  const transcript = item.kind === 'video' ? read(path.join(item.book, manifest.transcript_map_path)) : null;
  return item.turns.map((row, index) => {
    const question = row.turn[`${item.kind}_question`];
    const player = row.player_at_submit ?? (index === 0 ? item.actual_video_playback ?? item.player_before : item.turns[index - 1].player_after);
    const playerMs = player?.current_seconds * 1000;
    const start = item.kind === 'video' ? question.frame.actual_pts_ms : question.audio.start_ms;
    const end = item.kind === 'video' ? start : question.audio.end_ms;
    const targetEvents = baseline.events.filter(event => event.binding?.turn_id === row.descriptor.turn_id
      && (event.payload?.design?.target || event.kind === 'message_delivered' && event.payload?.move?.target));
    const targets = targetEvents.map(event => {
      const target = event.payload.design?.target ?? event.payload.move.target;
      return { event_id: event.event_id, stage: event.kind, binding: event.binding, target };
    });
    const sameSource = publicationMatches(question.publication, item.publication) && targets.every(event =>
      event.binding.source_id === item.publication.book_id && event.binding.tutor_session_id === item.session_id
      && [...(event.target.source_bindings ?? []), ...(event.target.media_bindings ?? []), ...(event.target.music_bindings ?? [])]
        .every(binding => binding.source_id === item.publication.book_id
          && (!binding.target?.publication || publicationMatches(binding.target.publication, item.publication))));
    const publishedRange = item.kind === 'video'
      ? transcript.segments.filter(segment => segment.location.start_ms <= start && segment.location.end_ms >= end)
        .map(segment => ({ lid: segment.text_range.lid, start_ms: segment.location.start_ms, end_ms: segment.location.end_ms, text: segment.text }))
      : manifest.occurrences.filter(occurrence => occurrence.occurrence_id === question.occurrence_id);
    return { index: row.index, turn_id: row.descriptor.turn_id, incomplete: row.turn.outcome?.incomplete,
      player_snapshot_basis: row.player_at_submit ? 'immediately_before_submission' : index === 0 ? 'initial_playback_or_position' : 'previous_turn_player_after',
      player, frozen_question: question, player_delta_from_frozen_start_ms: playerMs - start,
      position_corresponds: Number.isFinite(playerMs) && playerMs >= start - 250 && playerMs <= end + 250,
      same_source: sameSource, published_range: publishedRange, targets,
      delivered_target_count: targets.filter(event => event.stage === 'message_delivered').length,
      source_targets: (row.sources ?? []).map(source => ({ source_ref_id: source.source_ref_id,
        target: source.music?.target ?? source.media, stale: source.stale, can_open_in_reader: source.can_open_in_reader })) };
  });
}

// No upstream URL or secret is loaded. Even an unexpected background job can
// reach only this local rejection endpoint, and its attempt is reported.
const deniedProvider = http.createServer((request, response) => {
  report.blocked_local_provider_attempts.push({ method: request.method, path: request.url, at: new Date().toISOString() });
  request.resume();
  response.writeHead(503, { 'Content-Type': 'application/json' });
  response.end(JSON.stringify({ error: { message: 'Offline recovery: model access disabled.' } }));
});
await new Promise(resolve => deniedProvider.listen(0, '127.0.0.1', resolve));
let server;
try {
  for (const kind of ['video', 'music']) {
    if (!option(`--${kind}`)) continue;
    const input = path.resolve(option(`--${kind}`));
    const live = read(path.join(input, 'report.json'));
    const item = live.cases.find(candidate => candidate.kind === kind);
    assert(item, `No ${kind} case in ${input}`);
    const directory = path.join(output, kind), memory = path.join(directory, 'memory'), privateDir = path.join(directory, 'private');
    fs.mkdirSync(directory, { recursive: true });
    fs.cpSync(item.memory, memory, { recursive: true });
    fs.cpSync(path.join(item.directory, 'private'), privateDir, { recursive: true });
    const baseline = learning(memory);
    const result = { kind, input, publication: item.publication, book: item.book, goal: item.goal, checks: [],
      live_status: live.status, complete_teaching_turns: item.turns.filter(row => row.turn.status === 'completed' && !row.turn.outcome?.incomplete).length,
      scope: scopeEvidence(item, baseline), music_player_receipts: item.music_responses.filter(row => row.response?.accepted).map(row => row.response.receipt),
      coaching: baseline.jobs.map(job => ({ status: job.status, origin: job.input.origin, source_ref: job.input.source_ref, error: job.error })),
      original_player_before: item.player_before, actual_video_playback: item.actual_video_playback };
    report.cases.push(result); checkpoint();
    check(result, 'frozen_media_and_existing_targets_are_same_source', result.scope.every(row => row.same_source));
    check(result, 'frozen_time_corresponds_to_recorded_player', result.scope.every(row => row.position_corresponds));
    check(result, 'frozen_time_has_published_segment_or_occurrence', result.scope.every(row => row.published_range.length > 0));
    process.env.SEMANTIC_EVAL_SERVER = live.server.path;
    server = await startServer(item.book, memory, root, {
      OPENCODE_BASE_URL: `http://127.0.0.1:${deniedProvider.address().port}/v1`, OPENCODE_API_KEY: 'offline-no-upstream',
      FLUID_LLM_MODEL: live.model, UNDERSTAND_BOOK_PRIVATE_DIR: privateDir,
      UNDERSTAND_BOOK_LIBRARY_ROOT: path.join(directory, 'library'), UNDERSTAND_BOOK_WEB_DIST: live.web_dist,
    });
    result.restarted_reader_url = server.url;
    const state = await server.api('tutor/state');
    const history = await server.api('agent/history');
    save(path.join(directory, 'recovered-state.json'), state);
    save(path.join(directory, 'recovered-history.json'), history);
    check(result, 'tutor_state_identical_after_process_restart', isDeepStrictEqual(state, baseline.state));
    check(result, 'goal_and_enabled_active_session_recovered', state.control.enabled
      && state.control.current_tutor_session_id === item.session_id
      && state.sessions[item.session_id]?.status === 'active'
      && state.sessions[item.session_id]?.user_intent === item.goal,
    { control: state.control, session: state.sessions[item.session_id] });
    check(result, 'previous_material_goal_still_preserved', !!state.sessions[item.previous_session_id]);
    check(result, 'all_recorded_turns_recovered_unchanged', item.turns.every(row =>
      isDeepStrictEqual(history.current?.turns?.find(turn => turn.turn_id === row.descriptor.turn_id), row.turn)),
    item.turns.map(row => ({ turn_id: row.descriptor.turn_id, incomplete: row.turn.outcome?.incomplete })));
    const recovered = learning(memory);
    const targets = baseline.events.filter(event => event.payload?.design?.target || event.payload?.move?.target);
    check(result, 'all_existing_target_events_recovered_unchanged', targets.every(event =>
      isDeepStrictEqual(recovered.events.find(saved => saved.event_id === event.event_id), event)),
    targets.map(event => ({ event_id: event.event_id, kind: event.kind, turn_id: event.binding.turn_id })));
    result.recovered_sources = [];
    for (const row of item.turns) for (const source of row.sources ?? []) {
      const restored = await server.api('agent/source.resolve', { turn_id: row.descriptor.turn_id, source_ref_id: source.source_ref_id }, 'POST');
      check(result, `source_${row.index}_${source.source_ref_id}_recovers_same_target`, restored.stale === false && restored.can_open_in_reader === true
        && isDeepStrictEqual(restored.music?.target ?? restored.media, source.music?.target ?? source.media));
      result.recovered_sources.push(restored);
    }
    save(path.join(directory, 'recovered-sources.json'), result.recovered_sources);
    result.recovery_status = 'passed'; checkpoint();
    await server.stop(); server = null;
  }
  // Startup may resume a queued memory-review job. The local rejection server
  // prevents any external model call; these attempts are evidence, not a data
  // recovery failure and not proof of complete live teaching.
  report.status = 'passed_offline_scope_and_recovery_only';
} catch (error) {
  report.status = 'failed'; report.error = error.stack ?? error.message; process.exitCode = 1;
} finally {
  if (server) await server.stop();
  await new Promise(resolve => deniedProvider.close(resolve));
  report.finished_at = new Date().toISOString(); checkpoint();
  console.log(JSON.stringify({ output, status: report.status, live_acceptance: report.live_acceptance,
    model_calls: report.model_calls, blocked_local_provider_attempts: report.blocked_local_provider_attempts.length,
    cases: report.cases.map(item => ({ kind: item.kind, recovery_status: item.recovery_status,
      complete_teaching_turns: item.complete_teaching_turns, checks: item.checks.map(check => ({ name: check.name, passed: check.passed })) })), error: report.error }));
}
