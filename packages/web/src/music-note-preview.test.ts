// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import type { MusicContext } from './music-api';
import MusicReaderPane from './components/MusicReaderPane.vue';

vi.mock('./music-interaction', () => ({ registerMusicPlayer: () => () => {} }));
const renderedNotes = vi.hoisted(() => vi.fn());
vi.mock('./music-score', () => ({ MusicScoreMarkers: class { readonly ready = true; attach() {} dispose() {} colors() {} focus() {} render(...args: unknown[]) { renderedNotes(...args); } element() {} } }));

const context: MusicContext = { publication: { book_id: 'orchestra', publication_id: 'local' }, audio_url: '/recording.flac', demonstrations: [],
  score_pages: [1, 2, 3].map(i => ({ page_id: `page-${i}`, url: `/page-${i}.svg` })),
  manifest: { version: 'music_source_manifest.v1', book_id: 'orchestra', title: '完整乐章', work: { work_id: 'w', title: '完整乐章', composer: '作曲家', movement: '第四乐章' },
    recording: { source: { recording_id: 'take', revision: '1' }, title: '演奏', performers: '乐团', media_path: 'recording.flac', content_type: 'audio/flac', duration_ms: 10000, kind: 'performance' },
    score: { source: { score_id: 'score', revision: '1' }, title: '总谱', musicxml_path: 'score.xml' },
    pages: [1, 2, 3].map(i => ({ page_id: `page-${i}`, path: `page-${i}.svg`, width: 1800, height: 2200 })),
    rights: [], parts: [], score_objects: [], occurrences: [], note_alignments: [], coverage: [], provenance: [], events: [], structure: [], demonstrations: [] } };

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); localStorage.clear(); });

  it('previews pending notes only by explicit choice, preserves their status and restores the preference', async () => {
    vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
    vi.spyOn(HTMLMediaElement.prototype, 'duration', 'get').mockReturnValue(10);
    vi.spyOn(HTMLElement.prototype, 'scrollTo').mockImplementation(() => {});
    vi.stubGlobal('IntersectionObserver', class { observe() {} unobserve() {} disconnect() {} });
    vi.stubGlobal('fetch', vi.fn(async () => new Response('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1800 2200"><g id="note"/></svg>')));
    const material: MusicContext = { ...context, demonstrations: [{demonstration_id:'demo',url:'/demo.wav'}], manifest: { ...context.manifest, demonstrations: [{demonstration_id:'demo',title:'示范',kind:'score_synthesis',media_path:'demo.wav',content_type:'audio/wav',duration_ms:1000,part_ids:['piano'],score_object_ids:['note'],provenance_id:'synthesis'}],
      parts: [{part_id:'piano',name:'钢琴',instrument:'钢琴',group:'键盘',voices:['1']}],
      score_objects: ['note','hidden','failed'].map((id,i) => ({source_id:id,part_id:'piano',voice:'1',measure:i===2?'2':'1',beat:0,duration_beats:1,midi_pitch:60,rest:false,tie:'none',page_id:'page-1',svg_element_id:id})),
      occurrences: [{occurrence_id:'one',order:0,measure:'1',start_ms:0,end_ms:4000,score_object_ids:['note','hidden']},{occurrence_id:'two',order:1,measure:'2',start_ms:4000,end_ms:7000,score_object_ids:['failed']}],
      note_alignments: [{occurrence_id:'one',score_object_id:'note',start_ms:1000,end_ms:2000,attack:true,provenance_id:'candidate'},{occurrence_id:'one',score_object_id:'hidden',start_ms:1000,end_ms:2000,attack:false,unobserved:true,provenance_id:'candidate'},{occurrence_id:'two',score_object_id:'failed',start_ms:5000,end_ms:6000,attack:true,provenance_id:'candidate'}],
      coverage: [{start_ms:0,end_ms:7000,granularity:'measure',method:'audio_alignment',status:'accepted',provenance_id:'candidate',detail:'小节定位'},{start_ms:0,end_ms:4000,granularity:'note',method:'audio_alignment',status:'pending',provenance_id:'candidate',detail:'投影候选'},{start_ms:4000,end_ms:7000,granularity:'note',method:'audio_alignment',status:'failed',provenance_id:'candidate',detail:'失败区间'}],
    }};
    const coverage = JSON.stringify(material.manifest.coverage);
    let w=mount(MusicReaderPane,{props:{context:material,owner:'preview-test'}});
    try {
      await flushPromises();await w.get('audio').trigger('loadedmetadata');w.vm.seek(1500);await flushPromises();
      const toggle=w.get('[aria-label="估计逐音高亮"]');expect(toggle.attributes('aria-pressed')).toBe('false');expect(renderedNotes.mock.calls.at(-1)?.[0]).toEqual([]);
      await toggle.trigger('click');await flushPromises();expect(toggle.attributes('aria-pressed')).toBe('true');expect(toggle.text()).toContain('估计');
      expect(renderedNotes.mock.calls.at(-1)?.[0]).toEqual([expect.objectContaining({score_object_id:'note'})]);
      expect(JSON.stringify(material.manifest.coverage)).toBe(coverage);
      w.vm.seek(5100);await flushPromises();expect(renderedNotes.mock.calls.at(-1)?.[0]).toEqual([]);
      w.vm.seek(1500);await toggle.trigger('click');await flushPromises();expect(renderedNotes.mock.calls.at(-1)?.[0]).toEqual([]);
      await toggle.trigger('click');
      await w.get('.music-details-toggle').trigger('click');await w.get('.music-demonstrations button').trigger('click');await flushPromises();
      expect(w.get('[aria-label="估计逐音高亮"]').attributes('disabled')).toBeDefined();expect(renderedNotes.mock.calls.at(-1)?.[0]).toEqual([]);
      await w.get('[aria-label="返回原录音"]').trigger('click');await flushPromises();
      expect(w.get('[aria-label="估计逐音高亮"]').attributes('disabled')).toBeUndefined();expect(renderedNotes.mock.calls.at(-1)?.[0]).toEqual([expect.objectContaining({score_object_id:'note'})]);
      w.unmount();
      w=mount(MusicReaderPane,{props:{context:material,owner:'preview-test'}});await flushPromises();await w.get('audio').trigger('loadedmetadata');await flushPromises();
      expect(w.get('[aria-label="估计逐音高亮"]').attributes('aria-pressed')).toBe('true');
      await w.setProps({context:{...material,manifest:{...material.manifest,recording:{...material.manifest.recording,source:{recording_id:'other',revision:'1'}}}}});await flushPromises();
      expect(w.get('[aria-label="估计逐音高亮"]').attributes('aria-pressed')).toBe('false');
    } finally {w.unmount();}
  });
