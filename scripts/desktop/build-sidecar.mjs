import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { bunExecutable, desktopTarget, repoRoot, run } from './platform.mjs';
import { preparePdfResources } from './pdf-resources.mjs';
import { bundleNodeStage } from './build-reader-runtime.mjs';

const target = desktopTarget();
const outputDir = process.env.UNDERSTAND_BOOK_SIDECAR_OUTPUT_DIR
  ? path.resolve(process.env.UNDERSTAND_BOOK_SIDECAR_OUTPUT_DIR)
  : path.join(repoRoot, 'dist/build-engine', target.triple);
const output = path.join(outputDir, `understand-book-build-${target.triple}${target.suffix}`);
mkdirSync(outputDir, { recursive: true });
run(bunExecutable(), ['build', path.join(repoRoot, 'skills/build/sidecar-entry.ts'),
  '--compile', `--target=${target.bun}`, '--loader=.md:text', '--loader=.toml:text', '--loader=.cmd:text', '--loader=.py:text',
  '--external=@napi-rs/canvas', '--external=tesseract.js', `--outfile=${output}`]);
// LGPL renderer remains replaceable and accompanies its corresponding source.
run(process.execPath, [path.join(repoRoot, 'scripts/desktop/prepare-music-resources.mjs'), path.join(outputDir, 'resources/music/verovio')]);
preparePdfResources(path.join(outputDir, 'node_modules'));
bundleNodeStage(path.join(outputDir, 'resources/build'), path.join(repoRoot, 'skills/build/workbench-stage-runner.ts'));
