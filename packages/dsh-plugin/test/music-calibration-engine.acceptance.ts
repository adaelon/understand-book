/** Exercises the local calibration command against a copy of the actual curated workspace. */
import assert from 'node:assert/strict';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const source = path.resolve(process.env.UNDERSTAND_BOOK_MUSIC_WORKSPACE ?? '.understand-book/music-schubert');
const engine = path.resolve(process.env.UNDERSTAND_BOOK_MUSIC_ENGINE ?? 'target/music-operator-20261022/engine/understand-book-build-x86_64-pc-windows-msvc.exe');
const root = mkdtempSync(path.join(tmpdir(), 'ub-music-calibration-'));
const installed = path.join(root, 'engine');
cpSync(path.dirname(engine), installed, { recursive: true });
const results = [];
for (const mode of ['source', 'installed']) {
  const workspace = path.join(root, mode); cpSync(source, workspace, { recursive: true });
  assert.equal(existsSync(path.join(workspace, 'publication.json')), false, 'use an unpublished curated source');
  const manifestPath = path.join(workspace, 'music_source_manifest.json');
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8')), occurrence = manifest.occurrences[0];
  const historyPath = path.join(workspace, '.build/music/calibrations.json');
  const previousHistory = existsSync(historyPath) ? JSON.parse(readFileSync(historyPath, 'utf8')).length : 0;
  const audio = readFileSync(path.join(workspace, manifest.recording.media_path));
  const revision = { recording: manifest.recording.source, score: manifest.score.source,
    start_ms: occurrence.start_ms, end_ms: occurrence.end_ms,
    reason: '自动化命令测试：保存既有候选，不代表听核或接纳', occurrences: [occurrence],
    note_alignments: manifest.note_alignments.filter((note: any) => note.occurrence_id === occurrence.occurrence_id),
    coverage: manifest.coverage.filter((item: any) => item.start_ms < occurrence.end_ms && item.end_ms > occurrence.start_ms)
      .map((item: any) => ({ ...item, start_ms: Math.max(item.start_ms, occurrence.start_ms), end_ms: Math.min(item.end_ms, occurrence.end_ms) })),
    provenance: [] };
  const revisionFile = path.join(workspace, 'command-test-revision.json'); writeFileSync(revisionFile, JSON.stringify(revision));
  const program = mode === 'source' ? process.execPath : path.join(installed, path.basename(engine));
  const prefix = mode === 'source' ? ['--import', 'tsx', path.resolve('skills/build/music-calibration-cli.ts')] : ['music-calibrate'];
  const run = () => spawnSync(program, [...prefix, '--book-dir', workspace, '--revision-file', revisionFile], {
    encoding: 'utf8', windowsHide: true, cwd: process.cwd(), maxBuffer: 16 * 1024 * 1024,
  });
  const saved = run(); assert.equal(saved.status, 0, saved.stderr || String(saved.error));
  const output = JSON.parse(saved.stdout); assert.equal(output.manifest.book_id, manifest.book_id);
  assert.deepEqual(output.manifest, JSON.parse(readFileSync(manifestPath, 'utf8')));
  assert.equal(JSON.parse(readFileSync(historyPath, 'utf8')).length, previousHistory + 1);
  assert.ok(audio.equals(readFileSync(path.join(workspace, manifest.recording.media_path))));
  assert.ok(output.manifest.coverage.every((item: any) => item.status !== 'accepted'), 'test does not invent listened acceptance');
  const before = readFileSync(manifestPath), historyBefore = readFileSync(historyPath);
  writeFileSync(path.join(workspace, 'publication.json'), '{}');
  const rejected = run(); assert.notEqual(rejected.status, 0); assert.match(rejected.stderr, /Published music is immutable/);
  assert.equal(rejected.stdout, ''); assert.ok(before.equals(readFileSync(manifestPath))); assert.ok(historyBefore.equals(readFileSync(historyPath)));
  results.push({ mode, workspace, saved: true, history_appended: true, source_audio_preserved: true,
    pending_retained: true, published_rejected: true, rejection_leaves_manifest_and_history_unchanged: true });
}
mkdirSync('docs/performance/music-operator-20261022', { recursive: true });
writeFileSync('docs/performance/music-operator-20261022/calibration-engine-verification.json', JSON.stringify({ status: 'passed', source, installed_engine: path.join(installed, path.basename(engine)), results }, null, 2));
console.log(JSON.stringify({ status: 'passed', root, modes: results.map(result => result.mode) }));
