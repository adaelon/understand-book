import { describe, expect, it } from 'vitest';
import { readBoardRoute, boardRouteUrl } from './board-route';
import { readMcpSource } from './mcp-source';

describe('board direct addresses', () => {
  it('keeps the exact material and learning workspace without creating an incomplete source link', () => {
    const route = { boardId: 'board-1', publication: { book_id: 'book-A', publication_id: 'release-2' }, learningWorkspaceId: 'learning-3' };
    const url = boardRouteUrl('https://reader.example/?display=large', route);
    expect(readBoardRoute(url.search)).toEqual(route);
    expect(readMcpSource(url.search)).toBeNull();
    expect(url.searchParams.get('display')).toBe('large');
    const closed = boardRouteUrl(url.href, null);
    expect(readBoardRoute(closed.search)).toBeNull();
    expect(closed.searchParams.get('display')).toBe('large');
  });
  it('reopens the exact answer block and removes its address when closing the board', () => {
    const route = { boardId: 'board-1', publication: null, learningWorkspaceId: null, itemId: 'answer-2', blockId: 'block-answer-2' };
    const url = boardRouteUrl('https://reader.example/?source=kept#reading', route);
    expect(readBoardRoute(url.search)).toEqual(route);
    const closed = boardRouteUrl(url.href, null);
    expect(closed.searchParams.get('board_item_id')).toBeNull();
    expect(closed.searchParams.get('board_block_id')).toBeNull();
    expect(closed.searchParams.get('source')).toBe('kept');
    expect(closed.hash).toBe('#reading');
  });
});
