// Music source contracts mirror base-schema/src/music.rs. Audio times use the original
// recording timeline; SVG identifiers are display mappings, never source identities.
import { z } from "zod";
import type { PublishedBookRef } from "./generated/PublishedBookRef";

import type { MusicRecordingRef } from "./generated/MusicRecordingRef";
export type { MusicRecordingRef } from "./generated/MusicRecordingRef";
import type { MusicScoreRef } from "./generated/MusicScoreRef";
export type { MusicScoreRef } from "./generated/MusicScoreRef";
import type { MusicAudioLocation } from "./generated/MusicAudioLocation";
export type { MusicAudioLocation } from "./generated/MusicAudioLocation";
export type { MusicScoreLocation } from "./generated/MusicScoreLocation";
export type { MusicWork } from "./generated/MusicWork";
export type { MusicScore } from "./generated/MusicScore";
export type { MusicRecording } from "./generated/MusicRecording";
export type { MusicRights } from "./generated/MusicRights";
export type { MusicPart } from "./generated/MusicPart";
export type { MusicScorePage } from "./generated/MusicScorePage";
export type { MusicScoreObject } from "./generated/MusicScoreObject";
export type { MusicScoreMark } from "./generated/MusicScoreMark";
export type { MusicScoreMarkValue } from "./generated/MusicScoreMarkValue";
export type { MusicTonality } from "./generated/MusicTonality";
export type { MusicOccurrence } from "./generated/MusicOccurrence";
export type { MusicNoteAlignment } from "./generated/MusicNoteAlignment";
export type { MusicCoverage } from "./generated/MusicCoverage";
export type { MusicProvenance } from "./generated/MusicProvenance";
export type { MusicEvidence } from "./generated/MusicEvidence";
export type { MusicEventRole } from "./generated/MusicEventRole";
import type { MusicVoiceFocus } from "./generated/MusicVoiceFocus";
export type { MusicVoiceFocus } from "./generated/MusicVoiceFocus";
export type { MusicSourceTarget } from "./generated/MusicSourceTarget";
export type { MusicEventRelation } from "./generated/MusicEventRelation";
export type { MusicEvent } from "./generated/MusicEvent";
export type { MusicStructure } from "./generated/MusicStructure";
export type { MusicDemonstration } from "./generated/MusicDemonstration";
import type { MusicSourceManifest } from "./generated/MusicSourceManifest";
export type { MusicSourceManifest } from "./generated/MusicSourceManifest";
import type { MusicQuestionContext } from "./generated/MusicQuestionContext";
export type { MusicQuestionContext } from "./generated/MusicQuestionContext";

const id = z.string().refine(v => v.trim().length > 0);
const time = z.number().finite().nonnegative();
const relativePath = id.refine(v => !/^[/\\]/.test(v) && !v.includes(":") && !v.split(/[/\\]/).includes(".."));
export const MusicRecordingRefZ = z.object({ recording_id: id, revision: id });
export const MusicScoreRefZ = z.object({ score_id: id, revision: id });
export const MusicAudioLocationZ = z.object({ recording: MusicRecordingRefZ, start_ms: time, end_ms: time });
export const MusicScoreLocationZ = z.object({ score: MusicScoreRefZ, source_ids: z.array(id) });
export const MusicVoiceFocusZ: z.ZodType<MusicVoiceFocus> = z.object({ part_id: id, voice: id });
export const MusicScoreObjectZ = z.object({ source_id: id, part_id: id, measure: id, beat: time, duration_beats: time,
  voice: id, midi_pitch: z.number().int().min(0).max(127).nullable(), rest: z.boolean(), tie: z.enum(["none", "start", "continue", "stop"]), page_id: id, svg_element_id: id });
