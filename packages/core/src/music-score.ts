import { DOMParser, XMLSerializer } from '@xmldom/xmldom';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';
import { unzipSync, strFromU8 } from 'fflate';
import type { MusicPart, MusicScoreObject, MusicScorePage, MusicScoreMark, MusicScoreMarkValue } from './music-contract';

interface XmlElement { nodeType: number; tagName: string; childNodes: ArrayLike<XmlElement>; textContent: string | null; getAttribute(name: string): string | null; setAttribute(name: string, value: string): void; getElementsByTagName(name: string): ArrayLike<XmlElement> }
const children = (node: XmlElement, tag?: string): XmlElement[] => Array.from(node.childNodes).filter(n => n.nodeType === 1 && (!tag || n.tagName === tag));
const child = (node: XmlElement, tag: string) => children(node, tag)[0];
const value = (node: XmlElement, tag: string, fallback = '') => child(node, tag)?.textContent?.trim() || fallback;
const number = (node: XmlElement, tag: string, fallback = 0) => Number(value(node, tag, String(fallback)));

export interface ParsedMusicNote extends Omit<MusicScoreObject, 'page_id' | 'svg_element_id'> { measure_index: number; score_beat: number; grace: boolean }
export const MUSIC_SCORE_MARKS_PROVENANCE_ID = 'musicxml-score-marks';
// Update these versions when parsing, mapping or the packaged Verovio changes.
export const MUSIC_SCORE_PARSER_VERSION = 'musicxml-score-parser.v1';
export const MUSIC_SCORE_RENDER_PARAMETERS = { processor: 'music-score-render.v1', verovio: '6.3.0',
  options: { inputFrom: 'xml', font: 'Leipzig', pageWidth: 1800, pageHeight: 2200, scale: 45, adjustPageHeight: true,
    breaks: 'auto', svgViewBox: true, svgRemoveXlink: true, xmlIdSeed: 1 } } as const;
export interface ParsedMusicScore { musicxml: string; render_xml: string; parts: MusicPart[]; notes: ParsedMusicNote[]; marks: MusicScoreMark[]; measure_durations: number[] }

/** Preserve the source bytes separately: inserted IDs belong to the render input. */
export function musicXmlFromBytes(bytes: Uint8Array): string {
  if (bytes[0] !== 0x50 || bytes[1] !== 0x4b) return new TextDecoder().decode(bytes);
  const files = unzipSync(bytes);
  const container = files['META-INF/container.xml'];
  if (!container) throw new Error('Compressed MusicXML is missing META-INF/container.xml');
  const doc = new DOMParser().parseFromString(strFromU8(container), 'text/xml');
  const rootfile = Array.from(doc.getElementsByTagName('rootfile')).find(e => e.getAttribute('media-type') === 'application/vnd.recordare.musicxml+xml') ?? doc.getElementsByTagName('rootfile')[0];
  const filename = rootfile?.getAttribute('full-path');
  if (!filename || !files[filename]) throw new Error('Compressed MusicXML score entry is missing');
  return strFromU8(files[filename]);
}

