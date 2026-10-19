// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import SourceExcerpt from './SourceExcerpt.vue';
import { api, type AssetManifest, type SourcePopupView } from '../api';

afterEach(() => vi.restoreAllMocks());

function source(text: string, quote: string): SourcePopupView {
  const start = text.indexOf(quote);
  return { source_ref_id: 'a', label: '正文 · 方法', highlighted_quote: quote, context_before: '', context_after: '', stale: false, can_open_in_reader: true,
    excerpt: { text, highlight: { start, end: start + quote.length } } };
}

it('renders complete Markdown before highlighting a quote inside emphasis', () => {
  const text = '# 方法\n\nBefore **exact evidence** after.\n\n- First\n- Second\n\n| A | B |\n|---|---|\n| 1 | 2 |';
  const wrapper = mount(SourceExcerpt, { props: { source: source(text, 'exact evidence') } });
  expect(wrapper.get('h1').text()).toBe('方法');
  expect(wrapper.get('strong mark').text()).toBe('exact evidence');
  expect(wrapper.get('p').text()).toBe('Before exact evidence after.');
  expect(wrapper.findAll('li')).toHaveLength(2);
  expect(wrapper.findAll('td')).toHaveLength(2);
});

it('keeps formula rendering and code fences intact across quote boundaries', () => {
  const text = 'Energy $E=mc^2$.\n\n```python\nfirst = 1\nsecond = 2\n```';
  const wrapper = mount(SourceExcerpt, { props: { source: source(text, 'E=mc^2') } });
  expect(wrapper.find('.katex').exists()).toBe(true);
  expect(wrapper.find('mark .katex').exists()).toBe(true);
  expect(wrapper.get('pre code').text()).toContain('first = 1\nsecond = 2');
});

it('shows stale truncated snapshots as saved text', () => {
  const saved = source('**partial', '**partial');
  saved.stale = true;
  const wrapper = mount(SourceExcerpt, { props: { source: saved } });
  expect(wrapper.text()).toBe('**partial');
  expect(wrapper.find('strong').exists()).toBe(false);
});

it('renders the EPUB pipe table between paragraphs and keeps its quote in the correct cell', () => {
  const text = '同一批候选的两条命运。\n\n| 教学候选 | 所属单元与来源 | 选入章节 | 进入宏观路线 |\n| 缓存复用已有结果 | 缓存，1.2 | 是 | 是 |\n| 数据变化后使缓存失效 | 缓存，1.3.2 | 是 | 否 |\n\n实际结果为 3 个候选。';
  const wrapper = mount(SourceExcerpt, { props: { source: source(text, '数据变化后使缓存失效') } });
  expect(wrapper.findAll('tr')).toHaveLength(3);
  expect(wrapper.findAll('td')).toHaveLength(12);
  expect(wrapper.get('td mark').text()).toBe('数据变化后使缓存失效');
  expect(wrapper.findAll('p').map(p => p.text())).toEqual(['同一批候选的两条命运。', '实际结果为 3 个候选。']);
});

it('renders source HTML tables and leaves pipe examples in code fences as code', () => {
  const text = '结果\n\n<table>\n<tr><th colspan="2">候选</th></tr>\n<tr><td>缓存</td><td>复用</td></tr>\n</table>\n\n```text\n| A | B |\n| 1 | 2 |\n```';
  const wrapper = mount(SourceExcerpt, { props: { source: source(text, '复用') } });
  expect(wrapper.get('th').attributes('colspan')).toBe('2');
  expect(wrapper.get('td mark').text()).toBe('复用');
  expect(wrapper.findAll('table')).toHaveLength(1);
  expect(wrapper.get('pre code').text()).toContain('| A | B |');
});

const imageManifest: AssetManifest = { version: 'asset_manifest.v1', book_id: 'b', images: [{ kind: 'image', lid: '1.2',
  original_src: 'images/cache diagram.png', alt: '缓存流程', source: 'epub', status: 'available',
  stored_path: 'assets/cache.png', url_path: '/api/book/assets/cache.png', mime: 'image/png', sha256: null, size_bytes: null, warning: null }] };

it('resolves source images through the same asset manifest as the Reader', async () => {
  const manifest = vi.spyOn(api, 'assetManifest').mockResolvedValue(imageManifest);
  const wrapper = mount(SourceExcerpt, { props: { source: source('![缓存流程](<images/cache diagram.png>)\n\n缓存说明', '缓存说明') } });
  await flushPromises();
  expect(manifest).toHaveBeenCalledOnce();
  expect(wrapper.get('img').attributes('src')).toBe('/api/book/assets/cache.png');
  expect(wrapper.get('mark').text()).toBe('缓存说明');
  await wrapper.get('img').trigger('error');
  expect(wrapper.text()).toContain('缓存流程 · 图片暂不可用');
  wrapper.unmount();
});

it('uses the cited publication for cross-book images and ignores a previous source response', async () => {
  let finish!: (manifest: AssetManifest) => void;
  const manifest = vi.spyOn(api, 'assetManifest').mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const image = source('![缓存流程](<images/cache diagram.png>)', '缓存流程');
  image.source_url = '/?book_id=reference-book&publication_id=version-2&start_lid=1.2';
  const wrapper = mount(SourceExcerpt, { props: { source: image } });
  expect(manifest).toHaveBeenCalledWith({ book_id: 'reference-book', publication_id: 'version-2' });
  await wrapper.setProps({ source: source('另一份来源', '另一份来源') });
  finish(imageManifest);
  await flushPromises();
  expect(wrapper.find('img').exists()).toBe(false);
  expect(wrapper.get('mark').text()).toBe('另一份来源');
  wrapper.unmount();
});
