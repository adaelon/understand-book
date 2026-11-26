// Restart a real Reader from the frozen chat matching --source and replay the
// recorded public responses through its first length-limited business turn.
// --response-file substitutes one provider response for the scripted write.
// --expect-output-limit verifies a second recorded cutoff and its saved failure response.
// --replay-saved-page-fallback pairs the recorded budget stop and old auxiliary
// placeholder with the full captured action trajectory, without external calls.
// --live-recovery-base sends all business samplings from recovery onward to an
// independently budgeted local recorder, preserving provider wire responses.
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import assert from 'node:assert/strict';
import { startServer } from './server.mjs';
import { requestPurpose, startProviderRecorder } from './provider-recorder.mjs';
import { captureDeliveredPage } from './long-conversation-delivery.mjs';
import { recordedActions } from './long-conversation-recorded-actions.mjs';

const args = process.argv.slice(2);
const option = name => args.includes(name) ? args[args.indexOf(name) + 1] : undefined;
const root = process.cwd();
const seeds = path.resolve(option('--seeds') ?? 'docs/evidence/long-conversation-seeds-20261119/run03');
const source = path.resolve(option('--source') ?? 'docs/evidence/long-dialogue-20261119/paired-live-02/01-short-deepseek-flash-1');
const out = path.resolve(option('--out') ?? (() => { throw Error('Supply an unused --out directory'); })());
const reader = option('--reader') ?? process.env.SEMANTIC_EVAL_SERVER ??
  path.join('target', 'debug', process.platform === 'win32' ? 'reader.exe' : 'reader');
process.env.SEMANTIC_EVAL_SERVER = path.resolve(reader);
const responseFile = option('--response-file');
const liveBase = option('--live-recovery-base');
const actionDir = option('--replay-actions');
const actionCount = option('--replay-count');
const actions = actionDir && recordedActions(path.resolve(actionDir), actionCount === undefined ? undefined : Number(actionCount));
if (actions) assert.ok(!responseFile, 'A complete action trajectory already supplies the write');
const nextReply = option('--resume-response-file');
if (nextReply) assert.ok(actions && !liveBase, 'Supply the action prefix for the saved next model decision');
const expectOutputLimit = args.includes('--expect-output-limit');
if (expectOutputLimit) assert.ok(responseFile, '--expect-output-limit requires a recorded --response-file');
if (liveBase) {
  assert.ok(!responseFile, 'Choose a live recorder or a saved recovery response');
  const url = new URL(liveBase);
  assert.ok(url.protocol === 'http:' && ['127.0.0.1','localhost','[::1]'].includes(url.hostname), 'Supply the local budget recorder URL');
}
const read = name => JSON.parse(fs.readFileSync(name, 'utf8'));
let savedPageFailure;
if (args.includes('--replay-saved-page-fallback')) {
  assert.ok(actions && !liveBase && !actionCount, 'Replay the complete recorded trajectory through its provider stop');
  const stop = read(path.join(actionDir, 'forwarded-provider.json')).at(-1);
  assert.equal(stop.status, 402);
  assert.equal(stop.provider_error, 'TASK_SEND_LIMIT');
  const old = read(path.join(actionDir, 'outcome.json'));
  const placeholder = old.answer_view.parts.find(p => p.kind === 'markdown' && /^\s*\{\s*\}\s*$/.test(p.text));
  assert.ok(old.incomplete && old.warning === 'PROVIDER_ERROR' && placeholder, 'Expected the original failed auxiliary prose');
  savedPageFailure = { status: stop.status, reason: stop.provider_error, auxiliary_fixture_text: placeholder.text };
}
fs.mkdirSync(out);
const save = (name, data) => fs.writeFileSync(path.join(out, name), JSON.stringify(data, null, 2) + '\n');
const records = read(path.join(source, 'provider.json'));
const sample = read(path.join(source, 'sample.json'));
assert.ok(['short','long'].includes(sample.band), 'Expected a frozen short or long sample');
const original = records.filter(row => row.purpose === 'business_loop').slice(0, 3);
assert.deepEqual(original.map(row => row.response.choices[0].finish_reason), ['tool_calls', 'tool_calls', 'length']);
const recorded = original.map(row => row.response.choices[0]);
const cutoffCalls = recorded[2].message.tool_calls ?? [];
const message = '按之前的约定整理。';
const marker = 'presentation_output_recovery.v1';
const prepared = JSON.parse(recorded[1].message.tool_calls[0].function.arguments);
assert.equal(prepared.operation, 'prepare');
const events = [];
const summary = { mode: responseFile ? 'one supplied provider reply, controlled continuation' : 'controlled replay',
  source, reader: process.env.SEMANTIC_EVAL_SERVER, reader_modified: fs.statSync(reader).mtime.toISOString(), seeds,
  original_business_ordinals: original.map(row => row.ordinal), band: sample.band,
  original_first_input_tokens: sample.first_input, original_cutoff_input_tokens: original[2].usage?.prompt_tokens,
  cutoff_call_fragments: cutoffCalls.map(c => ({ id: c.id, tool: c.function.name, argument_characters: [...c.function.arguments].length })),
  actual_external_requests: 0, events };
