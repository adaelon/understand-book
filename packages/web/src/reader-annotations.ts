import type { MemoryRecord } from "./api";

type RecallAnnotations = (query: { book_id: string }) => Promise<MemoryRecord[]>;

/** A displayed card represents every canonical range of one selection. */
export function groupHighlightForDisplay(record: MemoryRecord, members: MemoryRecord[]): MemoryRecord {
  if (members.length < 2) return record;
  const content = members.map((member, index) =>
    (index > 0 && members[index - 1].anchor.lid !== member.anchor.lid ? '\n' : '') + member.content,
  ).join('');
  const ranges = members.flatMap(member => member.anchor.lid && member.range
    ? [{ lid: member.anchor.lid, range: { ...member.range } }] : []);
  return {
    ...record,
    content,
    selection_context: { status: 'resolved', resolution_basis: 'exact', raw_quote: content, resolved_quote: content, ranges },
  };
}

export async function recallBookAnnotations(
  bookId: string,
  recall: RecallAnnotations,
): Promise<MemoryRecord[]> {
  const records = await recall({ book_id: bookId });
  return records.filter((record) => record.book_id === bookId);
}
