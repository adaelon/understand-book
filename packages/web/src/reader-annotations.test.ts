import { describe, expect, it, vi } from "vitest";
import type { MemoryRecord } from "./api";
import { groupHighlightForDisplay, recallBookAnnotations } from "./reader-annotations";

function annotation(memId: string, bookId: string, type: "note" | "highlight"): MemoryRecord {
  return {
    mem_id: memId,
    type,
    layer: "long_term",
    book_id: bookId,
    anchor: { lid: "1.1", concept: null },
    content: memId,
  };
}

describe("reader annotation scope", () => {
  it('retains all subscript selection ranges in the displayed card and its source reference', () => {
    const source = 'C<sub>c</sub>、μ<sub>c</sub>';
    const ranges = [{ start: 0, end: 1 }, { start: 6, end: 7 }, { start: 13, end: 15 }, { start: 20, end: 21 }];
    const members = ranges.map((range, index) => ({
      ...annotation(`part-${index}`, 'book-a', 'highlight'), range,
      content: source.slice(range.start, range.end), source_session_id: 'highlight-group:one',
    }));
    const card = groupHighlightForDisplay(members[0], members);
    expect(card.content).toBe('Cc、μc');
    expect(card.selection_context?.resolved_quote).toBe('Cc、μc');
    expect(card.selection_context?.ranges).toEqual(ranges.map(range => ({ lid: '1.1', range })));
    expect(card.mem_id).toBe(members[0].mem_id);
    expect(members[0].content).toBe('C');
    const next = { ...annotation('next', 'book-a', 'highlight'), anchor: { lid: '1.2' }, range: { start: 0, end: 4 }, content: '后文解释' };
    expect(groupHighlightForDisplay(members[0], [...members, next]).content).toBe('Cc、μc\n后文解释');
  });

  it("requests and retains annotations for the current book only", async () => {
    const recall = vi.fn(async () => [
      annotation("note-a", "book-a", "note"),
      annotation("note-b", "book-b", "note"),
      annotation("highlight-a", "book-a", "highlight"),
    ]);

    const records = await recallBookAnnotations("book-a", recall);

    expect(recall).toHaveBeenCalledWith({ book_id: "book-a" });
    expect(records.map((record) => record.mem_id)).toEqual(["note-a", "highlight-a"]);
  });
});
