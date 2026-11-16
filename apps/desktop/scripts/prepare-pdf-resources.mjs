// Native Canvas and Tesseract's worker/WASM cannot live only inside a Bun executable.
import { cpSync, mkdirSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const output = path.resolve(process.argv[2]);
const copied = new Set();
function copyPackage(name, requireFrom) {
  if (copied.has(name)) return;
  const manifest = requireFrom.resolve(`${name}/package.json`);
  const spec = JSON.parse(readFileSync(manifest, 'utf8'));
  if (spec.os && !spec.os.includes(process.platform)) return;
  if (spec.cpu && !spec.cpu.includes(process.arch)) return;
  copied.add(name);
  const destination = path.join(output, name);
  mkdirSync(path.dirname(destination), { recursive: true });
  cpSync(path.dirname(manifest), destination, { recursive: true });
  const dependencyRequire = createRequire(manifest);
  for (const dependency of Object.keys(spec.dependencies ?? {})) copyPackage(dependency, dependencyRequire);
}
const require = createRequire(path.join(root, 'packages/core/package.json'));
for (const name of ['@napi-rs/canvas', '@napi-rs/canvas-win32-x64-msvc', 'pdfjs-dist', 'tesseract.js', '@tesseract.js-data/eng', '@tesseract.js-data/chi_sim']) copyPackage(name, require);
console.log(`Packaged PDF runtime: ${copied.size} packages`);
