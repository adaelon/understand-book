// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import BookCover from './BookCover.vue';
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
});
