//! Bounded, deterministic source prefetch inside the Resident loop. The book is
//! the backing store; only exact text delivered to the model becomes evidence.
use super::*;
use read_tools::reading_support::{ReadingPassage, ReadingSupport};
use serde_json::{json, Value};

const TURN_INLINE_BYTES: usize = 12 * 1024;
const PASSAGE_INLINE_BYTES: usize = 6 * 1024;
const SHORT_SECTION_UNITS: usize = 2048;

pub(super) struct InlineContextCache {
    remaining: usize,
    admitted: Vec<base_schema::Span>,
    teaching: bool,
    question: String,
    last_reading: Option<ReadingSupport>,
}

impl Default for InlineContextCache {
    fn default() -> Self {
        Self {
            remaining: TURN_INLINE_BYTES,
            admitted: Vec::new(),
            teaching: false,
            question: String::new(),
            last_reading: None,
        }
    }
}

fn size(value: &Value) -> usize {
    value.to_string().len()
}

fn identity(book: &Book) -> Value {
    json!({"source_id":book.base.book_id,"source_revision":book.source_fingerprint()})
}

fn same_source(body: &Value, book: &Book) -> bool {
    body["context_cache"]["source_id"] == book.base.book_id
        && body["context_cache"]["source_revision"] == book.source_fingerprint()
}

fn read_call(lid: &str, end: Option<&str>) -> Value {
    let mut args = json!({"lid":lid});
    if let Some(end) = end {
        args["end_lid"] = json!(end);
    }
    args
}

fn contains_read(book: &Book, args: &Value, lid: &str) -> bool {
    let Some(first) = args["lid"].as_str() else {
        return false;
    };
    let end = args["end_lid"].as_str().unwrap_or(first);
    match (
        book.source_span(first),
        book.source_span(end),
        book.source_span(lid),
    ) {
        (Some(a), Some(b), Some(target)) => {
            let from = args["start_utf16"].as_u64().map_or(a.start, |v| v as usize);
            from <= target.start && target.end <= b.end
        }
        _ => false,
    }
}

impl InlineContextCache {
    pub(super) fn for_turn(question: &str, tutor: Option<&Value>) -> Self {
        Self {
            teaching: tutor.is_some_and(|t| t["status"] == "practice"),
            question: question.into(),
            ..Self::default()
        }
    }

    fn attach_reading(&mut self, body: &mut Value, plan: &ReadingSupport, limit: usize) {
        if self.last_reading.as_ref() == Some(plan) {
            return;
        }
        let note = json!(plan);
        let bytes = size(&note) + 12;
        if bytes <= 1024 && bytes <= self.remaining && size(body) + bytes <= limit {
            body["context_cache"]["reading"] = note;
            self.remaining -= bytes;
            self.last_reading = Some(plan.clone());
        }
    }

    pub(super) fn projection_policy(
        &self,
        tool: &str,
        mut policy: crate::tool_registry::ToolOutputPolicy,
        available: usize,
        book: &Book,
    ) -> crate::tool_registry::ToolOutputPolicy {
        if !tool.starts_with("book.") {
            return policy;
        }
        let limit = policy.max_model_body_bytes.min(available);
        let prefetch = if (matches!(tool, "book.search" | "book.context")
            || (self.teaching && tool == "book.concept"))
            && self.remaining >= 1024
        {
            PASSAGE_INLINE_BYTES.min(self.remaining).min(limit / 2)
        } else {
            0
        };
        let citations = match tool {
            "book.text" => 256,
            "book.text_batch" => 256 * crate::book_text_batch::MAX_RANGES,
            _ => 0,
        };
        policy.max_model_body_bytes =
            limit.saturating_sub(size(&identity(book)) + 32 + prefetch + citations);
        policy
    }

    /// A canonical read already knows its exact delivered range. Register it
    /// here so citing it does not need another model/tool round trip.
    pub(super) fn cite_read(
        &self,
        tool: &str,
        arguments: &str,
        body: &mut Value,
        book: &Book,
        limit: usize,
        evidence: &mut TurnEvidenceLedger,
    ) {
        if !same_source(body, book) {
            return;
        }
        let items = match tool {
            "book.text" => vec![(
                serde_json::from_str::<Value>(arguments).unwrap_or_default(),
                body.clone(),
            )],
            "book.text_batch" => body["items"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|item| (item["arguments"].clone(), item["result"].clone()))
                .collect(),
            _ => return,
        };
        for (args, result) in items {
            let mut observed = TurnEvidenceLedger::default();
            observe_tool_evidence(
                &mut observed,
                "book.text",
                &args.to_string(),
                &result.to_string(),
                book,
            );
            let Some(range) = observed.teaching_source_ranges().into_iter().next() else {
                continue;
            };
            // Complete short reads use the existing {lid, text} shape. Paged
            // reads carry source_range. Derive both from the verified range,
            // so a complete read gets a citation as well as a truncated page.
            let Some(first) = book
                .base
                .lid_nodes
                .iter()
                .find(|node| node.lid == range.start_lid)
            else {
                continue;
            };
            let Some(last) = book
                .base
                .lid_nodes
                .iter()
                .find(|node| node.lid == range.end_lid)
            else {
                continue;
            };
            let span = base_schema::Span {
                start: first.span.start
                    + range.ranges.first().map_or(0, |r| r.range.start as usize),
                end: range
                    .ranges
                    .last()
                    .map_or(last.span.end, |r| last.span.start + r.range.end as usize),
            };
            let Ok(resolved) = book.resolve_source(&range, SOURCE_PRESENTATION_LOCALE, None) else {
                continue;
            };
            let mut citation = json!({"start_lid":range.start_lid,"end_lid":range.end_lid,
                "source_range":span});
            if size(body) + size(&citation) + 160 > limit {
                break;
            }
            let Ok(source) = evidence.present_prepared(book, range, resolved) else {
                continue;
            };
            citation["source_ref_id"] = json!(source.source_ref_id);
            if !body["context_cache"]["citations"].is_array() {
                body["context_cache"]["citations"] = json!([]);
            }
            body["context_cache"]["citations"]
                .as_array_mut()
                .unwrap()
                .push(citation);
        }
    }

