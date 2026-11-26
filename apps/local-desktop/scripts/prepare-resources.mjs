import { cpSync, mkdirSync, rmSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { desktopTarget, repoRoot, run } from '../../../scripts/desktop/platform.mjs';
import { buildReaderRuntime } from '../../../scripts/desktop/build-reader-runtime.mjs';
import { buildLearningCli } from '../../../scripts/desktop/build-learning-cli.mjs';
import { buildVoiceRuntime } from '../../../scripts/desktop/build-voice-runtime.mjs';

const target = desktopTarget();
const appRoot = fileURLToPath(new URL('../', import.meta.url));
const resourceDir = path.join(appRoot, 'src-tauri/resources');
const webRoot = path.join(repoRoot, 'packages/web');
const require = createRequire(path.join(webRoot, 'package.json'));
const env = { ...process.env, VITE_MULTI_USER: '0', VITE_LOCAL_DEBUG: '0' };
run(process.execPath, [require.resolve('vue-tsc/bin/vue-tsc.js'), '--noEmit', '-p', 'tsconfig.local.json'], { cwd: webRoot, env });
run(process.execPath, [path.join(path.dirname(require.resolve('vite/package.json')), 'bin/vite.js'),
  'build', '--outDir', 'dist-local-desktop'], { cwd: webRoot, env });
mkdirSync(resourceDir, { recursive: true });
// Replace only generated bundle directories; stale Vite chunks must not survive an upgrade build.
for (const name of ['web-dist', 'quickstart', 'reader-runtime', 'voice-runtime']) {
  const generated = path.resolve(resourceDir, name);
  if (path.dirname(generated) !== path.resolve(resourceDir)) throw new Error('Generated resource path left the bundle directory.');
  rmSync(generated, { recursive: true, force: true });
}
cpSync(path.join(webRoot, 'dist-local-desktop'), path.join(resourceDir, 'web-dist'), { recursive: true });
run(process.execPath, ['--import', 'tsx', path.join(repoRoot, 'examples/quickstart/create.ts'), path.join(resourceDir, 'quickstart')]);
buildReaderRuntime(path.join(resourceDir, 'reader-runtime'));
if (target.platform === 'win32') buildVoiceRuntime(path.join(resourceDir, 'voice-runtime'));
buildLearningCli(path.join(resourceDir, 'bin'));
run(process.execPath, [path.join(appRoot, 'scripts/assert-release-config.mjs'), '--resources']);
