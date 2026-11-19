import { copyFileSync, existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = fileURLToPath(new URL('../../', import.meta.url));
const require = createRequire(path.join(repo, 'packages/core/package.json'));
const toolkit = require.resolve('verovio/esm');
const packageRoot = path.resolve(path.dirname(toolkit), '..');
const version = JSON.parse(readFileSync(path.join(packageRoot, 'package.json'), 'utf8')).version;
const commit = '425dd7b6ba26edcecbe345c10b15586d095743d9';
if (version !== '6.3.0') throw new Error('Update the Verovio source revision and notices for the selected npm version');
const target = path.resolve(process.argv[2] ?? path.join(repo, 'dist/build-engine/resources/music/verovio'));
const cache = path.join(repo, 'target/dependencies/verovio-6.3.0');
mkdirSync(target, { recursive:true });mkdirSync(cache, { recursive:true });
for (const name of ['verovio-module.mjs','verovio.mjs']) copyFileSync(path.join(packageRoot, 'dist', name), path.join(target, name));
copyFileSync(path.join(packageRoot, 'package.json'), path.join(target, 'package.json'));
copyFileSync(path.join(repo, 'packages/core/assets/music/NOTICE.txt'), path.resolve(target, '../NOTICE.txt'));
async function obtain(name, url) {
  const file = path.join(cache, name);
  if (!existsSync(file)) {
    const response = await fetch(url);
    if (!response.ok) throw new Error(`Cannot obtain Verovio corresponding source/license: ${response.status} ${url}`);
    writeFileSync(file+'.part', Buffer.from(await response.arrayBuffer()));renameSync(file+'.part',file);
  }
  copyFileSync(file, path.join(target, name));
}
await obtain('COPYING', `https://raw.githubusercontent.com/rism-digital/verovio/${commit}/COPYING`);
await obtain('COPYING.LESSER', `https://raw.githubusercontent.com/rism-digital/verovio/${commit}/COPYING.LESSER`);
await obtain('verovio-6.3.0-source.tar.gz', `https://codeload.github.com/rism-digital/verovio/tar.gz/${commit}`);
writeFileSync(path.join(target, 'SOURCE.md'), `# Verovio 6.3.0 — replaceable music renderer

Understand Book uses Verovio by Laurent Pugin and the Verovio contributors under LGPL-3.0-or-later. The library is unmodified. Copyright and third-party notices are retained in the accompanying source archive; the GPLv3 and LGPLv3 texts are COPYING and COPYING.LESSER.

The runtime files verovio-module.mjs (with its embedded WASM) and verovio.mjs are copied unchanged from npm verovio 6.3.0. Its npm gitHead and toolkit version 6.3.0-425dd7b identify commit ${commit}, tag version-6.3.0, in https://github.com/rism-digital/verovio . Complete upstream source at that revision, including emscripten build scripts, npm wrapper sources, fonts and dependency notices, accompanies this directory as verovio-6.3.0-source.tar.gz.

The Engine imports these two files from disk at runtime. They are not embedded in the Engine executable. Stop the Engine and replace both files with an interface-compatible modified Verovio build to use your changes; restart the same Engine command. No signature or digest check restricts replacement. Modifying the library and reverse engineering the combined work to debug those modifications are permitted under the applicable LGPL terms.

To build a replacement, extract the source, install the upstream documented Emscripten C++20 toolchain, Perl, CMake, make and Node/npm, then run emscripten/buildNpmPackage from the emscripten directory. The shipped non-Humdrum WASM corresponds to buildToolkit -c -w -M -H -m; the ESM wrapper comes from npm run build in emscripten/npm. Copy npm/dist/verovio-module.mjs and npm/dist/verovio.mjs back here. The distributed runtime was taken from the upstream npm release, not locally rebuilt; this notice does not claim byte-for-byte reproducibility with an unspecified newer compiler.

Recordings, editions and reference materials have separate rights recorded in each curated music manifest. Generated score pages are served to Readers; the Verovio runtime remains in the build service.
`, 'utf8');
console.log(`Verovio ${version}: ${target}`);
