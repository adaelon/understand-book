import type { ParsedMusicNote } from './music-score';

export interface MusicDemonstrationRequest { demonstration_id: string; title: string; score_object_ids: string[]; tempo_bpm: number }
// This version also covers tie joining and sample encoding below.
export const MUSIC_DEMONSTRATION_PARAMETERS = { processor: 'score-oscillator.v3', sample_rate: 22050, grace_seconds: 0.08,
  release_seconds: 0.08, tail_seconds: 0.15, attack_seconds: 0.012, decay: 0.35, harmonic: 0.18, amplitude: 26000 } as const;

/** A deliberately simple score synthesizer, never an extracted performance stem. */
export function synthesizeScoreDemonstration(notes: ParsedMusicNote[], tempoBpm: number): { wav: Buffer; duration_ms: number; timings: Array<{ source_id: string; start_ms: number; end_ms: number; attack: boolean }> } {
  if (!Number.isFinite(tempoBpm) || tempoBpm <= 0) throw new Error('A score demonstration needs a positive tempo');
  const pitched = notes.filter(n => !n.rest && n.midi_pitch !== null && !(n.notation?.visible === false && n.notation.playback_dynamics === 0));
  if (!pitched.length) throw new Error('A score demonstration needs pitched score objects');
  const earliest = Math.min(...pitched.map(n => n.score_beat));
  const secondsPerBeat = 60 / tempoBpm;
  const ordered = pitched.sort((a, b) => a.score_beat - b.score_beat);
  const graceGroups = new Map<string, ParsedMusicNote[][]>();
  for (const note of ordered.filter(note => note.grace)) {
    const key = JSON.stringify([note.part_id, note.voice, note.score_beat]);
    const groups = graceGroups.get(key) ?? [];
    if (!note.notation?.chord || !groups.length) groups.push([]);
    groups.at(-1)!.push(note); graceGroups.set(key, groups);
  }
  const graceOffsets = new Map<ParsedMusicNote, number>();
  for (const groups of graceGroups.values()) for (const [index, group] of groups.entries()) {
    for (const note of group) graceOffsets.set(note, (index - groups.length) * MUSIC_DEMONSTRATION_PARAMETERS.grace_seconds);
  }
  const onset = (note: ParsedMusicNote) => (note.score_beat - earliest) * secondsPerBeat + (graceOffsets.get(note) ?? 0);
  // Opening grace notes need their own lead-in; shift all voices together so the
  // main chord still lands on one beat instead of colliding with the ornament.
  const leadIn = -Math.min(0, ...ordered.map(onset));
  const events: Array<{ start: number; end: number; pitch: number; voice: string; part: string }> = [];
  const timings: Array<{ source_id: string; start_ms: number; end_ms: number; attack: boolean }> = [];
  for (const note of ordered) {
    const start = onset(note) + leadIn;
    const end = start + (note.grace ? MUSIC_DEMONSTRATION_PARAMETERS.grace_seconds : note.duration_beats * secondsPerBeat);
    const previous = [...events].reverse().find(e => e.pitch === note.midi_pitch && e.voice === note.voice && e.part === note.part_id && Math.abs(e.end - start) < 0.002);
    timings.push({ source_id: note.source_id, start_ms: start * 1000, end_ms: end * 1000, attack: !((note.tie === 'stop' || note.tie === 'continue') && previous) });
    if ((note.tie === 'stop' || note.tie === 'continue') && previous) previous.end = end;
    else events.push({ start, end, pitch: note.midi_pitch!, voice: note.voice, part: note.part_id });
  }
  const rate = MUSIC_DEMONSTRATION_PARAMETERS.sample_rate, duration = Math.max(...events.map(e => e.end)) + MUSIC_DEMONSTRATION_PARAMETERS.tail_seconds;
  const audio = new Float32Array(Math.ceil(duration * rate));
  for (const event of events) {
    const frequency = 440 * 2 ** ((event.pitch - 69) / 12), start = Math.floor(event.start * rate), end = Math.min(audio.length, Math.ceil((event.end + MUSIC_DEMONSTRATION_PARAMETERS.release_seconds) * rate));
    for (let index = start; index < end; index++) {
      const time = (index - start) / rate, release = Math.max(0, (index / rate - event.end) / MUSIC_DEMONSTRATION_PARAMETERS.release_seconds);
      const envelope = Math.min(1, time / MUSIC_DEMONSTRATION_PARAMETERS.attack_seconds) * Math.max(0, 1 - release) * Math.exp(-MUSIC_DEMONSTRATION_PARAMETERS.decay * time);
      audio[index] += envelope * (Math.sin(2 * Math.PI * frequency * time) + MUSIC_DEMONSTRATION_PARAMETERS.harmonic * Math.sin(4 * Math.PI * frequency * time));
    }
  }
  let peak = 1; for (const sample of audio) peak = Math.max(peak, Math.abs(sample));
  const wav = Buffer.alloc(44 + audio.length * 2);
  wav.write('RIFF'); wav.writeUInt32LE(wav.length - 8, 4); wav.write('WAVEfmt ', 8); wav.writeUInt32LE(16, 16); wav.writeUInt16LE(1, 20); wav.writeUInt16LE(1, 22); wav.writeUInt32LE(rate, 24); wav.writeUInt32LE(rate * 2, 28); wav.writeUInt16LE(2, 32); wav.writeUInt16LE(16, 34); wav.write('data', 36); wav.writeUInt32LE(audio.length * 2, 40);
  audio.forEach((sample, index) => wav.writeInt16LE(Math.round(sample / peak * MUSIC_DEMONSTRATION_PARAMETERS.amplitude), 44 + index * 2));
  return { wav, duration_ms: audio.length / rate * 1000, timings };
}
