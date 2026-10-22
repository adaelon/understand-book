import { execFile } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, renameSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { promisify } from 'node:util';
import { z } from 'zod';
import { validateMusicMaterialSuccessor, validateMusicSourceManifest, type MusicSourceManifest, type MusicRecording, type MusicDemonstration } from './music-contract';
import { musicXmlFromBytes, parseMusicScore, renderMusicScore } from './music-score';
import { synthesizeScoreDemonstration, type MusicDemonstrationRequest } from './music-demonstrations';
import { videoTool } from './video-dependencies';
import { markdownToBlocks } from './md-adapter';
import { segment } from './segment';
import { publishAutomaticBuildArtifactSet } from './automatic-build-publication';
import { reviseMusicAlignment, type MusicAlignmentRevision } from './music-alignment';
import { bindMusicEventOccurrences } from './music-events';

const execute = promisify(execFile);
export const MUSIC_MANIFEST_FILE = 'music_source_manifest.json';
export interface MusicCuration extends Pick<MusicSourceManifest, 'book_id' | 'title' | 'work' | 'score' | 'rights'> {
  version: 'music_curation.v1';
  recording: Omit<MusicRecording, 'content_type' | 'duration_ms'>;
  guide?: string; parts?: MusicSourceManifest['parts'];
  occurrences?: MusicSourceManifest['occurrences']; note_alignments?: MusicSourceManifest['note_alignments'];
  coverage?: MusicSourceManifest['coverage']; provenance?: MusicSourceManifest['provenance'];
  events?: MusicSourceManifest['events']; structure?: MusicSourceManifest['structure'];
  alignment_path?: string; demonstration_requests?: MusicDemonstrationRequest[];
}
export interface MusicBuildProgress { phase: string; completed: number; total: number }
export interface MusicBuildOptions { curationPath: string; bookDir: string; onProgress?: (progress: MusicBuildProgress) => void; signal?: AbortSignal; ffprobe?: string; ffmpeg?: string }
const curationShape = z.object({ version: z.literal('music_curation.v1'), book_id: z.string().min(1), title: z.string().min(1),
  work: z.object({ work_id: z.string().min(1), title: z.string().min(1), composer: z.string().min(1), movement: z.string().nullable() }),
  score: z.object({ source: z.object({ score_id: z.string().min(1), revision: z.string().min(1) }), title: z.string().min(1), musicxml_path: z.string().min(1) }),
  recording: z.object({ source: z.object({ recording_id: z.string().min(1), revision: z.string().min(1) }), title: z.string().min(1), performers: z.string(), media_path: z.string().min(1), kind: z.enum(['performance', 'score_synthesis']) }),
  rights: z.array(z.unknown()),
}).passthrough();
const json = (data: unknown) => JSON.stringify(data, null, 2) + '\n';
const writeJson = (file: string, data: unknown) => { mkdirSync(path.dirname(file), { recursive: true }); writeFileSync(`${file}.tmp`, json(data)); renameSync(`${file}.tmp`, file); };

export function readMusicManifest(bookDir: string): MusicSourceManifest {
  return validateMusicSourceManifest(JSON.parse(readFileSync(path.join(bookDir, MUSIC_MANIFEST_FILE), 'utf8')));
}

/** Calibration is a candidate acceptance step; it preserves both the prior mapping and its replacement. */
export function calibrateMusicWorkspace(bookDir: string, revision: MusicAlignmentRevision): MusicSourceManifest {
  if (existsSync(path.join(bookDir, 'publication.json'))) throw new Error('Published music is immutable; calibrate its original local build workspace');
  const current = readMusicManifest(bookDir), result = reviseMusicAlignment(current, revision);
  const historyPath = '.build/music/calibrations.json';
  const history: Array<{ revision: MusicAlignmentRevision; receipt: unknown }> = existsSync(path.join(bookDir, historyPath))
    ? JSON.parse(readFileSync(path.join(bookDir, historyPath), 'utf8')) : [];
  history.push({ revision, receipt: result.receipt });
  const curationPath = path.join(bookDir, 'music/curation.json');
  const curation = JSON.parse(readFileSync(curationPath, 'utf8'));
  Object.assign(curation, { occurrences: result.manifest.occurrences, note_alignments: result.manifest.note_alignments, coverage: result.manifest.coverage,
    events: result.manifest.events, structure: result.manifest.structure,
    provenance: result.manifest.provenance.filter(item => !result.manifest.demonstrations.some(demo => demo.provenance_id === item.provenance_id)) });
  publishAutomaticBuildArtifactSet({ workspace_dir: bookDir, stage: 'music_prebuild', artifacts: {
    [MUSIC_MANIFEST_FILE]: json(result.manifest), [historyPath]: json(history),
    'music/calibration-history.json': json(history.map(item => item.receipt)), 'music/curation.json': json(curation),
  } });
  return result.manifest;
}

