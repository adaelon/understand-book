import { execFile } from 'node:child_process';
import { copyFileSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import { afterEach, expect, it, vi } from 'vitest';
import { buildCuratedMusicWorkspace, readMusicManifest, type MusicCuration } from './music-import';
import { parseMusicScore, renderMusicScore } from './music-score';
import { synthesizeScoreDemonstration } from './music-demonstrations';
import { publishAutomaticBuildArtifactSet } from './automatic-build-publication';

// Count real processor calls; ffprobe, full ffmpeg decode and Verovio still run.
vi.mock('node:child_process', async original => {
  const actual = await original<typeof import('node:child_process')>();
  const { promisify } = await import('node:util');
  const observed = vi.fn((...args: unknown[]) => Reflect.apply(actual.execFile, undefined, args));
  Object.defineProperty(observed, promisify.custom, { value: vi.fn(promisify(actual.execFile)) });
  return { ...actual, execFile: observed };
});
vi.mock('./music-score', async original => {
  const actual = await original<typeof import('./music-score')>();
  return { ...actual, parseMusicScore: vi.fn(actual.parseMusicScore), renderMusicScore: vi.fn(actual.renderMusicScore) };
});
vi.mock('./music-demonstrations', async original => {
  const actual = await original<typeof import('./music-demonstrations')>();
  return { ...actual, synthesizeScoreDemonstration: vi.fn(actual.synthesizeScoreDemonstration) };
});
vi.mock('./automatic-build-publication', async original => {
  const actual = await original<typeof import('./automatic-build-publication')>();
  return { ...actual, publishAutomaticBuildArtifactSet: vi.fn(actual.publishAutomaticBuildArtifactSet) };
});

const roots: string[] = [];
afterEach(() => { vi.clearAllMocks(); for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });
const execute = (execFile as unknown as Record<symbol, ReturnType<typeof vi.fn>>)[promisify.custom];
const xml = `<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>`;
function fixture() {
  const root = mkdtempSync(path.join(tmpdir(), 'music-incremental-')); roots.push(root);
  const bookDir = path.join(root, 'workspace'), curationPath = path.join(root, 'curation.json');
  writeFileSync(path.join(root, 'source.musicxml'), xml);
  writeFileSync(path.join(root, 'source.wav'), synthesizeScoreDemonstration(parseMusicScore(xml).notes, 120).wav);
  const curation: MusicCuration = { version: 'music_curation.v1', book_id: 'incremental-music', title: 'Two notes',
    work: { work_id: 'test-work', title: 'Two notes', composer: 'Test', movement: null },
    score: { source: { score_id: 'test-score', revision: '1' }, title: 'Selected score', musicxml_path: 'source.musicxml' },
    recording: { source: { recording_id: 'test-recording', revision: '1' }, title: 'Score demonstration', performers: 'Basic tone synthesizer', media_path: 'source.wav', kind: 'score_synthesis' },
    rights: [{ asset_kind: 'score', title: 'Test score', url: 'https://example.com/score', license: 'CC0', attribution: 'Test', distribution: 'allowed' },
      { asset_kind: 'recording', title: 'Test demonstration', url: 'https://example.com/audio', license: 'CC0', attribution: 'Test', distribution: 'allowed' }],
    demonstration_requests: [{ demonstration_id: 'phrase', title: 'Two notes', tempo_bpm: 120, score_object_ids: ['P1-m1-n1', 'P1-m1-n2'] }],
  };
  const save = () => writeFileSync(curationPath, JSON.stringify(curation)); save();
  const recordFile = path.join(bookDir, '.build/music/completed.json');
  const record = () => JSON.parse(readFileSync(recordFile, 'utf8'));
  const report = () => JSON.parse(readFileSync(path.join(bookDir, 'music/build-report.json'), 'utf8'));
  const run = (options: Partial<Parameters<typeof buildCuratedMusicWorkspace>[0]> = {}) => buildCuratedMusicWorkspace({ curationPath, bookDir, ...options });
  vi.clearAllMocks();
  return { root, bookDir, curationPath, curation, save, run, record, recordFile, report };
}
function calls(audio: number, parse: number, render: number, demonstrations: number) {
  expect(execute).toHaveBeenCalledTimes(audio);
  expect(parseMusicScore).toHaveBeenCalledTimes(parse);
  expect(renderMusicScore).toHaveBeenCalledTimes(render);
  expect(synthesizeScoreDemonstration).toHaveBeenCalledTimes(demonstrations);
}
function withoutRenderTime(bytes: Buffer) {
  return bytes.toString('utf8').replace(/<application\b[^>]*>[\s\S]*?<\/application>/g, application =>
    /<name\b[^>]*>Verovio<\/name>/.test(application) ? application.replace(/\s+isodate="[^"]*"/, '') : application);
}

it('records completed work for an old workspace and then reuses every processor without changing output bytes', async () => {
  const f = fixture(), first = await f.run();
  calls(2, 1, 1, 1);
  const files = ['music_source_manifest.json', 'music/score/parsed.json', 'music/score/render-input.musicxml',
    'music/score/derived.mei', 'music/score/score-synthesis.mid', ...first.pages.map(p => p.path), ...first.demonstrations.map(d => d.media_path)];
  const initialBytes = files.map(file => readFileSync(path.join(f.bookDir, file)));
  rmSync(f.recordFile); vi.clearAllMocks();
  await f.run(); calls(2, 1, 1, 1);
  const bytes = files.map(file => readFileSync(path.join(f.bookDir, file)));
  for (const [i, file] of files.entries()) {
    // Verovio stamps its application element on each actual render.
    if (file.endsWith('.mei')) expect(withoutRenderTime(bytes[i])).toBe(withoutRenderTime(initialBytes[i]));
    else expect(bytes[i]).toEqual(initialBytes[i]);
  }
  vi.clearAllMocks();
  expect(await f.run()).toEqual(first); calls(0, 0, 0, 0);
  expect(f.report()).toMatchObject({ model_calls: 0, reused_stages: { audio: true, score_parse: true, score_render: true, demonstrations: ['phrase'] } });
  for (const [i, file] of files.entries()) expect(readFileSync(path.join(f.bookDir, file))).toEqual(bytes[i]);
}, 30_000);

it('recreates only the stage whose output is missing, including just one missing demonstration', async () => {
  const f = fixture(), first = await f.run();
  for (const [file, expected] of [
    ['music/audio/probe.json', [2, 0, 0, 0]],
    ['music/score/parsed.json', [0, 1, 0, 0]],
    [first.pages[0].path, [0, 0, 1, 0]],
    ['music/score/derived.mei', [0, 0, 1, 0]],
    ['music/score/score-synthesis.mid', [0, 0, 1, 0]],
    [first.demonstrations[0].media_path, [0, 0, 0, 1]],
  ] as Array<[string, [number, number, number, number]]>) {
    const target = path.join(f.bookDir, file), bytes = readFileSync(target);
    rmSync(target); vi.clearAllMocks();
    expect(await f.run()).toEqual(first); calls(...expected);
    if (file.endsWith('.mei')) expect(withoutRenderTime(readFileSync(target))).toBe(withoutRenderTime(bytes));
    else expect(readFileSync(target)).toEqual(bytes);
  }
}, 30_000);

it('retains completed stages after a later failure and still admits current events on recovery', async () => {
  const f = fixture();
  await expect(f.run({ onProgress: update => { if (update.phase === 'music_alignment') throw new Error('interrupted alignment'); } })).rejects.toThrow('interrupted alignment');
  expect(existsSync(path.join(f.bookDir, 'music_source_manifest.json'))).toBe(false);
  expect(f.record()).toHaveProperty('audio'); expect(f.record()).toHaveProperty('score_parse'); expect(f.record()).toHaveProperty('score_render');
  vi.clearAllMocks(); await f.run(); calls(0, 0, 0, 1);
  f.curation.events = [{ event_id: 'pitch', title: 'C音', explanation: '谱面写出C4。', listen_for: '听一个持续音。', start_ms: 0, end_ms: 500,
    roles: [{ part_id: 'P1', role: '单音' }], evidence: [{ provenance_id: 'musicxml-score-marks', score_object_ids: ['P1-m1-n1'], audio: null }], relations: [], concept_tags: ['音高'] }];
  f.save(); vi.clearAllMocks();
  expect((await f.run()).events).toHaveLength(1); calls(0, 0, 0, 0);
  f.curation.events[0].evidence[0].score_object_ids = ['missing']; f.save();
  await expect(f.run()).rejects.toThrow();
  expect(readMusicManifest(f.bookDir).events[0].evidence[0].score_object_ids).toEqual(['P1-m1-n1']);
}, 30_000);

it('compares real source paths, extensions and processing versions before reusing stages', async () => {
  const f = fixture(); await f.run();
  copyFileSync(path.join(f.root, 'source.wav'), path.join(f.root, 'moved.wav'));
  f.curation.recording.media_path = 'moved.wav'; f.save(); vi.clearAllMocks();
  await f.run(); calls(2, 0, 0, 0);
  copyFileSync(path.join(f.root, 'source.musicxml'), path.join(f.root, 'source.xml'));
  f.curation.score.musicxml_path = 'source.xml'; f.save(); vi.clearAllMocks();
  await f.run(); calls(0, 1, 1, 0);
  const old = f.record();
  old.audio.input.processor = 'music-audio-check.v0';
  old.score_parse.input.processor = 'musicxml-score-parser.v0';
  old.score_render.input.parse.processor = 'musicxml-score-parser.v0';
  old.demonstrations.phrase.input.parameters.processor = 'score-oscillator.v0';
  writeFileSync(f.recordFile, JSON.stringify(old)); vi.clearAllMocks();
  await f.run(); calls(2, 1, 1, 1);
  const parameters = f.record(); parameters.score_render.input.parameters.options.pageWidth = 999;
  writeFileSync(f.recordFile, JSON.stringify(parameters)); vi.clearAllMocks();
  await f.run(); calls(0, 0, 1, 0);
  writeFileSync(path.join(f.root, 'moved.wav'), synthesizeScoreDemonstration(parseMusicScore(xml).notes, 60).wav);
  vi.clearAllMocks(); await expect(f.run()).rejects.toThrow('source snapshot changed'); calls(0, 0, 0, 0);
}, 30_000);

it('reuses renamed labels and admits changed tempo, selections and IDs without overwriting audio on a failed rebuild', async () => {
  const f = fixture(), first = await f.run(), demoFile = path.join(f.bookDir, first.demonstrations[0].media_path), bytes = readFileSync(demoFile);
  const request = f.curation.demonstration_requests![0];
  request.title = 'Renamed phrase'; f.save(); vi.clearAllMocks();
  expect((await f.run()).demonstrations[0].title).toBe('谱面合成示范 · Renamed phrase'); calls(0, 0, 0, 0);
  request.tempo_bpm = 60;
  f.curation.parts = [];
  f.save(); vi.clearAllMocks();
  await expect(f.run()).rejects.toThrow(); calls(0, 0, 0, 1);
  expect(readFileSync(demoFile)).toEqual(bytes);
  expect(f.record().demonstrations.phrase.input.tempo_bpm).toBe(120);
  delete f.curation.parts; f.save(); vi.clearAllMocks();
  const slower = await f.run(); calls(0, 0, 0, 1);
  expect(slower.demonstrations[0].duration_ms).toBeGreaterThan(first.demonstrations[0].duration_ms);
  expect(readFileSync(demoFile)).not.toEqual(bytes);
  const slowerBytes = readFileSync(demoFile);
  request.tempo_bpm = 120; request.score_object_ids = ['P1-m1-n2']; f.save(); vi.clearAllMocks();
  const single = await f.run(); calls(0, 0, 0, 1);
  expect(single.demonstrations[0].score_object_ids).toEqual(['P1-m1-n2']);
  expect(readFileSync(demoFile)).not.toEqual(slowerBytes);
  const singleBytes = readFileSync(demoFile);
  request.demonstration_id = 'second-note'; f.save(); vi.clearAllMocks();
  const changed = await f.run(); calls(0, 0, 0, 1);
  expect(changed.demonstrations[0].score_object_ids).toEqual(['P1-m1-n2']);
  expect(readFileSync(demoFile)).toEqual(singleBytes);
  expect(readFileSync(path.join(f.bookDir, changed.demonstrations[0].media_path))).not.toEqual(bytes);
}, 30_000);

it('rolls back changed audio and its completion record together when publication fails, then resumes the pending demonstration', async () => {
  const f = fixture(), first = await f.run(), demoFile = path.join(f.bookDir, first.demonstrations[0].media_path);
  const bytes = readFileSync(demoFile), priorRecord = readFileSync(f.recordFile);
  const actual = await vi.importActual<typeof import('./automatic-build-publication')>('./automatic-build-publication');
  f.curation.demonstration_requests![0].tempo_bpm = 60; f.save(); vi.clearAllMocks();
  vi.mocked(publishAutomaticBuildArtifactSet).mockImplementationOnce(input => {
    expect(input.artifacts['.build/music/completed.json']).toBeDefined();
    expect(input.artifacts[first.demonstrations[0].media_path]).toBeDefined();
    return actual.publishAutomaticBuildArtifactSet({ ...input, fault_injection: { fail_after_promotions: Object.keys(input.artifacts).length - 1 } });
  });
  await expect(f.run()).rejects.toThrow('injected publication failure'); calls(0, 0, 0, 1);
  expect(readFileSync(demoFile)).toEqual(bytes);
  expect(readFileSync(f.recordFile)).toEqual(priorRecord);
  expect(readMusicManifest(f.bookDir)).toEqual(first);
  vi.clearAllMocks();
  const recovered = await f.run(); calls(0, 0, 0, 1);
  expect(recovered.demonstrations[0].duration_ms).toBeGreaterThan(first.demonstrations[0].duration_ms);
  expect(f.record().demonstrations.phrase.input.tempo_bpm).toBe(60);
  const recoveredBytes = readFileSync(demoFile);
  vi.clearAllMocks(); await f.run(); calls(0, 0, 0, 0);
  expect(readFileSync(demoFile)).toEqual(recoveredBytes);
}, 30_000);
