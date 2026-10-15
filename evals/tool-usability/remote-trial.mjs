import { mkdir, readFile, writeFile, access } from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import { spawn } from 'node:child_process';
import http from 'node:http';
import { resolve } from 'node:path';
import { runCodex } from './codex-client.mjs';

const [directory, executable, mode] = process.argv.slice(2);
const dir = resolve(directory);
await mkdir(dir, { recursive: false });
const hostLog = createWriteStream(resolve(dir, 'host.log'));
const host = spawn(executable, ['mcp9_external_agent_fixture', '--ignored', '--nocapture', '--test-threads=1'], {
  env: { ...process.env, MCP9_AGENT_DIR: dir }, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
});
host.stdout.pipe(hostLog); host.stderr.pipe(hostLog);
const exited = new Promise((done, fail) => { host.on('error', fail); host.on('close', done); });
const deadline = Date.now() + 60_000;
try {
  while (true) {
    try { await access(resolve(dir, 'READY')); break; } catch { /* wait for the actual HTTP fixture */ }
    if (host.exitCode !== null || Date.now() >= deadline) throw new Error('MCP9 fixture did not become ready');
    await new Promise(done => setTimeout(done, 100));
  }
  const connection = JSON.parse(await readFile(resolve(dir, 'reader-mcp.json'), 'utf8')).mcpServers.beta;
  const body = JSON.stringify({ jsonrpc: '2.0', id: 'to14-catalog', method: 'tools/list', params: {} });
  const headers = { ...connection.headers, 'Content-Type': 'application/json', Accept: 'application/json, text/event-stream',
    'MCP-Protocol-Version': '2025-11-25', 'Content-Length': Buffer.byteLength(body) };
  const catalog = await new Promise((done, fail) => {
    const request = http.request(connection.url, { method: 'POST', headers }, response => {
      const chunks = []; response.on('data', c => chunks.push(c));
      response.on('end', () => { try { done(JSON.parse(Buffer.concat(chunks).toString('utf8'))); } catch (e) { fail(e); } });
    });
    request.on('error', fail); request.end(body);
  });
  await writeFile(resolve(dir, 'tools-list.json'), JSON.stringify(catalog, null, 2));
  if (!Array.isArray(catalog.result?.tools)) throw new Error('Actual tools/list capture failed');
  const guide = await readFile(new URL('../../docs/MCP接入.md', import.meta.url), 'utf8');
  const question = '请在 rainwater-notes 中查清北池从实验开始到结束减少了多少升水，并说明南池是否参与。先查找再核实原文；回答附能打开的来源链接和对应短引文。';
  await writeFile(resolve(dir, 'input.json'), JSON.stringify({ case: 'rainwater', question, guide }, null, 2));
  if (mode !== '--catalog-only') await runCodex({ directory: resolve(dir, 'agent'), prompt: `${guide}\n\n阅读问题：${question}\n`, mcp: connection, timeoutMs: 300_000 });
} finally {
  await writeFile(resolve(dir, 'STOP'), 'TO14 trial finished');
  const code = await exited;
  await writeFile(resolve(dir, 'host-exit.json'), JSON.stringify({ exit_code: code }));
}
