// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import type { MusicCommand, MusicContext, MusicPlaybackReceipt, MusicQuestionContext } from './music-api';
const registered = vi.hoisted(() => ({ value: null as { accepted(receipt: MusicPlaybackReceipt): void } | null }));
vi.mock('./music-interaction', () => ({ registerMusicPlayer: (host: typeof registered.value) => { registered.value = host; return () => { registered.value = null; }; } }));
import MusicReaderPane from './components/MusicReaderPane.vue';
const context = { publication: { book_id: 'music', publication_id: 'pub' }, audio_url: '/recording.mp3', score_pages: [], demonstrations: [], manifest: { version: 'music_source_manifest.v1', book_id: 'music', title: '作品', work: { work_id: 'w', title: '作品', composer: '作曲家', movement: null }, recording: { source: { recording_id: 'take', revision: 'r1' }, media_path: 'audio.mp3', content_type: 'audio/mpeg', duration_ms: 10000, title: '录音', performers: '演奏家', kind: 'performance' }, score: { source: { score_id: 'score', revision: 's1' }, title: '谱本', musicxml_path: 'score.xml' }, rights: [], parts: [], pages: [], score_objects: [], occurrences: [], note_alignments: [], coverage: [], provenance: [], events: [], structure: [], demonstrations: [] } } satisfies MusicContext;
function command(id: string, action?: MusicCommand['request']['action']): MusicCommand { return { command_id: id, session_id: 'session', run_turn_id: 'turn', request: { operation: action ? 'act' : 'observe', publication: context.publication, recording: context.manifest.recording.source, based_on: action ? 'server-receipt' : undefined, action } }; }
const listeningContext: MusicContext = {
  ...context,
  demonstrations: [{ demonstration_id: 'upper-line', url: '/upper-line.wav' }],
  manifest: {
    ...context.manifest,
    events: [
      { event_id: 'original-cue', title: '听旋律的起伏', explanation: '旋律先向上，再回落。', listen_for: '先听最高的一条旋律，留意它怎样向上再回落。', start_ms: 1000, end_ms: 3000, roles: [], evidence: [], relations: [], concept_tags: [] },
      { event_id: 'demo-cue', title: '单独听上声部', explanation: '示范突出上声部。', listen_for: '这次只听上声部，留意短句末尾停在哪里。', start_ms: 3000, end_ms: 5000, roles: [], evidence: [], relations: [], concept_tags: [] },
    ],
    demonstrations: [{ demonstration_id: 'upper-line', title: '上声部示范', kind: 'score_synthesis', media_path: 'upper-line.wav', content_type: 'audio/wav', duration_ms: 3150.49, part_ids: [], score_object_ids: [], provenance_id: 'synthesis' }],
  },
};
function mockMediaPlayback(onPlay?: (audio: HTMLMediaElement) => void) {
  const paused = new WeakMap<HTMLMediaElement, boolean>();
  vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
  vi.spyOn(HTMLMediaElement.prototype, 'paused', 'get').mockImplementation(function (this: HTMLMediaElement) { return paused.get(this) ?? true; });
  const play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(function (this: HTMLMediaElement) { onPlay?.(this); paused.set(this, false); return Promise.resolve(); });
  const pause = vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(function (this: HTMLMediaElement) { paused.set(this, true); });
  return { play, pause };
}
function accept(receipt: MusicPlaybackReceipt) { registered.value!.accepted({ ...receipt, receipt_id: 'server-receipt' }); }
afterEach(() => { vi.restoreAllMocks(); localStorage.clear(); });
describe('mounted music player receipts', () => {
  it('opens a curated excerpt on the original recording time axis and preserves a saved listener position', async () => {
    vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
    vi.spyOn(HTMLMediaElement.prototype, 'duration', 'get').mockReturnValue(10);
    const excerpt: MusicContext = { ...context, manifest: { ...context.manifest, structure: [{ section_id: 'excerpt', parent_id: null, title: '选段', start_ms: 3000, end_ms: 8000, coverage: 'detailed', event_ids: [], description: '' }] } };
    let wrapper = mount(MusicReaderPane, { props: { context: excerpt, owner: 'test' } });
    await wrapper.get('audio').trigger('loadedmetadata'); expect(wrapper.get('audio').element.currentTime).toBe(3);
    wrapper.vm.seek(5500); wrapper.unmount();
    wrapper = mount(MusicReaderPane, { props: { context: excerpt, owner: 'test' } });
    await wrapper.get('audio').trigger('loadedmetadata'); expect(wrapper.get('audio').element.currentTime).toBe(5.5);
    wrapper.unmount();
  });
  it('awaits the actual play promise and reports its rejection', async () => {
    vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
    const play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockRejectedValue(new Error('NotAllowedError'));
    const wrapper = mount(MusicReaderPane, { props: { context, owner: 'test' } });
    const observed = await wrapper.vm.execute(command('observe'));
    registered.value!.accepted({ ...observed, receipt_id: 'server-receipt' });
    const receipt = await wrapper.vm.execute(command('play', { kind: 'play' }));
    expect(play).toHaveBeenCalledOnce(); expect(receipt.status).toBe('failed'); expect(receipt.error).toContain('NotAllowedError');
    wrapper.unmount();
  });
  it('requires a fresh observation after the listener changes the controls', async () => {
    vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
    const wrapper = mount(MusicReaderPane, { props: { context, owner: 'test' } });
    const observed = await wrapper.vm.execute(command('observe'));
    registered.value!.accepted({ ...observed, receipt_id: 'server-receipt' });
    await wrapper.get('[aria-label="播放速度"]').setValue('0.75');
    const receipt = await wrapper.vm.execute(command('pause', { kind: 'pause' }));
    expect(receipt.status).toBe('failed'); expect(receipt.error).toContain('重新观察'); expect(receipt.playback_rate).toBe(.75);
    wrapper.unmount();
  });
  it('does not acknowledge a seek until the media element finishes seeking', async () => {
    vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
    const wrapper = mount(MusicReaderPane, { props: { context, owner: 'test' } });
    const observed = await wrapper.vm.execute(command('observe'));
    registered.value!.accepted({ ...observed, receipt_id: 'server-receipt' });
    const audio = wrapper.get('audio').element;
    Object.defineProperty(audio, 'seeking', { configurable: true, value: true });
    let returned = false;
    const receipt = wrapper.vm.execute(command('seek', { kind: 'seek', time_ms: 2400 })).then(value => { returned = true; return value; });
    await nextTick(); expect(returned).toBe(false);
    Object.defineProperty(audio, 'seeking', { configurable: true, value: false });
    audio.dispatchEvent(new Event('seeked'));
    expect((await receipt).current_ms).toBe(2400); expect(returned).toBe(true);
    wrapper.unmount();
  });
  it('resumes an existing loop when play has no explicit range', async () => {
    vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
    vi.spyOn(HTMLMediaElement.prototype, 'paused', 'get').mockReturnValue(false);
    vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue();
    const pause = vi.spyOn(HTMLMediaElement.prototype, 'pause');
    const wrapper = mount(MusicReaderPane, { props: { context, owner: 'test' } });
    registered.value!.accepted({ ...await wrapper.vm.execute(command('observe')), receipt_id: 'server-receipt' });
    registered.value!.accepted({ ...await wrapper.vm.execute(command('loop', { kind: 'loop', start_ms: 1000, end_ms: 3000 })), receipt_id: 'server-receipt' });
    const resumed = await wrapper.vm.execute(command('resume', { kind: 'play', start_ms: null, end_ms: null }));
    expect(resumed.status).toBe('applied'); expect(resumed.loop).toEqual({ start_ms: 1000, end_ms: 3000 });
    pause.mockClear(); wrapper.get('audio').element.currentTime = 3.1; await wrapper.get('audio').trigger('timeupdate');
    expect(wrapper.get('audio').element.currentTime).toBe(1); expect(pause).not.toHaveBeenCalled();
    wrapper.unmount();
  });
  it('clears an existing loop for an explicit single-play range and pauses at its end', async () => {
    vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
    vi.spyOn(HTMLMediaElement.prototype, 'paused', 'get').mockReturnValue(false);
    vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue();
    const pause = vi.spyOn(HTMLMediaElement.prototype, 'pause');
    const wrapper = mount(MusicReaderPane, { props: { context, owner: 'test' } });
    registered.value!.accepted({ ...await wrapper.vm.execute(command('observe')), receipt_id: 'server-receipt' });
    registered.value!.accepted({ ...await wrapper.vm.execute(command('loop', { kind: 'loop', start_ms: 1000, end_ms: 3000 })), receipt_id: 'server-receipt' });
    const bounded = await wrapper.vm.execute(command('bounded', { kind: 'play', start_ms: 1500, end_ms: 2500 }));
    expect(bounded.status).toBe('applied'); expect(bounded.loop).toBeNull();
    pause.mockClear(); wrapper.get('audio').element.currentTime = 2.6; await wrapper.get('audio').trigger('timeupdate');
    expect(pause).toHaveBeenCalledOnce(); expect(wrapper.get('audio').element.currentTime).toBe(2.5);
    wrapper.unmount();
  });
  it('keeps the original and demonstration clocks separate through playback, pause and ending', async () => {
    mockMediaPlayback();
    const wrapper = mount(MusicReaderPane, { props: { context: listeningContext, owner: 'test' } });
    try {
      const original = wrapper.get<HTMLAudioElement>('.music-reader > audio').element;
      original.currentTime = 4.2005;
      accept(await wrapper.vm.execute(command('observe')));
      const started = await wrapper.vm.execute(command('demo', { kind: 'demonstration', demonstration_id: 'upper-line' }));
      expect(started).toMatchObject({ status: 'applied', paused: true, demonstration_id: 'upper-line', demonstration_state: { current_ms: 0, paused: false } });
      expect(started.current_ms).toBeCloseTo(4200.5);
      const demonstration = wrapper.get<HTMLAudioElement>('.music-demo-player audio').element;
      demonstration.currentTime = 1.2345;
      const observed = await wrapper.vm.execute(command('observe-playing-demo'));
      expect(observed.current_ms).toBeCloseTo(4200.5);
      expect(observed.demonstration_state?.current_ms).toBeCloseTo(1234.5);
      expect(observed.demonstration_state?.paused).toBe(false);
      accept(observed);
      const paused = await wrapper.vm.execute(command('pause-demo', { kind: 'pause' }));
      expect(paused).toMatchObject({ status: 'applied', paused: true, demonstration_id: 'upper-line', demonstration_state: { paused: true } });
      expect(paused.current_ms).toBeCloseTo(4200.5);
      expect(paused.demonstration_state?.current_ms).toBeCloseTo(1234.5);
      expect(demonstration.paused).toBe(true);
      await demonstration.play();
      demonstration.currentTime = 3.15049;
      demonstration.pause();
      await wrapper.get('.music-demo-player audio').trigger('ended');
      const ended = await wrapper.vm.execute(command('observe-ended-demo'));
      expect(ended).toMatchObject({ status: 'observed', paused: true, demonstration_id: 'upper-line', demonstration_state: { paused: true } });
      expect(ended.current_ms).toBeCloseTo(4200.5);
      expect(ended.demonstration_state?.current_ms).toBeCloseTo(3150.49);
    } finally { wrapper.unmount(); }
  });
  it('pauses both players and anchors a question to the original recording while a demonstration is playing', async () => {
    mockMediaPlayback();
    const wrapper = mount(MusicReaderPane, { props: { context: listeningContext, owner: 'test' } });
    try {
      const original = wrapper.get<HTMLAudioElement>('.music-reader > audio').element;
      original.currentTime = 4.2005;
      accept(await wrapper.vm.execute(command('observe')));
      await wrapper.vm.execute(command('demo', { kind: 'demonstration', demonstration_id: 'upper-line' }));
      const demonstration = wrapper.get<HTMLAudioElement>('.music-demo-player audio').element;
      demonstration.currentTime = 1.2345;
      expect(demonstration.paused).toBe(false);
      const ask = wrapper.get('.music-ask');
      expect(ask.text()).toBe('问原声这段');
      expect(ask.attributes('title')).toContain('原录音');
      expect(ask.attributes('title')).toContain('0:04');
      await ask.trigger('click');
      expect(original.paused).toBe(true);
      expect(demonstration.paused).toBe(true);
      const questions = wrapper.emitted('question');
      expect(questions).toHaveLength(1);
      const question = questions![0][0] as MusicQuestionContext;
      expect(question.audio.recording).toEqual(listeningContext.manifest.recording.source);
      expect(question.audio.start_ms).toBeLessThanOrEqual(4200.5);
      expect(question.audio.end_ms).toBeGreaterThan(4200.5);
      expect(question.audio.start_ms).toBeGreaterThan(1234.5);
      expect(question.audio.end_ms - question.audio.start_ms).toBe(1000);
    } finally { wrapper.unmount(); }
  });
  it.each([
    { kind: 'play' },
    { kind: 'seek', time_ms: 2400 },
    { kind: 'loop', start_ms: 1000, end_ms: 3000 },
  ] satisfies NonNullable<MusicCommand['request']['action']>[])('exits the demonstration when the original recording receives $kind', async action => {
    mockMediaPlayback();
    const wrapper = mount(MusicReaderPane, { props: { context: listeningContext, owner: 'test' } });
    try {
      accept(await wrapper.vm.execute(command('observe')));
      accept(await wrapper.vm.execute(command('demo', { kind: 'demonstration', demonstration_id: 'upper-line' })));
      const demonstration = wrapper.get<HTMLAudioElement>('.music-demo-player audio').element;
      expect(demonstration.paused).toBe(false);
      const receipt = await wrapper.vm.execute(command('original-control', action));
      await nextTick();
      expect(receipt.status).toBe('applied');
      expect(receipt.demonstration_id).toBeNull();
      expect(receipt).not.toHaveProperty('demonstration_state');
      expect(demonstration.paused).toBe(true);
      expect(wrapper.find('.music-demo-player').exists()).toBe(false);
      expect(receipt.paused).toBe(action.kind !== 'play');
    } finally { wrapper.unmount(); }
  });
  it.each([
    { action: { kind: 'play', cue_event_id: 'original-cue' }, event: listeningContext.manifest.events[0], source: '/recording.mp3' },
    { action: { kind: 'demonstration', demonstration_id: 'upper-line', cue_event_id: 'demo-cue' }, event: listeningContext.manifest.events[1], source: '/upper-line.wav' },
  ] satisfies { action: NonNullable<MusicCommand['request']['action']>; event: MusicContext['manifest']['events'][number]; source: string }[])('shows the curated cue before $action.kind reaches the audio player', async ({ action, event, source }) => {
    const seenAtPlay: { text: string | null; connected: boolean; source: string | null }[] = [];
    mockMediaPlayback(audio => {
      const cue = document.querySelector('.music-play-cue');
      seenAtPlay.push({ text: cue?.textContent ?? null, connected: cue?.isConnected ?? false, source: audio.getAttribute('src') });
    });
    const wrapper = mount(MusicReaderPane, { attachTo: document.body, props: { context: listeningContext, owner: 'test' } });
    try {
      accept(await wrapper.vm.execute(command('observe')));
      const receipt = await wrapper.vm.execute(command('cued-play', action));
      expect(receipt.status).toBe('applied');
      expect(seenAtPlay).toHaveLength(1);
      expect(seenAtPlay[0]).toMatchObject({ connected: true, source });
      expect(seenAtPlay[0].text).toContain(event.listen_for);
      expect(wrapper.get('.music-play-cue').text()).toContain(event.listen_for);
      expect(receipt.displayed_cue).toEqual({ event_id: event.event_id, text: event.listen_for });
      const observed = await wrapper.vm.execute(command('observe-cue'));
      expect(observed.displayed_cue).toEqual(receipt.displayed_cue);
      await wrapper.get('[aria-label="播放速度"]').setValue('0.75');
      expect(wrapper.find('.music-play-cue').exists()).toBe(false);
      expect(await wrapper.vm.execute(command('observe-after-listener-control'))).not.toHaveProperty('displayed_cue');
    } finally { wrapper.unmount(); }
  });
  it.each([
    { kind: 'play', cue_event_id: 'missing-cue' },
    { kind: 'demonstration', demonstration_id: 'upper-line', cue_event_id: 'missing-cue' },
  ] satisfies NonNullable<MusicCommand['request']['action']>[])('rejects an unknown listening cue before $kind starts audio', async action => {
    const media = mockMediaPlayback();
    const wrapper = mount(MusicReaderPane, { props: { context: listeningContext, owner: 'test' } });
    try {
      accept(await wrapper.vm.execute(command('observe')));
      const receipt = await wrapper.vm.execute(command('invalid-cue', action));
      expect(receipt.status).toBe('failed');
      expect(receipt.error).toBeTruthy();
      expect(media.play).not.toHaveBeenCalled();
      expect(wrapper.find('.music-play-cue').exists()).toBe(false);
      expect(receipt).not.toHaveProperty('displayed_cue');
    } finally { wrapper.unmount(); }
  });
});
