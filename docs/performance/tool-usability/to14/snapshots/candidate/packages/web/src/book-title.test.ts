import { expect, it } from 'vitest';
import { bookTitle } from './book-title';

it('uses actual titles without changing punctuation, then readable folder names and IDs', () => {
  expect(bookTitle({ title: '  A/B — 研究标题  ', name: 'slug', book_id: 'id' })).toBe('A/B — 研究标题');
  expect(bookTitle({ title: ' ', name: 'memory-layout', book_id: 'id' })).toBe('memory layout');
  expect(bookTitle({ book_id: 'attention_is-all' })).toBe('attention is all');
});
