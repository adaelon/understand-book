import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
import { parseMusicScore } from './music-score';
import { synthesizeScoreDemonstration } from './music-demonstrations';

it('plays the real Schubert grace-note pairs in score order before the main chord, including at the beginning', () => {
  const xml = readFileSync(new URL('../../../evals/music/assets/schubert-d780-3-noctave.musicxml', import.meta.url), 'utf8');
  const notes = parseMusicScore(xml).notes.filter(note => note.measure === '4' && note.voice === '1:1');
  const { timings, wav } = synthesizeScoreDemonstration(notes, 120);
  const note = (id: number) => timings.find(timing => timing.source_id === `P1-m4-n${id}`)!;
  expect(note(1).start_ms).toBe(0);
  expect(note(2).start_ms).toBeCloseTo(note(1).end_ms);
  expect(note(3).start_ms).toBeCloseTo(note(2).end_ms);
  expect(note(4).start_ms).toBe(note(3).start_ms);
  expect(note(5).start_ms).toBeLessThan(note(6).start_ms);
  expect(note(6).start_ms).toBeCloseTo(note(5).end_ms);
  expect(wav.subarray(0, 4).toString()).toBe('RIFF');
});

it('keeps grace chords together and starts every main voice on the same shifted beat', () => {
  const xml = `<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes>
    <note><grace/><pitch><step>C</step><octave>4</octave></pitch><voice>1</voice></note>
    <note><grace/><chord/><pitch><step>E</step><octave>4</octave></pitch><voice>1</voice></note>
    <note><grace/><pitch><step>D</step><octave>4</octave></pitch><voice>1</voice></note>
    <note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice></note>
    <backup><duration>1</duration></backup>
    <note><pitch><step>C</step><octave>3</octave></pitch><duration>1</duration><voice>2</voice></note>
  </measure></part></score-partwise>`;
  const { timings } = synthesizeScoreDemonstration(parseMusicScore(xml).notes, 120);
  expect(timings.map(note => Math.round(note.start_ms))).toEqual([0, 0, 80, 160, 160]);
  expect(timings.map(note => Math.round(note.end_ms))).toEqual([80, 80, 160, 660, 660]);
});
