import type { MusicCoverage, MusicEvent, MusicNoteAlignment, MusicOccurrence, MusicProvenance, MusicRecordingRef, MusicScoreRef, MusicSourceManifest, MusicStructure } from './music-contract';
import { validateMusicSourceManifest } from './music-contract';
import { bindMusicEventOccurrences } from './music-events';

export interface MusicAlignmentRevision {
  recording: MusicRecordingRef; score: MusicScoreRef;
  start_ms: number; end_ms: number; reason: string;
  occurrences: MusicOccurrence[]; note_alignments: MusicNoteAlignment[];
  coverage: MusicCoverage[]; provenance: MusicProvenance[];
}
export interface MusicAlignmentRevisionReceipt {
  revised_at: string; reason: string; start_ms: number; end_ms: number;
  before: { occurrences: MusicOccurrence[]; note_alignments: MusicNoteAlignment[]; coverage: MusicCoverage[]; events: MusicEvent[]; structure: MusicStructure[] };
  after: { occurrences: MusicOccurrence[]; note_alignments: MusicNoteAlignment[]; coverage: MusicCoverage[]; events: MusicEvent[]; structure: MusicStructure[] };
}

/** Replace whole played measures in a reviewed window, preserving every other accepted interval. */
export function reviseMusicAlignment(manifest: MusicSourceManifest, revision: MusicAlignmentRevision): { manifest: MusicSourceManifest; receipt: MusicAlignmentRevisionReceipt } {
  if (revision.recording.recording_id !== manifest.recording.source.recording_id || revision.recording.revision !== manifest.recording.source.revision
    || revision.score.score_id !== manifest.score.source.score_id || revision.score.revision !== manifest.score.source.revision) throw new Error('Alignment revision belongs to another recording or score edition');
  if (!revision.reason.trim() || revision.start_ms < 0 || revision.end_ms <= revision.start_ms || revision.end_ms > manifest.recording.duration_ms) throw new Error('Alignment revision needs a valid audio interval and a reason');
  const overlaps = (item: { start_ms: number; end_ms: number }) => item.start_ms < revision.end_ms && item.end_ms > revision.start_ms;
  const contained = (item: { start_ms: number; end_ms: number }) => item.start_ms >= revision.start_ms && item.end_ms <= revision.end_ms;
  const beforeOccurrences = manifest.occurrences.filter(overlaps);
  if (beforeOccurrences.some(o => !contained(o)) || revision.occurrences.some(o => !contained(o))) throw new Error('Revise complete occurrences; an alignment window cannot cut a played measure');
  if (revision.note_alignments.some(o => o.start_ms < revision.start_ms || o.start_ms >= revision.end_ms) || revision.coverage.some(o => !contained(o))) throw new Error('Replacement alignment escapes the revision interval');
  const ids = new Set(beforeOccurrences.map(o => o.occurrence_id));
  const beforeNotes = manifest.note_alignments.filter(n => ids.has(n.occurrence_id));
  const beforeCoverage = manifest.coverage.filter(overlaps);
  const retainedCoverage = manifest.coverage.flatMap(c => !overlaps(c) ? [c] : [
    ...(c.start_ms < revision.start_ms ? [{ ...c, end_ms: revision.start_ms }] : []),
    ...(c.end_ms > revision.end_ms ? [{ ...c, start_ms: revision.end_ms }] : []),
  ]);
  const provenance = new Map(manifest.provenance.map(p => [p.provenance_id, p]));
  for (const item of revision.provenance) {
    const previous = provenance.get(item.provenance_id);
    if (previous && JSON.stringify(previous) !== JSON.stringify(item)) throw new Error('A calibration must introduce a new provenance ID instead of replacing its old evidence');
    provenance.set(item.provenance_id, item);
  }
  const occurrences = [...manifest.occurrences.filter(o => !ids.has(o.occurrence_id)), ...revision.occurrences].sort((a, b) => a.start_ms - b.start_ms).map((o, index) => ({ ...o, order: index }));
  const revisedOccurrences = new Map(occurrences.map(o => [o.occurrence_id, o]));
  const changedEvents = new Set<string>();
  const events = bindMusicEventOccurrences(manifest).map(event => {
    if (!event.occurrence_ids?.some(id => ids.has(id))) return event;
    const selected = event.occurrence_ids.map(id => {
      const occurrence = revisedOccurrences.get(id);
      if (!occurrence) throw new Error(`Calibration removed occurrence ${id} used by event ${event.event_id}; preserve its identity or revise the event binding first`);
      return occurrence;
    });
    const start_ms = Math.min(...selected.map(o => o.start_ms)), end_ms = Math.max(...selected.map(o => o.end_ms));
    if (start_ms === event.start_ms && end_ms === event.end_ms) return event;
    changedEvents.add(event.event_id);
    // Original audio evidence remains an immutable recording interval even when its playback window moves.
    return { ...event, start_ms, end_ms };
  });
  const oldEvents = new Map(manifest.events.map(e => [e.event_id, e])), updatedEvents = new Map(events.map(e => [e.event_id, e]));
  const structure = manifest.structure.map(section => {
    if (section.coverage !== 'detailed' || !section.event_ids.some(id => changedEvents.has(id))) return section;
    const before = section.event_ids.map(id => oldEvents.get(id)!);
    // A manually wider context window has its own source meaning; only derived event envelopes follow.
    if (section.start_ms !== Math.min(...before.map(e => e.start_ms)) || section.end_ms !== Math.max(...before.map(e => e.end_ms))) return section;
    const after = section.event_ids.map(id => updatedEvents.get(id)!);
    return { ...section, start_ms: Math.min(...after.map(e => e.start_ms)), end_ms: Math.max(...after.map(e => e.end_ms)) };
  });
  const result = validateMusicSourceManifest({ ...manifest, events, structure, occurrences,
    note_alignments: [...manifest.note_alignments.filter(n => !ids.has(n.occurrence_id)), ...revision.note_alignments].sort((a, b) => a.start_ms - b.start_ms),
    coverage: [...retainedCoverage, ...revision.coverage].sort((a, b) => a.start_ms - b.start_ms), provenance: [...provenance.values()],
  });
  const changedSections = new Set(structure.filter((section, i) => section !== manifest.structure[i]).map(section => section.section_id));
  return { manifest: result, receipt: { revised_at: new Date().toISOString(), reason: revision.reason, start_ms: revision.start_ms, end_ms: revision.end_ms,
    before: { occurrences: beforeOccurrences, note_alignments: beforeNotes, coverage: beforeCoverage, events: manifest.events.filter(e => changedEvents.has(e.event_id)), structure: manifest.structure.filter(s => changedSections.has(s.section_id)) },
    after: { occurrences: revision.occurrences, note_alignments: revision.note_alignments, coverage: revision.coverage, events: events.filter(e => changedEvents.has(e.event_id)), structure: structure.filter(s => changedSections.has(s.section_id)) } } };
}

export function musicAlignmentError(anchors: Array<{ expected_ms: number; actual_ms: number }>) {
  if (!anchors.length) throw new Error('Alignment accuracy requires independently recorded reference anchors');
  const errors = anchors.map(a => Math.abs(a.actual_ms - a.expected_ms)).sort((a, b) => a - b);
  return { anchors: errors.length, mean_absolute_ms: errors.reduce((a, b) => a + b, 0) / errors.length,
    median_absolute_ms: (errors[Math.floor((errors.length - 1) / 2)] + errors[Math.ceil((errors.length - 1) / 2)]) / 2,
    p95_absolute_ms: errors[Math.ceil(errors.length * 0.95) - 1], maximum_absolute_ms: errors.at(-1)! };
}
