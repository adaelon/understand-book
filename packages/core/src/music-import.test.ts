import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { buildCuratedMusicWorkspace, calibrateMusicWorkspace, readMusicManifest, type MusicCuration } from './music-import';
import { parseMusicScore } from './music-score';
import { synthesizeScoreDemonstration } from './music-demonstrations';

const xml = `<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>1</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>`;

describe('music import', () => {
  it('rebuilds both the original input and the portable curation after overlapping boundary calibrations', async () => {
    const folder = mkdtempSync(path.join(os.tmpdir(), 'music-calibration-rebuild-')), bookDir = path.join(folder, 'workspace');
    const score = xml.replace('</part></score-partwise>', [2, 3].map(measure => `<measure number="${measure}"><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure>`).join('') + '</part></score-partwise>');
    writeFileSync(path.join(folder, 'source.musicxml'), score);
    writeFileSync(path.join(folder, 'source.wav'), synthesizeScoreDemonstration(parseMusicScore(score).notes, 120).wav);
    const occurrence = (measure: number, start_ms: number, end_ms: number) => ({ occurrence_id: `o${measure}`, order: measure - 1, measure: String(measure), start_ms, end_ms, score_object_ids: [`P1-m${measure}-n1`] });
    const curation: MusicCuration = { version: 'music_curation.v1', book_id: 'calibrated-music', title: 'Three notes',
      work: { work_id: 'work', title: 'Three notes', composer: 'Test', movement: null },
      score: { source: { score_id: 'score', revision: '1' }, title: 'Score', musicxml_path: 'source.musicxml' },
      recording: { source: { recording_id: 'recording', revision: '1' }, title: 'Recording', performers: 'Synth', media_path: 'source.wav', kind: 'score_synthesis' },
      rights: ['score', 'recording'].map(asset_kind => ({ asset_kind: asset_kind as 'score' | 'recording', title: 'Original test', url: 'https://example.com', license: 'CC0', attribution: 'Test', distribution: 'allowed' })),
      occurrences: [occurrence(1, 0, 500), occurrence(2, 500, 1000), occurrence(3, 1000, 1500)] };
    const curationPath = path.join(folder, 'curation.json'); writeFileSync(curationPath, JSON.stringify(curation));
    await buildCuratedMusicWorkspace({ curationPath, bookDir });
    const revise = (start_ms: number, end_ms: number, occurrences: NonNullable<MusicCuration['occurrences']>, provenance_id: string) => calibrateMusicWorkspace(bookDir, {
      recording: curation.recording.source, score: curation.score.source, start_ms, end_ms, reason: 'Reviewed boundary correction', occurrences, note_alignments: [],
      coverage: [{ start_ms, end_ms, granularity: 'measure', method: 'manual', status: 'accepted', provenance_id, detail: 'Reviewed synthetic source' }],
      provenance: [{ provenance_id, kind: 'human_listening', description: 'Reviewed synthetic source', source_url: null, processor: null }],
    });
    revise(0, 1000, [occurrence(1, 0, 600), occurrence(2, 600, 1000)], 'first-review');
    const calibrated = revise(600, 1500, [occurrence(2, 600, 1100), occurrence(3, 1100, 1500)], 'second-review');
    const history = readFileSync(path.join(bookDir, '.build/music/calibrations.json'));
    const portablePath = path.join(bookDir, 'music/curation.json');
    for (const input of [portablePath, curationPath, portablePath]) {
      const rebuilt = await buildCuratedMusicWorkspace({ curationPath: input, bookDir });
      expect(rebuilt).toEqual({ ...calibrated, provenance: expect.arrayContaining(calibrated.provenance) });
      expect(rebuilt.provenance).toHaveLength(calibrated.provenance.length);
      expect(readFileSync(path.join(bookDir, '.build/music/calibrations.json'))).toEqual(history);
    }
    const portable = JSON.parse(readFileSync(portablePath, 'utf8'));
    portable.events = [{ event_id: 'middle', title: 'Middle note', explanation: 'Reviewed middle note', listen_for: 'Listen to the middle note',
      start_ms: 600, end_ms: 1100, occurrence_ids: ['o2'], roles: [], evidence: [{ provenance_id: 'second-review', score_object_ids: ['P1-m2-n1'], audio: null }], relations: [], concept_tags: [] }];
    writeFileSync(portablePath, JSON.stringify(portable));
    const edited = await buildCuratedMusicWorkspace({ curationPath: portablePath, bookDir });
    expect(edited.occurrences).toEqual(calibrated.occurrences);
    expect(edited.events).toEqual(portable.events);
    expect(readFileSync(path.join(bookDir, '.build/music/calibrations.json'))).toEqual(history);
  }, 30_000);

  it('publishes a transcript-free playable workspace and preserves source identity on semantic rebuild', async () => {
    const folder = mkdtempSync(path.join(os.tmpdir(), 'music-import-')), bookDir = path.join(folder, 'workspace');
    writeFileSync(path.join(folder, 'source.musicxml'), xml);
    writeFileSync(path.join(folder, 'source.wav'), synthesizeScoreDemonstration(parseMusicScore(xml).notes, 120).wav);
    const curation: MusicCuration = { version: 'music_curation.v1', book_id: 'test-music', title: 'One note', work: { work_id: 'test-work', title: 'One note', composer: 'Test', movement: null,
      tonality: { tonic: 'C', mode: 'major', explanation: 'Fixture composer designation, independently retained from the score encoding.', provenance_ids: ['score'] } },
      score: { source: { score_id: 'test-score', revision: '1' }, title: 'Selected score', musicxml_path: 'source.musicxml' },
      recording: { source: { recording_id: 'test-recording', revision: '1' }, title: 'Score demonstration', performers: 'Basic tone synthesizer', media_path: 'source.wav', kind: 'score_synthesis' },
      rights: [{ asset_kind: 'score', title: 'Test score', url: 'https://example.com/score', license: 'CC0', attribution: 'Test', distribution: 'allowed' }, { asset_kind: 'recording', title: 'Test demonstration', url: 'https://example.com/audio', license: 'CC0', attribution: 'Test', distribution: 'allowed' }],
      provenance: [{ provenance_id: 'score', kind: 'score', description: 'Written C4 in the test score', source_url: null, processor: null }],
    };
    const curationPath = path.join(folder, 'curation.json');
    writeFileSync(curationPath, JSON.stringify(curation));
    const first = await buildCuratedMusicWorkspace({ curationPath, bookDir });
    expect(first.recording.duration_ms).toBeGreaterThan(600);
    expect(first.pages).toHaveLength(1);
    expect(readMusicManifest(bookDir)).toEqual(first);
    expect(first.work.tonality).toEqual(curation.work.tonality);
    expect(first.score_marks?.[0].value).toEqual({ kind: 'time_signature', components: [{ beats: '1', beat_type: 4 }], symbol: null });
    expect(first.provenance.find(p => p.provenance_id === first.score_marks?.[0].provenance_id)?.kind).toBe('score');
    const guide = readFileSync(path.join(bookDir, 'source.txt'), 'utf8');
    expect(guide).toContain('音乐伴听导读');
    expect(JSON.parse(readFileSync(path.join(bookDir, 'base.json'), 'utf8')).book_id).toBe('test-music');
    const calibrated = calibrateMusicWorkspace(bookDir, { recording: first.recording.source, score: first.score.source, start_ms: 0, end_ms: first.recording.duration_ms,
      reason: 'Fixture calibration for score synthesis', occurrences: [{ occurrence_id: 'one', order: 0, measure: '1', start_ms: 0, end_ms: first.recording.duration_ms, score_object_ids: ['P1-m1-n1'] }],
      note_alignments: [{ occurrence_id: 'one', score_object_id: 'P1-m1-n1', start_ms: 80, end_ms: 550, attack: true, provenance_id: 'fixture-synthesis' }],
      coverage: [{ start_ms: 0, end_ms: first.recording.duration_ms, granularity: 'note', method: 'score_synthesis', status: 'accepted', provenance_id: 'fixture-synthesis', detail: 'Fixture calibration timing, not a performance' }],
      provenance: [{ provenance_id: 'fixture-synthesis', kind: 'score_synthesis', description: 'Generated test audio', source_url: null, processor: 'test fixture' }] });
    expect(calibrated.note_alignments[0].start_ms).toBe(80);
    curation.events = [{ event_id: 'pitch', title: 'C音', explanation: '谱面写出C4。', listen_for: '听一个持续音。', start_ms: 0, end_ms: 500, roles: [{ part_id: 'P1', role: '单音' }], evidence: [{ provenance_id: 'score', score_object_ids: ['P1-m1-n1'], audio: null }], relations: [], concept_tags: ['音高'] }];
    writeFileSync(curationPath, JSON.stringify(curation));
    const next = await buildCuratedMusicWorkspace({ curationPath, bookDir });
    expect(next.events).toHaveLength(1);
    expect(next.note_alignments).toEqual(calibrated.note_alignments);
    expect(readFileSync(path.join(bookDir, 'source.txt'), 'utf8')).toBe(guide);
    writeFileSync(path.join(folder, 'source.musicxml'), xml.replace('<step>C</step>', '<step>D</step>'));
    await expect(buildCuratedMusicWorkspace({ curationPath, bookDir })).rejects.toThrow('source snapshot changed');
    expect(readMusicManifest(bookDir).events).toHaveLength(1);
    const history = JSON.parse(readFileSync(path.join(bookDir, '.build/music/calibrations.json'), 'utf8'));
    writeFileSync(path.join(bookDir, 'publication.json'), '{}');
    expect(() => calibrateMusicWorkspace(bookDir, history[0].revision)).toThrow('Published music is immutable');
    expect(JSON.parse(readFileSync(path.join(bookDir, '.build/music/calibrations.json'), 'utf8'))).toEqual(history);
  }, 30000);
});
