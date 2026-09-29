import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readFileSync, renameSync, writeFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = fileURLToPath(new URL('../', import.meta.url));
const output = path.resolve(process.argv[2] ?? path.join(repo, 'dist/dsh-release'));
const pkg = path.join(repo, 'packages/dsh-plugin');
const manifest = JSON.parse(readFileSync(path.join(pkg, 'package.json'), 'utf8'));
mkdirSync(output, { recursive: true });
function run(exe, args, options = {}) {
  const result = spawnSync(exe, args, { cwd: repo, windowsHide: true, stdio: 'inherit', ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`Release build failed (${result.status})`);
  return result;
}
run(process.execPath, [path.join(pkg, 'scripts/build.mjs')]);
const npm = process.env.NPM_CLI_JS ?? path.join(path.dirname(process.execPath), 'node_modules/npm/bin/npm-cli.js');
if (!existsSync(npm)) throw new Error('Set NPM_CLI_JS to npm-cli.js (build-time only)');
run(process.execPath, [npm, 'pack', '--ignore-scripts', '--pack-destination', output], { cwd: pkg });
const engineDir = path.join(output, 'engine');
run(process.execPath, [path.join(repo, 'apps/desktop/scripts/build-sidecar.mjs')],
  { env: { ...process.env, UNDERSTAND_BOOK_SIDECAR_OUTPUT_DIR: engineDir } });
renameSync(path.join(engineDir, 'understand-book-build-x86_64-pc-windows-msvc.exe'), path.join(engineDir, 'understand-book-build.exe'));
copyFileSync(path.join(pkg, 'README.md'), path.join(output, 'INSTALL.md'));
writeFileSync(path.join(output, 'release.json'), JSON.stringify({
  version: manifest.version, plugin: `understand-book-dsh-plugin-${manifest.version}.tgz`,
  engine: 'engine/understand-book-build.exe', platform: 'win32', arch: 'x64',
  dsh: '0.1.7-rc.2', status: 'prerelease',
}, null, 2) + '\n');
console.log(`Release: ${output}`);
