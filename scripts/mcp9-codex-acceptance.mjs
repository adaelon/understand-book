// Usage: node scripts/mcp9-codex-acceptance.mjs <fresh-fixture-dir> <codex-executable>
import { spawn, spawnSync } from 'node:child_process';
import { readFile, writeFile, readdir } from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import { resolve } from 'node:path';
import { homedir } from 'node:os';

const [directory, executable] = process.argv.slice(2);
if (!directory || !executable) throw new Error('Expected fixture directory and Codex executable');
const dir = resolve(directory);
const guide = await readFile(new URL('../docs/MCP接入.md', import.meta.url), 'utf8');
const userConfig = await readFile(resolve(homedir(), '.codex/config.toml'), 'utf8');
const model = userConfig.match(/^model\s*=\s*("[^"]+")/m)?.[1];
const version = spawnSync(executable, ['--version'], { encoding: 'utf8', windowsHide: true });
if (version.status !== 0) throw new Error('Cannot identify the Codex client');
await writeFile(resolve(dir, 'client.json'), JSON.stringify({ client: version.stdout.trim(), model: model ? JSON.parse(model) : 'client default' }));
const disabledSkills = [];
for (const root of ['.codex/skills', '.codex/skills/.system', '.agents/skills']) {
  const path = resolve(homedir(), root);
  for (const entry of await readdir(path, { withFileTypes: true })) {
    if (entry.isDirectory() && !entry.name.startsWith('.')) disabledSkills.push(`{path=${JSON.stringify(resolve(path, entry.name).replaceAll('\\', '/'))},enabled=false}`);
  }
}
async function run(name, question) {
  const connection = JSON.parse(await readFile(resolve(dir, `${name}-mcp.json`), 'utf8')).mcpServers.beta;
  const config = [
    'approval_policy="never"', 'project_doc_max_bytes=0', 'web_search="disabled"',
    'features.shell_tool=false', 'features.plugins=false', 'features.apps=false',
    'features.multi_agent=false', 'features.memories=false', 'features.skill_search=false',
    `skills.config=[${disabledSkills.join(',')}]`,
    `mcp_servers.beta.url=${JSON.stringify(connection.url)}`,
    'mcp_servers.beta.http_headers={Host="reader.example"}',
    'mcp_servers.beta.bearer_token_env_var="MCP9_TOKEN"',
    // Authorized only for this isolated acceptance fixture, never written to user config.
    'mcp_servers.beta.default_tools_approval_mode="approve"',
    'tool_output_token_limit=18000',
  ];
  if (model) config.push(`model=${model}`);
  const args = ['exec', '--ignore-user-config', '--ephemeral', '--skip-git-repo-check',
    '--sandbox', 'read-only', '--json', ...config.flatMap(c => ['-c', c]), '-'];
  const output = createWriteStream(resolve(dir, `${name}.jsonl`));
  const errors = createWriteStream(resolve(dir, `${name}.stderr.log`));
  const child = spawn(executable, args, { cwd: dir, windowsHide: true,
    env: { ...process.env, MCP9_TOKEN: connection.headers.Authorization.slice('Bearer '.length) },
    stdio: ['pipe', 'pipe', 'pipe'] });
  child.stdout.pipe(output); child.stderr.pipe(errors);
  child.stdin.end(`${guide}\n\n阅读问题：${question}\n`);
  const timer = setTimeout(() => child.kill(), 300_000);
  const code = await new Promise((done, fail) => { child.on('error', fail); child.on('close', done); });
  clearTimeout(timer);
  if (code !== 0) throw new Error(`${name}: client exited ${code}; inspect saved transcript`);
  console.log(`${name}: transcript saved`);
}
try {
  await run('reader', '请在 rainwater-notes 中查清北池从实验开始到结束减少了多少升水，并说明南池是否参与。先查找再核实原文；回答附能打开的来源链接和对应短引文。');
  await run('exhausted', '请根据 rainwater-notes 解释北池实验结果，附原文来源。若当前无法读取，说明原因和我下一步能做什么。');
} finally {
  await writeFile(resolve(dir, 'STOP'), 'done');
}
