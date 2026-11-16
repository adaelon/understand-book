// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { answerDisplayParts } from './answer-markdown';
import { answerNoteExcerpt } from './answer-note-selection';
import { playPiano, stopPiano, pianoPitches, pianoExample } from './music-example-player';
import MusicExamplePiano from './components/MusicExamplePiano.vue';
import type { MusicExample } from './generated/MusicExample';
const sampleLoader = vi.hoisted(() => vi.fn(async (_context: unknown, pitches: number[]) => new Map(pitches.map(pitch => [pitch, {} as AudioBuffer]))));
vi.mock('./piano-samples', () => ({ loadPianoSamples: sampleLoader, pianoSample: (pitch: number) => ({ root: pitch, offset_frames: 0 }), pianoTuning: () => 0 }));
const example: MusicExample = { ref_id: 'm21', title: '谱面合成 · 第一小提琴 · 第 21 小节', kind: 'score', tempo_bpm: 66, source_ref_id: null,
  label: 'C♯4 + E4 + A4 + E5', duration_ms: 1818,
  notes: [61, 64, 69, 76].map(midi_pitch => ({ midi_pitch, start_ms: 909, end_ms: 1591 })) };
afterEach(() => { stopPiano(true); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

it.each([0, 127])('only offers playable MIDI keys for an example at pitch %d', async pitch => {
  const wrapper = mount(MusicExamplePiano);
  try {
    pianoExample.value = { ...example, notes: [{ midi_pitch: pitch, start_ms: 0, end_ms: 500 }] };
    await wrapper.vm.$nextTick();
    const keys = wrapper.findAll('[data-pitch]').map(key => Number(key.attributes('data-pitch')));
    expect(keys).toContain(pitch);
    expect(keys.every(key => key >= 0 && key <= 127)).toBe(true);
  } finally { wrapper.unmount(); }
});

it('keeps inline examples in a table cell and retains copyable note labels', () => {
  const parts = [{ kind: 'markdown' as const, text: '|小节|试听|\n|---|---|\n|21|' }, { kind: 'music_example' as const, example }, { kind: 'markdown' as const, text: '|\n' }];
  const display = answerDisplayParts(parts, []);
  expect(display).toHaveLength(1);
  if (display[0].kind !== 'html') throw Error('Expected HTML');
  const root = document.createElement('div'); root.innerHTML = display[0].html;
  expect(root.querySelectorAll('td .music-example-note')).toHaveLength(4);
  const range = document.createRange(); range.selectNodeContents(root.querySelector('.music-example-inline')!);
  expect(answerNoteExcerpt(range, display[0].markdown, root)).toContain('C♯4 + E4 + A4 + E5');
  expect(answerDisplayParts(parts, [], true)[0]).toMatchObject({ kind: 'html', html: expect.stringContaining('disabled') });
});

it('offers the continuous motive as one clickable, copyable phrase', () => {
  const phrase = { ...example, label: '第 17–21 小节连奏', duration_ms: 9091 };
  const parts = [{ kind: 'markdown' as const, text: '完整动机 ' }, { kind: 'music_example' as const, example: phrase }];
  const display = answerDisplayParts(parts, []);
  if (display[0].kind !== 'html') throw Error('Expected HTML');
  const root = document.createElement('div'); root.innerHTML = display[0].html;
  const button = root.querySelector<HTMLButtonElement>('.music-example-note')!;
  expect(button.textContent).toBe('第 17–21 小节连奏');
  expect(button.dataset.musicExampleIndex).toBe('1');
  expect(button.dataset.musicPitch).toBeUndefined();
  const range = document.createRange(); range.selectNodeContents(button);
  expect(answerNoteExcerpt(range, display[0].markdown, root)).toBe(phrase.label);
});

it('schedules the chord on one clock and lights exactly four separate keys after the rest', async () => {
  const starts: number[] = [];
  const parameter = () => ({ value: 0, setValueAtTime() {}, linearRampToValueAtTime() {}, exponentialRampToValueAtTime() {} });
  let now = 10, callback: FrameRequestCallback | undefined;
  vi.stubGlobal('requestAnimationFrame', (next: FrameRequestCallback) => { callback = next; return 1; });
  vi.stubGlobal('cancelAnimationFrame', () => {});
  vi.stubGlobal('AudioContext', class {
    get currentTime() { return now; } destination = {};
    async resume() {}
    createGain() { return { gain: parameter(), connect() {}, disconnect() {} }; }
    createDynamicsCompressor() { return { threshold: parameter(), knee: parameter(), ratio: parameter(), attack: parameter(), release: parameter(), connect() {}, disconnect() {} }; }
    createBufferSource() { return { buffer: null, playbackRate: parameter(), detune: parameter(), connect() {}, disconnect() {}, start(time: number) { starts.push(time); }, stop() {}, onended: null }; }
  });
  const wrapper = mount(MusicExamplePiano);
  await playPiano(example);
  expect(starts).toHaveLength(4); expect(new Set(starts).size).toBe(1);
  expect(pianoPitches.value).toEqual([]);
  now = 11.1; callback!(0); await wrapper.vm.$nextTick();
  expect(pianoPitches.value).toEqual([61, 64, 69, 76]);
  expect(wrapper.findAll('.piano-key.sounding')).toHaveLength(4);
  now = 11.7; callback!(0); expect(pianoPitches.value).toEqual([]);
  window.dispatchEvent(new Event('music-original-start'));
  await wrapper.vm.$nextTick(); expect(wrapper.find('.music-example-piano').exists()).toBe(false);
  let ready!: (value: Map<number, AudioBuffer>) => void;
  sampleLoader.mockImplementationOnce(() => new Promise(resolve => { ready = resolve; }));
  const pending = playPiano(example);
  await Promise.resolve(); await Promise.resolve();
  stopPiano(true);
  ready(new Map());
  await pending;
  expect(starts).toHaveLength(4);
  wrapper.unmount();
});