    fn admit(
        &mut self,
        book: &Book,
        mut args: Value,
        available: usize,
        evidence: &mut TurnEvidenceLedger,
    ) -> Option<Value> {
        // Reserve the item wrapper, continuation and opaque citation handle too.
        let limit = available
            .min(self.remaining)
            .min(PASSAGE_INLINE_BYTES)
            .checked_sub(768)?;
        if limit < 512 {
            return None;
        }
        let lid = args["lid"].as_str()?;
        let first = book.base.lid_nodes.iter().find(|n| n.lid == lid)?;
        let last = book
            .base
            .lid_nodes
            .iter()
            .find(|n| n.lid == args["end_lid"].as_str().unwrap_or(lid))?;
        let mut start = args["start_utf16"]
            .as_u64()
            .map_or(first.span.start, |v| v as usize);
        // Repeated search hits in a section reuse the already delivered passage.
        // A long passage advances only to the first still-unread interval.
        loop {
            let next = self
                .admitted
                .iter()
                .filter(|s| s.start <= start && start < s.end)
                .map(|s| s.end)
                .max()
                .unwrap_or(start);
            if next == start {
                break;
            }
            start = next;
        }
        if start >= last.span.end {
            return None;
        }
        args["start_utf16"] = json!(start);
        let input: book_tool_contracts::TextInput = serde_json::from_value(args.clone()).ok()?;
        let raw = book.read_text(&input).ok()?;
        let projection = crate::tool_result::project_tool_result(
            "book.text",
            &args.to_string(),
            &raw.to_string(),
            crate::tool_registry::ToolOutputPolicy {
                result_policy: crate::tool_registry::ToolResultPolicy::EvidenceProjection,
                max_model_body_bytes: limit,
            },
            limit,
            book,
        );
        let body = projection.model_body;
        if body["text"].as_str().is_none_or(str::is_empty) {
            return None;
        }
        let span: base_schema::Span = serde_json::from_value(body["source_range"].clone()).ok()?;
        let mut observed = TurnEvidenceLedger::default();
        observe_tool_evidence(
            &mut observed,
            "book.text",
            &args.to_string(),
            &body.to_string(),
            book,
        );
        let range = observed.teaching_source_ranges().into_iter().next()?;
        let resolved = book
            .resolve_source(&range, SOURCE_PRESENTATION_LOCALE, None)
            .ok()?;
        let mut item = json!({"arguments":args,"result":body});
        if let Some(next) = item["result"]["next_start_utf16"].as_u64() {
            let mut next_args = item["arguments"].clone();
            next_args["start_utf16"] = json!(next);
            item["next_call"] = json!({"tool":"book.text","arguments":next_args});
        }
        // The source ID is bounded by the existing source registry. Do not
        // register source text that will be omitted from the actual projection.
        if size(&item) + 128 > available.min(self.remaining) {
            return None;
        }
        let source = evidence.present_prepared(book, range, resolved).ok()?;
        item["source_ref_id"] = json!(source.source_ref_id);
        self.remaining -= size(&item);
        self.admitted.push(span);
        Some(item)
    }