export const MusicTonalityZ = z.object({ tonic: id, mode: id, explanation: id, provenance_ids: z.array(id).min(1) });
export const MusicScoreMarkZ = z.object({ mark_id: id, part_id: id, measure: id, measure_index: z.number().int().positive(), beat: z.number().finite(), staff: id.nullable(),
  score_object_ids: z.array(id).min(1), provenance_id: id,
  value: z.discriminatedUnion('kind', [
    z.object({ kind: z.literal('key_signature'), fifths: z.number().int(), encoded_mode: id.nullable() }),
    z.object({ kind: z.literal('time_signature'), components: z.array(z.object({ beats: id, beat_type: z.number().int().positive() })).min(1), symbol: id.nullable() }),
    z.object({ kind: z.literal('direction'), words: z.array(id), metronome: z.object({ beat_unit: id, dots: z.number().int().nonnegative(), per_minute: id }).nullable(), sound_tempo_quarter_bpm: time.positive().nullable() }),
  ]),
});
export const MusicSourceManifestZ: z.ZodType<MusicSourceManifest> = z.object({
  version: z.literal("music_source_manifest.v1"), book_id: id, title: id,
  work: z.object({ work_id: id, title: id, composer: id, movement: id.nullable(), tonality: MusicTonalityZ.nullable().optional() }),
  score: z.object({ source: MusicScoreRefZ, title: id, musicxml_path: relativePath }),
  recording: z.object({ source: MusicRecordingRefZ, title: id, performers: id, media_path: relativePath, content_type: id,
    duration_ms: time.positive(), kind: z.enum(["performance", "score_synthesis"]) }),
  rights: z.array(z.object({ asset_kind: z.enum(["recording", "score", "reference", "font", "soundfont"]), title: id, url: id,
    license: id, attribution: id, distribution: z.enum(["allowed", "local_only"]) })),
  parts: z.array(z.object({ part_id: id, name: id, instrument: id, group: id, voices: z.array(id).min(1) })),
  pages: z.array(z.object({ page_id: id, path: relativePath, width: time.positive(), height: time.positive() })),
  score_objects: z.array(MusicScoreObjectZ),
  score_marks: z.array(MusicScoreMarkZ).optional(),
  occurrences: z.array(z.object({ occurrence_id: id, order: z.number().int().nonnegative().max(0xffffffff), measure: id,
    start_ms: time, end_ms: time, score_object_ids: z.array(id).min(1) })),
  note_alignments: z.array(z.object({ occurrence_id: id, score_object_id: id, start_ms: time, end_ms: time, attack: z.boolean(), provenance_id: id, unobserved: z.boolean().optional() })),
  coverage: z.array(z.object({ start_ms: time, end_ms: time, granularity: z.enum(["phrase", "measure", "note"]),
    method: z.enum(["manual", "audio_alignment", "score_synthesis"]), status: z.enum(["accepted", "pending", "failed"]), provenance_id: id, detail: id })),
  provenance: z.array(z.object({ provenance_id: id, kind: z.enum(["score", "audio_analysis", "human_listening", "reference", "interpretation", "score_synthesis"]),
    description: id, source_url: id.nullable(), processor: id.nullable() })),
  events: z.array(z.object({ event_id: id, title: id, explanation: id, listen_for: id, start_ms: time, end_ms: time,
    roles: z.array(z.object({ part_id: id, role: id, voice_ids: z.array(id).optional(), theme_id: id.nullable().optional() })),
    evidence: z.array(z.object({ provenance_id: id, score_object_ids: z.array(id), audio: MusicAudioLocationZ.nullable() })).min(1),
    relations: z.array(z.object({ target_event_id: id, kind: z.enum(["repetition", "variation", "handoff", "response", "support", "contrast"]), description: id })),
    concept_tags: z.array(id), occurrence_ids: z.array(id).optional() })),
  structure: z.array(z.object({ section_id: id, parent_id: id.nullable(), title: id, description: id, start_ms: time.nullable(), end_ms: time.nullable(),
    event_ids: z.array(id), coverage: z.enum(["detailed", "navigation_only"]) })),
  demonstrations: z.array(z.object({ demonstration_id: id, title: id, kind: z.literal("score_synthesis"), media_path: relativePath, content_type: id, duration_ms: time.positive(),
    part_ids: z.array(id).min(1), score_object_ids: z.array(id).min(1), provenance_id: id })),
});
export const MusicQuestionContextZ: z.ZodType<MusicQuestionContext> = z.object({ draft_id: id,
  publication: z.object({ book_id: id, publication_id: id }), audio: MusicAudioLocationZ,
  score: MusicScoreLocationZ.nullable(), occurrence_id: id.nullable(), focused_part_ids: z.array(id), focused_voices: z.array(MusicVoiceFocusZ).optional(),
  image: z.object({ png_base64: z.string().min(1).max(5_592_408), width: z.number().int().min(1).max(2048),
    height: z.number().int().min(1).max(2048), origin: z.enum(['selection', 'upload']) }).nullable().optional() });

