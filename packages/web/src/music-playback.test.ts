import { describe, expect, it } from 'vitest';
import { MusicTimeline, musicNoteOpacity, musicQuestion } from './music-playback';
import type { MusicSourceManifest, MusicNoteAlignment } from '../../core/src/music-contract';
describe('music recording timeline', () => {
  it('retains sustained bass while chords enter and removes stale notes after seeks', () => {
    const timeline = new MusicTimeline([{ id: 'bass', start_ms: 0, end_ms: 5000 }, { id: 'chord-a', start_ms: 1000, end_ms: 1200 }, { id: 'chord-b', start_ms: 1000, end_ms: 1300 }, { id: 'next', start_ms: 1800, end_ms: 2100 }]);
    expect(timeline.at(1100).map(note => note.id)).toEqual(['bass', 'chord-a', 'chord-b']);
    expect(timeline.at(2000).map(note => note.id)).toEqual(['bass', 'next']);
    expect(timeline.at(100).map(note => note.id)).toEqual(['bass']);
    expect(timeline.at(5400, 420).map(note => note.id)).toEqual(['bass']);
    expect(timeline.at(5420, 420)).toEqual([]);
  });
  it('freezes the actual repeated occurrence without rewriting score identity', () => {
    const manifest = { coverage: [{ status: 'accepted', granularity: 'note', start_ms: 0, end_ms: 6000 }], recording: { source: { recording_id: 'take', revision: 'r1' }, duration_ms: 6000 }, score: { source: { score_id: 'edition', revision: 'e1' } }, occurrences: [{ occurrence_id: 'first', order: 0, measure: '1', start_ms: 0, end_ms: 2000, score_object_ids: ['note-1'] }, { occurrence_id: 'repeat', order: 1, measure: '1', start_ms: 4000, end_ms: 6000, score_object_ids: ['note-1'] }] } as MusicSourceManifest;
    const focus = ['piano'];
    const question = musicQuestion(manifest, { book_id: 'music', publication_id: 'pub-1' }, 4700, focus, 'draft');
    focus.length = 0; manifest.occurrences[1].score_object_ids.push('late-edit');
    expect(question.occurrence_id).toBe('repeat');
    expect(question.audio).toMatchObject({ start_ms: 4000, end_ms: 6000 });
    expect(question.score?.source_ids).toEqual(['note-1']);
    expect(question.focused_part_ids).toEqual(['piano']);
    expect(musicQuestion(manifest, question.publication, 2500, [], 'gap').occurrence_id).toBeNull();
    manifest.coverage[0].status = 'pending';
    const pending = musicQuestion(manifest, question.publication, 4700, [], 'pending');
    expect(pending.occurrence_id).toBeNull(); expect(pending.score).toBeNull();
    expect(pending.audio.start_ms).toBe(4700);
  });
  it('does not leave a false attack trail when paused or sought before the note', () => {
    const tied = { start_ms: 1000, end_ms: 2000, attack: false } as MusicNoteAlignment;
    expect(musicNoteOpacity(tied, 1500, false)).toBe(1);
    expect(musicNoteOpacity(tied, 2100, false)).toBe(0);
    expect(musicNoteOpacity(tied, 2100, true)).toBeGreaterThan(0);
    expect(musicNoteOpacity(tied, 900, true)).toBe(0);
  });
  it('keeps a reviewed unobserved score tone silent during playback, pauses and seeks', () => {
    const note = { start_ms: 1000, end_ms: 2000, attack: false, unobserved: true } as MusicNoteAlignment;
    for (const playing of [true, false]) for (const ms of [900, 1000, 1500, 2100]) {
      expect(musicNoteOpacity(note, ms, playing)).toBe(0);
    }
  });
});
