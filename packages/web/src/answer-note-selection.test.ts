// @vitest-environment happy-dom
import { expect, it } from 'vitest';
import { renderMarkdown } from './md';
import { answerNoteExcerpt } from './answer-note-selection';
import { answerDisplayParts } from './answer-markdown';

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
  const excerpt = answerNoteExcerpt(range, [source], root);
  expect(excerpt).toBe(source);
});

it('handles a partial formatted span and refuses to stitch separate Markdown parts', () => {
  const source = '前面 __abcdef__ 后面';
  const root = document.createElement('div'); root.innerHTML = renderMarkdown(source);
  const range = document.createRange(); range.setStart(root.querySelector('strong')!.firstChild!, 1); range.setEnd(root.querySelector('strong')!.firstChild!, 4);
  expect(answerNoteExcerpt(range, [source], root)).toBe('bcd');
  root.innerHTML = '<p>first</p><button class="agent-source-button">来源</button><p>second</p>';
  range.selectNodeContents(root);
  expect(answerNoteExcerpt(range, ['first', 'second'], root)).toBeNull();
});

it.each([
  { name: 'plain text into partial bold', startItem: 0, startBold: false, start: 0, endItem: 0, endBold: true, end: 2, expected: '给了第二个名字 → 终点取**那个' },
  { name: 'partial bold into plain text', startItem: 0, startBold: true, start: 2, endItem: 0, endBold: false, end: 4, expected: '名字**对应节点' },
  { name: 'across list items into partial bold', startItem: 0, startBold: false, start: 0, endItem: 1, endBold: true, end: 3, expected: '给了第二个名字 → 终点取**那个名字**对应节点的末尾；\n- 没给 → 终点取**第一个' },
  { name: 'across complete list items', startItem: 0, startBold: false, start: 0, endItem: 1, endBold: false, end: 4, expected: '给了第二个名字 → 终点取**那个名字**对应节点的末尾；\n- 没给 → 终点取**第一个名字自己**的末尾。' },
])('maps $name to the selected original source', ({ startItem, startBold, start, endItem, endBold, end, expected }) => {
  const source = '- 给了第二个名字 → 终点取**那个名字**对应节点的末尾；\n- 没给 → 终点取**第一个名字自己**的末尾。';
  const root = document.createElement('div'); root.innerHTML = renderMarkdown(source);
  const items = root.querySelectorAll('li');
  const range = document.createRange();
  range.setStart(startBold ? items[startItem].querySelector('strong')!.firstChild! : items[startItem].firstChild!, start);
  range.setEnd(endBold ? items[endItem].querySelector('strong')!.firstChild! : items[endItem].lastChild!, end);
  expect(answerNoteExcerpt(range, [source], root)).toBe(expected);
  expect(source).toContain(expected);
});

it('uses the selected occurrence when the same visible words have different Markdown', () => {
  const source = '前面 **同样文字** 后面。\n\n前面 __同样文字__ 后面。';
  const root = document.createElement('div'); root.innerHTML = renderMarkdown(source);
  const paragraph = root.querySelectorAll('p')[1];
  const range = document.createRange();
  range.setStart(paragraph.firstChild!, 0); range.setEnd(paragraph.querySelector('strong')!.firstChild!, 2);
  expect(answerNoteExcerpt(range, [source], root)).toBe('前面 __同样');
});

it('does not borrow matching text from another part to legitimize a cross-part selection', () => {
  const sources = ['first', 'second', 'firstsecond'];
  const root = document.createElement('div'); root.innerHTML = '<p>firstsecondfirstsecond</p>';
  const range = document.createRange(); range.setStart(root.firstChild!.firstChild!, 0); range.setEnd(root.firstChild!.firstChild!, 11);
  expect(answerNoteExcerpt(range, sources, root)).toBeNull();
});

it.each([
  ['前面 __粗体__ 后面', 'strong', '前面 __粗体__'],
  ['前面 **粗体** 后面', 'strong', '前面 **粗体**'],
  ['前面 `代码` 后面', 'code', '前面 `代码`'],
  ['前面 [链接](https://example.com) 后面', 'a', '前面 [链接](https://example.com)'],
])('retains complete formatting at the selection boundary: %s', (source, selector, expected) => {
  const root = document.createElement('div'); root.innerHTML = renderMarkdown(source);
  const range = document.createRange(); range.setStart(root.querySelector('p')!.firstChild!, 0);
  const text = root.querySelector(selector)!.firstChild!; range.setEnd(text, text.textContent!.length);
  expect(answerNoteExcerpt(range, [source], root)).toBe(expected);
});

it('maps text after an inline source chip to its own delivered part', () => {
  const display = answerDisplayParts([
    { kind: 'markdown', text: '第一段。' }, { kind: 'sources', source_ref_ids: ['source'] },
    { kind: 'markdown', text: '\n\n第二段 **加粗文字** 的末尾。' },
  ], []);
  const part = display[0]; if (part.kind !== 'html') throw new Error('Expected Markdown');
  const root = document.createElement('div'); root.innerHTML = part.html;
  const range = document.createRange(); range.setStart(root.querySelectorAll('p')[1].firstChild!, 0);
  range.setEnd(root.querySelector('strong')!.firstChild!, 2);
  expect(answerNoteExcerpt(range, part.markdown, root)).toBe('第二段 **加粗');
});
