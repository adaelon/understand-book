import { expect, test } from 'vitest';
import { focusPageGeometry } from './reader-pagination-budget';

test('two pages require twenty em each at the actual font size; phones stay single', () => {
  expect(focusPageGeometry(791, 19).pagesPerScreen).toBe(1);
  expect(focusPageGeometry(792, 19)).toEqual({ pagesPerScreen: 2, pageWidth: 380, gap: 32 });
  expect(focusPageGeometry(880, 22).pagesPerScreen).toBe(1);
  expect(focusPageGeometry(912, 22).pageWidth).toBe(440);
  expect(focusPageGeometry(844, 19, true).pagesPerScreen).toBe(1);
  expect(focusPageGeometry(288, 22).pageWidth).toBe(288);
});