export function parseMusicScore(musicxml: string): ParsedMusicScore {
  const errors: string[] = [];
  const document = new DOMParser({ errorHandler: { warning: () => {}, error: e => errors.push(e), fatalError: e => errors.push(e) } }).parseFromString(musicxml, 'text/xml');
  if (errors.length || document.documentElement.tagName !== 'score-partwise') throw new Error(`Expected valid score-partwise MusicXML: ${errors.join('; ')}`);
  const root = document.documentElement as unknown as XmlElement;
  const list = child(root, 'part-list');
  const definitions = new Map(children(list ?? root, 'score-part').map(e => [e.getAttribute('id'), e]));
  const parts: MusicPart[] = [], notes: ParsedMusicNote[] = [], marks: MusicScoreMark[] = [], measureDurations: number[] = [];
  for (const [partIndex, part] of children(root, 'part').entries()) {
    const partId = part.getAttribute('id') || `P${partIndex + 1}`;
    if (!/^[A-Za-z_][\w.-]*$/.test(partId)) throw new Error(`MusicXML part ID cannot be used for source mapping: ${partId}`);
    const definition = definitions.get(partId);
    const instrument = definition && child(definition, 'score-instrument');
    const midi = definition && child(definition, 'midi-instrument');
    const program = midi ? number(midi, 'midi-program', 0) : 0;
    const group = program >= 1 && program <= 8 ? '键盘' : program >= 41 && program <= 48 ? '弦乐' : program >= 57 && program <= 64 ? '铜管' : program >= 65 && program <= 80 ? '木管' : '其他';
    const voices = new Set<string>();
    let divisions = 1, transpose = 0, runningBeat = 0, meterBeats = 4;
    for (const [measureIndex, measure] of children(part, 'measure').entries()) {
      let cursor = 0, lastOnset = 0, measureEnd = 0, noteIndex = 0;
      const label = measure.getAttribute('number') || String(measureIndex + 1);
      const firstNote = notes.length, firstMark = marks.length;
      const addMark = (markValue: MusicScoreMarkValue, staff: string | null, beat = cursor) => {
        marks.push({ mark_id: `${partId}-m${measureIndex + 1}-mark${marks.length - firstMark + 1}`, part_id: partId,
          measure: label, measure_index: measureIndex + 1, beat, staff: staff || null, value: markValue,
          score_object_ids: [], provenance_id: MUSIC_SCORE_MARKS_PROVENANCE_ID });
      };
      for (const element of children(measure)) {
        if (element.tagName === 'attributes') {
          divisions = number(element, 'divisions', divisions);
          const transposition = child(element, 'transpose');
          if (transposition) transpose = number(transposition, 'chromatic') + 12 * number(transposition, 'octave-change');
          for (const key of children(element, 'key')) {
            if (child(key, 'fifths')) addMark({ kind: 'key_signature', fifths: number(key, 'fifths'), encoded_mode: value(key, 'mode') || null }, key.getAttribute('number'));
          }
          for (const time of children(element, 'time')) {
            const beats = children(time, 'beats'), types = children(time, 'beat-type');
            const components = beats.map((item, index) => ({ beats: item.textContent!.trim(), beat_type: Number(types[index]?.textContent) }));
            if (components.length) {
              meterBeats = components.reduce((sum, item) => sum + item.beats.split('+').reduce((n, beat) => n + Number(beat), 0) * 4 / item.beat_type, 0);
              addMark({ kind: 'time_signature', components, symbol: time.getAttribute('symbol') || null }, time.getAttribute('number'));
            }
          }
          continue;
        }
        if (element.tagName === 'backup') { cursor -= number(element, 'duration') / divisions; continue; }
        if (element.tagName === 'forward') { cursor += number(element, 'duration') / divisions; measureEnd = Math.max(measureEnd, cursor); continue; }
        if (element.tagName === 'direction' || element.tagName === 'sound') {
          const types = children(element, 'direction-type');
          const words = types.flatMap(type => children(type, 'words').map(word => word.textContent?.trim() || '')).filter(Boolean);
          const metronomeElement = types.flatMap(type => children(type, 'metronome')).find(item => child(item, 'per-minute'));
          const metronome = metronomeElement ? { beat_unit: value(metronomeElement, 'beat-unit'), dots: children(metronomeElement, 'beat-unit-dot').length, per_minute: value(metronomeElement, 'per-minute') } : null;
          const sound = element.tagName === 'sound' ? element : child(element, 'sound');
          const tempo = Number(sound?.getAttribute('tempo'));
          const sound_tempo_quarter_bpm = tempo > 0 && Number.isFinite(tempo) ? tempo : null;
          if (words.length || metronome || sound_tempo_quarter_bpm) {
            const offset = sound && child(sound, 'offset') ? number(sound, 'offset') : number(element, 'offset');
            addMark({ kind: 'direction', words, metronome, sound_tempo_quarter_bpm }, value(element, 'staff') || null, cursor + offset / divisions);
          }
          continue;
        }
        if (element.tagName !== 'note') continue;
        noteIndex++;
        const sourceId = `${partId}-m${measureIndex + 1}-n${noteIndex}`;
        element.setAttribute('id', sourceId);
        const grace = Boolean(child(element, 'grace')), chord = Boolean(child(element, 'chord'));
        const duration = grace ? 0 : number(element, 'duration') / divisions;
        const onset = chord ? lastOnset : cursor;
        if (!chord) { lastOnset = onset; cursor += duration; }
        const pitch = child(element, 'pitch'), rest = Boolean(child(element, 'rest'));
        if (!pitch && !rest) throw new Error(`Unsupported unpitched note ${sourceId}; provide a reviewed pitched transcription for this selected score`);
        const midiPitch = pitch ? ({ C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 }[value(pitch, 'step') as 'C'] + 12 * (number(pitch, 'octave') + 1) + number(pitch, 'alter') + transpose) : null;
        const ties = children(element, 'tie').map(t => t.getAttribute('type'));
        const tie = ties.includes('stop') && ties.includes('start') ? 'continue' : ties.includes('stop') ? 'stop' : ties.includes('start') ? 'start' : 'none';
        const voice = `${value(element, 'staff', '1')}:${value(element, 'voice', '1')}`;
        voices.add(voice);
        notes.push({ source_id: sourceId, part_id: partId, measure: label, measure_index: measureIndex + 1, beat: onset, score_beat: runningBeat + onset, duration_beats: duration, voice, midi_pitch: midiPitch, rest, tie, grace });
        measureEnd = Math.max(measureEnd, onset + duration);
      }
      const measureNotes = notes.slice(firstNote);
      for (const mark of marks.slice(firstMark)) {
        // One nearby note/rest per staff anchors the mark without pretending it is a pitched note.
        const candidates = measureNotes.filter(note => !mark.staff || note.voice.split(':')[0] === mark.staff);
        const anchors = new Map<string, ParsedMusicNote>();
        for (const note of candidates.length ? candidates : measureNotes) {
          const staff = note.voice.split(':')[0], previous = anchors.get(staff);
          if (!previous || Math.abs(note.beat - mark.beat) < Math.abs(previous.beat - mark.beat)) anchors.set(staff, note);
        }
        mark.score_object_ids = [...anchors.values()].map(note => note.source_id);
      }
      const actualDuration = measureEnd || meterBeats;
      measureDurations[measureIndex] = Math.max(measureDurations[measureIndex] ?? 0, actualDuration);
      runningBeat += actualDuration;
    }
    parts.push({ part_id: partId, name: definition ? value(definition, 'part-name', partId) : partId, instrument: instrument ? value(instrument, 'instrument-name', value(definition!, 'part-name', partId)) : definition ? value(definition, 'part-name', partId) : partId, group, voices: [...voices] });
  }
  if (!notes.length) throw new Error('MusicXML contains no score objects');
  return { musicxml, render_xml: new XMLSerializer().serializeToString(document), parts, notes, marks, measure_durations: measureDurations };
}

