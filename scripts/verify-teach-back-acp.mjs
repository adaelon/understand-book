// Submit an ordinary or course explanation from a saved Reader session to OpenCode ACP.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { setTimeout as delay } from 'node:timers/promises';
import { parseArgs } from 'node:util';
import { startProviderRecorder, measuredUsage } from '../evals/semantic/provider-recorder.mjs';
import { positiveLimit, selectTrialProvider } from './model-trial-config.mjs';

const { values } = parseArgs({ options: {
  opencode: { type: 'string' }, out: { type: 'string' }, 'course-seed': { type: 'string' },
  retry: { type: 'string' }, 'from-origin': { type: 'string' }, 'max-requests': { type: 'string' },
  reader: { type: 'string', default: process.platform === 'win32' ? 'target/debug/reader.exe' : 'target/debug/reader' },
} });
if (!values.opencode || !values.out || Boolean(values['course-seed']) === Boolean(values['from-origin'])) {
  throw new Error('Specify --opencode <binary>, --out <fresh directory> and either --course-seed or --from-origin.');
}
const course = Boolean(values['course-seed']);
const maxRequests = positiveLimit(values['max-requests'], 24, '--max-requests');
const tokenLimit = 300000;
const out = path.resolve(values.out), seed = path.resolve(values['course-seed'] ?? values['from-origin']), reader = path.resolve(values.reader);
const retry = values.retry ? path.resolve(values.retry) : undefined;
const agent = path.resolve(values.opencode);
const session = JSON.parse(fs.readFileSync(path.join(seed, course ? 'session.json' : 'run.json'), 'utf8'));
const before = course && JSON.parse(fs.readFileSync(path.join(seed, 'before.json'), 'utf8'));
const expression = course
  ? '原来我以为算力翻倍就会快一倍。图里读 70 GB 要 20.90 ms，计算却只要 0.14 ms。只加算力时还是要等读取。改成读 35 GB 后，读取下界是 10.45 ms，还是超过 9 ms。'
  : session.expression;
const write = (file, data) => fs.writeFileSync(path.join(out, file), JSON.stringify(data, null, 2));
if (fs.existsSync('.env')) process.loadEnvFile('.env');
const provider = selectTrialProvider(process.env);
if (provider.direct) throw new Error('Use the course trial environment for the OpenCode recording proxy.');
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.mkdirSync(out);
const profile = path.join(out, 'reader');
fs.mkdirSync(profile);
const seedProfile = retry ? path.join(retry, 'reader') : course ? seed : path.join(seed, 'reader');
for (const name of ['memory', 'private']) {
  fs.cpSync(path.join(seedProfile, name), path.join(profile, name), { recursive: true });
}
const originalBook = course ? path.join(seed, session.source_id) : session.book;
const book = path.join(out, path.basename(originalBook));
fs.mkdirSync(book);
for (const item of fs.readdirSync(originalBook, { withFileTypes: true })) {
  if (item.isFile()) fs.copyFileSync(path.join(originalBook, item.name), path.join(book, item.name));
}
const previousReport = retry && JSON.parse(fs.readFileSync(path.join(retry, 'run.json'), 'utf8'));
const previousRows = retry ? JSON.parse(fs.readFileSync(path.join(retry, 'provider.json'), 'utf8')) : [];
const used = previousReport?.cumulative_budget ?? {
  requests: previousRows.filter(r => !r.blocked).length,
  reported_tokens: previousRows.reduce((sum, r) => sum + (r.usage?.total_tokens ?? 0), 0),
};
const recorder = await startProviderRecorder(provider.base, {
  apiKey: process.env.OPENCODE_API_KEY, tokenLimit: tokenLimit - used.reported_tokens,
  beforeSend: (request, row) => used.requests + row.ordinal > maxRequests || request.model !== provider.model ? 'TRIAL_LIMIT' : null,
});
const env = Object.fromEntries(Object.entries(process.env).filter(([key]) =>
  /^(path|pathext|systemroot|windir|comspec|processor_architecture|number_of_processors|programfiles(?:\(x86\))?|systemdrive|programdata)$/i.test(key)));
