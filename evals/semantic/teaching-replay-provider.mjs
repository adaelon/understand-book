// Instrument the existing full-question replay. Forward native Messages bytes
// unchanged; retain usage and tool calls, never credentials or reasoning text.
import http from 'node:http';
import https from 'node:https';
import { createUnzip } from 'node:zlib';
import { createToolUseCapture, measureMessagesInput } from './teaching-patch-stream.mjs';

export function teachingPurpose(request) {
  const text = value => typeof value === 'string' ? value
    : (value ?? []).filter(block => block.type === 'text').map(block => block.text).join('\n');
  const system = text(request.system) || (request.messages ?? [])
    .filter(message => ['system', 'developer'].includes(message.role)).map(message => text(message.content)).join('\n');
  if (system.includes('Teaching delivery acceptance review.')) return 'tutor_delivery_review';
  if (system.includes('Teaching explanation acceptance review.')) return 'tutor_explanation_review';
  if (system.includes('You are the resident reading agent for the current book.')) return 'outer';
  if (system.startsWith('你是 Understand Book 的教学教练。本次在')) return 'expression_coach';
  if (system.startsWith('你是 Understand Book 的教学教练。')) return 'tutor_coach';
  if (system.startsWith('You extract durable reader-profile candidates')) return 'background_profile_review';
  if (system.includes('Evaluate only the frozen atomic rubric')) return 'tutor_assessment';
  return 'other';
}

export function teachingUsage(rows) {
  const groups = {};
  for (const purpose of new Set(rows.map(row => row.purpose))) {
    const calls = rows.filter(row => row.purpose === purpose);
    const complete = calls.every(row => row.native_usage?.input_tokens != null && row.native_usage?.output_tokens != null);
    const sum = field => complete ? calls.reduce((n, row) => n + (row.native_usage[field] ?? 0), 0) : null;
    const uncached = sum('input_tokens'), cached = sum('cache_read_input_tokens'), created = sum('cache_creation_input_tokens');
    groups[purpose] = { calls: calls.length, usage_complete: complete,
      input_tokens: complete ? uncached + cached + created : null,
      cached_input_tokens: cached, cache_creation_input_tokens: created,
      uncached_input_tokens: uncached, output_tokens: sum('output_tokens'),
      provider_seconds: calls.reduce((n, row) => n + (row.elapsed_ms ?? 0), 0) / 1000,
      failed_requests: calls.filter(row => row.error || row.http_status >= 400).length };
  }
  return groups;
}

export async function startTeachingRecorder(base, model, key, persist, background = 'unavailable') {
  const listingResponse = await fetch(new URL('/models', base), {
    headers: { authorization: `Bearer ${key}` }, signal: AbortSignal.timeout(15000),
  });
  if (!listingResponse.ok) throw new Error(`Model discovery: HTTP ${listingResponse.status}`);
  const listing = await listingResponse.json();
  const exact = listing.data?.find(item => item.id === model);
  const resolved = exact ?? listing.data?.find(item => item.id === 'deepseek-flash');
  if (!resolved?.api_capabilities?.anthropic_messages) throw new Error('Replay requires the configured native Messages capability.');
  const projected = exact ? listing : { ...listing, data: [...listing.data, { ...resolved, id: model }] };
  const records = [], pending = new Set();
  const metadata = { model, resolved_model: resolved.id, projected_alias: !exact,
    api_capabilities: resolved.api_capabilities, background,
    protocol: 'native-messages-full-question.v1' };
  const checkpoint = () => persist({ metadata, records, usage_by_purpose: teachingUsage(records.filter(row => row.forwarded)) });
  const send = (res, value, status = 200) => { res.writeHead(status, { 'content-type': 'application/json' }); res.end(JSON.stringify(value)); };
  const server = http.createServer(async (req, res) => {
    let row;
    try {
      if (req.method === 'GET' && req.url === '/models') { send(res, projected); return; }
      if (req.method !== 'POST' || req.url !== '/messages') throw new Error(`Unsupported replay transport: ${req.method} ${req.url}`);
      const chunks = []; for await (const chunk of req) chunks.push(chunk);
      const body = Buffer.concat(chunks), request = JSON.parse(body.toString('utf8'));
      row = { ordinal: records.length + 1, purpose: teachingPurpose(request), started_at: new Date().toISOString(),
        model: request.model, input_footprint: measureMessagesInput(request, body.length),
        settings: Object.fromEntries(['model', 'max_tokens', 'temperature', 'thinking', 'output_config', 'tool_choice']
          .filter(field => request[field] !== undefined).map(field => [field, request[field]])) };
      records.push(row);
      if (row.purpose === 'background_profile_review' || row.purpose === 'tutor_coach' && background === 'unavailable') {
        row.suppressed = true;
        send(res, { error: { message: 'PAIRED_REPLAY_BACKGROUND_UNAVAILABLE' } }, 503); checkpoint(); return;
      }
      row.forwarded = true; checkpoint();
      console.log(JSON.stringify({ event: 'provider-request', ordinal: row.ordinal, purpose: row.purpose }));
      await new Promise((resolve, reject) => {
        const target = new URL('/anthropic/v1/messages', base), started = performance.now();
        const upstream = (target.protocol === 'https:' ? https : http).request(target,
          { method: req.method, headers: { ...req.headers, host: target.host } }, incoming => {
            row.http_status = incoming.statusCode;
            const capture = createToolUseCapture(incoming.headers['content-type']);
            const observed = ['gzip', 'deflate'].includes(incoming.headers['content-encoding']) ? incoming.pipe(createUnzip()) : incoming;
            observed.on('data', chunk => capture.write(chunk));
            res.writeHead(incoming.statusCode, incoming.headers); incoming.pipe(res);
            observed.once('end', () => {
              const captured = capture.finish();
              row.tool_calls = captured.calls; row.native_usage = captured.usage;
              if (captured.errors.length) row.capture_errors = captured.errors;
              if (captured.providerError) row.provider_error = captured.providerError;
              row.elapsed_ms = performance.now() - started; row.finished_at = new Date().toISOString();
              pending.delete(upstream); checkpoint(); resolve();
            });
            observed.once('error', reject); incoming.once('error', reject);
          });
        pending.add(upstream);
        upstream.once('error', error => { pending.delete(upstream); reject(error); });
        upstream.end(body);
      });
    } catch (error) {
      if (row) row.error = error.message;
      if (!res.headersSent) send(res, { error: { message: 'REPLAY_TRANSPORT_FAILED' } }, 502);
      else res.end();
      checkpoint();
    }
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  checkpoint();
  return { url: `http://127.0.0.1:${server.address().port}`, records,
    async stop() {
      for (const request of pending) request.destroy(new Error('Replay stopped with request in flight'));
      server.closeAllConnections();
      await new Promise(resolve => server.close(resolve)); checkpoint();
    } };
}
