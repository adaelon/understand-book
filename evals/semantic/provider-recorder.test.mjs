import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { startProviderRecorder, measuredUsage, requestPurpose, usageByPurpose } from './provider-recorder.mjs';

test('model discovery is forwarded without consuming generation budget or adding missing usage', async () => {
  const listing = { data: [{ id: 'test-model', api_capabilities: { anthropic_messages: { tool_update: 'in-history' } } }] };
  let seen;
  const upstream = http.createServer((req, res) => { seen = [req.method, req.url]; res.end(JSON.stringify(listing)); });
  await new Promise(resolve => upstream.listen(0, '127.0.0.1', resolve));
  const recorder = await startProviderRecorder(`http://127.0.0.1:${upstream.address().port}`, { tokenLimit: 0 });
  try {
    const response = await fetch(recorder.url + '/models');
    assert.deepEqual(await response.json(), listing);
    assert.deepEqual(seen, ['GET', '/models']);
    assert.equal(recorder.records.length, 0);
    assert.equal(recorder.metadataRequests.length, 1);
    assert.equal(recorder.budgetRejections.length, 0);
  } finally { await recorder.stop(); upstream.closeAllConnections(); await new Promise(resolve => upstream.close(resolve)); }
});

test('native Messages preserves wire bytes, tools and cumulative cached usage without saving reasoning or keys', async () => {
  const events = [
    { type: 'message_start', message: { id: 'native', model: 'test-model', usage: { input_tokens: 7, cache_read_input_tokens: 90, cache_creation_input_tokens: 3, output_tokens: 0 } } },
    { type: 'content_block_start', index: 0, content_block: { type: 'thinking', thinking: 'PRIVATE_REASONING' } },
    { type: 'content_block_start', index: 1, content_block: { type: 'tool_use', id: 't1', name: 'music_read', input: {} } },
    { type: 'content_block_delta', index: 1, delta: { type: 'input_json_delta', partial_json: '{"measure":"1"}' } },
    { type: 'message_delta', delta: { stop_reason: 'tool_use' }, usage: { output_tokens: 9 } },
    { type: 'message_stop' },
  ];
  const wire = events.map(event => `data: ${JSON.stringify(event)}\n\n`).join('');
  let received;
  const upstream = http.createServer(async (req, res) => {
    const chunks = []; for await (const chunk of req) chunks.push(chunk);
    received = { body: Buffer.concat(chunks).toString(), beta: req.headers['anthropic-beta'], key: req.headers['x-api-key'] };
    res.writeHead(200, { 'content-type': 'text/event-stream' }); res.end(wire);
  });
  await new Promise(resolve => upstream.listen(0, '127.0.0.1', resolve));
  const recorder = await startProviderRecorder(`http://127.0.0.1:${upstream.address().port}`);
  const body = JSON.stringify({ model: 'test-model', messages: [{ role: 'assistant', content: [{ type: 'thinking', thinking: 'PRIVATE_PRIOR_REASONING' }] }] });
  try {
    const response = await fetch(recorder.url + '/messages', { method: 'POST', body, headers: { 'x-api-key': 'PRIVATE_KEY', 'anthropic-beta': 'updates' } });
    assert.equal(await response.text(), wire);
    assert.deepEqual(received, { body, beta: 'updates', key: 'PRIVATE_KEY' });
    const record = recorder.records[0];
    assert.equal(record.usage.total_tokens, 109);
    assert.equal(record.response_complete, true);
    assert.equal(record.response.choices[0].message.tool_calls[0].function.arguments, '{"measure":"1"}');
    assert(!JSON.stringify(recorder.records).includes('PRIVATE_'));
  } finally { await recorder.stop(); upstream.closeAllConnections(); await new Promise(resolve => upstream.close(resolve)); }
});

