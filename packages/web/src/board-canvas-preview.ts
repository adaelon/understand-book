import type { BoardItem } from './generated/BoardItem';

// The host knows which items this candidate adds or updates. Retained drawing
// authors and the ordering of imported answers do not identify that scope.
export function candidatePreviewDrawings(items: readonly BoardItem[], candidateItemIds: readonly string[]): BoardItem[] {
  const ids = new Set(candidateItemIds);
  return items.filter(item => item.target.kind === 'drawing' && ids.has(item.item_id));
}

export function candidatePreviewVisuals(items: readonly BoardItem[], candidateItemIds: readonly string[]): BoardItem[] {
  const ids = new Set(candidateItemIds);
  return items.filter(item => (item.target.kind === 'drawing' || item.target.kind === 'topic') && ids.has(item.item_id));
}
