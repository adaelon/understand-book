// Finish only never-started tasks from the original frozen schedule, in order.
// Existing successful, failed, invalid and interrupted samples are never rerun.
import { readFile, readdir, writeFile, mkdir } from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import { spawn } from 'node:child_process';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(new URL('../..', import.meta.url)));
const out = resolve(process.argv[2]);
const stop = JSON.parse(await readFile(resolve(out, 'batch-stop.json'), 'utf8'));
if (stop.reason !== 'cumulative_token_threshold') throw new Error('Only a known token-threshold stop can use this fixed coverage completion');
const schedule = JSON.parse(await readFile(resolve(out, 'schedule.json'), 'utf8'));
const existing = new Set(await readdir(resolve(out, 'runs')));
const pending = schedule.planned.filter(t => !existing.has(t.name));
const cap = 400_000;
await writeFile(resolve(out, 'coverage-addendum.json'), JSON.stringify({ reason: 'original budget ended before all preselected tasks were started',
  created_at: new Date().toISOString(), original_stop: stop, tasks: pending, token_threshold: cap,
  repeats_of_existing_tasks: 0, changed_model_or_product_inputs: false }, null, 2), { flag: 'wx' });
let total = 0;
for (const task of pending) {
  if (total >= cap) break;
  const dir = resolve(out, 'runs', task.name);
  await mkdir(dir);
  const log = createWriteStream(resolve(out, `${task.name}.log`), { flags: 'wx' });
  const env = { ...process.env, TO14_RUN_DIR: dir, TO14_CASE: task.task,
    TO14_SAMPLE_SCRIPT: resolve(root, 'evals/tool-usability/codex-sample.mjs'),
    TO14_ORIGINAL_ANSWER: resolve(root, 'docs/performance/tool-usability/to14/materials/original-answer.json') };
  if (task.task === 'rainwater') throw new Error('Unexpected unstarted remote case; this completion covers only Resident tasks');
  console.log(`START ${task.name} coverage_tokens=${total}`);
  const child = spawn(schedule.binaries[task.version], ['to14_resident_trial', '--ignored', '--nocapture', '--test-threads=1'],
    { cwd: root, env, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  child.stdout.pipe(log); child.stderr.pipe(log);
  const code = await new Promise((done, fail) => { child.on('close', done); child.on('error', fail); });
  await writeFile(resolve(dir, 'execution.json'), JSON.stringify({ ...task, exit_code: code, batch: 'coverage_completion' }, null, 2));
  for (const name of await readdir(dir)) if (/^sample-\d+$/.test(name)) {
    const metrics = JSON.parse(await readFile(resolve(dir, name, 'metrics.json'), 'utf8'));
    if (metrics.total_tokens === null) throw new Error('Usage unavailable; stop coverage completion');
    total += metrics.total_tokens;
  }
  console.log(`END ${task.name} exit=${code}`);
}
await writeFile(resolve(out, 'coverage-usage.json'), JSON.stringify({ total_tokens: total, threshold: cap, actual_cost: null }, null, 2));
