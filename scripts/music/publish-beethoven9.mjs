import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { validateMusicSourceManifest, validateMusicMaterialSuccessor } from '../../packages/core/src/music-contract.ts';
import { bindMusicEventOccurrences } from '../../packages/core/src/music-events.ts';
import { markdownToBlocks } from '../../packages/core/src/md-adapter.ts';
import { segment } from '../../packages/core/src/segment.ts';
import { publishAutomaticBuildArtifactSet } from '../../packages/core/src/automatic-build-publication.ts';
import { reviseMusicAlignment } from '../../packages/core/src/music-alignment.ts';

const root = path.resolve(process.argv[2] ?? '.understand-book/beethoven9-tennstedt');
for (const movement of ['m1', 'm2', 'm3', 'm4']) {
  const workspace = path.join(root, movement);
  let manifest = validateMusicSourceManifest(JSON.parse(readFileSync(path.join(workspace, '.build/music/import-candidate.json'), 'utf8')));
  manifest.events = bindMusicEventOccurrences(manifest);
  const historyFile = path.join(workspace, '.build/music/calibrations.json');
  if (existsSync(historyFile)) for (const entry of JSON.parse(readFileSync(historyFile, 'utf8'))) manifest = reviseMusicAlignment(manifest, entry.revision).manifest;
  const previous = path.join(workspace, 'music_source_manifest.json');
  if (existsSync(previous)) validateMusicMaterialSuccessor(manifest, JSON.parse(readFileSync(previous, 'utf8')));
  const source = readFileSync(path.join(workspace, 'source.txt'), 'utf8');
  const base = { book_id: manifest.book_id, lid_nodes: segment(markdownToBlocks(source)), graph_nodes: [], graph_edges: [] };
  const { version: _version, pages: _pages, score_objects: _objects, score_marks: _marks, demonstrations: _demos, ...curated } = manifest;
  const curation = { ...curated, version: 'music_curation.v1', guide: source,
    score: { ...manifest.score, musicxml_path: 'score/original.musicxml' },
    recording: { ...manifest.recording, media_path: 'audio/recording.flac' } };
  publishAutomaticBuildArtifactSet({ workspace_dir: workspace, stage: 'music_prebuild', artifacts: {
    'source.txt': source, 'base.json': JSON.stringify(base), 'music_source_manifest.json': JSON.stringify(manifest),
    'music/curation.json': JSON.stringify(curation),
    'music/build-report.json': readFileSync(path.join(workspace, 'music/import-report.json'), 'utf8'),
  } });
  const url = relative => `/@fs/${path.join(workspace, relative).replaceAll('\\', '/')}`;
  writeFileSync(path.join(workspace, 'music-context.json'), JSON.stringify({
    publication: { book_id: manifest.book_id, publication_id: 'local-beethoven9-tennstedt-preview' }, manifest,
    audio_url: url(manifest.recording.media_path), score_pages: manifest.pages.map(page => ({ page_id: page.page_id, url: url(page.path) })), demonstrations: manifest.demonstrations.map(demo => ({ demonstration_id: demo.demonstration_id, url: url(demo.media_path) })),
  }));
  console.log(JSON.stringify({ movement, book_id: manifest.book_id, pages: manifest.pages.length, objects: manifest.score_objects.length, occurrences: manifest.occurrences.length, events: manifest.events.length, published: true }));
}
