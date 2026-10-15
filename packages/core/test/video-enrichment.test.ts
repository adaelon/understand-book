import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { expect, it } from 'vitest';
import { publishVideoEnrichmentBatch, VIDEO_ENRICHMENTS } from '../src/video-enrichment';

it('atomically accumulates accepted batches, retries unchanged batches and rejects replacement or wrong material', () => {
  const workspace = mkdtempSync(path.join(tmpdir(), 'vi9-'));
  const fixture = JSON.parse(readFileSync(new URL('../../../crates/base-schema/fixtures/video-contract.json', import.meta.url), 'utf8')).cases[0];
  const manifest = fixture.manifest;
  writeFileSync(path.join(workspace, 'video_source_manifest.json'), JSON.stringify(manifest));
  const make = (batch_id: string) => ({ workspace_dir: workspace, material: manifest.material, batch_id,
    coverage: [{ capability: 'visual_index' as const, accepted: [{ video: manifest.source.video, start_ms: 0, end_ms: 1000 }], pending: [], failed: [] }],
    artifacts: { 'index.json': JSON.stringify({ frames: ['frame.png'] }), 'frame.png': new Uint8Array([1, 2, 3]) } });
  try {
    const first = publishVideoEnrichmentBatch(make('visual-1'));
    publishVideoEnrichmentBatch(make('visual-2'));
    publishVideoEnrichmentBatch(make('visual-1'));
    const saved = JSON.parse(readFileSync(path.join(workspace, VIDEO_ENRICHMENTS), 'utf8'));
    expect(saved.batches.map((b: { batch_id: string }) => b.batch_id)).toEqual(['visual-1', 'visual-2']);
    expect(first.stage).toBe('video_enrichment');
    expect(() => publishVideoEnrichmentBatch({ ...make('visual-1'), artifacts: { 'index.json': 'changed' } })).toThrow('cannot be replaced');
    expect(() => publishVideoEnrichmentBatch({ ...make('wrong'), material: { ...manifest.material, book_id: 'different' } })).toThrow('material changed');
    expect(JSON.parse(readFileSync(path.join(workspace, VIDEO_ENRICHMENTS), 'utf8'))).toEqual(saved);
  } finally { rmSync(workspace, { recursive: true, force: true }); }
});
