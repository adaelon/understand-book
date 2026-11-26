use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackBookNode {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub condition: Option<String>,
    #[serde(default)]
    pub evidence: Vec<TeachBackBookQuote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackBookEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub relation_type: TeachBackRelationType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub condition: Option<String>,
    pub evidence: Vec<TeachBackBookQuote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackBookGraph {
    pub nodes: Vec<TeachBackBookNode>,
    pub edges: Vec<TeachBackBookEdge>,
}

impl TeachBackBookGraph {
    pub(super) fn topology(&self) -> Topology<'_> {
        Topology {
            nodes: self
                .nodes
                .iter()
                .map(|n| Vertex {
                    id: &n.id,
                    label: &n.label,
                    condition: n.condition.as_deref(),
                })
                .collect(),
            edges: self
                .edges
                .iter()
                .map(|e| Link {
                    id: &e.id,
                    from: &e.from,
                    to: &e.to,
                    relation_type: e.relation_type,
                    condition: e.condition.as_deref(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
#[serde(bound(deserialize = "Q: Deserialize<'de>"))]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackStudentNode<Q = TeachBackStudentQuote> {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub condition: Option<String>,
    #[serde(default)]
    pub quotes: Vec<Q>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
#[serde(bound(deserialize = "Q: Deserialize<'de>"))]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackStudentEdge<Q = TeachBackStudentQuote> {
    pub id: String,
    pub from: String,
    pub to: String,
    pub relation_type: TeachBackRelationType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub condition: Option<String>,
    pub quotes: Vec<Q>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
#[serde(bound(deserialize = "Q: Deserialize<'de>"))]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackStudentGraph<Q = TeachBackStudentQuote> {
    pub nodes: Vec<TeachBackStudentNode<Q>>,
    pub edges: Vec<TeachBackStudentEdge<Q>>,
}

impl<Q> TeachBackStudentGraph<Q> {
    pub(super) fn topology(&self) -> Topology<'_> {
        Topology {
            nodes: self
                .nodes
                .iter()
                .map(|n| Vertex {
                    id: &n.id,
                    label: &n.label,
                    condition: n.condition.as_deref(),
                })
                .collect(),
            edges: self
                .edges
                .iter()
                .map(|e| Link {
                    id: &e.id,
                    from: &e.from,
                    to: &e.to,
                    relation_type: e.relation_type,
                    condition: e.condition.as_deref(),
                })
                .collect(),
        }
    }
}

struct Vertex<'a> {
    id: &'a str,
    label: &'a str,
    condition: Option<&'a str>,
}

struct Link<'a> {
    id: &'a str,
    from: &'a str,
    to: &'a str,
    relation_type: TeachBackRelationType,
    condition: Option<&'a str>,
}

pub(super) struct Topology<'a> {
    nodes: Vec<Vertex<'a>>,
    edges: Vec<Link<'a>>,
}

fn short_phrase(text: &str) -> bool {
    nonempty(text) && !text.contains(['\n', '\r'])
        && text.chars().count() <= 60
        && text.chars().filter(|c| matches!(c, '\u{3400}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{2fa1f}')).count() <= 22
}

impl Topology<'_> {
    pub(super) fn validate(&self) -> Result<(), String> {
        id_set(
            self.nodes
                .iter()
                .map(|v| v.id)
                .chain(self.edges.iter().map(|e| e.id)),
        )?;
        let nodes: HashSet<_> = self.nodes.iter().map(|v| v.id).collect();
        let mut labels = HashSet::new();
        for v in &self.nodes {
            if !short_phrase(v.label) || v.condition.is_some_and(|c| !short_phrase(c)) {
                return Err(format!("Shorten node {} to one proposition. Labels and conditions must be single-line, at most 22 Chinese and 60 total characters each. Put full wording in quotations.", v.id));
            }
            if !labels.insert((v.label.trim(), v.condition.map(str::trim))) {
                return Err(format!("Reuse the existing node ID for repeated proposition {:?}, including on connecting steps.", v.label));
            }
        }
        for (i, e) in self.edges.iter().enumerate() {
            if e.from == e.to || !nodes.contains(e.from) || !nodes.contains(e.to) {
                return Err(format!(
                    "Edge {} needs two distinct existing node IDs on its own side.",
                    e.id
                ));
            }
            if e.condition.is_some_and(|c| !short_phrase(c)) {
                return Err(format!("Shorten the condition of edge {} (single line, at most 22 Chinese and 60 total characters).", e.id));
            }
            if self.edges[..i].iter().any(|other| {
                other.from == e.from
                    && other.to == e.to
                    && other.relation_type == e.relation_type
                    && other.condition == e.condition
            }) {
                return Err(format!(
                    "Edge {} repeats an existing step; use one edge with all its quotations.",
                    e.id
                ));
            }
        }
        // Each layer has all its incoming links in previously placed layers.
        let mut placed = HashSet::new();
        while placed.len() < nodes.len() {
            let remaining = placed.len();
            for v in &self.nodes {
                if self
                    .edges
                    .iter()
                    .filter(|e| e.to == v.id)
                    .all(|e| placed.contains(e.from))
                {
                    placed.insert(v.id);
                }
            }
            if placed.len() == remaining {
                return Err("A directed cycle prevents the reasoning steps being ordered. Check edge directions and distinguish premises from conclusions.".into());
            }
        }
        Ok(())
    }

    pub(super) fn verify_ownership<'r>(
        &self,
        side: &str,
        groups: impl Iterator<Item = (&'r String, &'r Vec<String>, &'r Vec<String>)>,
        diagnostics: &mut Vec<String>,
    ) {
        let isolated: HashSet<_> = self
            .nodes
            .iter()
            .filter(|v| !self.edges.iter().any(|e| e.from == v.id || e.to == v.id))
            .map(|v| v.id)
            .collect();
        let mut assigned = HashSet::new();
        for (row, edge_ids, node_ids) in groups {
            let mut links = vec![];
            let mut vertices = HashSet::new();
            for id in edge_ids {
                if let Some(e) = self.edges.iter().find(|e| e.id == id) {
                    links.push(e);
                    vertices.extend([e.from, e.to]);
                } else {
                    diagnostics.push(format!("Focus {row}: {id} is not a {side} edge."));
                }
            }
            let mut isolated_here = Vec::new();
            for id in node_ids {
                if isolated.contains(id.as_str()) {
                    isolated_here.push(id);
                } else {
                    diagnostics.push(format!("Focus {row}: {id} is not an isolated {side} node. Remove it from {side}_node_ids. Represent connected steps by their incident edge IDs."));
                }
            }
            if isolated_here.len() > 1 || (!isolated_here.is_empty() && !links.is_empty()) {
                diagnostics.push(format!("Focus {row} contains disconnected {side} claims: isolated node IDs {isolated_here:?} each need their own focus on this side, with no {side} edge IDs."));
            }
            for id in links
                .iter()
                .map(|e| e.id)
                .chain(isolated_here.iter().map(|id| id.as_str()))
            {
                if !assigned.insert(id) {
                    diagnostics.push(format!("Focus {row} repeats {side} ID {id}. Assign each edge or isolated node to exactly one focus; group connected counterparts together."));
                }
            }
            if let Some(start) = links.first().map(|e| e.from) {
                let mut reached = HashSet::from([start]);
                loop {
                    let previous = reached.len();
                    for e in &links {
                        if reached.contains(e.from) || reached.contains(e.to) {
                            reached.extend([e.from, e.to]);
                        }
                    }
                    if reached.len() == previous {
                        break;
                    }
                }
                if reached.len() != vertices.len() {
                    let connected: Vec<_> = links
                        .iter()
                        .filter(|e| reached.contains(e.from))
                        .map(|e| e.id)
                        .collect();
                    let separate: Vec<_> = links
                        .iter()
                        .filter(|e| !reached.contains(e.from))
                        .map(|e| e.id)
                        .collect();
                    diagnostics.push(format!("Focus {row} contains disconnected {side} claims. Edges {connected:?} share no endpoint with {separate:?}. Split these groups and align each with the claim it addresses."));
                }
            }
        }
        let mut missing: Vec<_> = self
            .edges
            .iter()
            .map(|e| e.id)
            .chain(isolated)
            .filter(|id| !assigned.contains(id))
            .collect();
        missing.sort_unstable();
        if !missing.is_empty() {
            diagnostics.push(format!("Unaligned {side} IDs: {missing:?}. Place each edge and isolated node in one focus group."));
        }
    }
}

