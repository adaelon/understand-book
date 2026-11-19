// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { api, type AssetManifest, type BoardSourceReadResponse, type SourcePopupView, type SourceWindowLoader } from './api';
import BoardSourceReader from './components/BoardSourceReader.vue';

const request = { board_id: 'board-one', item_id: 'answer-one', source_ref_id: 'source-one' };
const source: SourcePopupView = { source_ref_id: request.source_ref_id, label: '距离与站位', material_title: '战术手册', highlighted_quote: '攻击距离是六格', context_before: '', context_after: '', stale: false, can_open_in_reader: true };
const segment = (lid: string, text = `原文段落 ${lid}。`): BoardSourceReadResponse['segments'][number] => ({ lid, text, kind: 'paragraph', highlights: [] });
const windowResult = (patch: Partial<BoardSourceReadResponse> = {}): BoardSourceReadResponse => ({
  source_ref_id: request.source_ref_id, source_lid: '2.3', focus_lid: '2.3',
  segments: [segment('2.1', '## 距离与站位'), segment('2.2'), { ...segment('2.3', '攻击距离是**六格**，请留意。'), highlights: [{ start: 0, end: 11 }] }, segment('2.4')],
  previous_lid: '2.1', next_lid: '2.4',
  outline: [{ lid: '1', title: '第一章 基础', depth: 0 }, { lid: '2', title: '第二章 距离与站位', depth: 0 }, { lid: '3', title: '第三章 练习', depth: 0 }],
  ...patch,
});
let view: ReturnType<typeof mount> | undefined;
function rectangle(top: number, height: number): DOMRect {
  return { x: 0, y: top, top, bottom: top + height, left: 0, right: 700, width: 700, height, toJSON: () => ({}) } as DOMRect;
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}
async function open() {
  const readWindow: SourceWindowLoader = options => api.boardSourceRead({ ...request, ...options });
  view = mount(BoardSourceReader, { props: { readWindow, source }, attachTo: document.body });
  await flushPromises();
  return view;
}
beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function(this: HTMLElement) {
    const scroll = document.querySelector<HTMLElement>('[data-source-reader-scroll]');
    if (this.hasAttribute('data-source-reader-scroll')) return rectangle(100, 500);
    const article = this.closest<HTMLElement>('[data-source-lid]');
    if (article) {
      const elements = [...document.querySelectorAll('[data-source-lid]')];
      const top = 100 + 80 + elements.indexOf(article) * 180 - (scroll?.scrollTop ?? 0);
      return rectangle(top + (this.tagName === 'MARK' ? 20 : 0), this.tagName === 'MARK' ? 25 : 160);
    }
    return rectangle(0, 0);
  });
  vi.spyOn(api, 'boardSourceRead').mockResolvedValue(windowResult());
});
afterEach(() => { view?.unmount(); view = undefined; document.body.innerHTML = ''; vi.restoreAllMocks(); });

