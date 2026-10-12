import { expect, it } from 'vitest';
import { readMcpSource, resolveMcpSource, displayedSourceRange } from './mcp-source';
import type { Manifest } from './api';

const manifest: Manifest = { tree: [
  { lid: '1', children: ['1.1', '1.2'], kind: 'chapter', span: { start: 0, end: 20 } },
  { lid: '1.1', children: [], kind: 'paragraph', span: { start: 0, end: 10 } },
  { lid: '1.2', children: [], kind: 'paragraph', span: { start: 12, end: 20 } },
], stats_by_lid: {} };
const query = '?book_id=书%26名&publication_id=original&start_lid=1&end_lid=1&start_utf16=4&end_utf16=16';
it('retains version and absolute UTF-16 positions across leaves and gaps', () => {
  const source = readMcpSource(query)!;
  expect(source.published_book_ref).toEqual({ book_id: '书&名', publication_id: 'original' });
  const target = resolveMcpSource(source, manifest);
  expect(target.lid).toBe('1.1');
  expect([...target.ranges]).toEqual([['1.1', [4, 10]], ['1.2', [0, 4]]]);
  const text = 'ab😀中文测试';
  expect(text.slice(...target.ranges.get('1.1')!)).toBe('中文测试');
  expect(displayedSourceRange([4, 10], 2, 8)).toEqual([2, 8]);
});
it('starts a continuation at its actual leaf instead of the original LID', () => {
  const source = readMcpSource(query.replace('start_utf16=4', 'start_utf16=14'))!;
  const target = resolveMcpSource(source, manifest);
  expect(target.lid).toBe('1.2'); expect([...target.ranges]).toEqual([['1.2', [2, 4]]]);
});
it('rejects incomplete or invalid links and locators outside the stated LIDs', () => {
  expect(readMcpSource('?account=mcp')).toBeNull();
  for (const value of ['?book_id=x', query.replace('start_utf16=4', 'start_utf16=-1'), query.replace('end_utf16=16', 'end_utf16=2')]) {
    expect(() => readMcpSource(value)).toThrow();
  }
  const source = readMcpSource(query)!;
  expect(() => resolveMcpSource({ ...source, end_utf16: 21 }, manifest)).toThrow('版本不符');
  expect(() => resolveMcpSource({ ...source, start_lid: 'absent' }, manifest)).toThrow('版本不符');
  expect(displayedSourceRange([0, 2], 3, 8)).toBeNull();
});
