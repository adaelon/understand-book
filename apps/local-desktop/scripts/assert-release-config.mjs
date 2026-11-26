import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { desktopTarget } from '../../../scripts/desktop/platform.mjs';
import { assertVoiceRuntime } from '../../../scripts/desktop/build-voice-runtime.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const config = JSON.parse(readFileSync(path.join(root, 'src-tauri/tauri.conf.json'), 'utf8'));
assert.equal(config.identifier, 'app.understandbook.local', 'Local and cloud applications require independent identities.');
assert.equal(config.productName, 'Understand Book Local');
assert.equal(config.bundle.windows.nsis.installMode, 'currentUser');
assert(config.build.beforeBuildCommand.includes('prepare-resources.mjs'), 'Build the local Reader resources before bundling.');
const resourcePaths = Array.isArray(config.bundle.resources) ? config.bundle.resources : Object.keys(config.bundle.resources ?? {});
assert(resourcePaths.some(value => value.startsWith('resources')), 'Local Setup must include its runtime resources.');
const cargo = readFileSync(path.join(root, 'src-tauri/Cargo.toml'), 'utf8');
assert(cargo.includes('name = "UnderstandBookLocal"'), 'The local executable must be distinct from the cloud client.');
const hooks = readFileSync(path.join(root, 'src-tauri/windows/installer-hooks.nsh'), 'utf8');
assert(hooks.includes('Software\\UnderstandBookLocal'), 'Local Setup owns only its independent registration.');
assert(!hooks.includes('"Software\\UnderstandBook"'), 'Local Setup must preserve the cloud client registration.');
assert(!/--install-codex-plugin|--uninstall-owned-codex-plugin/.test(hooks), 'Setup must preserve independently managed plugins.');
for (const icon of config.bundle.icon ?? []) assert(existsSync(path.resolve(root, 'src-tauri', icon)), `Missing icon: ${icon}`);
if (process.argv.includes('--resources')) {
  const target = desktopTarget();
  const resources = path.join(root, 'src-tauri/resources');
  for (const file of [`bin/understand-book${target.suffix}`, `bin/book_mcp${target.suffix}`, 'web-dist/index.html', 'quickstart/base.json', 'quickstart/source.txt',
    `reader-runtime/node${target.suffix}`, 'reader-runtime/workbench-stage.mjs', 'reader-runtime/NODE-LICENSE.txt']) {
    assert(existsSync(path.join(resources, file)), `Missing local release resource: ${file}`);
  }
  const runtime = JSON.parse(readFileSync(path.join(resources, 'reader-runtime/runtime.json'), 'utf8'));
  assert.equal(runtime.platform, target.platform, 'Rebuild resources natively on the target platform.');
  assert.equal(runtime.arch, target.arch, 'Rebuild resources using Node for the target architecture.');
  assert(runtime.packages.includes(target.canvas));
  assert(existsSync(path.join(resources, 'reader-runtime/node_modules', target.canvas, 'package.json')));
  if (target.platform === 'win32') assertVoiceRuntime(path.join(resources, 'voice-runtime'));
}
console.log('Local desktop release configuration ready.');