test('cost purposes follow system signatures, preserve unknown and missing usage', () => {
  const request = content => ({ messages: [{ role: 'system', content }] });
  assert.equal(requestPurpose(request('source_answer_repair.v3\ncontract')), 'source_repair');
  assert.equal(requestPurpose({ system: [{ type: 'text', text: 'source_answer_repair.v5\nReturn one revised final answer' }] }), 'source_repair');
  assert.equal(requestPurpose(request('You are the resident learning agent. The active turn may include a real reading material')), 'business_loop');
  assert.equal(requestPurpose({ messages: [{ role: 'user', content: 'source_answer_repair.v5' }] }), 'unclassified');
  assert.equal(requestPurpose(request('You are the resident reading agent for the current book.\nfinalization_sampling.v1')), 'finalization');
  assert.equal(requestPurpose({ messages: [{ role: 'user', content: 'source_answer_repair.v3' }] }), 'unclassified');
  assert.equal(requestPurpose(request('unknown')), 'unclassified');
  const costs = usageByPurpose([{ request: request('source_answer_repair.v3'), usage: { total_tokens: 10 } }, { request: request('unknown') }]);
  assert.equal(costs.source_repair.total_tokens, 10);
  assert.equal(costs.unclassified.missing_usage, 1);
  assert.equal(costs.unclassified.total_tokens, null);
  assert.equal(Object.values(costs).reduce((n, p) => n + p.requests, 0), 2);
});

test('experiment token cap records in-flight overshoot and never bills a rejected request', async () => {
  let calls = 0;
  const upstream = http.createServer((_req,res)=>{calls++;res.end('{"usage":{"total_tokens":13}}');});
  await new Promise(resolve=>upstream.listen(0,'127.0.0.1',resolve));
  const recorder=await startProviderRecorder(`http://127.0.0.1:${upstream.address().port}`,{tokenLimit:10});
  try {
    const send=()=>fetch(recorder.url+'/chat/completions',{method:'POST',body:'{}'});
    assert.equal((await send()).status,200);
    assert.equal((await send()).status,402);
    assert.equal(calls,1);
    assert.equal(recorder.records.length,1);
    assert.equal(measuredUsage(recorder.records).total_tokens,13);
    assert.equal(recorder.budgetRejections.length,1);
  } finally {await recorder.stop();upstream.closeAllConnections();await new Promise(resolve=>upstream.close(resolve));}
});

test('recorder preserves tool requests and response bytes, counts all calls, never saves key or reasoning', async () => {
  const received = [];
  const returned = ' {"choices":[{"message":{"role":"assistant","content":"answer","reasoning_content":"PRIVATE_REASONING"}}],"usage":{"total_tokens":17}} ';
  const upstream = http.createServer(async (req, res) => {
    const parts = []; for await (const b of req) parts.push(b);
    received.push({ url: req.url, body: Buffer.concat(parts).toString(), auth: req.headers.authorization });
    res.setHeader('content-type', 'application/json'); res.end(returned);
  });
  await new Promise(resolve => upstream.listen(0, '127.0.0.1', resolve));
  const recorder = await startProviderRecorder(`http://127.0.0.1:${upstream.address().port}/v1`);
  const body = ' {"model":"test","messages":[{"role":"user","content":"question"}],"tools":[{"type":"function","function":{"name":"book.text"}}]} ';
  try {
    for (let i = 0; i < 2; i++) {
      const response = await fetch(recorder.url + '/chat/completions', { method: 'POST', body,
        headers: { 'content-type': 'application/json', authorization: 'Bearer PRIVATE_KEY' } });
      assert.equal(await response.text(), returned);
    }
    assert(received.every(r => r.body === body && r.url === '/v1/chat/completions' && r.auth === 'Bearer PRIVATE_KEY'));
    assert.equal(recorder.records[0].request.tools[0].function.name, 'book.text');
    assert(!JSON.stringify(recorder.records).includes('PRIVATE_'));
    assert.deepEqual(measuredUsage(recorder.records), { requests: 2, total_tokens: 34, missing_usage: 0 });
    assert.equal(measuredUsage([...recorder.records, { error: 'failed' }]).total_tokens, null);
    assert.equal(measuredUsage([]).total_tokens, null);
  } finally {
    await recorder.stop(); upstream.closeAllConnections(); await new Promise(resolve => upstream.close(resolve));
  }
});

