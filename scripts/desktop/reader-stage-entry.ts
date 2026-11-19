// Reader installation provides deterministic source preparation. Semantic stages
// continue through the separately installed Build Engine and its executor.
import { createRequire } from 'node:module';

const args = process.argv.slice(2);
const stageIndex = args.indexOf('--stage');
const stage = stageIndex < 0 ? undefined : args[stageIndex + 1];
if (stage && !['source_reconciliation', 'hybrid_foundation'].includes(stage)) {
  console.error('This stage requires the Understand Book build plugin. Install the plugin and continue the existing preparation task there.');
  process.exit(2);
}
delete process.env.UNDERSTAND_BOOK_SIDECAR_SELF;
const require = createRequire(import.meta.url);
const canvas = require('@napi-rs/canvas');
globalThis.DOMMatrix ??= canvas.DOMMatrix;
globalThis.ImageData ??= canvas.ImageData;
globalThis.Path2D ??= canvas.Path2D;
await import('../../skills/build/workbench-stage-runner');
