// Run against the QA copies after saving through the calibration browser.
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import assert from 'node:assert/strict';
const { buildCuratedMusicWorkspace, readMusicManifest } = await import(pathToFileURL(path.resolve('packages/core/src/music-import.ts')).href);
const results = [];
for (const name of ['schubert', 'beethoven']) {
  const bookDir = path.resolve(`.understand-book/music-calibration-qa/${name}`);
  const curationPath = path.resolve(`evals/music/${name}-curation.json`);
  const before = readMusicManifest(bookDir), historyPath = path.join(bookDir, '.build/music/calibrations.json');
  const history = readFileSync(historyPath), media = readFileSync(path.join(bookDir, before.recording.media_path));
  assert.ok(JSON.parse(history).some(item => item.revision.reason.includes('界面流程验证')), 'Expected an actual browser save in this QA copy');
  const start = Date.now();
  const after = await buildCuratedMusicWorkspace({ curationPath, bookDir });
  for (const key of ['occurrences', 'note_alignments', 'coverage', 'events', 'structure']) assert.deepEqual(after[key], before[key], key);
  assert.deepEqual(readFileSync(historyPath), history);
  assert.deepEqual(readFileSync(path.join(bookDir, after.recording.media_path)), media);
  assert.ok(after.coverage.every(item => item.status === 'pending'));
  results.push({ work: name, source_curation: curationPath, workspace: bookDir, elapsed_ms: Date.now() - start,
    browser_revision_replayed: true, history_count: JSON.parse(history).length, history_unchanged: true,
    occurrences_unchanged: true, note_alignments_unchanged: true, coverage_unchanged: true, events_unchanged: true,
    structure_unchanged: true, original_audio_unchanged: true, actual_recording_alignment_status: 'pending' });
}
writeFileSync('docs/performance/music-calibration-20261022/rebuild-replay.json', JSON.stringify({ status: 'passed', results }, null, 2) + '\n');
console.log(JSON.stringify(results));
