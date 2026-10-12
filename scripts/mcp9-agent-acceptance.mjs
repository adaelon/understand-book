// Run against mcp9_external_agent_fixture, using a fresh Claude Code process per account.
// Usage: node scripts/mcp9-agent-acceptance.mjs <fixture-dir> <claude-executable>
import { spawn } from 'node:child_process';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { homedir } from 'node:os';

const [directory, executable] = process.argv.slice(2);
if (!directory || !executable) throw new Error('Expected fixture directory and Claude executable');
const dir = resolve(directory);
const guide = await readFile(new URL('../docs/MCP接入.md', import.meta.url), 'utf8');
// Preserve the user's existing provider credentials and model, without loading plugins or hooks.
const settings = JSON.parse(await readFile(resolve(homedir(), '.claude/settings.json'), 'utf8'));
async function run(name, question) {
  const args = ['-p', '--output-format', 'stream-json', '--verbose', '--no-session-persistence',
    '--strict-mcp-config', '--mcp-config', resolve(dir, `${name}-mcp.json`),
    '--tools', '', '--allowedTools', 'mcp__beta__*', '--permission-mode', 'dontAsk',
    '--disable-slash-commands', '--setting-sources', '', '--no-chrome'];
  if (settings.model) args.push('--model', settings.model);
  const child = spawn(executable, args, { cwd: dir, windowsHide: true,
    env: { ...process.env, ...settings.env, CLAUDE_CODE_DISABLE_AUTO_MEMORY: '1' },
    stdio: ['pipe', 'pipe', 'pipe'] });
  const output = [], errors = [];
  child.stdout.on('data', chunk => output.push(chunk));
  child.stderr.on('data', chunk => errors.push(chunk));
  child.stdin.end(`${guide}\n\n阅读问题：${question}\n`);
  const timer = setTimeout(() => child.kill(), 300_000);
  const code = await new Promise((done, fail) => { child.on('error', fail); child.on('close', done); });
  clearTimeout(timer);
  await writeFile(resolve(dir, `${name}.jsonl`), Buffer.concat(output));
  await writeFile(resolve(dir, `${name}.stderr.log`), Buffer.concat(errors));
  if (code !== 0) throw new Error(`${name}: client exited ${code}; inspect saved transcript`);
  console.log(`${name}: transcript saved`);
}
try {
  await run('reader', '请在 rainwater-notes 中查清北池从实验开始到结束减少了多少升水，并说明南池是否参与。先查找再核实原文；回答附能打开的来源链接和对应短引文。');
  await run('exhausted', '请根据 rainwater-notes 解释北池实验结果，附原文来源。若当前无法读取，说明原因和我下一步能做什么。');
} finally {
  await writeFile(resolve(dir, 'STOP'), 'done');
}
