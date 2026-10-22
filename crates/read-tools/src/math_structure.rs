//! Optional source-grounded mathematical index for technical learning books.
use crate::{Book, BookTextCall, ContextItem, Via};
use base_schema::{ReadOnlyBase, Span};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Definition,
    Theorem,
    Lemma,
    Proposition,
    Corollary,
    Assumption,
    Proof,
    Exercise,
    Algorithm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    Uses,
    Proves,
    AsksToProve,
    RefersTo,
}

impl Relation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Uses => "uses",
            Self::Proves => "proves",
            Self::AsksToProve => "asks_to_prove",
            Self::RefersTo => "refers_to",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Local,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    Resolved,
    Missing,
    Ambiguous,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Quotation {
    pub quote: String,
    pub evidence_lids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Reference {
    pub relation: Relation,
    pub target_kind: Kind,
    pub target_label: String,
    pub scope: Scope,
    pub quote: String,
    pub evidence_lids: Vec<String>,
    pub resolution: Resolution,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_lid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Item {
    pub anchor_lid: String,
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub evidence_lids: Vec<String>,
    pub assumptions: Vec<Quotation>,
    pub references: Vec<Reference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub book_id: String,
    pub book_version: String,
    pub profile_id: String,
    pub profile_version: String,
    pub core_schema_version: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    pub version: String,
    pub header: Header,
    pub items: Vec<Item>,
}

/// Index annotations for actually delivered source. Calls are locators, not reads.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ReadingItem {
    #[serde(flatten)]
    pub item: Item,
    pub missing_evidence_lids: Vec<String>,
    pub next_calls: Vec<BookTextCall>,
}

impl Book {
    /// Only quotations whose complete cited leaves were read may enter the annotation.
    pub fn math_reading_context(&self, ranges: &[Span]) -> Vec<ReadingItem> {
        let Some(index) = &self.math_structure else {
            return Vec::new();
        };
        let mut ranges: Vec<_> = ranges.iter().filter(|r| r.start < r.end).cloned().collect();
        ranges.sort_by_key(|r| r.start);
        let mut merged: Vec<Span> = Vec::new();
        for range in ranges {
            if let Some(last) = merged.last_mut().filter(|last| range.start <= last.end) {
                last.end = last.end.max(range.end);
            } else {
                merged.push(range);
            }
        }
        let read = |lid: &str| {
            self.node(lid).is_ok_and(|node| {
                merged
                    .iter()
                    .any(|range| range.start <= node.span.start && node.span.end <= range.end)
            })
        };
        index
            .items
            .iter()
            .filter(|item| {
                item.evidence_lids.iter().any(|lid| {
                    self.node(lid).is_ok_and(|node| {
                        merged
                            .iter()
                            .any(|range| node.span.start < range.end && range.start < node.span.end)
                    })
                })
            })
            .map(|item| {
                let mut visible = item.clone();
                visible
                    .assumptions
                    .retain(|q| q.evidence_lids.iter().all(|lid| read(lid)));
                visible
                    .references
                    .retain(|r| r.evidence_lids.iter().all(|lid| read(lid)));
                let mut missing: Vec<_> = item
                    .evidence_lids
                    .iter()
                    .filter(|lid| !read(lid))
                    .cloned()
                    .collect();
                // A proof link can cite its preceding statement outside the
                // proof item's own evidence. Offer that context before quoting it.
                for lid in item.references.iter().flat_map(|r| &r.evidence_lids) {
                    if !read(lid) && !missing.contains(lid) {
                        missing.push(lid.clone());
                    }
                }
                let mut next = missing.clone();
                for reference in &visible.references {
                    if reference.resolution != Resolution::Resolved {
                        continue;
                    }
                    if let Some(target) = index
                        .items
                        .iter()
                        .find(|target| Some(&target.anchor_lid) == reference.target_lid.as_ref())
                    {
                        next.extend(
                            target
                                .evidence_lids
                                .iter()
                                .filter(|lid| !read(lid))
                                .cloned(),
                        );
                    }
                }
                ReadingItem {
                    item: visible,
                    missing_evidence_lids: missing,
                    next_calls: self.math_text_calls(&next),
                }
            })
            .collect()
    }

    /// Keep disjoint evidence disjoint; adjoining inline-math leaves form one source read.
    fn math_text_calls(&self, lids: &[String]) -> Vec<BookTextCall> {
        let mut nodes: Vec<_> = lids.iter().filter_map(|lid| self.node(lid).ok()).collect();
        nodes.sort_by_key(|node| node.span.start);
        nodes.dedup_by_key(|node| &node.lid);
        let mut calls: Vec<BookTextCall> = Vec::new();
        for node in nodes {
            if let Some(call) = calls.last_mut() {
                let end = self
                    .node(
                        call.arguments
                            .end_lid
                            .as_deref()
                            .unwrap_or(&call.arguments.lid),
                    )
                    .unwrap()
                    .span
                    .end;
                if end <= node.span.start
                    && String::from_utf16_lossy(&self.source_u16[end..node.span.start])
                        .trim()
                        .is_empty()
                {
                    call.arguments.end_lid = Some(node.lid.clone());
                    continue;
                }
            }
            calls.push(BookTextCall::new(&node.lid, None));
        }
        calls
    }

    /// Direct source relationships are available in both directions, independent of chapter distance.
    pub(crate) fn math_context_items(&self, lid: &str) -> Vec<ContextItem> {
        let Some(index) = &self.math_structure else {
            return Vec::new();
        };
        let owns =
            |item: &Item| item.anchor_lid == lid || item.evidence_lids.iter().any(|e| e == lid);
        let mut result = Vec::new();
        for source in &index.items {
            for reference in &source.references {
                if reference.resolution != Resolution::Resolved {
                    continue;
                }
                let Some(target) = index
                    .items
                    .iter()
                    .find(|item| Some(&item.anchor_lid) == reference.target_lid.as_ref())
                else {
                    continue;
                };
                // Proof evidence may include the preceding theorem. Prefer an exact
                // anchor, then the target's own passage, over shared context leaves.
                let (other, direction) = if source.anchor_lid == lid {
                    (target, "outgoing")
                } else if owns(target) {
                    (source, "incoming")
                } else if owns(source) {
                    (target, "outgoing")
                } else {
                    continue;
                };
                if other.anchor_lid == lid {
                    continue;
                }
                let mut evidence = reference.evidence_lids.clone();
                evidence.extend(other.evidence_lids.iter().cloned());
                evidence.sort();
                evidence.dedup();
                result.push(ContextItem {
                    lid: other.anchor_lid.clone(),
                    layer: "near".into(),
                    next_calls: self.math_text_calls(&other.evidence_lids),
                    via: Via::Mathematical {
                        source_lid: source.anchor_lid.clone(),
                        target_lid: target.anchor_lid.clone(),
                        relation_type: reference.relation.as_str().into(),
                        direction: direction.into(),
                        evidence_lids: evidence,
                    },
                });
            }
        }
        result
    }
}

pub(crate) fn parse(source: &str, base: &ReadOnlyBase) -> Result<Index, String> {
    let index: Index = serde_json::from_str(source).map_err(|e| e.to_string())?;
    if index.version != "math_structure.v1"
        || index.header.profile_id != "technical_learning"
        || index.header.book_id != base.book_id
    {
        return Err("math structure identity does not match the book".into());
    }
    let lids: HashSet<&str> = base
        .lid_nodes
        .iter()
        .map(|node| node.lid.as_str())
        .collect();
    let anchors: HashSet<&str> = index
        .items
        .iter()
        .map(|item| item.anchor_lid.as_str())
        .collect();
    if anchors.len() != index.items.len() {
        return Err("duplicate math structure anchor".into());
    }
    let valid_evidence = |evidence: &[String]| {
        !evidence.is_empty() && evidence.iter().all(|lid| lids.contains(lid.as_str()))
    };
    for item in &index.items {
        if !item.evidence_lids.contains(&item.anchor_lid)
            || !valid_evidence(&item.evidence_lids)
            || item.assumptions.iter().any(|q| {
                q.quote.is_empty()
                    || !valid_evidence(&q.evidence_lids)
                    || q.evidence_lids
                        .iter()
                        .any(|lid| !item.evidence_lids.contains(lid))
            })
        {
            return Err("math structure item evidence is invalid".into());
        }
        for reference in &item.references {
            if reference.quote.is_empty()
                || !valid_evidence(&reference.evidence_lids)
                || (reference.relation == Relation::Proves && item.kind != Kind::Proof)
                || (reference.relation == Relation::AsksToProve && item.kind != Kind::Exercise)
            {
                return Err("math structure reference evidence or role is invalid".into());
            }
            match (&reference.resolution, &reference.target_lid) {
                (Resolution::Resolved, Some(target))
                    if reference.scope == Scope::Local && anchors.contains(target.as_str()) => {}
                (Resolution::External, None) if reference.scope == Scope::External => {}
                (Resolution::Missing | Resolution::Ambiguous, None)
                    if reference.scope == Scope::Local => {}
                _ => return Err("math structure reference resolution is invalid".into()),
            }
        }
    }
    Ok(index)
}

pub(crate) fn load(directory: &Path, base: &ReadOnlyBase) -> Result<Option<Index>, String> {
    match std::fs::read_to_string(directory.join("math_structure.json")) {
        Ok(text) => parse(&text, base).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as read_tools;
    use serde_json::json;
    include!("../testdata/math_book.rs");

    #[test]
    fn math_structure_reading_context_keeps_unread_conditions_out_and_offers_complete_reads() {
        let book = optimization_math_book();
        let read = |lid: &str, end_lid: Option<&str>| {
            book.read_text(&book_tool_contracts::TextInput {
                lid: lid.into(),
                end_lid: end_lid.map(str::to_owned),
                start_utf16: None,
            })
            .unwrap()
        };
        let partial = read("1.1", None);
        assert_eq!(
            partial["math_context"][0]["assumptions"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(!partial.to_string().contains("局部极小点"));
        assert_eq!(
            partial["math_context"][0]["missing_evidence_lids"],
            json!(["1.2", "1.3"])
        );
        assert_eq!(
            partial["math_context"][0]["next_calls"][0]["arguments"],
            json!({"lid":"1.2","end_lid":"1.3"})
        );
        let complete = read("1.1", Some("1.3"));
        assert_eq!(
            complete["math_context"][0]["assumptions"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            complete["math_context"][0]["missing_evidence_lids"],
            json!([])
        );
        let exercise = read("1.8", Some("1.9"));
        let item = &exercise["math_context"][0];
        assert_eq!(item["references"][0]["relation"], "uses");
        assert_eq!(item["references"][1]["relation"], "asks_to_prove");
        assert_eq!(
            item["next_calls"][0]["arguments"],
            json!({"lid":"1.5","end_lid":"1.7"})
        );
        assert!(!exercise.to_string().contains("在假设6.1的条件下"));
        let missing = read("1.5", Some("1.6"));
        assert_eq!(
            missing["math_context"][0]["references"][0]["resolution"],
            "missing"
        );
        assert_eq!(missing["math_context"][0]["next_calls"], json!([]));
        let proof = read("1.4", None);
        assert_eq!(proof["math_context"][0]["references"], json!([]));
        assert_eq!(
            proof["math_context"][0]["missing_evidence_lids"],
            json!(["1.1"])
        );
        assert_eq!(
            proof["math_context"][0]["next_calls"][0]["arguments"],
            json!({"lid":"1.1"})
        );
        let joined = book.math_reading_context(&[
            book.node("1.1").unwrap().span.clone(),
            book.node("1.4").unwrap().span.clone(),
        ]);
        let proof = joined
            .iter()
            .find(|item| item.item.kind == Kind::Proof)
            .unwrap();
        assert_eq!(proof.item.references[0].relation, Relation::Proves);
    }

    #[test]
    fn math_structure_context_and_guided_routes_preserve_relation_direction_and_exercise_roles() {
        let book = optimization_math_book();
        let exercise =
            serde_json::to_value(book.context("1.9", Some("near"), Some(2)).unwrap()).unwrap();
        assert_eq!(exercise["items"][0]["via"]["type"], "uses");
        assert_eq!(exercise["items"][1]["via"]["type"], "asks_to_prove");
        assert_eq!(
            exercise["items"][0]["next_calls"][0]["arguments"],
            json!({"lid":"1.5","end_lid":"1.6"})
        );
        let theorem = serde_json::to_value(book.context("1.2", None, Some(1)).unwrap()).unwrap();
        assert_eq!(theorem["items"][0]["lid"], "1.4");
        assert_eq!(theorem["items"][0]["via"]["type"], "proves");
        assert_eq!(theorem["items"][0]["via"]["direction"], "incoming");
        let route = serde_json::to_value(book.route_from("1.9", Some(10)).unwrap()).unwrap();
        assert!(route["back"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["lid"] == "1.5" && s["edge_type"] == "uses"));
        assert!(!route["back"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["edge_type"] == "asks_to_prove"));
        assert!(route["cross"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["lid"] == "1.7" && s["edge_type"] == "asks_to_prove"));
        let target = serde_json::to_value(book.route_from("1.7", Some(10)).unwrap()).unwrap();
        assert!(target["concretize"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["lid"] == "1.8" && s["edge_type"] == "asks_to_prove"));
    }

    #[test]
    fn math_structure_shared_proof_context_keeps_theorem_to_proof_direction() {
        let mut book = optimization_math_book();
        // The discourse packet for a proof can include the preceding theorem.
        book.math_structure.as_mut().unwrap().items[1].evidence_lids =
            ["1.1", "1.2", "1.3", "1.4"].map(str::to_owned).to_vec();
        for lid in ["1.1", "1.2"] {
            let items = book.math_context_items(lid);
            assert_eq!(items[0].lid, "1.4");
            assert!(
                matches!(&items[0].via, Via::Mathematical { direction, .. } if direction == "incoming")
            );
        }
        let items = book.math_context_items("1.4");
        assert_eq!(items[0].lid, "1.1");
        assert!(
            matches!(&items[0].via, Via::Mathematical { direction, .. } if direction == "outgoing")
        );
    }

    fn base() -> ReadOnlyBase {
        let mut base = base_schema::sample_base();
        base.lid_nodes[0].children.push("1.2".into());
        base.lid_nodes[1].span.end = 50;
        base.lid_nodes.push(base_schema::LidNode {
            lid: "1.2".into(),
            path: vec![1, 2],
            kind: base_schema::NodeKind::Paragraph,
            span: base_schema::Span {
                start: 50,
                end: 100,
            },
            children: vec![],
        });
        base
    }

    fn index(book_id: &str) -> serde_json::Value {
        json!({"version":"math_structure.v1", "header": {
            "book_id":book_id,"book_version":"v1","profile_id":"technical_learning",
            "profile_version":"technical_learning_v0","core_schema_version":"core_v0","generated_at":"1970-01-01T00:00:00.000Z"
        }, "items": [
            {"anchor_lid":"1.1", "kind":"theorem", "label":"定理 5.3", "evidence_lids":["1.1"],
             "assumptions":[{"quote":"f可微","evidence_lids":["1.1"]}], "references":[]},
            {"anchor_lid":"1.2", "kind":"exercise", "label":"习题 6.3", "evidence_lids":["1.2"], "assumptions":[],
             "references":[{"relation":"asks_to_prove","target_kind":"theorem","target_label":"定理5.3","scope":"local",
                "quote":"证明定理5.3","evidence_lids":["1.2"],"resolution":"resolved","target_lid":"1.1"}]}
        ]})
    }

    #[test]
    fn math_structure_loads_and_remains_optional_for_existing_books() {
        let base = base();
        let directory =
            std::env::temp_dir().join(format!("ub-math-structure-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("base.json"),
            serde_json::to_string(&base).unwrap(),
        )
        .unwrap();
        std::fs::write(directory.join("source.txt"), "X".repeat(100)).unwrap();
        std::fs::write(
            directory.join("math_structure.json"),
            index(&base.book_id).to_string(),
        )
        .unwrap();
        let book = crate::Book::load(directory.to_str().unwrap()).unwrap();
        assert_eq!(
            book.math_structure_items("1.2")[0].references[0].relation,
            Relation::AsksToProve
        );
        assert_eq!(
            book.math_structure_items("1.1")[0].assumptions[0].quote,
            "f可微"
        );
        std::fs::remove_file(directory.join("math_structure.json")).unwrap();
        assert!(crate::Book::load(directory.to_str().unwrap())
            .unwrap()
            .math_structure_items("1.1")
            .is_empty());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn math_structure_rejects_wrong_identity_dangling_targets_and_exercise_proofs() {
        let base = base();
        let value = index(&base.book_id);
        let mut invalid = value.clone();
        invalid["header"]["book_id"] = json!("another-book");
        assert!(parse(&invalid.to_string(), &base).is_err());
        invalid = value.clone();
        invalid["items"][1]["references"][0]["target_lid"] = json!("missing");
        assert!(parse(&invalid.to_string(), &base).is_err());
        invalid = value;
        invalid["items"][1]["references"][0]["relation"] = json!("proves");
        assert!(parse(&invalid.to_string(), &base).is_err());
    }
}
