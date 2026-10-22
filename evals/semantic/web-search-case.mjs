// Replays the authorized JSONL question in isolated production multi-user hosts.
// The original history stays local. Reports contain only the evaluated turn,
// tool receipts, supplier usage and the existing charge ledger.
import fs from 'node:fs';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import http from 'node:http';
import { fileURLToPath } from 'node:url';
import { startProviderRecorder, measuredUsage } from './provider-recorder.mjs';

export const originalQuestion = '好的，jsonl我知道是什么了，那为什么用jsonl呢，还有什么其他选择吗';
export const cacheFollowupQuestion = '那联系这个项目的模型调用流程，采用这种格式会怎样影响上下文缓存？如果改成 SQLite，这些收益还能保留吗？请区分项目已经实现的机制和一般可能性。';
export const diagnosticQuestion = '这是一项联网工具技术验收：先调用 web.search，设置 need_content=false，查找 DeepSeek Search/Fetch 官方 API 说明；再调用 web.fetch 读取选中的官方页面正文。根据实际取得的正文，解释两者的接口区别与计费条件，并链接来源。';
const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const writeJson = (filename, value) => fs.writeFileSync(filename, JSON.stringify(value, null, 2) + '\n');
const parse = text => { try { return JSON.parse(text); } catch { return null; } };

export function originalCase(text, question = originalQuestion) {
  const lines = text.replace(/^\uFEFF/, '').trimEnd().split(/\r?\n/);
  const events = lines.map(line => JSON.parse(line));
  const targetIndex = events.findIndex(event => event.kind === 'turn.accepted'
    && event.payload?.turn?.user === question);
  if (targetIndex < 1) throw new Error('Original question is absent or has no prior session context');
  const target = events[targetIndex];
  if (events[0].kind !== 'session.created') throw new Error('Expected original append-only session journal');
  const preceding = events.slice(0, targetIndex);
  const previousTurns = preceding.filter(event => event.kind === 'turn.accepted');
  return {
    prefix: lines.slice(0, targetIndex).join('\n') + '\n',
    question, target, sessionId: events[0].payload.session_id,
    bookId: events[0].payload.book_id,
    prefixLines: targetIndex,
    prefixThroughSeq: preceding.at(-1).seq,
    originalQuestionLine: targetIndex + 1,
    previousUserTurns: previousTurns.length,
  };
}

// An explicit one-day tariff snapshot, not a guessed recurring holiday calendar.
// 2026-10-21 is a non-holiday Wednesday in Hong Kong/Beijing time.
export function verifiedRates(provider, model, date = '2026-10-21') {
  if (date !== '2026-10-21') throw new Error('Re-verify the supplier tariff before using another date');
  const edges = ['00:00:00', '09:00:00', '12:00:00', '14:00:00', '18:00:00', '24:00:00']
    .map(time => Date.parse(`${date}T${time}+08:00`) / 1000);
  const source = {
    currency: 'CNY',
    original_prices: 'DeepSeek-V4.1-Flash-1017 (deepseek-v4-flash alias), CNY per 1M tokens: off-peak uncached input 1, cached input 0.02, output 4; peak 2, 0.04, 8. Weekday peak 09:00-12:00 and 14:00-18:00 Beijing time, excluding holidays.',
    cny_conversion: '1', checked_on: date,
    official_url: 'https://api-docs.deepseek.com/zh-cn/quick_start/pricing/',
  };
  return { rates: edges.slice(0, -1).map((starts_at, index) => {
    const multiplier = index === 1 || index === 3 ? 2 : 1;
    return {
      version: `deepseek-flash-${date}-${index}`, provider, model,
      starts_at, expires_at: edges[index + 1],
      input_micro_cny_per_million: 1_000_000 * multiplier,
      cached_input_micro_cny_per_million: 20_000 * multiplier,
      output_micro_cny_per_million: 4_000_000 * multiplier,
      usage_contract: 'deep_seek', image_meter: 'deep_seek1024', source,
    };
  }) };
}

