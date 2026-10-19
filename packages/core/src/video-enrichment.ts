import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { isDeepStrictEqual } from 'node:util';
import type { VideoCoverage } from './generated/VideoCoverage';
import type { VideoMaterial } from './generated/VideoMaterial';
import { validateVideoCoverage, validateVideoSourceManifest } from './video-contract';
import { publishAutomaticBuildArtifactSet } from './automatic-build-publication';

export interface VideoEnrichmentBatch {
  batch_id: string;
  coverage: VideoCoverage[];
  files: string[];
}
export interface VideoEnrichments {
  version: 'video_enrichments.v1';
  material: VideoMaterial;
  batches: VideoEnrichmentBatch[];
}
export const VIDEO_ENRICHMENTS = 'video_enrichments.json';

/** Rust publishes its BTreeMap in ID order; producers append in completion order.
 * Compare contributions without treating that presentation order as a change. */
export function sameVideoEnrichments(a: VideoEnrichments, b: VideoEnrichments): boolean {
  const ordered = (c: VideoEnrichments) => ({ ...c, batches: [...c.batches].sort((x, y) => x.batch_id.localeCompare(y.batch_id)) });
  return isDeepStrictEqual(ordered(a), ordered(b));
}

/** Called after candidate admission, once per meaningful completed batch. Files are
 * immutable contributions; the importer assembles their cumulative read snapshot. */
export function publishVideoEnrichmentBatch(input: {
  workspace_dir: string; batch_id: string; material: VideoMaterial;
  coverage: VideoCoverage[]; artifacts: Record<string, string | Uint8Array>;
}) {
  const manifest = validateVideoSourceManifest(JSON.parse(readFileSync(path.join(input.workspace_dir, 'video_source_manifest.json'), 'utf8')));
  if (!isDeepStrictEqual(input.material, manifest.material)) throw new Error('Enrichment material changed');
  if (!/^[a-zA-Z0-9_-]+$/.test(input.batch_id)) throw new Error('Invalid enrichment batch ID');
  if (!input.coverage.length || !Object.keys(input.artifacts).length) throw new Error('Enrichment needs coverage and dependencies');
  if (Object.keys(input.artifacts).length > 31) throw new Error('Split enrichment into batches of at most 31 dependencies plus the catalog');
  for (const c of input.coverage) {
    validateVideoCoverage(c, manifest.source);
    if (!c.accepted.length || c.pending.length || c.failed.length || c.capability === 'transcript') throw new Error('Only accepted enrichment enters publication');
  }
  const prefix = `video-enrichments/${input.batch_id}/`;
  const artifacts = Object.fromEntries(Object.entries(input.artifacts).map(([name, bytes]) => {
    if (!name || name.includes('\\') || name.includes(':') || name.split('/').some(p => !p || p === '.' || p === '..')) throw new Error('Invalid enrichment dependency path');
    return [prefix + name, bytes];
  }));
  const file = path.join(input.workspace_dir, VIDEO_ENRICHMENTS);
  const catalog: VideoEnrichments = existsSync(file) ? JSON.parse(readFileSync(file, 'utf8')) : { version: 'video_enrichments.v1', material: manifest.material, batches: [] };
  if (catalog.version !== 'video_enrichments.v1' || !isDeepStrictEqual(catalog.material, manifest.material)) throw new Error('Enrichment catalog material changed');
  const batch = { batch_id: input.batch_id, coverage: input.coverage, files: Object.keys(artifacts).sort() };
  const previous = catalog.batches.find(b => b.batch_id === input.batch_id);
  if (previous) {
    if (!isDeepStrictEqual(previous, batch) || batch.files.some(f => !readFileSync(path.join(input.workspace_dir, f)).equals(Buffer.from(artifacts[f])))) throw new Error('Accepted enrichment batch cannot be replaced');
  } else catalog.batches.push(batch);
  return publishAutomaticBuildArtifactSet({ workspace_dir: input.workspace_dir, stage: 'video_enrichment',
    artifacts: { ...artifacts, [VIDEO_ENRICHMENTS]: JSON.stringify(catalog, null, 2) + '\n' } });
}
