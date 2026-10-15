// @vitest-environment happy-dom
import { expect, it } from 'vitest';
import { renderMarkdown } from './md';
import { answerNoteExcerpt } from './answer-note-selection';

it.each([
  '__粗体__，以及 _斜体_。',
  '3. 第一项\n4. 第二项',
  '```typescript\nconst n = 2;\n```',
  '|列 A|列 B|\n|:---|---:|\n|甲|乙|',
  '公式 $ x^2 $ 与 [链接](https://example.com) 。',
  '> 引文\n> 第二行\n\n后段',
])('returns the delivered source for rendered selection: %s', source => {
  const root = document.createElement('div'); root.innerHTML = renderMarkdown(source);
  const range = document.createRange(); range.selectNodeContents(root);
  const excerpt = answerNoteExcerpt(range, [source]);
  expect(excerpt).toBe(source);
});

it('handles a partial formatted span and refuses to stitch separate Markdown parts', () => {
  const source = '前面 __abcdef__ 后面';
  const root = document.createElement('div'); root.innerHTML = renderMarkdown(source);
  const range = document.createRange(); range.setStart(root.querySelector('strong')!.firstChild!, 1); range.setEnd(root.querySelector('strong')!.firstChild!, 4);
  expect(answerNoteExcerpt(range, [source])).toBe('bcd');
  root.innerHTML = '<p>first</p><button class="agent-source-button">来源</button><p>second</p>';
  range.selectNodeContents(root);
  expect(answerNoteExcerpt(range, ['first', 'second'])).toBeNull();
});
