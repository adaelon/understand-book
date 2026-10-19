// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { expect, it, vi } from 'vitest';
import MediaSourcePreview from './MediaSourcePreview.vue';
import type { SourcePopupView } from '../api';
const video = { media_id: 'original', media_revision: '1' };
const publication = { book_id: 'book', publication_id: 'old' };
const frame = { video, actual_pts_ms: 492000, width: 1920, height: 1080 };
const transcript = { transcript_id: 't', transcript_revision: '1', video };
function source(): SourcePopupView { return {
  source_ref_id: 'saved', label: '联合来源 · 08:12', highlighted_quote: 'A frame.', context_before: '', context_after: '', stale: false, can_open_in_reader: true,
  media_url: '/original-publication/media', frames: [{ frame, url: 'data:image/png;base64,fixture' }],
  media: { publication, duration_ms: 600000, target: { kind: 'joint', members: [
    { target: { kind: 'visual', publication, frames: [frame], regions: [] }, support_role: '公式' },
    { target: { kind: 'audio', publication, location: { video, start_ms: 490000, end_ms: 495000 }, review_result_ref: 'review' }, support_role: '讲者原话' },
  ] }, transcript: [{ segment_id: 's', text: 'A frame.', text_range: { lid: '1', range: { start: 0, end: 8 } }, location: { video, start_ms: 490000, end_ms: 495000 }, words: [] }],
  translations: [{ transcript, segment_id: 's', zh_text: '一帧。' }] },
}; }
it('previews saved frames, English, translations and bounded audio without touching the main player', async () => {
  const main = document.createElement('video'); main.currentTime = 507; main.playbackRate = 1.5; document.body.append(main);
  const wrapper = mount(MediaSourcePreview, { props: { source: source() } });
  expect(wrapper.get('img').attributes('alt')).toContain('08:12');
  expect(wrapper.text()).toContain('公式'); expect(wrapper.text()).toContain('A frame.'); expect(wrapper.text()).toContain('一帧。');
  const audio = wrapper.get('audio'); await audio.trigger('loadedmetadata'); expect(audio.element.currentTime).toBe(490);
  const pause = vi.spyOn(audio.element, 'pause'); audio.element.currentTime = 496; await audio.trigger('timeupdate');
  expect(pause).toHaveBeenCalled(); expect(audio.element.currentTime).toBe(490);
  wrapper.unmount(); expect(main.currentTime).toBe(507); expect(main.playbackRate).toBe(1.5); main.remove();
});
it('keeps saved context when the original publication is unavailable and reports playback failures', async () => {
  const saved = source(); saved.stale = true; saved.media_url = null; saved.frames = []; saved.unavailable_reason = '原发布媒体当前不可用';
  const wrapper = mount(MediaSourcePreview, { props: { source: saved } });
  expect(wrapper.get('[role=alert]').text()).toContain('不可用'); expect(wrapper.text()).toContain('A frame.'); expect(wrapper.find('video').exists()).toBe(false);
  await wrapper.setProps({ source: source() }); await wrapper.get('video').trigger('error'); expect(wrapper.get('[role=alert]').text()).toContain('快照'); wrapper.unmount();
});

it('orders referenced frames and shows the intervening process as unobserved even without playable media', () => {
  const saved = source();
  const later = { ...frame, actual_pts_ms: 507000 };
  saved.media!.target = { kind: 'visual', publication, frames: [later, frame], regions: [] };
  saved.frames = [{ frame: later, url: 'later' }, { frame, url: 'earlier' }];
  saved.stale = true; saved.media_url = null;
  const wrapper = mount(MediaSourcePreview, { props: { source: saved } });
  expect(wrapper.findAll('img').map(i => i.attributes('src'))).toEqual(['earlier', 'later']);
  expect(wrapper.get('[aria-label="未连续观察的间隙"]').text()).toContain('08:12–08:27：帧间过程未连续观察');
  expect(wrapper.find('video').exists()).toBe(false);
  wrapper.unmount();
});

it('retains the actual audio review and its uncertainties with the saved source', async () => {
  const saved = source();
  saved.media!.audio_reviews = [{ review_result_ref: 'review', question: 'What was said?',
    location: { video, start_ms: 490000, end_ms: 495000 }, reviewed_text: 'underflow and overflow', uncertainties: ['末词仍听不清'] }];
  const wrapper = mount(MediaSourcePreview, { props: { source: saved } });
  expect(wrapper.get('[aria-label="原音频复核结果"]').text()).toContain('underflow and overflow');
  expect(wrapper.text()).toContain('末词仍听不清');
  expect(wrapper.text()).toContain('A frame.');
  await wrapper.setProps({ source: { ...saved, stale: true, media_url: null } });
  expect(wrapper.text()).toContain('末词仍听不清');
  expect(wrapper.find('audio').exists()).toBe(false);
  wrapper.unmount();
});
