// Rebuild this curated material from an exact publication while retaining its calibration.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildCuratedMusicWorkspace } from '../../packages/core/src/music-import.ts';

const args = process.argv.slice(2);
const option = key => args.includes(key) ? args[args.indexOf(key) + 1] : null;
assert(option('--source') && option('--out') && option('--curation'), 'Use --source exact-publication --curation existing-curation.json --out new-workspace');
const source = path.resolve(option('--source')), output = path.resolve(option('--out'));
const previous = JSON.parse(fs.readFileSync(path.join(source, 'music_source_manifest.json'), 'utf8'));
const publication = JSON.parse(fs.readFileSync(path.join(source, 'publication.json'), 'utf8')).reference;
const originalCuration = JSON.parse(fs.readFileSync(path.resolve(option('--curation')), 'utf8'));
const review = JSON.parse(fs.readFileSync(fileURLToPath(new URL('./schubert-tonality.json', import.meta.url)), 'utf8'));
assert.equal(previous.book_id, 'music-schubert-d780-3-vetter');
assert.equal(originalCuration.book_id, previous.book_id);
assert(!fs.existsSync(output), 'Use a fresh workspace; do not replace an existing build or publication.');
fs.mkdirSync(output, { recursive: true });
const curation = {
  version: 'music_curation.v1', book_id: previous.book_id, title: previous.title,
  work: { ...previous.work, tonality: review.tonality },
  score: { ...previous.score, musicxml_path: path.join(source, previous.score.musicxml_path) },
  recording: { ...previous.recording, media_path: path.join(source, previous.recording.media_path) },
  rights: previous.rights, parts: previous.parts, guide: fs.readFileSync(path.join(source, 'source.txt'), 'utf8'),
  occurrences: previous.occurrences, note_alignments: previous.note_alignments, coverage: previous.coverage,
  events: previous.events, structure: previous.structure,
  provenance: [...previous.provenance.filter(p => !previous.demonstrations.some(d => d.provenance_id === p.provenance_id)
    && p.provenance_id !== review.provenance.provenance_id), review.provenance],
  demonstration_requests: originalCuration.demonstration_requests,
};
const curationPath = path.join(output, 'input-curation.json');
fs.writeFileSync(curationPath, JSON.stringify(curation, null, 2) + '\n');
const result = await buildCuratedMusicWorkspace({ curationPath, bookDir: output,
  onProgress: progress => console.log(JSON.stringify(progress)) });
for (const key of ['score_objects', 'occurrences', 'note_alignments', 'coverage', 'events', 'structure', 'demonstrations']) {
  assert.deepEqual(result[key], previous[key], `Preserve latest published ${key}`);
}
for (const file of [previous.score.musicxml_path, previous.recording.media_path, 'source.txt', ...previous.demonstrations.map(d => d.media_path)]) {
  assert(fs.readFileSync(path.join(source, file)).equals(fs.readFileSync(path.join(output, file))), `Preserve original bytes: ${file}`);
}
assert(result.score_marks.some(mark => mark.value.kind === 'direction' && mark.value.words.includes('Allegro moderato')));
assert(result.score_marks.some(mark => mark.value.kind === 'key_signature' && mark.value.fifths === -4 && mark.value.encoded_mode === 'major'));
assert.equal(result.work.tonality.mode, 'minor');
const report = { source_publication: publication, output, work_tonality: result.work.tonality, score_marks: result.score_marks,
  preserved: { score_objects: result.score_objects.length, note_alignments: result.note_alignments.length,
    occurrences: result.occurrences.length, events: result.events.length, unobserved_notes: result.note_alignments.filter(n => n.unobserved).length,
    coverage: result.coverage, original_bytes: true, demonstrations: true }, status: 'passed' };
fs.writeFileSync(path.join(output, 'score-facts-rebuild.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({ status: report.status, source_publication: publication, score_marks: result.score_marks.length, output }));
