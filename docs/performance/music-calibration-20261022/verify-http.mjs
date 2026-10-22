import { readFileSync, writeFileSync, rmSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';

const workspace = path.resolve(process.argv[2]), origin = process.argv[3];
const reportPath = path.resolve(process.argv[4]);
const endpoint = '/api/build_workbench/music/';
const context = () => fetch(origin + endpoint + 'context').then(r => r.json());
const before = await context();
assert.ok(before?.manifest);
assert.equal(before.publication, undefined);
const manifest = before.manifest, occurrence = manifest.occurrences[0];
const originalMedia = readFileSync(path.join(workspace, manifest.recording.media_path));
const audio = await fetch(origin + before.audio_url, { headers: { Range: 'bytes=0-31' } });
assert.equal(audio.status, 206);
assert.deepEqual(Buffer.from(await audio.arrayBuffer()), originalMedia.subarray(0, 32));
const page = await fetch(origin + before.score_pages[0].url);
assert.equal(page.status, 200);
assert.ok((await page.text()).includes('<svg'));
const revision = {
  recording: manifest.recording.source, score: manifest.score.source,
  start_ms: occurrence.start_ms, end_ms: occurrence.end_ms,
  reason: 'HTTP transport verification: preserve every candidate timestamp; no listening acceptance',
  occurrences: [occurrence], note_alignments: manifest.note_alignments.filter(note => note.occurrence_id === occurrence.occurrence_id),
  coverage: manifest.coverage.filter(c => c.start_ms < occurrence.end_ms && c.end_ms > occurrence.start_ms)
    .map(c => ({ ...c, start_ms: Math.max(c.start_ms, occurrence.start_ms), end_ms: Math.min(c.end_ms, occurrence.end_ms) })),
  provenance: [],
};
const save = body => fetch(origin + endpoint + 'calibrate', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
const result = await save(revision), body = await result.json();
assert.equal(result.status, 200, JSON.stringify(body));
assert.equal(body.history.length, before.history.length + 1);
assert.ok(body.manifest.coverage.every(c => c.status === 'pending'));
assert.deepEqual((await context()).history, body.history);
const saved = readFileSync(path.join(workspace, 'music_source_manifest.json'));
const invalid = structuredClone(revision);
invalid.note_alignments = [];
invalid.coverage = [{ ...revision.coverage[0], granularity: 'note', status: 'accepted' }];
const failed = await save(invalid);
assert.notEqual(failed.status, 200);
assert.deepEqual(readFileSync(path.join(workspace, 'music_source_manifest.json')), saved);
const wrong = structuredClone(revision); wrong.recording.revision = 'not-the-previewed-recording';
const wrongSource = await save(wrong);
assert.notEqual(wrongSource.status, 200);
assert.deepEqual(readFileSync(path.join(workspace, manifest.recording.media_path)), originalMedia);
const publicationPath = path.join(workspace, 'publication.json');
writeFileSync(publicationPath, '{}');
let immutable;
try {
  assert.equal(await context(), null);
  const forbidden = await save(revision);
  assert.notEqual(forbidden.status, 200);
  const hidden = await fetch(origin + before.audio_url);
  assert.equal(hidden.status, 404);
  immutable = { context: null, save_status: forbidden.status, asset_status: hidden.status };
} finally { rmSync(publicationPath); }
const report = { workspace, origin, title: manifest.title, rights: manifest.rights.map(r => ({ asset_kind: r.asset_kind, distribution: r.distribution })),
  audio_range: { status: audio.status, bytes: 32, matches_original: true }, score_page: { status: page.status, svg: true },
  save: { status: result.status, history_before: before.history.length, history_after: body.history.length, candidate_times_unchanged: true, pending_retained: true, refreshed: true },
  incomplete_note_acceptance_status: failed.status, different_source_status: wrongSource.status, failed_save_preserved_manifest: true, original_audio_unchanged: true,
  publication_immutable: immutable, human_listening_performed: false };
mkdirSync(path.dirname(reportPath), { recursive: true }); writeFileSync(reportPath, JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify(report));
