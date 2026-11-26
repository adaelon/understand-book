// Sample one captured request, or resume the real Reader with --reader.
// The Reader runner owns the recorded prefix; this process meters every live call.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {createBudget, costCeiling} from './long-conversation-pair.mjs';
import {startProviderRecorder} from './provider-recorder.mjs';

const args = process.argv.slice(2);
const value = key => args.includes(key) ? args[args.indexOf(key) + 1] : undefined;
const requestPath = value('--request');
const out = value('--out');
if (!requestPath || !out) {
  throw new Error('Supply --request <Reader recovery-request.json> --out <new evidence directory>; use --live to sample.');
}
const maxCny = Number(value('--max-cny'));
if (!Number.isFinite(maxCny) || maxCny <= 0) throw new Error('Supply the remaining authorized --max-cny.');
const live = args.includes('--live');
const reader = value('--reader');
const replayActions = value('--resume-actions');
const reviewWire = value('--review-wire');
if (reviewWire && (reader || args.includes('--disable-thinking'))) throw new Error('Review control reuses the original request settings.');
if (replayActions && !reader) throw new Error('--resume-actions requires --reader.');
const request = JSON.parse(fs.readFileSync(requestPath, 'utf8'));
const hasRecoveryMarker = body => (body.messages ?? []).some(m => m.role === 'system' &&
  typeof m.content === 'string' && m.content.split('\n').includes('presentation_output_recovery.v1'));
if ((!reviewWire && !hasRecoveryMarker(request)) || !request.tools?.length) throw new Error('Expected a Reader request with active tools.');
if (live && !process.env.OPENCODE_API_KEY) throw new Error('Set the trial provider credential.');
const fixtureModel = request.model;
if (fixtureModel !== 'deepseek-flash') throw new Error('The frozen recovery request must target deepseek-flash.');
const disableThinking = args.includes('--disable-thinking');
if (reader && disableThinking) throw new Error('Single-request override is separate from the live Reader path.');
if (disableThinking) request.thinking = {type:'disabled'};
const deadline = Date.now() + (reader ? 240_000 : 180_000);
const budget = createBudget(maxCny, replayActions ? 4 : reader ? 12 : 1, 8192);
fs.mkdirSync(out, {recursive:false});
const save = (name, data) => fs.writeFileSync(path.join(out,name), JSON.stringify(data,null,2)+'\n');
const receipt = { live, source:path.resolve(requestPath), fixtureModel, model:request.model,
  thinking_override:disableThinking ? 'disabled' : null,
  output_limit:request.max_tokens, deadline:new Date(deadline).toISOString(), max_cny:maxCny };
