// Paired continuation from two frozen Reader private histories. Only --live forwards requests.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { startServer } from './server.mjs';
import { startProviderRecorder } from './provider-recorder.mjs';

export const models = ['deepseek-flash', 'deepseek-v4-pro'];
// Peak CNY per million tokens. Night sends have a documented 50% discount.
const rates = { 'deepseek-flash': [0.04, 2, 8], 'deepseek-v4-pro': [0.30, 9, 27] };
const window = 1_000_000;
const defaultOutputLimit = 8192;
const finalMessage = '按之前的约定整理。';
const write = (dir, name, data) => fs.writeFileSync(path.join(dir, name), JSON.stringify(data, null, 2) + '\n');
const integer = n => Number.isSafeInteger(n) && n >= 0;

export function inputUsage(usage) {
  if (integer(usage?.prompt_tokens)) return usage.prompt_tokens;
  const parts = [usage?.input_tokens, usage?.cache_read_input_tokens, usage?.cache_creation_input_tokens];
  return parts.every(integer) ? parts.reduce((a, b) => a + b) : null;
}

export function costCeiling(row) {
  if (!row.reservation) return 0;
  const u = row.usage;
  const input = inputUsage(u);
  const output = u?.completion_tokens ?? u?.output_tokens;
  if (input === null || !integer(output)) return row.reservation;
  const reportedHit = u.prompt_cache_hit_tokens ?? u.prompt_tokens_details?.cached_tokens ?? u.cache_read_input_tokens ?? 0;
  const hit = integer(reportedHit) && reportedHit <= input ? reportedHit : 0;
  const [read, miss, generated] = rates[row.request.model];
  return Math.ceil((hit * read + (input - hit) * miss + output * generated) * row.rate_multiplier);
}

function nightMultiplier(now, deadline) {
  if (!Number.isFinite(deadline)) return 1;
  const shanghai = new Date(now + 8*3600000);
  const hour = shanghai.getUTCHours();
  if (hour >= 8 && hour < 19) return 1;
  const nine = Date.UTC(shanghai.getUTCFullYear(),shanghai.getUTCMonth(),
    shanghai.getUTCDate() + (hour >= 19 ? 1 : 0), 9) - 8*3600000;
  // Even after the deadline, leave an extra hour before the next possible peak.
  return deadline + 3600000 < nine ? 0.5 : 1;
}

export function createBudget(maxCny, maxRequests, outputLimit=defaultOutputLimit) {
  const ledger = [];
  const consumed = () => ledger.reduce((sum, row) => sum + costCeiling(row), 0);
  return { consumed, ledger, admit(request, row, started, deadline, expectedModel, heldForLater = 0) {
    if (request.model !== expectedModel || !rates[request.model]) return 'MODEL_MISMATCH';
    if (Date.now() >= deadline) return 'TASK_DEADLINE';
    if (ledger.length - started >= maxRequests) return 'TASK_SEND_LIMIT';
    const maxOutput = request.max_tokens ?? request.max_completion_tokens;
    if (!integer(maxOutput) || maxOutput === 0 || maxOutput > outputLimit) return 'UNBOUND_OUTPUT_LIMIT';
    const bytes = Buffer.byteLength(JSON.stringify(request));
    const image = (request.messages ?? []).some(m => Array.isArray(m.content)
      && m.content.some(b => b.type === 'image_url' || b.type === 'image'));
    if (image && request.model === 'deepseek-v4-pro') return 'PROVIDER_IMAGE_UNSUPPORTED';
    // For textual input one token can encode as little as one byte. Include wire
    // framing for roles, tool calls and schemas; images reserve the full window.
    const framing = 4096 + 32 * ((request.messages?.length ?? 0) + (request.tools?.length ?? 0));
    const maxInput = image ? window : Math.min(window, bytes + framing);
    const [,miss,generated] = rates[expectedModel];
    row.rate_multiplier = nightMultiplier(Date.now(),deadline);
    const reserve = Math.ceil((maxInput * miss + maxOutput * generated) * row.rate_multiplier);
    if (consumed() + reserve > Math.floor((maxCny-heldForLater) * 1e6)) return 'BATCH_COST_LIMIT';
    row.reservation = reserve;
    ledger.push(row);
    return null;
  } };
}

function settings(base, model, outputLimit) {
  return { revision: 1, services: [{ id: 'trial', name: 'Trial recorder', base_url: base, auth: 'bearer', api_key: 'recorder-only' }],
    models: [{ id: 'subject', name: model, model_id: model, service_id: 'trial', api_protocol: 'openai_chat_completions',
      context_window_tokens: window, output_token_limit: outputLimit,
      capabilities: { native_tools: true, parallel_tools: false, streaming: true, streaming_tools: true,
        json_object: true, images: model === 'deepseek-flash' },
      generation: { temperature: 0, output_token_parameter: 'max_tokens' } }], bindings: { default_model: 'subject' } };
}

