import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const client = new Client({ name: 'code-mcp-project-smoke', version: '1.0.0' });
const transport = new StdioClientTransport({ command: process.execPath,
  args: [fileURLToPath(new URL('./server.mjs', import.meta.url))], stderr: 'inherit' });
try {
  await client.connect(transport);
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content));
    return result.structuredContent;
  };
  const overview = await call('project_overview');
  const memory = await call('list_files', { path: 'crates/memory/src' });
  assert.ok(memory.files.includes('crates/memory/src/lib.rs'));
  const source = await call('read_file', { path: 'packages/code-mcp/server.mjs', start_line: 1, end_line: 5 });
  assert.equal(source.segments.length, 5);
  const docs = await call('read_file', { path: 'docs/架构.md', start_line: 1, end_line: 5 });
  assert.ok(docs.segments.length);
  const search = await call('search_code', { query: 'MemoryStore', path: 'crates/memory/src', limit: 3 });
  assert.ok(search.matches.length);
  const changes = await call('working_changes');
  const tracked = changes.changes.find(p => p.status !== '??' && !p.status.includes('D'));
  if (tracked) await call('working_changes', { path: tracked.path });
  for (const name of ['read_file', 'working_changes']) {
    assert.equal((await client.callTool({ name, arguments: { path: '.env' } })).isError, true);
  }
  console.log(JSON.stringify({ result: 'passed', project: overview.project, visible_file_counts: overview.file_counts,
    source_read: source.path, document_read: docs.path, search_matches: search.matches.length,
    tracked_diff_read: tracked?.path ?? null, private_file_denied: true }, null, 2));
} finally {
  await client.close();
}
