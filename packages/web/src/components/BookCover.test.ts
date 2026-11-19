// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import BookCover from './BookCover.vue';
import MaterialCover from './MaterialCover.vue';
import { renderPdfCover } from '../pdf-cover';
vi.mock('../pdf-cover', () => ({ renderPdfCover: vi.fn(async () => {}) }));
afterEach(() => { vi.clearAllMocks(); vi.unstubAllGlobals(); });

describe('book covers before opening a book', () => {
  it('keeps a book-title cover when no source exists or its image fails', async () => {
    vi.stubGlobal('IntersectionObserver', undefined);
    const wrapper = mount(BookCover, { props: { title: '没有封面的材料' } });
    expect(wrapper.get('.book-cover-name').text()).toBe('没有封面的材料');
    await wrapper.setProps({ cover: { kind: 'image', url: '/cover.png' } });
    await wrapper.get('img').trigger('error');
    expect(wrapper.find('img').exists()).toBe(false);
    expect(wrapper.get('.book-cover-name').text()).toBe('没有封面的材料');
    await wrapper.setProps({ cover: { kind: 'image', url: '/another.png' } });
    await wrapper.get('img').trigger('load');
    expect(wrapper.find('.book-cover-fallback').exists()).toBe(false);
    wrapper.unmount();
  });
  it('only starts a PDF near the viewport and cancels it when leaving the library', async () => {
    let intersection!: IntersectionObserverCallback;
    vi.stubGlobal('IntersectionObserver', class {
      constructor(callback: IntersectionObserverCallback) { intersection = callback; }
      observe() {} disconnect() {}
    });
    let finish!: () => void;
    vi.mocked(renderPdfCover).mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    const wrapper = mount(BookCover, { props: { title: 'PDF', cover: { kind: 'pdf', url: '/book.pdf' } } });
    expect(renderPdfCover).not.toHaveBeenCalled();
    intersection([{ isIntersecting: true } as IntersectionObserverEntry], {} as IntersectionObserver);
    await flushPromises();
    expect(renderPdfCover).toHaveBeenCalledWith('/book.pdf', expect.any(HTMLCanvasElement), expect.any(AbortController));
    const controller = vi.mocked(renderPdfCover).mock.calls[0][2];
    wrapper.unmount();
    expect(controller.signal.aborted).toBe(true);
    finish(); await flushPromises();
  });
  it('shows the first-page canvas on success and the title on a broken PDF', async () => {
    vi.stubGlobal('IntersectionObserver', undefined);
    const wrapper = mount(BookCover, { props: { title: '论文', cover: { kind: 'pdf', url: '/first.pdf' } } });
    await flushPromises();
    expect(wrapper.get('canvas').isVisible()).toBe(true);
    expect(wrapper.find('.book-cover-fallback').exists()).toBe(false);
    vi.mocked(renderPdfCover).mockRejectedValueOnce(new Error('invalid PDF'));
    await wrapper.setProps({ cover: { kind: 'pdf', url: '/broken.pdf' } });
    await flushPromises();
    expect(wrapper.get('.book-cover-name').text()).toBe('论文');
    // happy-dom caches computed visibility for detached canvas nodes.
    expect(wrapper.get('canvas').element.style.display).toBe('none');
    wrapper.unmount();
  });
  it('defers PDF work through MaterialCover while opening is in progress', async () => {
    vi.stubGlobal('IntersectionObserver', undefined);
    const wrapper = mount(MaterialCover, { props: { title: 'PDF', cover: { kind: 'pdf', url: '/book.pdf' }, paused: true } });
    await flushPromises();
    expect(renderPdfCover).not.toHaveBeenCalled();
    await wrapper.setProps({ paused: false });
    await flushPromises();
    expect(renderPdfCover).toHaveBeenCalledTimes(1);
    expect(wrapper.find('.book-cover-fallback').exists()).toBe(false);
    wrapper.unmount();
  });
  it('cancels an in-flight PDF when paused and resumes without accepting its late result', async () => {
    vi.stubGlobal('IntersectionObserver', undefined);
    let finishCancelled!: () => void;
    let finishResumed!: () => void;
    vi.mocked(renderPdfCover)
      .mockImplementationOnce(() => new Promise(resolve => { finishCancelled = resolve; }))
      .mockImplementationOnce(() => new Promise(resolve => { finishResumed = resolve; }));
    const wrapper = mount(BookCover, { props: { title: 'PDF', cover: { kind: 'pdf', url: '/book.pdf' } } });
    await flushPromises();
    const cancelledController = vi.mocked(renderPdfCover).mock.calls[0][2];
    await wrapper.setProps({ paused: true });
    expect(cancelledController.signal.aborted).toBe(true);
    expect(renderPdfCover).toHaveBeenCalledTimes(1);
    await wrapper.setProps({ paused: false });
    expect(renderPdfCover).toHaveBeenCalledTimes(2);
    expect(vi.mocked(renderPdfCover).mock.calls[1][2].signal.aborted).toBe(false);
    finishCancelled();
    await flushPromises();
    expect(wrapper.find('.book-cover-fallback').exists()).toBe(true);
    finishResumed();
    await flushPromises();
    expect(wrapper.find('.book-cover-fallback').exists()).toBe(false);
    wrapper.unmount();
  });
  it('keeps an already loaded PDF visible across pause and resume without rendering again', async () => {
    vi.stubGlobal('IntersectionObserver', undefined);
    const wrapper = mount(BookCover, { props: { title: 'PDF', cover: { kind: 'pdf', url: '/book.pdf' } } });
    await flushPromises();
    const canvas = wrapper.get('canvas').element;
    await wrapper.setProps({ paused: true });
    expect(wrapper.find('.book-cover-fallback').exists()).toBe(false);
    expect(canvas.style.display).not.toBe('none');
    await wrapper.setProps({ paused: false });
    await flushPromises();
    expect(wrapper.get('canvas').element).toBe(canvas);
    expect(wrapper.find('.book-cover-fallback').exists()).toBe(false);
    expect(renderPdfCover).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });
  it('keeps image covers loaded while PDF cover work is paused', async () => {
    vi.stubGlobal('IntersectionObserver', undefined);
    const wrapper = mount(BookCover, { props: { title: '图片', cover: { kind: 'image', url: '/cover.png' } } });
    await flushPromises();
    await wrapper.get('img').trigger('load');
    await wrapper.setProps({ paused: true });
    expect(wrapper.get('img').attributes('src')).toBe('/cover.png');
    expect(wrapper.find('.book-cover-fallback').exists()).toBe(false);
    expect(renderPdfCover).not.toHaveBeenCalled();
    wrapper.unmount();
  });
});
