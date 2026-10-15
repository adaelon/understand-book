// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import VideoReaderPane from './VideoReaderPane.vue';
import type { PreparingVideo } from '../video-api';
import { createVideoQuestion } from '../video-question';
import type { VideoQuestionContext } from '../../../core/src/generated/VideoQuestionContext';
const initial: PreparingVideo = { video: { media_id: 'lecture', media_revision: '1' }, title: 'Lecture',
  stage: 'awaiting_transcript', copied_bytes: 10, total_bytes: 10, duration_ms: 10000, playable: true, qa_ready: false, failure: null };
afterEach(() => { vi.unstubAllGlobals(); localStorage.clear(); });
it('shows current bilingual text, missing and failed segments, then retry and accepted text without replacing video', async () => {
  const v = initial.video, t = { transcript_id: 't', transcript_revision: '1', video: v };
  const segments = [0, 1, 2].map(i => ({ segment_id: `s${i}`, text: `English ${i}`, location: { video: v, start_ms: i * 1000, end_ms: (i + 1) * 1000 }, text_range: { lid: `${i}`, range: { start: 0, end: 9 } }, words: [] }));
  const published = { time_base: [1, 1000] as [number, number], publication: { book_id: 'b', publication_id: 'p' }, material: { book_id: 'b', video: v, transcript: t },
    source: { video: v, duration_ms: 3000, width: 10, height: 10, imported_path: 'media/original.mp4', original_path: 'original.mp4' },
    map: { transcript: t, segments, machine_transcription: { engine: 'fixture', model: 'fixture', language: 'en' }, raw_result_path: 'raw.json' },
    translations: [{ transcript: t, segment_id: 's0', zh_text: '第一段中文' }],
    translation_progress: { tasks: [{ id: 'task', segment_ids: ['s1'], status: 'failed' as const, attempts: 1, failure: 'interrupted' }] } };
  const wrapper = mount(VideoReaderPane, { props: { initial, owner: 'local', published } });
  const video = wrapper.get('video');
  expect(wrapper.get('[aria-label="当前片段中英对照"]').text()).toContain('第一段中文');
  expect(wrapper.get('[aria-label="中文翻译进度"]').text()).toContain('中文 1 / 3');
  expect(wrapper.get('[data-segment="s1"]').text()).toContain('翻译失败');
  expect(wrapper.get('[data-segment="s2"]').text()).toContain('待翻译');
  await wrapper.setProps({ published: { ...published, translation_progress: { tasks: [{ ...published.translation_progress.tasks[0], status: 'running', attempts: 2 }] } } });
  video.element.currentTime = 1.5; await video.trigger('timeupdate');
  await flushPromises();
  expect(wrapper.get('[aria-label="当前片段中英对照"]').text()).toContain('正在重试翻译');
  const pane = wrapper.get('.video-reader .transcript').element as HTMLElement;
  const anchor = wrapper.get('[data-segment="s0"]').element;
  let reads = 0;
  vi.spyOn(pane, 'getBoundingClientRect').mockReturnValueOnce({ top: 100 } as DOMRect).mockReturnValue({ top: 175 } as DOMRect);
  vi.spyOn(anchor, 'getBoundingClientRect').mockImplementation(() => (++reads <= 2 ? { top: 110, bottom: 130 } : { top: 225, bottom: 245 }) as DOMRect);
  await wrapper.setProps({ published: { ...published, translations: segments.map(s => ({ transcript: t, segment_id: s.segment_id, zh_text: '全部中文' })) } });
  await flushPromises();
  expect(wrapper.get('video').element).toBe(video.element);
  expect(pane.scrollTop).toBe(40); // Added text, excluding the container's own 75px layout shift.
  expect(video.element.currentTime).toBe(1.5);
  expect(wrapper.get('[aria-label="中文翻译进度"]').text()).toContain('待补齐 0 段');
  expect(wrapper.get('[aria-label="当前片段中英对照"]').text()).toContain('全部中文');
  expect(wrapper.text()).not.toContain('翻译失败');
  wrapper.unmount();
});
it('selects and edits a long time range while keeping the original frame and independent region', async () => {
  const videoRef = { media_id: 'v', media_revision: '1' };
  const published = { time_base: [1, 1000] as [number, number], publication: { book_id: 'b', publication_id: 'original' },
    material: { book_id: 'b', video: videoRef, transcript: { transcript_id: 't', transcript_revision: '1', video: videoRef } },
    source: { video: videoRef, width: 1920, height: 1080, duration_ms: 700000, imported_path: 'media/original.mp4', original_path: 'fixture.mp4' }, map: { segments: [] } as never };
  const wrapper = mount(VideoReaderPane, { props: { initial, owner: 'local', published } });
  const video = wrapper.get('video'); let callback!: VideoFrameRequestCallback;
  Object.defineProperties(video.element, { readyState: { value: 2 }, videoWidth: { value: 1920 }, videoHeight: { value: 1080 },
    requestVideoFrameCallback: { value: (cb: VideoFrameRequestCallback) => { callback = cb; return 1; } }, cancelVideoFrameCallback: { value: vi.fn() } });
  await video.trigger('loadedmetadata'); callback(0, { mediaTime: 492 } as VideoFrameCallbackMetadata); await flushPromises();
  const click = async (text: string) => wrapper.findAll('button').find(b => b.text() === text)!.trigger('click');
  await click('选择时间范围'); await click('放弃修改');
  expect(wrapper.emitted('select-range')).toBeUndefined();
  await click('选择时间范围');
  const inputs = () => wrapper.findAll('input[type=number]');
  for (const [start, end] of [['', '650'], ['650', '555'], ['-1', '650'], ['555', '701']]) {
    await inputs()[0].setValue(start); await inputs()[1].setValue(end); await wrapper.get('form').trigger('submit');
    expect(wrapper.emitted('select-range')).toBeUndefined(); expect(wrapper.get('[role=alert]').text()).toContain('起止秒数');
  }
  video.element.currentTime = 555.123; await click('当前位置设为起点');
  video.element.currentTime = 651; await click('当前位置设为终点');
  callback(0, { mediaTime: 651 } as VideoFrameCallbackMetadata);
  await wrapper.get('form').trigger('submit');
  const selected = wrapper.emitted('select-range')![0][0] as VideoQuestionContext;
  expect(selected.frame.actual_pts_ms).toBe(492000);
  expect(selected.range).toEqual({ video: videoRef, start_ms: 555123, end_ms: 651000 });
  const region = { frame: selected.frame, rect_normalized: [.1, .2, .3, .4] as [number, number, number, number] };
  await wrapper.setProps({ question: { ...selected, region } });
  await video.trigger('play'); callback(0, { mediaTime: 670 } as VideoFrameCallbackMetadata); await flushPromises();
  await click('编辑时间范围'); await inputs()[0].setValue('550'); await wrapper.get('form').trigger('submit');
  const edited = wrapper.emitted('select-range')![1][0] as VideoQuestionContext;
  expect(edited.frame).toEqual(selected.frame); expect(edited.region).toEqual(region);
  expect(edited.range?.start_ms).toBe(550000);
  await click('取消时间范围'); expect(wrapper.emitted('clear-range')).toHaveLength(1);
  await click('编辑时间范围'); window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' })); await flushPromises();
  expect(wrapper.find('form').exists()).toBe(false);
  await wrapper.setProps({ selectionDisabled: true });
  expect(wrapper.findAll('button').find(b => b.text() === '编辑时间范围')!.attributes('disabled')).toBeDefined();
  wrapper.unmount();
});
it('binds a pointer selection to the displayed frame, hides it on play and cancels unfinished drags', async () => {
  const v = { media_id: 'v', media_revision: '1' };
  const context: VideoQuestionContext = { draft_id: 'selection', publication: { book_id: 'b', publication_id: 'original' },
    material: { book_id: 'b', video: v, transcript: { transcript_id: 't', transcript_revision: '1', video: v } },
    frame: { video: v, actual_pts_ms: 492000, width: 1920, height: 1080 }, region: null, range: null };
  const wrapper = mount(VideoReaderPane, { props: { initial, owner: 'local', embedded: true,
    published: { time_base: [1, 1000], publication: context.publication, material: context.material,
      source: { video: context.material.video, width: 1920, height: 1080, duration_ms: 510000,
        imported_path: 'media/original.mp4', original_path: 'fixture.mp4' }, map: { segments: [] } as never } } });
  const video = wrapper.get('video'); let callback!: VideoFrameRequestCallback;
  Object.defineProperties(video.element, { readyState: { value: 2 }, videoWidth: { value: 1920 }, videoHeight: { value: 1080 },
    requestVideoFrameCallback: { value: (cb: VideoFrameRequestCallback) => { callback = cb; return 1; } }, cancelVideoFrameCallback: { value: vi.fn() } });
  vi.spyOn(video.element, 'getBoundingClientRect').mockReturnValue({ left: 10, top: 20, width: 960, height: 800 } as DOMRect);
  await video.trigger('loadedmetadata'); callback(0, { mediaTime: 492 } as VideoFrameCallbackMetadata);
  await flushPromises();
  const button = () => wrapper.findAll('button').find(b => b.text() === '框选画面' || b.text() === '重新框选')!;
  await button().trigger('click');
  const layer = wrapper.get('[aria-label="画面框选区域"]');
  vi.spyOn(layer.element, 'getBoundingClientRect').mockReturnValue({ left: 10, top: 150, width: 960, height: 540 } as DOMRect);
  Object.defineProperty(layer.element, 'setPointerCapture', { value: vi.fn() });
  await layer.trigger('pointerdown', { pointerId: 1, button: 0, clientX: 130, clientY: 285 });
  await layer.trigger('pointerup', { pointerId: 1, button: 0, clientX: 610, clientY: 555 });
  const selected = wrapper.emitted('select-region')![0][0] as typeof context;
  expect(selected.frame.actual_pts_ms).toBe(492000);
  expect(selected.region?.rect_normalized).toEqual([.125, .25, .5, .5]);
  await wrapper.setProps({ question: selected }); expect(wrapper.find('.video-region-box').exists()).toBe(true);
  await video.trigger('play'); callback(0, { mediaTime: 507 } as VideoFrameCallbackMetadata);
  await flushPromises(); expect(wrapper.find('.video-region-box').exists()).toBe(false);
  expect(button().attributes('disabled')).toBeDefined();
  expect(wrapper.text()).toContain('回到问题画面');
  await video.trigger('pause'); callback(0, { mediaTime: 492 } as VideoFrameCallbackMetadata); await flushPromises();
  await button().trigger('click');
  await wrapper.get('[aria-label="画面框选区域"]').trigger('pointerdown', { pointerId: 2, button: 0, clientX: 130, clientY: 285 });
  await wrapper.get('[aria-label="画面框选区域"]').trigger('pointercancel');
  expect(wrapper.emitted('select-region')).toHaveLength(1);
  await button().trigger('click'); window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' })); await flushPromises();
  expect(wrapper.find('.selecting').exists()).toBe(false);
  await wrapper.findAll('button').find(b => b.text() === '取消选区')!.trigger('click');
  expect(wrapper.emitted('clear-region')).toHaveLength(1);
  wrapper.unmount();
});
it('captures the presented PTS and pauses once while later playback and edits leave it frozen', async () => {
  const v = { media_id: 'v', media_revision: '1' };
  const context = { publication: { book_id: 'b', publication_id: 'original' }, material: { book_id: 'b', video: v, transcript: { transcript_id: 't', transcript_revision: '1', video: v } }, frame: { video: v, actual_pts_ms: 491966.6875, width: 1920, height: 1080 } };
  const wrapper = mount(VideoReaderPane, { props: { initial, owner: 'local', embedded: true,
    published: { time_base: [1, 16000], publication: context.publication, material: context.material, source: {
      video: context.material.video, width: 1920, height: 1080, duration_ms: 510000, imported_path: 'media/original.mp4', original_path: 'fixture.mp4' },
      map: { segments: [] } as never } } });
  const video = wrapper.get('video');
  let callback!: VideoFrameRequestCallback;
  Object.defineProperties(video.element, { duration: { value: 510 }, readyState: { value: 2 },
    videoWidth: { value: 1920 }, videoHeight: { value: 1080 },
    requestVideoFrameCallback: { value: vi.fn(cb => { callback = cb; return 1; }) },
    cancelVideoFrameCallback: { value: vi.fn() } });
  const pause = vi.spyOn(video.element, 'pause');
  await video.trigger('loadedmetadata'); callback(0, { mediaTime: 491.966688 } as VideoFrameCallbackMetadata);
  const scene = createVideoQuestion('window', async () => ({ draft: null }), id => wrapper.vm.captureQuestion(id));
  expect(pause).not.toHaveBeenCalled(); scene.input('n'); scene.input('你');
  video.element.currentTime = 507; callback(1, { mediaTime: 507 } as VideoFrameCallbackMetadata);
  scene.input('你能解释代码吗'); await scene.submission();
  expect(pause).toHaveBeenCalledTimes(1);
  expect(scene.draft.value?.context.frame).toEqual(context.frame);
  wrapper.unmount();
});
it('loads the first transcript without replacing the player and seeks its sentence time',async()=> {
  const ready={...initial,english_ready:true,publication:{book_id:'book',publication_id:'pub'}};
  vi.stubGlobal('fetch',vi.fn(async(url:string)=>({ok:true,json:async()=>url.endsWith('/transcript')?
    {map:{segments:[{segment_id:'s1',text:'Machine transcript.',location:{start_ms:5500,end_ms:6500}}]}}:ready})));
  const wrapper=mount(VideoReaderPane,{props:{initial,owner:'local'}});
  const original=wrapper.get('video').element;
  await flushPromises();
  expect(wrapper.get('video').element).toBe(original);
  expect(wrapper.text()).toContain('Machine transcript.');
  await wrapper.get('[aria-label="回看 0:05"]').trigger('click');
  expect(original.currentTime).toBe(5.5);
  expect(wrapper.find('textarea').exists()).toBe(false);
  expect(wrapper.text()).toContain('打开视频阅读与提问');
  wrapper.unmount();
});
it('offers native playback, restores position by media identity and keeps questions disabled', async () => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => initial }));
  localStorage.setItem('video-position:alice:lecture:1', JSON.stringify({ time: 4, rate: 1.5 }));
  const wrapper = mount(VideoReaderPane, { props: { initial, owner: 'alice' } });
  const video = wrapper.get('video');
  Object.defineProperty(video.element, 'duration', { value: 10 });
  Object.defineProperty(video.element, 'readyState', { value: 1 });
  await video.trigger('loadedmetadata'); await flushPromises();
  expect(video.attributes('src')).toBe('/api/videos/lecture/1/media');
  expect(video.element.currentTime).toBe(4); expect(video.element.playbackRate).toBe(1.5);
  expect(wrapper.get('textarea').attributes('disabled')).toBeDefined();
  expect(wrapper.text()).toContain('全片英文转写与首个发布尚未就绪');
  video.element.currentTime = 7;
  window.dispatchEvent(new Event('pagehide'));
  expect(JSON.parse(localStorage.getItem('video-position:alice:lecture:1')!).time).toBe(7);
  wrapper.unmount();
});
it('shows actual copy progress and durable preparation failure without presenting a player', async () => {
  const failed = { ...initial, stage: 'failed' as const, playable: false, failure: 'ffprobe missing' };
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => failed }));
  const wrapper = mount(VideoReaderPane, { props: { initial: { ...initial, stage: 'copying', copied_bytes: 5, playable: false }, owner: 'local' } });
  expect(wrapper.get('progress').attributes('value')).toBe('5'); expect(wrapper.text()).toContain('50%');
  await flushPromises();
  expect(wrapper.get('[role=alert]').text()).toBe('ffprobe missing'); expect(wrapper.find('video').exists()).toBe(false);
  wrapper.unmount();
});
it('does not advertise playback when a completed import has lost its snapshot', async () => {
  const missing = { ...initial, playable: false, failure: '导入副本缺失或不完整' };
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => missing }));
  const wrapper = mount(VideoReaderPane, { props: { initial: missing, owner: 'local' } });
  await flushPromises();
  expect(wrapper.text()).not.toContain('原片可观看');
  expect(wrapper.get('[role=alert]').text()).toContain('副本缺失');
  expect(wrapper.find('video').exists()).toBe(false);
  wrapper.unmount();
});
