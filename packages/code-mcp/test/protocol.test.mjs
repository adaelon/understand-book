import test from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { fixture } from './fixture.mjs';

test('real stdio MCP client initializes, discovers and calls five read-only tools', async t => {
  const { root, write } = await fixture(t);
  const client = new Client({ name: 'code-mcp-test', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath,
    args: [fileURLToPath(new URL('../server.mjs', import.meta.url)), '--root', root], stderr: 'pipe' });
  t.after(() => client.close());
  await client.connect(transport);
  const { tools } = await client.listTools();
  assert.deepEqual(tools.map(t => t.name).sort(), ['list_files', 'project_overview', 'read_file', 'search_code', 'working_changes']);
  assert.ok(tools.every(t => t.annotations.readOnlyHint && !t.annotations.destructiveHint && !t.annotations.openWorldHint));
  for (const [name, args] of [
    ['project_overview', {}], ['list_files', {}], ['search_code', { query: 'original' }],
    ['read_file', { path: 'packages/demo/src/main.ts' }], ['working_changes', {}],
  ]) {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, name);
    assert.ok(result.structuredContent.observed_at);
  }
  for (const args of [{ path: '.env' }, { path: '../README.md' }, { path: 'README.md', start_line: 0 }, { path: 'README.md', command: 'anything' }]) {
    assert.equal((await client.callTool({ name: 'read_file', arguments: args })).isError, true);
  }
  assert.equal((await client.callTool({ name: 'execute', arguments: {} })).isError, true);
  await write('packages/demo/src/main.ts', Array.from({ length: 250 }, (_, i) => `updated line ${i}`).join('\n'));
  const diff = await client.callTool({ name: 'working_changes', arguments: { path: 'packages/demo/src/main.ts' } });
  assert.ok(diff.structuredContent.next);
  const continuation = await client.callTool({ name: 'working_changes', arguments: { path: 'packages/demo/src/main.ts', ...diff.structuredContent.next } });
  assert.ok(!continuation.isError);
  assert.equal(continuation.structuredContent.next, null);
});
