// @vitest-environment happy-dom
import { expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import MusicReaderPane from './components/MusicReaderPane.vue';
import type { MusicContext } from './music-api';

it('treats a listener pause during pending play as cancellation while preserving real playback errors', async () => {
  let paused = true, rejectPlay!: (reason: unknown) => void;
  vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
  vi.spyOn(HTMLMediaElement.prototype, 'paused', 'get').mockImplementation(() => paused);
  const play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(function(this: HTMLMediaElement) {
    paused = false; this.dispatchEvent(new Event('play'));
    return new Promise<void>((_, reject) => { rejectPlay = reject; });
  });
  vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(function(this: HTMLMediaElement) {
    paused = true; this.dispatchEvent(new Event('pause'));
  });
  const context = {publication:{book_id:'music',publication_id:'pub'},audio_url:'/recording.mp3',score_pages:[],demonstrations:[],manifest:{version:'music_source_manifest.v1',book_id:'music',title:'作品',work:{work_id:'w',title:'作品',composer:'作曲家',movement:null},recording:{source:{recording_id:'take',revision:'r1'},media_path:'audio.mp3',content_type:'audio/mpeg',duration_ms:10000,title:'录音',performers:'演奏家',kind:'performance'},score:{source:{score_id:'score',revision:'s1'},title:'谱本',musicxml_path:'score.xml'},rights:[],parts:[],pages:[],score_objects:[],occurrences:[],note_alignments:[],coverage:[],provenance:[],events:[],structure:[],demonstrations:[]}} satisfies MusicContext;
  const wrapper = mount(MusicReaderPane, {props:{context,owner:'cancel-test'}});
  try {
    await wrapper.get('[aria-label="播放音乐"]').trigger('click');
    await wrapper.get('[aria-label="暂停音乐"]').trigger('click');
    rejectPlay(new DOMException('play interrupted by pause', 'AbortError'));
    await vi.waitFor(() => expect(wrapper.find('.music-error').exists()).toBe(false));
    await nextTick();
    expect(wrapper.find('.music-error').exists()).toBe(false);
    play.mockRejectedValueOnce(new DOMException('permission denied', 'NotAllowedError'));
    await wrapper.get('[aria-label="播放音乐"]').trigger('click');
    await vi.waitFor(() => expect(wrapper.get('.music-error').text()).toContain('permission denied'));
  } finally {wrapper.unmount();vi.restoreAllMocks();localStorage.clear();}
});
