// Bundle the already pinned upstream distribution as a classic global for the
// offline, opaque-origin Author frame. No network or runtime module loading.
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const web = createRequire(path.join(root, 'packages/web/package.json'));
const { build } = createRequire(web.resolve('vite/package.json'))('esbuild');
const output = path.join(root, 'assets/presentation/three-0.180.0');
fs.mkdirSync(output, { recursive: true });
await build({ entryPoints: [path.join(root, 'examples/gear-transmission/vendor/three.module.min.js')],
  outfile: path.join(output, 'three.min.js'), bundle: true, format: 'iife', globalName: 'THREE',
  target: 'es2020', minify: true, legalComments: 'inline' });
fs.copyFileSync(path.join(root, 'examples/gear-transmission/vendor/THREE-LICENSE.txt'), path.join(output, 'LICENSE'));
console.log('Built offline Three.js 0.180.0');
