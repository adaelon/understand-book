import { describe, expect, it } from 'vitest';
import type { BoardItem } from './generated/BoardItem';
import { candidatePreviewDrawings } from './board-canvas-preview';

function drawing(id: string, turn = 'old-turn'): BoardItem {
  return { item_id: id, title: id, readable_content: id, explanation_blocks: [],
    position: { x: 0, y: 0 }, size: { width: 240, height: 100 }, created_at: '2026-10-27', source_bindings: [],
    target: { kind: 'drawing', graphic: { kind: 'text', text: id }, based_on: [], session_id: 'session', turn_id: turn } };
}
function answer(id: string, turn: string): BoardItem {
  return { ...drawing(id, turn), target: { kind: 'answer', session_id: 'session', turn_id: turn, question: id, answer_parts: [] } };
}

describe('native candidate preview scope', () => {
  it('checks new drawings without including untouched drawings from the same author', () => {
    const retained = drawing('retained', 'current-turn'), added = drawing('added', 'current-turn');
    expect(candidatePreviewDrawings([retained, answer('new-answer', 'current-turn'), added], ['new-answer', 'added']))
      .toEqual([added]);
  });

  it('checks an updated existing drawing even though its retained author belongs to an older turn', () => {
    const updated = drawing('updated-old-drawing'), untouched = drawing('untouched-old-drawing');
    expect(candidatePreviewDrawings([updated, untouched, answer('correction', 'current-turn')], ['correction', 'updated-old-drawing']))
      .toEqual([updated]);
  });

  it('keeps new drawings in scope when a historical imported answer is appended last', () => {
    const added = drawing('new-diagram', 'current-turn');
    const items = [answer('new-answer', 'current-turn'), added, answer('imported-answer', 'historical-turn')];
    expect(candidatePreviewDrawings(items, ['new-answer', 'new-diagram', 'imported-answer']))
      .toEqual([added]);
  });

  it('does not inspect unrelated retained drawings when the candidate has no drawing changes', () => {
    expect(candidatePreviewDrawings([drawing('retained'), answer('new-answer', 'current-turn')], ['new-answer']))
      .toEqual([]);
  });
});