function requireValid(ok: boolean, field: string): asserts ok {
  if (!ok) throw new Error(`invalid music contract: ${field}`);
}
function validInterval(start: number, end: number, duration: number): boolean { return start >= 0 && start < end && end <= duration; }
function unique(values: string[]): boolean { return new Set(values).size === values.length; }
function sameRecording(a: MusicRecordingRef, b: MusicRecordingRef): boolean { return a.recording_id === b.recording_id && a.revision === b.revision; }
function sameScore(a: MusicScoreRef, b: MusicScoreRef): boolean { return a.score_id === b.score_id && a.revision === b.revision; }
export function validateMusicAudioLocation(input: unknown, manifest: MusicSourceManifest): MusicAudioLocation {
  const location = MusicAudioLocationZ.parse(input);
  requireValid(sameRecording(location.recording, manifest.recording.source) && validInterval(location.start_ms, location.end_ms, manifest.recording.duration_ms), "audio location");
  return location;
}
export function validateMusicSourceManifest(input: unknown): MusicSourceManifest {
  const m = MusicSourceManifestZ.parse(input), duration = m.recording.duration_ms;
  const identities: [string, string[]][] = [["part", m.parts.map(p => p.part_id)], ["page", m.pages.map(p => p.page_id)],
    ["score object", m.score_objects.map(p => p.source_id)], ["occurrence", m.occurrences.map(p => p.occurrence_id)],
    ["provenance", m.provenance.map(p => p.provenance_id)], ["event", m.events.map(p => p.event_id)],
    ["section", m.structure.map(p => p.section_id)], ["demonstration", m.demonstrations.map(p => p.demonstration_id)]];
  for (const [kind, ids] of identities) requireValid(unique(ids), `${kind} identity`);
  const parts = new Map(m.parts.map(p => [p.part_id, p])), pages = new Set(m.pages.map(p => p.page_id));
  const notes = new Map(m.score_objects.map(n => [n.source_id, n])), occurrences = new Map(m.occurrences.map(o => [o.occurrence_id, o]));
  const provenance = new Map(m.provenance.map(p => [p.provenance_id, p])), events = new Set(m.events.map(e => e.event_id));
  const sections = new Map(m.structure.map(s => [s.section_id, s]));
  const measureObjects = new Map<string, number>();
  for (const n of m.score_objects) measureObjects.set(n.measure, (measureObjects.get(n.measure) ?? 0) + 1);
  for (const p of m.parts) requireValid(unique(p.voices), "part");
  if (m.work.tonality) {
    const tonality = m.work.tonality;
    requireValid(unique(tonality.provenance_ids) && tonality.provenance_ids.every(id => ['score', 'reference'].includes(provenance.get(id)?.kind ?? '')), 'work tonality');
  }
  requireValid(unique((m.score_marks ?? []).map(mark => mark.mark_id)), 'score mark identity');
  for (const mark of m.score_marks ?? []) {
    requireValid(parts.has(mark.part_id) && unique(mark.score_object_ids)
      && mark.score_object_ids.every(id => { const note = notes.get(id); return note?.part_id === mark.part_id && note.measure === mark.measure; })
      && provenance.get(mark.provenance_id)?.kind === 'score', 'score mark source');
    if (mark.value.kind === 'direction') requireValid(!!(mark.value.words.length || mark.value.metronome || mark.value.sound_tempo_quarter_bpm), 'score mark value');
  }
  const svgIds = new Map<string, Set<string>>();
  for (const n of m.score_objects) {
    const mapped = svgIds.get(n.page_id) ?? new Set<string>();
    requireValid(!!parts.get(n.part_id)?.voices.includes(n.voice) && pages.has(n.page_id)
      && (n.rest ? n.midi_pitch === null && n.tie === "none" : n.midi_pitch !== null) && !mapped.has(n.svg_element_id), "score object");
    mapped.add(n.svg_element_id); svgIds.set(n.page_id, mapped);
  }
  for (let i = 0; i < m.occurrences.length; i++) {
    const o = m.occurrences[i], previous = m.occurrences[i - 1];
    requireValid(validInterval(o.start_ms, o.end_ms, duration) && unique(o.score_object_ids)
      && o.score_object_ids.every(id => notes.get(id)?.measure === o.measure)
      && measureObjects.get(o.measure) === o.score_object_ids.length
      && (!previous || previous.order < o.order && previous.end_ms <= o.start_ms), "occurrence");
  }
  const aligned = new Map<string, Set<string>>();
  for (const a of m.note_alignments) {
    const o = occurrences.get(a.occurrence_id), n = notes.get(a.score_object_id), mapped = aligned.get(a.occurrence_id) ?? new Set<string>();
    const p = provenance.get(a.provenance_id);
    requireValid(validInterval(a.start_ms, a.end_ms, duration) && !!p && (p.kind === "human_listening" || p.kind === "audio_analysis" || p.kind === "score_synthesis" && m.recording.kind === "score_synthesis")
      && !!o && o.score_object_ids.includes(a.score_object_id) && a.start_ms >= o.start_ms && a.start_ms < o.end_ms
      && !!n && !n.rest && (!a.attack || !a.unobserved && n.tie !== "continue" && n.tie !== "stop") && !mapped.has(a.score_object_id), "note alignment");
    mapped.add(a.score_object_id); aligned.set(a.occurrence_id, mapped);
  }
  for (const granularity of ["phrase", "measure", "note"]) {
    const intervals = m.coverage.filter(c => c.granularity === granularity).sort((a, b) => a.start_ms - b.start_ms);
    requireValid(intervals.every((c, i) => i === 0 || intervals[i - 1].end_ms <= c.start_ms), "coverage overlap");
  }
  for (const c of m.coverage) {
    const p = provenance.get(c.provenance_id);
    requireValid(validInterval(c.start_ms, c.end_ms, duration) && !!p, "coverage");
    if (c.status !== "accepted") continue;
    requireValid(c.method === "manual" ? p.kind === "human_listening" || p.kind === "audio_analysis" : c.method === "audio_alignment"
      ? p.kind === "audio_analysis" : m.recording.kind === "score_synthesis" && p.kind === "score_synthesis", "coverage method");
    const selected = m.occurrences.filter(o => o.start_ms < c.end_ms && o.end_ms > c.start_ms);
    requireValid(selected.length > 0, "coverage without occurrences");
    let cursor = c.start_ms;
    for (const o of selected) { requireValid(o.start_ms <= cursor, "coverage occurrence gap"); cursor = Math.max(cursor, o.end_ms); }
    requireValid(cursor >= c.end_ms, "coverage occurrence gap");
    if (c.granularity === "note") requireValid(selected.every(o => o.score_object_ids.every(id => notes.get(id)!.rest || aligned.get(o.occurrence_id)?.has(id))), "note coverage gap");
  }
  for (const e of m.events) {
    requireValid(validInterval(e.start_ms, e.end_ms, duration), "event");
    requireValid(unique(e.occurrence_ids ?? []) && (e.occurrence_ids ?? []).every(id => occurrences.has(id)), "event occurrence");
    for (const role of e.roles) requireValid(parts.has(role.part_id) && (role.voice_ids ?? []).every(v => parts.get(role.part_id)!.voices.includes(v))
      && unique(role.voice_ids ?? []), "event role");
    for (const evidence of e.evidence) {
      const p = provenance.get(evidence.provenance_id);
      requireValid(!!p && evidence.score_object_ids.every(id => notes.has(id)), "event evidence");
      if (evidence.audio) validateMusicAudioLocation(evidence.audio, m);
      requireValid(evidence.score_object_ids.length > 0 || evidence.audio !== null || p.kind === "reference" || p.kind === "interpretation", "event evidence location");
    }
    for (const relation of e.relations) requireValid(relation.target_event_id !== e.event_id && events.has(relation.target_event_id), "event relation");
  }
  for (const s of m.structure) {
    requireValid(s.event_ids.every(id => events.has(id)), "structure");
    requireValid(s.start_ms === null && s.end_ms === null ? s.coverage === "navigation_only" : s.start_ms !== null && s.end_ms !== null && validInterval(s.start_ms, s.end_ms, duration), "structure location");
    const visited = new Set([s.section_id]); let parent = s.parent_id;
    while (parent !== null) { requireValid(!visited.has(parent) && sections.has(parent), "structure parent"); visited.add(parent); parent = sections.get(parent)!.parent_id; }
  }
  for (const d of m.demonstrations) requireValid(d.part_ids.every(id => parts.has(id)) && d.score_object_ids.every(id => notes.has(id))
    && provenance.get(d.provenance_id)?.kind === "score_synthesis", "demonstration");
  return m;
}
export function validateMusicQuestionContext(input: unknown, manifest: MusicSourceManifest, publication: PublishedBookRef): MusicQuestionContext {
  const q = MusicQuestionContextZ.parse(input);
  requireValid(q.publication.book_id === publication.book_id && q.publication.publication_id === publication.publication_id && publication.book_id === manifest.book_id, "question publication");
  validateMusicAudioLocation(q.audio, manifest);
  if (q.score) requireValid(sameScore(q.score.score, manifest.score.source) && q.score.source_ids.length > 0
    && q.score.source_ids.every(id => manifest.score_objects.some(o => o.source_id === id)), "question score");
  requireValid((q.occurrence_id === null || manifest.occurrences.some(o => o.occurrence_id === q.occurrence_id && o.start_ms < q.audio.end_ms && o.end_ms > q.audio.start_ms))
    && q.focused_part_ids.every(id => manifest.parts.some(p => p.part_id === id)), "question focus");
  const focusedVoices = new Map<string, Set<string>>();
  for (const focus of q.focused_voices ?? []) {
    validateMusicVoiceFocus(focus, manifest);
    const voices = focusedVoices.get(focus.part_id) ?? new Set<string>();
    requireValid(!voices.has(focus.voice), "question voice focus");
    voices.add(focus.voice); focusedVoices.set(focus.part_id, voices);
  }
  return q;
}

