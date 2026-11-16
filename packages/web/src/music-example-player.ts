import { ref, shallowRef } from 'vue';
import type { MusicExample } from './generated/MusicExample';
import { loadPianoSamples, pianoSample, pianoTuning } from './piano-samples';
import { t } from './i18n';

export const pianoExample = shallowRef<MusicExample | null>(null);
export const pianoPitches = ref<number[]>([]);
export const pianoPlaying = ref(false);
export const pianoError = ref('');
export const pianoLoading = ref(false);
let context: AudioContext | undefined;
let voices: AudioBufferSourceNode[] = [];
let output: GainNode | undefined;
let limiter: DynamicsCompressorNode | undefined;
let frame = 0, generation = 0;

export function pitchName(pitch: number) {
  return `${['C', 'C♯', 'D', 'D♯', 'E', 'F', 'F♯', 'G', 'G♯', 'A', 'A♯', 'B'][pitch % 12]}${Math.floor(pitch / 12) - 1}`;
}

export function stopPiano(close = false) {
  generation++;
  cancelAnimationFrame(frame);
  output?.disconnect(); output = undefined;
  limiter?.disconnect(); limiter = undefined;
  for (const voice of voices) { try { voice.stop(); } catch { /* Already ended. */ } }
  voices = [];
  pianoPlaying.value = false; pianoLoading.value = false; pianoPitches.value = [];
  if (close) pianoExample.value = null;
}

export async function playPiano(example: MusicExample, pitch?: number) {
  stopPiano();
  const version = generation;
  pianoExample.value = example; pianoError.value = '';
  try {
    window.dispatchEvent(new Event('music-example-start'));
    context ??= new AudioContext();
    await context.resume();
    if (version !== generation) return;
    const notes = pitch === undefined ? example.notes : [{ midi_pitch: pitch, start_ms: 0, end_ms: 700 }];
    const duration = pitch === undefined ? example.duration_ms : 700;
    pianoLoading.value = true;
    const buffers = await loadPianoSamples(context, notes.map(note => note.midi_pitch));
    if (version !== generation) return;
    pianoLoading.value = false;
    output = context.createGain(); output.gain.value = 0.9;
    output.connect(context.destination);
    limiter = context.createDynamicsCompressor();
    limiter.threshold.value = -8; limiter.knee.value = 12; limiter.ratio.value = 8;
    limiter.attack.value = 0.003; limiter.release.value = 0.12;
    limiter.connect(output);
    const start = context.currentTime + 0.025;
    const release = 0.14;
    for (const note of notes) {
      const onset = start + note.start_ms / 1000, end = start + note.end_ms / 1000;
      const sample = pianoSample(note.midi_pitch);
      const source = context.createBufferSource(), envelope = context.createGain();
      source.buffer = buffers.get(sample.root)!;
      source.playbackRate.value = 2 ** ((note.midi_pitch - sample.root) / 12);
      source.detune.value = pianoTuning(note.midi_pitch);
      envelope.gain.setValueAtTime(0, onset);
      envelope.gain.linearRampToValueAtTime(1, Math.min(end, onset + 0.003));
      envelope.gain.setValueAtTime(1, end);
      envelope.gain.exponentialRampToValueAtTime(0.001, end + release);
      source.connect(envelope); envelope.connect(limiter);
      source.onended = () => { source.disconnect(); envelope.disconnect(); };
      source.start(onset, sample.offset_frames / 48000);
      source.stop(end + release + 0.01); voices.push(source);
    }
    const audioEnd = Math.max(duration, ...notes.map(note => note.end_ms + release * 1000));
    pianoPlaying.value = true;
    function tick() {
      if (version !== generation || !context) return;
      const ms = (context.currentTime - start) * 1000;
      pianoPitches.value = [...new Set(notes.filter(note => note.start_ms <= ms && ms < note.end_ms).map(note => note.midi_pitch))];
      if (ms >= audioEnd + 20) { stopPiano(); return; }
      frame = requestAnimationFrame(tick);
    }
    tick();
  } catch (error) {
    if (version !== generation) return;
    stopPiano(); pianoError.value = t('钢琴音色暂未加载成功，请点重听。');
  }
}

window.addEventListener('music-original-start', () => stopPiano(true));
window.addEventListener('pagehide', () => stopPiano(true));
