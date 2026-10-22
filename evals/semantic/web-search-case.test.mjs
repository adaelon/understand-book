import test from 'node:test';
import assert from 'node:assert/strict';
import { originalCase, originalQuestion, verifiedRates, verifiedWebRates, publicProviderTrace, summarizeCharges, drainAccountCharges, turnAccounting } from './web-search-case.mjs';

test('replay stops before the first original question, excluding its answer and later cache hints', () => {
  const events = [
    { seq: 1, kind: 'session.created', payload: { session_id: 'chat-original', book_id: 'book' } },
    { seq: 2, kind: 'turn.accepted', payload: { turn: { user: '先解释格式' } } },
    { seq: 3, kind: 'turn.completed', payload: { answer: 'prior explanation' } },
    { seq: 4, kind: 'turn.accepted', payload: { turn: { user: originalQuestion } } },
    { seq: 5, kind: 'turn.completed', payload: { answer: 'future answer with cache hint' } },
    { seq: 6, kind: 'turn.accepted', payload: { turn: { user: originalQuestion } } },
  ];
  const fixture = originalCase(events.map(event => JSON.stringify(event)).join('\n'));
  assert.equal(fixture.prefixLines, 3);
  assert.equal(fixture.originalQuestionLine, 4);
  assert.equal(fixture.previousUserTurns, 1);
  assert.doesNotMatch(fixture.prefix, /cache hint|为什么用jsonl/);
});

test('drain waits for a late background charge before the recorder sees its request', async () => {
  const foreground = { call_id: 'foreground', state: 'settled', model: 'model', provider_cost_micro_cny: 1 };
  const records = [{ elapsed_ms: 1 }];
  let polls = 0;
  const api = async () => {
    polls++;
    if (polls === 1) return { items: [foreground], total: 1 };
    const background = { call_id: 'background', model: 'model', state: 'reserved', provider_cost_micro_cny: null };
    if (polls >= 3 && records.length === 1) records.push({ elapsed_ms: null });
    if (polls === 3) background.state = 'sent';
    if (polls >= 4) { background.state = 'settled'; background.provider_cost_micro_cny = 2; }
    if (polls >= 5) records[1].elapsed_ms = 3;
    return { items: [foreground, background], total: 2 };
  };
  const result = await drainAccountCharges(api, 'test', records, { timeout_ms: 1000, quiet_window_ms: 10, poll_ms: 1 });
  assert.ok(polls >= 5);
  assert.equal(result.drain.settled, true);
  assert.deepEqual(result.drain.in_flight_charge_ids, []);
  assert.equal(summarizeCharges(result.charges).model.confirmed_cost_micro_cny, 3);
  assert.equal(records[1].elapsed_ms, 3);
});

test('drain has a bounded timeout and retains an unresolved charge as unknown', async () => {
  const pending = { call_id: 'background', state: 'sent', model: 'model', provider_cost_micro_cny: null };
  const result = await drainAccountCharges(async () => ({ items: [pending], total: 1 }), 'test', [],
    { timeout_ms: 15, quiet_window_ms: 10, poll_ms: 1 });
  assert.equal(result.drain.settled, false);
  assert.deepEqual(result.drain.in_flight_charge_ids, ['background']);
  assert.equal(result.charges[0].state, 'sent');
  assert.equal(summarizeCharges(result.charges).model.unknown_cost_attempts, 1);
});