export function verifiedWebRates(date = '2026-10-21') {
  if (date !== '2026-10-21') throw new Error('Re-verify the supplier tariff before using another date');
  return { rates: ['web_search', 'web_fetch'].map(service_kind => ({
    version: `deepseek-${service_kind}-${date}`, provider: 'deepseek', service_kind,
    starts_at: Date.parse(`${date}T00:00:00+08:00`) / 1000,
    expires_at: Date.parse(`${date}T24:00:00+08:00`) / 1000,
    cost_micro_cny_per_request: 5000,
    source: `https://api-docs.deepseek.com/zh-cn/guides/search/ checked ${date}; direct DeepSeek channel: CNY 5 per 1000 successful requests; failed, empty search and timed-out requests are free.`,
  })) };
}

export function publicProviderTrace(records) {
  // Request messages include the entire private imported history. Persist only
  // current responses and tool receipts observed after a current response call.
  const currentCalls = new Map();
  for (const record of records) for (const choice of record.response?.choices ?? []) {
    for (const call of choice.message?.tool_calls ?? []) currentCalls.set(call.id, call.function);
  }
  const seen = new Set();
  return records.map(record => ({
    ordinal: record.ordinal, started_at: record.started_at, elapsed_ms: record.elapsed_ms,
    status: record.status, error: record.error ?? null, provider_error: record.provider_error ?? null,
    purpose: record.purpose, request_model: record.request?.model,
    request_diff: record.request_diff ?? null,
    response_model: record.response?.model, usage: record.usage ?? null,
    response_complete: record.response_complete ?? false,
    response: record.response ? { id: record.response.id, model: record.response.model,
      choices: (record.response.choices ?? []).map(choice => ({ finish_reason: choice.finish_reason,
        message: { role: choice.message?.role, content: choice.message?.content,
          tool_calls: choice.message?.tool_calls } })) } : null,
    tool_receipts: (record.request?.messages ?? []).flatMap(message => {
      const call = currentCalls.get(message.tool_call_id);
      if (message.role !== 'tool' || !call || seen.has(message.tool_call_id)) return [];
      seen.add(message.tool_call_id);
      return [{ tool_call_id: message.tool_call_id, name: call.name,
        arguments: parse(call.arguments) ?? call.arguments, content: parse(message.content) ?? message.content }];
    }),
  }));
}

export function summarizeCharges(charges) {
  const groups = { model: [], web_search: [], web_fetch: [], other: [] };
  for (const charge of charges) {
    const kind = charge.entry_kind ?? charge.service_kind ?? charge.usage_kind ?? charge.kind;
    const category = kind === 'web_search' || kind === 'web_fetch' ? kind
      : kind === 'model' || charge.model != null ? 'model' : 'other';
    groups[category].push(charge);
  }
  return Object.fromEntries(Object.entries(groups).map(([kind, entries]) => [kind, {
    attempts: entries.length,
    confirmed_cost_micro_cny: entries.reduce((sum, entry) => sum + (entry.provider_cost_micro_cny ?? 0), 0),
    unknown_cost_attempts: entries.filter(entry => entry.provider_cost_micro_cny == null).length,
    account_debit_micro_cny: entries.reduce((sum, entry) => sum + (entry.account_debit_micro_cny ?? 0), 0),
  }]));
}

export function summarizeTokens(records) {
  const sum = select => records.reduce((total, record) => total + (select(record.usage ?? {}) ?? 0), 0);
  const input = sum(usage => usage.prompt_tokens);
  const cached = sum(usage => usage.prompt_cache_hit_tokens ?? usage.prompt_tokens_details?.cached_tokens);
  return { input_tokens: input, cached_input_tokens: cached, uncached_input_tokens: input - cached,
    output_tokens: sum(usage => usage.completion_tokens), total_tokens: sum(usage => usage.total_tokens),
    cached_input_percent: input > 0 ? 100 * cached / input : null,
    unknown_usage_requests: records.filter(record => record.usage == null).length };
}

export function turnAccounting(charges, records, priorChargeIds, firstRecordIndex) {
  const previous = new Set(priorChargeIds);
  const currentCharges = charges.filter(charge => !previous.has(charge.call_id));
  const currentRecords = records.slice(firstRecordIndex);
  return { charges: currentCharges, costs: summarizeCharges(currentCharges),
    provider_ordinals: currentRecords.map(record => record.ordinal),
    provider_usage: measuredUsage(currentRecords), tokens: summarizeTokens(currentRecords),
    tokens_by_purpose: Object.fromEntries([...new Set(currentRecords.map(record => record.purpose))]
      .map(purpose => [purpose, summarizeTokens(currentRecords.filter(record => record.purpose === purpose))])) };
}