export function validateMusicVoiceFocus(input: unknown, manifest: MusicSourceManifest): MusicVoiceFocus {
  const focus = MusicVoiceFocusZ.parse(input);
  requireValid(manifest.parts.some(p => p.part_id === focus.part_id && p.voices.includes(focus.voice)), "voice focus");
  return focus;
}

export function validateMusicMaterialSuccessor(input: unknown, previous: MusicSourceManifest): MusicSourceManifest {
  const next = validateMusicSourceManifest(input);
  validateMusicSourceManifest(previous);
  if (next.book_id !== previous.book_id) return next;
  requireValid(next.work.work_id === previous.work.work_id && sameRecording(next.recording.source, previous.recording.source)
    && next.recording.duration_ms === previous.recording.duration_ms && next.recording.kind === previous.recording.kind
    && sameScore(next.score.source, previous.score.source), "immutable material source");
  const notes = new Map(next.score_objects.map(n => [n.source_id, n]));
  for (const old of previous.score_objects) {
    const n = notes.get(old.source_id);
    requireValid(!!n && n.part_id === old.part_id && n.measure === old.measure && n.beat === old.beat && n.duration_beats === old.duration_beats
      && n.voice === old.voice && n.midi_pitch === old.midi_pitch && n.rest === old.rest && n.tie === old.tie, "immutable score location");
  }
  return next;
}

/** A capability means usable coverage exists; coverage intervals define its extent. */
export function musicCapabilities(manifest: MusicSourceManifest): string[] {
  const capabilities = ["music"];
  if (manifest.pages.length && manifest.score_objects.length) capabilities.push("music_score");
  if (manifest.events.length) capabilities.push("music_events");
  if (manifest.note_alignments.length && manifest.coverage.some(c => c.status === "accepted" && c.granularity === "note")) capabilities.push("music_note_alignment");
  return capabilities;
}
