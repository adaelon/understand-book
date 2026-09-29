import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
let bun = process.env.BUN_BINARY;
if (!bun && process.platform === 'win32') {
  bun = (process.env.PATH ?? '').split(path.delimiter)
    .map(dir => path.join(dir, 'node_modules/bun/bin/bun.exe')).find(existsSync);
}
if (!bun && process.platform === 'win32') throw new Error('Set BUN_BINARY to the build-time bun.exe');
mkdirSync(path.join(root, 'dist'), { recursive: true });
const result = spawnSync(bun ?? 'bun', ['build', path.join(root, 'src/index.ts'),
  '--target=node', '--format=esm', '--external=@deepseek-ai/*',
  `--outfile=${path.join(root, 'dist/index.js')}`], { cwd: root, stdio: 'inherit', windowsHide: true });
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);