    pub(super) fn enrich(
        &mut self,
        tool: &str,
        arguments: &str,
        body: &mut Value,
        book: &Book,
        limit: usize,
        evidence: &mut TurnEvidenceLedger,
    ) {
        if !tool.starts_with("book.")
            || !body.is_object()
            || body.get("error_code").is_some()
            || body["projection"] == "budget_exhausted"
        {
            return;
        }
        let metadata = identity(book);
        if size(body) + size(&metadata) + 32 > limit {
            return;
        }
        body["context_cache"] = metadata;
        let args: Value = serde_json::from_str(arguments).unwrap_or_default();
        let focus = args
            .get("focus")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let mut plan: Option<ReadingSupport> = body
            .as_object_mut()
            .and_then(|v| v.remove("reading"))
            .and_then(|v| serde_json::from_value(v).ok());
        if plan.is_none()
            && self.teaching
            && matches!(tool, "book.search" | "book.concept" | "book.text")
        {
            let query = args["query"].as_str().unwrap_or(&self.question);
            let leaf = if tool == "book.text" {
                args["lid"].as_str().and_then(|lid| {
                    book.reading_leaf(
                        query,
                        lid,
                        args["end_lid"].as_str(),
                        serde_json::from_value(body["source_range"].clone())
                            .ok()
                            .as_ref(),
                    )
                })
            } else {
                None
            };
            let anchor = match tool {
                "book.search" => body["hits"][0]["start_lid"].as_str(),
                "book.concept" if body["matched_count"] == 1 => {
                    body["candidates"][0]["occurrences"][0].as_str()
                }
                "book.text" => leaf.as_deref().or_else(|| args["lid"].as_str()),
                _ => None,
            };
            plan = book.reading_support(query, anchor, args["within_lid"].as_str(), focus);
        }
        if let Some(plan) = &plan {
            self.attach_reading(body, plan, limit);
        }
        let mut candidates = Vec::new();
        if tool == "book.search" {
            for hit in body["hits"].as_array().into_iter().flatten().take(3) {
                // A short enclosing section supplies the local mechanism and its
                // qualifications. Long sections use the hit's exact read pointer.
                let section = hit["heading_path"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .rev()
                    .filter_map(|h| h["lid"].as_str())
                    .find(|lid| {
                        book.base.lid_nodes.iter().any(|n| {
                            n.lid == *lid && n.span.end - n.span.start <= SHORT_SECTION_UNITS
                        })
                    });
                if let Some(lid) = section {
                    candidates.push(read_call(lid, None));
                } else if let Some(lid) = hit["start_lid"].as_str() {
                    let mut args = read_call(lid, hit["end_lid"].as_str());
                    if let Some(start) = hit["source_range_utf16"]["start"].as_u64() {
                        args["start_utf16"] = json!(start);
                    }
                    candidates.push(args);
                }
            }
        } else if tool == "book.context" {
            for item in body["items"].as_array().into_iter().flatten().take(2) {
                if let Some(lid) = item["lid"].as_str() {
                    candidates.push(read_call(lid, None));
                }
            }
        }
        if let Some(plan) = &plan {
            let read = |p: &ReadingPassage| read_call(&p.lid, p.end_lid.as_deref());
            if matches!(tool, "book.search" | "book.concept") {
                // A specified focus replaces whole-section prefetch. Without one,
                // keep the leading lexical source and leave other hits as locators.
                if focus.is_some() {
                    candidates.clear();
                } else {
                    candidates.truncate(1);
                }
                if candidates
                    .first()
                    .is_none_or(|c| !contains_read(book, c, &plan.passage.lid))
                {
                    candidates.insert(0, read(&plan.passage));
                }
                if self.teaching
                    && !matches!(focus, Some(book_tool_contracts::ReadingFocus::Definition))
                {
                    if let Some(next) = &plan.connection {
                        candidates.push(read(&next.passage));
                    }
                }
            }
        }
        for args in candidates {
            let available = limit.saturating_sub(size(body) + 32);
            if let Some(item) = self.admit(book, args, available, evidence) {
                if !body["context_cache"]["inline"].is_array() {
                    body["context_cache"]["inline"] = json!([]);
                }
                body["context_cache"]["inline"]
                    .as_array_mut()
                    .unwrap()
                    .push(item);
            }
        }
        if tool == "book.search"
            && body["delivery"].is_object()
            && body["context_cache"]["inline"]
                .as_array()
                .is_some_and(|v| !v.is_empty())
        {
            body["delivery"]["evidence"] = json!("locators_with_inline_source");
        }
    }

    /// Restore locations only from successful results for this exact source.
    /// Reload a small recent working set from the book, rather than promoting
    /// historical snippets/receipts into newly observed evidence.
    pub(super) fn start_turn(
        &mut self,
        book: &Book,
        history: &[Message],
        tutor: Option<&Value>,
        locators: &mut TurnLocatorLedger,
        evidence: &mut TurnEvidenceLedger,
    ) -> Option<ContextFragment> {
        let mut calls = HashMap::new();
        let mut recent = Vec::new();
        for message in history {
            if message.role == Role::Assistant {
                for call in &message.tool_calls {
                    calls.insert(call.id.as_str(), call);
                }
            } else if message.role == Role::Tool {
                let Some(call) = message.tool_call_id.as_deref().and_then(|id| calls.get(id))
                else {
                    continue;
                };
                let Some(text) = message.content.as_deref() else {
                    continue;
                };
                let Ok(body) = serde_json::from_str::<Value>(text) else {
                    continue;
                };
                let observed = &body["resident_observation"];
                if observed["source_id"] != book.base.book_id
                    || observed["source_revision"] != book.source_fingerprint()
                    || body.get("error_code").is_some()
                {
                    continue;
                }
                for lid in observed["known_lids"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    locators.observe_lid(lid, LocatorOrigin::VerifiedEvidence, book);
                }
                let ranges: Vec<EvidenceRange> =
                    serde_json::from_value(observed["read_ranges"].clone()).unwrap_or_default();
                locators.observe_verified_evidence(&ranges, book);
                if !ranges.is_empty() && call.name == "book.text" {
                    if let Ok(args) = serde_json::from_str::<Value>(&call.arguments) {
                        for field in ["lid", "end_lid"] {
                            if let Some(lid) = args[field].as_str() {
                                locators.observe_lid(lid, LocatorOrigin::VerifiedEvidence, book);
                            }
                        }
                    }
                }
                for range in ranges {
                    let mut read = read_call(&range.start_lid, Some(&range.end_lid));
                    if let Some(part) = range.ranges.first() {
                        if let Some(span) = book.source_span(&part.lid) {
                            read["start_utf16"] = json!(span.start + part.range.start as usize);
                        }
                    }
                    recent.push(read);
                }
            }
        }
        let mut candidates = Vec::new();
        let selected = evidence.teaching_source_ranges();
        for range in &selected {
            if let Some(spans) = evidence_intervals(book, range) {
                self.admitted.extend(
                    spans
                        .into_iter()
                        .map(|(start, end)| base_schema::Span { start, end }),
                );
            }
        }
        let mut previous_focus = None;
        if let Some(tutor) = tutor {
            for target in [
                &tutor["learner_context"]["current_target"],
                &tutor["current_design"]["design"]["target"],
            ] {
                for source in target["source_bindings"].as_array().into_iter().flatten() {
                    if source["source_id"] == book.base.book_id
                        && source["source_revision"] == book.source_fingerprint()
                    {
                        if let Some(lid) = source["start_lid"].as_str() {
                            if previous_focus.is_none() {
                                previous_focus = target["learning_focus"].as_str();
                            }
                            candidates.push(read_call(lid, source["end_lid"].as_str()));
                        }
                    }
                }
            }
        }
        candidates.extend(recent.into_iter().rev().take(3));
        let mut body = json!({"context_cache":identity(book)});
        body["context_cache"]["inline"] = json!([]);
        if self.teaching {
            let previous = candidates.first();
            let anchor = selected
                .first()
                .map(|r| r.start_lid.as_str())
                .or_else(|| previous.and_then(|c| c["lid"].as_str()));
            let end = selected
                .first()
                .map(|r| r.end_lid.as_str())
                .or_else(|| previous.and_then(|c| c["end_lid"].as_str()));
            let topic = previous_focus
                .filter(|s| !s.is_empty())
                .unwrap_or(&self.question);
            let leaf = anchor.and_then(|lid| book.reading_leaf(topic, lid, end, None));
            if let Some(plan) =
                book.reading_support(&self.question, leaf.as_deref().or(anchor), None, None)
            {
                if let Some(focus) = previous_focus {
                    let label: String = focus.chars().take(100).collect();
                    let bytes = size(&json!(label)) + 28;
                    if bytes < self.remaining {
                        body["context_cache"]["previous_teaching_focus"] = json!(label);
                        self.remaining -= bytes;
                    }
                }
                if let Some(words) = history
                    .iter()
                    .rev()
                    .find(|m| m.role == Role::User)
                    .and_then(|m| m.content.as_deref())
                    .filter(|s| !s.is_empty() && s.chars().count() <= 220)
                {
                    let bytes = size(&json!(words)) + 25;
                    if bytes < self.remaining {
                        body["context_cache"]["previous_user_words"] = json!(words);
                        self.remaining -= bytes;
                    }
                }
                self.attach_reading(&mut body, &plan, PASSAGE_INLINE_BYTES);
                if anchor.is_some_and(|lid| {
                    lid != plan.passage.lid
                        && book
                            .source_span(lid)
                            .is_some_and(|s| s.end - s.start > SHORT_SECTION_UNITS)
                }) {
                    let local = book.local_reading_section(&plan.passage.lid, SHORT_SECTION_UNITS);
                    candidates.retain(|call| !contains_read(book, call, local));
                    candidates.insert(0, read_call(local, None));
                } else if candidates
                    .first()
                    .is_none_or(|c| !contains_read(book, c, &plan.passage.lid))
                {
                    candidates.insert(
                        0,
                        read_call(&plan.passage.lid, plan.passage.end_lid.as_deref()),
                    );
                }
            }
        }
        // Leave at least half the turn allowance for the current search/topic.
        for args in candidates.into_iter().take(3) {
            let available = PASSAGE_INLINE_BYTES.saturating_sub(size(&body));
            if let Some(item) = self.admit(book, args, available, evidence) {
                body["context_cache"]["inline"]
                    .as_array_mut()
                    .unwrap()
                    .push(item);
            }
        }
        observe_inline(evidence, &body, book);
        observe_locations(locators, &body, book);
        locators.observe_verified_evidence(&evidence.teaching_source_ranges(), book);
        if body["context_cache"]["inline"]
            .as_array()
            .unwrap()
            .is_empty()
            && body["context_cache"]["reading"].is_null()
        {
            return None;
        }
        Some(ContextFragment::new("reader.source_cache", FragmentScope::TurnFrozen, Role::User,
            format!("Exact canonical text prefetched from the current teaching target or recent read. For a student's follow-up resolve everyday words against the last delivered example and previous_teaching_focus before seeking another meaning elsewhere. A new explicit topic takes precedence. Only result.text is source evidence; metadata is a location. source_ref_id can be cited directly as [[source:<source_ref_id>]]. Unread remainder stays in the book; use next_call when needed.\n{body}"), FragmentSensitivity::Private))
    }
}

/// Keep the original result in the business journal. The small observation
/// records only locations/text ranges actually delivered, not the raw result's
/// potentially much larger source body. It is a reload pointer, never evidence.
pub(super) fn history_result(
    raw: &str,
    tool: &str,
    model_body: &Value,
    receipt: &HistoricalToolReceipt,
    book: &Book,
) -> Option<String> {
    if !same_source(model_body, book) {
        return None;
    }
    let original: Value = serde_json::from_str(raw).ok()?;
    let original = original.as_object()?;
    let mut locators = TurnLocatorLedger::default();
    locators.observe_tool_result(tool, &model_body.to_string(), book);
    locators.observe_verified_evidence(&receipt.accepted_evidence, book);
    let mut observation = identity(book);
    observation["known_lids"] = json!(locators.entries.keys().collect::<Vec<_>>());
    observation["read_ranges"] = json!(receipt.accepted_evidence);
    let prefix = raw.trim_end().strip_suffix('}')?;
    Some(format!(
        "{prefix}{}\"resident_observation\":{observation}}}",
        if original.is_empty() { "" } else { "," }
    ))
}

pub(super) fn observe_inline(ledger: &mut TurnEvidenceLedger, body: &Value, book: &Book) {
    if !same_source(body, book) {
        return;
    }
    for item in body["context_cache"]["inline"]
        .as_array()
        .into_iter()
        .flatten()
    {
        observe_tool_evidence(
            ledger,
            "book.text",
            &item["arguments"].to_string(),
            &item["result"].to_string(),
            book,
        );
    }
}

pub(super) fn observe_locations(ledger: &mut TurnLocatorLedger, body: &Value, book: &Book) {
    if !same_source(body, book) {
        return;
    }
    let reading = &body["context_cache"]["reading"];
    for stop in [
        &reading["passage"],
        &reading["connection"]["passage"],
        &reading["connection"]["example"],
    ] {
        for field in ["lid", "end_lid"] {
            if let Some(lid) = stop[field].as_str() {
                ledger.observe_lid(lid, LocatorOrigin::ContextResult, book);
            }
        }
    }
    for item in body["context_cache"]["inline"]
        .as_array()
        .into_iter()
        .flatten()
    {
        for field in ["lid", "end_lid"] {
            if let Some(lid) = item["arguments"][field].as_str() {
                ledger.observe_lid(lid, LocatorOrigin::VerifiedEvidence, book);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base_schema::{LidNode, NodeKind, ReadOnlyBase, Span};

    fn book(paragraphs: &[&str]) -> Book {
        let mut nodes = Vec::new();
        let mut at = 0;
        for (index, text) in paragraphs.iter().enumerate() {
            let end = at + text.encode_utf16().count();
            nodes.push(LidNode {
                lid: format!("1.{}", index + 1),
                path: vec![1, index as u32 + 1],
                kind: NodeKind::Paragraph,
                span: Span { start: at, end },
                children: vec![],
            });
            at = end;
        }
        nodes.insert(
            0,
            LidNode {
                lid: "1".into(),
                path: vec![1],
                kind: NodeKind::Chapter,
                span: Span { start: 0, end: at },
                children: (1..=paragraphs.len()).map(|i| format!("1.{i}")).collect(),
            },
        );
        Book::new(
            ReadOnlyBase {
                book_id: "cache-fixture".into(),
                lid_nodes: nodes,
                graph_nodes: vec![],
                graph_edges: vec![],
            },
            &paragraphs.concat(),
        )
    }

    fn search(book: &Book, query: &str) -> Value {
        serde_json::to_value(book.search_ranked(query, None, 8).unwrap()).unwrap()
    }

    fn indexed_book() -> Book {
        let unrelated = "计算实例外部的会话日志。".repeat(260);
        let paragraphs = [
            "定义直接注入\n",
            "1. 先给摘要\n",
            "2. 附上相邻对象\n",
            "3. 长文留入口\n",
            "每个对象携带下一跳的编号\n",
            "从消息找回负责处理的人\n",
            &unrelated,
        ];
        let mut base = book(&paragraphs).base;
        base.graph_nodes = serde_json::from_value(json!([
            {"id":"a","type":"concept","name":"Inline","occurrences":["1.1","1.3","1.5"],"source_lid":null},
            {"id":"b","type":"concept","name":"按需召回","occurrences":["1.5","1.6"],"source_lid":null}
        ])).unwrap();
        base.graph_edges = serde_json::from_value(json!([
            {"source":"b","target":"a","type":"builds_on","direction":"directed","scope":"local","weight":1.0}
        ])).unwrap();
        let roles = [
            "definition",
            "procedure_step",
            "procedure_step",
            "procedure_step",
            "explanation",
            "example",
        ];
        let items = roles.iter().enumerate().map(|(i,role)| {
            let links = if i==5 {json!([{"type":"exemplifies","target_lid":"1.5","direction":"backward","confidence":1.0,"evidence_lids":["1.5","1.6"]}])} else {json!([])};
            serde_json::from_value(json!({"lid":format!("1.{}",i+1),"mode":"informative","local_function":role,"relations":links})).unwrap()
        }).collect();
        Book::new(base, &paragraphs.concat()).with_discourse_items(items)
    }

    #[test]
    fn focus_returns_the_whole_local_procedure_and_one_reverse_connection() {
        let b = indexed_book();
        let args = json!({"query":"Inline","focus":"procedure"});
        let input = serde_json::from_value(args.clone()).unwrap();
        let mut body = json!(b.search_for_reading(&input).unwrap());
        let note = &body["reading"];
        assert_eq!(note["passage"]["lid"], "1.2");
        assert_eq!(note["passage"]["end_lid"], "1.4");
        assert_eq!(note["connection"]["from"], "按需召回");
        assert_eq!(note["connection"]["to"], "Inline");
        assert_eq!(note["connection"]["example"]["lid"], "1.6");
        let mut cache = InlineContextCache::for_turn("怎么做", Some(&json!({"status":"practice"})));
        let mut ev = TurnEvidenceLedger::default();
        cache.enrich(
            "book.search",
            &args.to_string(),
            &mut body,
            &b,
            16 * 1024,
            &mut ev,
        );
        let inline = &body["context_cache"]["inline"];
        assert_eq!(
            inline[0]["result"]["text"],
            b.text("1.2", Some("1.4")).unwrap()
        );
        assert!(inline[0]["source_ref_id"].is_string());
        let mut locators = TurnLocatorLedger::default();
        observe_locations(&mut locators, &body, &b);
        assert!(authorize_book_text(r#"{"lid":"1.6"}"#, &locators).is_ok());
        observe_inline(&mut ev, &body, &b);
        assert!(!ev
            .teaching_source_ranges()
            .iter()
            .any(|r| r.end_lid == "1.6"));
        assert!(b
            .reading_support("Inline", Some("1.1"), Some("1.1"), None)
            .unwrap()
            .connection
            .is_none());
        assert!(b.reading_support("inlined", None, None, None).is_none());
    }

    #[test]
    fn tutor_continuation_recalls_a_connection_without_a_named_term_or_a_tool() {
        let b = indexed_book();
        let target = json!({"status":"practice","learner_context":{"current_target":{"source_bindings":[
            {"source_id":b.base.book_id,"source_revision":b.source_fingerprint(),"start_lid":"1.1"}]}}});
        let mut cache = InlineContextCache::for_turn("继续", Some(&target));
        let mut ev = TurnEvidenceLedger::default();
        let mut locators = TurnLocatorLedger::default();
        let fragment = cache
            .start_turn(&b, &[], Some(&target), &mut locators, &mut ev)
            .unwrap();
        assert!(fragment.content.contains("builds_on"));
        assert!(authorize_book_text(r#"{"lid":"1.5"}"#, &locators).is_ok());
        assert!(ev
            .teaching_source_ranges()
            .iter()
            .all(|r| r.end_lid == "1.1"));
        let raw = book(&["只有原文"]);
        assert!(raw
            .reading_support("继续", Some("1.1"), None, None)
            .is_none());
    }

    #[test]
    fn followup_keeps_the_delivered_focus_inside_a_broad_chapter() {
        let b = indexed_book();
        let target = json!({"status":"practice","learner_context":{"current_target":{
            "learning_focus":"定义直接注入","source_bindings":[{"source_id":b.base.book_id,
            "source_revision":b.source_fingerprint(),"start_lid":"1","end_lid":"1"}]}}});
        let mut cache = InlineContextCache::for_turn("为什么日志放在外面", Some(&target));
        let fragment = cache
            .start_turn(
                &b,
                &[Message::user("我是从旧消息找回处理人的")],
                Some(&target),
                &mut TurnLocatorLedger::default(),
                &mut TurnEvidenceLedger::default(),
            )
            .unwrap();
        assert!(
            fragment.content.contains("定义直接注入")
                && fragment.content.contains("我是从旧消息找回处理人的")
        );
        assert!(!fragment.content.contains("计算实例外部"));
        let page = b.source_span("1.5").unwrap();
        assert_eq!(
            b.reading_leaf("下一跳的编号", "1", None, Some(page))
                .as_deref(),
            Some("1.5")
        );
    }

    #[test]
    fn context_cache_search_inlines_short_section_once_with_ready_citation() {
        let b = book(&[
            "# 上下文组织\nInline 预取当前需要的信息。",
            "Spill 把长内容留在外面并保留引用。",
        ]);
        let mut body = search(&b, "Inline Spill");
        let mut evidence = TurnEvidenceLedger::default();
        let mut cache = InlineContextCache::default();
        cache.enrich("book.search", "{}", &mut body, &b, 16 * 1024, &mut evidence);
        let inline = body["context_cache"]["inline"].as_array().unwrap();
        assert_eq!(
            inline.len(),
            1,
            "hits in the same short section must share one read"
        );
        assert_eq!(inline[0]["result"]["text"], b.text("1", None).unwrap());
        assert_eq!(evidence.bindings().len(), 1);
        assert!(
            evidence.teaching_source_ranges().is_empty(),
            "evidence is admitted by the final model projection"
        );
        observe_tool_evidence(&mut evidence, "book.search", "{}", &body.to_string(), &b);
        assert!(!evidence.teaching_source_ranges().is_empty());
        assert_eq!(
            inline[0]["source_ref_id"],
            evidence.bindings()[0].source_ref_id
        );
        let mut locators = TurnLocatorLedger::default();
        locators.observe_tool_result("book.search", &body.to_string(), &b);
        assert!(authorize_book_text(r#"{"lid":"1"}"#, &locators).is_ok());
        let remaining = cache.remaining;
        let mut again = search(&b, "Inline");
        cache.enrich(
            "book.search",
            "{}",
            &mut again,
            &b,
            16 * 1024,
            &mut evidence,
        );
        assert!(again["context_cache"]["inline"].is_null());
        assert_eq!(cache.remaining, remaining);
    }

    #[test]
    fn context_cache_long_passage_spills_with_exact_continuation_and_bounded_evidence() {
        let long = format!("缓存工作集。{}", "长日志😀不能全部内联。".repeat(1000));
        let b = book(&[&long]);
        let mut cache = InlineContextCache::default();
        let mut evidence = TurnEvidenceLedger::default();
        let item = cache
            .admit(&b, read_call("1.1", None), 2200, &mut evidence)
            .unwrap();
        assert!(size(&item) <= 2200);
        let span: Span = serde_json::from_value(item["result"]["source_range"].clone()).unwrap();
        assert_eq!(
            item["result"]["text"],
            b.text_in_source_range(&span).unwrap()
        );
        assert_eq!(item["next_call"]["arguments"]["start_utf16"], span.end);
        assert!(span.end < long.encode_utf16().count());
        assert!(evidence.bindings()[0]
            .evidence_range
            .as_ref()
            .unwrap()
            .ranges
            .iter()
            .all(|r| r.range.end as usize <= span.end));
        let input = serde_json::from_value(item["next_call"]["arguments"].clone()).unwrap();
        assert!(b.read_text(&input).unwrap()["text"]
            .as_str()
            .unwrap()
            .starts_with(
                &long
                    .chars()
                    .skip(item["result"]["text"].as_str().unwrap().chars().count())
                    .take(10)
                    .collect::<String>()
            ));
        let before = cache.remaining;
        assert!(cache
            .admit(&b, read_call("1.1", None), 200, &mut evidence)
            .is_none());
        assert_eq!(cache.remaining, before);
        let args = item["arguments"].to_string();
        let raw = b
            .read_text(&serde_json::from_value(item["arguments"].clone()).unwrap())
            .unwrap()
            .to_string();
        let mut delivered = item["result"].clone();
        delivered["context_cache"] = identity(&b);
        let receipt = tool_receipt("book.text", &args, &delivered.to_string(), &raw, &b);
        let saved = history_result(&raw, "book.text", &delivered, &receipt, &b).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&saved).unwrap()["text"],
            serde_json::from_str::<Value>(&raw).unwrap()["text"]
        );
        let historical = historical_tool_receipt("book.text", &args, &saved, &b);
        assert_eq!(
            serde_json::to_value(historical.accepted_evidence).unwrap(),
            serde_json::to_value(receipt.accepted_evidence).unwrap(),
            "the full journal body must not expand the range actually shown to the model"
        );
    }

    #[test]
    fn context_cache_read_citations_cover_only_delivered_text_and_fit_contract() {
        let b = book(&[
            &"首段原文和未读后续。".repeat(1000),
            "第二段不在本次预算内。",
        ]);
        let args = json!({"ranges":[{"lid":"1.1"},{"lid":"1.2"}]});
        let raw = crate::book_text_batch::read(&args.to_string(), &b);
        let cache = InlineContextCache::default();
        let policy = crate::tool_registry::ToolOutputPolicy {
            result_policy: crate::tool_registry::ToolResultPolicy::EvidenceProjection,
            max_model_body_bytes: 4000,
        };
        let mut projected = crate::tool_result::project_tool_result(
            "book.text_batch",
            &args.to_string(),
            &raw,
            cache.projection_policy("book.text_batch", policy, 4000, &b),
            4000,
            &b,
        );
        let mut evidence = TurnEvidenceLedger::default();
        InlineContextCache::default().enrich(
            "book.text_batch",
            "{}",
            &mut projected.model_body,
            &b,
            4000,
            &mut evidence,
        );
        cache.cite_read(
            "book.text_batch",
            &projected.evidence_arguments,
            &mut projected.model_body,
            &b,
            4000,
            &mut evidence,
        );
        assert!(size(&projected.model_body) <= 4000);
        let citations = projected.model_body["context_cache"]["citations"]
            .as_array()
            .unwrap();
        assert!(!citations.is_empty());
        assert_eq!(citations.len(), evidence.bindings().len());
        for citation in citations {
            let binding = evidence
                .bindings()
                .into_iter()
                .find(|s| s.source_ref_id == citation["source_ref_id"].as_str().unwrap())
                .unwrap();
            let resolved = b
                .resolve_source(
                    binding.evidence_range.as_ref().unwrap(),
                    SOURCE_PRESENTATION_LOCALE,
                    None,
                )
                .unwrap();
            let span = serde_json::from_value(citation["source_range"].clone()).unwrap();
            assert_eq!(
                resolved.highlighted_quote,
                b.text_in_source_range(&span).unwrap()
            );
        }
        let schema = crate::tool_result::book_envelope_schema("book.text_batch").unwrap();
        let body_schema = &schema["oneOf"][1]["properties"]["model_body"];
        crate::tool_registry::validate_schema(body_schema, &projected.model_body, "$").unwrap();

        let args = json!({"lid":"1.2"});
        let mut complete = json!({"lid":"1.2","text":b.text("1.2",None).unwrap()});
        assert!(complete.get("source_range").is_none());
        InlineContextCache::default().enrich(
            "book.text",
            "{}",
            &mut complete,
            &b,
            4000,
            &mut evidence,
        );
        cache.cite_read(
            "book.text",
            &args.to_string(),
            &mut complete,
            &b,
            4000,
            &mut evidence,
        );
        let full = &complete["context_cache"]["citations"][0];
        assert!(
            full["source_ref_id"].is_string(),
            "complete short reads also need ready citations"
        );
        let full_span = serde_json::from_value(full["source_range"].clone()).unwrap();
        assert_eq!(
            b.text_in_source_range(&full_span).unwrap(),
            complete["text"]
        );
        let schema = crate::tool_result::book_envelope_schema("book.text").unwrap();
        crate::tool_registry::validate_schema(
            &schema["oneOf"][0]["properties"]["model_body"],
            &complete,
            "$",
        )
        .unwrap();
    }

    #[test]
    fn context_cache_reuses_same_revision_locations_and_reloads_real_text() {
        let b = book(&["# 当前主题\n近期证据支持这个解释。", "另一个段落。"]);
        let mut body = search(&b, "近期证据");
        InlineContextCache::default().enrich(
            "book.search",
            "{}",
            &mut body,
            &b,
            16 * 1024,
            &mut TurnEvidenceLedger::default(),
        );
        let receipt = tool_receipt(
            "book.search",
            "{}",
            &body.to_string(),
            &body.to_string(),
            &b,
        );
        let saved = history_result(&body.to_string(), "book.search", &body, &receipt, &b).unwrap();
        let history = vec![
            Message {
                role: Role::Assistant,
                content: None,
                provider_continuation: None,
                tool_call_id: None,
                tool_calls: vec![crate::ToolCall {
                    id: "search".into(),
                    name: "book.search".into(),
                    arguments: r#"{"query":"近期证据"}"#.into(),
                }],
            },
            Message {
                role: Role::Tool,
                content: Some(saved),
                provider_continuation: None,
                tool_calls: vec![],
                tool_call_id: Some("search".into()),
            },
        ];
        let mut locators = TurnLocatorLedger::default();
        let mut evidence = TurnEvidenceLedger::default();
        let fragment = InlineContextCache::default()
            .start_turn(&b, &history, None, &mut locators, &mut evidence)
            .unwrap();
        assert!(fragment.content.contains("近期证据支持这个解释。"));
        assert!(locators.may_read_lid("1"));
        assert!(!evidence.teaching_source_ranges().is_empty());
        let changed = book(&["新版本不再包含旧结论。", "另一个段落。"]);
        let mut locators = TurnLocatorLedger::default();
        let mut evidence = TurnEvidenceLedger::default();
        assert!(InlineContextCache::default()
            .start_turn(&changed, &history, None, &mut locators, &mut evidence)
            .is_none());
        assert!(locators.entries.is_empty() && evidence.evidence.is_empty());
    }

    #[test]
    fn context_cache_zero_budget_and_search_preview_never_supply_evidence() {
        let b = book(&["只有实际原文能支持结论。"]);
        let mut body = search(&b, "原文");
        let before = body.clone();
        let mut cache = InlineContextCache::default();
        let mut evidence = TurnEvidenceLedger::default();
        cache.enrich(
            "book.search",
            "{}",
            &mut body,
            &b,
            size(&before),
            &mut evidence,
        );
        assert_eq!(body, before);
        observe_tool_evidence(&mut evidence, "book.search", "{}", &body.to_string(), &b);
        assert!(evidence.evidence.is_empty() && evidence.bindings().is_empty());
    }

    #[test]
    #[ignore = "offline replay requires UB_CONTEXT_CACHE_BOOK and UB_CONTEXT_CACHE_TURNS"]
    fn context_cache_saved_five_turn_paths() {
        struct Offline;
        impl ModelAdapter for Offline {
            fn complete(
                &self,
                _: crate::CompletionRequest,
            ) -> Result<crate::ParsedResponse, crate::AdapterError> {
                panic!("offline replay must not call a model")
            }
            fn chat(
                &self,
                _: &AgentRequestPlan,
            ) -> Result<crate::AssistantTurn, crate::AdapterError> {
                panic!("offline replay must not call a model")
            }
        }
        let b = Book::load(&std::env::var("UB_CONTEXT_CACHE_BOOK").unwrap()).unwrap();
        let turns: Value = serde_json::from_slice(
            &std::fs::read(std::env::var("UB_CONTEXT_CACHE_TURNS").unwrap()).unwrap(),
        )
        .unwrap();
        let mut history = Vec::new();
        let mut report = Vec::new();
        for turn in turns.as_array().unwrap() {
            let mut cache = InlineContextCache::default();
            let mut evidence = TurnEvidenceLedger::default();
            let mut locators = TurnLocatorLedger::from_turn(&b, "", &[], "1");
            let prefetched = cache
                .start_turn(&b, &history, None, &mut locators, &mut evidence)
                .is_some();
            let mut rejected = 0;
            let mut inline_bytes = 0;
            for (index, call) in turn["tools"].as_array().unwrap().iter().enumerate() {
                let name = call["tool"].as_str().unwrap();
                let args_value = call["args"]
                    .as_str()
                    .map(|s| serde_json::from_str::<Value>(s).unwrap())
                    .unwrap_or_else(|| call["args"].clone());
                let args = args_value.to_string();
                if !matches!(
                    name,
                    "book.search" | "book.text" | "book.text_batch" | "book.structure"
                ) {
                    continue;
                }
                let raw = if name == "book.text_batch" {
                    let input = crate::book_text_batch::parse(&args).unwrap();
                    let mut items = Vec::new();
                    for range in input.ranges {
                        let args = serde_json::to_value(&range).unwrap();
                        if authorize_book_text(&args.to_string(), &locators).is_err() {
                            rejected += 1;
                            continue;
                        }
                        items.push(json!({"arguments":args,"result":b.read_text(&range).unwrap()}));
                    }
                    json!({"items":items}).to_string()
                } else {
                    if name == "book.text" && authorize_book_text(&args, &locators).is_err() {
                        rejected += 1;
                        continue;
                    }
                    let tool = match name {
                        "book.search" => BookToolId::Search,
                        "book.text" => BookToolId::Text,
                        _ => BookToolId::Structure,
                    };
                    dispatch_resident_book_tool(tool, args_value, &b, &Offline).0
                };
                let registry = resident_tool_registry();
                let policy = registry.registration(name).unwrap().output_policy;
                let mut projected = crate::tool_result::project_tool_result(
                    name,
                    &args,
                    &raw,
                    cache.projection_policy(name, policy, 48 * 1024, &b),
                    48 * 1024,
                    &b,
                );
                cache.enrich(
                    name,
                    "{}",
                    &mut projected.model_body,
                    &b,
                    policy.max_model_body_bytes,
                    &mut evidence,
                );
                assert!(size(&projected.model_body) <= policy.max_model_body_bytes);
                inline_bytes += projected.model_body["context_cache"]["inline"]
                    .as_array()
                    .map_or(0, |v| v.iter().map(size).sum());
                let body = projected.model_body.to_string();
                observe_tool_evidence(
                    &mut evidence,
                    name,
                    &projected.evidence_arguments,
                    &body,
                    &b,
                );
                locators.observe_tool_result(name, &body, &b);
                locators.observe_verified_evidence(&evidence.teaching_source_ranges(), &b);
                let id = format!("{}-{index}", turn["n"]);
                history.push(Message {
                    role: Role::Assistant,
                    content: None,
                    provider_continuation: None,
                    tool_call_id: None,
                    tool_calls: vec![crate::ToolCall {
                        id: id.clone(),
                        name: name.into(),
                        arguments: args,
                    }],
                });
                let receipt = tool_receipt(name, &projected.evidence_arguments, &body, &raw, &b);
                let saved =
                    history_result(&raw, name, &projected.model_body, &receipt, &b).unwrap_or(raw);
                history.push(Message {
                    role: Role::Tool,
                    content: Some(saved),
                    provider_continuation: None,
                    tool_calls: vec![],
                    tool_call_id: Some(id),
                });
            }
            assert!(inline_bytes <= TURN_INLINE_BYTES);
            report.push(json!({"turn":turn["n"],"prefetched_followup":prefetched,
                "source_inline_bytes":inline_bytes,"rejected_read_ranges":rejected,
                "verified_ranges":evidence.teaching_source_ranges().len()}));
        }
        assert_eq!(report.len(), 5);
        // The concrete cache explanation is fully in the first search packet.
        let mut cache = InlineContextCache::default();
        let mut evidence = TurnEvidenceLedger::default();
        let search_call = turns[4]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["tool"] == "book.search")
            .unwrap();
        let args = search_call["args"]
            .as_str()
            .map(|s| serde_json::from_str::<Value>(s).unwrap())
            .unwrap_or_else(|| search_call["args"].clone());
        let raw = dispatch_resident_book_tool(BookToolId::Search, args.clone(), &b, &Offline).0;
        let policy = resident_tool_registry()
            .registration("book.search")
            .unwrap()
            .output_policy;
        let mut body = crate::tool_result::project_tool_result(
            "book.search",
            &args.to_string(),
            &raw,
            cache.projection_policy("book.search", policy, 48 * 1024, &b),
            48 * 1024,
            &b,
        )
        .model_body;
        cache.enrich("book.search", "{}", &mut body, &b, 16 * 1024, &mut evidence);
        let inline = body["context_cache"]["inline"].as_array().unwrap();
        let exact = b.text("1.36.6.3", Some("1.36.6.10")).unwrap();
        assert!(
            inline.iter().any(|i| i["result"]["text"]
                .as_str()
                .is_some_and(|text| text.contains(&exact))),
            "the real 859-character answer passage must arrive with the first cache search"
        );
        let report = json!({"turns":report,"first_cache_search_contains_original_answer_passage":true,
            "measurement":"fixed historical tool path; does not predict model choices or end-to-end savings"});
        if let Ok(path) = std::env::var("UB_CONTEXT_CACHE_PATH_MEASUREMENT") {
            std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        }
        println!("{report}");
    }
}