if (liveBase) {
  summary.mode = 'recorded prefix, all subsequent business samplings through independent provider recorder';
  delete summary.actual_external_requests;
}
if (actions) {
  summary.mode = liveBase ? 'recorded action prefix, model continuation' : 'recorded model actions, controlled completion';
  summary.rebuilt = actions.proof;
  if (nextReply) {
    summary.mode = 'recorded action prefix, one saved next model decision, controlled completion';
    summary.next_response = path.resolve(nextReply);
  }
}
if (savedPageFailure) {
  summary.mode = 'recorded actions, recorded budget stop, historical auxiliary fixture';
  summary.replayed_failure = savedPageFailure;
}
if (expectOutputLimit) {
  summary.expected_terminal = 'MODEL_OUTPUT_LIMIT';
  summary.second_response = path.resolve(responseFile);
}
const viewports = [
  { width: 320, height: 420, input: 'touch' }, { width: 640, height: 240, input: 'touch' },
  { width: 960, height: 720, input: 'mouse' },
];
const answer = (text = '') => ({ index: 0, finish_reason: 'stop', message: { role: 'assistant', content: text } });
const toolCall = (id, fields) => ({ index: 0, finish_reason: 'tool_calls', message: { role: 'assistant', content: '',
  tool_calls: [{ type: 'function', id, function: { name: 'presentation_author', arguments: JSON.stringify(fields) } }] } });

function controlledWrite() {
  const text = '第一步到 0.04。起点是 0，最低点在 2；初始斜率是 −4。向右走 0.04 后，离最低点还剩 1.96。';
  const formula = 'w₁ = w₀ − η × 2(w₀ − 2)，η = 0.01。';
  return toolCall('replay_write', { operation: 'write', new_object: true, title: '梯度下降的第一步',
    html: '<style>main{max-width:34em;margin:auto;padding:12px;color:#18333a;line-height:1.6}h1{font-size:1.3em}h2{font-size:1.05em}strong{color:#006b66;font-size:1.7em}</style>'
      + '<main><h1>梯度下降的第一步</h1><strong>0.04</strong><p>' + text + '</p><h2>更新公式</h2><p>' + formula
      + '</p><p>越接近 2，斜率的绝对值越小，每一步也越短。</p></main>', readable_content: text + ' ' + formula
      + ' 越接近 2，斜率的绝对值越小，每一步也越短。' });
}

function toolReceipt(body, id) {
  // Reader can also project a completed call as a quoted historical record
  // when the provider replay contains no private thinking block.
  const header = 'Historical conversation record (context only, not a request to execute tools):\n';
  const result = body.messages.map(m => {
    if (m.role === 'user' && typeof m.content === 'string' && m.content.startsWith(header))
      return JSON.parse(m.content.slice(header.length));
    return m;
  }).findLast(m => m.role.toLowerCase() === 'tool' && m.tool_call_id === id);
  assert.ok(result, 'Missing product receipt for ' + id);
  const parsed = JSON.parse(result.content);
  return parsed.model_body ?? parsed.body ?? parsed;
}

