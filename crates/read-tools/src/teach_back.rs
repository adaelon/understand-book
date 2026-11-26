//! Local material for a teach-back. Graph and discourse records locate passages.
//! Only the canonical text returned in `passages` has been read.
use super::{
    invalid_source_range, is_utf16_boundary, push_unique, reading_support::mentions, Book,
    LexicalTextIndex, StructureProjection, TechnicalLearningDiscourseItem, ToolError,
};
use base_schema::{Direction, EdgeScope, GraphNode, GraphNodeType, Span};
use serde::Serialize;
use std::collections::HashSet;

const PASSAGE_UNITS: usize = 1600;
const ANCHOR_UNITS: usize = 6400;
const INITIAL_PASSAGES: usize = 12;
const INITIAL_UNITS: usize = INITIAL_PASSAGES * PASSAGE_UNITS;
const MATCHED_PAGES: usize = 4;
const LOCAL_EDGES: usize = 12;
const TOPIC_TERMS: usize = 6;

#[derive(Debug, Clone, Serialize)]
pub struct TeachBackPassage {
    pub lid: String,
    pub source_range_utf16: Span,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_start_utf16: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TeachBackClaim {
    pub lid: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TeachBackEdge {
    pub source_name: String,
    pub source_lid: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub source_local_occurrences: Vec<String>,
    pub target_name: String,
    pub target_lid: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub target_local_occurrences: Vec<String>,
    pub relation_type: String,
    pub direction: Direction,
}

#[derive(Debug, Clone, Serialize)]
pub struct TeachBackEvidence {
    pub source_id: String,
    pub source_revision: String,
    pub anchor_lids: Vec<String>,
    pub passages: Vec<TeachBackPassage>,
    /// LIDs to which the Reader may grant a supplementary text read.
    pub nearby_lids: Vec<String>,
    pub edges: Vec<TeachBackEdge>,
    pub claims: Vec<TeachBackClaim>,
    pub discourse: Vec<TechnicalLearningDiscourseItem>,
    pub structure: Vec<StructureProjection>,
}

impl TeachBackEvidence {
    pub fn can_read_more(&self, lid: &str) -> bool {
        self.nearby_lids.iter().any(|allowed| allowed == lid)
            && self
                .passages
                .iter()
                .filter(|p| p.lid == lid)
                .min_by_key(|p| p.source_range_utf16.start)
                .map_or(true, |p| p.next_start_utf16.is_some())
    }

    // Join only adjacent canonical ranges. A keyword hit several pages ahead
    // must never make the intervening text quotable.
    fn add_page(&mut self, mut page: TeachBackPassage) {
        if let Some(next) = self.passages.iter().position(|p| {
            p.lid == page.lid && p.source_range_utf16.start == page.source_range_utf16.end
        }) {
            let next = self.passages.remove(next);
            page.text.push_str(&next.text);
            page.source_range_utf16.end = next.source_range_utf16.end;
            page.next_start_utf16 = next.next_start_utf16;
        }
        if let Some(prev) = self.passages.iter_mut().find(|p| {
            p.lid == page.lid && p.source_range_utf16.end == page.source_range_utf16.start
        }) {
            prev.text.push_str(&page.text);
            prev.source_range_utf16.end = page.source_range_utf16.end;
            prev.next_start_utf16 = page.next_start_utf16;
        } else {
            self.passages.push(page);
        }
    }
}

// An entity may have dozens of occurrences. Its graph endpoint has no unique passage
// in that case. Returning the local occurrences preserves this distinction.
fn endpoint(node: &GraphNode, neighborhood: &HashSet<&str>) -> (Option<String>, Vec<String>) {
    if let Some(lid) = &node.source_lid {
        return (Some(lid.clone()), vec![]);
    }
    if let [lid] = node.occurrences.as_slice() {
        return (Some(lid.clone()), vec![]);
    }
    let occurrences = node
        .occurrences
        .iter()
        .filter(|lid| neighborhood.contains(lid.as_str()))
        .cloned()
        .collect();
    (None, occurrences)
}

impl Book {
    /// The only book data admitted to blind extraction are section headings and names.
    /// A concrete question restricts names to those already mentioned in it or its heading.
    pub fn teach_back_topic_terms(
        &self,
        question: Option<&str>,
        anchors: &[String],
    ) -> Result<Vec<String>, ToolError> {
        let mut headings = Vec::new();
        for lid in anchors {
            if let Some(heading) = self.source_heading_path(self.node(lid)?)?.last() {
                push_unique(&mut headings, heading);
            }
        }
        let question = question.unwrap_or_default().trim().to_lowercase();
        let heading_text = headings.join("\n").to_lowercase();
        let mut terms = Vec::new();
        if question.is_empty() {
            terms.extend(
                headings
                    .iter()
                    .filter(|s| s.chars().count() <= 100)
                    .take(2)
                    .cloned(),
            );
        }
        let mut names = Vec::new();
        for node in &self.base.graph_nodes {
            if node.node_type == GraphNodeType::Claim || node.name.chars().count() > 60 {
                continue;
            }
            let at_anchor = node.occurrences.iter().any(|lid| anchors.contains(lid));
            let in_question = mentions(&question, &node.name);
            let in_heading = mentions(&heading_text, &node.name);
            let overview_entity =
                question.is_empty() && at_anchor && node.node_type == GraphNodeType::Entity;
            if in_question || in_heading || overview_entity {
                names.push((!in_question, !at_anchor, !in_heading, node.name.as_str()));
            }
        }
        names.sort_unstable();
        for (_, _, _, name) in names {
            push_unique(&mut terms, name);
            if terms.len() == TOPIC_TERMS {
                break;
            }
        }
        Ok(terms)
    }

    /// Read a bounded canonical range. The returned absolute span is exactly the
    /// text delivered, even for a long section or a supplementary-plane character.
    pub fn teach_back_read_passage(&self, lid: &str) -> Result<TeachBackPassage, ToolError> {
        self.teach_back_read_from(lid, self.node(lid)?.span.start)
    }

    /// Continue a bounded read at the absolute cursor previously returned for this LID.
    pub fn teach_back_read_from(
        &self,
        lid: &str,
        start_utf16: usize,
    ) -> Result<TeachBackPassage, ToolError> {
        self.teach_back_read_range(lid, start_utf16, PASSAGE_UNITS)
    }

    fn teach_back_read_range(
        &self,
        lid: &str,
        start_utf16: usize,
        limit: usize,
    ) -> Result<TeachBackPassage, ToolError> {
        let node = self.node(lid)?;
        let units = self.source_node_u16(node)?;
        let offset = start_utf16.checked_sub(node.span.start)
            .filter(|&i| i <= units.len() && (i < units.len() || units.is_empty()) && is_utf16_boundary(units, i))
            .ok_or_else(|| invalid_source_range("teach-back cursor must be a UTF-16 character boundary inside the requested LID"))?;
        let mut end = (offset + limit).min(units.len());
        if !is_utf16_boundary(units, end) {
            end -= 1;
        }
        let source_range_utf16 = Span {
            start: start_utf16,
            end: node.span.start + end,
        };
        Ok(TeachBackPassage {
            lid: lid.into(),
            text: self.text_in_source_range(&source_range_utf16)?,
            next_start_utf16: (end < units.len()).then_some(source_range_utf16.end),
            source_range_utf16,
        })
    }

    /// Continue the earliest gap, including when a matched page at the end of
    /// a long anchor was already supplied to the model.
    pub fn teach_back_read_more(
        &self,
        evidence: &mut TeachBackEvidence,
        lid: &str,
    ) -> Result<(), ToolError> {
        if !evidence.can_read_more(lid) {
            return Err(invalid_source_range(
                "teach-back LID is outside the neighborhood or fully read",
            ));
        }
        let start = evidence
            .passages
            .iter()
            .filter(|p| p.lid == lid)
            .map(|p| p.source_range_utf16.end)
            .min()
            .unwrap_or(self.node(lid)?.span.start);
        evidence.add_page(self.teach_back_read_from(lid, start)?);
        Ok(())
    }

    fn teach_back_neighbors(&self, lids: &[String], distance: usize) -> Vec<String> {
        let mut result = Vec::new();
        let roots = std::cell::OnceCell::new();
        for lid in lids {
            let Ok(node) = self.node(lid) else { continue };
            if !node.children.is_empty() {
                for child in node.children.iter().take(2 * distance) {
                    push_unique(&mut result, child);
                }
                continue;
            }
            let siblings = if let Some((parent, _)) = lid.rsplit_once('.') {
                let Ok(parent) = self.node(parent) else {
                    continue;
                };
                &parent.children
            } else {
                // Unheaded imports have paragraphs at the root of the LID tree.
                roots.get_or_init(|| {
                    let mut nodes: Vec<_> = self
                        .base
                        .lid_nodes
                        .iter()
                        .filter(|n| n.path.len() == 1)
                        .collect();
                    nodes.sort_by_key(|n| n.span.start);
                    nodes.into_iter().map(|n| n.lid.clone()).collect()
                })
            };
            if let Some(index) = siblings.iter().position(|child| child == lid) {
                // A neighboring chapter/section is a boundary; its first passage can
                // be far away in the source, especially on a backward read.
                let leaf = |lid: &&String| {
                    self.node(lid)
                        .is_ok_and(|n| n.children.is_empty() && !super::is_structural_node(n))
                };
                let mut before = siblings[..index].iter().rev().take_while(leaf);
                let mut after = siblings[index + 1..].iter().take_while(leaf);
                for _ in 0..distance {
                    for sibling in [before.next(), after.next()].into_iter().flatten() {
                        push_unique(&mut result, sibling);
                    }
                }
            }
        }
        result
    }

    fn teach_back_edges(&self, anchors: &[String], nearby: &[String]) -> Vec<TeachBackEdge> {
        let anchored: HashSet<_> = anchors
            .iter()
            .flat_map(|lid| self.nodes_anchored_at(lid))
            .collect();
        let neighborhood: HashSet<_> = nearby.iter().map(String::as_str).collect();
        let mut edges: Vec<_> = self
            .base
            .graph_edges
            .iter()
            .filter(|edge| {
                edge.scope == EdgeScope::Local
                    && (anchored.contains(edge.source.as_str())
                        || anchored.contains(edge.target.as_str()))
            })
            .collect();
        edges.sort_by(|a, b| b.weight.total_cmp(&a.weight));
        edges
            .into_iter()
            .filter_map(|edge| {
                let source = &self.base.graph_nodes[*self.node_idx.get(&edge.source)?];
                let target = &self.base.graph_nodes[*self.node_idx.get(&edge.target)?];
                let (source_lid, source_local_occurrences) = endpoint(source, &neighborhood);
                let (target_lid, target_local_occurrences) = endpoint(target, &neighborhood);
                // Shared concept IDs also have edges in unrelated chapters. Only anchored
                // claims have an unambiguous origin for following a one-hop local edge.
                let anchored_claim = [source, target].iter().any(|n| {
                    n.source_lid
                        .as_ref()
                        .is_some_and(|lid| anchors.contains(lid))
                });
                let reachable = |lid: &Option<String>, occurrences: &[String]| {
                    lid.as_ref().map_or(!occurrences.is_empty(), |lid| {
                        self.source_span(lid).is_some()
                            && (anchored_claim || neighborhood.contains(lid.as_str()))
                    })
                };
                if !reachable(&source_lid, &source_local_occurrences)
                    || !reachable(&target_lid, &target_local_occurrences)
                {
                    return None;
                }
                Some(TeachBackEdge {
                    source_lid,
                    source_local_occurrences,
                    source_name: source.name.clone(),
                    target_lid,
                    target_local_occurrences,
                    target_name: target.name.clone(),
                    relation_type: edge.edge_type.clone(),
                    direction: edge.direction.clone(),
                })
            })
            .take(LOCAL_EDGES)
            .collect()
    }

    pub fn teach_back_evidence(&self, anchors: &[String]) -> Result<TeachBackEvidence, ToolError> {
        self.teach_back_evidence_for(anchors, &[])
    }

    pub fn teach_back_evidence_for(
        &self,
        anchors: &[String],
        student_terms: &[&str],
    ) -> Result<TeachBackEvidence, ToolError> {
        if anchors.is_empty() {
            return Err(invalid_source_range(
                "teach-back requires a verified source anchor",
            ));
        }
        let mut anchor_lids = Vec::new();
        for lid in anchors {
            self.node(lid)?;
            push_unique(&mut anchor_lids, lid);
        }
        let mut nearby_lids = anchor_lids.clone();
        for lid in self.teach_back_neighbors(&anchor_lids, 6) {
            push_unique(&mut nearby_lids, &lid);
        }
        let edges = self.teach_back_edges(&anchor_lids, &nearby_lids);
        let mut read_lids = anchor_lids.clone();
        for edge in &edges {
            for lid in edge.source_lid.iter().chain(&edge.target_lid) {
                push_unique(&mut read_lids, lid);
                push_unique(&mut nearby_lids, lid);
            }
        }
        let linked = read_lids.clone();
        let immediate = self.teach_back_neighbors(&linked, 1);
        for lid in &immediate {
            push_unique(&mut nearby_lids, lid);
        }
        // Keep complete records in their published direction, including incoming
        // rhetorical relations. An item's summary never becomes a quotable passage.
        let discourse: Vec<_> = self
            .discourse_items()
            .iter()
            .filter(|item| {
                linked.contains(&item.lid)
                    || immediate.contains(&item.lid)
                    || (nearby_lids.contains(&item.lid)
                        && item
                            .relations
                            .iter()
                            .any(|r| linked.contains(&r.target_lid)))
            })
            .cloned()
            .collect();
        for item in &discourse {
            if linked.contains(&item.lid) {
                for relation in &item.relations {
                    if nearby_lids.contains(&relation.target_lid) {
                        push_unique(&mut read_lids, &relation.target_lid);
                    }
                }
            }
        }
        for lid in &immediate {
            push_unique(&mut read_lids, lid);
        }
        // Reserve an ordinary page for each remaining anchor before extending
        // the current one. Short anchors release their reservation immediately.
        let initial_anchors = anchor_lids.len().min(INITIAL_PASSAGES);
        let mut reserved = 0;
        for lid in &anchor_lids[..initial_anchors] {
            reserved += self
                .source_node_u16(self.node(lid)?)?
                .len()
                .min(PASSAGE_UNITS);
        }
        let mut remaining = INITIAL_UNITS;
        let mut passages = Vec::new();
        for (index, lid) in read_lids.iter().take(INITIAL_PASSAGES).enumerate() {
            let span = &self.node(lid)?.span;
            let limit = if index < initial_anchors {
                reserved -= (span.end - span.start).min(PASSAGE_UNITS);
                ANCHOR_UNITS.min(remaining - reserved)
            } else {
                PASSAGE_UNITS.min(remaining)
            };
            if limit == 0 {
                break;
            }
            let page = self.teach_back_read_range(lid, span.start, limit)?;
            remaining -= page.source_range_utf16.end - page.source_range_utf16.start;
            passages.push(page);
        }
        let claims = self
            .base
            .graph_nodes
            .iter()
            .filter(|n| n.node_type == GraphNodeType::Claim)
            .filter_map(|n| {
                let lid = n.source_lid.as_ref()?;
                read_lids.contains(lid).then(|| TeachBackClaim {
                    lid: lid.clone(),
                    name: n.name.clone(),
                })
            })
            .collect();
        let mut structure = Vec::new();
        if self.book_structure().is_some() {
            for lid in &anchor_lids {
                let mut local = self.structure(Some(lid))?;
                local
                    .key_stops
                    .retain(|stop| nearby_lids.contains(&stop.lid));
                if !structure.iter().any(|previous: &StructureProjection| {
                    previous.spine_index == local.spine_index
                        && previous.throughlines == local.throughlines
                        && previous.key_stops == local.key_stops
                }) {
                    structure.push(local);
                }
            }
        }
        let mut evidence = TeachBackEvidence {
            source_id: self.base.book_id.clone(),
            source_revision: self.source_fingerprint().into(),
            anchor_lids,
            passages,
            nearby_lids,
            edges,
            claims,
            discourse,
            structure,
        };
        self.read_matching_anchor_pages(&mut evidence, student_terms)?;
        Ok(evidence)
    }

    fn read_matching_anchor_pages(
        &self,
        evidence: &mut TeachBackEvidence,
        student_terms: &[&str],
    ) -> Result<(), ToolError> {
        let mut candidates = Vec::new();
        for anchor in evidence
            .passages
            .iter()
            .filter(|p| evidence.anchor_lids.contains(&p.lid))
        {
            let Some(mut cursor) = anchor.next_start_utf16 else {
                continue;
            };
            let query = student_terms
                .iter()
                .copied()
                .chain(
                    evidence
                        .claims
                        .iter()
                        .filter(|c| c.lid == anchor.lid)
                        .map(|c| c.name.as_str()),
                )
                .chain(
                    evidence
                        .discourse
                        .iter()
                        .filter(|d| d.lid == anchor.lid)
                        .filter_map(|d| d.local_summary.as_deref()),
                )
                .collect::<Vec<_>>()
                .join(" ");
            if query.trim().is_empty() {
                continue;
            }
            let mut unread = Vec::new();
            loop {
                let page = self.teach_back_read_from(&anchor.lid, cursor)?;
                let next = page.next_start_utf16;
                unread.push(page);
                let Some(next) = next else {
                    break;
                };
                cursor = next;
            }
            let index = LexicalTextIndex::new(unread.iter().map(|p| p.text.as_str()));
            for (page, score) in index.search(&query, MATCHED_PAGES, |_| true) {
                candidates.push((score, unread[page].clone()));
            }
        }
        candidates.sort_by(|(a, _), (b, _)| b.total_cmp(a));
        for (_, page) in candidates.into_iter().take(MATCHED_PAGES) {
            evidence.add_page(page);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base_schema::sample_base;

    #[test]
    fn blind_terms_exclude_claims_body_and_unmentioned_entities() {
        let mut base = sample_base();
        let mut entity = base.graph_nodes[0].clone();
        entity.name = "HBM".into();
        entity.id = "entity:hbm".into();
        base.graph_nodes.push(entity);
        let text = format!(
            "# Local resources\nThe secret answer is to change bandwidth.{}",
            "z".repeat(100)
        );
        let book = Book::new(base, &text);
        let anchors = vec!["1.1".into()];
        assert_eq!(
            book.teach_back_topic_terms(Some("What is HBM?"), &anchors)
                .unwrap(),
            ["HBM"]
        );
        let overview = book.teach_back_topic_terms(None, &anchors).unwrap();
        assert!(overview.contains(&"Local resources".into()) && overview.contains(&"HBM".into()));
        assert!(overview
            .iter()
            .all(|term| !term.contains("secret") && term != &book.base.graph_nodes[1].name));
        assert!(book.teach_back_evidence(&[]).is_err());
        assert!(book.teach_back_evidence(&["absent".into()]).is_err());
    }

    #[test]
    fn large_anchor_returns_only_the_utf16_interval_actually_read() {
        let source = format!("{}🦀 end", "x".repeat(ANCHOR_UNITS - 1));
        let mut base = sample_base();
        for node in &mut base.lid_nodes {
            node.span.end = source.encode_utf16().count();
        }
        base.graph_edges.clear();
        base.graph_nodes.clear();
        let book = Book::new(base, &source);
        let projected = book
            .teach_back_evidence(&["1.1".into(), "1.1".into()])
            .unwrap();
        assert!(
            projected.discourse.is_empty()
                && projected.structure.is_empty()
                && projected.edges.is_empty()
        );
        assert_eq!(projected.passages.len(), 1);
        let p = &projected.passages[0];
        assert_eq!(
            p.source_range_utf16,
            Span {
                start: 0,
                end: ANCHOR_UNITS - 1
            }
        );
        assert_eq!(p.text, "x".repeat(ANCHOR_UNITS - 1));
        let tail = book
            .teach_back_read_from(&p.lid, p.next_start_utf16.unwrap())
            .unwrap();
        assert_eq!(tail.text, "🦀 end");
        assert!(tail.next_start_utf16.is_none());
        assert!(serde_json::to_value(&tail)
            .unwrap()
            .get("next_start_utf16")
            .is_none());
        assert!(book.teach_back_read_from(&p.lid, ANCHOR_UNITS).is_err());
        assert!(book
            .teach_back_read_from(&p.lid, tail.source_range_utf16.end)
            .is_err());
        assert_eq!(projected.source_revision, book.source_fingerprint());
    }

    #[test]
    fn long_anchor_reads_matching_pages_from_student_claims_and_discourse_only() {
        let mut source = "a".repeat(ANCHOR_UNITS);
        let page = |text: &str| {
            format!(
                "{}{}",
                text,
                " ".repeat(PASSAGE_UNITS - text.encode_utf16().count())
            )
        };
        for text in [
            "unrelated gap",
            "双方自愿交换",
            "bandwidth",
            "晚期保护结论",
            "unread ending",
        ] {
            source.push_str(&page(text));
        }
        let mut base = sample_base();
        for node in &mut base.lid_nodes {
            node.span.end = source.encode_utf16().count();
        }
        base.graph_nodes[1].name = "bandwidth limits".into();
        let book =
            Book::new(base, &source).with_discourse_items(vec![TechnicalLearningDiscourseItem {
                lid: "1.1".into(),
                mode: "argumentative".into(),
                local_function: None,
                rhetorical_move: None,
                local_summary: Some("主张获得保护".into()),
                relations: vec![],
            }]);
        let mut result = book
            .teach_back_evidence_for(&["1.1".into()], &["自愿交易"])
            .unwrap();
        let read_text = result
            .passages
            .iter()
            .map(|p| p.text.as_str())
            .collect::<String>();
        for clue in ["双方自愿交换", "bandwidth", "晚期保护结论"] {
            assert!(read_text.contains(clue));
        }
        assert!(!read_text.contains("unrelated gap") && !read_text.contains("unread ending"));
        assert_eq!(result.passages.len(), 2);
        book.teach_back_read_more(&mut result, "1.1").unwrap();
        assert_eq!(result.passages.len(), 1);
        assert!(result.passages[0].text.contains("unrelated gap"));
        book.teach_back_read_more(&mut result, "1.1").unwrap();
        assert_eq!(result.passages[0].text, source);
        assert!(!result.can_read_more("1.1"));
    }

    #[test]
    fn anchors_share_the_initial_budget_and_neighbors_stay_at_one_page() {
        let source = "x ".repeat(28_000);
        let mut base = sample_base();
        base.graph_nodes.clear();
        base.graph_edges.clear();
        base.lid_nodes = (0..7)
            .map(|i| base_schema::LidNode {
                lid: (i + 1).to_string(),
                path: vec![(i + 1) as u32],
                kind: base_schema::NodeKind::Paragraph,
                span: Span {
                    start: i * 8000,
                    end: (i + 1) * 8000,
                },
                children: vec![],
            })
            .collect();
        let book = Book::new(base, &source);
        let small = book.teach_back_evidence(&["2".into(), "3".into()]).unwrap();
        assert_eq!(
            small
                .passages
                .iter()
                .map(|p| p.text.len())
                .collect::<Vec<_>>(),
            [6400, 6400, 1600, 1600]
        );
        let crowded = book
            .teach_back_evidence(&(1..=5).map(|i| i.to_string()).collect::<Vec<_>>())
            .unwrap();
        let lengths: Vec<_> = crowded.passages.iter().map(|p| p.text.len()).collect();
        assert_eq!(lengths.len(), 5);
        assert_eq!(lengths.iter().sum::<usize>(), INITIAL_UNITS);
        assert!(lengths.iter().all(|len| *len >= PASSAGE_UNITS));
    }

    #[test]
    #[ignore = "requires local justin-sun-z-library-sk-1lib-sk-z-lib-sk workspace"]
    fn local_book_long_anchor_contains_transaction_protection_in_the_first_comparison() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.understand-book/justin-sun-z-library-sk-1lib-sk-z-lib-sk");
        let book = Book::load(root.to_str().unwrap()).unwrap();
        let evidence = book
            .teach_back_evidence_for(&["40.2".into()], &["自愿交易合法且应被保护"])
            .unwrap();
        let page = &evidence.passages[0];
        assert_eq!(page.lid, "40.2");
        assert_eq!(page.text, book.text("40.2", None).unwrap());
        let (prefix, _) = page.text.split_once("自由交易是需要被。保护的").unwrap();
        assert!(prefix.encode_utf16().count() > PASSAGE_UNITS);
        assert!(page.next_start_utf16.is_none());
        eprintln!(
            "40.2: {} UTF-16 units read, protection statement at +{}",
            page.text.encode_utf16().count(),
            prefix.encode_utf16().count()
        );
    }

    #[test]
    fn unheaded_plain_text_reads_neighboring_root_paragraphs() {
        let mut base = sample_base();
        base.graph_nodes.clear();
        base.graph_edges.clear();
        base.lid_nodes = (1..=5)
            .map(|n| base_schema::LidNode {
                lid: n.to_string(),
                path: vec![n],
                kind: if n == 4 {
                    base_schema::NodeKind::Chapter
                } else {
                    base_schema::NodeKind::Paragraph
                },
                children: vec![],
                span: Span {
                    start: n as usize - 1,
                    end: n as usize,
                },
            })
            .collect();
        let book = Book::new(base, "甲乙丙丁戊");
        let p = book.teach_back_evidence(&["2".into()]).unwrap();
        assert!(!p.nearby_lids.contains(&"4".into()) && !p.nearby_lids.contains(&"5".into()));
        assert_eq!(
            p.passages
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>(),
            ["乙", "甲", "丙"]
        );
    }

    // Re-run against local published books, whose licensed text is outside Git.
    #[test]
    #[ignore = "requires local ai-infra-book-complete and performance-hints workspaces"]
    fn local_books_preserve_directed_claims_discourse_structure_and_exact_text() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.understand-book");
        let open = |name: &str| Book::load(root.join(name).to_str().unwrap()).unwrap();
        for (name, from, to, long_lid) in [
            ("ai-infra-book-complete", "3.6.3.14", "3.6.3.8", "9.14.6.17"),
            ("performance-hints", "1.6.14.5", "1.6.14.3", "1.18.44"),
        ] {
            let book = open(name);
            let first = book.teach_back_read_passage(long_lid).unwrap();
            let tail = book
                .teach_back_read_from(long_lid, first.next_start_utf16.unwrap())
                .unwrap();
            assert_eq!(first.source_range_utf16.end, tail.source_range_utf16.start);
            assert_eq!(
                format!("{}{}", first.text, tail.text),
                book.text(long_lid, None).unwrap()
            );
            assert!(tail.next_start_utf16.is_none());
            let p = book.teach_back_evidence(&[from.into()]).unwrap();
            let e = p
                .edges
                .iter()
                .find(|e| {
                    e.source_lid.as_deref() == Some(from)
                        && e.target_lid.as_deref() == Some(to)
                        && e.relation_type == "builds_on"
                })
                .unwrap();
            assert_eq!(e.direction, Direction::Directed);
            let incoming = book.teach_back_evidence(&[to.into()]).unwrap();
            assert!(incoming
                .edges
                .iter()
                .any(|edge| edge.source_lid.as_deref() == Some(from)
                    && edge.target_lid.as_deref() == Some(to)
                    && edge.direction == Direction::Directed));
            for (lid, name) in [(from, &e.source_name), (to, &e.target_name)] {
                assert!(p.claims.iter().any(|c| c.lid == lid && &c.name == name));
                let passage = p.passages.iter().find(|v| v.lid == lid).unwrap();
                assert_eq!(passage.text, book.text(lid, None).unwrap());
                assert_eq!(&passage.source_range_utf16, book.source_span(lid).unwrap());
            }
            if name == "ai-infra-book-complete" {
                assert!(p.discourse.is_empty() && p.structure.is_empty());
                assert!(p
                    .passages
                    .iter()
                    .any(|p| p.lid == from && p.text.contains("10.45 ms")));
                assert!(p.passages.iter().any(|p| p.lid == "3.6.3.15"));
            } else {
                let item = p.discourse.iter().find(|item| item.lid == from).unwrap();
                assert_eq!(item, book.discourse_item(from).unwrap());
                assert_eq!(item.local_function.as_deref(), Some("summary"));
                assert_eq!(item.rhetorical_move.as_deref(), Some("worked_example"));
                assert!(item
                    .relations
                    .iter()
                    .any(|r| r.target_lid == "1.6.14.4" && r.direction == "backward"));
                assert!(p.structure.iter().any(|s| s
                    .throughlines
                    .iter()
                    .any(|t| t.id == "thread:performance-workflow")));
            }
            eprintln!("{name}: {from} -> {to}, {} canonical passages, {} directed/local edges, {} discourse items, {} structure projections",
                p.passages.len(), p.edges.len(), p.discourse.len(), p.structure.len());
        }
    }
}
