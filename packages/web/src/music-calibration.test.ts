import { describe, expect, it } from 'vitest';
import { calibrationRevision, createCalibrationDraft } from './music-calibration';
import type { MusicSourceManifest } from '../../core/src/music-contract';
const manifest = {
  recording: { source: { recording_id: 'take', revision: '1' }, duration_ms: 7000 }, score: { source: { score_id: 'score', revision: '1' } },
  score_objects: [{ source_id: 'a', rest: false, tie: 'none' }, { source_id: 'b', rest: false, tie: 'stop' }, { source_id: 'rest', rest: true, tie: 'none' }],
  occurrences: [{ occurrence_id: 'first', order: 0, measure: '1', start_ms: 0, end_ms: 2000, score_object_ids: ['a', 'b', 'rest'] }, { occurrence_id: 'repeat', order: 1, measure: '1', start_ms: 4000, end_ms: 6000, score_object_ids: ['a', 'b', 'rest'] }],
  note_alignments: [{ occurrence_id: 'first', score_object_id: 'a', start_ms: 100, end_ms: 1000, attack: true, provenance_id: 'old' }],
} as MusicSourceManifest;
function ready(first = 0) { const draft = createCalibrationDraft(manifest, first, first); draft.reason = 'Candidate correction'; draft.evidence = 'Manual time entry, awaiting listening'; return draft; }
describe('local music calibration revisions', () => {
  it('preserves unobserved tones until a reviewer explicitly confirms an attack', () => {
    const source = structuredClone(manifest);
    source.note_alignments[0].attack = false; source.note_alignments[0].unobserved = true;
    const draft = createCalibrationDraft(source, 0, 0);
    draft.reason = 'Retain evidence'; draft.evidence = 'Existing audio review';
    expect(calibrationRevision(source, draft, 'new').note_alignments[0]).toMatchObject({ unobserved: true, provenance_id: 'old' });
    draft.notes[0].attack = true;
    expect(() => calibrationRevision(source, draft, 'new')).toThrow('复听依据');
    draft.reviewed = true;
    expect(calibrationRevision(source, draft, 'new').note_alignments[0]).toMatchObject({ unobserved: false, attack: true, provenance_id: 'new' });
  });
  it('keeps repeated occurrences distinct and missing tie continuations blank', () => {
    const first = ready(), repeat = ready(1);
    expect(first.notes).toHaveLength(2); expect(first.notes[0].start_ms).toBe(100);
    expect(repeat.notes[0].start_ms).toBeNull(); expect(repeat.notes[1].attack).toBe(false);
    expect(calibrationRevision(manifest, repeat, 'new').note_alignments).toEqual([]);
  });
  it('saves candidates without claiming listening or accepting missing notes', () => {
    const result = calibrationRevision(manifest, ready(), 'new');
    expect(result.provenance[0].kind).toBe('interpretation');
    expect(result.coverage.every(item => item.status === 'pending')).toBe(true);
    expect(result.note_alignments[0].provenance_id).toBe('old');
    const draft = ready(); draft.acceptance = 'note'; draft.reviewed = true;
    expect(() => calibrationRevision(manifest, draft, 'new')).toThrow('还有 1 个音符');
  });
  it('requires listening before acceptance and retains pending note coverage for measure-only acceptance', () => {
    const draft = ready(); draft.acceptance = 'measure';
    expect(() => calibrationRevision(manifest, draft, 'new')).toThrow('复听');
    draft.reviewed = true;
    const result = calibrationRevision(manifest, draft, 'new');
    expect(result.coverage.map(item => [item.granularity, item.status])).toEqual([['measure', 'accepted'], ['note', 'pending']]);
    expect(result.provenance[0].kind).toBe('human_listening');
  });
  it('rejects an outer boundary move and an onset outside its occurrence while allowing a sustained end', () => {
    const draft = ready(); draft.occurrences[0].end_ms = 2100;
    expect(() => calibrationRevision(manifest, draft, 'new')).toThrow('外移边界');
    draft.occurrences[0].end_ms = 2000; draft.notes[0].start_ms = 2000;
    expect(() => calibrationRevision(manifest, draft, 'new')).toThrow();
    draft.notes[0].start_ms = 100; draft.notes[0].end_ms = 3000;
    expect(() => calibrationRevision(manifest, draft, 'new')).toThrow('复听依据');
    draft.reviewed = true;
    expect(calibrationRevision(manifest, draft, 'new').note_alignments[0]).toMatchObject({ end_ms: 3000, provenance_id: 'new' });
    expect(manifest.note_alignments[0].end_ms).toBe(1000);
  });
});
