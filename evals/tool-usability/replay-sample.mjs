// Diagnostic replay of already sampled decisions; performs no model call.
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
const [directory, index] = process.argv.slice(2);
const name = String(index).padStart(2, '0');
const original = process.env.TO14_REPLAY_FROM;
if (!original) throw new Error('TO14_REPLAY_FROM is required');
const metrics = JSON.parse(await readFile(resolve(original, `sample-${name}/metrics.json`), 'utf8'));
if (!metrics.final || metrics.exit_code !== 0 || metrics.timed_out) throw new Error('No completed decision to replay');
const value = JSON.parse(metrics.final);
await writeFile(resolve(directory, `response-${name}.json`), JSON.stringify({ ...value, usage_total_tokens: 0,
  replayed_from: resolve(original, `sample-${name}/events.jsonl`) }, null, 2));
