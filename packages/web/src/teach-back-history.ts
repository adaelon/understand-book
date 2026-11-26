import type { TeachBackComparison } from './generated/TeachBackComparison';
import type { TeachBackBookRelation } from './generated/TeachBackBookRelation';
import type { TeachBackStudentRelation } from './generated/TeachBackStudentRelation';
import type { TeachBackRow } from './generated/TeachBackRow';

type OldRelation = TeachBackBookRelation | TeachBackStudentRelation;
type OldRow = Omit<TeachBackRow, 'book_node_ids' | 'book_edge_ids' | 'student_node_ids' | 'student_edge_ids'> & {
  book_relation_ids: string[]; student_relation_ids: string[];
};
export type StoredTeachBackComparison = TeachBackComparison | (Omit<TeachBackComparison, 'book_graph' | 'student_graph' | 'rows'> & {
  book_relations: TeachBackBookRelation[];
  student_relations: TeachBackStudentRelation[];
  rows: OldRow[];
});

// Course reviews and stored chat snapshots can carry the original JSON directly.
export function comparisonGraphs(card: StoredTeachBackComparison): TeachBackComparison {
  if ('book_graph' in card) return card;
  function project<R extends OldRelation, Q>(relations: R[], citations: (r?: R) => Q) {
    const nodes = relations.flatMap(r => {
      const conclusion = { id: r.id + ':to', label: r.conclusion, condition: r.premise?.trim() ? undefined : r.condition };
      return r.premise?.trim()
        ? [{ id: r.id + ':from', label: r.premise!, ...citations() }, { ...conclusion, ...citations() }]
        : [{ ...conclusion, ...citations(r) }];
    });
    const edges = relations.filter(r => r.premise?.trim()).map(r => ({
      id: r.id, from: r.id + ':from', to: r.id + ':to', condition: r.condition,
      relation_type: r.relation_type, ...citations(r),
    }));
    return { nodes, edges };
  }
  const book_graph = project(card.book_relations, r => ({ evidence: r?.evidence ?? [] }));
  const student_graph = project(card.student_relations, r => ({ quotes: r?.quotes ?? [] }));
  const rows = card.rows.map(({ book_relation_ids, student_relation_ids, ...row }) => {
    const members = (ids: string[], edges: {id: string}[]) => {
      const edge = (id: string) => edges.some(e => e.id === id);
      return { edges: ids.filter(edge), nodes: ids.filter(id => !edge(id)).map(id => id + ':to') };
    };
    const book = members(book_relation_ids, book_graph.edges);
    const student = members(student_relation_ids, student_graph.edges);
    return { ...row, book_edge_ids: book.edges, book_node_ids: book.nodes, student_edge_ids: student.edges, student_node_ids: student.nodes };
  });
  return { ...card, book_graph, student_graph, rows };
}