function copyOriginal(input: string, output: string) {
  if (!statSync(input).isFile() || !statSync(input).size) throw new Error(`Music source is not a nonempty file: ${input}`);
  mkdirSync(path.dirname(output), { recursive: true });
  if (existsSync(output)) {
    if (!readFileSync(input).equals(readFileSync(output))) throw new Error('A source snapshot changed; create a new workspace/revision to preserve old music references');
    return;
  }
  copyFileSync(input, `${output}.part`); renameSync(`${output}.part`, output);
}

function curatorGuide(curation: MusicCuration): string {
  if (curation.guide?.trim()) return curation.guide;
  return `# ${curation.title}\n\n这是一份音乐伴听导读，文字由编选者准备。原声与谱面是各自独立的来源。\n\n## 作品与录音\n\n作曲：${curation.work.composer}。作品：${curation.work.title}${curation.work.movement ? `，${curation.work.movement}` : ''}。录音：${curation.recording.title}；演奏者：${curation.recording.performers}。\n\n## 怎样开始\n\n先完整听一个短片段，再选一个声部或节奏特点重听。播放器显示的对齐覆盖说明哪些位置已有逐音符校准。解释来自谱面、录音分析或参考资料时，会分别保留其依据。\n\n## 素材来源\n\n${curation.rights.map(right => `- ${right.title}：${right.license}。${right.attribution} ${right.url}`).join('\n')}\n`;
}

