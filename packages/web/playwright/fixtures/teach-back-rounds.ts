import { type Page } from '@playwright/test';
import type { TeachBackComparison } from '../../src/generated/TeachBackComparison';
import { setup, scenario } from './tutor-continuation';

export function averageSpeedComparison(turnId: string): TeachBackComparison {
  return {
    round_id: 'tb:' + turnId + ':r1', source: { source_id: scenario.book_id, source_revision: 'tc0-source' },
    book_graph: {
      nodes: [
        { id: 'bn1', label: 'Total distance', evidence: [] },
        { id: 'bn2', label: 'Divide by elapsed time', evidence: [] },
        { id: 'bn3', label: 'Count the stop', evidence: [] },
      ],
      edges: [
        { id: 'b1', from: 'bn1', to: 'bn2', relation_type: 'defines', evidence: [{ lid: '1.1', quote: scenario.quote }] },
        { id: 'b2', from: 'bn2', to: 'bn3', condition: 'When a trip includes a stop', relation_type: 'requires', evidence: [{ lid: '1.1', quote: 'total time' }] },
      ],
    },
    student_graph: {
      nodes: [
        { id: 'sn1', label: 'Distance travelled', quotes: [] },
        { id: 'sn2', label: 'Divide by trip time', quotes: [] },
        { id: 'sn3', label: 'Exclude stopped time', quotes: [] },
      ],
      edges: [
        { id: 's1', from: 'sn1', to: 'sn2', relation_type: 'defines', quotes: [{ turn_id: turnId, text: 'I divide total distance by total time.' }] },
        { id: 's2', from: 'sn2', to: 'sn3', relation_type: 'concludes', quotes: [{ turn_id: turnId, text: 'Stopped time can be removed.' }] },
      ],
    },
    rows: [
      { row_id: 'r1', book_node_ids: [], student_node_ids: [], book_edge_ids: ['b1'], student_edge_ids: ['s1'], status: 'consistent', summary: 'Both use distance divided by time.' },
      { row_id: 'r2', book_node_ids: [], student_node_ids: [], book_edge_ids: ['b2'], student_edge_ids: ['s2'], status: 'divergent', summary: 'The denominators differ for a stop.', divergence_condition: 'A trip has a nonzero stop.' },
    ],
    side_notes: [{ label: 'Another consideration', text: 'The denominator must be positive.', evidence: [{ lid: '1.1', quote: 'The total time must be positive.' }] }],
  };
}

export function setClarifiedStop(round: TeachBackComparison, turnId: string, quote: string, dispute = false) {
  round.book_graph.nodes = round.book_graph.nodes.filter(n => n.id !== 'bn1');
  round.book_graph.edges = round.book_graph.edges.filter(e => e.id === 'b2');
  round.student_graph.nodes = [{ id: 'sn3', label: 'Count the whole stop', quotes: [{ turn_id: turnId, text: quote }] }];
  round.student_graph.edges = [];
  round.rows = [{ row_id: 'clarified-stop', book_edge_ids: ['b2'], book_node_ids: [],
    student_edge_ids: [], student_node_ids: ['sn3'], status: dispute ? 'consistent' : 'clarified',
    summary: 'Both include the entire stop in elapsed time.' }];
}

export const unrelatedQuestion = 'How do I bookmark the next chapter?';

/** Saved history + Reader HTTP responses. The browser owns selection, display, dispatch and replay. */
export async function setupComparisonHistory(page: Page, misinterpreted = false) {
  const fixture = await setup(page, 'markdown', true, true);
  const origin = 'tc0-answer';
  const firstId = 'tc2-turn-1';
  const first = averageSpeedComparison(firstId);
  first.support_delivery_refs = ['usage:tc0:assistant:tc0-answer'];
  const oldQuote = misinterpreted ? 'The wheels stop, but my clock keeps ticking.' : 'Stopped time can be removed.';
  first.student_graph.edges[1].quotes[0].text = oldQuote;
  first.follow_up = { row_id: 'r2', question: 'Does elapsed time include the stop?' };
  fixture.turns[0].can_explain_back = true;
  const originalOutcome = fixture.turns[0].outcome as object;
  function appendComparison(id: string, message: string, comparison: TeachBackComparison, operationId: string,
    expressionId: string, reply?: object) {
    fixture.turns.push({ turn_id: id, user_turn_ordinal: fixture.turns.length + 1,
      user: message, status: 'completed', effect_labels: [],
      domain: { interaction: { answer_ref: { turn_id: origin }, teach_back_reply: reply },
        effects: [], teach_back_attempts: [{ operation_id: operationId, expression_id: expressionId }] },
      outcome: { ...originalOutcome, answer: '', answer_view: { parts: [{ kind: 'teach_back_comparison', comparison }], sources: [] } },
    });
  }
  appendComparison(firstId, 'I divide total distance by total time. ' + oldQuote, first, 'first-op', 'first-exp');
  await page.route('**/api/tutor/display', route => {
    fixture.displayed.push(route.request().postDataJSON());
    return route.fulfill({ json: { event_id: 'displayed' } });
  });
  await page.route('**/api/agent/runs', async route => {
    const body = route.request().postDataJSON();
    const id = 'tc2-turn-' + fixture.turns.length;
    fixture.requests.push(body);
    if (body.teach_back_reply && body.message !== unrelatedQuestion) {
      const reply = body.teach_back_reply;
      const dispute = reply.kind === 'dispute_interpretation';
      const next = averageSpeedComparison(id);
      next.continuation = { kind: reply.kind, parent_round_id: reply.round_id,
        first_round_id: first.round_id, row_id: reply.row_id };
      const displayedRounds = fixture.displayed.flatMap(receipt => typeof receipt.round_id === 'string' ? [receipt.round_id] : []);
      next.support_delivery_refs = [...first.support_delivery_refs!];
      if (!dispute) next.support_delivery_refs.push(...new Set(displayedRounds));
      setClarifiedStop(next, dispute ? firstId : id, dispute ? oldQuote : body.message, dispute);
      appendComparison(id, body.message, next, body.operation_id, body.expression_id, reply);
    } else {
      const text = 'Open the chapter menu to bookmark a passage.';
      fixture.turns.push({ turn_id: id, user_turn_ordinal: fixture.turns.length + 1, user: body.message,
        status: 'completed', effect_labels: [], outcome: { ...originalOutcome, answer: text,
          answer_view: { parts: [{ kind: 'markdown', text }], sources: [] } } });
    }
    await route.fulfill({ json: { book_id: scenario.book_id, session_id: 'tc0-chat', turn_id: id } });
  });
  return { ...fixture, first, firstId, origin, oldQuote };
}
