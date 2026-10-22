import { expect, it } from 'vitest';
import { bookTitle } from './book-title';

it('uses actual titles without changing punctuation, then readable folder names and IDs', () => {
  expect(bookTitle({ title: '  A/B — 研究标题  ', name: 'slug', book_id: 'id' })).toBe('A/B — 研究标题');
  expect(bookTitle({ title: ' ', name: 'memory-layout', book_id: 'id' })).toBe('memory layout');
  expect(bookTitle({ book_id: 'attention_is-all' })).toBe('attention is all');
});

it('keeps opaque publication identities out of reader-facing titles', () => {
  const book_id = '19ac880d-a788-43f4-9725-6c56f4980189';
  expect(bookTitle({ book_id })).toBe('未命名资料');
  expect(bookTitle({ book_id, title: 'CS336 第 2 讲 · PyTorch' })).toBe('CS336 第 2 讲 · PyTorch');
  expect(bookTitle({ book_id: 'f'.repeat(64) })).toBe('未命名资料');
});
