import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { musicAlignmentError, reviseMusicAlignment, type MusicAlignmentRevision } from './music-alignment';
import type { MusicSourceManifest } from './music-contract';

const fixture = (): MusicSourceManifest => JSON.parse(readFileSync(new URL('../../../crates/base-schema/tests/fixtures/music.json', import.meta.url), 'utf8'));

describe('music alignment calibration', () => {
  it('changes one repeated occurrence without rewriting the original occurrence or audio source', () => {
    const original = fixture(), occurrence = original.occurrences[1];
    const provenance = { provenance_id: 'review-2', kind: 'human_listening' as const, description: 'Reviewed repeated passage', source_url: null, processor: null };
    const revision: MusicAlignmentRevision = { recording: original.recording.source, score: original.score.source, start_ms: occurrence.start_ms, end_ms: occurrence.end_ms,
      reason: 'Correct the second occurrence onset', occurrences: [occurrence],
      note_alignments: original.note_alignments.filter(n => n.occurrence_id === occurrence.occurrence_id).map(n => ({ ...n, start_ms: n.start_ms + 60, provenance_id: provenance.provenance_id })),
      coverage: [{ ...original.coverage[0], start_ms: occurrence.start_ms, end_ms: occurrence.end_ms, provenance_id: provenance.provenance_id, method: 'manual' }], provenance: [provenance] };
    const result = reviseMusicAlignment(original, revision);
    expect(result.manifest.note_alignments.slice(0, 2)).toEqual(original.note_alignments.slice(0, 2));
    expect(result.manifest.note_alignments[2].start_ms).toBe(original.note_alignments[2].start_ms + 60);
    expect(result.receipt.before.note_alignments[0]).toEqual(original.note_alignments[2]);
    expect(result.manifest.recording).toEqual(original.recording);
    expect(() => reviseMusicAlignment(original, { ...revision, recording: { ...revision.recording, revision: 'different' } })).toThrow('another recording');
    expect(() => reviseMusicAlignment(original, { ...revision, start_ms: occurrence.start_ms + 1 })).toThrow('complete occurrences');
  });

  it('reports timing deviations only against supplied reference anchors', () => {
    expect(musicAlignmentError([{ expected_ms: 1000, actual_ms: 1060 }, { expected_ms: 2000, actual_ms: 2020 }])).toMatchObject({ anchors: 2, mean_absolute_ms: 40, median_absolute_ms: 40, maximum_absolute_ms: 60 });
    expect(() => musicAlignmentError([])).toThrow('independently recorded reference anchors');
  });
  it('refreshes bound event playback windows and their detailed section while retaining original audio evidence and full-work navigation', () => {
    const original = fixture();
    original.events[1].occurrence_ids = ['repeat'];
    original.structure.push({ ...original.structure[0], section_id: 'navigation', coverage: 'navigation_only' });
    const oldAudioEvidence = structuredClone(original.events[1].evidence);
    const revision: MusicAlignmentRevision = { recording: original.recording.source, score: original.score.source, start_ms: 0, end_ms: 4000, reason: 'Reviewed starts of both occurrences',
      occurrences: original.occurrences.map((o, index) => ({ ...o, start_ms: o.start_ms + 100, end_ms: index === 0 ? 2100 : 4000 })),
      note_alignments: original.note_alignments.map(n => ({ ...n, start_ms: n.start_ms + 100, end_ms: n.end_ms + 100 })),
      coverage: [{ ...original.coverage[0], start_ms: 100 }], provenance: [] };
    const result = reviseMusicAlignment(original, revision);
    expect(result.manifest.events[0]).toMatchObject({ occurrence_ids: ['first'], start_ms: 100, end_ms: 2100 });
    expect(result.manifest.events[1]).toMatchObject({ occurrence_ids: ['repeat'], start_ms: 2100, end_ms: 4000 });
    expect(result.manifest.events[1].evidence).toEqual(oldAudioEvidence);
    expect(result.manifest.structure[0]).toMatchObject({ start_ms: 100, end_ms: 4000 });
    expect(result.manifest.structure[1]).toMatchObject({ start_ms: 0, end_ms: 4000 });
    expect(result.receipt.before.events[0].start_ms).toBe(0);
    expect(result.receipt.after.events[0].start_ms).toBe(100);
    const fixedAudio = reviseMusicAlignment(fixture(), revision).manifest;
    expect(fixedAudio.events[1].start_ms).toBe(2000);
    expect(fixedAudio.events[1].evidence).toEqual(oldAudioEvidence);
  });
});
