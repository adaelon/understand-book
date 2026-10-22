import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const read = (relative) => readFileSync(path.join(repo, relative), 'utf8');
const manifest = JSON.parse(read('plugins/understand-book/.codex-plugin/plugin.json'));
assert.match(manifest.description, /Operator-only/);
for (const [file, setting] of [['start-build-executor-mcp.cmd','UNDERSTAND_BOOK_BUILD_EXE'],['start-video.cmd','UNDERSTAND_BOOK_BUILD_EXE'],['start-book-mcp.cmd','UNDERSTAND_BOOK_MCP_BIN']]) {
  const source = read(`plugins/understand-book/scripts/${file}`);
  assert(source.includes(setting));
  assert(!source.includes('reg.exe'), 'Maintenance tools must not infer sidecars from cloud Setup');
  assert(!/install Understand Book Setup/i.test(source), 'Missing maintenance tools must not recommend cloud Setup');
}
const config = JSON.parse(read('apps/desktop/src-tauri/tauri.conf.json'));
assert(!(config.bundle.externalBin?.length));
console.log('Cloud Setup and explicit maintenance-plugin entry points are consistent.');
