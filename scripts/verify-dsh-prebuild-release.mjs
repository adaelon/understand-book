import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = fileURLToPath(new URL('../', import.meta.url));
const [releaseArg, appArg, oldArg] = process.argv.slice(2);
if (!releaseArg || !appArg || !oldArg) throw new Error('usage: node scripts/verify-dsh-prebuild-release.mjs <release directory> <DSH installation> <old Engine exe>');
const release = path.resolve(releaseArg), app = path.resolve(appArg);
const work = mkdtempSync(path.join(path.dirname(release), '分发验证 '));
assert.ok(path.relative(repo, work).startsWith('..'), 'Verification must run outside the source repository');
const manifest = JSON.parse(readFileSync(path.join(release, 'release.json'), 'utf8'));
function run(exe, args, options = {}) {
  const result = spawnSync(exe, args, { cwd: work, windowsHide: true, encoding: 'utf8', ...options });
  if (result.error) throw result.error;
  return result;
}
const tar = path.join(process.env.SystemRoot, 'System32/tar.exe');
const extracted = run(tar, ['-xf', path.join(release, manifest.plugin)]);
assert.equal(extracted.status, 0, extracted.stderr);
const pkg = path.join(work, 'package');
const info = JSON.parse(readFileSync(path.join(pkg, 'package.json'), 'utf8'));
assert.equal(info.dsh.bundle.patch, './cordis.patch.yml');
assert.equal(info.dependencies, undefined);
assert.equal(existsSync(path.join(pkg, 'node_modules')), false);
const bundle = readFileSync(path.join(pkg, 'dist/index.js'), 'utf8');
assert.match(bundle, /You execute exactly one Understand Book opaque handoff/);
assert.match(bundle, /dsh_build_capabilities\.v1/);
assert.match(bundle, /automatic_build_executor_session\.v4/);
assert.doesNotMatch(bundle, /from\s+["'](?:\.\.|[A-Z]:|file:)/u);
assert.ok(info.peerDependencies['@deepseek-ai/dsh-agent']);
const bun = process.env.BUN_BINARY ?? (process.env.PATH ?? '').split(path.delimiter)
  .map(dir => path.join(dir, 'node_modules/bun/bin/bun.exe')).find(existsSync);
assert.ok(bun, 'BUN_BINARY is required to compile the verification fixture');
const compiled = run(bun, ['build', path.join(repo, 'packages/dsh-plugin/test/release.probe.ts'),
  '--target=node', '--external=@deepseek-ai/*', `--outfile=${path.join(work, 'release.probe.mjs')}`]);
assert.equal(compiled.status, 0, compiled.stderr);
copyFileSync(path.join(repo, 'packages/dsh-plugin/test/installed-loader.mjs'), path.join(work, 'installed-loader.mjs'));
const result = run(path.join(app, 'DeepSeek Harness.exe'), ['--import', './installed-loader.mjs', '--test', './release.probe.mjs'], {
  env: { ...process.env, ELECTRON_RUN_AS_NODE: '1', NODE_PATH: '', NODE_OPTIONS: '',
    PATH: path.join(process.env.SystemRoot, 'System32'),
    UB_RELEASE_PLUGIN: path.join(pkg, 'dist/index.js'), UB_RELEASE_ENGINE: path.join(release, manifest.engine),
    UB_OLD_ENGINE: path.resolve(oldArg), UNDERSTAND_BOOK_DSH_INSTALLED_ROOT: path.join(app, 'resources/app.asar/dsh') },
  timeout: 180000,
});
writeFileSync(path.join(work, 'verification.log'), result.stdout + result.stderr);
process.stdout.write(result.stdout + result.stderr);
assert.equal(result.status, 0, `See ${path.join(work, 'verification.log')}`);
console.log(`Verified tarball, installed host and Engine with system-only PATH: ${work}`);