function publicRequest(body) {
  return { ...body, messages: body.messages.map(({ reasoning_content, ...publicMessage }) => publicMessage) };
}

let step = 0, candidate, lastCall, preview = 0, delivered = false, scriptError, fallbackCalls = 0;
const lastId = choice => choice.message.tool_calls?.at(-1)?.id;
function next(body) {
  const purpose = requestPurpose(body);
  if (purpose !== 'business_loop') {
    if (body.messages[0]?.content?.startsWith?.('available_answer.v2')) {
      // No model supplied new prose for these receipts. Only the regression
      // replay reproduces the old controlled response to test its rejection.
      fallbackCalls++;
      return answer(savedPageFailure?.auxiliary_fixture_text);
    }
    const recordedAux = records.find(r => r.purpose === purpose &&
      r.request.messages[0]?.content === body.messages[0]?.content);
    return recordedAux?.response.choices[0] ?? answer('{}');
  }
  const n = ++step;
  const text = JSON.stringify(body.messages);
  const file = 'business-' + String(n).padStart(2, '0') + '.json';
  save(file, publicRequest(body));
  const event = { request: file };
  events.push(event);
  if (n <= 3) {
    event.original_ordinal = original[n-1].ordinal;
    if (n === 3) {
      assert.ok(text.includes(prepared.framework), 'Real prepare state missing before forced length stop');
      event.prepare = toolReceipt(body, lastId(recorded[1]));
      assert.equal(event.prepare.status, 'authoring_context_prepared');
    }
    return recorded[n-1];
  }
  if (n === 4) {
    const diagnostic = body.messages.filter(m => m.role === 'system' && typeof m.content === 'string' && m.content.includes(marker));
    assert.ok(diagnostic.length, 'Reader never sent the output recovery diagnostic');
    assert.ok(text.includes(message) && text.includes(prepared.framework), 'Original reader request or prepared state lost');
    for (const call of cutoffCalls) assert.ok(!text.includes(call.id), 'Cutoff call was published into recovery history');
    event.diagnostic = diagnostic;
    save('recovery-request.json', publicRequest(body));
  }
  if (actions && n <= 4 + actions.length) {
    const scripted = actions.sample(n-4, body, memory, toolReceipt);
    if (scripted) {
      lastCall = lastId(scripted);
      return scripted;
    }
    save('rebuilt-actions.json', actions.proof);
    if (nextReply) {
      const saved = read(path.resolve(nextReply));
      const decision = actions.rebind(saved.choices?.[0] ?? saved);
      assert.ok(decision.message && decision.finish_reason, 'Expected original OpenAI choice from the new sampling');
      lastCall = lastId(decision);
      return decision;
    }
    if (savedPageFailure) return null;
  }
  if (n === 4) {
    if (liveBase) return null;
    const supplied = responseFile && read(path.resolve(responseFile));
    const choice = !supplied ? controlledWrite() : supplied.choices?.[0] ?? (supplied.message ? supplied :
      { index: 0, finish_reason: supplied.tool_calls?.length ? 'tool_calls' : 'stop', message: supplied });
    assert.ok(choice.message && choice.finish_reason, 'Expected an OpenAI choice or assistant message');
    if (expectOutputLimit) assert.equal(choice.finish_reason, 'length');
    lastCall = lastId(choice);
    save('recovery-response.json', choice);
    return choice;
  }
  if (liveBase) return null;
  const receipt = lastCall && toolReceipt(body, lastCall);
  event.receipt = receipt;
  if (n === (actions ? 4 + actions.length + (nextReply ? 1 : 0) : 5)) {
    if (nextReply) assert.ok(['version_saved','candidate_saved'].includes(receipt?.status),
      'The new model decision did not save a corrected candidate or deliver a version');
    if (actions && receipt.status === 'version_saved') {
      summary.reference = receipt.reference;
      return answer('讲义已整理，可打开阅读。');
    }
    if (actions) candidate = receipt.candidate_id ?? Object.values(actions.proof.candidate_mapping).at(-1)?.current;
    else {
      assert.equal(receipt?.status, 'candidate_saved', 'Supplied recovery reply did not create a candidate');
      candidate = receipt.candidate_id;
    }
    assert.ok(candidate, 'Missing confirmed candidate in the recorded trajectory');
  } else if (preview && !delivered) {
    assert.ok(['preview_environment_recorded','preview_ready_for_inspection'].includes(receipt?.status),
      'Candidate preview failed: ' + JSON.stringify(receipt?.errors ?? receipt).slice(0,800));
    const label = receipt.environment_name;
    const caption = 'Browser observation: candidate ' + candidate + ', environment ' + label + ' ';
    const seen = body.messages.filter(m => Array.isArray(m.content)).some(m => m.content.some((b, index) =>
      b.type === 'text' && b.text?.startsWith(caption) && m.content[index+1]?.image_url?.url?.startsWith('data:image/png;')));
    assert.ok(seen, 'Missing candidate-bound preview screenshot ' + label);
    event.observed = label;
  }
  if (preview < viewports.length) {
    const viewport = viewports[preview++];
    lastCall = 'replay_preview_' + preview;
    return toolCall(lastCall, { operation: 'preview', candidate_id: candidate, viewport });
  }
  if (!delivered) {
    assert.equal(receipt.status, 'preview_ready_for_inspection');
    lastCall = 'replay_deliver'; delivered = true;
    return toolCall(lastCall, { operation: 'deliver', candidate_id: candidate });
  }
  assert.equal(receipt.status, 'version_saved');
  summary.reference = receipt.reference;
  return answer('讲义已整理，可打开阅读。');
}

