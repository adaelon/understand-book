//! Previously saved flat rounds are projected on read. Stable focus IDs and the
//! original quotations let disputes and revisions still address the original turn.
use super::*;

#[derive(Deserialize)]
pub(super) struct StoredComparison {
    round_id: String,
    continuation: Option<TeachBackRoundContext>,
    support_delivery_refs: Option<Vec<String>>,
    source: TeachBackSource,
    side_notes: Vec<TeachBackSideNote>,
    follow_up: Option<TeachBackFollowUp>,
    #[serde(flatten)]
    body: StoredGraphs,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredGraphs {
    Current {
        book_graph: TeachBackBookGraph,
        student_graph: TeachBackStudentGraph,
        rows: Vec<TeachBackRow>,
    },
    Flat {
        book_relations: Vec<TeachBackBookRelation>,
        student_relations: Vec<TeachBackStudentRelation>,
        rows: Vec<FlatRow>,
    },
}

#[derive(Deserialize)]
struct FlatRow {
    row_id: String,
    book_relation_ids: Vec<String>,
    student_relation_ids: Vec<String>,
    status: TeachBackStatus,
    summary: String,
    divergence_condition: Option<String>,
}

fn project_book(relations: Vec<TeachBackBookRelation>) -> TeachBackBookGraph {
    let mut graph = TeachBackBookGraph {
        nodes: vec![],
        edges: vec![],
    };
    for r in relations {
        let to = format!("{}:to", r.id);
        let (condition, evidence) = if let Some(label) = r.premise.filter(|p| nonempty(p)) {
            let from = format!("{}:from", r.id);
            graph.nodes.push(TeachBackBookNode {
                id: from.clone(),
                label,
                condition: None,
                evidence: vec![],
            });
            graph.edges.push(TeachBackBookEdge {
                id: r.id,
                from,
                to: to.clone(),
                relation_type: r.relation_type,
                condition: r.condition,
                evidence: r.evidence,
            });
            (None, vec![])
        } else {
            (r.condition, r.evidence)
        };
        graph.nodes.push(TeachBackBookNode {
            id: to,
            label: r.conclusion,
            condition,
            evidence,
        });
    }
    graph
}

fn project_student(relations: Vec<TeachBackStudentRelation>) -> TeachBackStudentGraph {
    let mut graph = TeachBackStudentGraph {
        nodes: vec![],
        edges: vec![],
    };
    for r in relations {
        let to = format!("{}:to", r.id);
        let (condition, quotes) = if let Some(label) = r.premise.filter(|p| nonempty(p)) {
            let from = format!("{}:from", r.id);
            graph.nodes.push(TeachBackStudentNode {
                id: from.clone(),
                label,
                condition: None,
                quotes: vec![],
            });
            graph.edges.push(TeachBackStudentEdge {
                id: r.id,
                from,
                to: to.clone(),
                relation_type: r.relation_type,
                condition: r.condition,
                quotes: r.quotes,
            });
            (None, vec![])
        } else {
            (r.condition, r.quotes)
        };
        graph.nodes.push(TeachBackStudentNode {
            id: to,
            label: r.conclusion,
            condition,
            quotes,
        });
    }
    graph
}

fn project_ids(ids: Vec<String>, has_edge: impl Fn(&str) -> bool) -> (Vec<String>, Vec<String>) {
    let (edges, nodes): (Vec<_>, Vec<_>) = ids.into_iter().partition(|id| has_edge(id));
    (
        edges,
        nodes.into_iter().map(|id| format!("{id}:to")).collect(),
    )
}

impl From<StoredComparison> for TeachBackComparison {
    fn from(value: StoredComparison) -> Self {
        let (book_graph, student_graph, rows) = match value.body {
            StoredGraphs::Current {
                book_graph,
                student_graph,
                rows,
            } => (book_graph, student_graph, rows),
            StoredGraphs::Flat {
                book_relations,
                student_relations,
                rows,
            } => {
                let book = project_book(book_relations);
                let student = project_student(student_relations);
                let rows = rows
                    .into_iter()
                    .map(|r| {
                        let (book_edge_ids, book_node_ids) =
                            project_ids(r.book_relation_ids, |id| {
                                book.edges.iter().any(|e| e.id == id)
                            });
                        let (student_edge_ids, student_node_ids) =
                            project_ids(r.student_relation_ids, |id| {
                                student.edges.iter().any(|e| e.id == id)
                            });
                        TeachBackRow {
                            row_id: r.row_id,
                            book_edge_ids,
                            book_node_ids,
                            student_edge_ids,
                            student_node_ids,
                            status: r.status,
                            summary: r.summary,
                            divergence_condition: r.divergence_condition,
                        }
                    })
                    .collect();
                (book, student, rows)
            }
        };
        Self {
            round_id: value.round_id,
            continuation: value.continuation,
            support_delivery_refs: value.support_delivery_refs,
            source: value.source,
            book_graph,
            student_graph,
            rows,
            side_notes: value.side_notes,
            follow_up: value.follow_up,
        }
    }
}
