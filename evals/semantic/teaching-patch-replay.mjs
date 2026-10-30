// Fixed-draft revision benchmark. Rebuild the recorded prefix through real tools,
// replay its first factual rejection, then use the real Provider for all repairs.
// --live-first-review measures the initial audit and repairs of that same draft.
// Neither mode measures generation of a complete new answer.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import https from 'node:https';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { createUnzip } from 'node:zlib';
import { createToolUseCapture, measureMessagesInput } from './teaching-patch-stream.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
if (args.includes('--help')) {
  console.log('node evals/semantic/teaching-patch-replay.mjs --server BINARY --out FRESH_DIR [--label baseline|candidate] [--seed OLD_REPLAY_DIR] [--live-first-review] [--prepare-only]');
  process.exit(0);
}
const output = path.resolve(option('--out', 'tmp/teaching-patch-20261029/replay'));
const binary = path.resolve(option('--server', 'tmp/teaching-revision-20261029/server-candidate-v3.exe'));
const seedDirectory = path.resolve(option('--seed', 'tmp/teaching-revision-20261029/candidate-03'));
const prepareOnly = args.includes('--prepare-only');
const liveFirstReview = args.includes('--live-first-review');
const label = option('--label', 'candidate');
assert(!fs.existsSync(output), 'Use a fresh directory; every attempt must remain available.');
assert(fs.existsSync(binary), 'Select the intended frozen server executable.');
if (fs.existsSync(path.join(root, '.env'))) process.loadEnvFile(path.join(root, '.env'));
const upstreamBase = new URL(process.env.OPENCODE_BASE_URL);
assert.equal(upstreamBase.origin, 'https://api.deepseek.com', 'This replay preserves the recorded official DeepSeek Messages route.');
assert.equal(process.env.FLUID_LLM_MODEL, 'deepseek-v4-flash');
assert(process.env.OPENCODE_API_KEY, 'The configured real Provider credentials are required.');
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const seedRun = read(path.join(seedDirectory, 'run.json'));
const seedEvent = read(path.join(seedDirectory, 'teaching-events.private.json')).find(event => event.kind === 'agent_work_reviewed');
assert(seedEvent, 'The seed requires a persisted first-draft review.');
const seed = seedEvent.payload;
assert.equal(seed.review.decision, 'revise');
assert.equal(seed.independent_reviews.explanation.status, 'skipped');
assert(seed.independent_reviews.factual.raw && seed.proposed_delivery.moves.length === 1);
const prefix = [];
for (const call of seedRun.turn.outcome.trace) {
  const arguments_ = JSON.parse(call.args);
  if (call.tool === 'tutor.step' && arguments_.operation === 'select') {
    arguments_.move.delivery_text = seed.proposed_delivery.moves[0].delivery_text;
    arguments_.move.participation = seed.proposed_delivery.moves[0].participation;
  }
  let round = prefix.at(-1);
  if (!round || round.original_loop !== call.model_tool_loop) {
    round = { original_loop: call.model_tool_loop, calls: [] }; prefix.push(round);
  }
  round.calls.push({ name: call.tool.replaceAll('.', '_'), arguments: arguments_ });
  if (call.tool === 'tutor.step' && arguments_.operation === 'select') break;
}
assert.equal(prefix.at(-1).calls.at(-1).arguments.operation, 'select');
// Resume the actual recorded Provider continuation. Native thinking blocks are
// opaque protocol state: preserve them unchanged, never invent or print them.
const sessionDirectory = path.join(seedDirectory, 'memory', 'agent-sessions');
const sessionFiles = fs.readdirSync(sessionDirectory).filter(name => name.endsWith('.jsonl'));
assert.equal(sessionFiles.length, 1, 'The isolated seed must identify one original session log.');
const sessionFile = path.join(sessionDirectory, sessionFiles[0]);
const originalMessages = fs.readFileSync(sessionFile, 'utf8').trim().split(/\r?\n/).map(JSON.parse)
  .flatMap(event => event.payload?.messages ?? []).filter(message => message.role === 'Assistant' && message.tool_calls?.length);