impl TeachBackStudentGraph<String> {
    pub(super) fn freeze(
        self,
        turn: &str,
        expression: &str,
    ) -> Result<TeachBackStudentGraph, String> {
        if self.nodes.is_empty() {
            return Err("An explanation needs at least one quoted student claim.".into());
        }
        self.topology().validate()?;
        let quotes = |id: &str,
                      values: Vec<String>,
                      required: bool|
         -> Result<Vec<TeachBackStudentQuote>, String> {
            if required && values.is_empty()
                || values
                    .iter()
                    .any(|q| !nonempty(q) || !expression.contains(q))
            {
                return Err(format!(
                    "{id} needs exact quotations from the current expression."
                ));
            }
            Ok(values
                .into_iter()
                .map(|text| TeachBackStudentQuote {
                    turn_id: turn.into(),
                    text,
                })
                .collect())
        };
        let mut nodes = Vec::new();
        for n in self.nodes {
            let isolated = !self.edges.iter().any(|e| e.from == n.id || e.to == n.id);
            // A verbatim standalone label already supplies its own source text.
            // Paraphrases and supplied quotations still require an exact match.
            let source_words = if isolated && n.quotes.is_empty() && expression.contains(&n.label) {
                vec![n.label.clone()]
            } else {
                n.quotes
            };
            nodes.push(TeachBackStudentNode {
                quotes: quotes(&n.id, source_words, isolated)?,
                id: n.id,
                label: n.label,
                condition: n.condition,
            });
        }
        let mut edges = Vec::new();
        for e in self.edges {
            edges.push(TeachBackStudentEdge {
                quotes: quotes(&e.id, e.quotes, true)?,
                id: e.id,
                from: e.from,
                to: e.to,
                condition: e.condition,
                relation_type: e.relation_type,
            });
        }
        Ok(TeachBackStudentGraph { nodes, edges })
    }
}
