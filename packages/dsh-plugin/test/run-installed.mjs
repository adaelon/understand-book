import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const app = process.argv[2];
if (!app) throw new Error('usage: node test/run-installed.mjs <installed directory>');
const cwd = fileURLToPath(new URL('../', import.meta.url));
const result = spawnSync(path.join(app, 'DeepSeek Harness.exe'), [
  '--import', 'tsx', '--import', './test/installed-loader.mjs', '--test', ...process.argv.slice(3),
  ...(process.argv.length > 3 ? [] : ['test/binding.test.ts', 'test/capability.test.ts']),
], {
  cwd, windowsHide: true, stdio: 'inherit',
  env: { ...process.env, ELECTRON_RUN_AS_NODE: '1',
    UNDERSTAND_BOOK_TEST_NODE: process.execPath,
    UNDERSTAND_BOOK_DSH_INSTALLED_ROOT: path.join(app, 'resources/app.asar/dsh') },
});
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