export function pairedOrder() {
  return [1, 2].flatMap(repeat => (repeat === 1
    ? [['short', models[0]], ['long', models[0]], ['long', models[1]], ['short', models[1]]]
    : [['short', models[1]], ['long', models[1]], ['long', models[0]], ['short', models[0]]]
  ).map(([band, model]) => ({ band, model, repeat })));
}

function classifyFirst(records, band) {
  // Require a business sampling with Reader tools, excluding compaction and
  // auxiliary requests that may run before the first decision.
  const row = records.find(r => r.purpose === 'business_loop' || r.request?.tools?.some(
    t => ['tool_search','tool.search'].includes(t.name ?? t.function?.name)));
  if (!row) return { band_status: 'first_main_request_missing' };
  const tokens = inputUsage(row.usage);
  const tools = (row.request.tools ?? []).map(t => t.name ?? t.function?.name).sort();
  const supplied = row.request.messages.filter(m=>m.role==='user').map(m=>typeof m.content === 'string' ? m.content :
    (m.content ?? []).filter(b=>b.type==='text').map(b=>b.text).join('\n')).join('\n');
  return { first_ordinal: row.ordinal, first_input: tokens, first_tools: tools,
    verbatim_user_constraints: {
      ordering_and_symbol: supplied.includes('先给数值、再写公式，学习率全篇用 η'),
      save_condition: supplied.includes('等我明确说保存再写笔记'),
      correction: supplied.includes('示例值改为 0.01'), final_request:supplied.includes(finalMessage),
    },
    band_status: tokens === null ? 'usage_missing' : (band === 'long' ? tokens > 100000 : tokens < 100000) ? 'matched' : 'outside_band' };
}

async function runSample(options, schedule, budget, out) {
  const { model, band } = schedule;
  // On Windows a deep evidence path can exceed the effective Win32 rename
  // limit for nested presentation receipts. Keep the live store near the drive
  // root and archive its contents after Reader releases its locks.
  const scratch = fs.mkdtempSync(path.join(options.scratch, 'p-'));
  const memory = path.join(scratch, 'memory');
  fs.cpSync(options[band], memory, { recursive: true });
  const privateDir = path.join(scratch, 'private');
  fs.mkdirSync(privateDir);
  const started = budget.ledger.length;
  const deadline = Date.now() + options.seconds * 1000;
  const sample = { ...schedule, mode: options.live ? 'live' : 'preflight', started_at: new Date().toISOString() };
  const recorder = await startProviderRecorder('https://api.deepseek.com', {
    apiKey: options.live ? process.env.OPENCODE_API_KEY : null,
    beforeSend: (req, row) => {
      if (!options.live) return 'OFFLINE_PREFLIGHT';
      const heldShare = !options.only && schedule.repeat === 1 ? ({'short-deepseek-flash':0.5,'long-deepseek-flash':0.35,'long-deepseek-v4-pro':0.07}[band+'-'+model] ?? 0) : 0;
      const rejection = budget.admit(req, row, started, deadline, model, heldShare * options.maxCny);
      const evidence = { ordinal:row.ordinal,purpose:row.purpose,model:req.model,band,
        reserved_micro_cny:row.reservation ?? 0, batch_cost_ceiling_micro_cny:budget.consumed(),
        stop:rejection, at:new Date().toISOString() };
      fs.appendFileSync(path.join(out,'send-ledger.jsonl'),JSON.stringify(evidence)+'\n');
      return rejection;
    } });
  let server;
  try {
    write(out, 'model-settings.json', settings(recorder.url, model, options.outputLimit));
    server = await startServer(options.book, memory, process.cwd(), {
      UNDERSTAND_BOOK_PRIVATE_DIR: privateDir, UNDERSTAND_BOOK_MODEL_SETTINGS: path.join(out, 'model-settings.json'),
      UNDERSTAND_BOOK_MODEL_CONFIG_ID: 'subject',
      OPENCODE_API_KEY: 'recorder-only', OPENCODE_BASE_URL: recorder.url, FLUID_LLM_MODEL: model,
      UB_OBSERVABILITY_MODE: 'off', LANGSMITH_TRACING: 'false',
    });
    const before = await server.api('agent/history');
    write(out, 'before-history.json', before);
    if (before.current?.turns?.length !== 7) throw new Error('Frozen Reader must select the intended seven-turn chat');
    write(out, 'before-notes.json', await server.api('memory/recall', { type: 'note' }, 'POST'));
    const response = await fetch(new URL('/api/agent/chat',server.url), { method:'POST', headers:{'Content-Type':'application/json'},
      body: JSON.stringify({message:finalMessage}), signal:AbortSignal.timeout(Math.max(1,deadline-Date.now())) });
    const outcome = await response.json();
    write(out, 'outcome.json', outcome);
    if (!response.ok) throw new Error('agent/chat: HTTP ' + response.status + ' ' + outcome.error_code);
    sample.completed = outcome.incomplete === false && !outcome.error;
  } catch (e) { sample.error = e.message; }
  finally {
    if (server) {
      try {
        const after = await server.api('agent/history');
        write(out, 'after-history.json', after);
        write(out, 'after-notes.json', await server.api('memory/recall', { type: 'note' }, 'POST'));
        if (options.live) {
          const { captureDeliveredPage } = await import('./long-conversation-delivery.mjs');
          const delivery = await captureDeliveredPage(server, out, after.current?.turns?.length === 8 ? after.current.turns.at(-1) : null);
          sample.opening_order = delivery.opening_order;
        }
      } catch (e) { sample.inspection_error = e.message; }
      await server.stop();
    }
    await recorder.stop(options.live ? 60000 : 3000);
    for (const [source,name] of [[memory,'memory'],[privateDir,'private']]) {
      try { fs.renameSync(source,path.join(out,name)); }
      catch (error) {
        if (error.code !== 'EXDEV') throw error;
        fs.cpSync(source,path.join(out,name),{recursive:true});
        fs.rmSync(source,{recursive:true});
      }
    }
    fs.rmdirSync(scratch);
    write(out, 'provider.json', recorder.records);
    Object.assign(sample, classifyFirst(recorder.records, band), {
      finished_at: new Date().toISOString(), sends: budget.ledger.length - started,
      stops: recorder.budgetRejections, batch_cost_ceiling_micro_cny: budget.consumed() });
    write(out, 'sample.json', sample);
  }
  return sample;
}

