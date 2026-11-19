import type { MusicSourceManifest, MusicNoteAlignment, MusicQuestionContext, MusicEvent } from '../../core/src/music-contract';
import type { PublishedBookRef } from './reader-context';

/** Prefix end bounds retain overlapping sustained notes without scanning the whole score each frame. */
export class MusicTimeline<T extends { start_ms: number; end_ms: number }> {
  readonly entries: T[];
  private ends: number[];
  constructor(entries: readonly T[]) {
    this.entries = [...entries].sort((a, b) => a.start_ms - b.start_ms);
    let end = -Infinity;
    this.ends = this.entries.map(entry => (end = Math.max(end, entry.end_ms)));
  }
  at(ms: number, tail = 0): T[] {
    let lo = 0, hi = this.entries.length;
    while (lo < hi) { const mid = (lo + hi) >>> 1; if (this.entries[mid].start_ms <= ms) lo = mid + 1; else hi = mid; }
    const result: T[] = [];
    for (let i = lo - 1; i >= 0 && this.ends[i] + tail > ms; i--) if (this.entries[i].end_ms + tail > ms) result.push(this.entries[i]);
    return result.reverse();
  }
}
export const MUSIC_PART_COLORS = ['#bd5939', '#398574', '#7869a7', '#b58938', '#4c81ab', '#ac5882'];
export const MUSIC_TRAIL_MS = 420;
export function musicTime(ms: number): string { const s = Math.max(0, Math.floor(ms / 1000)); return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`; }
export function musicNoteOpacity(note: MusicNoteAlignment, ms: number, playing: boolean): number {
  if (note.unobserved) return 0;
  if (ms < note.start_ms || ms >= note.end_ms + MUSIC_TRAIL_MS) return 0;
  if (ms < note.end_ms) return 1;
  return playing ? Math.max(0, 1 - (ms - note.end_ms) / MUSIC_TRAIL_MS) : 0;
}
export function musicQuestion(manifest: MusicSourceManifest, publication: PublishedBookRef, currentMs: number, focused: readonly string[], draftId: string, voices: { part_id: string; voice: string }[] = [], selected?: MusicEvent | null): MusicQuestionContext {
  const ms = Math.max(0, Math.min(Math.floor(currentMs), manifest.recording.duration_ms - 1));
  const aligned = manifest.coverage.some(item => item.status === 'accepted' && item.granularity !== 'phrase' && item.start_ms <= ms && item.end_ms > ms);
  const occurrence = aligned ? new MusicTimeline(manifest.occurrences).at(ms).at(-1) : undefined;
  const ids = selected ? [...new Set(selected.evidence.flatMap(evidence => evidence.score_object_ids))] : occurrence?.score_object_ids ?? [];
  return { draft_id: draftId, publication: { ...publication },
    audio: { recording: { ...manifest.recording.source }, start_ms: selected?.start_ms ?? occurrence?.start_ms ?? ms, end_ms: selected?.end_ms ?? occurrence?.end_ms ?? Math.min(ms + 1000, manifest.recording.duration_ms) },
    score: ids.length ? { score: { ...manifest.score.source }, source_ids: [...ids] } : null,
    occurrence_id: selected ? null : occurrence?.occurrence_id ?? null, focused_part_ids: [...focused], focused_voices: voices.map(voice => ({ ...voice })) };
}