/** Curated inputs are reviewable data. Semantic judgments are not generated by a transcription surrogate. */
export async function buildCuratedMusicWorkspace(options: MusicBuildOptions): Promise<MusicSourceManifest> {
  const inputPath = path.resolve(options.curationPath), bookDir = path.resolve(options.bookDir), inputDir = path.dirname(inputPath);
  const curation = curationShape.parse(JSON.parse(readFileSync(inputPath, 'utf8'))) as unknown as MusicCuration;
  const previous = existsSync(path.join(bookDir, MUSIC_MANIFEST_FILE)) ? readMusicManifest(bookDir) : null;
  if (!previous && existsSync(path.join(bookDir, 'base.json'))) throw new Error('Use a music workspace rather than overwriting an existing book');
  if (previous && (previous.book_id !== curation.book_id || previous.recording.source.recording_id !== curation.recording.source.recording_id
    || previous.recording.source.revision !== curation.recording.source.revision || previous.score.source.score_id !== curation.score.source.score_id || previous.score.source.revision !== curation.score.source.revision)) throw new Error('Music source identity changed; create a new workspace/revision');
  const progress = (phase: string, completed: number, total = 1) => { options.signal?.throwIfAborted(); options.onProgress?.({ phase, completed, total }); };
  const stateFile = path.join(bookDir, '.build/music/state.json');
  const started = Date.now();
  progress('music_import', 0);
  try {
    const audioInput = path.resolve(inputDir, curation.recording.media_path), xmlInput = path.resolve(inputDir, curation.score.musicxml_path);
    const extension = path.extname(audioInput).toLowerCase();
    const mediaPath = `music/audio/recording${extension}`, musicxmlPath = 'music/score/original.musicxml';
    copyOriginal(audioInput, path.join(bookDir, mediaPath));
    copyOriginal(xmlInput, path.join(bookDir, `music/score/source${path.extname(xmlInput).toLowerCase()}`));
    writeJson(stateFile, { version: 'music_build_state.v1', stage: 'probing', input_path: inputPath, book_id: curation.book_id, failure: null });
    const probe = await execute(options.ffprobe ?? videoTool('ffprobe'), ['-v', 'error', '-show_format', '-show_streams', '-of', 'json', path.join(bookDir, mediaPath)], { signal: options.signal, windowsHide: true });
    const media = JSON.parse(probe.stdout), stream = media.streams.find((s: { codec_type: string }) => s.codec_type === 'audio');
    if (!stream || !(Number(media.format.duration) > 0)) throw new Error('Selected recording has no decodable audio or duration');
    await execute(options.ffmpeg ?? videoTool('ffmpeg'), ['-v', 'error', '-xerror', '-i', path.join(bookDir, mediaPath), '-map', '0:a:0', '-f', 'null', '-'], { signal: options.signal, windowsHide: true });
    const contentType: Record<string, string> = { '.mp3': 'audio/mpeg', '.wav': 'audio/wav', '.ogg': 'audio/ogg', '.flac': 'audio/flac', '.m4a': 'audio/mp4', '.aac': 'audio/aac', '.opus': 'audio/ogg' };
    if (!contentType[extension]) throw new Error(`Unsupported recording file extension: ${extension}`);
    const recording: MusicRecording = { ...curation.recording, media_path: mediaPath, content_type: contentType[extension], duration_ms: Number(media.format.duration) * 1000 };
    writeFileSync(path.join(bookDir, 'music/audio/probe.json'), probe.stdout);
    progress('music_import', 1);
    progress('music_score', 0);
    const parsed = parseMusicScore(musicXmlFromBytes(readFileSync(xmlInput)));
    writeFileSync(path.join(bookDir, musicxmlPath), parsed.musicxml);
    writeFileSync(path.join(bookDir, 'music/score/render-input.musicxml'), parsed.render_xml);
    writeJson(path.join(bookDir, 'music/score/parsed.json'), { parts: parsed.parts, notes: parsed.notes, measure_durations: parsed.measure_durations });
    const rendered = await renderMusicScore(parsed);
    const sidecar = process.env.UNDERSTAND_BOOK_SIDECAR_SELF;
    const notice = readFileSync(sidecar ? path.join(path.dirname(sidecar), 'resources/music/NOTICE.txt') : new URL('../assets/music/NOTICE.txt', import.meta.url), 'utf8');
    writeFileSync(path.join(bookDir, 'music/NOTICE.txt'), notice);
    for (const page of rendered.pages) writeFileSync(path.join(bookDir, page.path), rendered.svg.get(page.page_id)!);
    writeFileSync(path.join(bookDir, 'music/score/derived.mei'), rendered.mei);
    writeFileSync(path.join(bookDir, 'music/score/score-synthesis.mid'), rendered.midi);
    progress('music_score', 1);
    progress('music_alignment', 0);
    const alignment = curation.alignment_path ? JSON.parse(readFileSync(path.resolve(inputDir, curation.alignment_path), 'utf8')) : {};
    const provenance = [...(curation.provenance ?? []), ...(alignment.provenance ?? [])];
    progress('music_alignment', 1);
    progress('music_events', 0);
    const demonstrations: MusicDemonstration[] = [];
    for (const request of curation.demonstration_requests ?? []) {
      if (!/^[A-Za-z0-9_-]+$/.test(request.demonstration_id)) throw new Error('Demonstration IDs must contain letters, digits, underscores or hyphens');
      const selected = parsed.notes.filter(note => request.score_object_ids.includes(note.source_id));
      if (new Set(selected.map(note => note.source_id)).size !== new Set(request.score_object_ids).size) throw new Error(`Demonstration ${request.demonstration_id} includes unknown score objects`);
      const demo = synthesizeScoreDemonstration(selected, request.tempo_bpm), media_path = `music/demonstrations/${request.demonstration_id}.wav`, provenance_id = `score-synthesis:${request.demonstration_id}`;
      mkdirSync(path.dirname(path.join(bookDir, media_path)), { recursive: true }); writeFileSync(path.join(bookDir, media_path), demo.wav);
      provenance.push({ provenance_id, kind: 'score_synthesis', description: `谱面合成示范：按选定谱音以 ${request.tempo_bpm} BPM 生成基础音色，装饰音采用 80 毫秒的呈现时值；不是原录音的分轨。`, source_url: null, processor: 'understand-book score oscillator v1' });
      demonstrations.push({ demonstration_id: request.demonstration_id, title: `谱面合成示范 · ${request.title}`, kind: 'score_synthesis', media_path, content_type: 'audio/wav', duration_ms: demo.duration_ms, part_ids: [...new Set(selected.map(note => note.part_id))], score_object_ids: request.score_object_ids, provenance_id });
    }
    let manifest = validateMusicSourceManifest({ version: 'music_source_manifest.v1', book_id: curation.book_id, title: curation.title, work: curation.work,
      score: { ...curation.score, musicxml_path: musicxmlPath }, recording,
      rights: [...curation.rights.filter(right => !(right.asset_kind === 'font' && right.title === 'Leipzig music font')),
        { asset_kind: 'font', title: 'Leipzig music font', url: 'https://github.com/rism-digital/leipzig/tree/a8838c6fe6bf3aaf4bfb5d831436b4f7594dbf9c', license: 'SIL Open Font License 1.1', attribution: 'Leipzig music font, Etienne Darbellay, Jean-Francois Marti and Laurent Pugin; copyright and license in music/NOTICE.txt', distribution: 'allowed' }],
      parts: curation.parts ?? parsed.parts, pages: rendered.pages, score_objects: rendered.score_objects,
      occurrences: alignment.occurrences ?? curation.occurrences ?? [], note_alignments: alignment.note_alignments ?? curation.note_alignments ?? [], coverage: alignment.coverage ?? curation.coverage ?? [],
      provenance, events: curation.events ?? [], structure: curation.structure ?? [], demonstrations });
    manifest.events = bindMusicEventOccurrences(manifest);
    const calibrationPath = path.join(bookDir, '.build/music/calibrations.json');
    if (existsSync(calibrationPath)) for (const item of JSON.parse(readFileSync(calibrationPath, 'utf8')) as Array<{ revision: MusicAlignmentRevision }>) manifest = reviseMusicAlignment(manifest, item.revision).manifest;
    if (previous) validateMusicMaterialSuccessor(manifest, previous);
    progress('music_events', 1);
    progress('music_publication', 0);
    const guide = curatorGuide(curation);
    if (previous && readFileSync(path.join(bookDir, 'source.txt'), 'utf8') !== guide) throw new Error('The curator guide is a stable text source; use a new material revision to change it');
    const base = { book_id: curation.book_id, lid_nodes: segment(markdownToBlocks(guide)), graph_nodes: [], graph_edges: [] };
    const portableCuration = { ...curation, guide, parts: manifest.parts, score: { ...manifest.score, musicxml_path: path.posix.relative('music', manifest.score.musicxml_path) },
      recording: { ...manifest.recording, media_path: path.posix.relative('music', manifest.recording.media_path) }, occurrences: manifest.occurrences,
      note_alignments: manifest.note_alignments, coverage: manifest.coverage, events: manifest.events, structure: manifest.structure,
      provenance: manifest.provenance.filter(item => !demonstrations.some(demo => demo.provenance_id === item.provenance_id)) };
    delete portableCuration.alignment_path;
    publishAutomaticBuildArtifactSet({ workspace_dir: bookDir, stage: 'music_prebuild', artifacts: {
      'source.txt': guide, 'base.json': json(base), [MUSIC_MANIFEST_FILE]: json(manifest),
      'music/curation.json': json(portableCuration),
      'music/build-report.json': json({ version: 'music_build_report.v1', renderer: rendered.renderer, elapsed_ms: Date.now() - started, audio_processed: { path: mediaPath, operation: 'ffprobe metadata and full ffmpeg decode', music_listening_model: false }, score_objects: manifest.score_objects.length, pages: manifest.pages.length, note_alignments: manifest.note_alignments.length, coverage: manifest.coverage, model_calls: 0 }),
    } });
    writeJson(stateFile, { version: 'music_build_state.v1', stage: 'completed', input_path: inputPath, book_id: curation.book_id, failure: null });
    progress('music_publication', 1);
    progress('completed', 1);
    return manifest;
  } catch (error) {
    writeJson(stateFile, { version: 'music_build_state.v1', stage: 'failed', input_path: inputPath, book_id: curation.book_id, failure: error instanceof Error ? error.message : String(error) });
    throw error;
  }
}
