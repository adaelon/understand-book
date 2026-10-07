// @vitest-environment happy-dom
import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import SourceExcerpt from './SourceExcerpt.vue';
import type { SourcePopupView } from '../api';

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
