import { chmodSync, copyFileSync, mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { bunExecutable, desktopTarget, nodeLicense, repoRoot, run } from './platform.mjs';
import { preparePdfResources } from './pdf-resources.mjs';

export function bundleNodeStage(output, entry) {
  const target = desktopTarget();
  mkdirSync(output, { recursive: true });
  const node = `node${target.suffix}`;
  copyFileSync(process.execPath, path.join(output, node));
  if (target.platform !== 'win32') chmodSync(path.join(output, node), 0o755);
  copyFileSync(nodeLicense(), path.join(output, 'NODE-LICENSE.txt'));
  run(bunExecutable(), ['build', entry,
    '--target=node', '--format=esm', '--external=@napi-rs/canvas', '--external=tesseract.js', '--external=pdfjs-dist',
    `--outfile=${path.join(output, 'workbench-stage.mjs')}`]);
  return { target, node };
}

export function buildReaderRuntime(output) {
  const { target, node } = bundleNodeStage(output, path.join(repoRoot, 'scripts/desktop/reader-stage-entry.ts'));
  const packages = preparePdfResources(path.join(output, 'node_modules'));
  writeFileSync(path.join(output, 'runtime.json'), JSON.stringify({
    platform: target.platform, arch: target.arch, node, node_version: process.version,
    runner: 'workbench-stage.mjs', stages: ['source_reconciliation', 'hybrid_foundation'], packages,
  }, null, 2) + '\n');
  console.log(`Reader runtime: ${output}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  if (!process.argv[2]) throw new Error('usage: node scripts/desktop/build-reader-runtime.mjs <output>');
  buildReaderRuntime(path.resolve(process.argv[2]));
}
