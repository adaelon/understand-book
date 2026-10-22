// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import MusicCalibrationPane from './components/MusicCalibrationPane.vue';
import type { MusicCalibrationContext } from './music-calibration';
const context = { audio_url: '/audio.mp3', score_pages: [{ page_id: 'p1', path: '1.svg', width: 100, height: 100, url: '/1.svg' }], history: [], manifest: {
  work: { title: '真实材料测试' }, recording: { source: { recording_id: 'take', revision: 'r1' }, duration_ms: 5000, title: '原录音' }, score: { source: { score_id: 'score', revision: 's1' } },
  parts: [{ part_id: 'piano', name: '钢琴' }], score_objects: [{ source_id: 'n1', svg_element_id: 'note', page_id: 'p1', part_id: 'piano', voice: '1:1', beat: 1, midi_pitch: 60, rest: false, tie: 'none' }, { source_id: 'n2', svg_element_id: 'note2', page_id: 'p1', part_id: 'piano', voice: '1:1', beat: 1, midi_pitch: 64, rest: false, tie: 'none' }],
  occurrences: [{ occurrence_id: 'first', order: 0, measure: '1', start_ms: 0, end_ms: 2000, score_object_ids: ['n1', 'n2'] }, { occurrence_id: 'next', order: 1, measure: '2', start_ms: 2000, end_ms: 4000, score_object_ids: [] }], note_alignments: [],
} } as unknown as MusicCalibrationContext;
function setup() {
  vi.stubGlobal('fetch', vi.fn(async () => ({ ok: true, text: async () => '<svg><g id="note"><path /></g></svg>' })));
  return mount(MusicCalibrationPane, { props: { context } });
}
afterEach(() => vi.unstubAllGlobals());
describe('local calibration editor', () => {
  it('stops its media-clock animation frame when paused and unmounted', async () => {
    const request = vi.fn(() => 42), cancel = vi.fn();
    vi.stubGlobal('requestAnimationFrame', request); vi.stubGlobal('cancelAnimationFrame', cancel);
    const wrapper = setup(); await flushPromises();
    await wrapper.get('audio').trigger('play'); expect(request).toHaveBeenCalledOnce();
    await wrapper.get('audio').trigger('pause'); expect(cancel).toHaveBeenCalledWith(42);
    await wrapper.get('audio').trigger('play'); cancel.mockClear(); wrapper.unmount();
    expect(cancel).toHaveBeenCalledWith(42);
  });
  it('moves a selected shared boundary, keeps unsaved work on failure, and resets only after successful context replacement', async () => {
    const wrapper = setup(); await flushPromises();
    await wrapper.get('[aria-label="窗口结束小节"]').setValue('1');
    await wrapper.get('[aria-label="小节终点秒"]').setValue('2.1');
    await wrapper.get('[aria-label="当前小节出现"]').setValue('next');
    expect((wrapper.get('[aria-label="小节起点秒"]').element as HTMLInputElement).value).toBe('2.1');
    await wrapper.get('[aria-label="修改原因"]').setValue('候选修正');
    await wrapper.get('[aria-label="时间判断的依据"]').setValue('尚未复听，保持待核');
    await wrapper.findAll('button').find(button => button.text() === '保存本地修订')!.trigger('click');
    const revision = wrapper.emitted('save')![0][0] as any;
    expect(revision.coverage[0].status).toBe('pending'); expect(revision.provenance[0].kind).toBe('interpretation');
    await wrapper.setProps({ saveError: '磁盘写入失败' });
    expect(wrapper.get('[role="alert"]').text()).toBe('磁盘写入失败');
    expect((wrapper.get('[aria-label="修改原因"]').element as HTMLInputElement).value).toBe('候选修正');
    expect(wrapper.text()).not.toContain('已保存到本地工作区');
    await wrapper.setProps({ saveError: '', context: { ...context, manifest: { ...context.manifest, occurrences: revision.occurrences } } });
    expect(wrapper.get('[role="status"]').text()).toContain('已保存到本地工作区');
    expect((wrapper.get('[aria-label="修改原因"]').element as HTMLInputElement).value).toBe('');
    wrapper.unmount();
  });
  it('selects the real score object and refuses note acceptance with a missing onset', async () => {
    const wrapper = setup(); await flushPromises();
    await wrapper.get('#note path').trigger('click');
    expect((wrapper.get('[aria-label="选择校准音符"]').element as HTMLSelectElement).value).toBe('n1');
    const choices = wrapper.get('[aria-label="选择校准音符"]').text();
    expect(choices).toContain('C4（实音）'); expect(choices).toContain('E4（实音）');
    expect((wrapper.get('[aria-label="音符起点秒"]').element as HTMLInputElement).value).toBe('');
    await wrapper.get('[aria-label="修改原因"]').setValue('验收'); await wrapper.get('[aria-label="时间判断的依据"]').setValue('复听');
    await wrapper.get('[aria-label="保存精度"]').setValue('note');
    await wrapper.findAll('input[type="checkbox"]').at(-1)!.setValue(true);
    await wrapper.findAll('button').find(button => button.text() === '保存本地修订')!.trigger('click');
    expect(wrapper.get('[role="alert"]').text()).toContain('还有 2 个音符'); expect(wrapper.emitted('save')).toBeUndefined();
    wrapper.unmount();
  });
});
