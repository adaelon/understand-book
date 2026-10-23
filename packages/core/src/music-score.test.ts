import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { parseMusicScore, renderMusicScore } from './music-score';
import { synthesizeScoreDemonstration } from './music-demonstrations';

export const selectedScore = `<?xml version="1.0"?><score-partwise version="4.0"><work><work-title>Selected phrase</work-title></work><part-list><score-part id="P1"><part-name>Clarinet</part-name><score-instrument id="I1"><instrument-name>Clarinet</instrument-name></score-instrument><midi-instrument id="I1"><midi-channel>1</midi-channel><midi-program>72</midi-program></midi-instrument></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>4</divisions><time><beats>2</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef><transpose><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>quarter</type><tie type="start"/></note><note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>quarter</type></note><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>quarter</type><tie type="stop"/></note><backup><duration>8</duration></backup><note><rest/><duration>8</duration><voice>2</voice><type>half</type></note></measure><measure number="2"><note><grace slash="yes"/><pitch><step>D</step><octave>4</octave></pitch><voice>1</voice><type>eighth</type></note><note><pitch><step>E</step><octave>4</octave></pitch><duration>8</duration><voice>1</voice><type>half</type></note><barline location="right"><repeat direction="backward"/></barline></measure></part></score-partwise>`;

describe('curated MusicXML', () => {
  it('retains Schubert key encoding, meter and tempo as source-anchored score marks', () => {
    const original = readFileSync(new URL('../../../evals/music/assets/schubert-d780-3-noctave.musicxml', import.meta.url), 'utf8');
    const parsed = parseMusicScore(original);
    expect(parsed.musicxml).toBe(original);
    expect(parsed.marks.slice(0, 3).map(mark => mark.value)).toEqual([
      { kind: 'key_signature', fifths: -4, encoded_mode: 'major' },
      { kind: 'time_signature', components: [{ beats: '2', beat_type: 4 }], symbol: null },
      { kind: 'direction', words: ['Allegro moderato'], metronome: null, sound_tempo_quarter_bpm: 80 },
    ]);
    for (const mark of parsed.marks) {
      expect(mark.score_object_ids.length).toBeGreaterThan(0);
      expect(mark.score_object_ids.every(id => parsed.notes.some(note => note.source_id === id && note.part_id === mark.part_id && note.measure === mark.measure))).toBe(true);
    }
  });
  it('preserves original bytes and maps chords, voices, ties, grace notes and sounding pitch independently', () => {
    const parsed = parseMusicScore(selectedScore);
    expect(parsed.musicxml).toBe(selectedScore);
    expect(parsed.notes.map(n => [n.source_id, n.beat, n.midi_pitch, n.voice])).toEqual([
      ['P1-m1-n1', 0, 58, '1:1'], ['P1-m1-n2', 0, 62, '1:1'], ['P1-m1-n3', 1, 58, '1:1'], ['P1-m1-n4', 0, null, '1:2'],
      ['P1-m2-n1', 0, 60, '1:1'], ['P1-m2-n2', 0, 62, '1:1'],
    ]);
    expect(parsed.notes[2].tie).toBe('stop');
    expect(parsed.notes[4].grace).toBe(true);
    expect(parsed.notes[5].score_beat).toBe(2);
    expect(parsed.render_xml).toContain('id="P1-m2-n2"');
  });

  it('keeps later key changes, staff scope, metronome units and direction offsets', () => {
    const xml = selectedScore.replace('<time>', '<key number="1"><fifths>-4</fifths><mode>minor</mode></key><time>')
      .replace('<measure number="2">', '<measure number="2"><attributes><key number="1"><fifths>0</fifths><mode>major</mode></key></attributes><direction><direction-type><words>Andante</words><metronome><beat-unit>eighth</beat-unit><beat-unit-dot/><per-minute>96</per-minute></metronome></direction-type><offset>2</offset><staff>1</staff></direction>');
    const parsed = parseMusicScore(xml);
    const later = parsed.marks.filter(mark => mark.measure === '2');
    expect(later.map(mark => [mark.measure_index, mark.beat, mark.staff, mark.value])).toEqual([
      [2, 0, '1', { kind: 'key_signature', fifths: 0, encoded_mode: 'major' }],
      [2, 0.5, '1', { kind: 'direction', words: ['Andante'], metronome: { beat_unit: 'eighth', dots: 1, per_minute: '96' }, sound_tempo_quarter_bpm: null }],
    ]);
    expect(later[1].score_object_ids).toContain('P1-m2-n1');
  });

  it('renders source IDs into actual server SVG without binding source identity to page layout', async () => {
    const rendered = await renderMusicScore(parseMusicScore(selectedScore));
    expect(rendered.score_objects).toHaveLength(6);
    for (const note of rendered.score_objects) expect(rendered.svg.get(note.page_id)).toContain(`id="${note.svg_element_id}"`);
    expect(rendered.midi.subarray(0, 4).toString()).toBe('MThd');
    expect(rendered.renderer).toMatch(/Verovio/);
  });

  it('creates a playable, explicitly synthesized WAV from selected score content', () => {
    const demo = synthesizeScoreDemonstration(parseMusicScore(selectedScore).notes.slice(0, 4), 120);
    expect(demo.wav.subarray(0, 4).toString()).toBe('RIFF');
    expect(demo.wav.readUInt32LE(24)).toBe(22050);
    expect(demo.duration_ms).toBeCloseTo(1150, 0);
    expect(demo.wav.readUInt32LE(40)).toBe(demo.wav.length - 44);
    expect(demo.timings).toEqual([
      { source_id: 'P1-m1-n1', start_ms: 0, end_ms: 500, attack: true },
      { source_id: 'P1-m1-n2', start_ms: 0, end_ms: 500, attack: true },
      { source_id: 'P1-m1-n3', start_ms: 500, end_ms: 1000, attack: false },
    ]);
  });
});
