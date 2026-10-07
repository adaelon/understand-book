use crate::tool_registry::{ToolOutputPolicy, ToolResultPolicy};
use crate::{Message, Role, ToolCall};
use book_tool_contracts::{validate_input, BookToolId, BookToolInput};
use read_tools::{Book, EvidenceRange, SearchTextResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub const TOOL_RESULT_ENVELOPE_VERSION: &str = "tool_result_envelope.v1";
pub const ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES: usize = 48 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistoricalToolStatus {
    Ok,
    Error,
    LegacyUnparsed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoricalToolReceipt {
    pub version: String,
    pub tool: String,
    pub locator_args: Value,
    pub status: HistoricalToolStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepted_evidence: Vec<EvidenceRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    pub opaque_result_digest: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolResultStatus {
    Ok,
    Partial,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ToolContinuation {
    ToolCursor {
        tool: String,
        arguments: Value,
    },
    NextCall {
        tool: String,
        arguments: Value,
        reason: String,
    },
    RefineCall {
        tool: String,
        arguments: Value,
        guidance: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ToolResultEnvelope {
    pub version: String,
    pub status: ToolResultStatus,
    pub model_body: Value,
    pub receipt: HistoricalToolReceipt,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation: Option<ToolContinuation>,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolResultDraft {
    pub status: ToolResultStatus,
    pub model_body: Value,
    pub truncated: bool,
    pub continuation: Option<ToolContinuation>,
    pub evidence_arguments: String,
}

impl ToolResultDraft {
    pub fn model_body_json(&self) -> String {
        serde_json::to_string(&self.model_body).unwrap_or_else(|_| "null".into())
    }

    pub fn into_envelope(self, receipt: HistoricalToolReceipt) -> ToolResultEnvelope {
        ToolResultEnvelope {
            version: TOOL_RESULT_ENVELOPE_VERSION.into(),
            status: self.status,
            model_body: self.model_body,
            receipt,
            truncated: self.truncated,
            continuation: self.continuation,
        }
    }
}

fn json_len(value: &Value) -> usize {
    serde_json::to_vec(value).map_or(0, |bytes| bytes.len())
}

fn next_search_cursor(tool: &str, arguments: &str, body: &Value) -> Option<ToolContinuation> {
    let cursor = body.get("next_cursor")?.as_str()?;
    let mut next_arguments = serde_json::from_str::<Value>(arguments).ok()?;
    next_arguments
        .as_object_mut()?
        .insert("cursor".into(), Value::String(cursor.into()));
    Some(ToolContinuation::ToolCursor {
        tool: tool.into(),
        arguments: next_arguments,
    })
}

fn pop_named_array(value: &mut Value, target: &str) -> bool {
    match value {
        Value::Object(object) => {
            if let Some(array) = object.get_mut(target).and_then(Value::as_array_mut) {
                if !array.is_empty() {
                    array.pop();
                    return true;
                }
            }
            object
                .values_mut()
                .any(|child| pop_named_array(child, target))
        }
        Value::Array(array) => array.iter_mut().any(|child| pop_named_array(child, target)),
        _ => false,
    }
}

fn pop_any_array(value: &mut Value) -> bool {
    match value {
        Value::Object(object) => object.values_mut().any(pop_any_array),
        Value::Array(array) => {
            if !array.is_empty() {
                array.pop();
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

fn truncate_one_string(value: &mut Value, field: Option<&str>) -> bool {
    const PROTECTED: &[&str] = &[
        "status",
        "error_code",
        "category",
        "version",
        "source_revision",
        "lid",
        "start_lid",
        "end_lid",
        "next_cursor",
    ];
    match value {
        Value::Object(object) => object
            .iter_mut()
            .any(|(key, child)| truncate_one_string(child, Some(key.as_str()))),
        Value::Array(array) => array
            .iter_mut()
            .any(|child| truncate_one_string(child, None)),
        Value::String(text)
            if !field.is_some_and(|field| PROTECTED.contains(&field))
                && text.chars().count() > 32 =>
        {
            let keep = (text.chars().count() / 2).max(16);
            *text = format!(
                "{}...[truncated]",
                text.chars().take(keep).collect::<String>()
            );
            true
        }
        _ => false,
    }
}

fn bounded_value(mut value: Value, limit: usize, drop_order: &[&str]) -> Value {
    if json_len(&value) <= limit {
        return value;
    }
    loop {
        let changed = drop_order
            .iter()
            .any(|field| pop_named_array(&mut value, field))
            || truncate_one_string(&mut value, None)
            || pop_any_array(&mut value);
        if json_len(&value) <= limit {
            return value;
        }
        if !changed {
            break;
        }
    }
    let summary = json!({
        "projection": "bounded_summary",
        "message": "tool result exceeded the model-body budget; use continuation"
    });
    if json_len(&summary) <= limit {
        summary
    } else {
        Value::Null
    }
}

fn refine_continuation(tool: &str, arguments: &str, guidance: &str) -> ToolContinuation {
    ToolContinuation::RefineCall {
        tool: tool.into(),
        arguments: serde_json::from_str(arguments).unwrap_or_else(|_| json!({})),
        guidance: guidance.into(),
    }
}

fn first_citation_lid(value: &Value) -> Option<&str> {
    value
        .get("citations")?
        .as_array()?
        .first()?
        .get("lid")?
        .as_str()
}

fn project_text(arguments: &str, raw: &Value, limit: usize, book: &Book) -> ToolResultDraft {
    let input = serde_json::from_str::<Value>(arguments)
        .ok()
        .and_then(|value| validate_input(BookToolId::Text, value).ok());
    let Some(BookToolInput::Text(input)) = input else {
        return ToolResultDraft {
            status: ToolResultStatus::Partial,
            model_body: bounded_value(raw.clone(), limit, &[]),
            truncated: true,
            continuation: Some(refine_continuation(
                "book.text",
                arguments,
                "retry with a narrower LID range",
            )),
            evidence_arguments: arguments.into(),
        };
    };
    let end_lid = input.end_lid.as_deref().unwrap_or(&input.lid);
    let start = book
        .base
        .lid_nodes
        .iter()
        .find(|node| node.lid == input.lid);
    let end = book.base.lid_nodes.iter().find(|node| node.lid == end_lid);
    let mut leaves = match (start, end) {
        (Some(start), Some(end)) => book
            .base
            .lid_nodes
            .iter()
            .filter(|node| {
                node.children.is_empty()
                    && node.span.start >= start.span.start
                    && node.span.end <= end.span.end
            })
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    leaves.sort_by_key(|node| (node.span.start, node.span.end));

    let Some(first) = leaves.first() else {
        return ToolResultDraft {
            status: ToolResultStatus::Partial,
            model_body: bounded_value(raw.clone(), limit, &[]),
            truncated: true,
            continuation: Some(refine_continuation(
                "book.text",
                arguments,
                "retry with a narrower child LID",
            )),
            evidence_arguments: arguments.into(),
        };
    };

    let mut best: Option<(usize, Value)> = None;
    for (index, last) in leaves.iter().enumerate() {
        let Ok(text) = book.text(
            &first.lid,
            (first.lid != last.lid).then_some(last.lid.as_str()),
        ) else {
            break;
        };
        let candidate = json!({"lid": first.lid, "text": text});
        if json_len(&candidate) > limit {
            break;
        }
        best = Some((index, candidate));
    }

    let Some((last_index, model_body)) = best else {
        return ToolResultDraft {
            status: ToolResultStatus::Partial,
            model_body: bounded_value(raw.clone(), limit, &[]),
            truncated: true,
            continuation: Some(refine_continuation(
                "book.text",
                arguments,
                "the selected LID is too large; request a narrower child LID or nearby context",
            )),
            evidence_arguments: arguments.into(),
        };
    };
    let included_end = &leaves[last_index].lid;
    let evidence_arguments = serde_json::to_string(&json!({
        "lid": first.lid,
        "end_lid": included_end,
    }))
    .unwrap_or_else(|_| arguments.into());
    let continuation = leaves
        .get(last_index + 1)
        .map(|next| ToolContinuation::NextCall {
            tool: "book.text".into(),
            arguments: json!({"lid": next.lid, "end_lid": leaves.last().unwrap().lid}),
            reason: "continue the omitted LID range".into(),
        });
    ToolResultDraft {
        status: ToolResultStatus::Partial,
        model_body,
        truncated: true,
        continuation,
        evidence_arguments,
    }
}

fn project_search(arguments: &str, raw: &Value, limit: usize) -> ToolResultDraft {
    let Ok(mut projected) = serde_json::from_value::<SearchTextResult>(raw.clone()) else {
        return ToolResultDraft {
            status: ToolResultStatus::Partial,
            model_body: bounded_value(raw.clone(), limit, &["occurrences", "section_counts"]),
            truncated: true,
            continuation: Some(refine_continuation(
                "book.search_text",
                arguments,
                "retry with a smaller page_size or narrower scope",
            )),
            evidence_arguments: arguments.into(),
        };
    };
    let original = projected.clone();
    while serde_json::to_vec(&projected).map_or(0, |bytes| bytes.len()) > limit
        && !projected.occurrences.is_empty()
    {
        projected.occurrences.pop();
    }
    if serde_json::to_vec(&projected).map_or(0, |bytes| bytes.len()) > limit {
        projected.section_counts.clear();
    }
    projected.exhaustive = false;
    projected.next_cursor = None;
    while serde_json::to_vec(&projected).map_or(0, |bytes| bytes.len()) > limit
        && projected
            .occurrences
            .last()
            .is_some_and(|occurrence| occurrence.excerpt.chars().count() > 32)
    {
        let occurrence = projected
            .occurrences
            .last_mut()
            .expect("last occurrence was checked");
        let keep = (occurrence.excerpt.chars().count() / 2).max(16);
        occurrence.excerpt = format!(
            "{}...[truncated]",
            occurrence.excerpt.chars().take(keep).collect::<String>()
        );
    }
    let model_body = bounded_value(
        serde_json::to_value(&projected).unwrap_or(Value::Null),
        limit,
        &["section_counts", "occurrences"],
    );
    let kept = model_body
        .get("occurrences")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let omitted = original.occurrences.get(kept);
    let continuation = omitted
        .and_then(|occurrence| {
            let mut next = serde_json::from_str::<Value>(arguments).ok()?;
            let object = next.as_object_mut()?;
            object.remove("cursor");
            object.insert("page_size".into(), Value::from(1));
            object.insert("scope".into(), json!({"within_lid": occurrence.start_lid}));
            Some(ToolContinuation::NextCall {
                tool: "book.search_text".into(),
                arguments: next,
                reason: "continue from the first occurrence omitted by the model-body budget"
                    .into(),
            })
        })
        .or_else(|| next_search_cursor("book.search_text", arguments, raw))
        .or_else(|| {
            Some(refine_continuation(
                "book.search_text",
                arguments,
                "retry with a smaller page_size or narrower scope",
            ))
        });
    ToolResultDraft {
        status: ToolResultStatus::Partial,
        model_body,
        truncated: true,
        continuation,
        evidence_arguments: arguments.into(),
    }
}

fn project_structured(
    tool: &str,
    arguments: &str,
    raw: &Value,
    limit: usize,
    result_policy: ToolResultPolicy,
) -> ToolResultDraft {
    let drop_order = match result_policy {
        ToolResultPolicy::QueryResponse => &[
            "model_supplement",
            "suggested_probing",
            "related_concepts",
            "evidence_chain",
            "support",
            "bindings",
            "citations",
        ][..],
        ToolResultPolicy::ProfileProjection => &[
            "warnings",
            "projections",
            "presets",
            "slots",
            "facts",
            "items",
        ][..],
        ToolResultPolicy::NavigationProjection => {
            &["warnings", "frontier", "route", "questions", "key_stops"][..]
        }
        ToolResultPolicy::ArtifactProjection => &["relations", "records", "hits", "artifacts"][..],
        _ => &[
            "warnings",
            "entries",
            "items",
            "occurrences",
            "references",
            "questions",
        ][..],
    };
    let model_body = bounded_value(raw.clone(), limit, drop_order);
    let continuation = first_citation_lid(raw)
        .map(|lid| ToolContinuation::NextCall {
            tool: "book.text".into(),
            arguments: json!({"lid": lid}),
            reason: "read exact evidence omitted by the bounded query result".into(),
        })
        .or_else(|| {
            Some(refine_continuation(
                tool,
                arguments,
                "retry with a narrower target, scope, stage, or field selection",
            ))
        });
    ToolResultDraft {
        status: ToolResultStatus::Partial,
        model_body,
        truncated: true,
        continuation,
        evidence_arguments: arguments.into(),
    }
}

fn project_error(arguments: &str, raw: &Value, limit: usize) -> ToolResultDraft {
    let model_body = bounded_value(raw.clone(), limit, &[]);
    ToolResultDraft {
        status: ToolResultStatus::Error,
        truncated: model_body != *raw,
        model_body,
        continuation: None,
        evidence_arguments: arguments.into(),
    }
}

fn project_selected_result(arguments: &str, raw: &Value, limit: usize) -> ToolResultDraft {
    // This observation is atomic: trimming its state or text can change a number's meaning.
    let mut body = serde_json::Map::new();
    for key in ["candidate_id", "status", "error_code", "category", "errors", "environment", "environment_name", "recorded_environments", "missing_environments", "reading"] {
        if let Some(value) = raw.get(key) {
            body.insert(key.into(), value.clone());
        }
    }
    body.insert("projection".into(), json!("selected_result"));
    let body = Value::Object(body);
    if json_len(&body) <= limit {
        return ToolResultDraft {
            status: if raw.get("error_code").is_some() { ToolResultStatus::Error } else { ToolResultStatus::Ok },
            model_body: body,
            truncated: false,
            continuation: None,
            evidence_arguments: arguments.into(),
        };
    }
    let guidance = "Complete reading and its context exceed the budget. Narrow read_selector or split the requested result; no partial reading was returned.";
    ToolResultDraft {
        status: ToolResultStatus::Partial,
        model_body: bounded_value(json!({"error_code":"PRESENTATION_READING_BUDGET","message":guidance}), limit, &[]),
        truncated: true,
        continuation: Some(refine_continuation("presentation.author", arguments, guidance)),
        evidence_arguments: arguments.into(),
    }
}

// Large whole-document DOM excerpts must not evict the observed reading positions.
fn project_preview(arguments: &str, raw: &Value, limit: usize) -> ToolResultDraft {
    let mut body = raw.clone();
    if let Some(observations) = body["observations"].as_array_mut() {
        for observation in observations {
            let visible = observation["dom"]["visible_text"].clone();
            observation.as_object_mut().unwrap().remove("dom");
            observation.as_object_mut().unwrap().remove("layout");
            observation["visible_text"] = visible;
        }
    }
    // Only viewport text is expendable here; action/position/issue bindings stay intact.
    while json_len(&body) > limit {
        let changed = body["observations"].as_array_mut().is_some_and(|observations| {
            observations.iter_mut().any(|o| truncate_one_string(&mut o["visible_text"], None))
        });
        if !changed { break; }
    }
    let fits = json_len(&body) <= limit;
    ToolResultDraft {
        status: if fits && raw.get("error_code").is_some() { ToolResultStatus::Error } else { ToolResultStatus::Partial },
        model_body: if fits { body } else { bounded_value(json!({"error_code":"PRESENTATION_PREVIEW_BUDGET","message":"Preview positions and issues exceed the result budget; use fewer actions per preview."}), limit, &[]) },
        truncated: true,
        continuation: Some(refine_continuation("presentation.author", arguments, "Use fewer actions or read_selector for the relevant visible region; full DOM/layout was omitted.")),
        evidence_arguments: arguments.into(),
    }
}

// Editable source must remain a contiguous prefix; generic string truncation would
// leave next_offset pointing past omitted code.
fn project_presentation_source(arguments: &str, raw: &Value, limit: usize) -> ToolResultDraft {
    let source: Vec<char> = raw["text"].as_str().unwrap_or_default().chars().collect();
    let offset = raw["offset"].as_u64().unwrap_or_default() as usize;
    let total = raw.get("end_offset").unwrap_or(&raw["total_characters"]).as_u64().unwrap_or_default() as usize;
    let mut body = raw.clone();
    // A file read asks for editable source. Repeating the page's full prose in
    // each bounded chunk starved real EX11 revisions of code. The metadata read
    // (no explicit file) still returns it; keep all asset and state metadata here.
    if serde_json::from_str::<Value>(arguments).ok()
        .is_some_and(|args| args.get("file").and_then(Value::as_str).is_some())
    {
        if let Some(object) = body.as_object_mut() { object.remove("readable_content"); }
    }
    let mut low = 0;
    let mut high = source.len();
    while low < high {
        let count = (low + high + 1) / 2;
        body["text"] = json!(source[..count].iter().collect::<String>());
        body["next_offset"] = json!((offset + count < total).then_some(offset + count));
        body["chunk_characters"] = json!(count);
        if json_len(&body) <= limit { low = count; } else { high = count - 1; }
    }
    if low == 0 {
        let guidance = "Page metadata and source exceed the remaining output budget; retry this read in a separate call. No source was returned.";
        return ToolResultDraft {
            status: ToolResultStatus::Partial,
            model_body: bounded_value(json!({"error_code":"PRESENTATION_SOURCE_BUDGET","message":guidance}), limit, &[]),
            truncated: true,
            continuation: Some(refine_continuation("presentation.author", arguments, guidance)),
            evidence_arguments: arguments.into(),
        };
    }
    body["text"] = json!(source[..low].iter().collect::<String>());
    body["next_offset"] = json!((offset + low < total).then_some(offset + low));
    body["chunk_characters"] = json!(low);
    let continuation = (offset + low < total).then(|| {
        let mut next: Value = serde_json::from_str(arguments).unwrap_or_else(|_| json!({}));
        next["offset"] = json!(offset + low);
        if raw.get("end_offset").is_some() { next["length"] = json!(total - offset - low); }
        ToolContinuation::NextCall {
            tool: "presentation.author".into(), arguments: next,
            reason: "continue editable source at the first omitted character".into(),
        }
    });
    ToolResultDraft {
        status: ToolResultStatus::Partial, model_body: body, truncated: true,
        continuation, evidence_arguments: arguments.into(),
    }
}

fn project_presentation_search(arguments: &str, raw: &Value, limit: usize) -> ToolResultDraft {
    let mut body = raw.clone();
    while json_len(&body) > limit {
        let Some(omitted) = body["matches"].as_array_mut().and_then(Vec::pop) else {
            let guidance = "Search context exceeds the remaining output budget; retry separately or use a shorter query.";
            return ToolResultDraft { status:ToolResultStatus::Partial,
                model_body:bounded_value(json!({"error_code":"PRESENTATION_SOURCE_BUDGET","message":guidance}),limit,&[]),
                truncated:true,continuation:Some(refine_continuation("presentation.author",arguments,guidance)),evidence_arguments:arguments.into() };
        };
        body["next_offset"] = omitted["offset"].clone();
    }
    let continuation = body["next_offset"].as_u64().map(|offset| {
        let mut next: Value = serde_json::from_str(arguments).unwrap_or_else(|_| json!({}));
        next["offset"] = json!(offset);
        ToolContinuation::NextCall { tool:"presentation.author".into(), arguments:next, reason:"continue source search at the first omitted match".into() }
    });
    let truncated = body != *raw;
    ToolResultDraft {status:if truncated {ToolResultStatus::Partial} else {ToolResultStatus::Ok}, model_body:body,
        truncated,continuation,evidence_arguments:arguments.into()}
}

pub(crate) fn project_tool_result(
    tool: &str,
    arguments: &str,
    raw_result: &str,
    output_policy: ToolOutputPolicy,
    remaining_turn_body_bytes: usize,
    book: &Book,
) -> ToolResultDraft {
    let limit = output_policy
        .max_model_body_bytes
        .min(remaining_turn_body_bytes);
    let raw = serde_json::from_str::<Value>(raw_result).unwrap_or_else(|error| {
        json!({
            "error_code": "TOOL_RESULT_INVALID_JSON",
            "category": "internal",
            "message": error.to_string(),
        })
    });
    if tool == "presentation.author" && raw.get("reading").is_some_and(|value| !value.is_null()) {
        return project_selected_result(arguments, &raw, limit);
    }
    if tool == "presentation.author" && raw["observations"].is_array() && json_len(&raw) > limit {
        return project_preview(arguments, &raw, limit);
    }
    if raw.get("error_code").and_then(Value::as_str).is_some() {
        return project_error(arguments, &raw, limit);
    }
    if tool == "presentation.author" && raw["status"] == "source_matches" {
        return project_presentation_search(arguments, &raw, limit);
    }
    if json_len(&raw) <= limit {
        return ToolResultDraft {
            status: ToolResultStatus::Ok,
            continuation: (tool == "book.search_text")
                .then(|| next_search_cursor(tool, arguments, &raw))
                .flatten(),
            model_body: raw,
            truncated: false,
            evidence_arguments: arguments.into(),
        };
    }
    match tool {
        "presentation.author" if raw["status"] == "version_read" => project_presentation_source(arguments, &raw, limit),
        "book.text" => project_text(arguments, &raw, limit, book),
        "book.search_text" => project_search(arguments, &raw, limit),
        _ => project_structured(tool, arguments, &raw, limit, output_policy.result_policy),
    }
}

#[derive(Debug, Clone)]
struct ActiveToolResult {
    envelope: ToolResultEnvelope,
    sampled: bool,
    retain_model_body: bool,
    sequence: u64,
    saved_author_call: Option<SavedAuthorCall>,
}

/// A current-run save receipt is the authority for this model-only projection.
/// Keep the latest patch's exact edits until an observed child save supplies the
/// next modification context. Independent writes never imply supersession.
#[derive(Debug, Clone)]
struct SavedAuthorCall {
    candidate_id: String,
    parent_candidate_id: Option<String>,
    arguments: Value,
}

impl SavedAuthorCall {
    fn from_result(call: &ToolCall, envelope: &ToolResultEnvelope) -> Option<Self> {
        if call.name != "presentation.author"
            || envelope.status != ToolResultStatus::Ok
            || envelope.model_body["status"] != "candidate_saved"
        {
            return None;
        }
        let candidate_id = envelope.model_body["candidate_id"].as_str()
            .filter(|id| !id.is_empty())?.to_owned();
        let mut arguments: Value = serde_json::from_str(&call.arguments).ok()?;
        if !matches!(arguments["operation"].as_str(), Some("write" | "patch")) {
            return None;
        }
        let parent_candidate_id = arguments["candidate_id"].as_str().map(str::to_owned);
        let object = arguments.as_object_mut()?;
        let mut omitted = Vec::new();
        for field in ["html", "readable_content"] {
            if object.remove(field).is_some() { omitted.push(field); }
        }
        object.insert("saved_source".into(), json!({
            "candidate_id":candidate_id, "file":"index.html", "omitted_fields":omitted,
            "read":{"operation":"read","candidate_id":candidate_id},
            "note":"Saved source omitted from model history after its successful receipt was sampled. Read this candidate in the current run for source and readable_content; this locator grants no preview or delivery eligibility."
        }));
        Some(Self { candidate_id, parent_candidate_id, arguments })
    }

    fn retire_edits(&mut self) {
        if let Some(edits) = self.arguments.as_object_mut().unwrap().remove("edits") {
            self.arguments["saved_source"]["applied_edits"] = json!(edits.as_array().map_or(0, Vec::len));
            self.arguments["saved_source"]["omitted_fields"].as_array_mut().unwrap().push(json!("edits"));
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ActiveToolResultLedger {
    by_call_id: HashMap<String, ActiveToolResult>,
    next_sequence: u64,
}

impl ActiveToolResultLedger {
    pub fn project_result(&mut self, tool: &str, arguments: &str, raw: &str, mut policy: ToolOutputPolicy, book: &Book) -> ToolResultDraft {
        // An editable file can use the existing active-result budget; a small
        // result must not evict earlier source just to reserve its maximum size.
        if tool == "presentation.author" && serde_json::from_str::<Value>(arguments).ok().is_some_and(|v| v["operation"] == "read") {
            policy.max_model_body_bytes = ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES;
        }
        let projected = project_tool_result(tool, arguments, raw, policy, ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES, book);
        let needed = json_len(&projected.model_body);
        self.make_room_for(needed);
        let available = self.remaining_model_body_bytes();
        if needed <= available { projected }
        else { project_tool_result(tool, arguments, raw, policy, available, book) }
    }

    pub fn remaining_model_body_bytes(&self) -> usize {
        let fresh: usize = self
            .by_call_id
            .values()
            .filter(|result| result.retain_model_body)
            .map(|result| json_len(&result.envelope.model_body))
            .sum();
        ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES.saturating_sub(fresh)
    }

    pub fn make_room_for(&mut self, desired_model_body_bytes: usize) {
        let desired = desired_model_body_bytes.min(ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES);
        while self.remaining_model_body_bytes() < desired {
            let oldest = self
                .by_call_id
                .iter()
                .filter(|(_, result)| result.sampled && result.retain_model_body)
                .min_by_key(|(_, result)| result.sequence)
                .map(|(call_id, _)| call_id.clone());
            let Some(oldest) = oldest else {
                break;
            };
            if let Some(result) = self.by_call_id.get_mut(&oldest) {
                result.retain_model_body = false;
            }
        }
    }

    pub fn insert(&mut self, call_id: impl Into<String>, envelope: ToolResultEnvelope) {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.by_call_id.insert(
            call_id.into(),
            ActiveToolResult {
                envelope,
                sampled: false,
                retain_model_body: true,
                sequence,
                saved_author_call: None,
            },
        );
    }

    pub fn insert_call(&mut self, call: &ToolCall, envelope: ToolResultEnvelope) {
        let saved_author_call = SavedAuthorCall::from_result(call, &envelope);
        self.insert(call.id.clone(), envelope);
        self.by_call_id.get_mut(&call.id).unwrap().saved_author_call = saved_author_call;
    }

    pub fn project_messages(&self, messages: &mut [Message]) {
        // Only project complete pairs still present in this history window.
        // The ledger is run-local and is never reconstructed from old messages.
        let paired: HashSet<_> = messages.iter().filter(|m| m.role == Role::Tool)
            .filter_map(|m| m.tool_call_id.clone()).collect();
        for call in messages.iter_mut().filter(|m| m.role == Role::Assistant)
            .flat_map(|m| &mut m.tool_calls)
        {
            if let Some(saved) = self.by_call_id.get(&call.id)
                .filter(|result| result.sampled && paired.contains(&call.id))
                .and_then(|result| result.saved_author_call.as_ref())
            {
                call.arguments = saved.arguments.to_string();
            }
        }
        for message in messages
            .iter_mut()
            .filter(|message| message.role == Role::Tool)
        {
            let Some(result) = message
                .tool_call_id
                .as_deref()
                .and_then(|call_id| self.by_call_id.get(call_id))
            else {
                continue;
            };
            let mut envelope = result.envelope.clone();
            if !result.retain_model_body {
                envelope.model_body = Value::Null;
            }
            message.content = Some(
                serde_json::to_string(&envelope).unwrap_or_else(|_| {
                    r#"{"version":"tool_result_envelope.v1","status":"error","model_body":null,"truncated":true}"#.into()
                }),
            );
        }
    }

    pub fn mark_projected_fresh_results_sampled(&mut self) {
        for result in self.by_call_id.values_mut() {
            result.sampled = true;
        }
        let parents: HashSet<_> = self.by_call_id.values()
            .filter_map(|result| result.saved_author_call.as_ref())
            .filter_map(|saved| saved.parent_candidate_id.clone()).collect();
        for saved in self.by_call_id.values_mut().filter_map(|r| r.saved_author_call.as_mut()) {
            if parents.contains(&saved.candidate_id) { saved.retire_edits(); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn author_save(ledger: &mut ActiveToolResultLedger, messages: &mut Vec<Message>, id: &str, args: Value, body: Value) {
        let call = ToolCall { id:id.into(), name:"presentation.author".into(), arguments:args.to_string() };
        let envelope = ToolResultEnvelope {
            version:TOOL_RESULT_ENVELOPE_VERSION.into(),
            status:if body.get("error_code").is_some() { ToolResultStatus::Error } else { ToolResultStatus::Ok },
            model_body:body, receipt:receipt("presentation.author"), truncated:false, continuation:None,
        };
        ledger.insert_call(&call, envelope.clone());
        messages.push(Message { role:Role::Assistant, content:None, tool_calls:vec![call], tool_call_id:None, provider_continuation:None });
        messages.push(Message { role:Role::Tool, content:Some(serde_json::to_string(&envelope.receipt).unwrap()), tool_calls:vec![], tool_call_id:Some(id.into()), provider_continuation:None });
    }

    fn projected_author_args(ledger: &ActiveToolResultLedger, messages: &[Message], id: &str) -> Value {
        let mut projected = messages.to_vec();
        ledger.project_messages(&mut projected);
        serde_json::from_str(&projected.iter().flat_map(|m| &m.tool_calls).find(|c| c.id == id).unwrap().arguments).unwrap()
    }

    #[test]
    fn ex13_author_source_requires_observed_save_and_keeps_receipts_after_eviction() {
        let mut ledger = ActiveToolResultLedger::default();
        let mut messages = vec![Message::user("Keep both requested works")];
        let args = json!({"operation":"write","html":"<p>source</p>","readable_content":"source", "title":"work",
            "based_on":{"presentation_id":"p","revision":2},"new_object":false,"source_ref_ids":["source-a"],"asset_refs":["asset-a"],
            "libraries":["konva"],"assumptions":["fixed input"],"state_contract":{"x":"number"},"initial_state":{"x":1}});
        for (id, body) in [
            ("failed", json!({"error_code":"PRESENTATION_AUTHORING_FAILED"})),
            ("missing-landing", json!({"status":"candidate_saved"})),
            ("wrong-status", json!({"status":"version_read","candidate_id":"c-old"})),
            ("saved", json!({"status":"candidate_saved","candidate_id":"c1"})),
        ] { author_save(&mut ledger, &mut messages, id, args.clone(), body); }
        assert_eq!(projected_author_args(&ledger, &messages, "saved"), args, "first receipt sampling must keep source");
        ledger.mark_projected_fresh_results_sampled();
        for id in ["failed", "missing-landing", "wrong-status"] {
            assert_eq!(projected_author_args(&ledger, &messages, id), args);
        }
        let compact = projected_author_args(&ledger, &messages, "saved");
        assert!(compact.get("html").is_none() && compact.get("readable_content").is_none());
        for field in ["operation","title","based_on","new_object","source_ref_ids","asset_refs","libraries","assumptions","state_contract","initial_state"] {
            assert_eq!(compact[field], args[field]);
        }
        assert_eq!(compact["saved_source"]["read"], json!({"operation":"read","candidate_id":"c1"}));
        // The immutable candidate remains a recovery target after result-body
        // eviction. Neither receipt identity nor raw history is rewritten.
        ledger.make_room_for(ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES);
        let mut projected = messages.clone();
        ledger.project_messages(&mut projected);
        let envelope: ToolResultEnvelope = serde_json::from_str(projected.last().unwrap().content.as_deref().unwrap()).unwrap();
        assert!(envelope.model_body.is_null());
        assert_eq!(envelope.receipt, receipt("presentation.author"));
        assert_eq!(projected_author_args(&ledger, &messages, "saved"), compact);
        assert_eq!(projected_author_args(&ActiveToolResultLedger::default(), &messages, "saved"), args, "a new run cannot recover eligibility from history");
        let incomplete = &messages[..messages.len() - 1];
        assert_eq!(projected_author_args(&ledger, incomplete, "saved"), args, "unpaired call must stay intact");
    }

    #[test]
    fn ex13_patch_chain_retires_edits_only_after_observed_child_save() {
        let mut ledger = ActiveToolResultLedger::default();
        let mut messages = vec![Message::user("Revise this exact version")];
        let edits = json!([{"old_text":"original","new_text":"corrected"}]);
        author_save(&mut ledger, &mut messages, "patch-1", json!({"operation":"patch","reference":{"presentation_id":"p","revision":2},"edits":edits,"readable_content":"corrected"}), json!({"status":"candidate_saved","candidate_id":"p1"}));
        ledger.mark_projected_fresh_results_sampled();
        let recent = projected_author_args(&ledger, &messages, "patch-1");
        assert_eq!(recent["edits"], edits);
        assert!(recent.get("readable_content").is_none());
        author_save(&mut ledger, &mut messages, "independent", json!({"operation":"write","html":"different work","readable_content":"different"}), json!({"status":"candidate_saved","candidate_id":"other"}));
        author_save(&mut ledger, &mut messages, "failed-child", json!({"operation":"patch","candidate_id":"p1","edits":edits}), json!({"error_code":"PRESENTATION_AUTHORING_FAILED"}));
        ledger.mark_projected_fresh_results_sampled();
        assert_eq!(projected_author_args(&ledger, &messages, "patch-1"), recent);
        let child = json!({"operation":"patch","candidate_id":"p1","edits":[{"old_text":"corrected","new_text":"final"}]});
        author_save(&mut ledger, &mut messages, "patch-2", child.clone(), json!({"status":"candidate_saved","candidate_id":"p2"}));
        assert_eq!(projected_author_args(&ledger, &messages, "patch-1"), recent, "unobserved child must not retire recent edits");
        ledger.mark_projected_fresh_results_sampled();
        let retired = projected_author_args(&ledger, &messages, "patch-1");
        assert!(retired.get("edits").is_none());
        assert_eq!(retired["reference"], recent["reference"]);
        assert_eq!(retired["saved_source"]["candidate_id"], "p1");
        assert_eq!(retired["saved_source"]["applied_edits"], 1);
        let latest = projected_author_args(&ledger, &messages, "patch-2");
        assert_eq!(latest["candidate_id"], "p1");
        assert_eq!(latest["saved_source"]["candidate_id"], "p2");
        assert_eq!(latest["edits"], child["edits"]);
        ledger.mark_projected_fresh_results_sampled();
        assert_eq!(projected_author_args(&ledger, &messages, "patch-1"), retired, "projection must stay stable");
    }

    #[test]
    fn ex11_explicit_source_read_does_not_repeat_prose_at_expense_of_code() {
        let raw=json!({"status":"version_read","reference":{"presentation_id":"p","revision":1},
            "file":"index.html","offset":4000,"total_characters":8000,"chunk_characters":4000,
            "text":"x".repeat(4000),"next_offset":null,"readable_content":"解释".repeat(1000),
            "asset_refs":["animation-1"],"libraries":[],"state_contract":{"step":"number"}});
        let draft=project_tool_result("presentation.author",r#"{"operation":"read","file":"index.html","offset":4000}"#,&raw.to_string(),policy(ToolResultPolicy::EvidenceProjection,4096),4096,&two_leaf_book());
        assert!(draft.model_body["text"].as_str().unwrap_or_default().len()>3000);
        assert_eq!(draft.model_body["asset_refs"],raw["asset_refs"]);
        assert_eq!(draft.model_body["state_contract"],raw["state_contract"]);
        assert!(json_len(&draft.model_body)<=4096);
        assert!(draft.model_body.get("readable_content").is_none());
        let metadata=project_tool_result("presentation.author",r#"{"operation":"read"}"#,&raw.to_string(),policy(ToolResultPolicy::EvidenceProjection,16384),16384,&two_leaf_book());
        assert_eq!(metadata.model_body["readable_content"],raw["readable_content"]);
    }

    #[test]
    fn ex11_source_projection_continues_without_skipping_chinese_code() {
        let source = "const 标签 = \"学习率\";\n".repeat(500);
        let chars: Vec<char> = source.chars().collect();
        let mut offset = 0;
        let mut reconstructed = String::new();
        let mut shortened = false;
        while offset < chars.len() {
            let end = (offset + 4000).min(chars.len());
            let raw = json!({"status":"version_read", "reference":{"presentation_id":"p","version_id":"v"},
                "file":"index.html","offset":offset,"text":chars[offset..end].iter().collect::<String>(),
                "next_offset":(end < chars.len()).then_some(end),"total_characters":chars.len(),"chunk_characters":4000,
                "readable_content":"可编辑页面说明".repeat(600),"libraries":[{"name":"konva","version":"10.7.0"}]});
            let args = json!({"operation":"read","reference":raw["reference"],"file":"index.html","offset":offset}).to_string();
            let draft = project_tool_result("presentation.author", &args, &raw.to_string(), policy(ToolResultPolicy::EvidenceProjection,4096),4096,&two_leaf_book());
            assert!(json_len(&draft.model_body) <= 4096);
            let text = draft.model_body["text"].as_str().unwrap();
            assert!(!text.is_empty());
            assert!(!text.contains("[truncated]"));
            reconstructed.push_str(text);
            let next = offset + text.chars().count();
            assert_eq!(draft.model_body["next_offset"],json!((next < chars.len()).then_some(next)));
            assert_eq!(draft.model_body["libraries"],raw["libraries"]);
            if let Some(ToolContinuation::NextCall {arguments,..}) = draft.continuation { assert_eq!(arguments["offset"],next); }
            shortened |= next < end;
            offset = next;
        }
        assert!(shortened);
        assert_eq!(reconstructed,source);
    }

    #[test]
    fn ex13_readable_content_ranges_recover_prose_larger_than_the_active_budget() {
        let source = "Readable explanation with parameters and results. ".repeat(1500);
        assert!(source.len() > ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES);
        let mut args = json!({"operation":"read","reference":{"presentation_id":"p","revision":1},"file":"readable_content","offset":0});
        let mut restored = String::new();
        loop {
            let offset = args["offset"].as_u64().unwrap() as usize;
            let raw = json!({"status":"version_read","reference":args["reference"],"file":"readable_content",
                "text":&source[offset..],"offset":offset,"end_offset":source.len(),"total_characters":source.len(),"chunk_characters":source.len()-offset,"next_offset":null});
            let draft = project_tool_result("presentation.author", &args.to_string(), &raw.to_string(),
                policy(ToolResultPolicy::EvidenceProjection, ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES), ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES, &two_leaf_book());
            assert!(json_len(&draft.model_body) <= ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES);
            assert_eq!(draft.model_body["reference"], args["reference"]);
            restored.push_str(draft.model_body["text"].as_str().unwrap());
            match draft.continuation {
                Some(ToolContinuation::NextCall { arguments, .. }) => {
                    assert_eq!(arguments["file"], "readable_content");
                    assert_eq!(arguments["offset"], restored.len());
                    args = arguments;
                }
                None => break,
                other => panic!("expected a recoverable range, got {other:?}"),
            }
        }
        assert_eq!(restored, source);
    }

    #[test]
    fn selected_result_projection_preserves_context_or_omits_whole_reading() {
        let reading = json!({"action_step":2,"selector":"#result","text":"eta=1.2; k=5; w=12.75648; next k=6, w=-13.059072".repeat(8),"page_state":{"eta":1.2,"k":5},"controls":[{"id":"eta","value":"1.2"}]});
        let raw = json!({"candidate_id":"c","environment_name":"desktop-content","status":"preview_environment_recorded","reading":reading,"observations":[{"dom":{"text":"intro".repeat(10000)}}]});
        let project = |limit| project_tool_result("presentation.author",r##"{"operation":"preview","read_selector":"#result"}"##,&raw.to_string(),policy(ToolResultPolicy::EvidenceProjection,limit),limit,&two_leaf_book());
        let full = project(1200);
        assert_eq!(full.model_body["reading"], reading);
        assert!(!full.truncated);
        let small = project(160);
        assert!(small.model_body.get("reading").is_none());
        assert!(small.truncated);
        assert!(small.continuation.is_some());
    }

    #[test]
    fn selected_result_projection_preserves_preview_failures() {
        let raw = json!({"candidate_id":"c","status":"preview_failed","error_code":"PRESENTATION_PREVIEW_FAILED","errors":[{"kind":"candidate_execution","message":"render failed"}],"reading":{"text":"w=12.756480","page_state":{"eta":1.2,"k":5}},"observations":[{"dom":{"text":"intro".repeat(10000)}}]});
        let draft = project_tool_result("presentation.author", "{}", &raw.to_string(), policy(ToolResultPolicy::EvidenceProjection, 1024), 1024, &two_leaf_book());
        assert_eq!(draft.status, ToolResultStatus::Error);
        assert_eq!(draft.model_body["reading"], raw["reading"]);
        assert_eq!(draft.model_body["errors"], raw["errors"]);
        assert_eq!(draft.model_body["error_code"], raw["error_code"]);
    }

    #[test]
    fn ex12_large_preview_keeps_action_positions_and_errors() {
        let scroll = json!({"x":0,"y":1980,"max_y":1980,"viewport_width":960,"viewport_height":720});
        let action = json!({"kind":"scroll","y":99999});
        let raw = json!({"candidate_id":"c","environment_name":"desktop-content","environment":{"width":960,"height":720,"input":"mouse"},"status":"preview_failed","error_code":"PRESENTATION_PREVIEW_FAILED","errors":[{"kind":"result_mismatch","message":"wrong text"}],"observations":[{"step":1,"action":action,"scroll":scroll,"issues":[],"dom":{"text":"intro".repeat(10000),"semantic_text":"intro".repeat(10000),"visible_text":"end ".repeat(3000)},"layout":{"cssLayoutViewport":{"pageY":1980}}}]});
        let draft = project_tool_result("presentation.author", "{}", &raw.to_string(), policy(ToolResultPolicy::EvidenceProjection, 2048), 2048, &two_leaf_book());
        assert_eq!(draft.status, ToolResultStatus::Error);
        assert_eq!(draft.model_body["observations"][0]["scroll"], scroll);
        assert_eq!(draft.model_body["observations"][0]["action"], action);
        assert_eq!(draft.model_body["observations"][0]["step"], 1);
        assert_eq!(draft.model_body["errors"], raw["errors"]);
        assert_eq!(draft.model_body["candidate_id"], "c");
        assert!(json_len(&draft.model_body) <= 2048);
        let mut selected = raw;
        selected["reading"] = json!({"action_step":1,"text":"END","scroll":scroll});
        let draft = project_tool_result("presentation.author", "{}", &selected.to_string(), policy(ToolResultPolicy::EvidenceProjection, 2048), 2048, &two_leaf_book());
        assert_eq!(draft.model_body["reading"]["scroll"], scroll);
    }
    use base_schema::{LidNode, NodeKind, ReadOnlyBase, Span};
    use book_tool_contracts::{SearchMatchMode, SearchOrder, SearchTextInput};

    fn policy(result_policy: ToolResultPolicy, max_model_body_bytes: usize) -> ToolOutputPolicy {
        ToolOutputPolicy {
            result_policy,
            max_model_body_bytes,
        }
    }

    fn receipt(tool: &str) -> HistoricalToolReceipt {
        HistoricalToolReceipt {
            version: "historical_tool_receipt.v1".into(),
            tool: tool.into(),
            locator_args: json!({}),
            status: HistoricalToolStatus::Ok,
            error_code: None,
            accepted_evidence: Vec::new(),
            source_refs: Vec::new(),
            opaque_result_digest: "digest".into(),
        }
    }

    fn two_leaf_book() -> Book {
        let source = format!("{}{}", "A".repeat(600), "B".repeat(600));
        Book::new(
            ReadOnlyBase {
                book_id: "bounded-text".into(),
                lid_nodes: vec![
                    LidNode {
                        lid: "1".into(),
                        path: vec![1],
                        kind: NodeKind::Chapter,
                        span: Span {
                            start: 0,
                            end: 1200,
                        },
                        children: vec!["1.1".into(), "1.2".into()],
                    },
                    LidNode {
                        lid: "1.1".into(),
                        path: vec![1, 1],
                        kind: NodeKind::Paragraph,
                        span: Span { start: 0, end: 600 },
                        children: Vec::new(),
                    },
                    LidNode {
                        lid: "1.2".into(),
                        path: vec![1, 2],
                        kind: NodeKind::Paragraph,
                        span: Span {
                            start: 600,
                            end: 1200,
                        },
                        children: Vec::new(),
                    },
                ],
                graph_nodes: Vec::new(),
                graph_edges: Vec::new(),
            },
            &source,
        )
    }

    #[test]
    fn tool_result_projection_text_is_bounded_at_lid_boundaries_and_continuable() {
        let book = two_leaf_book();
        let raw = serde_json::to_string(&json!({
            "lid": "1",
            "text": book.text("1", None).unwrap(),
        }))
        .unwrap();

        let draft = project_tool_result(
            "book.text",
            r#"{"lid":"1"}"#,
            &raw,
            policy(ToolResultPolicy::EvidenceProjection, 700),
            700,
            &book,
        );

        assert_eq!(draft.status, ToolResultStatus::Partial);
        assert!(draft.truncated);
        assert!(draft.model_body_json().len() <= 700);
        assert_eq!(draft.model_body["lid"], "1.1");
        assert_eq!(draft.model_body["text"].as_str().unwrap().len(), 600);
        assert_eq!(
            serde_json::from_str::<Value>(&draft.evidence_arguments).unwrap(),
            json!({"lid":"1.1","end_lid":"1.1"})
        );
        assert!(matches!(
            draft.continuation,
            Some(ToolContinuation::NextCall { ref tool, .. }) if tool == "book.text"
        ));
    }

    #[test]
    fn tool_result_projection_search_keeps_valid_json_and_marks_context_truncation() {
        let source = "needle ".repeat(80);
        let end = source.len();
        let book = Book::new(
            ReadOnlyBase {
                book_id: "bounded-search".into(),
                lid_nodes: vec![LidNode {
                    lid: "1.1".into(),
                    path: vec![1, 1],
                    kind: NodeKind::Paragraph,
                    span: Span { start: 0, end },
                    children: Vec::new(),
                }],
                graph_nodes: Vec::new(),
                graph_edges: Vec::new(),
            },
            &source,
        );
        let input = SearchTextInput {
            query: "needle".into(),
            match_mode: SearchMatchMode::Exact,
            scope: None,
            order: SearchOrder::Document,
            cursor: None,
            page_size: 50,
        };
        let raw = serde_json::to_string(&book.search_text(&input).unwrap()).unwrap();
        let arguments = serde_json::to_string(&input).unwrap();

        let draft = project_tool_result(
            "book.search_text",
            &arguments,
            &raw,
            policy(ToolResultPolicy::EvidenceProjection, 1_200),
            1_200,
            &book,
        );

        assert!(draft.truncated);
        assert!(draft.model_body_json().len() <= 1_200);
        let projected: SearchTextResult = serde_json::from_value(draft.model_body).unwrap();
        assert!(projected.occurrences.len() < 50);
        assert!(draft.continuation.is_some());
    }

    #[test]
    fn tool_result_projection_native_search_cursor_is_not_context_truncation() {
        let source = "needle needle";
        let book = Book::new(
            ReadOnlyBase {
                book_id: "native-search-page".into(),
                lid_nodes: vec![LidNode {
                    lid: "1.1".into(),
                    path: vec![1, 1],
                    kind: NodeKind::Paragraph,
                    span: Span {
                        start: 0,
                        end: source.len(),
                    },
                    children: Vec::new(),
                }],
                graph_nodes: Vec::new(),
                graph_edges: Vec::new(),
            },
            source,
        );
        let input = SearchTextInput {
            query: "needle".into(),
            match_mode: SearchMatchMode::Exact,
            scope: None,
            order: SearchOrder::Document,
            cursor: None,
            page_size: 1,
        };
        let raw = serde_json::to_string(&book.search_text(&input).unwrap()).unwrap();
        let arguments = serde_json::to_string(&input).unwrap();

        let draft = project_tool_result(
            "book.search_text",
            &arguments,
            &raw,
            policy(ToolResultPolicy::EvidenceProjection, 16 * 1024),
            16 * 1024,
            &book,
        );

        assert_eq!(draft.status, ToolResultStatus::Ok);
        assert!(!draft.truncated);
        assert!(matches!(
            draft.continuation,
            Some(ToolContinuation::ToolCursor { .. })
        ));
    }

    #[test]
    fn tool_result_projection_query_paper_profile_and_error_are_explicitly_bounded() {
        let book = two_leaf_book();
        let query = json!({
            "status":"complete",
            "answer":"A".repeat(3_000),
            "citations":[{"lid":"1.1","text":"A".repeat(600),"role":"support"}],
            "bindings":[],"support":[],"model_supplement":[]
        });
        let paper = json!({"available":true,"questions":vec!["Q".repeat(800); 20],"warnings":[]});
        let profile = json!({"slots":vec!["S".repeat(800); 20],"presets":[],"projections":[]});
        for (tool, value, result_policy) in [
            ("book.query", query, ToolResultPolicy::QueryResponse),
            (
                "book.paper_reading_guide",
                paper,
                ToolResultPolicy::NavigationProjection,
            ),
            (
                "profile.manifest",
                profile,
                ToolResultPolicy::ProfileProjection,
            ),
        ] {
            let draft = project_tool_result(
                tool,
                "{}",
                &value.to_string(),
                policy(result_policy, 1_024),
                1_024,
                &book,
            );
            assert_eq!(draft.status, ToolResultStatus::Partial);
            assert!(draft.truncated);
            assert!(draft.model_body_json().len() <= 1_024);
            assert!(draft.continuation.is_some());
        }

        let error = json!({
            "error_code":"LID_NOT_FOUND",
            "category":"not_found",
            "message":"M".repeat(5_000),
        });
        let draft = project_tool_result(
            "book.text",
            "{}",
            &error.to_string(),
            policy(ToolResultPolicy::EvidenceProjection, 512),
            512,
            &book,
        );
        assert_eq!(draft.status, ToolResultStatus::Error);
        assert!(draft.truncated);
        assert_eq!(draft.model_body["error_code"], "LID_NOT_FOUND");
        assert!(draft.model_body_json().len() <= 512);
        assert!(draft.continuation.is_none());
    }

    #[test]
    fn tool_result_projection_fresh_body_is_retained_until_budget_pressure_then_receipt_only() {
        let envelope = ToolResultEnvelope {
            version: TOOL_RESULT_ENVELOPE_VERSION.into(),
            status: ToolResultStatus::Ok,
            model_body: json!({"text":"FRESH_BODY"}),
            receipt: receipt("book.text"),
            truncated: false,
            continuation: None,
        };
        let durable = Message {
            provider_continuation: None,
            role: Role::Tool,
            content: Some("RAW_DURABLE_BODY".into()),
            tool_calls: Vec::new(),
            tool_call_id: Some("call-1".into()),
        };
        let mut ledger = ActiveToolResultLedger::default();
        ledger.insert("call-1", envelope);

        let mut first = vec![durable.clone()];
        ledger.project_messages(&mut first);
        assert!(first[0].content.as_deref().unwrap().contains("FRESH_BODY"));
        ledger.mark_projected_fresh_results_sampled();

        let mut second = vec![durable.clone()];
        ledger.project_messages(&mut second);
        let second: ToolResultEnvelope =
            serde_json::from_str(second[0].content.as_deref().unwrap()).unwrap();
        assert_eq!(second.model_body, json!({"text":"FRESH_BODY"}));

        ledger.make_room_for(ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES);
        let mut evicted = vec![durable.clone()];
        ledger.project_messages(&mut evicted);
        let evicted: ToolResultEnvelope =
            serde_json::from_str(evicted[0].content.as_deref().unwrap()).unwrap();
        assert_eq!(evicted.model_body, Value::Null);
        assert_eq!(durable.content.as_deref(), Some("RAW_DURABLE_BODY"));
    }

    #[test]
    fn presentation_edit_budget_keeps_old_source_when_actual_new_result_fits() {
        let mut ledger = ActiveToolResultLedger::default();
        ledger.insert("old-source", ToolResultEnvelope {
            version: TOOL_RESULT_ENVELOPE_VERSION.into(), status: ToolResultStatus::Ok,
            model_body: json!({"text":"x".repeat(40_000)}), receipt: receipt("presentation.author"),
            truncated: false, continuation: None,
        });
        ledger.mark_projected_fresh_results_sampled();
        let result = ledger.project_result("presentation.author", r#"{"operation":"search"}"#,
            r#"{"status":"source_matches","matches":[],"next_offset":null}"#,
            policy(ToolResultPolicy::EvidenceProjection,16384), &two_leaf_book());
        assert!(!result.truncated);
        assert!(ledger.by_call_id["old-source"].retain_model_body, "a tiny response must not evict 40KB of useful source");
    }

    #[test]
    fn presentation_edit_budget_reads_medium_page_whole() {
        let mut ledger = ActiveToolResultLedger::default();
        let raw=json!({"status":"version_read","file":"index.html","offset":0,"end_offset":35000,
            "total_characters":35000,"chunk_characters":35000,"text":"a".repeat(35000),"next_offset":null});
        let result=ledger.project_result("presentation.author",r#"{"operation":"read","file":"index.html"}"#,
            &raw.to_string(),policy(ToolResultPolicy::EvidenceProjection,16384),&two_leaf_book());
        assert!(!result.truncated);
        assert_eq!(result.model_body["text"].as_str().unwrap().len(),35000);
    }

    #[test]
    fn presentation_edit_truncated_range_continues_only_to_requested_end() {
        let raw=json!({"status":"version_read","file":"index.html","offset":3,"end_offset":1003,
            "total_characters":5000,"chunk_characters":1000,"text":"中".repeat(1000),"next_offset":null});
        let result=project_tool_result("presentation.author",r#"{"operation":"read","file":"index.html","offset":3,"length":1000}"#,
            &raw.to_string(),policy(ToolResultPolicy::EvidenceProjection,700),700,&two_leaf_book());
        let ToolContinuation::NextCall { arguments, .. }=result.continuation.unwrap() else {panic!("expected range continuation")};
        let next=arguments["offset"].as_u64().unwrap();
        assert!(next>3 && next<1003);
        assert_eq!(next+arguments["length"].as_u64().unwrap(),1003);
    }

    #[test]
    fn presentation_edit_search_projection_preserves_matches_and_continuation() {
        let matches:Vec<_>=(0..12).map(|i| json!({"offset":i*400,"text":"中".repeat(100),"line":i+1})).collect();
        let raw=json!({"status":"source_matches","file":"index.html","query":"中","offset":0,"matches":matches,"next_offset":null});
        let draft=project_tool_result("presentation.author",r#"{"operation":"search","candidate_id":"c","query":"中"}"#,&raw.to_string(),policy(ToolResultPolicy::EvidenceProjection,1000),1000,&two_leaf_book());
        let kept=draft.model_body["matches"].as_array().unwrap();
        assert!(!kept.is_empty() && kept.len()<matches.len());
        assert_eq!(kept,&matches[..kept.len()]);
        assert_eq!(draft.model_body["next_offset"],matches[kept.len()]["offset"]);
        let ToolContinuation::NextCall { arguments, .. }=draft.continuation.unwrap() else {panic!("search continuation missing")};
        assert_eq!(arguments["offset"],matches[kept.len()]["offset"]);
        assert_eq!(arguments["candidate_id"],"c");
    }

    #[test]
    fn tool_result_projection_active_fresh_bodies_share_one_turn_budget() {
        let book = two_leaf_book();
        let mut ledger = ActiveToolResultLedger::default();
        for index in 0..4 {
            let draft = ledger.project_result(
                "profile.manifest",
                "{}",
                &json!({"slots":["X".repeat(30_000)]}).to_string(),
                policy(ToolResultPolicy::ProfileProjection, 20 * 1024),
                &book,
            );
            let receipt = receipt("profile.manifest");
            ledger.insert(format!("call-{index}"), draft.into_envelope(receipt));
        }
        let used: usize = ledger
            .by_call_id
            .values()
            .map(|result| json_len(&result.envelope.model_body))
            .sum();
        assert!(used <= ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES);
        assert!(ledger.by_call_id.values().all(|result| result.retain_model_body),
            "a batch must not evict results before the model has seen them");
    }
}