test('recorder preserves non-JSON provider errors and reports missing usage', async () => {
  const upstream = http.createServer((_req, res) => { res.writeHead(429, { 'content-type': 'text/plain' }); res.end('quota exceeded'); });
  await new Promise(resolve => upstream.listen(0, '127.0.0.1', resolve));
  const recorder = await startProviderRecorder(`http://127.0.0.1:${upstream.address().port}`);
  try {
    const response = await fetch(recorder.url + '/chat/completions', { method: 'POST', body: '{}' });
    assert.equal(response.status, 429);
    assert.equal(await response.text(), 'quota exceeded');
    assert.deepEqual(measuredUsage(recorder.records), { requests: 1, total_tokens: null, missing_usage: 1 });
  } finally {
    await recorder.stop(); upstream.closeAllConnections(); await new Promise(resolve => upstream.close(resolve));
  }
});

test('recorder retains public SSE output, fragmented tool calls, usage and completion without reasoning', async () => {
  const events = [
    { id: 'chat-stream', model: 'test-model', choices: [{ index: 0, delta: { role: 'assistant', reasoning_content: 'PRIVATE_REASONING' } }] },
    { choices: [{ index: 0, delta: { content: '公开', tool_calls: [{ index: 0, id: 'call-1', type: 'function', function: { name: 'book.text', arguments: '{"lid":' } }] } }] },
    { choices: [{ index: 0, delta: { content: '回答', tool_calls: [{ index: 0, function: { arguments: '"10.9.7.2"}' } }] }, finish_reason: 'tool_calls' }] },
    { choices: [], usage: { prompt_tokens: 11, completion_tokens: 6, total_tokens: 17 } },
  ];
  const returned = events.map(event => `data: ${JSON.stringify(event)}\r\n\r\n`).join('') + 'data: [DONE]\r\n\r\n';
  let received;
  const upstream = http.createServer(async (req, res) => {
    const parts = []; for await (const bytes of req) parts.push(bytes);
    received = Buffer.concat(parts).toString();
    res.writeHead(200, { 'content-type': 'text/event-stream; charset=utf-8' });
    res.end(returned);
  });
  await new Promise(resolve => upstream.listen(0, '127.0.0.1', resolve));
  const recorder = await startProviderRecorder(`http://127.0.0.1:${upstream.address().port}`);
  const body = JSON.stringify({ stream: true, messages: [{ role: 'assistant', content: '', reasoning_content: 'PRIVATE_PRIOR_REASONING' }] });
  try {
    const response = await fetch(recorder.url + '/chat/completions', { method: 'POST', body,
      headers: { authorization: 'Bearer PRIVATE_KEY' } });
    assert.equal(await response.text(), returned);
    assert.equal(received, body);
    const record = recorder.records[0];
    assert.deepEqual(record.response, { id: 'chat-stream', model: 'test-model', choices: [{ index: 0,
      finish_reason: 'tool_calls', message: { role: 'assistant', content: '公开回答',
        tool_calls: [{ id: 'call-1', type: 'function', function: { name: 'book.text', arguments: '{"lid":"10.9.7.2"}' } }] } }] });
    assert.equal(record.response_complete, true);
    assert.equal(record.stream_done, true);
    assert.equal(record.non_json_response, undefined);
    assert.deepEqual(measuredUsage(recorder.records), { requests: 1, total_tokens: 17, missing_usage: 0 });
    assert(!JSON.stringify(record).includes('PRIVATE_'));
  } finally { await recorder.stop(); upstream.closeAllConnections(); await new Promise(resolve => upstream.close(resolve)); }
});

