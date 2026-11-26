//! Reader-owned teach-back. Only the second model stage can see book evidence.
use std::collections::HashSet;

use read_tools::teach_back::{TeachBackEvidence, TeachBackPassage};
use read_tools::{Book, ToolError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

mod graph;
mod history;
mod quote_repair;
pub use graph::*;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackSource {
    pub source_id: String,
    pub source_revision: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum TeachBackRelationType {
    Causes,
    Requires,
    Contrasts,
    Defines,
    Rebuts,
    Supports,
    Concludes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum TeachBackStatus {
    Consistent,
    Divergent,
    Omitted,
    AlternativeValid,
    UnverifiedAgainstPassage,
    InsufficientBookBasis,
    Clarified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackBookQuote {
    pub lid: String,
    pub quote: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackStudentQuote {
    pub turn_id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackBookRelation {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub premise: Option<String>,
    pub relation_type: TeachBackRelationType,
    pub conclusion: String,
    pub evidence: Vec<TeachBackBookQuote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackStudentRelation<Q = TeachBackStudentQuote> {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub premise: Option<String>,
    pub relation_type: TeachBackRelationType,
    pub conclusion: String,
    pub quotes: Vec<Q>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackRow {
    pub row_id: String,
    #[serde(default)]
    pub book_edge_ids: Vec<String>,
    #[serde(default)]
    pub book_node_ids: Vec<String>,
    #[serde(default)]
    pub student_edge_ids: Vec<String>,
    #[serde(default)]
    pub student_node_ids: Vec<String>,
    pub status: TeachBackStatus,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub divergence_condition: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackSideNote {
    pub label: String,
    pub text: String,
    pub evidence: Vec<TeachBackBookQuote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackFollowUp {
    pub row_id: String,
    pub question: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum TeachBackReplyKind {
    DisputeInterpretation,
    ReviseUnderstanding,
    FollowUp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackReply {
    pub kind: TeachBackReplyKind,
    pub round_id: String,
    pub row_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackRoundContext {
    pub kind: TeachBackReplyKind,
    pub parent_round_id: String,
    pub first_round_id: String,
    pub row_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(from = "history::StoredComparison")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachBackComparison {
    pub round_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub continuation: Option<TeachBackRoundContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub support_delivery_refs: Option<Vec<String>>,
    pub source: TeachBackSource,
    pub book_graph: TeachBackBookGraph,
    pub student_graph: TeachBackStudentGraph,
    pub rows: Vec<TeachBackRow>,
    pub side_notes: Vec<TeachBackSideNote>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub follow_up: Option<TeachBackFollowUp>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ExpressionDraft {
    Help,
    Explanation {
        student_graph: TeachBackStudentGraph<String>,
    },
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum DisputedExpressionDraft {
    NeedsRevision,
    Explanation {
        student_graph: TeachBackStudentGraph<String>,
    },
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ComparisonDraft {
    source: TeachBackSource,
    book_graph: TeachBackBookGraph,
    rows: Vec<TeachBackRow>,
    /// An alternative way to reach the goal that changes a given outside the question.
    /// Include it here with the passage that actually introduces it. Keep the main
    /// comparison on the original givens. Do not repeat a core point as a side note.
    side_notes: Vec<TeachBackSideNote>,
    follow_up: Option<TeachBackFollowUp>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PassageRequest {
    read_lids: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
enum ComparisonResponse {
    Read(PassageRequest),
    Answer(ComparisonDraft),
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ReplyDisposition {
    NeedsRevision,
    OrdinaryChat,
}

#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
enum ReplyResponse {
    Comparison(ComparisonResponse),
    Disposition(ReplyDisposition),
}

pub struct TeachBackContinuation<'a> {
    pub kind: TeachBackReplyKind,
    pub previous: &'a TeachBackComparison,
    pub row_id: &'a str,
    pub first_expression: &'a str,
    pub first_turn_id: &'a str,
}

#[derive(Debug)]
pub enum TeachBackResult {
    OrdinaryChat,
    NeedsRevision,
    Comparison(TeachBackComparison),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeachBackStage {
    OrganizeExpression,
    ComparePassages,
}

const ORGANIZE: &str = r#"Read only the original question, topic names and current expression. Return kind "help" for a pure help request, a new question, or a message without an explanation. A tentative substantive claim, analogy or claim followed by a question is an explanation. For kind "explanation", reconstruct ONLY the reasoning the reader actually expressed, in their language. Never borrow a premise from the question, fill in missing reasoning, correct the reader, or decide truth. If a prior prediction is contrasted with a new view, distinguish them. Preserve pronouns, stated conditions and inference direction.
Draw student_graph as short propositions (nodes) and directed relations (edges). Reuse the SAME node ID wherever a step ends and the next begins, including branches and convergences. Split separately asserted steps into separate edges, even when conveyed in one sentence. If only a conclusion is expressed, use an isolated node with quotes, never invent an edge. Keep labels and optional conditions single-line, each at most 22 Chinese characters and 60 characters total; preserve complete wording in quotations. Give every edge and every isolated node one or more exact contiguous quotes (plain strings) from the current expression. For a standalone statement, copy its original words into quotes even if the label already repeats them. Every node AND every edge has its own required id. Edge wire shape: {"id":"se1","from":"sn1","to":"sn2","relation_type":"requires","quotes":["<exact words>"]}, where both endpoints are IDs of nodes in your graph.
Read the whole expression before drawing it. A later "provided that", "it requires", or similar qualification limits the earlier claim: keep that scope in the node/edge condition or a requires edge from the qualified claim to its prerequisite. Preserve each explicitly stated inference once. Do not add a shortcut A→C alongside A→B→C just because C follows transitively. Coordinate outcomes in one clause can remain one short node when neither outcome is a reason for the other. Conversely, if a sentence explicitly gives a dependency or a cause and effect, split it into proposition nodes joined by that relation; never hide the entire inference inside one label. Sentence order, a shared topic, a transition like "after that", or an example following a definition is not by itself evidence for a causal edge. Separate independent claims may form separate components."#;
const COMPARE: &str = r#"Compare frozen_student_graph to the passages actually supplied. The student side is immutable. Return only the schema object in the reader's language, echoing the exact source identity. Draw book_graph for this question or, if no question, the reader's topic and section throughline. Use up to 6 short proposition nodes and 5 directed edges. Where LIDs span paragraphs, use the local directed Pass1 edges and discourse relations (causes, results_in, supports, contrasts, explains, answers, depends_on) to locate the core argument and separate illustrations, transitions and alternatives. Within a single passage, including when discourse relations are empty, use claims and local_summary to find premises, conditions, intermediate steps and conclusions in the actual passage. Preserve the book's most relevant connected chain and reuse its intermediate node IDs. Never pack a whole definition or multiple inferences into an edgeless long label. Node labels and optional conditions must be a single short phrase (max 22 Chinese characters and 60 characters total). Put the full wording in evidence. Unless all rows are insufficient_book_basis, include an actual directed book edge. Cite exact contiguous text from the supplied LID on each edge and any isolated book node. Multiple exact quotations can support an inference across paragraphs. Metadata guides reading but never substitutes for a passage. An optional read_lids request can retrieve up to four nearby LIDs or additional pages before answering.
Match edges and isolated nodes into numbered focus groups (rows). Assign EVERY edge and EVERY isolated node to EXACTLY ONE row on its own side; use book_edge_ids/student_edge_ids for edges and book_node_ids/student_node_ids ONLY for isolated nodes. Never list endpoints of referenced edges in the node ID arrays. Match connected subgraphs (including 2:2, 1:N, N:1) when they address the same point; group related edges on one row instead of reusing an edge across rows. Keep distinct disagreements separate. Each side within a single row must be connected. An unmatched core claim may occupy a single-sided row. First interpret the complete frozen graph, including all quotes, node/edge conditions and adjacent requires edges, THEN group it. An edge A requires B makes B a necessary condition of A regardless of sentence order. Never evaluate A as an unconditional claim or test it in a case that violates B. A later stated restriction qualifies the earlier assertion even if you display the links in different focus groups. Keep a qualifying link with the claim it qualifies when that resolves an apparent disagreement. Ask a follow-up only after checking that no other part of the expression already answers it. Use focus_targets as the exact allowed student ID lists: edges go in student_edge_ids; student_node_ids may ONLY come from focus_targets.student_node_ids. Thus if that list is empty, write [] on every row. Each listed isolated node needs its own focus on its side, with NO other node or edge IDs on that side. For a reader who states an independent fact and a separate disputed inference, compare these in two focuses. Apply the same rule to your authored book graph. Start each focus from the claim being compared and follow just its connected edges. Each book edge must carry its own id, from, to. Keep IDs exclusively in reference fields. In visible prose, including summaries, divergence conditions and questions, name the actual propositions in everyday language. Never refer to "your e1", "e2-e4", or a node, edge or row ID. Address the reader directly in visible text. Keep genuine alternative approaches outside the main question in side_notes with their own evidence; do not repeat core points there.
Use measured or calculated comparisons to support interpretations in the correct direction. For each supports edge, read it aloud as "FROM is evidence for TO": observations or numerical comparisons support a general interpretation. Do not use a general takeaway to infer a specific measured number. A formula with its inputs may yield that number. Include only calculation steps needed for this question; a control case that helps establish the book's point can support a main edge without becoming a separate obligation for the reader. When an upstream passage supplies the baseline measurement or formula and a later passage reports the changed outcome, cite both at the step they support. State which variable changes while the other givens stay fixed. A coincident change in a second variable does not make that second change necessary. Keep calculated lower bounds distinct from actual elapsed time. A matching conclusion alone does not establish unstated reasoning or a fact the passage does not verify. For an unsupported measurement, condition or comparison, set the row status to unverified_against_passage (insufficient_book_basis if no basis exists); explain precisely what remains unknown. Treat equivalent analogies and valid shorter arguments as successful. Use omitted ONLY when answering the original question requires stating this particular step. A comparison, example or independent derivation used by the author to show why a claim holds can stay in evidence or a side note; a reader who gives the sufficient explanation in other words need not reproduce it. Keep worked-example assumptions as conditions and avoid necessity language unless the source establishes necessity. Omitted means unexpressed. Never invent the missing side. For divergence, state a concrete condition under which predictions differ. Ask at most one follow-up for the most important gap; end without a question when the reasoning agrees. No grades or mastery decisions."#;

const REREAD: &str = "The reader disputes how their FIRST expression was interpreted. Use only those first words, original question and topic names for extraction. correction_request suggests a reading but is never student evidence. Inspect negation, direction, metaphor and contrast; disputed_quotes point to the selected wording. If ANY requested change depends on a reason, condition or information those words did not express, return kind needs_revision and stop. Otherwise return kind explanation with the corrected graph of ALL original reasoning. Follow the same graph and quotation rules as blind extraction. Never return kind help for a dispute.";
const REPLY: &str = "This is a reply about an earlier comparison. When a self-correction introduces a substantive revised claim, keep the self-correction as context or a quotation on that claim; do not create a separate book-comparison relation just for announcing a mistake. Preserve the exact words of short answers, including yes/no, corrections and references to a preceding sentence, as an explanation even when their full meaning depends on the question. A change of subject, including a substantive claim clearly unrelated to the original topic, goes to help.";
const DISPUTE_CHECK: &str = "The reader is disputing the interpretation of the FIRST expression. Evaluate the correction_request against the first_expression and the independent, frozen rereading. The request identifies a possible misreading; it is never student evidence. If the proposed interpretation introduces a reason, condition or claim the original wording does not support, return {\"kind\":\"needs_revision\"}. This also applies if only part of the proposed interpretation is new. When the original words support the correction, deliver a whole comparison of those words, retaining their original quotes and correcting the selected misreading. Do not mark a reinterpretation clarified.";
const REVISION_CHECK: &str = "This round compares the reader's NEW expression. A revision can resolve several claims at once: use separate rows for distinct comparisons. Match connected steps concerning one issue in one focus; separate unrelated claims, preserving unique ownership. The selected prior row and its question explain what a short answer refers to. Previous feedback is help, never proof that the reader stated a reason. Address the selected issue in the new comparison. Use clarified on a two-sided row only if the new words actually resolve the earlier uncertainty or correct the reasoning. Preserve missing or still divergent reasoning, including when a yes/no prediction is correct but its reason is unstated. Never credit a new expression with unspoken reasons from the previous card.";
const FOLLOW_UP_CHECK: &str = "If this message asks for ordinary help or introduces a different topic instead of responding to the selected question, return {\"kind\":\"ordinary_chat\"}.";

fn schema_instruction<T: JsonSchema>(instructions: &str) -> String {
    let schema = schemars::schema_for!(T);
    format!(
        "{instructions}\n\nJSON Schema:\n{}",
        serde_json::to_string(&schema).unwrap()
    )
}

fn nonempty(text: &str) -> bool {
    !text.trim().is_empty()
}

// Generated item IDs sometimes appear between Chinese words, without spaces.
fn names_item(text: &str, id: &str) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    text.match_indices(id).any(|(offset, _)| {
        !text[..offset].chars().next_back().is_some_and(word)
            && !text[offset + id.len()..].chars().next().is_some_and(word)
    })
}

fn id_set<'a>(values: impl IntoIterator<Item = &'a str>) -> Result<HashSet<&'a str>, String> {
    let mut ids = HashSet::new();
    for id in values {
        if !nonempty(id) || !ids.insert(id) {
            return Err(format!("Empty or repeated ID: {id:?}"));
        }
    }
    Ok(ids)
}

fn freeze(
    draft: ExpressionDraft,
    turn: &str,
    expression: &str,
) -> Result<Option<TeachBackStudentGraph>, String> {
    match draft {
        ExpressionDraft::Help => Ok(None),
        ExpressionDraft::Explanation { student_graph } => {
            student_graph.freeze(turn, expression).map(Some)
        }
    }
}

fn passage_has_quote(passage: &TeachBackPassage, quote: &str) -> bool {
    nonempty(quote)
        && passage.text.match_indices(quote).any(|(byte, _)| {
            let span = &passage.source_range_utf16;
            let start = span.start + passage.text[..byte].encode_utf16().count();
            start + quote.encode_utf16().count() <= span.end
        })
}

fn verify_quotes(quotes: &[TeachBackBookQuote], read: &[TeachBackPassage]) -> Result<(), String> {
    if quotes.is_empty() {
        return Err("Each book relation and side note needs a passage quote.".into());
    }
    for quote in quotes {
        if !read
            .iter()
            .any(|p| p.lid == quote.lid && passage_has_quote(p, &quote.quote))
        {
            return Err(quote_repair::missing_quote(quote, read));
        }
    }
    Ok(())
}

impl ComparisonDraft {
    fn verify(
        &self,
        evidence: &TeachBackEvidence,
        student: &TeachBackStudentGraph,
        is_new_expression_after_feedback: bool,
    ) -> Result<(), String> {
        if self.source.source_id != evidence.source_id
            || self.source.source_revision != evidence.source_revision
        {
            return Err(
                "source_id/source_revision must match the passages supplied for this round.".into(),
            );
        }
        let mut diagnostics = Vec::new();
        if self.rows.is_empty() {
            diagnostics.push("Include at least one focus row.".into());
        }
        if self.book_graph.nodes.len() > 6 || self.book_graph.edges.len() > 5 {
            diagnostics.push("Keep the local book graph to 6 nodes and 5 edges at most.".into());
        }
        if self.book_graph.edges.is_empty()
            && self
                .rows
                .iter()
                .any(|r| r.status != TeachBackStatus::InsufficientBookBasis)
        {
            diagnostics.push("book_graph needs a directed edge with distinct endpoints unless all rows have insufficient_book_basis. Split the argument into short propositions and connect them.".into());
        }
        if let Err(message) = self.book_graph.topology().validate() {
            diagnostics.push(message);
        }
        for n in &self.book_graph.nodes {
            if !n.evidence.is_empty()
                || !self
                    .book_graph
                    .edges
                    .iter()
                    .any(|e| e.from == n.id || e.to == n.id)
            {
                if let Err(message) = verify_quotes(&n.evidence, &evidence.passages) {
                    diagnostics.push(message);
                }
            }
        }
        for edge in &self.book_graph.edges {
            if let Err(message) = verify_quotes(&edge.evidence, &evidence.passages) {
                diagnostics.push(message);
            }
        }
        for note in &self.side_notes {
            if !nonempty(&note.label) || !nonempty(&note.text) {
                diagnostics.push("Empty side note.".into());
            }
            if let Err(message) = verify_quotes(&note.evidence, &evidence.passages) {
                diagnostics.push(message);
            }
        }
        self.verify_alignment(student, is_new_expression_after_feedback, &mut diagnostics);
        let prose: Vec<_> = self
            .rows
            .iter()
            .flat_map(|r| {
                std::iter::once(r.summary.as_str()).chain(r.divergence_condition.as_deref())
            })
            .chain(self.follow_up.iter().map(|f| f.question.as_str()))
            .collect();
        let ids = self
            .book_graph
            .nodes
            .iter()
            .map(|n| &n.id)
            .chain(self.book_graph.edges.iter().map(|e| &e.id))
            .chain(student.nodes.iter().map(|n| &n.id))
            .chain(student.edges.iter().map(|e| &e.id))
            .chain(self.rows.iter().map(|r| &r.row_id));
        for id in ids {
            // A book or reader may actually discuss a domain term such as L2.
            if id.ends_with(|c: char| c.is_ascii_digit())
                && id.starts_with(|c: char| c.is_ascii_alphabetic())
                && prose.iter().any(|t| names_item(t, id))
                && !evidence.passages.iter().any(|p| names_item(&p.text, id))
                && !student
                    .nodes
                    .iter()
                    .flat_map(|n| &n.quotes)
                    .chain(student.edges.iter().flat_map(|e| &e.quotes))
                    .any(|q| names_item(&q.text, id))
            {
                diagnostics.push(format!("Replace internal graph ID {id} in reader-visible prose with the actual proposition. Keep IDs only in reference fields."));
            }
        }
        if let Some(follow) = &self.follow_up {
            let pending = self.rows.iter().any(|row| {
                row.row_id == follow.row_id
                    && !matches!(
                        row.status,
                        TeachBackStatus::Consistent
                            | TeachBackStatus::AlternativeValid
                            | TeachBackStatus::Clarified
                    )
            });
            if !pending || !nonempty(&follow.question) {
                diagnostics.push("A follow-up must target an unresolved row. If all rows agree, set follow_up to null.".into());
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics.join("\n"))
        }
    }

    fn verify_alignment(
        &self,
        student: &TeachBackStudentGraph,
        is_new_expression_after_feedback: bool,
        diagnostics: &mut Vec<String>,
    ) {
        if let Err(message) = id_set(self.rows.iter().map(|r| r.row_id.as_str())) {
            diagnostics.push(message);
        }
        self.book_graph.topology().verify_ownership(
            "book",
            self.rows
                .iter()
                .map(|r| (&r.row_id, &r.book_edge_ids, &r.book_node_ids)),
            diagnostics,
        );
        student.topology().verify_ownership(
            "student",
            self.rows
                .iter()
                .map(|r| (&r.row_id, &r.student_edge_ids, &r.student_node_ids)),
            diagnostics,
        );
        for row in &self.rows {
            let book = !(row.book_edge_ids.is_empty() && row.book_node_ids.is_empty());
            let learner = !(row.student_edge_ids.is_empty() && row.student_node_ids.is_empty());
            if !book && !learner || !nonempty(&row.summary) {
                diagnostics.push(format!(
                    "Focus {} needs at least one graph item and a comparison summary.",
                    row.row_id
                ));
            }
            let problem = match row.status {
                TeachBackStatus::Omitted if !book || learner => "Omitted is a book-only core relationship.",
                TeachBackStatus::Consistent | TeachBackStatus::Divergent if !book || !learner =>
                    "Agreement and divergence require both sides. Match this claim with its counterpart. Group connected supporting book edges with the argument they support. Move optional book-only alternatives into side_notes.",
                TeachBackStatus::Divergent if !row.divergence_condition.as_deref().is_some_and(nonempty) =>
                    "Describe the condition under which the two predictions diverge.",
                TeachBackStatus::Clarified if !is_new_expression_after_feedback || !book || !learner =>
                    "Clarified requires a new expression after feedback and a relation on each side.",
                _ => continue,
            };
            diagnostics.push(format!("Focus {}: {problem}", row.row_id));
        }
    }
}

fn sample_checked<T>(
    stage: TeachBackStage,
    instructions: &str,
    packet: &Value,
    repair_available: &mut bool,
    sample: &mut impl FnMut(TeachBackStage, &str, &Value) -> Result<String, ToolError>,
    check: impl Fn(&Value) -> Result<T, String>,
) -> Result<T, ToolError> {
    let mut revision = None;
    loop {
        let raw = sample(stage, instructions, revision.as_ref().unwrap_or(packet))?;
        let result = crate::extract_json_object(&raw)
            .ok_or_else(|| "Expected a JSON object.".to_owned())
            .and_then(|s| serde_json::from_str(s).map_err(|e| e.to_string()))
            .and_then(|value| check(&value));
        match result {
            Ok(valid) => return Ok(valid),
            Err(diagnostic) if *repair_available => {
                *repair_available = false;
                revision = Some(
                    json!({"input":packet, "previous_draft":raw, "validation_error":diagnostic,
                    "instruction":"Correct the reported error against the same inputs and schema. Retain valid quotations verbatim, including their wording, punctuation and numbers. Return only the corrected JSON object."}),
                );
            }
            Err(message) => {
                return Err(ToolError {
                    error_code: "TEACH_BACK_INVALID".into(),
                    category: "validation".into(),
                    message,
                })
            }
        }
    }
}

/// The host must supply a fresh, tool-free session for each private sample.
/// Only a verified comparison can produce an interpretation or expression receipt.
pub fn run(
    book: &Book,
    anchors: &[String],
    original_question: Option<&str>,
    visible_course_task: Option<&Value>,
    learner_turn_id: &str,
    expression: &str,
    continuation: Option<TeachBackContinuation<'_>>,
    mut progress: impl FnMut(TeachBackStage),
    mut sample: impl FnMut(TeachBackStage, &str, &Value) -> Result<String, ToolError>,
) -> Result<TeachBackResult, ToolError> {
    let mut repair_available = true;
    progress(TeachBackStage::OrganizeExpression);
    let topic_terms = book.teach_back_topic_terms(original_question, anchors)?;
    let kind = continuation.as_ref().map(|reply| reply.kind);
    let dispute = kind == Some(TeachBackReplyKind::DisputeInterpretation);
    let mut selected_row = None;
    let mut referenced_quotes = Vec::new();
    if let Some(reply) = &continuation {
        let previous = reply.previous;
        let row = previous
            .rows
            .iter()
            .find(|row| row.row_id == reply.row_id)
            .ok_or_else(|| ToolError {
                error_code: "TEACH_BACK_INVALID".into(),
                category: "validation".into(),
                message: "Unknown comparison row.".into(),
            })?;
        let student_edges: Vec<_> = previous
            .student_graph
            .edges
            .iter()
            .filter(|e| row.student_edge_ids.contains(&e.id))
            .collect();
        let student_nodes: Vec<_> = previous
            .student_graph
            .nodes
            .iter()
            .filter(|n| {
                row.student_node_ids.contains(&n.id)
                    || student_edges.iter().any(|e| e.from == n.id || e.to == n.id)
            })
            .collect();
        let book_edges: Vec<_> = previous
            .book_graph
            .edges
            .iter()
            .filter(|e| row.book_edge_ids.contains(&e.id))
            .collect();
        let book_nodes: Vec<_> = previous
            .book_graph
            .nodes
            .iter()
            .filter(|n| {
                row.book_node_ids.contains(&n.id)
                    || book_edges.iter().any(|e| e.from == n.id || e.to == n.id)
            })
            .collect();
        let follow_up = previous
            .follow_up
            .as_ref()
            .filter(|f| f.row_id == row.row_id);
        selected_row = Some(
            json!({"row":row,"book_graph":{"nodes":book_nodes,"edges":book_edges},"student_graph":{"nodes":student_nodes,"edges":student_edges},"follow_up":follow_up}),
        );
        if dispute {
            referenced_quotes.extend(
                student_edges
                    .iter()
                    .flat_map(|r| &r.quotes)
                    .chain(student_nodes.iter().flat_map(|r| &r.quotes))
                    .filter(|q| {
                        q.turn_id == reply.first_turn_id && reply.first_expression.contains(&q.text)
                    })
                    .map(|q| &q.text),
            );
        }
    }
    // The dispute identifies a reading to check. Only the original submission
    // can supply quote text, turn IDs and evidence for that reading.
    let (quote_turn, learner_words) = continuation
        .as_ref()
        .filter(|_| dispute)
        .map(|reply| (reply.first_turn_id, reply.first_expression))
        .unwrap_or((learner_turn_id, expression));
    let mut input = json!({"original_question":original_question, "topic_terms":topic_terms, "expression":learner_words});
    if dispute {
        input["correction_request"] = json!(expression);
        if !referenced_quotes.is_empty() {
            input["disputed_quotes"] = json!(referenced_quotes);
        }
    }
    let organize = match kind {
        Some(TeachBackReplyKind::DisputeInterpretation) => {
            schema_instruction::<DisputedExpressionDraft>(&format!("{ORGANIZE}\n{REREAD}"))
        }
        Some(_) => schema_instruction::<ExpressionDraft>(&format!("{ORGANIZE}\n{REPLY}")),
        None => schema_instruction::<ExpressionDraft>(ORGANIZE),
    };
    let student_graph = sample_checked(
        TeachBackStage::OrganizeExpression,
        &organize,
        &input,
        &mut repair_available,
        &mut sample,
        |value| {
            let mut value = value.clone();
            // Private agents sometimes emit unused optional properties as null.
            // Only absence-valued extras can be dropped. All actual graph data
            // still goes through the same strict deserializer and quote checks.
            if let Some(graph) = value.get_mut("student_graph") {
                for field in ["nodes", "edges"] {
                    for item in graph
                        .get_mut(field)
                        .and_then(Value::as_array_mut)
                        .into_iter()
                        .flatten()
                    {
                        if let Some(fields) = item.as_object_mut() {
                            fields.retain(|_, value| !value.is_null());
                        }
                    }
                }
            }
            if let Some(edges) = value
                .pointer("/student_graph/edges")
                .and_then(Value::as_array)
            {
                if let Some(index) = edges.iter().position(|e| e.get("id").is_none()) {
                    return Err(format!("student_graph.edges[{index}] lacks its required id. Give EVERY edge a unique string id distinct from node IDs, and retain its from/to, relation_type and exact quotes."));
                }
            }
            let draft = if dispute {
                match serde_json::from_value(value).map_err(|e| e.to_string())? {
                    DisputedExpressionDraft::NeedsRevision => return Ok(None),
                    DisputedExpressionDraft::Explanation { student_graph } => {
                        ExpressionDraft::Explanation { student_graph }
                    }
                }
            } else {
                serde_json::from_value(value).map_err(|e| e.to_string())?
            };
            freeze(draft, quote_turn, learner_words)
        },
    )?;
    let Some(student_graph) = student_graph else {
        return Ok(if dispute {
            TeachBackResult::NeedsRevision
        } else {
            TeachBackResult::OrdinaryChat
        });
    };

    progress(TeachBackStage::ComparePassages);
    let terms: Vec<_> = student_graph
        .nodes
        .iter()
        .flat_map(|n| std::iter::once(n.label.as_str()).chain(n.condition.as_deref()))
        .collect();
    let mut evidence = book.teach_back_evidence_for(anchors, &terms)?;
    let instructions = match kind {
        None => schema_instruction::<ComparisonResponse>(&format!(
            "{COMPARE} This is a first expression; do not mark it clarified."
        )),
        Some(TeachBackReplyKind::DisputeInterpretation) => {
            schema_instruction::<ReplyResponse>(&format!("{COMPARE}\n{DISPUTE_CHECK}"))
        }
        Some(TeachBackReplyKind::ReviseUnderstanding) => {
            schema_instruction::<ComparisonResponse>(&format!("{COMPARE}\n{REVISION_CHECK}"))
        }
        Some(TeachBackReplyKind::FollowUp) => schema_instruction::<ReplyResponse>(&format!(
            "{COMPARE}\n{REVISION_CHECK}\n{FOLLOW_UP_CHECK}"
        )),
    };
    let instructions = if visible_course_task.is_some() {
        format!("{instructions}\nThe visible_course_task supplies the conditions of this classroom problem, including changed targets, assumptions and observed settings. Keep those conditions even when the book's illustrative case has different numbers. Use the quoted passage for the rule or measured quantity, then apply it to the task givens, including ordinary arithmetic and comparisons. Say which facts come from the lesson and which from the passage. A changed target alone does not make a derivable result unverified. Preserve whether operations overlap; do not infer serial execution from unequal durations. A lower bound above the target rules it out; a bound below the target alone does not guarantee actual performance. Never attribute the displayed lesson explanation to the student.")
    } else {
        instructions
    };
    let focus_targets = json!({
        "student_edge_ids": student_graph.edges.iter().map(|e| &e.id).collect::<Vec<_>>(),
        "student_node_ids": student_graph.nodes.iter().filter(|n| !student_graph.edges.iter().any(|e| e.from == n.id || e.to == n.id)).map(|n| &n.id).collect::<Vec<_>>()
    });
    let mut read_available = true;
    loop {
        let mut input = json!({"original_question":original_question, "frozen_student_graph":student_graph,
            "focus_targets":focus_targets, "evidence":evidence, "nearby_read_available":read_available});
        if let Some(task) = visible_course_task {
            input["visible_course_task"] = task.clone();
        }
        if let Some(row) = &selected_row {
            input["selected_prior_row"] = row.clone();
            if dispute {
                input["correction_request"] = json!(expression);
                input["first_expression"] = json!(learner_words);
            } else {
                input["new_expression"] = json!(expression);
            }
        }
        let response = sample_checked(
            TeachBackStage::ComparePassages,
            &instructions,
            &input,
            &mut repair_available,
            &mut sample,
            |value| {
                if value.get("kind").is_some() {
                    let disposition =
                        serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
                    match (&disposition, kind) {
                        (
                            ReplyDisposition::NeedsRevision,
                            Some(TeachBackReplyKind::DisputeInterpretation),
                        )
                        | (ReplyDisposition::OrdinaryChat, Some(TeachBackReplyKind::FollowUp)) => {}
                        _ => {
                            return Err(
                                "This disposition does not apply to the current kind of reply."
                                    .into(),
                            )
                        }
                    }
                    return Ok(ReplyResponse::Disposition(disposition));
                }
                if value.get("read_lids").is_some() {
                    let request: PassageRequest =
                        serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
                    let ids = id_set(request.read_lids.iter().map(String::as_str))?;
                    if !read_available
                        || !(1..=4).contains(&ids.len())
                        || ids.iter().any(|id| !evidence.can_read_more(id))
                    {
                        return Err("Select 1–4 distinct nearby LIDs that are unread or have a next_start_utf16 offset; only one read is available.".into());
                    }
                    return Ok(ReplyResponse::Comparison(ComparisonResponse::Read(request)));
                }
                let draft: ComparisonDraft =
                    serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
                draft.verify(&evidence, &student_graph, kind.is_some() && !dispute)?;
                Ok(ReplyResponse::Comparison(ComparisonResponse::Answer(draft)))
            },
        )?;
        match response {
            ReplyResponse::Disposition(ReplyDisposition::NeedsRevision) => {
                return Ok(TeachBackResult::NeedsRevision)
            }
            ReplyResponse::Disposition(ReplyDisposition::OrdinaryChat) => {
                return Ok(TeachBackResult::OrdinaryChat)
            }
            ReplyResponse::Comparison(ComparisonResponse::Read(request)) => {
                read_available = false;
                for lid in request.read_lids {
                    book.teach_back_read_more(&mut evidence, &lid)?;
                }
            }
            ReplyResponse::Comparison(ComparisonResponse::Answer(draft)) => {
                return Ok(TeachBackResult::Comparison(TeachBackComparison {
                    continuation: None,
                    support_delivery_refs: None,
                    round_id: format!("tb:{learner_turn_id}:r1"),
                    source: draft.source,
                    book_graph: draft.book_graph,
                    student_graph,
                    rows: draft.rows,
                    side_notes: draft.side_notes,
                    follow_up: draft.follow_up,
                }));
            }
        }
    }
}

/// Build each private call from scratch; no parent answer, history or mounted tools.
pub fn sample_model(
    adapter: &dyn crate::ModelAdapter,
    stage: TeachBackStage,
    instructions: &str,
    packet: &Value,
) -> Result<(String, u32), crate::AdapterError> {
    let messages = [
        crate::Message::system(instructions),
        crate::Message::user(packet.to_string()),
    ];
    let plan = crate::AgentRequestPlan::for_ad_hoc(adapter.model_runtime_profile(), &messages, &[]);
    if !plan.active_context.fits {
        return Err(crate::AdapterError {
            spend_stop: None,
            message: "Teach-back input exceeds the model's context budget.".into(),
        });
    }
    let purpose = match stage {
        TeachBackStage::OrganizeExpression => "teach_back_expression",
        TeachBackStage::ComparePassages => "teach_back_comparison",
    };
    let _scope = crate::run_events::purpose(adapter, purpose);
    let response = adapter.chat(&plan)?;
    let text = response
        .text
        .filter(|_| response.tool_calls.is_empty())
        .unwrap_or_default();
    Ok((text, response.usage_total_tokens.unwrap_or(0)))
}
