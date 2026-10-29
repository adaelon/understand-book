// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import type { SourcePopupView } from './api';
import BoardSourcePreview from './components/BoardSourcePreview.vue';
import MediaSourcePreview from './components/MediaSourcePreview.vue';
import MusicSourcePreview from './components/MusicSourcePreview.vue';
import SourceExcerpt from './components/SourceExcerpt.vue';

const base = (): SourcePopupView => ({ source_ref_id: 'board-source', label: '另一份材料 · 保留的来源', material_title: '来源材料标题', highlighted_quote: '这是引用当时保留的完整原话。', context_before: '', context_after: '', stale: false, can_open_in_reader: false });
let wrapper: ReturnType<typeof mount> | undefined;
beforeEach(() => { vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {}); });
afterEach(() => { wrapper?.unmount(); wrapper = undefined; document.body.innerHTML = ''; vi.restoreAllMocks(); });
async function open(source: SourcePopupView) { wrapper = mount(BoardSourcePreview, { props: { source }, attachTo: document.body }); await flushPromises(); return wrapper; }

describe('board retained source preview', () => {
  it('reads the available quote with surrounding context before explicitly opening the reader', async () => {
    const source = { ...base(), can_open_in_reader: true, context_before: '前一段。', context_after: '后一段。', heading_path: ['第六章', '攻击距离'] };
    const view = await open(source);
    const dialog = document.querySelector('[role="dialog"]')!;
    expect(dialog.textContent).toContain('前一段。'); expect(dialog.textContent).toContain('后一段。');
    expect(dialog.querySelector('mark')?.textContent).toBe(source.highlighted_quote);
    expect(dialog.textContent).toContain('第六章 › 攻击距离');
    expect(view.emitted('open-reader')).toBeUndefined();
    (dialog.querySelector('.board-source-open-reader') as HTMLButtonElement).click(); await flushPromises();
    expect(view.emitted('open-reader')).toHaveLength(1);
    await view.setProps({ opening: true });
    expect(dialog.querySelector('.board-source-open-reader')).toHaveProperty('disabled', true);
    expect(dialog.textContent).toContain('正在打开…');
  });
  it('shows pending and retryable errors without an empty excerpt or reader navigation', async () => {
    wrapper = mount(BoardSourcePreview, { props: { source: null, loading: true }, attachTo: document.body });
    await flushPromises();
    const dialog = document.querySelector('[role="dialog"]')!;
    expect(dialog.textContent).toContain('正在加载原文…');
    expect(wrapper.findComponent(SourceExcerpt).exists()).toBe(false);
    expect(dialog.querySelector('.board-source-open-reader')).toBeNull();
    await wrapper.setProps({ loading: false, error: '网络连接已断开' });
    expect(dialog.querySelector('[role="alert"]')?.textContent).toContain('网络连接已断开');
    (dialog.querySelector('.board-source-error button') as HTMLButtonElement).click(); await flushPromises();
    expect(wrapper.emitted('retry')).toHaveLength(1);
  });
  it('renders a stale text snapshot and its reason without navigating away from the board', async () => {
    const before = window.location.href;
    const source = { ...base(), stale: true, unavailable_reason: '原发布版本当前不可读取' };
    const view = await open(source);
    expect(view.findComponent(SourceExcerpt).props('source')).toEqual(source);
    expect(view.findComponent(MediaSourcePreview).exists()).toBe(false);
    const dialog = document.querySelector('[role="dialog"]')!;
    expect(dialog.textContent).toContain(source.label); expect(dialog.textContent).toContain(source.highlighted_quote);
    expect(dialog.textContent).toContain('引用时的快照'); expect(dialog.textContent).toContain(source.unavailable_reason);
    expect(dialog.querySelector('.board-source-open-reader')).toBeNull();
    expect(window.location.href).toBe(before);
  });
  it('uses the retained media projection and preserves unavailable feedback from the existing preview', async () => {
    const source: SourcePopupView = { ...base(), stale: true, unavailable_reason: '原发布媒体离线，已保留画面引用', media: { target: { kind: 'visual', publication: { book_id: 'another-video', publication_id: 'video-pub' }, frames: [], regions: [] }, publication: { book_id: 'another-video', publication_id: 'video-pub' }, duration_ms: 10000, transcript: [], translations: [] } };
    const view = await open(source);
    expect(view.findComponent(MediaSourcePreview).props('source')).toEqual(source);
    expect(view.findComponent(SourceExcerpt).exists()).toBe(false);
    const content = document.querySelector('[role="dialog"]')!.textContent!;
    expect(content).toContain(source.unavailable_reason);
    expect(content.split(source.unavailable_reason!).length - 1).toBe(1);
  });
  it('plays the exact retained music URL, passes its source version through, and pauses on close', async () => {
    const source: SourcePopupView = { ...base(), media_url: '/published/other-music/pub-7/recording.mp3', music: { title: '原录音', label: '第六小节', preview: '引用时听到的旋律', material: {}, target: { publication: { book_id: 'other-music', publication_id: 'pub-7' }, audio: { recording: { recording_id: 'performance', revision: 'r7' }, start_ms: 2000, end_ms: 4500 }, score: null, occurrence_ids: [], event_ids: [], provenance_ids: [] } } };
    const view = await open(source);
    expect(view.findComponent(MusicSourcePreview).props('source')).toEqual(source);
    expect(document.querySelector('audio')!.getAttribute('src')).toBe(source.media_url);
    (document.querySelector('.board-source-close') as HTMLButtonElement).click(); await flushPromises();
    expect(view.emitted('close')).toHaveLength(1); expect(HTMLMediaElement.prototype.pause).toHaveBeenCalled();
  });
  it('keeps keyboard focus inside the modal and restores the original control after Escape closes it', async () => {
    const origin = document.createElement('button'); origin.textContent = '画板上的来源'; document.body.append(origin); origin.focus();
    const view = await open(base());
    const first = document.querySelector('.board-source-close') as HTMLButtonElement;
    const last = document.querySelector('.board-source-dialog footer button') as HTMLButtonElement;
    expect(document.activeElement).toBe(first);
    first.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true, cancelable: true }));
    expect(document.activeElement).toBe(last);
    last.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }));
    expect(document.activeElement).toBe(first);
    // A loading chapter can disable a focused control, leaving focus on body.
    first.blur();
    document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })); await flushPromises();
    expect(view.emitted('close')).toHaveLength(1);
    view.unmount(); wrapper = undefined;
    expect(document.activeElement).toBe(origin);
  });
});