for (const key of ['HOME', 'USERPROFILE', 'APPDATA', 'LOCALAPPDATA', 'XDG_STATE_HOME', 'XDG_CACHE_HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME']) {
  env[key] = path.join(out, key === 'USERPROFILE' ? 'home' : key.toLowerCase());
  fs.mkdirSync(env[key], { recursive: true });
}
const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'teach-back-acp-'));
Object.assign(env, { TEMP: scratch, TMP: scratch, NO_BROWSER: '1', UNDERSTAND_BOOK_LOCAL_DATA_DIR: profile,
  UNDERSTAND_BOOK_WEB_DIST: path.resolve('packages/web/dist'), GIT_CEILING_DIRECTORIES: scratch,
  OPENCODE_DISABLE_AUTOUPDATE: '1', OPENCODE_DISABLE_MODELS_FETCH: '1', OPENCODE_DISABLE_SHARE: '1',
  OPENCODE_DISABLE_LSP_DOWNLOAD: '1', OPENCODE_CONFIG_CONTENT: JSON.stringify({
    model: 'trial/' + provider.model, small_model: 'trial/' + provider.model, snapshot: false,
    provider: { trial: { npm: '@ai-sdk/openai-compatible', name: 'Recorded trial',
      options: { baseURL: recorder.url, apiKey: 'local-trial' }, models: { [provider.model]: {
        name: provider.model, ...(provider.model.startsWith('deepseek-') ? { options: { thinking: { type: 'disabled' } } } : {}),
      } } } },
  }) });
const log = fs.openSync(path.join(out, 'reader.log'), 'w');
const host = spawn(reader, ['--local-profile', book], { cwd: out, env, stdio: ['ignore', log, log], windowsHide: true });
const result = { seed, retry, reader, agent, model: provider.model, expression, started: new Date(),
  previous_budget: used, budget_limits: { requests: maxRequests, reported_tokens: tokenLimit } };