const continuations = prefix.map((round, index) => {
  const originalMessage = originalMessages[index];
  const state = originalMessage?.provider_continuation;
  const offsets = { text: 0, thinking: 0 };
  const sources = { text: Buffer.from(originalMessage.content ?? ''), thinking: Buffer.from(state?.reasoning_content ?? '') };
  // Mirror runtime replay_blocks: persisted blocks store byte lengths separately
  // from their exact concatenated content, retaining all other native metadata.
  const blocks = state?.messages_content?.map(block => {
    const expanded = { ...block };
    for (const field of ['text', 'thinking']) {
      const lengthKey = `${field}_bytes`;
      if (block[lengthKey] == null) continue;
      const end = offsets[field] + block[lengthKey];
      assert(end <= sources[field].length, 'Recorded continuation byte range is invalid.');
      expanded[field] = sources[field].subarray(offsets[field], end).toString('utf8');
      delete expanded[lengthKey]; offsets[field] = end;
    }
    return expanded;
  });
  const restoredThinking = blocks?.filter(block => block.type === 'thinking').map(block => block.thinking ?? '').join('');
  assert(restoredThinking === state.reasoning_content, 'Native thinking continuation must be restored exactly.');
  assert(blocks?.some(block => block.type === 'thinking'), 'Recorded native continuation is required for thinking-mode tool history.');
  const toolBlocks = blocks.filter(block => block.type === 'tool_use');
  const inputs = new Map();
  assert.equal(toolBlocks.length, round.calls.length);
  for (const [callIndex, call] of round.calls.entries()) {
    assert.equal(toolBlocks[callIndex].name, call.name);
    const originalCall = originalMessage.tool_calls.find(item => item.id === toolBlocks[callIndex].id);
    assert(originalCall, 'Native continuation tool id must match its persisted arguments.');
    const originalArguments = JSON.parse(originalCall.arguments);
    if (call.name === 'tutor_step' && originalArguments.operation === 'select') {
      originalArguments.move.delivery_text = seed.proposed_delivery.moves[0].delivery_text;
      originalArguments.move.participation = seed.proposed_delivery.moves[0].participation;
    }
    assert.deepEqual(originalArguments, call.arguments, 'Recorded continuation tools differ from the frozen prefix.');
    inputs.set(originalCall.id, originalArguments);
  }
  return blocks.map(block => block.type === 'tool_use'
    ? { ...block, input: inputs.get(block.id) } : block);
});
fs.mkdirSync(output, { recursive: true });
const save = (name, value) => fs.writeFileSync(path.join(output, name), JSON.stringify(value, null, 2) + '\n');
save('frozen-prefix.private.json', { original_run: seedRun.descriptor, prefix,
  source_evidence: seed.source_evidence, draft: seed.proposed_delivery.text,
  first_factual_review: seed.independent_reviews.factual.raw, confirmed_preferences: seed.frozen_input.confirmed_preferences });
