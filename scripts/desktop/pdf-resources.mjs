import { cpSync, mkdirSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { desktopTarget, repoRoot } from './platform.mjs';

export function preparePdfResources(output) {
  const target = desktopTarget();
  const copied = new Set();
  function copyPackage(name, requireFrom) {
    if (copied.has(name)) return;
    const manifest = requireFrom.resolve(`${name}/package.json`);
    const spec = JSON.parse(readFileSync(manifest, 'utf8'));
    if (spec.os && !spec.os.includes(target.platform)) throw new Error(`${name} does not support ${target.platform}`);
    if (spec.cpu && !spec.cpu.includes(target.arch)) throw new Error(`${name} does not support ${target.arch}`);
    copied.add(name);
    const destination = path.join(output, name);
    mkdirSync(path.dirname(destination), { recursive: true });
    cpSync(path.dirname(manifest), destination, {
      recursive: true, dereference: true,
      filter: source => path.basename(source) !== 'node_modules',
    });
    const dependencyRequire = createRequire(manifest);
    for (const dependency of Object.keys(spec.dependencies ?? {})) copyPackage(dependency, dependencyRequire);
  }
  const require = createRequire(path.join(repoRoot, 'packages/core/package.json'));
  const canvasRequire = createRequire(require.resolve('@napi-rs/canvas/package.json'));
  copyPackage(target.canvas, canvasRequire);
  for (const name of ['@napi-rs/canvas', 'pdfjs-dist', 'tesseract.js', '@tesseract.js-data/eng', '@tesseract.js-data/chi_sim']) {
    copyPackage(name, require);
  }
  console.log(`Packaged PDF runtime for ${target.platform}/${target.arch}: ${copied.size} packages`);
  return [...copied];
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  if (!process.argv[2]) throw new Error('usage: node scripts/desktop/pdf-resources.mjs <node_modules-output>');
  preparePdfResources(path.resolve(process.argv[2]));
}
