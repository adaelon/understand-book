//! Source locators for one reading focus and one adjacent teaching connection.
//! These describe published material, never a learner's knowledge or observed text.
use super::{Book, ConceptCandidateSet, GraphNodeType, RankedSearchResult, ToolError};
use base_schema::{GraphNode, Span};
use book_tool_contracts::{ConceptInput, ReadingFocus, SearchInput};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct ReadingPassage {
    pub lid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub end_lid: Option<String>,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct ReadingConnection {
    pub from: String,
    pub relation: String,
    pub to: String,
    pub direction: String,
    pub passage: ReadingPassage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub example: Option<ReadingPassage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct ReadingSupport {
    pub topics: Vec<String>,
    pub passage: ReadingPassage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub connection: Option<ReadingConnection>,
}

fn locations(node: &GraphNode) -> impl Iterator<Item = &str> {
    node.occurrences
        .iter()
        .chain(node.source_lid.iter())
        .map(String::as_str)
}

fn mentions(query: &str, label: &str) -> bool {
    let label = label.trim().to_lowercase();
    if label.is_empty() || label.chars().count() == 1 {
        return !label.is_empty() && query == label;
    }
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    query.match_indices(&label).any(|(at, _)| {
        let left_join = label.starts_with(word) && query[..at].ends_with(word);
        let right_join = label.ends_with(word) && query[at + label.len()..].starts_with(word);
        !left_join && !right_join
    })
}

fn matches_focus(focus: ReadingFocus, role: &str) -> bool {
    match focus {
        ReadingFocus::Definition => role == "definition",
        ReadingFocus::Procedure => matches!(role, "procedure_step" | "method_description"),
        ReadingFocus::Explanation => {
            matches!(role, "explanation" | "effect" | "result_interpretation")
        }
        ReadingFocus::Example => matches!(role, "example" | "application"),
    }
}

impl Book {
    /// Turn a whole-section address into a leaf within the page actually delivered.
    /// The previous teaching focus can locate that leaf even for a reply like "continue".
    pub fn reading_leaf(
        &self,
        query: &str,
        lid: &str,
        end_lid: Option<&str>,
        page: Option<&Span>,
    ) -> Option<String> {
        let first = self.node(lid).ok()?;
        let last = self.node(end_lid.unwrap_or(lid)).ok()?;
        if first.children.is_empty() && first.lid == last.lid {
            return Some(first.lid.clone());
        }
        let start = page.map_or(first.span.start, |p| p.start.max(first.span.start));
        let end = page.map_or(last.span.end, |p| p.end.min(last.span.end));
        if start >= end {
            return None;
        }
        let scope = (first.span.end >= end).then_some(lid);
        let result = self.search_ranked(query, scope, 8).ok()?;
        result
            .hits
            .into_iter()
            .find(|h| h.source_range_utf16.start < end && h.source_range_utf16.end > start)
            .map(|h| h.start_lid)
    }

    pub fn local_reading_section<'a>(&self, lid: &'a str, maximum_units: usize) -> &'a str {
        let Some((parent, _)) = lid.rsplit_once('.') else {
            return lid;
        };
        if self
            .source_span(parent)
            .is_some_and(|s| s.end - s.start <= maximum_units)
        {
            parent
        } else {
            lid
        }
    }

    pub fn search_for_reading(&self, input: &SearchInput) -> Result<RankedSearchResult, ToolError> {
        let mut result = self.search_ranked(
            &input.query,
            input.within_lid.as_deref(),
            input
                .limit
                .unwrap_or(book_tool_contracts::DEFAULT_SEARCH_LIMIT),
        )?;
        if let Some(focus) = input.focus {
            result.reading = self.reading_support(
                &input.query,
                result.hits.first().map(|h| h.start_lid.as_str()),
                input.within_lid.as_deref(),
                Some(focus),
            );
        }
        Ok(result)
    }

    pub fn concept_for_reading(
        &self,
        input: &ConceptInput,
    ) -> Result<ConceptCandidateSet, ToolError> {
        let mut result =
            self.concept_candidates(&input.query, input.anchor_lid.as_deref(), input.limit)?;
        if let Some(focus) = input.focus {
            // Only the caller can disambiguate competing concept candidates.
            let anchor = result
                .candidates
                .first()
                .filter(|_| result.matched_count == 1)
                .and_then(|c| c.occurrences.first())
                .map(String::as_str);
            result.reading = self.reading_support(&input.query, anchor, None, Some(focus));
        }
        Ok(result)
    }

    pub fn reading_support(
        &self,
        query: &str,
        anchor_lid: Option<&str>,
        within_lid: Option<&str>,
        focus: Option<ReadingFocus>,
    ) -> Option<ReadingSupport> {
        if self.experimental_read_access().is_some()
            || (self.base.graph_nodes.is_empty() && self.discourse_items().is_empty())
        {
            return None;
        }
        let span = |lid: &str| self.source_span(lid);
        let scope = match within_lid {
            Some(lid) => Some(span(lid)?),
            None => None,
        };
        let contains =
            |outer: &Span, inner: &Span| outer.start <= inner.start && inner.end <= outer.end;
        let allowed =
            |lid: &str| span(lid).is_some_and(|s| scope.is_none_or(|outer| contains(outer, s)));
        let anchor = anchor_lid.filter(|lid| allowed(lid)).and_then(span);
        let at_anchor = |lid: &str| {
            anchor_lid == Some(lid)
                || anchor
                    .zip(span(lid))
                    .is_some_and(|(a, s)| a.end - a.start <= 2048 && contains(a, s))
        };
        let distance = |lid: &str| {
            anchor
                .zip(span(lid))
                .map_or(usize::MAX, |(a, s)| a.start.abs_diff(s.start))
        };
        let query = query.trim().to_lowercase();
        let mut named = Vec::new();
        let mut local = Vec::new();
        for node in &self.base.graph_nodes {
            if node.node_type == GraphNodeType::Claim || !locations(node).any(&allowed) {
                continue;
            }
            if mentions(&query, &node.name) {
                named.push(node);
            } else if locations(node).any(&at_anchor) {
                local.push(node);
            }
        }
        let mut selected = if named.is_empty() {
            local
        } else {
            let mut counts = HashMap::new();
            for n in &named {
                *counts.entry(n.name.to_lowercase()).or_insert(0) += 1;
            }
            named.retain(|n| counts[&n.name.to_lowercase()] == 1 || locations(n).any(&at_anchor));
            named
        };
        selected.sort_by_key(|n| {
            (
                !locations(n).any(&at_anchor),
                n.node_type != GraphNodeType::Concept,
            )
        });
        selected.truncate(2);
        let roles: HashMap<_, _> = self
            .discourse_items()
            .iter()
            .map(|i| (i.lid.as_str(), i))
            .collect();
        let role = |lid: &str| {
            roles
                .get(lid)
                .and_then(|i| i.local_function.as_deref())
                .unwrap_or("occurrence")
        };
        let mut seeds: Vec<&str> = selected
            .iter()
            .flat_map(|n| locations(n))
            .filter(|lid| allowed(lid))
            .collect();
        if seeds.is_empty() {
            let lid = anchor_lid.filter(|lid| allowed(lid) && roles.contains_key(lid))?;
            seeds.push(lid);
        }
        let seed_set: HashSet<&str> = seeds.iter().copied().collect();
        let neighbors: Vec<_> = self
            .discourse_items()
            .iter()
            .filter(|i| allowed(&i.lid))
            .flat_map(|i| {
                i.relations
                    .iter()
                    .filter(|r| {
                        self.discourse_relation_valid(r)
                            && allowed(&r.target_lid)
                            && r.evidence_lids.iter().all(|lid| allowed(lid))
                    })
                    .map(move |r| (i.lid.as_str(), r))
            })
            .filter(|(from, r)| seed_set.contains(from) || seed_set.contains(r.target_lid.as_str()))
            .collect();
        let mut choices = seeds.clone();
        if let Some(wanted) = focus {
            for &(from, r) in &neighbors {
                for lid in [from, r.target_lid.as_str()] {
                    if matches_focus(wanted, role(lid)) {
                        choices.push(lid);
                    }
                }
            }
        }
        let main = choices.into_iter().min_by_key(|lid| {
            (
                focus.is_some_and(|f| !matches_focus(f, role(lid))),
                !seed_set.contains(lid),
                !at_anchor(lid),
                distance(lid),
                self.lid_idx.get(*lid).copied(),
            )
        })?;
        let passage = |lid: &str| ReadingPassage {
            lid: lid.to_owned(),
            end_lid: None,
            role: role(lid).to_owned(),
        };
        let mut primary = passage(main);
        // Keep a short numbered procedure together, including an initial step
        // that has no graph occurrence of its own.
        if role(main) == "procedure_step" && focus == Some(ReadingFocus::Procedure) {
            if let Some(parent) = main.rsplit_once('.').and_then(|(p, _)| self.node(p).ok()) {
                if let Some(mid) = parent.children.iter().position(|l| l == main) {
                    let same_step = |i: usize| {
                        allowed(&parent.children[i])
                            && role(&parent.children[i]) == "procedure_step"
                    };
                    let mut start = mid;
                    while start > 0 && mid - start < 4 && same_step(start - 1) {
                        start -= 1;
                    }
                    let mut end = mid;
                    while end + 1 < parent.children.len() && end - start < 5 && same_step(end + 1) {
                        end += 1;
                    }
                    primary.lid = parent.children[start].clone();
                    if end != start {
                        primary.end_lid = Some(parent.children[end].clone());
                    }
                }
            }
        }
        let nearby = |lid: &str| {
            span(lid)
                .zip(span(main))
                .map_or(usize::MAX, |(a, b)| a.start.abs_diff(b.start))
        };
        let mut graph = Vec::new();
        for edge in &self.base.graph_edges {
            let src = selected.iter().any(|n| n.id == edge.source);
            let dst = selected.iter().any(|n| n.id == edge.target);
            if src == dst {
                continue;
            }
            let (Some(&s), Some(&t)) = (
                self.node_idx.get(&edge.source),
                self.node_idx.get(&edge.target),
            ) else {
                continue;
            };
            let (source, target) = (&self.base.graph_nodes[s], &self.base.graph_nodes[t]);
            let other = if src { target } else { source };
            for lid in locations(other).filter(|lid| allowed(lid) && *lid != main) {
                graph.push((
                    (
                        !seed_set.contains(lid),
                        role(lid) != "explanation",
                        nearby(lid),
                    ),
                    source,
                    target,
                    edge,
                    lid,
                ));
            }
        }
        let mut connection = graph
            .into_iter()
            .min_by_key(|v| v.0)
            .map(|(_, s, t, e, lid)| ReadingConnection {
                from: s.name.clone(),
                relation: e.edge_type.clone(),
                to: t.name.clone(),
                passage: passage(lid),
                example: None,
                direction: match e.direction {
                    base_schema::Direction::Directed => "directed",
                    base_schema::Direction::Undirected => "undirected",
                }
                .into(),
            })
            .or_else(|| {
                neighbors
                    .iter()
                    .filter_map(|&(from, r)| {
                        let to = r.target_lid.as_str();
                        let other = if from == main {
                            to
                        } else if to == main {
                            from
                        } else {
                            return None;
                        };
                        Some((
                            (
                                role(other) != "explanation",
                                role(other) != "example",
                                nearby(other),
                            ),
                            from,
                            r,
                            other,
                        ))
                    })
                    .min_by_key(|v| v.0)
                    .map(|(_, from, r, lid)| ReadingConnection {
                        from: from.to_owned(),
                        relation: r.relation_type.clone(),
                        to: r.target_lid.clone(),
                        passage: passage(lid),
                        example: None,
                        direction: r.direction.clone(),
                    })
            });
        if let Some(c) = &mut connection {
            c.example = self
                .discourse_items()
                .iter()
                .filter(|i| role(&i.lid) == "example" && allowed(&i.lid))
                .find(|i| {
                    i.relations.iter().any(|r| {
                        r.target_lid == c.passage.lid
                            && r.relation_type == "exemplifies"
                            && self.discourse_relation_valid(r)
                            && r.evidence_lids.iter().all(|lid| allowed(lid))
                    })
                })
                .map(|i| passage(&i.lid));
        }
        Some(ReadingSupport {
            topics: selected.iter().map(|n| n.name.clone()).collect(),
            passage: primary,
            connection,
        })
    }
}