fs.writeFileSync(path.join(output, 'frozen-draft.md'), seed.proposed_delivery.text, 'utf8');
const report = { protocol: 'teaching-fixed-draft-revision.v1', label, started_at: new Date().toISOString(),
  status: 'preparing', binary, seed_directory: seedDirectory, prepare_only: prepareOnly,
  live_first_review: liveFirstReview,
  measurement_start: liveFirstReview ? 'first_actual_factual_review' : 'first_author_after_frozen_factual_rejection',
  original_question: seed.original_request, model: process.env.FLUID_LLM_MODEL,
  source_revision: seedRun.source_revision, prefix_rounds: prefix.length, provider_requests: [],
  interpretation: liveFirstReview
    ? 'Identical recorded first draft, actual source-read tool prefix and preferences. The first factual review and every subsequent author/review call use the real Provider. Timing and usage include the first factual review; the original historical review is archived but never injected. Prefix time and fixture author calls are excluded. This is initial audit plus revision latency, not fresh end-to-end question latency or the earlier repair-only boundary.'
    : 'Identical recorded first draft, actual source-read tool prefix and first factual feedback. The first factual verdict is a fixture; all subsequent author/review calls use the real Provider. Prefix time and fixture calls are excluded from repair metrics. This is not a fresh end-to-end question latency measurement.',
  transport: 'DeepSeek Messages; request bodies and streamed response bytes forwarded unchanged after the fixed prefix. The localhost model listing projects the official alias onto the exact configured model id so runtime transport capability discovery stays equivalent.',
  background_control: 'Automatic coach/profile jobs receive an unavailable response in both variants, keeping the recorded no-coach repair context fixed and preventing unrelated paid work. Only foreground author and acceptance reviews are measured.',
  native_continuation: { source: sessionFile, rounds: continuations.length, original_blocks_preserved: true, reasoning_text_in_proxy_archives: false },
};
const checkpoint = () => save('comparison-run.json', report);
let prefixIndex = 0, frozenReviewSent = false, barrierReached = false, child, proxy;
let observedTurnId, observedSessionId;
let cancelledForPreparation = false;
let incomingRequestIndex = 0;
const activeUpstreams = new Set();
const textOf = content => typeof content === 'string' ? content : (content ?? []).filter(item => item.type === 'text').map(item => item.text).join('\n');
const systemOf = request => textOf(request.system) || (request.messages ?? []).filter(item => ['system', 'developer'].includes(item.role)).map(item => textOf(item.content)).join('\n');
function purpose(request) {
  const system = systemOf(request);
  if (system.includes('Teaching delivery acceptance review.')) return 'tutor_delivery_review';
  if (system.includes('Teaching explanation acceptance review.')) return 'tutor_explanation_review';
  if (system.includes('You are the resident reading agent for the current book.')) return 'outer';
  if (system.startsWith('你是 Understand Book 的教学教练。')) return 'tutor_coach';
  if (system.startsWith('You extract durable reader-profile candidates from resident user turns.')) return 'background_profile_review';
  return 'other';
}
function localIds(request) {
  const text = systemOf(request) + '\n' + (request.messages ?? []).map(item => textOf(item.content)).join('\n');
  const contextLine = text.split('\n').find(line => line.startsWith('Tutor input context ('));
  if (contextLine) {
    const context = JSON.parse(contextLine.slice(contextLine.indexOf(': ') + 2));
    observedTurnId ??= context.binding.turn_id;
    observedSessionId ??= context.binding.chat_session_id;
  }
  assert(observedTurnId && observedSessionId, 'The fixed prefix must bind to the actual isolated teaching turn.');
}
function remap(value) {
  return JSON.parse(JSON.stringify(value)
    .replaceAll(seedRun.descriptor.turn_id, observedTurnId)
    .replaceAll(seedRun.descriptor.session_id, observedSessionId));
}
function nativeMessage(content, stopReason) {
  return { id: `fixed-prefix-${report.provider_requests.length}`, type: 'message', role: 'assistant',
    model: process.env.FLUID_LLM_MODEL, content, stop_reason: stopReason, stop_sequence: null,
    usage: { input_tokens: 0, output_tokens: 0, cache_read_input_tokens: 0, cache_creation_input_tokens: 0 } };
}
function sendJson(res, value, status = 200) {
  res.writeHead(status, { 'content-type': 'application/json' }); res.end(JSON.stringify(value));
}
function archiveRequest(request) {
  // Preserve actual tool arguments/receipts and review inputs, not provider thinking text.
  return { ...request, messages: request.messages?.map(message => ({ ...message,
    ...(Array.isArray(message.content) ? { content: message.content.filter(block => !['thinking', 'redacted_thinking'].includes(block.type)) } : {}) })) };
}
async function cancelPreparedTurn() {
  if (cancelledForPreparation) return;
  cancelledForPreparation = true;
  const inner = read(path.join(output, 'reader', 'run.json'));
  await fetch(new URL(`/api/agent/runs/${inner.descriptor.turn_id}/cancel`, inner.reader_url),
    { method: 'POST', headers: { 'content-type': 'application/json' }, body: '{}' });
}
function markMeasurementStart(row) {
  barrierReached = true;
  report.repair_started_at = row.started_at;
  report.first_live_request_ordinal = row.ordinal;
  console.log(JSON.stringify({ event: 'measurement-start', measurement_start: report.measurement_start,
    prefix_verified: report.prefix_verified, prepare_only: prepareOnly }));
}
async function prepareOrForward(req, res, body, row, trace) {
  if (prepareOnly) {
    trace.phase = 'cancel_prepared_turn';
    row.preparation_barrier = true;
    await cancelPreparedTurn();
    sendJson(res, { error: { message: 'FIXED_PREFIX_PREPARED_NO_MODEL_CALL' } }, 499);
  } else {
    row.forwarded = true;
    console.log(JSON.stringify({ event: 'provider-request', ordinal: row.ordinal, purpose: row.purpose }));
    await forward(req, res, body, row, trace);
  }
}
async function loadListing() {
  const response = await fetch(new URL('/models', upstreamBase), {
    headers: { authorization: `Bearer ${process.env.OPENCODE_API_KEY}` }, signal: AbortSignal.timeout(15000),
  });
  assert(response.ok, `Provider model capability discovery failed (${response.status}).`);
  const listing = await response.json();
  const model = process.env.FLUID_LLM_MODEL;
  const exact = listing.data?.find(item => item.id === model);
  const resolved = exact ?? listing.data?.find(item => item.id === 'deepseek-flash');
  assert(resolved?.api_capabilities?.anthropic_messages, 'The recorded model must support Messages on the configured official route.');
  const projected = exact ? listing : { ...listing, data: [...listing.data, { ...resolved, id: model }] };
  report.model_listing = { configured_model: model, official_resolved_model: resolved.id,
    projected_alias: !exact, api_capabilities: resolved.api_capabilities };
  checkpoint();
  return projected;
}
function forward(req, res, body, row, trace) {
  return new Promise((resolve, reject) => {
    const target = new URL(req.url === '/messages' ? '/anthropic/v1/messages' : '/chat/completions', upstreamBase);
    const started = performance.now();
    trace.phase = 'upstream_connecting';
    row.forward_started_at = new Date().toISOString();
    const upstream = (target.protocol === 'https:' ? https : http).request(target,
      { method: req.method, headers: { ...req.headers, host: target.host } }, incoming => {
        trace.phase = 'response_stream';
        row.response_headers_at = new Date().toISOString();
        row.http_status = incoming.statusCode;
        const capture = createToolUseCapture(incoming.headers['content-type']);
        const observed = ['gzip', 'deflate'].includes(incoming.headers['content-encoding']) ? incoming.pipe(createUnzip()) : incoming;
        observed.on('data', chunk => capture.write(chunk));
        res.writeHead(incoming.statusCode, incoming.headers);
        incoming.pipe(res);
        observed.once('end', () => {
          row.response_end_at = new Date().toISOString();
          trace.phase = 'response_complete';
          const captured = capture.finish();
          row.tool_calls = captured.calls;
          if (captured.usage) row.native_usage = captured.usage;
          if (captured.errors.length) row.tool_capture_errors = captured.errors;
          if (captured.providerError) row.provider_error = captured.providerError;
          row.elapsed_ms = performance.now() - started;
          activeUpstreams.delete(upstream); checkpoint(); resolve();
        });
        observed.once('error', error => { trace.error_origin = 'observed_response'; reject(error); });
        incoming.once('error', error => { trace.error_origin = 'upstream_response'; reject(error); });
      });
    activeUpstreams.add(upstream);
    upstream.once('error', error => { trace.error_origin = 'upstream_request'; activeUpstreams.delete(upstream); reject(error); });
    upstream.end(body);
  });
}
function deriveRepair() {
  const innerPath = path.join(output, 'reader', 'run.json');
  if (!fs.existsSync(innerPath)) return;
  const inner = read(innerPath);
  report.reader_status = inner.status;
  report.reader_completed_at = inner.completed_at;
  report.reader_finished_at = inner.finished_at;
  const activities = inner.snapshot?.activities ?? [];
  const firstFact = activities.find(item => item.kind === 'model' && item.name === 'tutor_delivery_review');
  const firstLive = liveFirstReview ? firstFact
    : activities.find(item => item.kind === 'model' && item.name === 'outer' && item.step_id > (firstFact?.step_id ?? Infinity));
  if (!firstLive) return;
  const fromStep = firstLive.step_id;
  const included = activities.filter(item => item.step_id >= fromStep);
  const models = included.filter(item => item.kind === 'model' && !['tutor_coach', 'background_profile_review'].includes(item.name));
  report.excluded_background_activities = included.filter(item => item.kind === 'model' && ['tutor_coach', 'background_profile_review'].includes(item.name));
  const tools = included.filter(item => item.kind === 'tool');
  // The fixed select starts before its nested first audit. Include its outcome
  // for live-audit rejection counts without treating it as a paid author edit.
  const initialSelect = liveFirstReview ? activities.findLast(item => item.kind === 'tool'
    && item.name === 'tutor.step' && item.step_id < fromStep) : undefined;
  const outcomeTools = initialSelect ? [initialSelect, ...tools] : tools;
  const failed = outcomeTools.filter(item => ['failed', 'rejected'].includes(item.status));
  const events = fs.readFileSync(path.join(output, 'reader', 'events.private.jsonl'), 'utf8').trim().split('\n').filter(Boolean).map(JSON.parse);
  const terminal = events.find(event => ['run.completed', 'run.failed', 'run.cancelled', 'run.persistence_failed'].includes(event.type));
  const firstAnswer = events.find(event => event.type === 'answer.patch' && event.payload?.view != null);
  const authorCalls = report.provider_requests.filter(item => item.forwarded && item.purpose === 'outer')
    .flatMap(item => (item.tool_calls ?? []).map(call => ({ provider_request: item.ordinal, ...call })));
  const teachingCalls = authorCalls.filter(item => ['tutor_step', 'tutor.step'].includes(item.name));
  const teachingActivities = tools.filter(item => item.name === 'tutor.step');
  report.author_teaching_operations = teachingCalls.map((call, index) => ({
    provider_request: call.provider_request, id: call.id,
    operation: call.arguments?.operation, edits: call.arguments?.edits,
    delivery_text: call.arguments?.move?.delivery_text,
    ...(teachingCalls.length === teachingActivities.length ? {
      tool_step_id: teachingActivities[index].step_id, status: teachingActivities[index].status,
      error_code: teachingActivities[index].error_code,
    } : {}),
  }));
  const submissions = report.author_teaching_operations.filter(item => ['select', 'revise'].includes(item.operation));
  const usageTotal = field => models.every(item => item.usage?.[field] != null)
    ? models.reduce((sum, item) => sum + item.usage[field], 0) : null;
  const inputTokens = usageTotal('input_tokens');
  const cachedTokens = usageTotal('cached_input_tokens');
  const createdTokens = usageTotal('cache_creation_input_tokens');
  const forwarded = report.provider_requests.filter(item => item.forwarded);
  report.request_usage_alignment = { model_steps: models.length, forwarded_requests: forwarded.length,
    matched: models.length === forwarded.length, method: 'Same sequential foreground request order; background calls are excluded.' };
  if (models.length === forwarded.length) {
    for (const [index, item] of forwarded.entries()) {
      const model = models[index];
      item.model_step_id = model.step_id;
      item.model_activity_purpose = model.name;
      item.normalized_usage = model.usage;
      if (item.native_usage) {
        const native = item.native_usage;
        item.native_usage_matches_activity = native.input_tokens + native.cache_read_input_tokens + native.cache_creation_input_tokens === model.usage?.input_tokens
          && native.cache_read_input_tokens === model.usage?.cached_input_tokens
          && native.output_tokens === model.usage?.output_tokens;
      }
    }
  }
  report.repair_metrics = {
    start_step_id: fromStep, start_elapsed_ms: firstLive.started_ms,
    total_ms: terminal?.elapsed_ms != null ? terminal.elapsed_ms - firstLive.started_ms : null,
    first_answer_ms: firstAnswer?.elapsed_ms != null ? firstAnswer.elapsed_ms - firstLive.started_ms : null,
    real_model_calls: models.length,
    model_purposes: Object.fromEntries([...new Set(models.map(item => item.name))].map(name => [name, models.filter(item => item.name === name).length])),
    model_seconds: Object.fromEntries([...new Set(models.map(item => item.name))].map(name => [name, models.filter(item => item.name === name).reduce((sum, item) => sum + (item.duration_ms ?? 0), 0) / 1000])),
    real_provider_requests: report.provider_requests.filter(item => item.forwarded).length,
    provider_wait_seconds: report.provider_requests.filter(item => item.forwarded).reduce((sum, item) => sum + (item.elapsed_ms ?? 0), 0) / 1000,
    author_submission_attempts: submissions.length,
    full_select_attempts: submissions.filter(item => item.operation === 'select').length,
    body_patch_attempts: submissions.filter(item => item.operation === 'revise' && item.edits?.some(edit => edit.field === 'delivery_text')).length,
    invitation_only_patch_attempts: submissions.filter(item => item.operation === 'revise' && item.edits?.length > 0 && item.edits.every(edit => edit.field === 'public_prompt')).length,
    evidence_only_resubmission_attempts: submissions.filter(item => item.operation === 'revise' && item.edits?.length === 0).length,
    tool_capture_complete: !report.provider_requests.some(item => item.tool_capture_errors?.length) && teachingCalls.length === teachingActivities.length,
    failed_or_rejected_tools: failed.length,
    errors_by_code: Object.fromEntries([...new Set(failed.map(item => item.error_code))].map(code => [code, failed.filter(item => item.error_code === code).length])),
    teaching_revision_rejections: failed.filter(item => item.name === 'tutor.step' && item.error_code === 'TEACHING_INVALID').length,
    initial_audit_rejected: liveFirstReview ? initialSelect?.error_code === 'TEACHING_INVALID' : null,
    post_initial_audit_rejections: tools.filter(item => ['failed', 'rejected'].includes(item.status)
      && item.name === 'tutor.step' && item.error_code === 'TEACHING_INVALID').length,
    tutor_argument_failures: failed.filter(item => item.name === 'tutor.step' && item.error_code === 'TOOL_ARGUMENTS_INVALID').length,
    final_answer_chars: [...(inner.turn?.outcome?.answer ?? '')].length,
    total_tokens: models.every(item => item.usage?.total_tokens != null) ? models.reduce((sum, item) => sum + item.usage.total_tokens, 0) : null,
    input_tokens: inputTokens,
    cached_input_tokens: cachedTokens,
    cache_creation_input_tokens: createdTokens,
    uncached_input_tokens: [inputTokens, cachedTokens, createdTokens].every(value => value != null) ? inputTokens - cachedTokens - createdTokens : null,
    output_tokens: usageTotal('output_tokens'),
    request_body_bytes: forwarded.reduce((sum, item) => sum + item.input_footprint.request_body_bytes, 0),
    history_system_text_bytes: forwarded.reduce((sum, item) => sum + item.input_footprint.history_system_text_bytes, 0),
    first_author_input_footprint: forwarded.find(item => item.purpose === 'outer')?.input_footprint,
  };
  const learningFile = path.join(output, 'reader', 'teaching-events.private.json');
  if (fs.existsSync(learningFile)) {
    const reviews = read(learningFile).filter(event => event.kind === 'agent_work_reviewed');
    report.repair_metrics.revised_draft_reviews = Math.max(0, reviews.length - 1);
    report.repair_metrics.actual_body_revision_rounds = reviews.slice(1).filter((event, index) => event.payload.proposed_delivery.text !== reviews[index].payload.proposed_delivery.text).length;
    report.repair_metrics.unchanged_body_review_rounds = reviews.slice(1).filter((event, index) => event.payload.proposed_delivery.text === reviews[index].payload.proposed_delivery.text).length;
    report.reviews = reviews.map((event, index) => ({ round: event.payload.round, scripted_first_feedback: !liveFirstReview && index === 0,
      decision: event.payload.review.decision, findings: event.payload.review.findings,
      draft_chars: [...event.payload.proposed_delivery.text].length,
      explanation_status: event.payload.independent_reviews.explanation.status ?? event.payload.independent_reviews.explanation.review?.decision }));
  }
}

