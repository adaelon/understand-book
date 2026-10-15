import { describe, expect, it } from 'vitest';
import type { MemoryRecord } from './api';
import { findNotes, noteKind, notePreview, noteTimeLabel, noteTimestamp } from './reading-notes';

describe('RN4 current-material projection', () => {
  const legacy: MemoryRecord = { mem_id: 'old', type: 'note', layer: 'long_term', book_id: 'book', anchor: {}, content: '> 不推断作者' };
  it('reads native host millisecond timestamps and sorts them with seconds and ISO dates', () => {
    const native = { ...legacy, mem_id: 'native', generated_at: '1791590400000' };
    const records = [native, { ...legacy, mem_id: 'seconds', generated_at: '1791590401' },
      { ...legacy, mem_id: 'iso', generated_at: '2026-10-10T00:00:02Z' }];
    expect(noteTimestamp(native)).toBe(Date.parse('2026-10-10T00:00:00Z'));
    expect(noteTimeLabel(native)).toContain('2026');
    expect(findNotes(records, '', 'all', 'recent', [], () => '').map(n => n.mem_id)).toEqual(['iso', 'seconds', 'native']);
  });
  it('keeps missing times and associations unknown and sorts numeric seconds with ISO dates', () => {
    const records = [legacy, { ...legacy, mem_id: 'seconds', generated_at: '1700000000' },
      { ...legacy, mem_id: 'iso', generated_at: '2025-01-01T00:00:00Z' }, { ...legacy, mem_id: 'invalid', generated_at: 'bad' }];
    expect(noteKind(legacy)).toBe('unknown'); expect(noteTimeLabel(legacy)).toBe('保存时间未知');
    expect(noteTimestamp(records[1])).toBe(1700000000000);
    expect(findNotes(records, '', 'all', 'recent', [], () => '').map(n => n.mem_id)).toEqual(['iso', 'seconds', 'old', 'invalid']);
    expect(notePreview({ ...legacy, content: '长文\n'.repeat(200) }).length).toBeLessThanOrEqual(161);
  });
  it('keeps identical text from distinct presentation scenes and searches binding labels and retained excerpts', () => {
    const scene = (id: string): MemoryRecord => ({ ...legacy, mem_id: id, content: '同一句想法', note: {
      material: { book_id: 'book', publication_id: null }, retained_excerpt: null,
      association: { kind: 'presentation', title: '梯度演示', receipt: { session_id: 'deleted', turn_id: 't', reference: { presentation_id: 'p', revision: 2 }, state_revision: 1, saved_state_ref: id } }, source_bindings: [],
    } });
    const records = [scene('state-a'), scene('state-b')];
    expect(findNotes(records, '同一句', 'presentation', 'recent', [], () => '梯度演示')).toHaveLength(2);
    expect(findNotes(records, '梯度', 'presentation', 'recent', [], () => '梯度演示')).toHaveLength(2);
    expect(findNotes(records, '', 'original', 'recent', [], () => '')).toEqual([]);
  });
});
