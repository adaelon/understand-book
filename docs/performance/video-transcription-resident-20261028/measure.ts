/** Local-only whole-build comparison; no provider or network calls. */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import { importVideo } from '../../../packages/core/src/video-import';
import { buildVideoTranscript } from '../../../packages/core/src/video-transcript';

const [label, input, python, modelDir, outputRoot, count = '2'] = process.argv.slice(2);
if (!outputRoot) throw new Error('Usage: node --import tsx measure.ts <label> <input.mp4> <python> <model-dir> <output-root> [repetitions]');
mkdirSync(outputRoot, { recursive: true });
const results = [];
const run = promisify(execFile);
for (let i = 0; i < Number(count); i++) {
  const workspace = path.join(outputRoot, `${label}-${i + 1}`);
  await importVideo(input, workspace);
  const started = performance.now();
  const build = await buildVideoTranscript(workspace, { python, modelDir, chunkMs: 18_000, contextMs: 2_000,
    ...(label === 'baseline' ? { recognize: async (audio: string, output: string, chunk: {id: string}) => {
      const request = path.join(path.dirname(audio), `${chunk.id}.request.json`);
      writeFileSync(request, JSON.stringify({audio, model_dir:path.resolve(modelDir)}));
      await run(python, [fileURLToPath(new URL('./baseline/video-whisper.py', import.meta.url)),request,output], {windowsHide:true});
    } } : {}),
  });
  const elapsed_ms = performance.now() - started;
  const raw = build.chunks.map(c => JSON.parse(readFileSync(path.join(workspace, `transcript/raw/${c.id}.json`), 'utf8')));
  results.push({ workspace, stage: build.stage, failure: build.failure, elapsed_ms,
    chunks: build.chunks.map(c => ({ id: c.id, start_ms: c.start_ms, end_ms: c.end_ms, input_start_ms: c.input_start_ms, input_end_ms: c.input_end_ms, status: c.status })),
    model_load_ms: raw.map(r => r.load_ms), inference_ms: raw.map(r => r.inference_ms),
    segments: raw.map(r => r.segments), text: build.stage === 'ready' ? readFileSync(path.join(workspace, 'source.txt'), 'utf8') : null });
  writeFileSync(path.join(outputRoot, `${label}.json`), JSON.stringify({ label, input, python, modelDir, chunk_ms: 18_000, context_ms: 2_000, api_calls: 0, results }, null, 2) + '\n');
  console.log(JSON.stringify({ label, repetition: i + 1, stage: build.stage, elapsed_ms, model_load_ms: raw.map(r => r.load_ms) }));
}
