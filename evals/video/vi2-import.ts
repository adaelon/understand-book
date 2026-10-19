// Real-source VI2 acceptance. Only task-owned copies are moved; the supplied source stays untouched.
import { copyFileSync, existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { importVideo, readVideoImport } from '../../packages/core/src/video-import';
const sample = JSON.parse(readFileSync('evals/video/samples.json', 'utf8'));
const root = path.resolve(process.argv[2] || 'evals/video/generated/vi2');
mkdirSync(root, { recursive: true });
const external = path.join(root, 'external.mp4'), originalWorkspace = path.join(root, 'workspace');
const workspace = path.join(root, 'moved-workspace');
assert(!existsSync(workspace), 'Use a fresh output directory for a new acceptance run');
copyFileSync(sample.source.path, external);
const start = Date.now();
const imported = await importVideo(external, originalWorkspace);
assert.equal(imported.source?.width, sample.source.width);
assert.equal(imported.source?.height, sample.source.height);
assert.equal(imported.total_bytes, sample.source.size_bytes);
renameSync(external, path.join(root, 'external-moved.mp4'));
renameSync(originalWorkspace, workspace);
const restored = await importVideo(undefined, workspace);
assert.deepEqual(restored, imported);
assert.equal(existsSync(restored.original_path), false);
assert.equal(existsSync(path.join(workspace, 'base.json')), false);
assert.deepEqual(readVideoImport(workspace), imported);
const result = { date: new Date().toISOString(), source_sample: sample.source.id, workspace,
  imported, elapsed_ms: Date.now() - start, external_path_missing: true, workspace_moved: true,
  restored_identity: restored.video, book_created: false, qa_ready: false };
writeFileSync(path.join(root, 'import-verification.json'), JSON.stringify(result, null, 2));
console.log(JSON.stringify(result, null, 2));
