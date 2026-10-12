import { spawn, spawnSync } from 'node:child_process';
import { mkdir, readFile, writeFile, readdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { homedir } from 'node:os';

export const MODEL = 'gpt-6.1-sol';
export const EFFORT = 'medium';

export function summarizeEvents(rows) {
  const completed = rows.filter(r => r.type === 'turn.completed');
  const usage = completed.length && completed.every(r => r.usage)
    ? completed.reduce((a, r) => ({
      input_tokens: a.input_tokens + r.usage.input_tokens,
      cached_input_tokens: a.cached_input_tokens + (r.usage.cached_input_tokens ?? 0),
      output_tokens: a.output_tokens + r.usage.output_tokens,
    }), { input_tokens: 0, cached_input_tokens: 0, output_tokens: 0 }) : null;
  return {
    completed: rows.findLast(r => ['turn.completed', 'turn.failed'].includes(r.type))?.type === 'turn.completed',
    recovered_errors: rows.filter(r => r.type === 'error').map(r => r.message),
    usage,
    total_tokens: usage ? usage.input_tokens + usage.output_tokens : null,
    actual_cost: null,
    final: rows.filter(r => r.type === 'item.completed' && r.item?.type === 'agent_message').at(-1)?.item.text ?? null,
  };
}

export async function runCodex({ directory, prompt, schema, mcp, timeoutMs = 180_000 }) {
  await mkdir(directory, { recursive: true });
  const executable = process.env.TO14_CODEX;
  if (!executable) throw new Error('TO14_CODEX must select the fixed Codex executable');
  const version = spawnSync(executable, ['--version'], { encoding: 'utf8', windowsHide: true });
  if (version.status !== 0) throw new Error('Cannot identify Codex');
  const disabled = [];
  for (const root of ['.codex/skills', '.codex/skills/.system', '.agents/skills']) {
    const path = resolve(homedir(), root);
    for (const item of await readdir(path, { withFileTypes: true }).catch(() => [])) {
      if (item.isDirectory() && !item.name.startsWith('.')) disabled.push(`{path=${JSON.stringify(resolve(path, item.name).replaceAll('\\', '/'))},enabled=false}`);
    }
  }
  const settings = ['approval_policy="never"', 'project_doc_max_bytes=0', 'web_search="disabled"',
    'features.shell_tool=false', 'features.plugins=false', 'features.apps=false',
    'features.multi_agent=false', 'features.memories=false', 'features.skill_search=false',
    `skills.config=[${disabled.join(',')}]`, `model_reasoning_effort="${EFFORT}"`,
    'tool_output_token_limit=18000'];
  const env = { ...process.env };
  if (mcp) {
    settings.push(`mcp_servers.beta.url=${JSON.stringify(mcp.url)}`, 'mcp_servers.beta.http_headers={Host="reader.example"}',
      'mcp_servers.beta.bearer_token_env_var="TO14_FIXTURE_TOKEN"', 'mcp_servers.beta.default_tools_approval_mode="approve"');
    env.TO14_FIXTURE_TOKEN = mcp.headers.Authorization.slice('Bearer '.length);
  }
  const args = ['exec', '--ignore-user-config', '--ephemeral', '--skip-git-repo-check', '--sandbox', 'read-only',
    '--model', MODEL, '--json', ...settings.flatMap(s => ['-c', s])];
  if (schema) {
    await writeFile(resolve(directory, 'output-schema.json'), JSON.stringify(schema, null, 2));
    args.push('--output-schema', resolve(directory, 'output-schema.json'));
  }
  args.push('-');
  await writeFile(resolve(directory, 'prompt.txt'), prompt);
  await writeFile(resolve(directory, 'client.json'), JSON.stringify({ client: version.stdout.trim(), model: MODEL,
    reasoning_effort: EFFORT, temperature: null, timeout_ms: timeoutMs, transport: mcp ? 'streamable-http' : 'structured-assistant-turn',
    monetary_budget: 'No paid API configured by this runner; host cost unavailable' }, null, 2));
  const rows = [];
  let pending = '', stderr = '', timedOut = false;
  const started = performance.now();
  const child = spawn(executable, args, { cwd: directory, windowsHide: true, env, stdio: ['pipe', 'pipe', 'pipe'] });
  function line(text) {
    if (!text.trim()) return;
    try {
      const value = JSON.parse(text);
      if (value.item?.type !== 'reasoning') rows.push(value);
    } catch { rows.push({ type: 'invalid_cli_event', text }); }
  }
  child.stdout.setEncoding('utf8'); child.stderr.setEncoding('utf8');
  child.stdout.on('data', chunk => { pending += chunk; const parts = pending.split('\n'); pending = parts.pop(); parts.forEach(line); });
  child.stderr.on('data', chunk => { stderr += chunk; });
  const timer = setTimeout(() => { timedOut = true; child.kill(); }, timeoutMs);
  child.stdin.end(prompt);
  const code = await new Promise((ok, no) => { child.on('error', no); child.on('close', ok); });
  clearTimeout(timer); line(pending);
  const result = { ...summarizeEvents(rows), exit_code: code, timed_out: timedOut, elapsed_ms: performance.now() - started };
  result.completed &&= code === 0 && !timedOut;
  await writeFile(resolve(directory, 'events.jsonl'), rows.map(r => JSON.stringify(r)).join('\n') + '\n');
  await writeFile(resolve(directory, 'stderr.log'), stderr);
  await writeFile(resolve(directory, 'metrics.json'), JSON.stringify(result, null, 2));
  return result;
}

export async function recordedUsage(directory) {
  let total = 0;
  for (const item of await readdir(directory, { withFileTypes: true })) {
    if (!item.isDirectory() || !/^sample-\d+$/.test(item.name)) continue;
    try {
      const value = JSON.parse(await readFile(resolve(directory, item.name, 'metrics.json'), 'utf8'));
      if (value.total_tokens === null) throw new Error('Usage unavailable; no further samples');
      total += value.total_tokens;
    } catch (e) { if (e.code !== 'ENOENT') throw e; }
  }
  return total;
}