export interface RenderedMusicScore { pages: MusicScorePage[]; svg: Map<string, string>; score_objects: MusicScoreObject[]; mei: string; midi: Uint8Array; renderer: string }

/** All Verovio execution stays on the build host; the Reader receives only SVG. */
export async function renderMusicScore(parsed: ParsedMusicScore): Promise<RenderedMusicScore> {
  // Keep LGPL Verovio replaceable beside the installed Engine; source runs use the npm package.
  const sidecar = process.env.UNDERSTAND_BOOK_SIDECAR_SELF;
  const sourceRequire = sidecar ? null : createRequire(import.meta.url);
  const moduleUrl = (entry: 'wasm' | 'esm') => pathToFileURL(sidecar
    ? path.join(path.dirname(sidecar), 'resources/music/verovio', entry === 'wasm' ? 'verovio-module.mjs' : 'verovio.mjs')
    : sourceRequire!.resolve(`verovio/${entry}`)).href;
  const { default: createModule } = await import(moduleUrl('wasm'));
  const { VerovioToolkit } = await import(moduleUrl('esm'));
  const toolkit = new VerovioToolkit(await createModule());
  try {
    toolkit.setOptions({ ...MUSIC_SCORE_RENDER_PARAMETERS.options });
    if (!toolkit.loadData(parsed.render_xml)) throw new Error(`Verovio could not load the score: ${toolkit.getLog()}`);
    const pages: MusicScorePage[] = [], svg = new Map<string, string>(), mappings = new Map<string, string>();
    for (let page = 1; page <= toolkit.getPageCount(); page++) {
      const pageId = `page-${page}`, content: string = toolkit.renderToSVG(page);
      const document = new DOMParser().parseFromString(content, 'image/svg+xml');
      const viewBox = document.documentElement.getAttribute('viewBox')?.split(/\s+/).map(Number);
      if (!viewBox || viewBox.length !== 4) throw new Error(`Verovio page ${page} has no viewBox`);
      pages.push({ page_id: pageId, path: `music/score/${pageId}.svg`, width: viewBox[2], height: viewBox[3] });
      svg.set(pageId, content);
      for (const element of Array.from(document.getElementsByTagName('g'))) {
        const id = element.getAttribute('id');
        if (id) mappings.set(id, pageId);
      }
    }
    const score_objects = parsed.notes.map(({ measure_index: _measureIndex, score_beat: _scoreBeat, grace: _grace, ...note }) => {
      const page_id = mappings.get(note.source_id);
      if (!page_id) throw new Error(`Rendered score is missing source object ${note.source_id}; this score needs a reviewed import mapping`);
      return { ...note, page_id, svg_element_id: note.source_id };
    });
    return { pages, svg, score_objects, mei: toolkit.getMEI(), midi: Buffer.from(toolkit.renderToMIDI(), 'base64'), renderer: `Verovio ${toolkit.getVersion()}` };
  } finally { toolkit.destroy(); }
}
