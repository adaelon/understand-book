import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { validateMusicSourceManifest, validateMusicMaterialSuccessor } from '../../packages/core/src/music-contract.ts';
import { markdownToBlocks } from '../../packages/core/src/md-adapter.ts';
import { segment } from '../../packages/core/src/segment.ts';
import { publishAutomaticBuildArtifactSet } from '../../packages/core/src/automatic-build-publication.ts';

const workspace = path.resolve(process.argv[2] ?? '.understand-book/erlkoenig/workspace');
const manifest = validateMusicSourceManifest(JSON.parse(readFileSync(path.join(workspace, '.build/music/import-candidate.json'), 'utf8')));
const previous = path.join(workspace, 'music_source_manifest.json');
if (existsSync(previous)) validateMusicMaterialSuccessor(manifest, JSON.parse(readFileSync(previous, 'utf8')));
const guide = readFileSync(path.join(workspace, 'source.txt'), 'utf8');
const base = { book_id: manifest.book_id, lid_nodes: segment(markdownToBlocks(guide)), graph_nodes: [], graph_edges: [] };
publishAutomaticBuildArtifactSet({ workspace_dir: workspace, stage: 'music_prebuild', artifacts: {
  'source.txt': guide, 'base.json': JSON.stringify(base), 'music_source_manifest.json': JSON.stringify(manifest),
  'music/build-report.json': readFileSync(path.join(workspace, 'music/build-report.json'), 'utf8'),
} });
console.log(JSON.stringify({ book_id: manifest.book_id, pages: manifest.pages.length, events: manifest.events.length, indexed_notes: manifest.score_objects.length, prepared: true }));
