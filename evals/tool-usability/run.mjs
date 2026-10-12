import { mkdir, readFile, writeFile, readdir } from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import { spawn } from 'node:child_process';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(new URL('../..', import.meta.url)));
const frozen = resolve(root, 'docs/performance/tool-usability/to14');
const out = resolve(process.argv[2] ?? frozen);
const runs = resolve(out, 'runs');
const binaries = { baseline: process.env.TO14_BASELINE, candidate: process.env.TO14_CANDIDATE };
if (!binaries.baseline || !binaries.candidate || !process.env.TO14_CODEX) throw new Error('Set TO14_BASELINE, TO14_CANDIDATE, TO14_CODEX');
const schedule = [];
for (let repetition = 1; repetition <= 3; repetition++) {
  for (const task of ['rainwater', 'tutor-continue']) {
    for (const version of ['baseline', 'candidate']) schedule.push({ name: `${version}-${task}-${repetition}`, version, task, repetition });
  }
}
for (const task of ['save-note', 'explain-only']) for (const version of ['baseline', 'candidate']) {
  schedule.push({ name: `${version}-${task}-1`, version, task, repetition: 1 });
}
await mkdir(out, { recursive: true });
await mkdir(runs, { recursive: false });
await writeFile(resolve(out, 'schedule.json'), JSON.stringify({ model: 'gpt-6.1-sol', effort: 'medium', planned: schedule, binaries,
  cumulative_token_threshold: 1_500_000, per_task_token_threshold: 120_000 }, null, 2));

async function execute(executable, args, directory, env) {
  const log = createWriteStream(resolve(out, `${directory}.log`));
  const child = spawn(executable, args, { cwd: root, windowsHide: true, env, stdio: ['ignore', 'pipe', 'pipe'] });
  child.stdout.pipe(log); child.stderr.pipe(log);
  const code = await new Promise((done, fail) => { child.on('error', fail); child.on('close', done); });
  return code;
}

async function usageTree(dir) {
  let total = 0, unknown = false;
  for (const f of await readdir(dir, { withFileTypes: true })) {
    if (f.isDirectory()) {
      const child = await usageTree(resolve(dir, f.name)); total += child.total; unknown ||= child.unknown;
    } else if (f.name === 'metrics.json') {
      const m = JSON.parse(await readFile(resolve(dir, f.name), 'utf8'));
      if (m.total_tokens === null) unknown = true;
      else total += m.total_tokens;
    }
  }
  return { total, unknown };
}

for (const task of schedule) {
  const usage = await usageTree(runs);
  if (usage.total >= 1_500_000 || usage.unknown) {
    await writeFile(resolve(out, 'batch-stop.json'), JSON.stringify({ before: task.name, reason: usage.unknown ? 'usage_unavailable' : 'cumulative_token_threshold', ...usage }, null, 2));
    break;
  }
  console.log(`START ${task.name} cumulative_tokens=${usage.total}`);
  const dir = resolve(runs, task.name);
  const env = { ...process.env, TO14_RUN_DIR: dir, TO14_CASE: task.task,
    TO14_SAMPLE_SCRIPT: resolve(root, 'evals/tool-usability/codex-sample.mjs'),
    TO14_ORIGINAL_ANSWER: resolve(frozen, 'materials/original-answer.json') };
  const code = task.task === 'rainwater'
    ? await execute(process.execPath, [resolve(root, 'evals/tool-usability/remote-trial.mjs'), dir, binaries[task.version]], task.name, env)
    : await execute(binaries[task.version], ['to14_resident_trial', '--ignored', '--nocapture', '--test-threads=1'], task.name, env);
  await mkdir(dir, { recursive: true });
  await writeFile(resolve(dir, 'execution.json'), JSON.stringify({ ...task, exit_code: code }, null, 2));
  console.log(`END ${task.name} exit=${code}`);
}
console.log('Fixed schedule ended; no failed task was resampled.');