async function main(args) {
  const arg = key => args.includes(key) ? args[args.indexOf(key)+1] : undefined;
  const absolute = key => path.resolve(arg(key) ?? (()=>{ throw new Error('Missing ' + key); })());
  const live = args.includes('--live');
  const options = { live, book: absolute('--book'), short: absolute('--short-memory'), long: absolute('--long-memory'),
    scratch:path.resolve(arg('--scratch') ?? '.tmp/paired'),
    only:arg('--only') ?? null,outputLimit:Number(arg('--output-limit') ?? defaultOutputLimit),
    seconds: Number(arg('--timeout-seconds') ?? 300) };
  const out = absolute('--out');
  const maxCny = Number(arg('--max-cny') ?? 0), maxRequests = Number(arg('--max-requests') ?? 12);
  options.maxCny = maxCny;
  if (live && (!Number.isFinite(maxCny) || maxCny <= 0 || !process.env.OPENCODE_API_KEY))
    throw new Error('Live sampling requires a positive --max-cny and configured OPENCODE_API_KEY');
  if (!Number.isSafeInteger(maxRequests) || maxRequests < 1 || !Number.isSafeInteger(options.seconds) || options.seconds < 1)
    throw new Error('Invalid invocation or time limit');
  if (![8192,16384].includes(options.outputLimit)) throw new Error('Output limit must be 8192 or 16384');
  let order=pairedOrder().slice(0,live ? 8 : 4);
  if (options.only) {
    order=order.filter(s=>s.repeat===1 && s.band+'-'+s.model===options.only);
    if (!order.length) throw new Error('--only expects a supported band-model pair, e.g. long-deepseek-flash');
  }
  const budget = createBudget(maxCny, maxRequests, options.outputLimit);
  fs.mkdirSync(out);
  fs.mkdirSync(options.scratch,{recursive:true});
  const server = process.env.SEMANTIC_EVAL_SERVER ?? path.resolve('target/debug/reader' + (process.platform === 'win32' ? '.exe' : ''));
  const report = { ...options, maxCny, maxRequests, models, planned_samples:order.length, output_limit: options.outputLimit, high_tariff_cny_per_million: rates,
    code: execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),
    reader: { path: server, modified: fs.statSync(server).mtime.toISOString() },
    samples: [] };
  fs.writeFileSync(path.join(out,'source.diff'),execFileSync('git',['diff','HEAD','--','crates/runtime','crates/reader-service','crates/reader','evals/semantic/provider-recorder.mjs'], {maxBuffer:32*1024*1024}));
  fs.copyFileSync(new URL(import.meta.url),path.join(out,'runner.mjs'));
  write(out,'run.json',report);
  for (const [index, entry] of order.entries()) {
    const id = [String(index+1).padStart(2,'0'),entry.band,entry.model,entry.repeat].join('-');
    const dir = path.join(out,id);
    fs.mkdirSync(dir);
    const row = await runSample(options,entry,budget,dir);
    report.samples.push({ id,...row });
    write(out,'run.json',report);
    console.log(id,row.band_status,row.sends,row.error ?? '');
    if (live && budget.consumed() >= Math.floor(maxCny*1000000)) break;
  }
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url)
  await main(process.argv.slice(2));