export async function drainAccountCharges(api, user, records, options = {}) {
  const timeout_ms = options.timeout_ms ?? 30000;
  const quiet_window_ms = options.quiet_window_ms ?? 2000;
  const poll_ms = options.poll_ms ?? 250;
  const started = performance.now();
  const deadline = started + timeout_ms;
  let charges = [], quietSince = null, previousCounts = '';
  let settled = false;
  while (performance.now() < deadline) {
    charges = [];
    for (let offset = 0; ; offset += 100) {
      const page = await api(`admin/charges?user_id=${user}&limit=100&offset=${offset}`);
      charges.push(...page.items);
      if (charges.length >= page.total) break;
    }
    const counts = `${charges.length}/${records.length}`;
    const inFlight = charges.some(charge => ['reserved', 'sent'].includes(charge.state))
      || records.some(record => record.elapsed_ms == null);
    if (inFlight || counts !== previousCounts) quietSince = null;
    else quietSince ??= performance.now();
    previousCounts = counts;
    if (quietSince != null && performance.now() - quietSince >= quiet_window_ms) { settled = true; break; }
    await new Promise(resolve => setTimeout(resolve, poll_ms));
  }
  return { charges, drain: { settled, elapsed_ms: performance.now() - started, timeout_ms, quiet_window_ms,
    in_flight_charge_ids: charges.filter(charge => ['reserved', 'sent'].includes(charge.state)).map(charge => charge.call_id) } };
}

