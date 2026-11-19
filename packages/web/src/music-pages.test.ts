// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import type { MusicContext } from './music-api';
import { scorePageTurns, upcomingScorePages } from './music-score-pages';
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

describe('large orchestral score pages', () => {
  it('lists resting instruments, disables parts absent from the page, and hides navigation for image-only scores', async () => {
    vi.spyOn(HTMLElement.prototype, 'scrollTo').mockImplementation(() => {});
    vi.stubGlobal('fetch', vi.fn(async () => new Response('<svg xmlns="http://www.w3.org/2000/svg"/>')));
    const material: MusicContext = { ...context, manifest: { ...context.manifest,
      parts: ['flute', 'bass'].map(part_id => ({ part_id, name: part_id, instrument: part_id, group: 'orchestra', voices: ['1'] })),
      score_objects: [{ source_id: 'rest', part_id: 'flute', measure: '1', beat: 0, duration_beats: 4, voice: '1', midi_pitch: null, rest: true, tie: 'none', page_id: 'page-1', svg_element_id: 'rest' }],
    } };
    const wrapper = mount(MusicReaderPane, { props: { context: material, owner: 'test' } });
    try {
      await flushPromises();
      expect(wrapper.get('[data-part="flute"]').attributes('disabled')).toBeUndefined();
      expect(wrapper.get('[data-part="bass"]').attributes('disabled')).toBeDefined();
      await wrapper.get('[data-part="flute"]').trigger('click');
      expect(wrapper.get('[data-part="flute"]').attributes('aria-pressed')).toBe('true');
      await wrapper.setProps({ context: { ...material, manifest: { ...material.manifest, score_objects: [] } } });
      expect(wrapper.find('.music-instrument-nav').exists()).toBe(false);
    } finally { wrapper.unmount(); }
  });
  it('seeks an indexed head to its nearest performance occurrence while leaving score selection alone', async () => {
    vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
    vi.spyOn(HTMLElement.prototype, 'scrollTo').mockImplementation(() => {});
    vi.stubGlobal('IntersectionObserver', class { observe() {} unobserve() {} disconnect() {} });
    vi.stubGlobal('fetch', vi.fn(async () => new Response('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1800 2200"><g id="rendered-head"><path d="M10 10h10v10z"/></g></svg>')));
    const material: MusicContext = { ...context, manifest: { ...context.manifest,
      parts: [{ part_id: 'piano', name: '钢琴', instrument: '钢琴', group: '键盘', voices: ['1'] }],
      score_objects: [{ source_id: 'native-head', part_id: 'piano', measure: '1', beat: 0, duration_beats: 1, voice: '1', midi_pitch: 60, rest: false, tie: 'none', page_id: 'page-1', svg_element_id: 'rendered-head' }],
      occurrences: [1000, 5000].map((start, i) => ({ occurrence_id: `take-${i}`, order: i, measure: '1', start_ms: start, end_ms: start + 1000, score_object_ids: ['native-head'] })),
      note_alignments: [1000, 5000].map((start, i) => ({ occurrence_id: `take-${i}`, score_object_id: 'native-head', start_ms: start, end_ms: start + 500, attack: true, provenance_id: 'audio' })),
      coverage: [{ start_ms: 0, end_ms: 10000, granularity: 'note', method: 'audio_alignment', status: 'accepted', provenance_id: 'audio', detail: '自动跟谱；部分起音为插值。' }] } };
    const wrapper = mount(MusicReaderPane, { props: { context: material, owner: 'test' } });
    await flushPromises();
    wrapper.vm.seek(4300); await wrapper.get('#rendered-head path').trigger('click');
    expect(wrapper.get('audio').element.currentTime).toBe(5);
    expect(wrapper.get('.music-material-tag').text()).toContain('自动跟谱');
    wrapper.vm.seek(4300); await wrapper.get('.music-screenshot').trigger('click');
    await wrapper.get('#rendered-head path').trigger('click');
    expect(wrapper.get('audio').element.currentTime).toBe(4.3);
    await wrapper.setProps({ context: { ...material, manifest: { ...material.manifest, coverage: [
      { start_ms: 0, end_ms: 10000, granularity: 'measure', method: 'audio_alignment', status: 'accepted', provenance_id: 'audio', detail: '小节定位' },
      { start_ms: 0, end_ms: 3000, granularity: 'note', method: 'audio_alignment', status: 'accepted', provenance_id: 'audio', detail: '有起音依据' },
      { start_ms: 3000, end_ms: 10000, granularity: 'note', method: 'audio_alignment', status: 'pending', provenance_id: 'audio', detail: '含未观测起音' },
    ] } } });
    await flushPromises();
    expect(wrapper.get('.music-material-tag').text()).toContain('部分跟谱');
    expect(wrapper.get('.music-material-tag').attributes('title')).toContain('未观测音不显示光点');
    await wrapper.setProps({ context: { ...material, manifest: { ...material.manifest, coverage: [
      { start_ms: 0, end_ms: 10000, granularity: 'measure', method: 'audio_alignment', status: 'accepted', provenance_id: 'audio', detail: '小节定位' },
      { start_ms: 0, end_ms: 1200, granularity: 'note', method: 'audio_alignment', status: 'accepted', provenance_id: 'audio', detail: '有起音依据' },
      { start_ms: 1200, end_ms: 10000, granularity: 'note', method: 'audio_alignment', status: 'pending', provenance_id: 'audio', detail: '含未观测起音' },
    ] } } });
    await flushPromises(); renderedNotes.mockClear(); wrapper.vm.seek(1300); await flushPromises();
    expect(renderedNotes.mock.calls.at(-1)?.[0]).toEqual(expect.arrayContaining([expect.objectContaining({ score_object_id: 'native-head', start_ms: 1000 })]));
    renderedNotes.mockClear(); wrapper.vm.seek(5100); await flushPromises();
    expect(renderedNotes.mock.calls.at(-1)?.[0]).toEqual([]);
    wrapper.unmount();
  });
  it('keeps the requested opening page when its first download finishes while the mobile reader is hidden', async () => {
    let notify!: IntersectionObserverCallback;
    vi.stubGlobal('IntersectionObserver', class { constructor(callback: IntersectionObserverCallback) { notify = callback; } observe() {} unobserve() {} disconnect() {} });
    let finish!: (response: Response) => void;
    vi.stubGlobal('fetch', vi.fn((url: string) => url === '/page-1.svg' ? new Promise<Response>(resolve => { finish = resolve; }) : Promise.resolve(new Response('<svg/>'))));
    const wrapper = mount(MusicReaderPane, { props: { context, owner: 'test' } });
    await nextTick(); await nextTick();
    notify([{ target: wrapper.findAll('.music-score-page')[0].element, isIntersecting: false }] as IntersectionObserverEntry[], {} as IntersectionObserver);
    finish(new Response('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1800 2200"/>'));
    await flushPromises();
    expect(wrapper.findAll('.music-score-page')[0].find('svg').exists()).toBe(true);
    wrapper.unmount();
  });
  it('starts from the remembered page and drops a late response from the previous material', async () => {
    localStorage.setItem('music-position:test:take:1', JSON.stringify({ ms: 4000, rate: 1 }));
    vi.stubGlobal('IntersectionObserver', class { observe() {} unobserve() {} disconnect() {} });
    let finish!: (response: Response) => void;
    const fetch = vi.fn((url: string) => url === '/page-3.svg' ? new Promise<Response>(resolve => { finish = resolve; }) : Promise.resolve(new Response('<svg id="new-score"/>')));
    vi.stubGlobal('fetch', fetch);
    const pdf = { ...context, manifest: { ...context.manifest, events: [{ event_id: 'third', title: '', explanation: '', listen_for: '', roles: [], relations: [], concept_tags: [],
      start_ms: 3000, end_ms: 8000, evidence: [{ provenance_id: 'pdf', score_object_ids: [], score_page_ids: ['page-3'], audio: null }] }] } };
    const wrapper = mount(MusicReaderPane, { props: { context: pdf, owner: 'test' } });
    try {
      await flushPromises();
      expect(fetch.mock.calls.map(([url]) => url)).toEqual(['/page-3.svg']);
      await wrapper.setProps({ context: { ...pdf, score_pages: pdf.score_pages.map(page => ({ ...page, url: '/new' + page.url })) } });
      await flushPromises();
      finish(new Response('<svg id="old-score"/>')); await flushPromises();
      expect(wrapper.get('[data-page="page-3"] svg').attributes('id')).toBe('new-score');
    } finally { wrapper.unmount(); }
  });
  it('opens an unperformed score page without moving the recording and returns to following', async () => {
    vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
    const pause = vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {});
    vi.spyOn(HTMLElement.prototype, 'scrollTo').mockImplementation(() => {});
    vi.stubGlobal('IntersectionObserver', class { observe() {} unobserve() {} disconnect() {} });
    const fetch = vi.fn(async (_url: string) => new Response('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1800 2200"/>'));
    vi.stubGlobal('fetch', fetch);
    const material: MusicContext = { ...context, score_pages: context.score_pages.map(page => page.page_id === 'page-3' ? { ...page, page_id: 'unperformed-99-99' } : page), manifest: { ...context.manifest, pages: context.manifest.pages.map(page => page.page_id === 'page-3' ? { ...page, page_id: 'unperformed-99-99' } : page), score_objects: [
      { source_id: 'unplayed', part_id: 'p', voice: '1', measure: '99', beat: 0, duration_beats: 1, midi_pitch: 60, rest: false, tie: 'none', page_id: 'unperformed-99-99', svg_element_id: 'unplayed' },
    ] } };
    const wrapper = mount(MusicReaderPane, { props: { context: material, owner: 'test' } });
    try {
      await flushPromises();
      wrapper.get('audio').element.currentTime = 2;
      await wrapper.get('.music-details-toggle').trigger('click');
      await wrapper.get('.music-score-only button').trigger('click');
      await flushPromises();
      expect(fetch.mock.calls.map(call => call[0])).toContain('/page-3.svg');
      expect(wrapper.get('audio').element.currentTime).toBe(2);
      expect(pause).toHaveBeenCalled();
      expect(wrapper.get('[role="status"]').text()).toContain('本次录音未演奏');
      expect(wrapper.get('.music-ask').attributes('disabled')).toBeDefined();
      await wrapper.get('[role="status"] button').trigger('click');
      expect(wrapper.find('[role="status"]').exists()).toBe(false);
      expect(wrapper.get('.music-ask').attributes('disabled')).toBeUndefined();
    } finally { wrapper.unmount(); }
  });
  it('loads an evidenced PDF page on seek without asserting note alignment', async () => {
    vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
    vi.spyOn(HTMLElement.prototype, 'scrollTo').mockImplementation(() => {});
    vi.stubGlobal('IntersectionObserver', class { observe() {} unobserve() {} disconnect() {} });
    const fetch = vi.fn(async (_url: string) => new Response('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1800 2200"/>'));
    vi.stubGlobal('fetch', fetch);
    const pdf: MusicContext = { ...context, manifest: { ...context.manifest,
      score: { source: context.manifest.score.source, title: 'PDF', pdf_path: 'score.pdf' },
      events: [{ event_id: 'page-three', title: '末段', explanation: '原谱', listen_for: '留意末段', start_ms: 3000, end_ms: 8000,
        roles: [], evidence: [{ provenance_id: 'pdf', score_object_ids: [], score_page_ids: ['page-3'], audio: null }], relations: [], concept_tags: [] }] } };
    const wrapper = mount(MusicReaderPane, { props: { context: pdf, owner: 'test' } });
    await flushPromises();
    wrapper.vm.seek(4000); await flushPromises();
    expect(fetch.mock.calls.map(call => call[0])).toContain('/page-3.svg');
    expect(wrapper.find('[title="原录音 · 谱页与片段导航，逐音符跟谱待校准"]').exists()).toBe(true);
    expect(wrapper.get('.music-follow-toggle').attributes('disabled')).toBeUndefined();
    await wrapper.get('.music-score-surface').trigger('wheel');
    expect(wrapper.get('.music-follow-toggle').classes()).toContain('is-detached');
    await wrapper.get('.music-follow-toggle').trigger('click');
    expect(wrapper.get('.music-follow-toggle').classes()).not.toContain('is-detached');
    expect(wrapper.findAll('.music-note-overlay circle')).toHaveLength(0);
    wrapper.unmount();
  });
  it('prepares one adjacent source, releases offscreen SVGs, and reopens a recent page without another download', async () => {
    let notify!: IntersectionObserverCallback;
    vi.stubGlobal('IntersectionObserver', class { constructor(callback: IntersectionObserverCallback) { notify = callback; } observe() {} unobserve() {} disconnect() {} });
    const fetch = vi.fn(async () => new Response('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1800 2200"/>'));
    vi.stubGlobal('fetch', fetch);
    const wrapper = mount(MusicReaderPane, { props: { context, owner: 'test' } });
    await nextTick();
    const hosts = wrapper.findAll('.music-score-page');
    notify([{ target: hosts[0].element, isIntersecting: true }] as IntersectionObserverEntry[], {} as IntersectionObserver);
    await flushPromises();
    expect(wrapper.findAll('.music-score-page > svg')).toHaveLength(1);
    expect(fetch.mock.calls).toHaveLength(2);
    expect(hosts[1].find('svg').exists()).toBe(false);
    notify([{ target: hosts[0].element, isIntersecting: false }, { target: hosts[2].element, isIntersecting: true }] as IntersectionObserverEntry[], {} as IntersectionObserver);
    await flushPromises();
    expect(hosts[0].find('svg').exists()).toBe(false);
    expect(hosts[2].find('svg').exists()).toBe(true);
    expect(fetch.mock.calls).toHaveLength(3);
    notify([{ target: hosts[2].element, isIntersecting: false }, { target: hosts[0].element, isIntersecting: true }] as IntersectionObserverEntry[], {} as IntersectionObserver);
    await flushPromises();
    expect(hosts[0].find('svg').exists()).toBe(true);
    expect(fetch.mock.calls).toHaveLength(3);
    wrapper.unmount();
  });

  it('renders the next performed page before scrolling, including a skip and a repeat', async () => {
    let paused = true, notify!: IntersectionObserverCallback;
    vi.spyOn(HTMLMediaElement.prototype, 'paused', 'get').mockImplementation(() => paused);
    vi.stubGlobal('requestAnimationFrame', () => 0);
    vi.stubGlobal('IntersectionObserver', class { constructor(callback: IntersectionObserverCallback) { notify = callback; } observe() {} unobserve() {} disconnect() {} });
    const svg = '<svg xmlns="http://www.w3.org/2000/svg"/>';
    const fetch = vi.fn(async (_url: string) => new Response(svg)); vi.stubGlobal('fetch', fetch);
    const material: MusicContext = { ...context, manifest: { ...context.manifest,
      score_objects: [1, 2, 3].map(page => ({ source_id: 'n' + page, svg_element_id: 'n' + page, part_id: 'p', voice: '1',
        measure: String(page), beat: 0, duration_beats: 1, midi_pitch: 60, rest: false, tie: 'none', page_id: 'page-' + page })),
      occurrences: [1, 3, 2, 3].map((page, i) => ({ occurrence_id: 'o' + i, order: i, measure: String(page), start_ms: i * 2000,
        end_ms: (i + 1) * 2000, score_object_ids: ['n' + page] })),
      coverage: [{ start_ms: 0, end_ms: 8000, granularity: 'measure', method: 'audio_alignment', status: 'accepted', provenance_id: 'audio', detail: '小节定位' }] } };
    const wrapper = mount(MusicReaderPane, { props: { context: material, owner: 'test' } });
    try {
      await flushPromises();
      const audio = wrapper.get('audio'), hosts = wrapper.findAll('.music-score-page'), surface = wrapper.get('.music-score-surface').element as HTMLElement;
      vi.spyOn(hosts[2].element, 'getBoundingClientRect').mockReturnValue(new DOMRect(0, 900, 500, 700));
      const intersect = (i: number, visible: boolean) => notify([{ target: hosts[i].element, isIntersecting: visible }] as IntersectionObserverEntry[], {} as IntersectionObserver);
      intersect(0, true);
      expect(fetch.mock.calls.map(([url]) => url)).toEqual(['/page-1.svg', '/page-3.svg', '/page-2.svg']);
      expect(hosts[2].find('svg').exists()).toBe(false);
      paused = false; await audio.trigger('play'); await flushPromises();
      intersect(2, false); await flushPromises();
      expect(hosts[2].find('svg').exists()).toBe(true);
      expect(hosts[1].find('svg').exists()).toBe(false);
      expect(surface.scrollTop).toBe(0);
      audio.element.currentTime = 2.1; await audio.trigger('timeupdate'); await flushPromises();
      expect(surface.scrollTop).toBe(900);
      expect(hosts[1].find('svg').exists()).toBe(true);
      intersect(0, false); await nextTick(); expect(hosts[0].find('svg').exists()).toBe(false);
      audio.element.currentTime = 4.1; await audio.trigger('timeupdate'); await flushPromises();
      expect(hosts[2].find('svg').exists()).toBe(true);
      expect(fetch.mock.calls.map(([url]) => url)).toEqual(['/page-1.svg', '/page-3.svg', '/page-2.svg']);
      intersect(1, true);
      await wrapper.get('.music-score-surface').trigger('wheel');
      expect(hosts[1].find('svg').exists()).toBe(true);
      expect(hosts[2].find('svg').exists()).toBe(false);
    } finally { paused = true; wrapper.unmount(); }
  });

  it('prepares PDF pages in gaps between aligned bars and returns to the start of a loop', () => {
    const scoreObjects: MusicContext['manifest']['score_objects'] = [1, 3].map(i => ({ source_id: 'n' + i, svg_element_id: 'n' + i,
      part_id: 'p', voice: '1', measure: String(i), beat: 0, duration_beats: 1, midi_pitch: 60, rest: false, tie: 'none', page_id: 'page-' + i }));
    const bars = scoreObjects.map((object, i) => ({ occurrence_id: 'o' + i, order: i, measure: object.measure,
      start_ms: i * 4000, end_ms: i * 4000 + 2000, score_object_ids: [object.source_id] }));
    const turns = scorePageTurns({ ...context.manifest, occurrences: bars, coverage: bars.map(bar => ({ start_ms: bar.start_ms,
      end_ms: bar.end_ms, status: 'accepted', granularity: 'measure', method: 'audio_alignment', provenance_id: 'p', detail: '' })),
      events: [{ event_id: 'pdf', title: '', explanation: '', listen_for: '', roles: [], relations: [], concept_tags: [], start_ms: 0, end_ms: 6000,
        evidence: [{ provenance_id: 'p', score_object_ids: [], score_page_ids: ['page-2'], audio: null }] }] },
    new Map(scoreObjects.map(object => [object.source_id, object])));
    expect(turns).toEqual([{ ms: 0, id: 'page-1' }, { ms: 2000, id: 'page-2' }, { ms: 4000, id: 'page-3' }]);
    expect(upcomingScorePages(turns, 'page-1', 1000, null, 1)).toEqual({ next: 'page-2', soonAfter: 'page-3' });
    expect(upcomingScorePages(turns, 'page-2', 2200, { start_ms: 0, end_ms: 3500 }, 1).next).toBe('page-1');
  });

  it('keeps the readable page through a delayed or failed turn, then retries only the destination', async () => {
    vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
    vi.stubGlobal('IntersectionObserver', class { observe() {} unobserve() {} disconnect() {} });
    const svg = '<svg xmlns="http://www.w3.org/2000/svg"/>';
    let finish!: (response: Response) => void;
    const fetch = vi.fn((url: string) => url === '/page-2.svg' ? new Promise<Response>(resolve => { finish = resolve; }) : Promise.resolve(new Response(svg)));
    vi.stubGlobal('fetch', fetch);
    const pdf = { ...context, manifest: { ...context.manifest, events: [{ event_id: 'second', title: '', explanation: '', listen_for: '', roles: [], relations: [], concept_tags: [],
      start_ms: 3000, end_ms: 8000, evidence: [{ provenance_id: 'pdf', score_object_ids: [], score_page_ids: ['page-2'], audio: null }] }] } };
    const wrapper = mount(MusicReaderPane, { props: { context: pdf, owner: 'test' } });
    try {
      await flushPromises();
      const surface = wrapper.get('.music-score-surface').element as HTMLElement;
      vi.spyOn(surface, 'getBoundingClientRect').mockReturnValue(new DOMRect(0, 0, 500, 400));
      vi.spyOn(wrapper.get('[data-page="page-2"]').element, 'getBoundingClientRect').mockReturnValue(new DOMRect(0, 700, 500, 700));
      wrapper.vm.seek(4000); await nextTick();
      expect(surface.scrollTop).toBe(0);
      expect(wrapper.get('[role="status"]').text()).toContain('第 2 页');
      expect(wrapper.find('[data-page="page-1"] svg').exists()).toBe(true);
      finish(new Response('', { status: 503 })); await flushPromises();
      expect(surface.scrollTop).toBe(0);
      expect(wrapper.get('[role="alert"]').text()).toContain('第 2 页');
      expect(fetch.mock.calls.map(([url]) => url)).toEqual(['/page-1.svg', '/page-2.svg']);
      await wrapper.get('[role="alert"] button').trigger('click');
      finish(new Response(svg)); await flushPromises();
      expect(surface.scrollTop).toBe(700);
      expect(wrapper.find('.music-score-state').exists()).toBe(false);
      expect(wrapper.find('[data-page="page-2"] svg').exists()).toBe(true);
      expect(wrapper.get('audio').element.currentTime).toBe(4);
      expect(fetch.mock.calls.map(([url]) => url)).toEqual(['/page-1.svg', '/page-2.svg', '/page-2.svg']);
    } finally { wrapper.unmount(); }
  });

  it('does not move back to a slow page after a newer seek or a manual scroll', async () => {
    vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
    vi.stubGlobal('IntersectionObserver', class { observe() {} unobserve() {} disconnect() {} });
    let finish!: (response: Response) => void;
    const svg = '<svg xmlns="http://www.w3.org/2000/svg"/>';
    vi.stubGlobal('fetch', vi.fn((url: string) => url === '/page-2.svg' ? new Promise<Response>(resolve => { finish = resolve; }) : Promise.resolve(new Response(svg))));
    const pdf = { ...context, manifest: { ...context.manifest, events: [2, 3].map(i => ({ event_id: String(i), title: '', explanation: '', listen_for: '', roles: [], relations: [], concept_tags: [],
      start_ms: i * 1000, end_ms: (i + 1) * 1000, evidence: [{ provenance_id: 'pdf', score_object_ids: [], score_page_ids: ['page-' + i], audio: null }] })) } };
    const wrapper = mount(MusicReaderPane, { props: { context: pdf, owner: 'test' } });
    try {
      await flushPromises();
      const surface = wrapper.get('.music-score-surface').element as HTMLElement;
      vi.spyOn(surface, 'getBoundingClientRect').mockReturnValue(new DOMRect(0, 0, 500, 400));
      for (const i of [2, 3]) vi.spyOn(wrapper.get('[data-page="page-' + i + '"]').element, 'getBoundingClientRect').mockReturnValue(new DOMRect(0, i * 700, 500, 700));
      wrapper.vm.seek(2100); wrapper.vm.seek(3100); await flushPromises();
      expect(surface.scrollTop).toBe(2100);
      expect(wrapper.find('.music-score-state').exists()).toBe(false);
      wrapper.vm.seek(2100); await nextTick();
      await wrapper.get('.music-score-surface').trigger('wheel');
      finish(new Response(svg)); await flushPromises();
      expect(surface.scrollTop).toBe(2100);
      expect(wrapper.find('[data-page="page-2"] svg').exists()).toBe(false);
    } finally { wrapper.unmount(); }
  });

  it('combines simultaneous choir lyrics and keeps them out of the thematic navigation', async () => {
    vi.spyOn(HTMLMediaElement.prototype, 'readyState', 'get').mockReturnValue(2);
    const vocal = { ...context, score_pages: [], manifest: { ...context.manifest, pages: [], events: ['女高音合唱', '男高音合唱'].map((voice, i) => ({
      event_id: `lyric-${i}`, title: i ? 'freude' : 'Freude', explanation: '欢乐', listen_for: '唱词', start_ms: 1000, end_ms: 3000,
      roles: [], evidence: [], relations: [], concept_tags: ['lyrics', voice],
    })) } };
    const wrapper = mount(MusicReaderPane, { props: { context: vocal, owner: 'test' } });
    wrapper.get('audio').element.currentTime = 1.5;
    await wrapper.get('audio').trigger('timeupdate');
    expect(wrapper.findAll('.music-lyric-line')).toHaveLength(1);
    expect(wrapper.get('.music-lyrics').text()).toContain('女高音合唱 · 男高音合唱');
    expect(wrapper.find('.music-listening-cue').exists()).toBe(false);
    wrapper.unmount();
  });
});