test('per-turn accounting includes background work without recounting the prior turn', () => {
  const charges = [
    { call_id: 'old', model: 'model', state: 'settled', provider_cost_micro_cny: 100 },
    { call_id: 'new-model', model: 'model', state: 'settled', provider_cost_micro_cny: 10 },
    { call_id: 'new-background', model: 'model', state: 'settled', provider_cost_micro_cny: 20 },
    { call_id: 'new-fetch', entry_kind: 'web_fetch', state: 'settled', provider_cost_micro_cny: 5000 },
    { call_id: 'new-unknown', model: 'model', state: 'pending', provider_cost_micro_cny: null },
  ];
  const records = [
    { ordinal: 1, purpose: 'business_loop', usage: { prompt_tokens: 1000, prompt_cache_hit_tokens: 1000, completion_tokens: 200, total_tokens: 1200 } },
    { ordinal: 2, purpose: 'business_loop', usage: { prompt_tokens: 100, prompt_cache_hit_tokens: 80, completion_tokens: 10, total_tokens: 110 } },
    { ordinal: 3, purpose: 'background_profile_review', usage: { prompt_tokens: 50, prompt_cache_hit_tokens: 20, completion_tokens: 10, total_tokens: 60 } },
    { ordinal: 4, purpose: 'business_loop', usage: null },
  ];
  const current = turnAccounting(charges, records, ['old'], 1);
  assert.deepEqual(current.provider_ordinals, [2, 3, 4]);
  assert.equal(current.costs.model.confirmed_cost_micro_cny, 30);
  assert.equal(current.costs.model.unknown_cost_attempts, 1);
  assert.equal(current.costs.web_fetch.confirmed_cost_micro_cny, 5000);
  assert.equal(current.tokens.input_tokens, 150);
  assert.equal(current.tokens.cached_input_tokens, 100);
  assert.equal(current.tokens.uncached_input_tokens, 50);
  assert.equal(current.tokens.output_tokens, 20);
  assert.equal(current.tokens.total_tokens, 170);
  assert.equal(current.tokens.unknown_usage_requests, 1);
  assert.equal(current.tokens_by_purpose.background_profile_review.input_tokens, 50);
});

test('verified peak transitions are contiguous and prices are real CNY per million', () => {
  const { rates } = verifiedRates('http://127.0.0.1:1234', 'deepseek-v4-flash');
  assert.equal(rates.length, 5);
  for (let i = 1; i < rates.length; i++) assert.equal(rates[i - 1].expires_at, rates[i].starts_at);
  const rateAt = local => rates.find(rate => rate.starts_at <= Date.parse(local) / 1000 && Date.parse(local) / 1000 < rate.expires_at);
  assert.equal(rateAt('2026-10-21T08:59:59+08:00').input_micro_cny_per_million, 1_000_000);
  assert.equal(rateAt('2026-10-21T09:00:00+08:00').cached_input_micro_cny_per_million, 40_000);
  assert.equal(rateAt('2026-10-21T12:00:00+08:00').output_micro_cny_per_million, 4_000_000);
  assert.equal(rateAt('2026-10-21T14:00:00+08:00').output_micro_cny_per_million, 8_000_000);
  assert.equal(rateAt('2026-10-21T18:00:00+08:00').input_micro_cny_per_million, 1_000_000);
  assert.throws(() => verifiedRates('p', 'm', '2026-10-22'), /Re-verify/);
  assert.deepEqual(verifiedWebRates().rates.map(rate => [rate.service_kind, rate.cost_micro_cny_per_request]), [['web_search', 5000], ['web_fetch', 5000]]);
});

test('saved provider trace omits imported history and reasoning but keeps current tool receipts', () => {
  const records = [
    { ordinal: 1, request: { model: 'flash', messages: [{ role: 'user', content: 'private prior history' }, { role: 'tool', tool_call_id: 'old', content: 'private prior tool' }] },
      response: { model: 'actual-version', choices: [{ message: { role: 'assistant', content: '', reasoning_content: 'private reasoning', tool_calls: [{ id: 'current', function: { name: 'web_search', arguments: '{"query":"JSONL"}' } }] } }] } },
    { ordinal: 2, request: { model: 'flash', messages: [{ role: 'tool', tool_call_id: 'old', content: 'private prior tool' }, { role: 'tool', tool_call_id: 'current', content: '{"results":[]}' }] }, response: { choices: [] } },
  ];
  const trace = publicProviderTrace(records);
  assert.doesNotMatch(JSON.stringify(trace), /private prior|private reasoning/);
  assert.equal(trace[1].tool_receipts.length, 1);
  assert.deepEqual(trace[1].tool_receipts[0].arguments, { query: 'JSONL' });
});

test('charge summary separates model, search, fetch and retains unknown attempts', () => {
  const report = summarizeCharges([
    { model: 'flash', provider_cost_micro_cny: 100, account_debit_micro_cny: 100 },
    { entry_kind: 'web_search', provider_cost_micro_cny: 5000, account_debit_micro_cny: 5000 },
    { service_kind: 'web_fetch', provider_cost_micro_cny: null },
    { entry_kind: 'web_search', provider_cost_micro_cny: 0, account_debit_micro_cny: 0 },
  ]);
  assert.equal(report.model.confirmed_cost_micro_cny, 100);
  assert.equal(report.web_search.attempts, 2);
  assert.equal(report.web_search.confirmed_cost_micro_cny, 5000);
  assert.equal(report.web_fetch.unknown_cost_attempts, 1);
});