function command(binary, args, input = '') {
  const result = spawnSync(binary, args, { cwd: repository, windowsHide: true, input, encoding: 'utf8' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${path.basename(binary)} failed: ${result.stderr?.trim()}`);
  return result.stdout.trim();
}

export async function startNetworkServer(binary, serviceRoot, environment) {
  const origin = 'https://web-search-eval.invalid';
  const child = spawn(binary, ['--multi-user', serviceRoot, '--origin', origin, '--addr', '127.0.0.1:0'], {
    cwd: repository, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env: environment,
  });
  let log = '';
  const base = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => { child.kill(); reject(new Error('Isolated network host startup timed out')); }, 30000);
    child.once('error', error => { clearTimeout(timer); reject(error); });
    child.once('exit', code => { clearTimeout(timer); reject(new Error(`Isolated network host exited (${code}): ${log}`)); });
    child.stderr.on('data', bytes => {
      log = (log + bytes.toString()).slice(-6000);
      const match = log.match(/listening at (http:\/\/\S+)/);
      if (match) { clearTimeout(timer); resolve(match[1]); }
    });
    child.stdout.resume();
  });
  let cookie = '', csrf = '';
  const api = async (route, body, timeout = 30000) => {
    // Node's fetch rewrites Host to its loopback URL; the production proxy
    // boundary intentionally requires the configured public Host and Origin.
    const bytes = body === undefined ? undefined : Buffer.from(JSON.stringify(body));
    const response = await new Promise((resolve, reject) => {
      const request = http.request(new URL(`/api/${route}`, base), {
        method: body === undefined ? 'GET' : 'POST',
        headers: { Host: new URL(origin).host, Origin: origin, ...(cookie ? { Cookie: cookie } : {}),
          ...(csrf ? { 'X-CSRF-Token': csrf } : {}),
          ...(bytes === undefined ? {} : { 'Content-Type': 'application/json', 'Content-Length': bytes.length }) },
      }, incoming => {
        const chunks = [];
        incoming.on('data', chunk => chunks.push(chunk));
        incoming.on('end', () => resolve({ status: incoming.statusCode, headers: incoming.headers,
          text: Buffer.concat(chunks).toString('utf8') }));
        incoming.on('error', reject);
      });
      request.setTimeout(timeout, () => request.destroy(new Error(`${route}: request timed out`)));
      request.on('error', reject);
      request.end(bytes);
    });
    const text = response.text;
    const value = parse(text);
    const receivedCookie = response.headers['set-cookie']?.[0];
    if (receivedCookie) cookie = receivedCookie.split(';')[0];
    if (response.status < 200 || response.status >= 300) throw new Error(`${route}: HTTP ${response.status} ${value?.error_code ?? text.slice(0, 150)}`);
    if (value?.csrf_token) csrf = value.csrf_token;
    return value;
  };
  return { api, base,
    async stop() {
      if (child.exitCode !== null || child.signalCode !== null) return;
      await new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('Owned evaluation server did not stop')), 10000);
        child.once('exit', () => { clearTimeout(timer); resolve(); }); child.kill();
      });
    },
  };
}

// The live orchestration is below; pure projections above are separately tested.
export async function runCase(options) {
  const variant = options.variant;
  if (!['baseline', 'candidate', 'diagnostic'].includes(variant)) throw new Error('Use --variant baseline|candidate|diagnostic');
  const diagnostic = variant === 'diagnostic';
  const cacheSequence = options.cacheSequence === true;
  if (diagnostic && cacheSequence) throw new Error('The cache sequence uses baseline/candidate, not the explicit diagnostic');
  const webEnabled = cacheSequence || variant !== 'baseline';
  const binary = options.binDir ? path.resolve(options.binDir, process.platform === 'win32' ? 'server.exe' : 'server') : path.resolve(options.binary);
  const binaryDirectory = path.dirname(binary);
  const executable = name => path.join(binaryDirectory, name + (process.platform === 'win32' ? '.exe' : ''));
  const output = path.resolve(options.output);
  const serviceRoot = path.join(output, 'private-service');
  const fixture = originalCase(fs.readFileSync(options.session, 'utf8'));
  if (fixture.prefixLines !== 547 || fixture.bookId !== 'understandbook') throw new Error('Unexpected original JSONL fixture; inspect before sending');
  if (diagnostic) {
    // This is separately identified capability verification, never another
    // sample of the original-question quality comparison.
    const created = JSON.parse(fixture.prefix.split('\n')[0]);
    created.at = String(Math.floor(Date.now() / 1000));
    created.payload.created_at = created.at;
    created.payload.session_id = `chat-web-diagnostic-${randomUUID()}`;
    fixture.prefix = JSON.stringify(created) + '\n';
    fixture.sessionId = created.payload.session_id;
    fixture.question = diagnosticQuestion;
    fixture.previousUserTurns = 0;
    fixture.prefixLines = 1;
  }
  if (fs.existsSync(output)) throw new Error('Output already exists; retain it and select a new output directory');
  for (const filename of [binary, executable('manage_reader'), executable('publish_book')]) {
    if (!fs.existsSync(filename)) throw new Error(`Required built executable missing: ${filename}`);
  }
  if (fs.existsSync(path.join(repository, '.env'))) process.loadEnvFile(path.join(repository, '.env'));
  const providerBase = process.env.OPENCODE_BASE_URL;
  const model = process.env.FLUID_LLM_MODEL;
  if (new URL(providerBase).hostname !== 'api.deepseek.com' || model !== 'deepseek-v4-flash' || !process.env.OPENCODE_API_KEY) {
    throw new Error('This verified-rate experiment requires the existing direct DeepSeek deepseek-v4-flash configuration');
  }
  const day = new Intl.DateTimeFormat('en-CA', { timeZone: 'Asia/Hong_Kong', year: 'numeric', month: '2-digit', day: '2-digit' }).format(new Date());
  if (day !== '2026-10-21') throw new Error('Tariff snapshot expired; re-verify official prices and intervals');
  fs.mkdirSync(serviceRoot, { recursive: true });
  const user = 'web-search-eval';
  const password = randomUUID() + randomUUID();
  const recorder = await startProviderRecorder(providerBase);
  const modelRates = verifiedRates(recorder.url, model);
  const webRates = verifiedWebRates();
  writeJson(path.join(serviceRoot, 'model-rates.json'), modelRates);
  writeJson(path.join(serviceRoot, 'web-rates.json'), webRates);
  const report = {
    schema_version: 'web-search-case.v2', variant, case_kind: diagnostic ? 'explicit_web_capability_diagnostic' : cacheSequence ? 'original_jsonl_with_cache_followup' : 'original_jsonl_question',
    status: 'preparing', started_at: new Date().toISOString(),
    question: fixture.question, model, provider: providerBase, binary,
    budget_micro_cny: cacheSequence ? 4_000_000 : 2_000_000,
    questions: cacheSequence ? [fixture.question, cacheFollowupQuestion] : [fixture.question], turns: [],
    original_context: diagnostic ? null : { source: path.relative(repository, options.session), journal_prefix_lines: fixture.prefixLines,
      through_seq: fixture.prefixThroughSeq, original_question_line: fixture.originalQuestionLine,
      prior_user_turns: fixture.previousUserTurns, book_id: fixture.bookId,
      retained: 'Original journal prefix, unchanged; original question and anchor; same imported book and current binary for both variants.',
      excluded: 'Original failed attempt, Continue turn and later answers; no cache-answer hints added.',
      limitation: 'Isolated Reader state is reconstructed from the exported journal and local book; original live private memory/teaching databases and old publication identity are not restored.' },
    diagnostic_context: diagnostic ? 'Fresh chat containing only the initial system message, no prior user/assistant/tool history; separate isolated account and ledger. Not a JSONL quality result.' : null,
    protocol: { samples: 1, timeout_ms: 300000, isolation: `New multi-user service root/account with CNY ${cacheSequence ? 4 : 2} allowance per condition; no deployment.`,
      sequence: cacheSequence ? 'Same account, process, session and book for the original question followed by the fixed cache question; both conditions enable Search/Fetch. Each turn waits for ledger/provider settlement before the next.' : 'One question.',
      model_cost: 'Production ledger with official one-day tariffs and actual provider-reported usage. Local recorder endpoint forwards to direct DeepSeek; no ahriapi rate assumption.',
      condition: webEnabled ? 'DEEPSEEK_SEARCH_API_KEY is supplied in child environment.' : 'DEEPSEEK_SEARCH_API_KEY absent in child environment.',
      privacy: 'Imported history stays only in local private-service; shared report/trace excludes all prior request messages, API credentials and provider reasoning.',
      cache: 'Fresh process and empty application web cache for each condition; upstream model cache is observable but cannot be reset.' },
    rates: { model: modelRates, web: webRates },
  };
  const save = () => writeJson(path.join(output, 'run.json'), report);
  let server;
  let turnId;
  let started;
  save();
  try {
    command(executable('manage_reader'), [serviceRoot, 'create', user], password);
    command(executable('manage_reader'), [serviceRoot, 'grant-admin', user]);
    const publication = JSON.parse(command(executable('publish_book'), [serviceRoot, 'import', path.resolve(options.book)]));
    const reference = publication.reference;
    command(executable('publish_book'), [serviceRoot, 'grant', user, reference.book_id, reference.publication_id]);
    const privateMemory = path.join(serviceRoot, 'users', user, 'memory');
    const sessions = path.join(privateMemory, 'agent-sessions');
    fs.mkdirSync(sessions, { recursive: true });
    fs.writeFileSync(path.join(sessions, `s_${Buffer.from(fixture.sessionId).toString('base64url')}.jsonl`), fixture.prefix, { flag: 'wx' });
    writeJson(path.join(privateMemory, 'agent-chat-selection.json'), { active_by_book: { [fixture.bookId]: fixture.sessionId } });
    report.publication = reference;
    const environment = { ...process.env, OPENCODE_BASE_URL: recorder.url, UB_OBSERVABILITY_MODE: 'off',
      UNDERSTAND_BOOK_REVIEW_DRAIN_TIMEOUT_MS: '5000' };
    delete environment.DEEPSEEK_SEARCH_API_KEY;
    if (webEnabled) environment.DEEPSEEK_SEARCH_API_KEY = process.env.OPENCODE_API_KEY;
    server = await startNetworkServer(binary, serviceRoot, environment);
    await server.api('auth/login', { username: user, password });
    const now = Math.floor(Date.now() / 1000);
    const period = await server.api(`admin/users/${user}/allowance-periods`, {
      operation_id: 'eval-period', starts_at: now - 1, expires_at: now + 3600,
    });
    await server.api(`admin/users/${user}/allowance-adjustments`, {
      operation_id: 'eval-budget', period_id: period.allowance_period.period_id, revision: 0,
      delta_micro_cny: report.budget_micro_cny, reason: 'Authorized isolated JSONL answer-quality and actual-cost experiment',
    });
    const attachment = randomUUID();
    let scene = await server.api('workspaces', { attachment_id: attachment, published_book_ref: reference });
    const stamp = () => ({ attachment_id: attachment, generation: scene.generation, expected_revision: scene.revision });
    scene = await server.api(`workspaces/${scene.workspace_id}/chat/select`, { ...stamp(), session_id: fixture.sessionId });
    const anchor = diagnostic ? undefined : fixture.target.payload.turn.question_anchor_lid;
    if (!diagnostic) {
      const readerAnchor = fixture.target.payload.input?.reader?.state?.viewport?.anchor_lid ?? anchor;
      scene = await server.api(`workspaces/${scene.workspace_id}/reader/goto`, { ...stamp(), lid: readerAnchor });
      report.reader_anchor_lid = readerAnchor;
    }
    const history = await server.api(`agent/history?session_id=${encodeURIComponent(fixture.sessionId)}`);
    let current = history.id === fixture.sessionId ? history : history.current ?? history.sessions?.find(session => session.id === fixture.sessionId);
    if (!current || current.turns?.length !== fixture.previousUserTurns || current.turns.some(turn => turn.user === fixture.question)) {
      throw new Error('Isolated journal did not restore exactly the preceding user turns');
    }
    report.restored_user_turns = current.turns.length;
    report.status = 'running'; save();
    started = performance.now();
    let priorChargeIds = [];
    for (const [index, question] of report.questions.entries()) {
      if (index > 0) {
        scene = await server.api(`workspaces/${scene.workspace_id}`);
        const nextHistory = await server.api(`agent/history?session_id=${encodeURIComponent(fixture.sessionId)}`);
        current = nextHistory.id === fixture.sessionId ? nextHistory : nextHistory.current ?? nextHistory.sessions?.find(session => session.id === fixture.sessionId);
        if (current?.turns?.length !== fixture.previousUserTurns + index || current.turns.at(-1)?.turn_id !== report.turns[index - 1].turn_id) {
          throw new Error('Previous evaluated turn is not the current chat head; refusing to start the follow-up in a different history');
        }
      }
      const turnStarted = performance.now();
      const turnReport = { index: index + 1, question, status: 'running', started_at: new Date().toISOString(),
        provider_record_start: recorder.records.length, prior_charge_ids: [...priorChargeIds] };
      report.turns.push(turnReport); save();
      const admission = await server.api('agent/runs', {
        ...stamp(), workspace_id: scene.workspace_id, session_id: fixture.sessionId,
        published_book_ref: reference, client_request_id: `web-search-${variant}-${index + 1}-${randomUUID()}`,
        expected_chat_head: current.turns.at(-1)?.turn_id ?? null, message: question, question_anchor_lid: anchor,
      });
      turnId = admission.turn_id;
      turnReport.turn_id = turnId; report.turn_id = turnId; save();
      while (performance.now() - turnStarted < report.protocol.timeout_ms) {
        const status = await server.api(`agent/runs/${turnId}`);
        if (['settled', 'admission_failed'].includes(status.dispatch_state)) {
          turnReport.turn = status.turn;
          turnReport.status = status.turn?.status === 'completed' ? 'completed' : 'failed';
          turnReport.dispatch_state = status.dispatch_state;
          break;
        }
        await new Promise(resolve => setTimeout(resolve, 1000));
      }
      if (turnReport.status === 'running') {
        turnReport.status = 'timeout';
        await server.api(`agent/runs/${turnId}/cancel`, {});
        for (let attempt = 0; attempt < 30; attempt++) {
          const status = await server.api(`agent/runs/${turnId}`);
          if (status.dispatch_state === 'settled') { turnReport.turn = status.turn; break; }
          await new Promise(resolve => setTimeout(resolve, 1000));
        }
      }
      turnReport.elapsed_ms = performance.now() - turnStarted;
      report.turn = turnReport.turn;
      report.dispatch_state = turnReport.dispatch_state;
      const { charges, drain } = await drainAccountCharges(server.api, user, recorder.records);
      Object.assign(turnReport, turnAccounting(charges, recorder.records, priorChargeIds, turnReport.provider_record_start));
      turnReport.accounting_drain = drain;
      turnReport.allowance = await server.api('account/allowance');
      turnReport.finished_at = new Date().toISOString();
      priorChargeIds = charges.map(charge => charge.call_id);
      save();
      if (turnReport.status !== 'completed') { report.status = turnReport.status; break; }
      if (!drain.settled) { report.status = 'accounting_in_flight'; break; }
    }
    if (report.status === 'running') report.status = 'completed';
    report.elapsed_ms = performance.now() - started;
  } catch (error) {
    report.status = 'failed'; report.error = error.message;
    const lastTurn = report.turns.at(-1);
    if (lastTurn?.status === 'running') { lastTurn.status = 'failed'; lastTurn.error = error.message; }
    report.elapsed_ms = started === undefined ? null : performance.now() - started;
  } finally {
    if (server) {
      try {
        // The account is dedicated to this one replay. Include failed attempts
        // and background model work, even if not attributed to the main turn.
        // A profile review may reserve a charge after the foreground turn has
        // completed but before its request reaches the recorder. Observe both
        // the ledger and recorder, allowing a quiet interval for that handoff.
        const { charges, drain } = await drainAccountCharges(server.api, user, recorder.records);
        report.accounting_drain = drain;
        report.charges = charges;
        report.costs = summarizeCharges(charges);
        report.cost_summary = await server.api(`admin/cost-summary?user_id=${user}`);
        report.allowance = await server.api('account/allowance');
        const lastTurn = report.turns.at(-1);
        if (lastTurn) {
          Object.assign(lastTurn, turnAccounting(charges, recorder.records, lastTurn.prior_charge_ids, lastTurn.provider_record_start));
          lastTurn.accounting_drain = drain;
          lastTurn.allowance = report.allowance;
        }
      } catch (error) { report.accounting_error = error.message; }
      await server.stop();
    }
    await recorder.stop(5000);
    report.provider_usage = measuredUsage(recorder.records);
    report.tokens = summarizeTokens(recorder.records);
    report.actual_response_models = [...new Set(recorder.records.map(record => record.response?.model).filter(Boolean))];
    report.finished_at = new Date().toISOString();
    writeJson(path.join(output, 'provider-trace.json'), publicProviderTrace(recorder.records));
    save();
  }
  return report;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const option = (key, fallback) => { const at = args.indexOf(key); return at < 0 ? fallback : args[at + 1]; };
  const session = path.resolve(option('--session', path.join(repository, 'tmp/jsonl-search-analysis-20261019/original-session.jsonl')));
  const cacheSequence = args.includes('--cache-sequence');
  const binDir = option('--bin-dir');
  if (args.includes('--validate')) {
    const fixture = originalCase(fs.readFileSync(session, 'utf8'));
    console.log(JSON.stringify({ question: fixture.question, prefix_lines: fixture.prefixLines,
      prior_user_turns: fixture.previousUserTurns, original_question_line: fixture.originalQuestionLine,
      questions: cacheSequence ? [fixture.question, cacheFollowupQuestion] : [fixture.question],
      cache_sequence: cacheSequence, bin_dir: binDir ? path.resolve(binDir) : null,
      budget_micro_cny: cacheSequence ? 4_000_000 : 2_000_000,
      web_enabled_in_both_conditions: cacheSequence }, null, 2));
  } else {
    const variant = option('--variant');
    const report = await runCase({ variant, session, cacheSequence, binDir,
      binary: option('--binary', path.join(repository, 'target/debug', process.platform === 'win32' ? 'server.exe' : 'server')),
      book: option('--book', path.join(repository, '.understand-book/understandbook')),
      output: option('--out', path.join(repository, cacheSequence ? 'tmp/web-cache-eval-20261021' : 'tmp/web-search-eval-20261021', variant ?? 'unspecified')),
    });
    console.log(JSON.stringify({ variant: report.variant, status: report.status, costs: report.costs,
      elapsed_ms: report.elapsed_ms, provider_usage: report.provider_usage, tokens: report.tokens,
      turns: report.turns.map(turn => ({ index: turn.index, status: turn.status, turn_id: turn.turn_id,
        costs: turn.costs, tokens: turn.tokens, accounting_drain: turn.accounting_drain })), error: report.error ?? null,
      accounting_error: report.accounting_error ?? null }, null, 2));
    if (report.status !== 'completed' || report.accounting_error) process.exitCode = 1;
  }
}
