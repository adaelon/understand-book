#!/usr/bin/env node
// Small, bounded upstream probe. Sends only a fixed synthetic prompt, never book
// or account content. No retries: the report keeps the upstream failure rate.
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';
import { parseArgs } from 'node:util';
import { performance } from 'node:perf_hooks';

const { values } = parseArgs({ options: {
  output: { type: 'string' },
  stages: { type: 'string', default: '2,8,16' },
  requests: { type: 'string', default: '32' },
  rounds: { type: 'string', default: '2' },
} });
if (!values.output) throw new Error('Provide --output for the measured results.');
try { process.loadEnvFile('.env'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
const base = process.env.OPENCODE_BASE_URL?.replace(/\/+$/, '');
const key = process.env.OPENCODE_API_KEY;
const model = process.env.FLUID_LLM_MODEL;
if (!base || !key || !model) throw new Error('Configure OPENCODE_BASE_URL, OPENCODE_API_KEY and FLUID_LLM_MODEL.');
const stages = values.stages.split(',').map(Number);
const count = Number(values.requests);
const rounds = Number(values.rounds);
if (!stages.length || !stages.every(n => Number.isInteger(n) && n >= 1 && n <= 32)
    || !Number.isInteger(count) || count < Math.max(...stages) || count > 64
    || !Number.isInteger(rounds) || rounds < 1 || rounds > 3) {
  throw new Error('Use 1–32 concurrent requests, 1–64 requests per stage and 1–3 rounds.');
}
const body = JSON.stringify({
  model, messages: [{ role: 'user', content: 'Reply with exactly this JSON: {"ok":true}' }],
  max_tokens: 32, temperature: 0, stream: false,
  ...(model.startsWith('deepseek-') ? { thinking: { type: 'disabled' } } : {}),
});
const report = {
  measured_at: new Date().toISOString(), endpoint_origin: new URL(base).origin,
  model, max_output_tokens: 32, requests_per_stage: count, rounds,
  scope: 'Direct Chat Completions API with a fixed short prompt; not Reader end-to-end capacity.',
  warmup: null, stages: [],
};
await mkdir(dirname(values.output), { recursive: true });
const save = () => writeFile(values.output, `${JSON.stringify(report, null, 2)}\n`);
async function call(index) {
  const started = performance.now();
  let status = null;
  try {
    const response = await fetch(`${base}/chat/completions`, {
      method: 'POST', headers: { Authorization: `Bearer ${key}`, 'Content-Type': 'application/json' },
      body, signal: AbortSignal.timeout(45_000),
    });
    status = response.status;
    if (!response.ok) {
      await response.body?.cancel();
      return { index, status, ok: false, ms: performance.now() - started, error: `HTTP_${status}` };
    }
    const data = await response.json();
    const content = data.choices?.[0]?.message?.content;
    const valid = typeof content === 'string' && JSON.parse(content).ok === true
      && data.choices[0].finish_reason === 'stop';
    return { index, status, ok: valid, ms: performance.now() - started,
      input_tokens: data.usage?.prompt_tokens ?? null,
      output_tokens: data.usage?.completion_tokens ?? null,
      cached_input_tokens: data.usage?.prompt_cache_hit_tokens ?? null,
      ...(valid ? {} : { error: 'INVALID_OUTPUT' }) };
  } catch (error) {
    return { index, status, ok: false, ms: performance.now() - started,
      error: error.name === 'TimeoutError' ? 'TIMEOUT' : 'TRANSPORT_OR_INVALID_JSON' };
  }
}
report.warmup = await call(-1);
await save();
if (!report.warmup.ok) {
  console.log(JSON.stringify({ warmup_failed: report.warmup.error, status: report.warmup.status }));
  process.exitCode = 1;
} else {
  outer: for (let round = 1; round <= rounds; round++) {
    for (const concurrency of stages) {
      let cursor = 0;
      const results = [];
      const started = performance.now();
      await Promise.all(Array.from({ length: concurrency }, async () => {
        while (cursor < count) results.push(await call(cursor++));
      }));
      const elapsed = performance.now() - started;
      results.sort((a, b) => a.index - b.index);
      const times = results.map(r => r.ms).sort((a, b) => a - b);
      const successful = results.filter(r => r.ok).length;
      const summary = {
        round, concurrency, sent: results.length, successful, failed: results.length - successful,
        elapsed_ms: elapsed, requests_per_second: successful / (elapsed / 1000),
        median_ms: times[Math.ceil(times.length * 0.5) - 1],
        p95_ms: times[Math.ceil(times.length * 0.95) - 1],
      };
      report.stages.push({ ...summary, requests: results });
      await save();
      console.log(JSON.stringify(summary));
      if (successful !== count) { process.exitCode = 1; break outer; }
    }
  }
}