function reservationBody(body) {
  // DeepSeek's vision docs bound an inline image to 1024 tokens. Substitute
  // an equal number of one-byte characters only for admission, never on wire.
  let count = 0;
  const messages = body.messages.map(message => {
    if (!Array.isArray(message.content)) return message;
    const content = message.content.map(block => {
      if (block.type !== 'image_url' || !block.image_url?.url?.startsWith('data:image/png;base64,iVBORw0KGgo')) return block;
      count++;
      return {type:'text', text:'i'.repeat(1024)};
    });
    return {...message, content};
  });
  return {estimate:count ? {...body, messages} : body, count};
}
function authorize(body, row) {
  if (reader && !replayActions && !budget.ledger.length && (!hasRecoveryMarker(body) || body.thinking?.type !== 'disabled')) {
    return 'PRODUCTION_RECOVERY_CONFIGURATION_NOT_PRESENT';
  }
  const {estimate, count} = reservationBody(body);
  row.official_inline_images = count;
  const rejection = budget.admit(estimate,row,0,deadline,'deepseek-flash');
  Object.assign(receipt, { blocked:rejection, reserved_micro_cny:row.reservation ?? 0,
    multiplier:row.rate_multiplier });
  save('receipt.json', receipt);
  return rejection;
}
// Keep field shape only so provider history compatibility can be checked.
function reasoningShapes(raw, streaming) {
  const chunks = streaming ? raw.split(/\r?\n/).filter(line => line.startsWith('data:'))
    .map(line => line.slice(5).trim()).filter(data => data && data !== '[DONE]') : [raw];
  const shapes = new Set();
  for (const chunk of chunks) {
    for (const choice of JSON.parse(chunk).choices ?? []) {
      const delta = choice.delta ?? choice.message ?? {};
      if (!Object.hasOwn(delta,'reasoning_content')) shapes.add('absent');
      else if (delta.reasoning_content === null) shapes.add('null');
      else if (delta.reasoning_content === '') shapes.add('empty');
      else shapes.add('present');
    }
  }
  return [...shapes];
}
// The official endpoint branch cannot recognize the localhost replay service.
// Mirror only its recovery setting. Subsequent request bodies pass unmodified.
async function startOfficialRelay(recorderUrl) {
  const exchanges = [];
  const server = http.createServer(async (req,res) => {
    try {
      const chunks = [];
      for await (const chunk of req) chunks.push(chunk);
      let bytes = Buffer.concat(chunks);
      const body = JSON.parse(bytes.toString());
      const first = !exchanges.length;
      const exchange = {ordinal:exchanges.length + 1, wire_thinking:body.thinking ?? null,
        mapped_official_setting:false};
      if (first && replayActions) {
        exchange.continuation = verifyReconstructedRequest(body, replayActions);
      } else if (first) {
        if (!hasRecoveryMarker(body) || !body.messages.some(m => typeof m.content === 'string'
          && m.content.includes('omit introductory material and optional controls'))) {
          throw new Error('Expected final recovery ordering instruction on the first forwarded request.');
        }
        if (body.thinking === undefined) {
          body.thinking = {type:'disabled'};
          exchange.mapped_official_setting = true;
          bytes = Buffer.from(JSON.stringify(body));
        }
      }
      exchanges.push(exchange);
      const response = await fetch(recorderUrl + req.url, {method:'POST', body:bytes,
        headers:{'content-type':'application/json'}});
      const responseBytes = Buffer.from(await response.arrayBuffer());
      const type = response.headers.get('content-type') ?? 'application/json';
      if (response.ok) exchange.response_reasoning_shapes = reasoningShapes(responseBytes.toString(), type.includes('text/event-stream'));
      save('relay.json',exchanges);
      res.writeHead(response.status, {'content-type':type});
      res.end(responseBytes);
    } catch (error) {
      save('relay-error.json', {message:error.message});
      res.writeHead(502, {'content-type':'application/json'});
      res.end(JSON.stringify({error:{message:error.message}}));
    }
  });
  await new Promise(resolve => server.listen(0,'127.0.0.1',resolve));
  return {url:'http://127.0.0.1:' + server.address().port,
    stop:() => { server.closeAllConnections(); return new Promise(resolve => server.close(resolve)); }};
}
function originalContinuations(source) {
  const sessionDir = path.join(source,'memory','agent-sessions');
  const eventRows = fs.readFileSync(path.join(sessionDir,fs.readdirSync(sessionDir)[0]),'utf8').trim().split('\n').map(JSON.parse);
  return new Map(eventRows.flatMap(r => r.payload?.messages ?? []).filter(m => m.provider_continuation)
    .map(m => [m.tool_calls?.[0]?.id, m.provider_continuation.reasoning_content]));
}
const read = (root, name) => JSON.parse(fs.readFileSync(path.join(root,name),'utf8'));
function verifyReconstructedRequest(body, source) {
  const rebuilt = read(path.join(out,'reader'),'rebuilt-actions.json');
  const mapping = Object.values(rebuilt.candidate_mapping);
  if (rebuilt.model_samplings !== 12 || rebuilt.restored_continuations !== 11 ||
      mapping.length !== 4 || !mapping.every(pair => pair.same_content)) {
    throw new Error('Replayed decisions or four stored candidate versions have diverged.');
  }
  const previous = read(source,'business-16.json');
  const reasoning = originalContinuations(source);
  const stripCandidateIds = text => text.replace(/candidate-\d+-\d+/g,'candidate-ID');
  const assistant = body.messages.filter(m => m.role === 'assistant');
  const original = previous.messages.filter(m => m.role === 'assistant');
  if (body.thinking || original.length !== 11 || assistant.length !== original.length || reasoning.size !== 11) {
    throw new Error('Unmatched continuation reasoning mode or history length.');
  }
  for (const [i, m] of assistant.entries()) {
    const key = original[i].tool_calls[0].id;
    if (stripCandidateIds(JSON.stringify(m.tool_calls)) !== stripCandidateIds(JSON.stringify(original[i].tool_calls)) ||
      typeof m.reasoning_content !== 'string' || stripCandidateIds(m.reasoning_content) !== stripCandidateIds(reasoning.get(key))) {
      throw new Error('Unmatched original assistant decision or continuation at index ' + i);
    }
  }
  const markers = /TASK_SEND_LIMIT|EXPERIMENT_TOKEN_BUDGET|BATCH_COST_LIMIT/;
  const publicHistory = body.messages.map(({reasoning_content, ...m}) => m);
  if (markers.test(JSON.stringify(publicHistory))) throw new Error('Fixture budget error entered Reader history.');
  const imageCount = messages => messages.flatMap(m => Array.isArray(m.content) ? m.content : [])
    .filter(b => b.type === 'image_url').length;
  const toolReceipts = messages => messages.filter(m => m.role === 'tool').map(m => {
    const result = JSON.parse(m.content);
    const receipt = result.model_body ?? result.body ?? result;
    return [m.tool_call_id,receipt.status ?? result.status,receipt.environment_name,
      stripCandidateIds(String(receipt.error ?? receipt.message ?? ''))];
  });
  if (body.messages.length !== previous.messages.length ||
      imageCount(body.messages) !== imageCount(previous.messages) ||
      JSON.stringify(toolReceipts(body.messages)) !== JSON.stringify(toolReceipts(previous.messages))) {
    throw new Error('Unmatched Reader tool receipts, preview environment, image count, or transcript length.');
  }
  const replays = assistant.map(m => m.tool_calls[0].id);
  return { paid_resume_ordinal:13, verified_prior_reasoning:reasoning.size, replays,
    original_messages:previous.messages.length, resumed_messages:body.messages.length,
    original_images:imageCount(previous.messages), resumed_images:imageCount(body.messages), fixture_budget_error:false };
}
if (reviewWire) {
  const sample = JSON.parse(fs.readFileSync(reviewWire,'utf8'));
  const before = request.messages.at(-1), current = sample.messages.at(-1);
  const images = m => m.content.filter(b => b.type === 'image_url').map(b => b.image_url.url);
  const captions = m => m.content.filter(b => b.type === 'text');
  const noId = s => s.replace(/candidate-\d+-\d+/g,'candidate-ID');
  const originals = captions(before), fresh = captions(current);
  if (before.role !== 'user' || current.role !== 'user' || originals.length !== 2 || fresh.length !== 2 ||
      !originals[0].text.includes('step 0, status preview_ready_for_inspection') ||
      !originals[0].text.includes('actual document position x=0, y=0 CSS px') ||
      JSON.stringify(images(before)) !== JSON.stringify(images(current)) || images(before).length !== 2 ||
      !noId(fresh[0].text).startsWith(noId(originals[0].text)) || noId(originals[1].text) !== noId(fresh[1].text)) {
    throw new Error('The two original screenshots, scroll positions or unchanged caption have diverged.');
  }
  const suffix = noId(fresh[0].text).slice(noId(originals[0].text).length);
  if (!suffix.trim()) throw new Error('The real Reader caption did not change.');
  const source = path.dirname(requestPath);
  const decision = read(source,'forwarded-provider.json').find(r => r.response?.choices[0]?.message.tool_calls?.some(
    c => c.function?.name === 'presentation_author' && JSON.parse(c.function.arguments).operation === 'deliver'));
  if (!decision || JSON.stringify(decision.request) !== JSON.stringify(request)) throw new Error('The original wire is not the observed delivery decision.');
  // The public recorder omitted model reasoning. Restore from the Reader's
  // own session and match every field against the actual provider call IDs.
  const thought = originalContinuations(source);
  const previous = request.messages.filter(m => m.role === 'assistant');
  if (previous.length !== 13) throw new Error('Expected the original 13 assistant continuation messages.');
  for (const m of previous) {
    const text = thought.get(m.tool_calls[0].id);
    if (typeof text !== 'string' || !text.length) throw new Error('Missing original protocol continuation.');
    m.reasoning_content = text;
  }
  originals[0].text += suffix;
  receipt.caption_comparison = {production_request:path.resolve(reviewWire), changed_captions:1,
    identical_pngs:2, unchanged_scrolled_caption:true, original_decision:'deliver',
    restored_provider_continuations:previous.length, appended_text:suffix};
}
if (!live) {
  let preflight = request;
  if (replayActions) {
    preflight = read(replayActions,'business-16.json');
    const reasoning = originalContinuations(replayActions);
    for (const message of preflight.messages.filter(m => m.role === 'assistant')) {
      message.reasoning_content = reasoning.get(message.tool_calls[0].id);
    }
  } else if (reader && !request.thinking) {
    preflight = {...request, thinking:{type:'disabled'}};
    receipt.localhost_recovery_mapping = true;
  }
  const rejected = authorize(preflight, {request:preflight});
  receipt.upstream_sends = 0;
  save('receipt.json', receipt);
  if (rejected) process.exitCode = 1;
} else {
  const recorder = await startProviderRecorder('https://api.deepseek.com', {
    apiKey:process.env.OPENCODE_API_KEY, beforeSend:authorize
  });
  let stopping;
  try {
    if (reader) {
      const relay = await startOfficialRelay(recorder.url);
      const child = spawn(process.execPath, ['evals/semantic/long-conversation-output-recovery.mjs',
        '--reader', reader, '--out', path.join(out,'reader'), '--live-recovery-base', relay.url,
        ...(replayActions ? ['--replay-actions',replayActions] : [])], {stdio:'inherit'});
      const cutoff = setTimeout(() => { stopping = recorder.stop(); }, Math.max(0,deadline-Date.now()));
      try {
        const [exitCode] = await once(child,'exit');
        receipt.reader_exit_code = exitCode;
        if (exitCode !== 0) process.exitCode = 1;
      } finally {
        clearTimeout(cutoff);
        await relay.stop();
      }
    } else {
      const res = await fetch(recorder.url + '/chat/completions', {
        method:'POST', headers:{'content-type':'application/json'}, body:JSON.stringify(request),
        signal:AbortSignal.timeout(180_000)
      });
      const raw = await res.text();
      if (!res.ok) throw new Error('Recorder or provider HTTP ' + res.status);
      receipt.response_reasoning_field_shapes = reasoningShapes(raw,
        (res.headers.get('content-type') ?? '').includes('text/event-stream'));
    }
  } finally {
    await (stopping ?? recorder.stop());
    const [row] = recorder.records;
    if (reader) {
      receipt.upstream_sends = budget.ledger.length;
      receipt.charged_upper_micro_cny = budget.consumed();
      receipt.requests = recorder.records.map(row => ({ordinal:row.ordinal, status:row.status,
        thinking:row.request?.thinking, images:row.official_inline_images, usage:row.usage,
        reserve:row.reservation, cost:costCeiling(row), blocked:row.blocked, error:row.provider_error ?? row.error,
        finish:row.response?.choices?.[0]?.finish_reason}));
      save('receipt.json',receipt);
      save('providers.json',recorder.records);
    } else if (row) {
      Object.assign(receipt, { status:row.status, upstream_sends:budget.ledger.length,
        usage:row.usage, finish_reason:row.response?.choices?.[0]?.finish_reason,
        charged_upper_micro_cny:costCeiling(row), provider_error:row.provider_error });
      save('receipt.json',receipt);
      save('provider.json',row);
      if (row.response?.choices?.[0]) save('response.json',row.response);
    }
  }
}
