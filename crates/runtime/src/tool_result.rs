use crate::tool_registry::{ToolOutputPolicy, ToolResultPolicy};
use crate::{Message, Role, ToolCall};
use book_tool_contracts::{validate_input, BookToolId, BookToolInput};
use read_tools::{Book, EvidenceRange, SearchTextResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

#[cfg(test)]
#[path = "structured_result_tests.rs"]
mod structured_tests;

#[path = "book_delivery_schema.rs"]
mod book_delivery_schema;
pub use book_delivery_schema::book_envelope_schema;

pub const TOOL_RESULT_ENVELOPE_VERSION: &str = "tool_result_envelope.v1";
/// Shared allowance for fresh results awaiting their first model sampling.
/// Previously sampled bodies stay in history until whole-request pressure.
pub const ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES: usize = 48 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HistoricalToolStatus {
    Ok,
    Error,
    LegacyUnparsed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolResultStatus {
    Ok,
    Partial,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
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

fn next_text_call(arguments: &str, start: usize, reason: &str) -> Option<ToolContinuation> {
    let mut next: Value = serde_json::from_str(arguments).ok()?;
    next.as_object_mut()?
        .insert("start_utf16".into(), json!(start));
    Some(ToolContinuation::NextCall {
        tool: "book.text".into(),
        arguments: next,
        reason: reason.into(),
    })
}

fn project_text(arguments: &str, raw: &Value, limit: usize, book: &Book) -> ToolResultDraft {
    let input = serde_json::from_str::<Value>(arguments)
        .ok()
        .and_then(|value| validate_input(BookToolId::Text, value).ok());
    let Some(BookToolInput::Text(input)) = input else {
        return project_error(
            arguments,
            &json!({"error_code":"BOOK_TOOL_INPUT_INVALID"}),
            limit,
        );
    };
    let first = book
        .base
        .lid_nodes
        .iter()
        .find(|n| n.lid == input.lid)
        .expect("successful text start");
    let last = book
        .base
        .lid_nodes
        .iter()
        .find(|n| n.lid == input.end_lid.as_deref().unwrap_or(&input.lid))
        .expect("successful text end");
    let start = input.start_utf16.unwrap_or(first.span.start);
    let text = raw["text"].as_str().expect("successful canonical text");
    // Count serialized bytes, including JSON escapes, without repeatedly cloning
    // the full passage. A cut always preserves an exact Unicode prefix.
    let mut body = json!({"lid":input.lid,"text":"",
        "source_range":{"start":start,"end":last.span.end},
        "requested_range":{"start":first.span.start,"end":last.span.end},
        "next_start_utf16":last.span.end});
    let overhead = json_len(&body);
    let mut bytes = overhead;
    let mut end = start;
    let mut cut = 0;
    for (at, ch) in text.char_indices() {
        let cost = match ch {
            '"' | '\\' | '\n' | '\r' | '\t' | '\u{08}' | '\u{0c}' => 2,
            ch if ch < ' ' => 6,
            ch => ch.len_utf8(),
        };
        if bytes + cost > limit {
            break;
        }
        bytes += cost;
        end += ch.len_utf16();
        cut = at + ch.len_utf8();
    }
    // Prefer a whole leaf when one fits; keep headings and inter-leaf gaps in
    // the continuous range, and use offsets when an individual leaf is large.
    if let Some(boundary) = book
        .base
        .lid_nodes
        .iter()
        .filter(|n| n.children.is_empty() && n.span.end > start && n.span.end <= end)
        .map(|n| n.span.end)
        .max()
    {
        if boundary < end {
            let mut position = start;
            cut = text
                .char_indices()
                .find_map(|(at, ch)| {
                    if position == boundary {
                        Some(at)
                    } else {
                        position += ch.len_utf16();
                        None
                    }
                })
                .unwrap_or(cut);
            end = boundary;
        }
    }
    body["text"] = json!(&text[..cut]);
    body["source_range"]["end"] = json!(end);
    body["next_start_utf16"] = if end < last.span.end {
        json!(end)
    } else {
        Value::Null
    };
    // Re-scope annotations to delivered text, never to the untruncated canonical result.
    if cut > 0 {
        let math = book.math_reading_context(&[base_schema::Span { start, end }]);
        if !math.is_empty() {
            body["math_context"] = json!(math);
            if json_len(&body) > limit { body.as_object_mut().unwrap().remove("math_context"); }
        }
    }
    if cut == 0 {
        body = bounded_value(
            json!({"projection":"budget_exhausted","message":"No source text delivered; retry the continuation when model-body budget is available."}),
            limit,
            &[],
        );
    }
    ToolResultDraft {
        status: ToolResultStatus::Partial,
        model_body: body,
        truncated: true,
        continuation: (end < last.span.end)
            .then(|| {
                next_text_call(
                    arguments,
                    end,
                    "continue the canonical source after the delivered UTF-16 range",
                )
            })
            .flatten(),
        evidence_arguments: arguments.into(),
    }
}

fn project_search(arguments: &str, raw: &Value, limit: usize) -> ToolResultDraft {
    let (Ok(original), Ok(BookToolInput::SearchText(input))) = (
        serde_json::from_value::<SearchTextResult>(raw.clone()),
        serde_json::from_str(arguments)
            .map_err(|e| e.to_string())
            .and_then(|v| validate_input(BookToolId::SearchText, v).map_err(|e| e.message)),
    ) else {
        return project_error(
            arguments,
            &json!({"error_code":"BOOK_TOOL_INPUT_INVALID"}),
            limit,
        );
    };
    let mut projected = original.clone();
    // The aggregate scan remains exhaustive. The envelope marks delivery
    // truncation; its continuation replaces the raw page's cursor.
    projected.next_cursor = None;
    projected.section_counts.clear();
    let mut body = serde_json::to_value(&projected).unwrap();
    body["delivery"] = json!({"returned_count":projected.occurrences.len(),"section_counts_omitted":!original.section_counts.is_empty()});
    while json_len(&body) > limit && !projected.occurrences.is_empty() {
        projected.occurrences.pop();
        body["occurrences"] = json!(projected.occurrences);
        body["delivery"]["returned_count"] = json!(projected.occurrences.len());
    }
    let kept = projected.occurrences.len();
    let cursor = original
        .cursor_after_prefix(&input, kept)
        .expect("validated search request");
    let continuation = cursor.map(|cursor| {
        let mut next: Value = serde_json::from_str(arguments).unwrap();
        next["cursor"] = json!(cursor);
        ToolContinuation::NextCall { tool:"book.search_text".into(), arguments:next,
            reason:if kept == 0 { "No occurrence fits; retry when model-body budget is available.".into() }
                else { "continue the original search after the delivered occurrences; query, scope, order and page_size are unchanged".into() } }
    });
    ToolResultDraft {
        status: ToolResultStatus::Partial,
        model_body: bounded_value(body, limit, &["section_counts"]),
        truncated: true,
        continuation,
        evidence_arguments: arguments.into(),
    }
}

/// Ranked search delivers candidate locators, never source evidence or an
/// exhaustive collection. A hit and its exact reading calls are one atomic unit.
fn project_ranked_search(tool: &str, arguments: &str, raw: &Value, limit: usize) -> ToolResultDraft {
    let nested = tool == "library.read";
    let hits_path = if nested { "/data/hits" } else { "/hits" };
    let mut body = raw.clone();
    let original_count = body.pointer(hits_path).and_then(Value::as_array).map_or(0, Vec::len);
    body["exhaustive"] = json!(false);
    if nested {
        body["data"]["exhaustive"] = json!(false);
        // Remote source links are locators too; search does not deliver text.
        body.as_object_mut().unwrap().remove("sources");
        for hit in body["data"]["hits"].as_array_mut().into_iter().flatten() {
            for call in hit.get_mut("next_calls").and_then(Value::as_array_mut).into_iter().flatten() {
                if call["tool"] == "book.text" || call["name"] == "book.text" {
                    let mut args = call["arguments"].clone();
                    args["operation"] = json!("text");
                    args["published_book_ref"] = raw["published_book_ref"].clone();
                    *call = json!({"name":"library.read","arguments":args});
                }
            }
        }
    }
    loop {
        let hits = body.pointer(hits_path).and_then(Value::as_array).cloned().unwrap_or_default();
        let mut calls = Vec::new();
        for hit in &hits {
            if let Some(hit_calls) = hit.get("next_calls").and_then(Value::as_array).filter(|calls| !calls.is_empty()) {
                for call in hit_calls {
                    if !calls.contains(call) { calls.push(call.clone()); }
                }
            } else if nested {
                // MCP can keep the executable calls on its envelope instead of
                // inside data. Match the exact candidate range, not just a LID:
                // multiple chunks can start in the same paragraph.
                for call in raw["next_calls"].as_array().into_iter().flatten() {
                    let args = &call["arguments"];
                    if args["operation"] == "text"
                        && args["published_book_ref"] == raw["published_book_ref"]
                        && args["lid"] == hit["start_lid"]
                        && args["start_utf16"] == hit["source_range_utf16"]["start"]
                        && !calls.contains(call)
                    {
                        calls.push(call.clone());
                    }
                }
            }
        }
        body["next_calls"] = json!(calls);
        if nested && body["data"].get("next_calls").is_some() {
            body["data"]["next_calls"] = body["next_calls"].clone();
        }
        let kept = hits.len();
        let truncated = kept < original_count;
        body["delivery"] = json!({
            "status":if truncated {"partial"} else {"complete"},
            "returned_count":kept,
            "omitted_count":original_count - kept,
            "evidence":"locators_only",
        });
        if (original_count == 0 || kept > 0) && json_len(&body) <= limit {
            return ToolResultDraft {
                status: if truncated { ToolResultStatus::Partial } else { ToolResultStatus::Ok },
                model_body: body,
                truncated,
                // Reading a candidate is optional follow-up work. There is no
                // cursor that can exhaust the ranked retrieval through paging.
                continuation: None,
                evidence_arguments: arguments.into(),
            };
        }
        if kept == 0 { break; }
        body.pointer_mut(hits_path).and_then(Value::as_array_mut).unwrap().pop();
    }
    ToolResultDraft {
        status: ToolResultStatus::Partial,
        model_body: semantic_budget_failure(raw, limit),
        truncated: true,
        continuation: Some(refine_continuation(tool, arguments,
            "No complete search candidate fits the result budget. Narrow the query or scope, or retry in a fresh turn with sufficient budget. Ranked search is not exhaustive and has no paging continuation.")),
        evidence_arguments: arguments.into(),
    }
}

fn project_legacy_structured(
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
    let mut model_body = bounded_value(raw.clone(), limit, drop_order);
    if tool == "book.concept" {
        // The budget trims occurrences independently. Keep optional reads attached only
        // to locations still delivered in this candidate and accepted by the locator ledger.
        if let Some(candidates) = model_body.get_mut("candidates").and_then(Value::as_array_mut) {
            for candidate in candidates {
                let lids: HashSet<String> = candidate["occurrences"].as_array().into_iter().flatten()
                    .filter_map(Value::as_str).map(str::to_owned).collect();
                if let Some(calls) = candidate.get_mut("next_calls").and_then(Value::as_array_mut) {
                    calls.retain(|call| call["arguments"]["lid"].as_str().is_some_and(|lid| lids.contains(lid)));
                }
            }
        }
    }
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

// These are collection boundaries owned by the actual result types. Everything
// else (warnings, support verdicts, bindings, source identity) stays atomic.
fn structured_paths(tool: &str, raw: &Value) -> Option<Vec<String>> {
    let fields: &[&str] = match tool {
        "tool.search" => &["matches"],
        "book.context" => &["items"],
        "book.concept" => {
            let mut paths = Vec::new();
            for field in ["next_calls", "previews", "occurrences"] {
                for index in 0..raw["candidates"].as_array().map_or(0, Vec::len) {
                    paths.push(format!("/candidates/{index}/{field}"));
                }
            }
            paths.push("/candidates".into());
            return Some(paths);
        }
        "book.query" => &["model_supplement", "candidates", "citations"],
        "book.synthesize" => &[
            "model_supplement",
            "suggested_probing",
            "related_concepts",
            "evidence_chain",
            "citations",
        ],
        "book.structure" => &["throughlines", "key_stops"],
        "book.guide_path" => &["segments"],
        "book.paper_lexicon" => &["entries"],
        "book.paper_metadata" => &[
            "references/value",
            "authors/value",
            "affiliations/value",
            "keywords/value",
            "field_labels/value",
            "datasets/value",
            "code_links/value",
            "funding/value",
        ],
        "book.paper_reading_guide" => &[
            "questions",
            "codebook/terms",
            "codebook/throughlines",
            "codebook/key_stops",
            "abstract_aid/key_terms",
            "abstract_aid/comprehension_checks",
            "abstract_aid/excerpts",
        ],
        "profile.manifest" => &["layout_presets", "ui_slots", "projections", "agent_tools"],
        "source.present" => &[],
        _ => return None,
    };
    Some(fields.iter().map(|field| format!("/{field}")).collect())
}

fn update_structured_delivery(body: &mut Value, raw: &Value, paths: &[String], tool: &str) {
    let mut collections = serde_json::Map::new();
    for path in paths {
        if let (Some(original), Some(delivered)) = (
            raw.pointer(path).and_then(Value::as_array),
            body.pointer(path).and_then(Value::as_array),
        ) {
            collections.insert(
                path.clone(),
                json!({"total":original.len(),"delivered":delivered.len()}),
            );
        }
    }
    body["delivery"]["collections"] = Value::Object(collections);
    // Omitted quote text is not evidence. Its original LID remains a locator,
    // so the caller can read the canonical text without rerunning inference.
    let sources: &[(&str, &str)] = match tool {
        "book.query" | "book.synthesize" => &[("/citations", "/lid")],
        "book.structure" => &[("/key_stops", "/lid"), ("/throughlines", "/lids/0")],
        "book.guide_path" => &[("/segments", "/spine_unit/lid")],
        _ => &[],
    };
    let mut lids = Vec::new();
    for (path, lid_path) in sources {
        let delivered = body
            .pointer(path)
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        for item in raw
            .pointer(path)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .skip(delivered)
        {
            if let Some(lid) = item.pointer(lid_path).and_then(Value::as_str) {
                if !lids.contains(&lid) {
                    lids.push(lid);
                }
            }
        }
    }
    if !lids.is_empty() {
        body["delivery"]["read_lids"] = json!(lids);
    }
}

fn structured_recovery(
    tool: &str,
    arguments: &str,
    body: &Value,
    raw: &Value,
) -> Option<ToolContinuation> {
    let mut args: Value = serde_json::from_str(arguments).ok()?;
    let original = args.clone();
    let read = |lid: &str, reason: &str| {
        Some(ToolContinuation::NextCall {
            tool: "book.text".into(),
            arguments: json!({"lid":lid}),
            reason: reason.into(),
        })
    };
    let guidance = match tool {
        "book.query" | "book.synthesize" => {
            let lid = body
                .pointer("/delivery/read_lids/0")
                .and_then(Value::as_str)
                .or_else(|| first_citation_lid(body))
                .or_else(|| {
                    body.pointer("/candidates/0/excerpts/0/lid")
                        .and_then(Value::as_str)
                })?;
            return read(lid, "read source text from the returned query; this does not regenerate the omitted answer");
        }
        "book.concept" => {
            let candidate = body["candidates"].as_array()?.first()?;
            let occurrences = candidate["occurrences"].as_array()?;
            let calls = candidate["next_calls"].as_array().map_or(0, Vec::len);
            if let Some(lid) = occurrences
                .get(calls)
                .or_else(|| occurrences.first())
                .and_then(Value::as_str)
            {
                return read(lid, "read a returned occurrence; other returned LIDs can be read with book.text, and this is not candidate pagination");
            }
            args["limit"] = json!(1);
            "Recall one candidate; this narrows candidate selection and does not page occurrences."
        }
        "book.context" => {
            let count = body["items"].as_array()?.len();
            args["k"] = json!(count.max(1));
            "Request fewer context pointers; this narrows selection and does not retrieve the omitted tail."
        }
        "book.structure" | "book.guide_path" => {
            let lid = body
                .pointer("/delivery/read_lids/0")
                .and_then(Value::as_str)
                .or_else(|| body.pointer("/spine_unit/lid").and_then(Value::as_str))?;
            args["at"] = json!(lid);
            if tool == "book.guide_path" {
                // at selects a current segment but does not shorten guide_path.
                return Some(ToolContinuation::NextCall { tool: "book.structure".into(), arguments: json!({"at":lid}),
                    reason: "inspect the omitted route segment's structure without repeating the whole guide".into() });
            }
            if args == original {
                return read(
                    lid,
                    "read the returned structural location as canonical text",
                );
            }
            "Inspect the returned location's structure; this is a focused view, not collection pagination."
        }
        "tool.search" => {
            let next = body["blocked"]
                .as_array()
                .into_iter()
                .flatten()
                .find_map(|entry| entry["next_searches"].as_array().and_then(|a| a.first()));
            if let Some(next) = next {
                // Public capability recovery already carries valid TaskNeed fields.
                args = next.clone();
            } else {
                let delivered = body["matches"].as_array()?.len();
                let omitted = raw["matches"].as_array()?.get(delivered)?;
                args["required_capabilities"] =
                    json!([omitted["matched_capabilities"].as_array()?.first()?]);
                args["max_results"] = json!(1);
            }
            "Discover the reported capability with a smaller request; this is not pagination of the original matches."
        }
        "book.paper_reading_guide" => {
            if !matches!(args["stage"].as_str(), Some("critical" | "creative")) {
                return None;
            }
            args["stage"] = json!("passive");
            "Request the passive stage without critique/research answer slots; this changes the selected reading scope."
        }
        _ => return None,
    };
    if args == original {
        return None;
    }
    Some(ToolContinuation::RefineCall {
        tool: tool.into(),
        arguments: args,
        guidance: guidance.into(),
    })
}

fn semantic_budget_failure(raw: &Value, limit: usize) -> Value {
    let mut body = json!({
        "projection":"semantic_budget_exceeded",
        "delivery":{"status":"omitted","reason":"Required semantics exceed the model-body budget; no content or evidence was delivered. Retry in a fresh turn with sufficient budget; this tool may have no paging API."}
    });
    for key in [
        "available",
        "error_code",
        "category",
        "required_capability",
        "profile_id",
        "profile_version",
    ] {
        if let Some(value) = raw.get(key) {
            body[key] = value.clone();
        }
    }
    if json_len(&body) <= limit {
        return body;
    }
    let compact = json!({"projection":"semantic_budget_exceeded","delivery":{"status":"omitted"}});
    if json_len(&compact) <= limit {
        compact
    } else {
        Value::Null
    }
}

fn project_structured(
    tool: &str,
    arguments: &str,
    raw: &Value,
    limit: usize,
    result_policy: ToolResultPolicy,
) -> ToolResultDraft {
    let Some(paths) = structured_paths(tool, raw) else {
        return project_legacy_structured(tool, arguments, raw, limit, result_policy);
    };
    let mut body = raw.clone();
    body["delivery"] = json!({"status":"partial"});
    if matches!(
        tool,
        "profile.manifest" | "book.paper_metadata" | "book.paper_lexicon" | "source.present"
    ) {
        body["delivery"]["recovery"] =
            json!("No paging or smaller selection API; retry only with sufficient result budget.");
    }
    if tool == "tool.search" {
        let redundant = ["description", "matched_fields", "matched_terms", "score"];
        for item in body["matches"].as_array_mut().into_iter().flatten() {
            for key in redundant {
                item.as_object_mut().unwrap().remove(key);
            }
        }
        body["delivery"]["omitted_match_fields"] = json!(redundant);
    }
    update_structured_delivery(&mut body, raw, &paths, tool);
    if tool == "source.present" {
        for field in ["preview", "label"] {
            if json_len(&body) <= limit {
                break;
            }
            if body.as_object_mut().unwrap().remove(field).is_some() {
                if body["delivery"]["omitted_fields"].is_null() {
                    body["delivery"]["omitted_fields"] = json!([]);
                }
                body["delivery"]["omitted_fields"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!(format!("/{field}")));
            }
        }
    }
    // Never shorten a generated statement into a different claim. Citations,
    // support verdicts and bindings remain separate from the omitted prose.
    if matches!(tool, "book.query" | "book.synthesize")
        && json_len(&body) > limit
        && !body["answer"].is_null()
    {
        body["answer"] = Value::Null;
        body["delivery"]["omitted_fields"] = json!(["/answer"]);
    }
    for path in &paths {
        while json_len(&body) > limit {
            let Some(items) = body.pointer_mut(path).and_then(Value::as_array_mut) else {
                break;
            };
            let minimum = usize::from(tool == "book.concept" && path.ends_with("/next_calls"));
            if items.len() <= minimum {
                break;
            }
            items.pop();
            if tool == "book.concept" && path.ends_with("/occurrences") {
                let parent = path.strip_suffix("/occurrences").unwrap();
                let candidate = body.pointer_mut(parent).unwrap();
                let lids: HashSet<String> = candidate["occurrences"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect();
                if let Some(calls) = candidate["next_calls"].as_array_mut() {
                    calls.retain(|c| {
                        c["arguments"]["lid"]
                            .as_str()
                            .is_some_and(|lid| lids.contains(lid))
                    });
                }
            }
            update_structured_delivery(&mut body, raw, &paths, tool);
        }
    }
    if json_len(&body) > limit {
        body = semantic_budget_failure(raw, limit);
    }
    let continuation = structured_recovery(tool, arguments, &body, raw);
    if continuation.is_none()
        && body.get("delivery").is_some()
        && body["projection"].is_null()
        && body["delivery"].get("recovery").is_none()
    {
        body["delivery"]["recovery"] = json!("No executable continuation is available for the omitted fields. Do not repeat the same call at the same budget; use a fresh turn with sufficient budget or choose a smaller supported request.");
        // Recovery prose must not push a previously bounded body over budget.
        if json_len(&body) > limit {
            body["delivery"].as_object_mut().unwrap().remove("recovery");
        }
    }
    ToolResultDraft {
        status: ToolResultStatus::Partial,
        model_body: body,
        truncated: true,
        continuation,
        evidence_arguments: arguments.into(),
    }
}

fn project_error(arguments: &str, raw: &Value, limit: usize) -> ToolResultDraft {
    let mut model_body = raw.clone();
    if json_len(&model_body) > limit {
        if model_body
            .as_object_mut()
            .and_then(|o| o.remove("details"))
            .is_some()
        {
            model_body["delivery"] = json!({"status":"partial","omitted_fields":["/details"]});
        }
        if json_len(&model_body) > limit {
            model_body = semantic_budget_failure(raw, limit);
        }
    }
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
    // Method length is a page size; ordinary editable reads retain their
    // explicit range end. A clipped method page must still lead to file EOF.
    let method_read = raw["status"] == "method_read";
    let total = if method_read { &raw["total_characters"] }
        else { raw.get("end_offset").unwrap_or(&raw["total_characters"]) }
        .as_u64().unwrap_or_default() as usize;
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
        if method_read { body["end_offset"] = json!(offset + count); }
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
    if method_read { body["end_offset"] = json!(offset + low); }
    body["next_offset"] = json!((offset + low < total).then_some(offset + low));
    body["chunk_characters"] = json!(low);
    let continuation = (offset + low < total).then(|| {
        let mut next: Value = serde_json::from_str(arguments).unwrap_or_else(|_| json!({}));
        next["offset"] = json!(offset + low);
        if !method_read && raw.get("end_offset").is_some() { next["length"] = json!(total - offset - low); }
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

fn project_text_batch(arguments: &str, raw: &Value, limit: usize, book: &Book) -> ToolResultDraft {
    let mut body = json!({"items": []});
    let mut pending = Vec::new();
    let items = raw["items"].as_array().map(Vec::as_slice).unwrap_or_default();
    let mut partial = false;
    let mut truncated = false;
    for (index, item) in items.iter().enumerate() {
        let args = &item["arguments"];
        // Share the remaining bytes so a long first passage cannot consume the
        // entire batch. Wrapper bytes count against the same model-body budget.
        let share = limit.saturating_sub(json_len(&body)) / (items.len() - index);
        let wrapper = json_len(&json!({"arguments": args, "result": null}));
        let item_limit = share.saturating_sub(wrapper + 1).saturating_add(4);
        if share <= wrapper {
            pending.push(args.clone());
            truncated = true;
            continue;
        }
        let projected = project_tool_result("book.text", &args.to_string(), &item["result"].to_string(),
            ToolOutputPolicy { result_policy: ToolResultPolicy::EvidenceProjection, max_model_body_bytes: item_limit },
            item_limit, book);
        body["items"].as_array_mut().unwrap().push(json!({"arguments":args,"result":projected.model_body}));
        if json_len(&body) > limit {
            body["items"].as_array_mut().unwrap().pop();
            pending.push(args.clone());
            truncated = true;
            continue;
        }
        partial |= projected.status != ToolResultStatus::Ok;
        truncated |= projected.truncated;
        if let Some(ToolContinuation::NextCall { arguments, .. } | ToolContinuation::ToolCursor { arguments, .. }) = projected.continuation {
            pending.push(arguments);
        }
    }
    if json_len(&body) > limit { body = Value::Null; }
    ToolResultDraft {
        status: if partial || !pending.is_empty() { ToolResultStatus::Partial } else { ToolResultStatus::Ok },
        model_body: body,
        truncated,
        continuation: (!pending.is_empty()).then(|| ToolContinuation::NextCall {
            tool: "book.text_batch".into(), arguments: json!({"ranges":pending}),
            reason: "Continue only the undelivered source ranges; each item keeps its own UTF-16 position.".into(),
        }),
        evidence_arguments: arguments.into(),
    }
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
    if tool == "workspace.notes" {
        return crate::workspace_notes::project_result(arguments, &raw, limit);
    }
    if tool == "board.author" && matches!(raw["status"].as_str(), Some("board_read" | "board_page")) {
        return crate::board_author::project_read(arguments, &raw, limit);
    }
    if matches!(tool, "music.read" | "music.interact") && json_len(&raw) > limit {
        // Music locators and player receipts are atomic. A shortened ID or a
        // clipped event would authorize a citation/action the model never read.
        let guidance = if tool == "music.read" {
            "The prepared music result does not fit this request. Read a smaller original-time range, fewer parts, or a lower limit; no source from this result was observed."
        } else { "The actual browser receipt did not fit the model budget. Observe again before taking another action; do not infer completion." };
        let mut next: Value = serde_json::from_str(arguments).unwrap_or_else(|_| json!({}));
        if tool == "music.read" { next["limit"] = json!((next["limit"].as_u64().unwrap_or(20) / 2).max(1)); }
        return ToolResultDraft { status: ToolResultStatus::Partial,
            model_body: bounded_value(json!({"error_code":"MUSIC_RESULT_BUDGET","category":"budget","message":guidance}),limit,&[]),
            truncated:true, continuation:Some(ToolContinuation::RefineCall { tool:tool.into(),arguments:next,guidance:guidance.into() }), evidence_arguments:arguments.into() };
    }
    if matches!(tool, "web.search" | "web.fetch") {
        return crate::web::project_result(tool, arguments, &raw, limit);
    }
    if tool == "tutor.step" && serde_json::from_str::<Value>(arguments).is_ok_and(|v| matches!(v["operation"].as_str(), Some("design"|"evidence"|"assess"))) {
        // Full results remain in the trace and have already refreshed the runtime.
        // Supply current derived state in replaceable fragments, not snapshots in
        // historical receipts. Keep accepted references atomic under the budget.
        let mut body = raw.clone();
        for key in ["design", "learner_context_update", "learner_context"] {
            body.as_object_mut().unwrap().remove(key);
        }
        for optional in ["instruction", "teaching_implication"] {
            if json_len(&body) > limit { body.as_object_mut().unwrap().remove(optional); }
        }
        let fits = json_len(&body) <= limit;
        return ToolResultDraft {
            status: if fits { ToolResultStatus::Ok } else { ToolResultStatus::Partial },
            model_body: if fits { body } else { semantic_budget_failure(&body, limit) },
            truncated: !fits, continuation: None, evidence_arguments: arguments.into(),
        };
    }
    if tool == "book.text_batch" {
        return project_text_batch(arguments, &raw, limit, book);
    }
    if matches!(tool, "book.search" | "library.search")
        || (tool == "library.read" && serde_json::from_str::<Value>(arguments).is_ok_and(|v| v["operation"] == "search"))
    {
        return project_ranked_search(tool, arguments, &raw, limit);
    }
    if tool == "library.read" && serde_json::from_str::<Value>(arguments).is_ok_and(|v| v["operation"] == "text") {
        return project_library_text(arguments, &raw, limit, book);
    }
    if matches!(tool, "library.list" | "library.read") && json_len(&raw) > limit {
        return project_library_collections(tool, arguments, &raw, limit);
    }
    if tool == "library.learning" && json_len(&raw) > limit {
        return project_library_learning(arguments, &raw, limit);
    }
    if tool == "presentation.author" && raw["status"] == "source_matches" {
        return project_presentation_search(arguments, &raw, limit);
    }
    if json_len(&raw) <= limit {
        return ToolResultDraft {
            status: ToolResultStatus::Ok,
            continuation: if tool == "book.text" {
                raw["next_start_utf16"].as_u64().and_then(|start| next_text_call(arguments,start as usize,"continue the native text page"))
            } else if tool == "book.search_text" {
                next_search_cursor(tool,arguments,&raw)
            } else if tool == "presentation.author" && raw["status"] == "method_read" {
                raw["next_offset"].as_u64().map(|offset| {
                    let mut next: Value = serde_json::from_str(arguments).unwrap_or_else(|_| json!({}));
                    next["offset"] = json!(offset);
                    ToolContinuation::NextCall {
                        tool: tool.into(), arguments: next,
                        reason: "continue method source at the first unread character".into(),
                    }
                })
            } else { None },
            model_body: raw,
            truncated: false,
            evidence_arguments: arguments.into(),
        };
    }
    match tool {
        "presentation.author" if matches!(raw["status"].as_str(), Some("version_read" | "method_read")) => project_presentation_source(arguments, &raw, limit),
        "book.text" => project_text(arguments, &raw, limit, book),
        "book.search_text" => project_search(arguments, &raw, limit),
        _ => project_structured(tool, arguments, &raw, limit, output_policy.result_policy),
    }
}

// Understanding records and their identifiers are atomic. Both native collections use
// the same offset, so trimming them together keeps the next page from skipping records.
fn project_library_learning(arguments: &str, raw: &Value, limit: usize) -> ToolResultDraft {
    let mut next: Value = serde_json::from_str(arguments).unwrap();
    let mut body = raw.clone();
    body["delivery"] = json!({"status":"partial"});
    if next["operation"] != "read" {
        body.as_object_mut().unwrap().remove("teaching_implication");
        if json_len(&body) > limit { body = semantic_budget_failure(raw, limit); }
        return ToolResultDraft { status:ToolResultStatus::Partial, model_body:body,
            truncated:true, continuation:None, evidence_arguments:arguments.into() };
    }
    while json_len(&body) > limit && body["recent_facts"].as_array().is_some_and(|items| !items.is_empty()) {
        body["recent_facts"].as_array_mut().unwrap().pop();
        body["delivery"]["recent_facts_omitted"] = json!(true);
    }
    let count = |field: &str| raw[field].as_array().map_or(0, Vec::len);
    let original = count("rows").max(count("interpretations"));
    let mut kept = original;
    while json_len(&body) > limit && kept > 0 {
        kept -= 1;
        for field in ["rows", "interpretations"] {
            if let Some(items) = body[field].as_array_mut() { items.truncate(kept); }
        }
        body["next"] = json!(next["after"].as_u64().unwrap_or(0) + kept as u64);
    }
    let continuation = if kept == 0 && original > 0 || json_len(&body) > limit {
        body = semantic_budget_failure(raw, limit);
        Some(refine_continuation("library.learning", arguments,
            "A complete understanding record did not fit. Retry with sufficient context budget; no learning record was delivered."))
    } else {
        body["next"].as_u64().map(|after| {
            next["after"] = json!(after);
            ToolContinuation::NextCall { tool:"library.learning".into(), arguments:next,
                reason:"Continue complete understanding records for the same publication.".into() }
        })
    };
    ToolResultDraft { status:ToolResultStatus::Partial, model_body:body, truncated:true,
        continuation, evidence_arguments:arguments.into() }
}

/// Trim only collection boundaries and keep executable, publication-bound continuations.
fn project_library_collections(tool:&str, arguments:&str, raw:&Value, limit:usize)->ToolResultDraft {
    let args:Value=serde_json::from_str(arguments).unwrap();
    let mut body=raw.clone();
    // Resident citations are registered from text, not these remote source links.
    body.as_object_mut().unwrap().remove("sources");
    let mut calls=raw["next_calls"].as_array().cloned().unwrap_or_default();
    body["complete"]=json!(false);
    if tool=="library.list" {
        while json_len(&body)>limit && body["items"].as_array().is_some_and(|items|!items.is_empty()) {
            body["items"].as_array_mut().unwrap().pop();
            let mut next=args.clone();
            if let Some(last)=body["items"].as_array().and_then(|items|items.last()) {next["after"]=last["published_book_ref"].clone();}
            next["limit"]=json!(1.max(body["items"].as_array().unwrap().len()));
            calls=vec![json!({"name":tool,"arguments":next})];
            body["next_calls"]=json!(calls);
        }
    } else {
        fn arrays(value:&Value,path:String,paths:&mut Vec<String>) {
            match value {
                Value::Array(_) => paths.push(path),
                Value::Object(fields) => for (key,value) in fields { arrays(value,format!("{path}/{}",key.replace('~',"~0").replace('/',"~1")),paths); },
                _=>{}
            }
        }
        let mut paths=vec![]; arrays(&body["data"],String::new(),&mut paths);
        for path in paths {
            let original=body["data"].pointer(&path).and_then(Value::as_array).map_or(0,Vec::len);
            let full_path=format!("{}{path}",raw["data_path"].as_str().unwrap_or_default());
            let base=if path.is_empty() {args["page"]["offset"].as_u64().unwrap_or(0)} else {0};
            while json_len(&body)>limit {
                let items=body["data"].pointer_mut(&path).and_then(Value::as_array_mut).unwrap();
                if items.is_empty(){break;}
                items.pop(); let kept=items.len();
                let mut next=args.clone();next["page"]=json!({"path":full_path,"offset":base+kept as u64,"limit":original.max(1).min(10)});
                calls.retain(|c|c["arguments"]["page"]["path"]!=full_path);
                calls.insert(0,json!({"name":tool,"arguments":next}));
                body["next_calls"]=json!(calls);
            }
        }
    }
    if json_len(&body)>limit {
        body=json!({"projection":"budget_exhausted"});
        calls=vec![json!({"name":tool,"arguments":args})];
    }
    ToolResultDraft {status:ToolResultStatus::Partial,model_body:body,truncated:true,evidence_arguments:arguments.into(),
        continuation:calls.first().map(|c|ToolContinuation::NextCall {tool:tool.into(),arguments:c["arguments"].clone(),reason:"Continue omitted items from the same publication and collection; no omitted item was delivered.".into()})}
}

/// Keep exact text prefixes and the publication identity when the model budget cuts a remote page.
fn project_library_text(arguments: &str, raw: &Value, limit: usize, book: &Book) -> ToolResultDraft {
    let mut args: Value = serde_json::from_str(arguments).expect("validated library arguments");
    args.as_object_mut().unwrap().remove("operation");
    args.as_object_mut().unwrap().remove("published_book_ref");
    let native_next = raw["next_calls"].as_array().and_then(|v|v.first()).cloned();
    let next_call = |call: &Value| ToolContinuation::NextCall { tool:"library.read".into(), arguments:call["arguments"].clone(), reason:"continue the same publication".into() };
    if json_len(raw) <= limit {
        return ToolResultDraft { status:ToolResultStatus::Ok, model_body:raw.clone(), truncated:false,
            continuation:native_next.as_ref().map(next_call), evidence_arguments:arguments.into() };
    }
    let mut body = raw.clone();
    body["data"] = Value::Null;
    let budget = limit.saturating_sub(json_len(&body) + 256);
    let draft = project_text(&args.to_string(), &raw["data"], budget, book);
    body["data"] = draft.model_body;
    if let Ok(reference) = serde_json::from_value(raw["published_book_ref"].clone()) {
        crate::library::scope_math_read_calls(&mut body["data"], &reference);
    }
    // Projected raw-text coordinates, not the original full page, delimit what was observed.
    if let Some(end) = body["data"]["source_range"]["end"].as_u64() {
        for source in body["sources"].as_array_mut().into_iter().flatten() {
            source["source_range"]["end"] = json!(end);
            source.as_object_mut().unwrap().remove("source_url");
        }
    } else { body["sources"] = json!([]); }
    let next = if let Some(ToolContinuation::NextCall { arguments:mut next, .. }) = draft.continuation {
        next["operation"] = json!("text"); next["published_book_ref"] = raw["published_book_ref"].clone();
        Some(json!({"name":"library.read","arguments":next}))
    } else { native_next };
    body["next_calls"] = json!(next.iter().collect::<Vec<_>>());
    body["complete"] = json!(next.is_none());
    if json_len(&body) > limit {
        // Publication-scoped calls can make optional annotations larger. Keep
        // delivered source ahead of annotations, as in the local projection.
        if let Some(data) = body["data"].as_object_mut() { data.remove("math_context"); }
    }
    if json_len(&body) > limit {
        return ToolResultDraft { status:ToolResultStatus::Partial,
            model_body:json!({"projection":"budget_exhausted"}), truncated:true,
            continuation:Some(ToolContinuation::NextCall { tool:"library.read".into(),
                arguments:serde_json::from_str(arguments).unwrap(), reason:"Retry the same range when context budget is available; no source was delivered.".into() }),
            evidence_arguments:arguments.into() };
    }
    ToolResultDraft { status:ToolResultStatus::Partial, model_body:body, truncated:true,
        continuation:next.as_ref().map(next_call), evidence_arguments:arguments.into() }
}

#[derive(Debug, Clone)]
struct ActiveToolResult {
    envelope: ToolResultEnvelope,
    sampled: bool,
    retain_model_body: bool,
    saved_author_call: Option<SavedAuthorCall>,
}

/// A current-run save receipt is the authority for this model-only projection.
/// Source is retained until a request-pressure boundary. Keep the latest patch's
/// exact edits until an observed child save supplies the next modification
/// context, then retire them at the next boundary. Independent writes never
/// imply supersession.
#[derive(Debug, Clone)]
struct SavedAuthorCall {
    candidate_id: String,
    parent_candidate_id: Option<String>,
    arguments: Value,
    retired: bool,
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
        let arguments: Value = serde_json::from_str(&call.arguments).ok()?;
        if !matches!(arguments["operation"].as_str(), Some("write" | "patch")) {
            return None;
        }
        let parent_candidate_id = arguments["candidate_id"].as_str().map(str::to_owned);
        arguments.as_object()?;
        Some(Self { candidate_id, parent_candidate_id, arguments, retired: false })
    }

    fn reclaimable_bytes(&self, superseded: bool) -> usize {
        ["html", "readable_content", "edits"].into_iter()
            .filter(|field| *field != "edits" || superseded)
            .filter_map(|field| self.arguments.get(field).map(|value| json_len(value) + field.len() + 3))
            .sum()
    }

    fn retire(&mut self, superseded: bool) -> bool {
        let mut changed = !self.retired;
        self.retired = true;
        let arguments = self.arguments.as_object_mut().unwrap();
        for field in ["html", "readable_content", "edits"] {
            if field != "edits" || superseded {
                changed |= arguments.remove(field).is_some();
            }
        }
        changed
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ActiveToolResultLedger {
    by_call_id: HashMap<String, ActiveToolResult>,
}

impl ActiveToolResultLedger {
    /// Supply a reviewer only the external page segments already sampled by the
    /// answering model. Search locators, failed pages and retired bodies are not
    /// evidence. Keep whole delivered segments within the existing body budget.
    pub fn web_review_evidence(&self, messages: &[Message]) -> Value {
        let current_start = messages.iter().rposition(|m| m.role == Role::User).unwrap_or(0);
        let mut pages = Vec::new();
        let mut seen = HashSet::new();
        let mut omitted = false;
        for message in messages[current_start..].iter().rev().filter(|m| m.role == Role::Tool) {
            let Some(result) = message.tool_call_id.as_ref().and_then(|id| self.by_call_id.get(id)) else { continue; };
            let tool = result.envelope.receipt.tool.as_str();
            if !result.sampled || !matches!(tool, "web.search" | "web.fetch") { continue; }
            if !result.retain_model_body { omitted = true; continue; }
            let body = &result.envelope.model_body;
            let candidates: Vec<&Value> = if tool == "web.search" {
                body["results"].as_array().map(|v| v.iter().collect()).unwrap_or_default()
            } else { vec![body] };
            for page in candidates {
                let (Some(url), Some(text)) = (page["url"].as_str(), page["content"].as_str()) else { continue; };
                if page["content_status"] != "available" || text.trim().is_empty() { continue; }
                let offset = if tool == "web.search" { 0 } else { page["offset"].as_u64().unwrap_or(0) };
                let end_offset = offset + text.chars().count() as u64;
                if !seen.insert((url.to_owned(), offset, end_offset)) { continue; }
                let source = json!({"url":url,"title":page["title"],"date":page["date"],
                    "observed_at":body["observed_at"],"offset":offset,"end_offset":end_offset,"text":text});
                pages.push(source);
                if json_len(&json!({"pages":&pages,"omitted":true})) > ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES {
                    pages.pop();
                    omitted = true;
                }
            }
        }
        pages.reverse();
        json!({"pages":pages,"omitted":omitted})
    }

    pub fn project_result(&mut self, tool: &str, arguments: &str, raw: &str, mut policy: ToolOutputPolicy, book: &Book) -> ToolResultDraft {
        // An editable file can use the fresh-result batch allowance. Admission
        // never rewrites a body that was already sent to the model.
        if tool == "presentation.author" && serde_json::from_str::<Value>(arguments).ok().is_some_and(|v| matches!(v["operation"].as_str(), Some("read" | "read_method"))) {
            policy.max_model_body_bytes = ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES;
        }
        let projected = project_tool_result(tool, arguments, raw, policy, ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES, book);
        let needed = json_len(&projected.model_body);
        let available = self.remaining_model_body_bytes();
        if needed <= available { projected }
        else { project_tool_result(tool, arguments, raw, policy, available, book) }
    }

    pub fn remaining_model_body_bytes(&self) -> usize {
        let fresh: usize = self
            .by_call_id
            .values()
            .filter(|result| !result.sampled && result.retain_model_body)
            .map(|result| json_len(&result.envelope.model_body))
            .sum();
        ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES.saturating_sub(fresh)
    }

    pub fn sampled_model_body_bytes(&self) -> usize {
        self.by_call_id.values()
            .filter(|result| result.sampled && result.retain_model_body && !result.envelope.model_body.is_null())
            .map(|result| json_len(&result.envelope.model_body))
            .sum()
    }

    /// Saved source lives in assistant arguments, so tiny save receipts must
    /// not hide a large reclaimable batch from the request-pressure gate.
    pub fn sampled_reclaimable_bytes(&self) -> usize {
        let parents = self.observed_author_parents();
        self.sampled_model_body_bytes() + self.by_call_id.values()
            .filter(|result| result.sampled)
            .filter_map(|result| result.saved_author_call.as_ref())
            .map(|saved| saved.reclaimable_bytes(parents.contains(&saved.candidate_id)))
            .sum::<usize>()
    }

    fn observed_author_parents(&self) -> HashSet<String> {
        self.by_call_id.values()
            .filter(|result| result.sampled)
            .filter_map(|result| result.saved_author_call.as_ref())
            .filter_map(|saved| saved.parent_candidate_id.clone())
            .collect()
    }

    /// Retire one observed batch at a request-pressure boundary. Fresh results
    /// must still reach a successful sampling before becoming eligible.
    pub fn retire_sampled_bodies(&mut self) -> bool {
        let parents = self.observed_author_parents();
        let mut changed = false;
        for result in self.by_call_id.values_mut()
            .filter(|result| result.sampled)
        {
            if result.retain_model_body && !result.envelope.model_body.is_null() {
                result.retain_model_body = false;
                changed = true;
            }
            if let Some(saved) = result.saved_author_call.as_mut() {
                changed |= saved.retire(parents.contains(&saved.candidate_id));
            }
        }
        changed
    }

    pub fn insert(&mut self, call_id: impl Into<String>, envelope: ToolResultEnvelope) {
        self.by_call_id.insert(
            call_id.into(),
            ActiveToolResult {
                envelope,
                sampled: false,
                retain_model_body: true,
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
        // Provider call IDs are only unique within a user turn. This ledger
        // contains the current run's results, never results for earlier turns.
        let current_start=messages.iter().rposition(|message|message.role==Role::User).unwrap_or(0);
        let messages=&mut messages[current_start..];
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
                .filter(|saved| saved.retired)
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
            if let Some(saved) = result.saved_author_call.as_ref().filter(|saved| saved.retired) {
                // Recovery belongs in the result, never in an invented input
                // parameter that the model may copy into its next write/patch.
                envelope.continuation = Some(ToolContinuation::NextCall {
                    tool: "presentation.author".into(),
                    arguments: json!({"operation":"read","candidate_id":saved.candidate_id,"file":"index.html"}),
                    reason: "The preceding write/patch arguments are a historical summary, not an executable call: saved html/readable_content and superseded edits may be omitted. Before editing this candidate, read/search its actual source and copy old_text exactly from it; readable_content is explanatory prose, not the HTML source. This locator grants no preview or delivery eligibility.".into(),
                });
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_review_uses_only_sampled_available_segments_with_original_conditions() {
        let mut ledger = ActiveToolResultLedger::default();
        let mut messages = vec![Message::user("Explain this mechanism")];
        for (id, tool, body) in [
            ("search", "web.search", json!({"results":[{"url":"https://example.com/summary","summary":"Unverified summary","content_status":"summary_only"}]})),
            ("blocked", "web.fetch", json!({"url":"https://example.com/login","content":"Sign in","content_status":"blocked"})),
            ("read", "web.fetch", json!({"url":"https://example.com/doc","title":"Primary source","content":"The result applies only under condition A.","content_status":"available","offset":400})),
        ] {
            ledger.insert(id, ToolResultEnvelope { version:TOOL_RESULT_ENVELOPE_VERSION.into(), status:ToolResultStatus::Ok,
                model_body:body, receipt:receipt(tool), truncated:false, continuation:None });
            messages.push(Message {role:Role::Tool,content:None,tool_calls:vec![],tool_call_id:Some(id.into()),provider_continuation:None});
        }
        assert!(ledger.web_review_evidence(&messages)["pages"].as_array().unwrap().is_empty());
        ledger.mark_projected_fresh_results_sampled();
        let evidence = ledger.web_review_evidence(&messages);
        assert_eq!(evidence["pages"].as_array().unwrap().len(), 1);
        assert_eq!(evidence["pages"][0]["text"], "The result applies only under condition A.");
        assert_eq!(evidence["pages"][0]["offset"], 400);
        assert!(evidence["pages"][0].get("lid").is_none());
        ledger.retire_sampled_bodies();
        let retired = ledger.web_review_evidence(&messages);
        assert!(retired["pages"].as_array().unwrap().is_empty());
        assert_eq!(retired["omitted"], true);
    }

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

    fn projected_author_read(ledger: &ActiveToolResultLedger, messages: &[Message], id: &str) -> Value {
        let mut projected = messages.to_vec();
        ledger.project_messages(&mut projected);
        let result: Value = serde_json::from_str(projected.iter().rfind(|m| m.tool_call_id.as_deref() == Some(id)).unwrap().content.as_deref().unwrap()).unwrap();
        assert!(result["continuation"]["reason"].as_str().unwrap().contains("not an executable call"));
        let read = result["continuation"]["arguments"].clone();
        crate::orchestrator::resident_tool_registry().registration("presentation.author").unwrap().validate_arguments(&read.to_string()).unwrap();
        read
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
        let mut first = messages.clone();
        ledger.project_messages(&mut first);
        assert!(!ledger.retire_sampled_bodies(), "unobserved results are not reclaimable even under pressure");
        ledger.mark_projected_fresh_results_sampled();
        let mut observed = messages.clone();
        ledger.project_messages(&mut observed);
        assert_eq!(observed, first, "successful sampling cannot change any previous provider message");
        for id in ["failed", "missing-landing", "wrong-status"] {
            assert_eq!(projected_author_args(&ledger, &messages, id), args);
        }
        assert_eq!(projected_author_args(&ledger, &messages, "saved"), args);
        assert!(ledger.retire_sampled_bodies());
        let compact = projected_author_args(&ledger, &messages, "saved");
        assert!(compact.get("html").is_none() && compact.get("readable_content").is_none());
        for field in ["operation","title","based_on","new_object","source_ref_ids","asset_refs","libraries","assumptions","state_contract","initial_state"] {
            assert_eq!(compact[field], args[field]);
        }
        assert!(compact.get("saved_source").is_none());
        assert_eq!(projected_author_read(&ledger, &messages, "saved"), json!({"operation":"read","candidate_id":"c1","file":"index.html"}));
        // Source, result body and recovery instruction change together. The
        // immutable candidate remains the recovery target after retirement.
        let mut projected = messages.clone();
        ledger.project_messages(&mut projected);
        let envelope: ToolResultEnvelope = serde_json::from_str(projected.last().unwrap().content.as_deref().unwrap()).unwrap();
        assert!(envelope.model_body.is_null());
        assert_eq!(projected_author_read(&ledger, &messages, "saved")["candidate_id"], "c1");
        assert_eq!(envelope.receipt, receipt("presentation.author"));
        assert_eq!(projected_author_args(&ledger, &messages, "saved"), compact);
        assert!(!ledger.retire_sampled_bodies(), "the same pressure boundary must not keep changing history");
        assert_eq!(projected_author_args(&ActiveToolResultLedger::default(), &messages, "saved"), args, "a new run cannot recover eligibility from history");
        let incomplete = &messages[..messages.len() - 1];
        assert_eq!(projected_author_args(&ledger, incomplete, "saved"), args, "unpaired call must stay intact");
    }

    #[test]
    fn ex13_patch_chain_retires_edits_only_at_pressure_after_observed_child_save() {
        let mut ledger = ActiveToolResultLedger::default();
        let mut messages = vec![Message::user("Revise this exact version")];
        let edits = json!([{"old_text":"original","new_text":"corrected"}]);
        author_save(&mut ledger, &mut messages, "patch-1", json!({"operation":"patch","reference":{"presentation_id":"p","revision":2},"edits":edits,"readable_content":"corrected"}), json!({"status":"candidate_saved","candidate_id":"p1"}));
        ledger.mark_projected_fresh_results_sampled();
        let recent = projected_author_args(&ledger, &messages, "patch-1");
        assert_eq!(recent["edits"], edits);
        assert_eq!(recent["readable_content"], "corrected");
        let mut first = messages.clone();
        ledger.project_messages(&mut first);
        author_save(&mut ledger, &mut messages, "independent", json!({"operation":"write","html":"different work","readable_content":"different"}), json!({"status":"candidate_saved","candidate_id":"other"}));
        author_save(&mut ledger, &mut messages, "failed-child", json!({"operation":"patch","candidate_id":"p1","edits":edits}), json!({"error_code":"PRESENTATION_AUTHORING_FAILED"}));
        ledger.mark_projected_fresh_results_sampled();
        assert_eq!(projected_author_args(&ledger, &messages, "patch-1"), recent);
        let child = json!({"operation":"patch","candidate_id":"p1","edits":[{"old_text":"corrected","new_text":"final"}]});
        author_save(&mut ledger, &mut messages, "patch-2", child.clone(), json!({"status":"candidate_saved","candidate_id":"p2"}));
        assert_eq!(projected_author_args(&ledger, &messages, "patch-1"), recent, "unobserved child must not retire recent edits");
        let mut with_child = messages.clone();
        ledger.project_messages(&mut with_child);
        assert_eq!(&with_child[..first.len()], first.as_slice(), "adding a child must preserve the previously sent prefix");
        ledger.mark_projected_fresh_results_sampled();
        let mut observed_child = messages.clone();
        ledger.project_messages(&mut observed_child);
        assert_eq!(observed_child, with_child, "observing a child must not retire its parent's edits");
        assert!(ledger.retire_sampled_bodies());
        let retired = projected_author_args(&ledger, &messages, "patch-1");
        assert!(retired.get("edits").is_none());
        assert_eq!(retired["reference"], recent["reference"]);
        assert!(retired.get("saved_source").is_none());
        assert_eq!(projected_author_read(&ledger, &messages, "patch-1")["candidate_id"], "p1");
        let latest = projected_author_args(&ledger, &messages, "patch-2");
        assert_eq!(latest["candidate_id"], "p1");
        assert!(latest.get("saved_source").is_none());
        assert_eq!(projected_author_read(&ledger, &messages, "patch-2")["candidate_id"], "p2");
        assert_eq!(latest["edits"], child["edits"]);
        ledger.mark_projected_fresh_results_sampled();
        assert_eq!(projected_author_args(&ledger, &messages, "patch-1"), retired, "projection must stay stable");
        let mut after_pressure = messages.clone();
        ledger.project_messages(&mut after_pressure);
        assert!(!ledger.retire_sampled_bodies());
        let mut repeated = messages.clone();
        ledger.project_messages(&mut repeated);
        assert_eq!(repeated, after_pressure);
    }

    #[test]
    fn author_retirement_keeps_fresh_child_and_latest_edits_until_the_next_pressure_boundary() {
        let mut ledger = ActiveToolResultLedger::default();
        let mut messages = vec![Message::user("Revise the same candidate")];
        let edits = json!([{"old_text":"old","new_text":"x".repeat(ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES)}]);
        let parent = json!({"operation":"patch","candidate_id":"initial","edits":edits,"readable_content":"parent"});
        author_save(&mut ledger, &mut messages, "parent", parent.clone(), json!({"status":"candidate_saved","candidate_id":"p1"}));
        ledger.mark_projected_fresh_results_sampled();
        let child = json!({"operation":"patch","candidate_id":"p1","edits":[{"old_text":"x","new_text":"final"}],"readable_content":"child"});
        author_save(&mut ledger, &mut messages, "child", child.clone(), json!({"status":"candidate_saved","candidate_id":"p2"}));
        assert!(ledger.sampled_reclaimable_bytes() < ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES,
            "an unread child cannot make the parent's latest edits reclaimable");
        assert!(ledger.retire_sampled_bodies());
        assert_eq!(projected_author_args(&ledger, &messages, "parent")["edits"], edits);
        assert_eq!(projected_author_args(&ledger, &messages, "child"), child);
        let mut first_boundary = messages.clone();
        ledger.project_messages(&mut first_boundary);
        let fresh: ToolResultEnvelope = serde_json::from_str(first_boundary.last().unwrap().content.as_deref().unwrap()).unwrap();
        assert_eq!(fresh.model_body["candidate_id"], "p2");
        assert!(fresh.continuation.is_none(), "an unread child must retain its original result");
        ledger.mark_projected_fresh_results_sampled();
        let mut observed = messages.clone();
        ledger.project_messages(&mut observed);
        assert_eq!(observed, first_boundary, "sampling after retirement must not silently cause another rebuild");
        assert!(ledger.sampled_reclaimable_bytes() >= ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES,
            "superseded assistant arguments must count toward the next pressure batch");
        assert!(ledger.retire_sampled_bodies(), "a retired body's edits can become reclaimable later");
        assert!(projected_author_args(&ledger, &messages, "parent").get("edits").is_none());
        assert_eq!(projected_author_args(&ledger, &messages, "child")["edits"], child["edits"]);
        assert_eq!(projected_author_read(&ledger, &messages, "parent")["candidate_id"], "p1");
        assert_eq!(projected_author_read(&ledger, &messages, "child")["candidate_id"], "p2");
    }

    #[test]
    fn sampled_author_html_counts_toward_reclaimable_pressure_without_rewriting_messages() {
        let mut ledger = ActiveToolResultLedger::default();
        let mut messages = vec![Message::user("Create a page")];
        let arguments = json!({"operation":"write","html":"x".repeat(ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES),"readable_content":"small explanation"});
        author_save(&mut ledger, &mut messages, "write", arguments.clone(), json!({"status":"candidate_saved","candidate_id":"page"}));
        assert_eq!(ledger.sampled_reclaimable_bytes(), 0);
        ledger.mark_projected_fresh_results_sampled();
        assert!(ledger.sampled_model_body_bytes() < ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES);
        assert!(ledger.sampled_reclaimable_bytes() >= ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES);
        assert_eq!(projected_author_args(&ledger, &messages, "write"), arguments);
        assert!(ledger.retire_sampled_bodies());
        assert_eq!(ledger.sampled_reclaimable_bytes(), 0);
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

    #[test]
    fn teaching_design_receipt_keeps_complete_reference_without_repeating_context() {
        let reference = "design:turn_b5910f5c3cdf0ec6:borrowed-access";
        let raw = json!({"design_ref":reference,"instruction":"Consume the accepted design before choosing teaching support.",
            "design":{"organizing_model":"One owned value and a borrowed access path"},
            "learner_context_update":{"interpretation":"A detailed learner explanation. ".repeat(1000)}});
        for limit in [4096, 80] {
            let projected = project_tool_result("tutor.step",r#"{"operation":"design"}"#,&raw.to_string(),
                policy(ToolResultPolicy::MemoryReceipt,limit),limit,&two_leaf_book());
            assert_eq!(projected.model_body["design_ref"],reference);
            assert!(projected.model_body.get("design").is_none());
            assert!(projected.model_body.get("learner_context_update").is_none());
            assert!(!projected.truncated);
            assert!(json_len(&projected.model_body)<=limit);
        }
    }

    #[test]
    fn teaching_evidence_receipt_keeps_identity_without_historical_learner_snapshot() {
        let reference = "interpretation:turn_b5910f5c3cdf0ec6:revise-relation";
        let raw = json!({"evidence_ref":reference,"nature":"hypothesis","instruction":"Use this current interpretation.",
            "teaching_implication":"A detailed teaching implication. ".repeat(500),
            "learner_context_update":{"interpretations":[{"interpretation":"The learner's latest explanation. ".repeat(500)}]}});
        for limit in [4096, 128] {
            let projected=project_tool_result("tutor.step",r#"{"operation":"evidence"}"#,&raw.to_string(),
                policy(ToolResultPolicy::MemoryReceipt,limit),limit,&two_leaf_book());
            assert_eq!(projected.model_body["evidence_ref"],reference);
            assert!(projected.model_body.get("learner_context_update").is_none());
            assert!(!projected.truncated);
            assert!(json_len(&projected.model_body)<=limit);
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
    fn board_read_budget_continuations_recover_all_nine_items_and_long_answer_layout() {
        use crate::board_author::{read_page, AuthorRequest};
        let items:Vec<_>=(0..9).map(|i|json!({"item_id":format!("item-{i}"),"title":format!("Board item {i}"),
            "position":{"x":i as f64*320.5,"y":760.25},"size":{"width":290.25,"height":180.75},
            "readable_content":"完整保留的讲解。".repeat(160),"explanation_blocks":[],
            "target":{"kind":if i>=6 {"drawing"} else {"answer"}}})).collect();
        let raw=json!({"status":"board_read","board":{"board_id":"board-paging","revision":9,"items":items,
                "links":[{"link_id":"link-last","from":{"kind":"item","item_id":"item-7"},"to":{"kind":"item","item_id":"item-8"}}],
                "mappings":[],"routes":[],"deliveries":[]},
            "reserved_answer_item_id":"answer-new","candidate_revision":0,"candidate":null,
            "proposed_content_revision":10,"answer_position":{"x":400,"y":800},"answer_size":{"width":320,"height":220},
            "understanding_items":[],"understanding_refs":[],"formal_condition_refs":[]});
        let book=two_leaf_book();
        let registry=crate::orchestrator::resident_tool_registry();
        let registration=registry.registration("board.author").unwrap();
        let project=|args:&Value,body:&Value,limit|project_tool_result("board.author",&args.to_string(),&body.to_string(),
            policy(ToolResultPolicy::EvidenceProjection,limit),limit,&book);
        let first=project(&json!({"operation":"read"}),&raw,16*1024);
        assert!(first.truncated,"the observed multi-answer board exceeds the real per-tool allowance");
        let mut recovered=first.model_body["board"]["items"].as_array().cloned().unwrap_or_default();
        let mut pending=first.model_body["next_calls"].as_array().unwrap().iter()
            .filter(|call|call["arguments"]["collection"]=="items").map(|call|call["arguments"].clone()).collect::<Vec<_>>();
        assert!(!pending.is_empty(),"omitted native geometry must have an executable read");
        while let Some(args)=pending.pop() {
            registration.validate_arguments(&args.to_string()).unwrap();
            let AuthorRequest::Read{collection,offset,length,board_revision,candidate_revision}=serde_json::from_value(args.clone()).unwrap() else {panic!()};
            let page=read_page(raw.clone(),collection,offset,length,board_revision,candidate_revision).unwrap();
            let delivered=project(&args,&page,16*1024);
            assert_eq!(delivered.model_body["reserved_answer_item_id"],raw["reserved_answer_item_id"]);
            assert_eq!(delivered.model_body["proposed_content_revision"],raw["proposed_content_revision"]);
            let rows=delivered.model_body["records"].as_array().unwrap();
            assert!(!rows.is_empty(),"continuations must make progress with complete records");
            recovered.extend(rows.iter().cloned());
            pending.extend(delivered.model_body["next_calls"].as_array().unwrap().iter().map(|call|call["arguments"].clone()));
        }
        assert_eq!(recovered,raw["board"]["items"].as_array().unwrap().clone());

        // A large retained explanation must never hide occupied canvas space.
        let mut long=raw.clone();
        long["board"]["items"][0]["readable_content"]=json!("长回答".repeat(30000));
        let mut next=json!({"operation":"read","collection":"layout","offset":0,"length":20,"board_revision":9});
        let mut geometry=vec![];
        let mut page_count=0;
        loop {
            registration.validate_arguments(&next.to_string()).unwrap();
            let AuthorRequest::Read{collection,offset,length,board_revision,candidate_revision}=serde_json::from_value(next.clone()).unwrap() else {panic!()};
            let page=read_page(long.clone(),collection,offset,length,board_revision,candidate_revision).unwrap();
            let delivered=project(&next,&page,1200);
            for field in ["reserved_answer_item_id","proposed_content_revision","answer_position","answer_size"] {
                assert_eq!(delivered.model_body[field],raw[field],"{field} must survive paginated layout projection");
            }
            let rows=delivered.model_body["records"].as_array().unwrap();
            assert!(!rows.is_empty());
            geometry.extend(rows.iter().cloned());
            page_count+=1;
            let Some(ToolContinuation::NextCall{arguments,..})=delivered.continuation else {break;};
            assert!(arguments["offset"].as_u64().unwrap()>next["offset"].as_u64().unwrap());
            next=arguments;
        }
        assert!(page_count>1,"test must exercise budget-generated layout continuations");
        assert_eq!(geometry.len(),9);
        for (index,item) in geometry.iter().enumerate() {
            assert_eq!(item["item_id"],raw["board"]["items"][index]["item_id"]);
            assert_eq!(item["position"],raw["board"]["items"][index]["position"]);
            assert_eq!(item["size"],raw["board"]["items"][index]["size"]);
            assert!(item.get("readable_content").is_none());
        }
        long["board"]["revision"]=json!(10);
        let AuthorRequest::Read{collection,offset,length,board_revision,candidate_revision}=serde_json::from_value(next).unwrap() else {panic!()};
        assert_eq!(read_page(long,collection,offset,length,board_revision,candidate_revision).unwrap_err().error_code,"BOARD_REVISION_CONFLICT");
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
            policy(ToolResultPolicy::EvidenceProjection, 850),
            850,
            &book,
        );

        assert_eq!(draft.status, ToolResultStatus::Partial);
        assert!(draft.truncated);
        assert!(draft.model_body_json().len() <= 850);
        assert_eq!(draft.model_body["lid"], "1");
        assert_eq!(draft.model_body["text"].as_str().unwrap().len(), 600);
        assert_eq!(
            serde_json::from_str::<Value>(&draft.evidence_arguments).unwrap(),
            json!({"lid":"1"})
        );
        assert_eq!(draft.model_body["source_range"], json!({"start":0,"end":600}));
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

    fn ranked_search_hit(index: usize) -> Value {
        let start = index * 400;
        json!({
            "rank":index + 1,"score":10.0 / (index + 1) as f64,
            "start_lid":"1.1","end_lid":"1.1",
            "source_range_utf16":{"start":start,"end":start + 400},
            "ranges":[{"lid":"1.1","start_utf16":start,"end_utf16":start + 400}],
            "heading_path":["Full heading that must not be shortened"],
            "excerpt":format!("Candidate {index}: {}", "相关段落".repeat(60)),
            "matched_terms":["term","another term"],
            "next_calls":[{"tool":"book.text","arguments":{"lid":"1.1","start_utf16":start,"max_chars":400}}],
        })
    }

    #[test]
    fn ranked_search_projection_keeps_candidates_and_read_calls_atomic() {
        let hits: Vec<_> = (0..8).map(ranked_search_hit).collect();
        let raw = json!({"version":"search_ranked.v1","query":"related content","total_candidates":42,"hits":hits});
        let draft = project_tool_result("book.search", r#"{"query":"related content"}"#, &raw.to_string(),
            policy(ToolResultPolicy::EvidenceProjection, 2200), 2200, &two_leaf_book());
        let kept = draft.model_body["hits"].as_array().unwrap();
        assert!(!kept.is_empty() && kept.len() < hits.len());
        assert_eq!(kept, &hits[..kept.len()], "No excerpt, coordinate or read call may be partially shortened");
        let calls: Vec<_> = kept.iter().flat_map(|hit| hit["next_calls"].as_array().unwrap().iter().cloned()).collect();
        assert_eq!(draft.model_body["next_calls"], json!(calls));
        assert_eq!(draft.model_body["total_candidates"], 42);
        assert_eq!(draft.model_body["exhaustive"], false);
        assert_eq!(draft.model_body["delivery"]["evidence"], "locators_only");
        assert!(draft.model_body.get("text").is_none());
        assert!(draft.model_body.get("next_cursor").is_none());
        assert!(draft.continuation.is_none(), "Ranked candidates have no exhaustive paging continuation");
        assert_eq!(draft.status, ToolResultStatus::Partial);
        assert!(draft.truncated);
        assert!(draft.model_body_json().len() <= 2200);
    }

    #[test]
    fn workspace_ranked_search_projection_preserves_publication_identity() {
        let hits: Vec<_> = (0..8).map(|index| {
            let mut hit = ranked_search_hit(index);
            let reference = json!({"book_id":format!("material-{index}"),"publication_id":format!("publication-{index}-{}", "p".repeat(50))});
            hit["published_book_ref"] = reference.clone();
            hit["source_revision"] = json!(format!("source-{index}"));
            hit["title"] = json!(format!("Material {index}"));
            let mut args = hit["next_calls"][0]["arguments"].clone();
            args["published_book_ref"] = reference;
            args["operation"] = json!("text");
            hit["next_calls"] = json!([{"name":"library.read","arguments":args}]);
            hit
        }).collect();
        let calls: Vec<_> = hits.iter().flat_map(|hit| hit["next_calls"].as_array().unwrap().iter().cloned()).collect();
        let retrieval = json!({"lexical":"bm25","semantic":{"status":"disabled","reason":"not configured"}});
        let scope = json!({"workspace_id":"workspace-1","materials":8});
        let raw = json!({"version":"workspace_search.v1","query":"related content","retrieval":retrieval,
            "scope":scope,"exhaustive":false,"hits":hits,"next_calls":calls});
        let draft = project_tool_result("library.search", r#"{"query":"related content"}"#, &raw.to_string(),
            policy(ToolResultPolicy::EvidenceProjection, 2600), 2600, &two_leaf_book());
        let kept = draft.model_body["hits"].as_array().unwrap();
        assert!(!kept.is_empty() && kept.len() < hits.len());
        assert_eq!(kept, &hits[..kept.len()]);
        assert_eq!(draft.model_body["next_calls"], json!(&calls[..kept.len()]));
        assert_eq!(draft.model_body["retrieval"], retrieval);
        assert_eq!(draft.model_body["scope"], scope);
        assert!(draft.model_body_json().len() <= 2600);
        assert!(draft.continuation.is_none());
    }

    #[test]
    fn library_read_ranked_search_filters_envelope_calls_by_exact_candidate_range() {
        let reference = json!({"book_id":"supplement","publication_id":"edition-2"});
        let mut hits: Vec<_> = (0..8).map(ranked_search_hit).collect();
        let calls: Vec<_> = hits.iter_mut().map(|hit| {
            let mut args = hit["next_calls"][0]["arguments"].clone();
            args["published_book_ref"] = reference.clone();
            args["operation"] = json!("text");
            hit.as_object_mut().unwrap().remove("next_calls");
            json!({"name":"library.read","arguments":args})
        }).collect();
        let raw = json!({"operation":"search","published_book_ref":reference,
            "data":{"version":"search_ranked.v1","query":"related content","hits":hits},
            "next_calls":calls,"sources":[{"excerpt":"This search locator is not source evidence"}],"complete":true});
        let args = json!({"operation":"search","published_book_ref":reference,"query":"related content"});
        let draft = project_tool_result("library.read", &args.to_string(), &raw.to_string(),
            policy(ToolResultPolicy::EvidenceProjection, 2400), 2400, &two_leaf_book());
        let kept = draft.model_body["data"]["hits"].as_array().unwrap();
        assert!(!kept.is_empty() && kept.len() < hits.len());
        assert_eq!(kept, &hits[..kept.len()]);
        assert_eq!(draft.model_body["published_book_ref"], reference);
        assert_eq!(draft.model_body["next_calls"], json!(&calls[..kept.len()]));
        assert!(draft.model_body.get("sources").is_none());
        assert_eq!(draft.model_body["data"]["exhaustive"], false);
        assert!(draft.continuation.is_none());
        assert!(draft.model_body_json().len() <= 2400);
    }

    #[test]
    fn library_read_ranked_search_scopes_nested_calls_before_delivery() {
        let reference = json!({"book_id":"supplement","publication_id":"edition-2"});
        let hit = ranked_search_hit(0);
        let raw = json!({"operation":"search","published_book_ref":reference,
            "data":{"version":"search_ranked.v1","query":"related content","hits":[hit]},"next_calls":[]});
        let args = json!({"operation":"search","published_book_ref":reference,"query":"related content"});
        let draft = project_tool_result("library.read", &args.to_string(), &raw.to_string(),
            policy(ToolResultPolicy::EvidenceProjection, 8000), 8000, &two_leaf_book());
        assert_eq!(draft.status, ToolResultStatus::Ok);
        assert!(!draft.truncated);
        let call = &draft.model_body["data"]["hits"][0]["next_calls"][0];
        assert_eq!(call["name"], "library.read");
        assert_eq!(call["arguments"]["published_book_ref"], reference);
        assert_eq!(call["arguments"]["operation"], "text");
        assert_eq!(call["arguments"]["max_chars"], 400);
        assert_eq!(draft.model_body["next_calls"], json!([call]));
    }

    #[test]
    fn ranked_search_projection_omits_oversized_hit_without_read_hints_or_paging() {
        for tool in ["book.search", "library.search"] {
            let raw = json!({"version":"search_ranked.v1","query":"related content","hits":[ranked_search_hit(0)]});
            let draft = project_tool_result(tool, r#"{"query":"related content"}"#, &raw.to_string(),
                policy(ToolResultPolicy::EvidenceProjection, 450), 450, &two_leaf_book());
            assert_eq!(draft.status, ToolResultStatus::Partial);
            assert!(draft.truncated);
            assert_eq!(draft.model_body["projection"], "semantic_budget_exceeded");
            assert!(draft.model_body.get("hits").is_none());
            assert!(draft.model_body.get("next_calls").is_none());
            assert!(draft.model_body_json().len() <= 450);
            let Some(ToolContinuation::RefineCall { guidance, .. }) = draft.continuation else {
                panic!("No complete hit should suggest narrowing, not paging or reading an omitted hit");
            };
            assert!(guidance.contains("Narrow the query or scope"));
        }
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
            // A manifest has neither paging nor a field-selection parameter.
            // Its delivery metadata explains the budget limit instead of
            // suggesting the same ineffective request as a refinement.
            assert_eq!(draft.continuation.is_some(), tool == "book.query");
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
    fn active_tool_results_do_not_replace_a_prior_turn_with_the_same_call_id() {
        let mut messages=vec![Message::user("The earlier page")];
        let mut previous=ActiveToolResultLedger::default();
        author_save(&mut previous,&mut messages,"save",json!({"operation":"write","title":"old","html":"OLD_PAGE_HTML","readable_content":"Old meaning"}),
            json!({"status":"candidate_saved","candidate_id":"old-candidate"}));
        messages.push(Message {provider_continuation:None,role:Role::Assistant,content:Some("The earlier answer".into()),tool_calls:vec![],tool_call_id:None});
        let historical=messages.clone();
        messages.push(Message::user("The current page"));
        let mut current=ActiveToolResultLedger::default();
        author_save(&mut current,&mut messages,"save",json!({"operation":"write","title":"new","html":"CURRENT_PAGE_HTML","readable_content":"Current meaning"}),
            json!({"status":"candidate_saved","candidate_id":"current-candidate"}));
        current.mark_projected_fresh_results_sampled();
        let mut projected=messages.clone();
        current.project_messages(&mut projected);
        assert_eq!(&projected[..historical.len()],historical.as_slice(),"current results and saved-author projections must not overwrite earlier turns sharing an ID");
        let active_args=&projected[historical.len()+1].tool_calls[0].arguments;
        assert!(active_args.contains("CURRENT_PAGE_HTML"),"successful sampling must preserve the current source until pressure");
        let active:ToolResultEnvelope=serde_json::from_str(projected.last().unwrap().content.as_deref().unwrap()).unwrap();
        assert_eq!(active.model_body["candidate_id"],"current-candidate");
        assert!(current.retire_sampled_bodies());
        let mut retired=messages.clone();
        current.project_messages(&mut retired);
        assert_eq!(&retired[..historical.len()],historical.as_slice());
        assert!(!retired[historical.len()+1].tool_calls[0].arguments.contains("CURRENT_PAGE_HTML"));
        assert_eq!(projected_author_read(&current, &messages, "save")["candidate_id"], "current-candidate");
        assert!(messages[historical.len()+1].tool_calls[0].arguments.contains("CURRENT_PAGE_HTML"),"projection must not rewrite durable history");
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

        assert!(ledger.retire_sampled_bodies());
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
    fn cc2_sampled_bodies_remain_when_new_results_cross_the_old_rolling_limit() {
        let mut ledger = ActiveToolResultLedger::default();
        let book = two_leaf_book();
        let mut messages = Vec::new();
        for index in 0..3 {
            let raw = json!({"status":"version_read","file":"index.html","offset":0,
                "end_offset":35000,"total_characters":35000,"chunk_characters":35000,
                "text":"a".repeat(35000),"next_offset":null});
            let draft = ledger.project_result(
                "presentation.author", r#"{"operation":"read","file":"index.html"}"#,
                &raw.to_string(), policy(ToolResultPolicy::EvidenceProjection, 16384), &book,
            );
            assert!(!draft.truncated, "sampled history must not consume the fresh-result allowance");
            assert_eq!(draft.model_body["text"].as_str().unwrap().len(), 35000);
            let id = format!("cc2-read-{index}");
            ledger.insert(id.clone(), draft.into_envelope(receipt("presentation.author")));
            messages.push(Message {
                provider_continuation: None, role: Role::Tool,
                content: Some(format!("durable raw result {index}")),
                tool_calls: Vec::new(), tool_call_id: Some(id),
            });
            let mut projected = messages.clone();
            ledger.project_messages(&mut projected);
            for message in &projected {
                let envelope: ToolResultEnvelope = serde_json::from_str(message.content.as_deref().unwrap()).unwrap();
                assert_eq!(envelope.model_body["text"].as_str().map(str::len), Some(35000),
                    "new results must not retire any already sampled body");
            }
            ledger.mark_projected_fresh_results_sampled();
        }
        assert!(ledger.by_call_id.values().map(|result| json_len(&result.envelope.model_body)).sum::<usize>()
            > ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES);
        assert_eq!(messages[0].content.as_deref(), Some("durable raw result 0"));
    }

    #[test]
    fn cc2_fresh_batch_allowance_excludes_sampled_history() {
        let mut ledger = ActiveToolResultLedger::default();
        ledger.insert("sampled", ToolResultEnvelope {
            version: TOOL_RESULT_ENVELOPE_VERSION.into(), status: ToolResultStatus::Ok,
            model_body: json!({"text":"x".repeat(40_000)}), receipt: receipt("book.text"),
            truncated: false, continuation: None,
        });
        ledger.mark_projected_fresh_results_sampled();
        assert_eq!(ledger.remaining_model_body_bytes(), ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES);
        let fresh = json!({"text":"new unsampled source"});
        ledger.insert("fresh", ToolResultEnvelope {
            version: TOOL_RESULT_ENVELOPE_VERSION.into(), status: ToolResultStatus::Ok,
            model_body: fresh.clone(), receipt: receipt("book.text"),
            truncated: false, continuation: None,
        });
        assert_eq!(ledger.remaining_model_body_bytes(),
            ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES - json_len(&fresh));
        assert!(ledger.by_call_id["sampled"].retain_model_body);
        assert!(ledger.by_call_id["fresh"].retain_model_body);
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
    fn cross_book_collection_budget_reassembles_without_skipping_items_or_changing_identity() {
        let reference=json!({"book_id":"rust","publication_id":"rust-published-edition-1"});
        let original:Vec<_>=(0..18).map(|i|json!({"lid":format!("1.{i}"),"summary":"长描述".repeat(30)})).collect();
        let mut args=json!({"operation":"context","published_book_ref":reference,"lid":"1.1"});
        let mut rebuilt=vec![];
        for _ in 0..30 {
            let offset=args["page"]["offset"].as_u64().unwrap_or(0) as usize;
            let data=if args["page"].is_null() {json!({"anchor":"1.1","items":original})} else {json!(&original[offset..])};
            let raw=json!({"operation":"context","published_book_ref":reference,"data":data,
                "data_path":if args["page"].is_null(){""}else{"/items"},"sources":[],"complete":true,"next_calls":[]});
            let draft=project_tool_result("library.read",&args.to_string(),&raw.to_string(),policy(ToolResultPolicy::EvidenceProjection,1500),1500,&two_leaf_book());
            assert_eq!(draft.model_body["published_book_ref"],reference);
            let delivered=if draft.model_body["data_path"]=="" {&draft.model_body["data"]["items"]}else{&draft.model_body["data"]};
            rebuilt.extend(delivered.as_array().unwrap().clone());
            if !draft.truncated {break;}
            let Some(ToolContinuation::NextCall {arguments,..})=draft.continuation else {panic!("omitted items need continuation")};
            assert!(arguments["page"]["offset"].as_u64().unwrap()>offset as u64);
            args=arguments;
        }
        assert_eq!(rebuilt,original);
    }

    #[test]
    fn cross_book_understanding_budget_preserves_whole_records_and_exact_next_offset() {
        let reference = json!({"book_id":"rust","publication_id":"p".repeat(70)});
        let records: Vec<_> = (0..12).map(|i| json!({"evidence_id":format!("learning-record-{i}-{}", "e".repeat(60)),
            "interpretation":"理解及其局限".repeat(24),"assistance_refs":["assistance".repeat(10)],"status":"unassessed"})).collect();
        let mut args = json!({"operation":"read","published_book_ref":reference});
        let mut received = vec![];
        loop {
            let offset = args["after"].as_u64().unwrap_or(0) as usize;
            let raw = json!({"published_book_ref":reference,"rows":[],"interpretations":&records[offset..],
                "recent_facts":[{"event_id":"fact","payload":"long history".repeat(2000)}],"next":null});
            let draft = project_tool_result("library.learning", &args.to_string(), &raw.to_string(),
                policy(ToolResultPolicy::MemoryProjection, 1700), 1700, &two_leaf_book());
            assert!(json_len(&draft.model_body) <= 1700);
            assert_eq!(draft.model_body["published_book_ref"], reference);
            let page = draft.model_body["interpretations"].as_array().unwrap();
            assert!(!page.is_empty());
            received.extend(page.clone());
            let Some(ToolContinuation::NextCall { arguments, .. }) = draft.continuation else { break; };
            assert_eq!(arguments["after"], received.len());
            args = arguments;
        }
        assert_eq!(received, records);
    }

    #[test]
    fn se2_method_projection_reassembles_actual_source_with_contiguous_pages() {
        let source = include_str!("../../../skills/presentation/methods/discrete-update/v1/example/index.html");
        let chars: Vec<_> = source.chars().collect();
        for page_size in [chars.len(), 1200, 6000] {
            let mut args = json!({"operation":"read_method","method_id":"discrete-update","revision":1,
                "file":"example/index.html","offset":0,"length":page_size});
            let mut reconstructed = String::new();
            loop {
                let offset = args["offset"].as_u64().unwrap() as usize;
                let end = (offset + page_size).min(chars.len());
                let raw = json!({"status":"method_read","method_id":"discrete-update","revision":1,
                    "file":"example/index.html","offset":offset,"end_offset":end,
                    "total_characters":chars.len(),"chunk_characters":end-offset,
                    "next_offset":(end < chars.len()).then_some(end),
                    "text":chars[offset..end].iter().collect::<String>()});
                let draft = project_tool_result("presentation.author", &args.to_string(), &raw.to_string(),
                    policy(ToolResultPolicy::EvidenceProjection, 900), 900, &two_leaf_book());
                assert!(json_len(&draft.model_body) <= 900);
                assert_eq!(draft.model_body["method_id"], "discrete-update");
                assert_eq!(draft.model_body["revision"], 1);
                let text = draft.model_body["text"].as_str().expect("contiguous method source");
                assert!(!text.is_empty());
                let actual_end = offset + text.chars().count();
                assert!(text.chars().count() <= page_size);
                assert_eq!(draft.model_body["end_offset"], actual_end);
                assert_eq!(draft.model_body["next_offset"], json!((actual_end < chars.len()).then_some(actual_end)));
                reconstructed.push_str(text);
                match draft.continuation {
                    Some(ToolContinuation::NextCall { arguments, .. }) => {
                        assert_eq!(arguments["operation"], "read_method");
                        assert_eq!(arguments["file"], args["file"]);
                        assert_eq!(arguments["method_id"], args["method_id"]);
                        assert_eq!(arguments["revision"], args["revision"]);
                        assert_eq!(arguments["offset"], reconstructed.chars().count());
                        assert_eq!(arguments["length"], page_size, "continuation preserves the requested page size");
                        args = arguments;
                    }
                    None => break,
                    _ => panic!("expected the next contiguous method page"),
                }
            }
            assert_eq!(reconstructed, source, "all method pages must reassemble the file through EOF");
        }
    }

    #[test]
    fn se2_method_projection_fitting_6000_character_page_still_continues_to_eof() {
        for (offset, count, next_offset) in [(0, 6000, Some(6000)), (12000, 7, None)] {
            let args = json!({"operation":"read_method","method_id":"discrete-update","revision":1,
                "file":"example/index.html","offset":offset,"length":6000});
            let raw = json!({"status":"method_read","method_id":"discrete-update","revision":1,
                "file":"example/index.html","offset":offset,"end_offset":offset + count,
                "total_characters":12007,"chunk_characters":count,"next_offset":next_offset,
                "text":"a".repeat(count)});
            let draft = project_tool_result("presentation.author", &args.to_string(), &raw.to_string(),
                policy(ToolResultPolicy::EvidenceProjection, 6500), 6500, &two_leaf_book());
            assert!(!draft.truncated, "the complete page fits the result budget");
            assert_eq!(draft.model_body, raw);
            match (next_offset, draft.continuation) {
                (Some(offset), Some(ToolContinuation::NextCall { tool, arguments, .. })) => {
                    assert_eq!(tool, "presentation.author");
                    let mut expected = args;
                    expected["offset"] = json!(offset);
                    assert_eq!(arguments, expected, "only offset changes, not the requested page size or file identity");
                }
                (None, None) => {},
                _ => panic!("a full-sized method page must continue until the actual EOF"),
            }
        }
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