test('recorder keeps partial public SSE output without claiming a completed response', async () => {
  const returned = 'data: {"choices":[{"index":0,"delta":{"content":"partial"}}]}\n\n';
  const upstream = http.createServer((_req, res) => { res.writeHead(200, { 'content-type': 'text/event-stream' }); res.end(returned); });
  await new Promise(resolve => upstream.listen(0, '127.0.0.1', resolve));
  const recorder = await startProviderRecorder(`http://127.0.0.1:${upstream.address().port}`);
  try {
    const response = await fetch(recorder.url + '/chat/completions', { method: 'POST', body: '{}' });
    assert.equal(await response.text(), returned);
    assert.equal(recorder.records[0].response.choices[0].message.content, 'partial');
    assert.equal(recorder.records[0].response_complete, false);
    assert.equal(recorder.records[0].stream_done, false);
    assert.equal(measuredUsage(recorder.records).total_tokens, null);
  } finally { await recorder.stop(); upstream.closeAllConnections(); await new Promise(resolve => upstream.close(resolve)); }
});

test('recorder can drain an already issued request before closing', async () => {
  let entered;
  const ready = new Promise(resolve => { entered = resolve; });
  const upstream = http.createServer((_req, res) => {
    entered(); setTimeout(() => res.end('{"usage":{"total_tokens":9}}'), 100);
  });
  await new Promise(resolve => upstream.listen(0, '127.0.0.1', resolve));
  const recorder = await startProviderRecorder(`http://127.0.0.1:${upstream.address().port}`);
  try {
    const response = fetch(recorder.url + '/chat/completions', { method: 'POST', body: '{}' }).then(r => r.text());
    await ready;
    await recorder.stop(1000);
    await response;
    assert.equal(measuredUsage(recorder.records).total_tokens, 9);
  } finally { upstream.closeAllConnections(); await new Promise(resolve => upstream.close(resolve)); }
});

test('diagnostic interception records scripted calls separately from forwarded provider calls', async () => {
  let forwarded = 0;
  const upstream = http.createServer((_req, res) => {
    forwarded++;
    res.setHeader('content-type', 'application/json');
    res.end('{"choices":[{"message":{"content":"answer"}}],"usage":{"total_tokens":9}}');
  });
  await new Promise(resolve => upstream.listen(0, '127.0.0.1', resolve));
  const recorder = await startProviderRecorder(`http://127.0.0.1:${upstream.address().port}`, {
    intercept: (_request, records) => records.length === 1 ? { choices: [], usage: { total_tokens: 0 } } : null,
  });
  try {
    const send = () => fetch(recorder.url + '/chat/completions', { method: 'POST', body: '{}' });
    assert.equal((await send()).status, 200);
    assert.equal((await send()).status, 200);
    assert.equal(forwarded, 1);
    assert.equal(recorder.records[0].scripted, true);
    assert.equal(measuredUsage(recorder.records.filter(r => !r.scripted)).total_tokens, 9);
  } finally { await recorder.stop(); upstream.closeAllConnections(); await new Promise(resolve => upstream.close(resolve)); }
});

test('request diff observes reasoning before removal and isolates background review traffic', async () => {
  const recorder = await startProviderRecorder('http://unused.invalid', {
    intercept: () => ({ choices: [], usage: { total_tokens: 0 } }),
  });
  const main = { messages: [
    { role: 'system', content: 'You are the resident reading agent for the current book.' },
    { role: 'assistant', content: '', reasoning_content: 'PRIVATE_OLD_REASONING' },
  ] };
  try {
    const send = request => fetch(recorder.url + '/chat/completions', { method: 'POST', body: JSON.stringify(request) });
    await send(main);
    await send({ messages: [{ role: 'system', content: 'You extract durable reader-profile candidates from resident user turns.' }] });
    const changed = structuredClone(main);
    changed.messages[1].reasoning_content = 'PRIVATE_NEW_REASONING';
    await send(changed);
    assert.equal(recorder.records[2].request_diff.previous_ordinal, 1);
    assert.equal(recorder.records[2].request_diff.messages.first_old_difference.category, 'reasoning');
    assert(!JSON.stringify(recorder.records).includes('PRIVATE_'));
  } finally { await recorder.stop(); }
});