async function waitFor(read, ms) {
  const start = Date.now();
  while (Date.now() - start < ms) {
    if (host.exitCode !== null) throw new Error('Local Reader exited. See reader.log.');
    const value = await read();
    if (value) return value;
    await delay(250);
  }
  throw new Error('Timed out waiting for the local Reader.');
}
try {
  const record = path.join(profile, 'reader-host.json');
  const location = await waitFor(() => fs.existsSync(record) && JSON.parse(fs.readFileSync(record, 'utf8')), 25000);
  assert.equal(location.pid, host.pid);
  const api = async (route, body) => {
    const response = await fetch(location.url + '/api/' + route, body === undefined ? {} : {
      method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
    });
    const data = await response.json();
    assert.ok(response.ok, JSON.stringify(data));
    return data;
  };
  await api('local-agent/settings', { action: 'save', agent: { id: 'opencode-trial', name: 'OpenCode', command: agent, args: ['acp'] } });
  const { current } = await api('agent/history');
  assert.equal(current.id, session.session_id);
  const { source_id, source_revision } = await api('tutor/readiness');
  const failed = retry ? current.turns.at(-1) : null;
  if (retry) {
    assert.equal(failed?.status, 'failed');
    assert.equal(failed.user, expression);
  }
  const id = failed?.domain.teach_back_attempts[0].expression_id ?? crypto.randomUUID();
  const submitted = Date.now();
  const origin = course ? { course_context: { run_id: session.run_id, revision: before.progress.revision, surface: 'course' } }
    : { answer_ref: { turn_id: session.origin_turn_id } };
  const accepted = await api('agent/runs', { local_agent_id: 'opencode-trial', message: expression, display_user: expression,
    entry_action: 'explain_back', ...origin,
    session_id: current.id, expected_chat_head: current.turns.at(-1)?.turn_id ?? null, source_id, source_revision, expression_id: id,
    client_request_id: id, operation_id: failed ? crypto.randomUUID() : id, ...(failed ? { retry_of: failed.turn_id } : {}),
    response_language: 'zh-CN' });
  const turn = (accepted.descriptor ?? accepted).turn_id;
  if (failed) assert.equal(turn, failed.turn_id);
  const complete = await waitFor(async () => {
    const run = await api('agent/runs/' + turn);
    return ['saved', 'failed', 'unsaved'].includes(run.persistence_state) && run;
  }, 180000);
  result.submission_to_saved_ms = Date.now() - submitted;
  write('turn.json', complete);
  result.activities = complete.activities.filter(a => a.kind === 'teach_back')
    .map(({ name, duration_ms, status }) => ({ name, duration_ms, status }));
  if (complete.execution_state !== 'completed') {
    const file = 's_' + Buffer.from(session.session_id).toString('base64url') + '.jsonl';
    const log = fs.readFileSync(path.join(profile, 'memory', 'agent-sessions', file), 'utf8');
    const finished = log.trimEnd().split(/\r?\n/).map(JSON.parse)
      .findLast(event => event.kind === 'turn.finished' && event.turn_id === turn);
    throw new Error(JSON.stringify(finished?.payload?.error ?? complete.final_view?.error));
  }
  const turns = (await api('agent/history')).current.turns;
  assert.equal(turns.filter(t => t.user === expression).length, 1);
  const saved = turns.at(-1);
  assert.equal(saved.domain.local_agent.id, 'opencode-trial');
  const card = saved.outcome.answer_view.parts.find(p => p.kind === 'teach_back_comparison')?.comparison;
  assert.ok(card);
  write('card.json', card);
  if (course) {
    const lesson = await api(`book/course/${encodeURIComponent(source_id)}?run_id=${encodeURIComponent(session.run_id)}`);
    write('course.json', lesson);
    assert.equal(lesson.progress.stage, before.progress.stage);
    assert.equal(lesson.round_index, before.round_index);
    const review = lesson.reviews.find(item => item.expression.teach_back?.round_id === card.round_id);
    assert.equal(review.expression.response, expression);
    assert.equal(review.expression.teach_back.session_id, session.session_id);
    assert.equal(review.expression.teach_back.turn_id, turn);
    assert.deepEqual(review.comparison, card);
  }
  result.stages = recorder.records.flatMap(row => {
    if (row.request.messages?.some(message => message.role === 'system' && typeof message.content === 'string'
      && message.content.startsWith('You are a title generator.'))) return [];
    const text = row.request.messages?.at(-1)?.content;
    if (typeof text !== 'string') return [];
    let packet;
    try { packet = JSON.parse(text.slice(text.lastIndexOf('\n\n') + 2)); } catch { return []; }
    const input = packet.input ?? packet;
    if (input.expression === expression) {
      assert.deepEqual(Object.keys(input).sort(), ['expression', 'original_question', 'topic_terms']);
      assert.equal(input.original_question, course ? before.reflection_prompt : session.question);
      assert.ok(!row.request.tools?.some(tool => /understand.book|book_text|reader_history/i.test(tool.function?.name)));
      return [{ stage: 'expression', request: row.ordinal, elapsed_ms: row.elapsed_ms }];
    }
    if (input.evidence && input.frozen_student_graph) {
      assert.deepEqual(input.frozen_student_graph, card.student_graph);
      if (course) {
        assert.equal(input.visible_course_task.displayed_explanation, before.explanation);
        assert.deepEqual(input.visible_course_task.saved_exploration, before.exploration);
      }
      return [{ stage: 'comparison', request: row.ordinal, elapsed_ms: row.elapsed_ms,
        read_ranges: input.evidence.passages.map(({ lid, source_range_utf16 }) => ({ lid, ...source_range_utf16 })) }];
    }
    return [];
  });
  assert.equal(result.stages[0]?.stage, 'expression');
  assert.equal(result.stages.at(-1)?.stage, 'comparison');
  result.status = 'delivered_pending_semantic_review';
} catch (error) {
  result.status = recorder.budgetRejections.length ? 'stopped_at_trial_budget' : 'failed';
  result.error = error.message; process.exitCode = 1;
} finally {
  if (host.exitCode === null) { const exited = once(host, 'close'); host.kill(); await exited; }
  fs.closeSync(log);
  await recorder.stop(30000);
  write('provider.json', recorder.records);
  const sent = recorder.records.filter(r => !r.blocked);
  result.usage = measuredUsage(sent);
  result.cumulative_budget = {
    requests: used.requests + sent.length,
    reported_tokens: used.reported_tokens + sent.reduce((sum, r) => sum + (r.usage?.total_tokens ?? 0), 0),
  };
  result.budget_stops = recorder.budgetRejections;
  result.finished = new Date();
  write('run.json', result);
  console.log(result);
}