try {
  const listing = await loadListing();
  proxy = http.createServer(async (req, res) => {
    const trace = { incoming_request_id: ++incomingRequestIndex, received_at: new Date().toISOString(),
      request_ordinal: null, path: req.url, method: req.method, phase: 'request_validation' };
    try {
      if (req.method === 'GET' && req.url === '/models') { sendJson(res, listing); return; }
      assert.equal(req.method, 'POST');
      assert(['/messages', '/chat/completions'].includes(req.url), `Unexpected Provider path ${req.url}`);
      trace.phase = 'read_request_body';
      const chunks = []; for await (const chunk of req) chunks.push(chunk);
      const body = Buffer.concat(chunks);
      trace.phase = 'parse_request_body';
      const request = JSON.parse(body.toString('utf8'));
      const row = { ordinal: report.provider_requests.length + 1, purpose: purpose(request),
        incoming_request_id: trace.incoming_request_id, received_at: trace.received_at,
        started_at: new Date().toISOString(), model: request.model, path: req.url, scripted: false,
        input_footprint: measureMessagesInput(request, body.length) };
      report.provider_requests.push(row);
      trace.request_ordinal = row.ordinal;
      trace.purpose = row.purpose;
      trace.phase = 'route_request';
      if (['tutor_coach', 'background_profile_review'].includes(row.purpose)) {
        row.background_suppressed = true;
        sendJson(res, { error: { message: 'FIXED_REVISION_BENCHMARK_BACKGROUND_UNAVAILABLE' } }, 503);
        checkpoint(); return;
      }
      if (['outer', 'tutor_delivery_review', 'tutor_explanation_review'].includes(row.purpose)) {
        assert.equal(req.url, '/messages', 'Measured author and review calls must retain Messages transport.');
      }
      save(`request-${String(row.ordinal).padStart(2, '0')}.private.json`, archiveRequest(request));
      if (!frozenReviewSent && row.purpose === 'outer') {
        localIds(request);
        assert(prefixIndex < prefix.length, 'Prefix exhausted before reaching its factual review.');
        const roundIndex = prefixIndex++;
        const round = prefix[roundIndex];
        row.scripted = true; row.original_loop = round.original_loop;
        sendJson(res, nativeMessage(continuations[roundIndex].map(block => block.type === 'tool_use'
          ? { ...block, input: remap(block.input) } : block), 'tool_use'));
      } else if (!frozenReviewSent && row.purpose === 'tutor_delivery_review') {
        assert.equal(prefixIndex, prefix.length);
        const packet = JSON.parse(textOf(request.messages.filter(message => message.role === 'user').at(-1).content));
        save('first-review-input.private.json', packet);
        assert.equal(packet.proposed_delivery.text, seed.proposed_delivery.text, 'Rebuilt first draft differs from the recorded rejected draft.');
        assert.deepEqual(packet.current_source_evidence, seed.source_evidence, 'Rebuilt first-review evidence differs from the frozen evidence.');
        assert.deepEqual(packet.frozen_input.confirmed_preferences, remap(seed.frozen_input.confirmed_preferences), 'Reader preferences changed across the fixed prefix.');
        report.prefix_verified = { draft_equal: true, source_evidence_equal: true, confirmed_preferences_equal: true,
          source_segments: seed.source_evidence.reduce((sum, source) => sum + source.segments.length, 0),
          findings: seed.independent_reviews.factual.raw.findings.length };
        report.initial_review_request_ordinal = row.ordinal;
        frozenReviewSent = true;
        if (liveFirstReview) {
          row.live_initial_review = true;
          markMeasurementStart(row);
          await prepareOrForward(req, res, body, row, trace);
        } else {
          row.scripted = true; row.frozen_first_feedback = true;
          sendJson(res, nativeMessage([{ type: 'text', text: JSON.stringify(remap(seed.independent_reviews.factual.raw)) }], 'end_turn'));
        }
      } else {
        assert(frozenReviewSent, `Unexpected ${row.purpose} request during the fixed prefix.`);
        if (!barrierReached) {
          assert.equal(row.purpose, 'outer', 'Repair must begin with the author after the frozen rejection.');
          markMeasurementStart(row);
        }
        await prepareOrForward(req, res, body, row, trace);
      }
      checkpoint();
    } catch (error) {
      (report.proxy_errors ??= []).push({ ...trace, occurred_at: new Date().toISOString(),
        message: error.message, code: error.code ?? null, request_complete: req.complete,
        request_destroyed: req.destroyed, response_headers_sent: res.headersSent,
        response_ended: res.writableEnded, child_exited_at: report.child_exited_at ?? null,
        cleanup_started_at: report.cleanup_started_at ?? null });
      report.proxy_error = error.message; checkpoint();
      if (!res.headersSent) sendJson(res, { error: { message: `Fixed-prefix replay: ${error.message}` } }, 502);
      else res.destroy();
      try { await cancelPreparedTurn(); } catch {}
    }
  });
  await new Promise(resolve => proxy.listen(0, '127.0.0.1', resolve));
  const proxyUrl = `http://127.0.0.1:${proxy.address().port}`;
  report.status = 'running'; checkpoint();
  const log = fs.createWriteStream(path.join(output, 'reader-output.log'));
  child = spawn(process.execPath, [path.join(root, 'evals/semantic/teaching-revision-replay.mjs'),
    '--server', binary, '--out', path.join(output, 'reader'), '--label', label,
    '--turn-timeout-ms', option('--turn-timeout-ms', '900000')], {
    cwd: root, windowsHide: true, env: { ...process.env, OPENCODE_BASE_URL: proxyUrl }, stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.stdout.on('data', chunk => { process.stdout.write(chunk); log.write(chunk); });
  child.stderr.on('data', chunk => { process.stderr.write(chunk); log.write(chunk); });
  const exitCode = await new Promise((resolve, reject) => { child.once('error', reject); child.once('exit', resolve); });
  report.child_exited_at = new Date().toISOString();
  log.end();
  deriveRepair();
  assert(!report.proxy_error, report.proxy_error);
  assert(report.prefix_verified && barrierReached, 'The fixed repair starting point was not reached.');
  if (prepareOnly) {
    report.preparation_observation_metrics = report.repair_metrics;
    delete report.repair_metrics;
    report.status = 'prepared_without_paid_model_calls';
  }
  else {
    assert.equal(exitCode, 0, 'The actual revision replay failed; retain this attempt for diagnosis.');
    assert.equal(report.reader_status, 'completed');
    report.status = 'completed';
  }
} catch (error) {
  report.status = 'failed'; report.error = error.message; console.error(error.message); process.exitCode = 1;
} finally {
  report.cleanup_started_at = new Date().toISOString();
  if (child && child.exitCode === null) child.kill();
  for (const upstream of activeUpstreams) upstream.destroy();
  if (proxy) await new Promise(resolve => { proxy.closeAllConnections(); proxy.close(resolve); });
  report.finished_at = new Date().toISOString(); checkpoint();
  console.log(JSON.stringify({ status: report.status, output, error: report.error, metrics: report.repair_metrics }));
}
