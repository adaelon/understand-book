import type { PublishedBookRef } from './reader-context';

export interface BoardRoute {
  boardId: string;
  publication: PublishedBookRef | null;
  learningWorkspaceId: string | null;
  itemId?: string;
  blockId?: string;
}
const keys = ['board_id', 'board_book_id', 'board_publication_id', 'board_learning_workspace_id', 'board_item_id', 'board_block_id'];

export function readBoardRoute(search: string): BoardRoute | null {
  const params = new URLSearchParams(search), boardId = params.get('board_id');
  if (!boardId) return null;
  const book = params.get('board_book_id'), publication = params.get('board_publication_id');
  return { boardId, publication: book && publication ? { book_id: book, publication_id: publication } : null,
    learningWorkspaceId: params.get('board_learning_workspace_id') || null,
    ...(params.get('board_item_id') ? { itemId: params.get('board_item_id')! } : {}),
    ...(params.get('board_block_id') ? { blockId: params.get('board_block_id')! } : {}) };
}

export function boardRouteUrl(href: string, route: BoardRoute | null): URL {
  const url = new URL(href);
  keys.forEach(key => url.searchParams.delete(key));
  if (route) {
    url.searchParams.set('board_id', route.boardId);
    if (route.publication) {
      url.searchParams.set('board_book_id', route.publication.book_id);
      url.searchParams.set('board_publication_id', route.publication.publication_id);
    }
    if (route.learningWorkspaceId) url.searchParams.set('board_learning_workspace_id', route.learningWorkspaceId);
    if (route.itemId) url.searchParams.set('board_item_id', route.itemId);
    if (route.blockId) url.searchParams.set('board_block_id', route.blockId);
  }
  return url;
}