describe('continuous source reading', () => {
  it('rejoins the published MLA sentence across inline formulas while retaining per-source highlights', async () => {
    const parts = [
      ['paragraph', '只保存潜变量的依据如下。设键的上投影为 '],
      ['formula', '$U_k$'],
      ['paragraph', '，上下文潜变量按行组成矩阵 '],
      ['formula', '$c$'],
      ['paragraph', '，展开键为 '],
      ['formula', '$K=cU_k$'],
      ['paragraph', '。矩阵乘法的结合律给出'],
    ].map(([kind, text], index) => ({ lid: `4.20.6.${13 + index}`, kind, text, highlights: [{ start: 0, end: index === 6 ? 1 : text.length }] }));
    const display = { lid: '4.20.6.20', kind: 'formula', text: '$$\nqK^{\\mathsf T}=(qU_k^{\\mathsf T})c^{\\mathsf T}.\n$$', highlights: [] };
    vi.mocked(api.boardSourceRead).mockResolvedValueOnce(windowResult({ source_lid: parts[0].lid, focus_lid: parts[0].lid,
      segments: [...parts, display, segment('4.20.6.21', '下一段保持独立。'), segment('4.20.6.22', '再下一段。')] }));
    const reader = await open();
    const sentence = reader.find('[data-source-lid="4.20.6.13"]').element.parentElement!;
    expect(sentence.tagName).toBe('P');
    expect([...sentence.querySelectorAll('[data-source-lid]')].map(node => node.getAttribute('data-source-lid'))).toEqual(parts.map(part => part.lid));
    expect(sentence.querySelectorAll('p, article')).toHaveLength(0);
    expect(sentence.querySelectorAll('.katex')).toHaveLength(3);
    for (const part of parts) expect(reader.find(`[data-source-lid="${part.lid}"] mark`).exists()).toBe(true);
    expect(reader.find('[data-source-lid="4.20.6.19"] mark').text()).toBe('。');
    expect(reader.find('[data-source-lid="4.20.6.20"] .katex-display').exists()).toBe(true);
    expect(sentence.contains(reader.find('[data-source-lid="4.20.6.20"]').element)).toBe(false);
    for (const lid of ['4.20.6.21', '4.20.6.22']) {
      expect(reader.find(`[data-source-lid="${lid}"]`).element.tagName).toBe('ARTICLE');
      expect(reader.find(`[data-source-lid="${lid}"] p`).exists()).toBe(true);
    }
  });

  it('preserves block markdown in paragraph leaves outside an inline formula sentence', async () => {
    vi.mocked(api.boardSourceRead).mockResolvedValueOnce(windowResult({ segments: [
      segment('2.3', '- 保存键\n- 保存值'), segment('2.4', '> 核对原文。'),
    ] }));
    const reader = await open();
    expect(reader.findAll('[data-source-lid="2.3"] ul li').map(item => item.text())).toEqual(['保存键', '保存值']);
    expect(reader.find('[data-source-lid="2.4"] blockquote').text()).toBe('核对原文。');
  });

  it('renders book anchor markers without visible markup while preserving the following citation highlight', async () => {
    vi.mocked(api.boardSourceRead).mockResolvedValueOnce(windowResult({ segments: [
      segment('2.2', '<a id="ch20-exercises"></a>'),
      { ...segment('2.3', '习题引用的文字。'), highlights: [{ start: 0, end: 7 }] },
    ] }));
    const reader = await open();
    expect(reader.find('[data-source-lid="2.2"]').text()).toBe('');
    expect(reader.find('[data-source-lid="2.2"] a').attributes('id')).toBe('ch20-exercises');
    expect(reader.find('[data-source-lid="2.3"] mark').text()).toBe('习题引用的文字');
  });

  it('joins a sentence split across fetched pages without losing the original citation or cursor', async () => {
    const first = { ...segment('2.3', '保存变量 '), highlights: [{ start: 0, end: 4 }] };
    vi.mocked(api.boardSourceRead).mockResolvedValueOnce(windowResult({ segments: [first], next_lid: first.lid }));
    const reader = await open();
    const scroll = reader.find('[data-source-reader-scroll]').element as HTMLElement;
    const originalTop = reader.find('[data-source-lid="2.3"]').element.getBoundingClientRect().top;
    vi.mocked(api.boardSourceRead).mockResolvedValueOnce(windowResult({ segments: [{ ...segment('2.4', '$c$'), kind: 'formula' }, segment('2.5', '，随后恢复。')], next_lid: null, outline: undefined }));
    await reader.find('.source-reader-edge-after button').trigger('click');
    await flushPromises();
    const sentence = reader.find('[data-source-lid="2.3"]').element.parentElement!;
    expect(sentence.tagName).toBe('P');
    expect(sentence.querySelectorAll('[data-source-lid]')).toHaveLength(3);
    expect(reader.find('[data-source-lid="2.3"] mark').text()).toBe('保存变量');
    expect(reader.find('[data-source-lid="2.3"]').element.getBoundingClientRect().top).toBe(originalTop);
    expect(scroll.scrollTop).toBeGreaterThan(0);
    expect(api.boardSourceRead).toHaveBeenLastCalledWith({ ...request, anchor_lid: '2.3', direction: 'after', include_outline: false });
  });

  it('loads the citation binding read-only, renders original markdown and locates the cited paragraph', async () => {
    const reader = await open();
    expect(api.boardSourceRead).toHaveBeenCalledOnce();
    expect(api.boardSourceRead).toHaveBeenCalledWith({ ...request, anchor_lid: undefined, direction: 'around', include_outline: true });
    expect(reader.find('[data-source-lid="2.3"] strong').text()).toBe('六格');
    expect(reader.find('[data-source-lid="2.3"] mark').text()).toContain('攻击距离');
    expect(reader.emitted('position')?.at(-1)).toEqual(['2.3']);
    expect(reader.find('[data-outline-lid="2"]').attributes('aria-current')).toBe('location');
    expect((reader.find('[data-source-reader-scroll]').element as HTMLElement).scrollTop).toBeGreaterThan(0);
  });

  it('prepends earlier text without moving the paragraph being read, then appends later text', async () => {
    const reader = await open();
    const scroll = reader.find('[data-source-reader-scroll]').element as HTMLElement;
    const originalTop = reader.find('[data-source-lid="2.3"]').element.getBoundingClientRect().top;
    vi.mocked(api.boardSourceRead).mockResolvedValueOnce(windowResult({ segments: [segment('1.8'), segment('1.9')], previous_lid: '1.8', next_lid: '1.9', outline: undefined }));
    await reader.find('.source-reader-edge-before button').trigger('click');
    await flushPromises();
    expect(api.boardSourceRead).toHaveBeenLastCalledWith({ ...request, anchor_lid: '2.1', direction: 'before', include_outline: false });
    expect(reader.findAll('[data-source-lid]').map(node => node.attributes('data-source-lid'))).toEqual(['1.8', '1.9', '2.1', '2.2', '2.3', '2.4']);
    expect(reader.find('[data-source-lid="2.3"]').element.getBoundingClientRect().top).toBe(originalTop);
    const retainedScroll = scroll.scrollTop;
    vi.mocked(api.boardSourceRead).mockResolvedValueOnce(windowResult({ segments: [segment('2.5'), segment('2.6')], previous_lid: '2.5', next_lid: null, outline: undefined }));
    await reader.find('.source-reader-edge-after button').trigger('click');
    await flushPromises();
    expect(api.boardSourceRead).toHaveBeenLastCalledWith({ ...request, anchor_lid: '2.4', direction: 'after', include_outline: false });
    expect(reader.find('[data-source-lid="2.6"]').exists()).toBe(true);
    expect(scroll.scrollTop).toBe(retainedScroll);
    expect(reader.find('.source-reader-edge-after').text()).toBe('已到材料末尾');
  });

  it('jumps through the chapter contents and returns to the original citation', async () => {
    const reader = await open();
    vi.mocked(api.boardSourceRead).mockResolvedValueOnce(windowResult({ focus_lid: '3.1', segments: [segment('2.40'), segment('3.1', '## 第三章 练习'), segment('3.2')], outline: undefined }));
    await reader.find('[data-outline-lid="3"]').trigger('click');
    await flushPromises();
    expect(api.boardSourceRead).toHaveBeenLastCalledWith({ ...request, anchor_lid: '3', direction: 'around', include_outline: false });
    expect(reader.emitted('position')?.at(-1)).toEqual(['3.1']);
    expect(reader.find('.source-reader-location').text()).toBe('第三章 练习');
    vi.mocked(api.boardSourceRead).mockResolvedValueOnce(windowResult({ outline: undefined }));
    await reader.find('.source-reader-return').trigger('click');
    await flushPromises();
    expect(api.boardSourceRead).toHaveBeenLastCalledWith({ ...request, anchor_lid: undefined, direction: 'around', include_outline: false });
    expect(reader.emitted('position')?.at(-1)).toEqual(['2.3']);
  });

  it('retains readable text after an incremental failure and retries the same page', async () => {
    const reader = await open();
    vi.mocked(api.boardSourceRead).mockRejectedValueOnce(new Error('连接暂时中断'));
    await reader.find('.source-reader-edge-after button').trigger('click');
    await flushPromises();
    expect(reader.find('[data-source-lid="2.3"]').exists()).toBe(true);
    expect(reader.find('[role="alert"]').text()).toContain('连接暂时中断');
    vi.mocked(api.boardSourceRead).mockResolvedValueOnce(windowResult({ segments: [segment('2.5')], next_lid: null, outline: undefined }));
    await reader.find('.source-reader-error button').trigger('click');
    await flushPromises();
    expect(api.boardSourceRead).toHaveBeenLastCalledWith({ ...request, anchor_lid: '2.4', direction: 'after', include_outline: false });
    expect(reader.find('[data-source-lid="2.5"]').exists()).toBe(true);
    expect(reader.find('[role="alert"]').exists()).toBe(false);
  });

  it('ignores an older source response after a different citation is selected', async () => {
    const earlier = deferred<BoardSourceReadResponse>();
    vi.mocked(api.boardSourceRead).mockReturnValueOnce(earlier.promise);
    const reader = await open();
    const laterRequest = { ...request, source_ref_id: 'source-two' };
    vi.mocked(api.boardSourceRead).mockResolvedValueOnce(windowResult({ source_ref_id: 'source-two', source_lid: '9.1', focus_lid: '9.1', segments: [segment('9.1', '新的来源内容')], previous_lid: null, next_lid: null }));
    await reader.setProps({ readWindow: (options: Parameters<SourceWindowLoader>[0]) => api.boardSourceRead({ ...laterRequest, ...options }), source: { ...source, source_ref_id: 'source-two' } });
    await flushPromises();
    earlier.resolve(windowResult());
    await flushPromises();
    expect(reader.find('[data-source-lid="9.1"]').text()).toBe('新的来源内容');
    expect(reader.find('[data-source-lid="2.3"]').exists()).toBe(false);
    expect(reader.emitted('position')?.at(-1)).toEqual(['9.1']);
  });

  it('loads one image manifest for the citation publication and shares it between paragraphs', async () => {
    const publication = { book_id: 'another-book', publication_id: 'retained-publication' };
    const manifest: AssetManifest = { version: 'asset_manifest.v1', book_id: publication.book_id, images: [{ kind: 'image', lid: '2.3', alt: '图示', original_src: 'images/range.png', source: 'markdown', status: 'available', stored_path: null, url_path: '/published/another-book/retained-publication/range.png', mime: 'image/png', sha256: null, size_bytes: 1024, warning: null }] };
    vi.spyOn(api, 'assetManifest').mockResolvedValue(manifest);
    vi.mocked(api.boardSourceRead).mockResolvedValueOnce(windowResult({ published_book_ref: publication, segments: [segment('2.3', '![图示](images/range.png)'), segment('2.4', '![另一图示](images/range.png)')] }));
    const reader = await open();
    expect(api.assetManifest).toHaveBeenCalledOnce();
    expect(api.assetManifest).toHaveBeenCalledWith(publication);
    expect(reader.findAll('img').map(image => image.attributes('src'))).toEqual([manifest.images[0].url_path, manifest.images[0].url_path]);
  });

  it('keeps the rendered window bounded while retaining a cursor back to discarded text', async () => {
    const reader = await open();
    for (let page = 0; page < 6; page++) {
      const additions = Array.from({ length: 20 }, (_, index) => segment(`4.${page * 20 + index + 1}`));
      vi.mocked(api.boardSourceRead).mockResolvedValueOnce(windowResult({ segments: additions, next_lid: additions.at(-1)!.lid, outline: undefined }));
      await reader.find('.source-reader-edge-after button').trigger('click');
      await flushPromises();
    }
    expect(reader.findAll('[data-source-lid]')).toHaveLength(96);
    expect(reader.find('.source-reader-edge-before button').exists()).toBe(true);
    expect(reader.find('[data-source-lid="4.120"]').exists()).toBe(true);
  });
});
