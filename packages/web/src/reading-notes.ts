import type { MemoryRecord } from './api';

export type NoteFilter = 'all' | 'original' | 'answer' | 'presentation' | 'highlight' | 'unknown';
export function noteKind(record: MemoryRecord): Exclude<NoteFilter, 'all'> {
  if (record.type === 'highlight') return 'highlight';
  const kind = record.note?.association.kind;
  if (kind === 'answer' || kind === 'presentation') return kind;
  if (record.selection_context || record.note_placement || kind === 'selection' || kind === 'body_placement') return 'original';
  return 'unknown';
}
export function noteKindLabel(record: MemoryRecord): string {
  return { original: '原文笔记', answer: '回答笔记', presentation: '演示笔记', highlight: '高亮', unknown: '阅读笔记' }[noteKind(record)];
}
export function noteTimestamp(record: MemoryRecord): number | null {
  const value = record.generated_at;
  if (!value) return null;
  // Native hosts persist milliseconds; older records may contain Unix seconds.
  const time = /^\d+$/.test(value) ? Number(value) * (value.length <= 10 ? 1000 : 1) : Date.parse(value);
  return Number.isFinite(time) && !Number.isNaN(new Date(time).getTime()) ? time : null;
}
export function noteTimeLabel(record: MemoryRecord): string {
  const time = noteTimestamp(record);
  return time === null ? '保存时间未知' : new Date(time).toLocaleString('zh-CN', { hour12: false });
}
export function notePreview(record: MemoryRecord): string {
  const text = (record.content.trim() || record.note?.retained_excerpt?.text || '').replace(/\s+/g, ' ');
  return text.length > 160 ? `${text.slice(0, 160)}…` : text;
}
export function findNotes(records: MemoryRecord[], query: string, filter: NoteFilter,
  order: 'recent' | 'original', bookOrder: string[], title: (record: MemoryRecord) => string): MemoryRecord[] {
  const needle = query.trim().toLocaleLowerCase();
  const ranks = new Map(bookOrder.map((id, index) => [id, index]));
  return records.filter(record => (filter === 'all' || noteKind(record) === filter)
    && (!needle || [record.content, record.note?.retained_excerpt?.text ?? '', title(record),
      ...(record.note?.source_bindings ?? []).map(source => source.label_snapshot)]
      .some(text => text.toLocaleLowerCase().includes(needle))))
    .sort((a, b) => order === 'recent'
      ? (noteTimestamp(b) ?? -Infinity) - (noteTimestamp(a) ?? -Infinity) || 0
      : (ranks.get(a.mem_id) ?? Infinity) - (ranks.get(b.mem_id) ?? Infinity) || 0);
}
