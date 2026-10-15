import { spawn, spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync, appendFileSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { cases, output, question, root, save } from './image-experiment.mjs';

// Explicit binary and model: the experiment records the host actually selected.
const [cli, model, ...selected] = process.argv.slice(2);
if (!cli || !model) throw new Error('usage: node image-codex.mjs CODEX_JS MODEL [CASE ...]');
const manifest = JSON.parse(readFileSync(join(output, 'inputs.json'), 'utf8'));
const binary = cli.endsWith('.js') ? process.execPath : resolve(cli);
const prefix = cli.endsWith('.js') ? [resolve(cli)] : [];
const hostVersion = spawnSync(binary, [...prefix, '--version'], { encoding: 'utf8', windowsHide: true }).stdout.trim();
for (const id of selected.length ? selected : Object.keys(cases)) {
  if (!cases[id]) throw new Error(`Unknown case ${id}`);
  const dir = join(output, 'codex', id);
  mkdirSync(dir, { recursive: true });
  // A failed rerun must never inherit the previous run's images or answer.
  for (const name of ['answer.txt', 'mcp-call.json', 'mcp-result.json', 'transport.json', 'run.json']) {
    rmSync(join(dir, name), { force: true });
  }
  const prompt = `Call video_frames exactly once, then answer this image-only question. ${question}`;
  const args = ['exec', '--ignore-user-config', '--skip-git-repo-check', '--ephemeral',
    '-C', dir, '-m', model, '-c', 'model_reasoning_effort="low"',
    '-c', 'mcp_servers.vi0c.command=' + JSON.stringify(process.execPath),
    '-c', 'mcp_servers.vi0c.args=' + JSON.stringify([join(root, 'evals/video/image-experiment.mjs'), 'serve', id, dir]),
    '--json', '-o', join(dir, 'answer.txt'), prompt];
  const request = { host: 'codex-cli', host_version: hostVersion, executable: resolve(cli), model, transport: 'MCP native image content blocks', case: id,
    frames: cases[id].map(frame => manifest.frames[frame]), prompt, args, started_at: new Date().toISOString() };
  save(join(dir, 'request.json'), request);
  const started = Date.now();
  writeFileSync(join(dir, 'events.jsonl'), '');
  writeFileSync(join(dir, 'stderr.txt'), '');
  const child = spawn(binary, [...prefix, ...args], { cwd: dir, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let timedOut = false;
  const timer = setTimeout(() => { timedOut = true; child.kill(); }, 300_000);
  child.stdout.on('data', bytes => { appendFileSync(join(dir, 'events.jsonl'), bytes); });
  child.stderr.on('data', bytes => { appendFileSync(join(dir, 'stderr.txt'), bytes); });
  let exit;
  try {
    exit = await new Promise((resolve, reject) => { child.on('error', reject); child.on('close', resolve); });
  } finally { clearTimeout(timer); }
  save(join(dir, 'run.json'), { exit_code: exit, timed_out: timedOut, elapsed_ms: Date.now() - started, host: 'codex-cli', host_version: hostVersion, model });
  console.log(JSON.stringify({ case: id, exit_code: exit, elapsed_ms: Date.now() - started }));
  if (exit !== 0 || timedOut) process.exitCode = 1;
}
