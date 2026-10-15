import { expect, it } from 'vitest';
import { sourceChipLabel } from './source-chip.js';

it('distinguishes same-section evidence and keeps repeated references stable', () => {
  const sources = [{ source_ref_id: 'a', label: '正文 · Methods' }, { source_ref_id: 'b', label: '正文 · Methods' }];
  expect(sourceChipLabel(sources, 'a')).toBe('Methods [1]');
  expect(sourceChipLabel(sources, 'b')).toBe('Methods [2]');
  expect(sourceChipLabel(sources, 'a')).toBe('Methods [1]');
});

it('bounds long titles and falls back to the source kind', () => {
  const sources = [{ source_ref_id: 'a', label: '正文 · Chapter One / A very long section heading for evidence' }, { source_ref_id: 'b', label: '正文' }];
  expect(sourceChipLabel(sources, 'a')).toBe('A very long se… [1]');
  expect(sourceChipLabel(sources, 'b')).toBe('正文 [2]');
  expect(sourceChipLabel(sources, 'missing')).toBe('来源不可用');
});