function send(res, body, choice) {
  const base = { id: 'local-replay', created: 1, model: body.model };
  if (!body.stream) {
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end(JSON.stringify({ ...base, object: 'chat.completion', choices: [choice] }));
    return;
  }
  res.writeHead(200, { 'content-type': 'text/event-stream' });
  const chunk = (delta, finish_reason) => res.write('data: ' + JSON.stringify({ ...base, object: 'chat.completion.chunk',
    choices: [{ index: 0, delta, finish_reason }] }) + '\n\n');
  const delta = { ...choice.message };
  if (delta.tool_calls) delta.tool_calls = delta.tool_calls.map((c, index) => ({ ...c, index }));
  chunk(delta, null);
  chunk({}, choice.finish_reason);
  res.end('data: [DONE]\n\n');
}

const forwarded = liveBase && await startProviderRecorder(liveBase);
const provider = http.createServer(async (req, res) => {
  try {
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    const bytes = Buffer.concat(chunks);
    const body = JSON.parse(bytes.toString());
    if (scriptError) throw scriptError;
    const choice = next(body);
    if (!choice && savedPageFailure) {
      events.at(-1).replayed_http_status = savedPageFailure.status;
      res.writeHead(savedPageFailure.status, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ error: { message: savedPageFailure.reason } }));
      return;
    }
    if (!choice && forwarded) {
      const upstream = await fetch(forwarded.url + req.url, { method: 'POST', body: bytes,
        headers: { 'content-type': req.headers['content-type'] ?? 'application/json' } });
      const response = Buffer.from(await upstream.arrayBuffer());
      const row = forwarded.records.at(-1);
      events.at(-1).forwarded_ordinal = row.ordinal;
      if (row.ordinal === 1 && row.response) save('recovery-response.json', row.response);
      res.writeHead(upstream.status, { 'content-type': upstream.headers.get('content-type') ?? 'application/json' });
      res.end(response);
      return;
    }
    if (requestPurpose(body) === 'business_loop') {
      const { reasoning_content, ...publicMessage } = choice.message;
      events.at(-1).response = { ...choice, message: publicMessage };
    }
    send(res, body, choice);
  } catch (e) {
    scriptError ??= e;
    res.writeHead(400, { 'content-type': 'application/json' });
    res.end(JSON.stringify({ error: { message: e.message } }));
  }
});
await new Promise(resolve => provider.listen(0, '127.0.0.1', resolve));
const address = 'http://127.0.0.1:' + provider.address().port;
const scratch = fs.mkdtempSync(path.join(process.platform === 'win32' ? path.parse(out).root : path.dirname(out), 'ub-r-'));
const memory = path.join(scratch, 'memory'), priv = path.join(scratch, 'private');
let readerServer;
try {
  fs.cpSync(path.join(seeds, sample.band + '-memory'), memory, { recursive: true });
  fs.mkdirSync(priv);
  const settings = read(path.join(source, 'model-settings.json'));
  settings.services = [{ ...settings.services[0], base_url: address, api_key: 'local-replay' }];
  save('model-settings.json', settings);
  const openReader = () => startServer(path.join(seeds, 'book'), memory, root, {
    UNDERSTAND_BOOK_PRIVATE_DIR: priv, UNDERSTAND_BOOK_MODEL_SETTINGS: path.join(out,'model-settings.json'),
    UNDERSTAND_BOOK_MODEL_CONFIG_ID: 'subject', OPENCODE_API_KEY: 'local-replay', OPENCODE_BASE_URL: address,
    FLUID_LLM_MODEL: settings.models[0].model_id, UB_OBSERVABILITY_MODE: 'off', LANGSMITH_TRACING: 'false',
  });
  readerServer = await openReader();
  const history = await readerServer.api('agent/history');
  assert.equal(history.current.turns.length, 7);
  const notes = await readerServer.api('memory/recall', { type: 'note' }, 'POST');
  save('before-notes.json', notes);
  assert.deepEqual(notes, []);
  // The full saved-page failure includes every preview from two recorded runs.
  const chatTimeout = savedPageFailure ? 600000 : liveBase ? 240000 : 180000;
  const outcome = await readerServer.api('agent/chat', { message }, 'POST', chatTimeout);
  save('outcome.json', outcome);
  const prepareStep = outcome.trace.find(s => s.tool === 'presentation.author' && JSON.parse(s.args).operation === 'prepare');
  assert.ok(Number.isInteger(prepareStep?.model_tool_loop), 'Missing product trace for the successful prepare');
  summary.cutoff_tool_loop = prepareStep.model_tool_loop + 1;
  const cutoffExecutions = outcome.trace.filter(s => s.model_tool_loop === summary.cutoff_tool_loop);
  summary.cutoff_executions = cutoffExecutions;
  assert.deepEqual(cutoffExecutions, [], 'Reader executed a tool from the length-limited third business response');
  if (scriptError) throw scriptError;
  const after = await readerServer.api('agent/history');
  save('after-history.json', after);
  assert.equal(after.current.turns.length, 8);
  const afterNotes = await readerServer.api('memory/recall', { type: 'note' }, 'POST');
  save('after-notes.json', afterNotes);
  assert.deepEqual(afterNotes, notes);
  const delivery = await captureDeliveredPage(readerServer, out, after.current.turns.at(-1));
  summary.opening_order = delivery.opening_order;
  assert.equal(outcome.incomplete, expectOutputLimit || Boolean(savedPageFailure), 'Unexpected Reader outcome: ' + (outcome.warning ?? ''));
  if (savedPageFailure) {
    assert.equal(outcome.warning, 'PROVIDER_ERROR');
    assert.equal(fallbackCalls, 1);
    assert.equal(delivery.delivered, 1);
    assert.ok(outcome.trace.some(s => s.tool === 'presentation.author' && JSON.parse(s.args).operation === 'deliver'));
    assert.match(outcome.answer, /可直接打开/);
    assert.match(outcome.answer, /模型服务本次未能完成回复/);
    assert.doesNotMatch(outcome.answer, /(?:^|\n)\s*\{\s*\}\s*(?:\n|$)/);
    assert.ok(!outcome.answer_view.parts.some(p => p.kind === 'markdown' && /^\s*\{\s*\}\s*$/.test(p.text)));
    assert.equal(after.current.turns.at(-1).outcome.answer, outcome.answer);
    summary.observed = { warning: outcome.warning, answer: outcome.answer, incomplete: outcome.incomplete,
      delivered_pages: delivery.delivered, reference: delivery.observations[0].reference, persisted_answer: true };
  } else if (expectOutputLimit) {
    assert.equal(step, 4, 'Expected one business recovery sampling');
    assert.equal(fallbackCalls, 0, 'The terminal cause must be reported by the host');
    assert.equal(outcome.warning, summary.expected_terminal);
    assert.match(outcome.answer, /单次输出上限/);
    assert.match(outcome.answer, /在当前对话中继续/);
    assert.match(outcome.answer, /已开始准备页面.*尚未交付/);
    assert.doesNotMatch(outcome.answer, /重新提供|重贴|音乐|播放|服务恢复/);
    const pageOperations = outcome.trace.filter(s => s.tool === 'presentation.author').map(s => JSON.parse(s.args).operation);
    assert.deepEqual(pageOperations, ['prepare']);
    assert.deepEqual(outcome.effects, []);
    assert.equal(delivery.delivered, 0);
    await readerServer.stop();
    readerServer = await openReader();
    const reopened = await readerServer.api('agent/history');
    const reopenedNotes = await readerServer.api('memory/recall', { type: 'note' }, 'POST');
    assert.equal(reopened.current.id, after.current.id);
    assert.deepEqual(reopened.current.turns.at(-1), after.current.turns.at(-1));
    assert.deepEqual(reopenedNotes, notes);
    summary.observed = { warning: outcome.warning, incomplete: outcome.incomplete, answer: outcome.answer,
      fallback_calls: fallbackCalls, page_operations: pageOperations, delivered_pages: delivery.delivered,
      note_delta: reopenedNotes.length - notes.length, reopened_same_turn: true };
  } else {
    assert.equal(delivery.delivered, 1);
    const observation = delivery.observations[0];
    if (liveBase) {
      summary.reference = observation.reference;
      summary.page_operations = outcome.trace.filter(s => s.tool === 'presentation.author').map(s => JSON.parse(s.args).operation);
      assert.ok(summary.page_operations.includes('deliver'), 'Missing real model deliver tool call');
    } else assert.deepEqual(observation.reference, summary.reference);
    if (!responseFile && !liveBase && !actions) {
      const operations = outcome.trace.filter(s => s.tool === 'presentation.author').map(s => JSON.parse(s.args).operation);
      assert.deepEqual(operations, ['prepare','write','preview','preview','preview','deliver']);
      summary.page_operations = operations;
    }
    assert.ok(observation.width === 320 && observation.scrollWidth <= 320 && !observation.errors.length);
    const eta = observation.text.indexOf('η');
    assert.ok(observation.opening_order.first_step && eta >= 0, 'Missing corrected numerical value or learning-rate notation');
    summary.first_value_offset = observation.opening_order.first_step.text_offset;
    summary.eta_offset = eta;
    if (!responseFile && !liveBase && !actions) assert.equal(observation.opening_order.status,'passed');
  }
  summary.status = 'passed';
} catch (e) {
  summary.status = 'failed';
  summary.error = (scriptError ?? e).message;
  process.exitCode = 1;
} finally {
  summary.unsampled_available_answers = fallbackCalls;
  await readerServer?.stop();
  await new Promise(resolve => provider.close(resolve));
  if (forwarded) {
    await forwarded.stop();
    save('forwarded-provider.json', forwarded.records);
  }
  for (const [from, to] of [[memory,'memory'],[priv,'private']]) {
    if (fs.existsSync(from)) fs.renameSync(from,path.join(out,to));
  }
  fs.rmdirSync(scratch);
  save('replay.json', summary);
  console.log(JSON.stringify({ status: summary.status, error: summary.error, operations: step, out }));
}
