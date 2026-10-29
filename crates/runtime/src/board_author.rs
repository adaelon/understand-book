//! Native board candidates share the ordinary answer compiler and final-turn boundary.
use memory::boards::{
    BoardContentRef, BoardExplanationMapping, BoardGraphic, BoardLink, BoardPosition, BoardRoute,
    BoardSize,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Shared by the board tool contract and the frozen context for direct replies.
pub const SOURCE_CITATION_POLICY: &str = "Board source citations: Substantive claims about the book, including its rules, conditions and specific values, require clickable original-source citations in direct text answers, explanation blocks and each committed part. Read the relevant book/library evidence in this turn and call source.present with the supporting observed passage. Then place [[source:<source_ref_id>]] after the supported sentence or closely related group of claims, using the exact returned ID; calling source.present alone does not attach a citation. Put these markers in blocks[].markdown before prepare/revise, so the saved explanation blocks and committed parts retain clickable sources. A derivation must identify its cited source premises and distinguish the inference from the author's statement. Label learner assumptions and illustrative values as such; they are not book evidence. Historical source_ref_ids and retained source_bindings are not this turn's evidence: reacquire support for new book claims, and cite only the references actually supporting the current explanation. If a required claim has no supporting observed passage, state the gap or narrow the claim; never invent a citation. Greetings, interface operations and layout-only changes need no forced citation. A concise recap may reuse clear nearby citations without repeating them.";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplanationBlock {
    pub block_id: String,
    pub markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Drawing {
    pub item_id: String,
    pub title: String,
    pub position: BoardPosition,
    pub size: BoardSize,
    pub graphic: BoardGraphic,
    #[serde(default)]
    pub based_on: Vec<BoardContentRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceImport {
    pub source: Value,
    #[serde(default)]
    pub published_book_ref: Option<base_schema::PublishedBookRef>,
    pub item_id: String,
    pub position: BoardPosition,
    pub size: BoardSize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutUpdate {
    pub item_id: String,
    pub position: BoardPosition,
    pub size: BoardSize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadCollection {
    Committed,
    Layout,
    Items,
    Links,
    Mappings,
    Routes,
    UnderstandingItems,
    UnderstandingRefs,
    FormalConditionRefs,
    CandidateItems,
    CandidateBlocks,
    CandidateUpdates,
    CandidateLinks,
    CandidateMappings,
    CandidateRoutes,
}

impl ReadCollection {
    pub fn path(&self) -> &'static str {
        match self {
            Self::Committed => "/committed",
            Self::Layout => "/layout",
            Self::Items => "/board/items",
            Self::Links => "/board/links",
            Self::Mappings => "/board/mappings",
            Self::Routes => "/board/routes",
            Self::UnderstandingItems => "/understanding_items",
            Self::UnderstandingRefs => "/understanding_refs",
            Self::FormalConditionRefs => "/formal_condition_refs",
            Self::CandidateItems => "/candidate/items",
            Self::CandidateBlocks => "/candidate/explanation_blocks",
            Self::CandidateUpdates => "/candidate/update_items",
            Self::CandidateLinks => "/candidate/links",
            Self::CandidateMappings => "/candidate/mappings",
            Self::CandidateRoutes => "/candidate/routes",
        }
    }
}

/// The server applies this to its authoritative read under the normal user scope.
/// Pages stay on the same board/candidate revision, and records are never clipped.
pub fn read_page(
    body: Value,
    collection: Option<ReadCollection>,
    offset: usize,
    length: Option<usize>,
    board_revision: Option<u32>,
    candidate_revision: Option<u32>,
) -> Result<Value, read_tools::ToolError> {
    let revision = body["board"]["revision"].as_u64().unwrap_or(0);
    if board_revision.is_some_and(|expected| u64::from(expected) != revision) {
        return Err(error("BOARD_REVISION_CONFLICT", "The board changed between read pages. Read the current board and restart its pages before preparing an update."));
    }
    if candidate_revision
        .is_some_and(|expected| body["candidate_revision"].as_u64() != Some(u64::from(expected)))
    {
        return Err(error("BOARD_CANDIDATE_CHANGED", "The native candidate changed between read pages. Read its current revision before continuing."));
    }
    let Some(collection) = collection else {
        if offset != 0 || length.is_some() {
            return Err(error(
                "BOARD_READ_COLLECTION_REQUIRED",
                "Choose a read collection before providing offset or length.",
            ));
        }
        return Ok(body);
    };
    let length = length.unwrap_or(20);
    if !(1..=50).contains(&length) {
        return Err(error(
            "BOARD_READ_LENGTH_INVALID",
            "Read length must be between 1 and 50 complete records.",
        ));
    }
    let records = if matches!(&collection, ReadCollection::Layout) {
        body["board"]["items"].as_array().into_iter().flatten().map(|item| {
            json!({"item_id":item["item_id"],"title":item["title"],"kind":item["target"]["kind"],
                "position":item["position"],"size":item["size"],"reply_to_item_id":item["reply_to_item_id"],
                "block_id":item["block_id"],"explanation_block_ids":item["explanation_blocks"].as_array().into_iter().flatten()
                    .map(|block|block["block_id"].clone()).collect::<Vec<_>>()})
        }).collect::<Vec<_>>()
    } else {
        body.pointer(collection.path())
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    if offset > records.len() {
        return Err(error(
            "BOARD_READ_OFFSET_INVALID",
            "Read offset is beyond this collection; restart from its current revision.",
        ));
    }
    let end = offset.saturating_add(length).min(records.len());
    let collection = serde_json::to_value(collection).expect("board read collection serializes");
    let mut next = json!({"operation":"read","collection":collection,"offset":end,"length":length,"board_revision":revision});
    if collection
        .as_str()
        .is_some_and(|name| name.starts_with("candidate_"))
        && body["candidate_revision"].as_u64().unwrap_or(0) > 0
    {
        next["candidate_revision"] = body["candidate_revision"].clone();
    }
    Ok(
        json!({"status":"board_page","board_id":body["board"]["board_id"],"board_revision":revision,
        "reserved_answer_item_id":body["reserved_answer_item_id"],"proposed_content_revision":body["proposed_content_revision"],
        "answer_position":body["answer_position"],"answer_size":body["answer_size"],
        "candidate_revision":body["candidate_revision"],"collection":collection,"offset":offset,"length":length,
        "total":records.len(),"records":records[offset..end],"complete":end==records.len(),
        "next_calls":if end<records.len() {vec![json!({"name":"board.author","arguments":next})]} else {vec![]}}),
    )
}

/// Preserve board record boundaries and provide calls the public read accepts.
/// The generic JSON reducer cannot safely shorten IDs, geometry or evidence.
pub(crate) fn project_read(
    arguments: &str,
    raw: &Value,
    limit: usize,
) -> crate::tool_result::ToolResultDraft {
    use crate::tool_result::{ToolContinuation, ToolResultDraft, ToolResultStatus};
    let bytes = |value: &Value| serde_json::to_vec(value).map_or(usize::MAX, |value| value.len());
    let next_call = |body: &Value| {
        body["next_calls"].as_array().and_then(|calls| calls.first()).map(|call| ToolContinuation::NextCall {
        tool:"board.author".into(),arguments:call["arguments"].clone(),
        reason:"Read complete omitted board records at the same revision; do not infer their geometry or contents.".into(),
    })
    };
    if bytes(raw) <= limit {
        return ToolResultDraft {
            status: ToolResultStatus::Ok,
            model_body: raw.clone(),
            truncated: false,
            continuation: next_call(raw),
            evidence_arguments: arguments.into(),
        };
    }
    let mut body = raw.clone();
    if raw["status"] == "board_page" {
        let mut next: Value = serde_json::from_str(arguments).expect("validated board read");
        let offset = raw["offset"].as_u64().unwrap_or(0);
        let mut kept = raw["records"].as_array().map_or(0, Vec::len);
        while bytes(&body) > limit && kept > 0 {
            kept -= 1;
            body["records"].as_array_mut().unwrap().pop();
            next["offset"] = json!(offset + kept as u64);
            next["board_revision"] = raw["board_revision"].clone();
            if raw["collection"]
                .as_str()
                .is_some_and(|name| name.starts_with("candidate_"))
                && raw["candidate_revision"].as_u64().unwrap_or(0) > 0
            {
                next["candidate_revision"] = raw["candidate_revision"].clone();
            }
            body["complete"] = json!(false);
            body["next_calls"] = json!([{"name":"board.author","arguments":next}]);
        }
        if kept == 0 || bytes(&body) > limit {
            body = json!({"status":"board_read_budget","board_id":raw["board_id"],"board_revision":raw["board_revision"],
                "reserved_answer_item_id":raw["reserved_answer_item_id"],"proposed_content_revision":raw["proposed_content_revision"],
                "answer_position":raw["answer_position"],"answer_size":raw["answer_size"],
                "collection":raw["collection"],"offset":offset,"records":[],
                "message":"No complete record fits the available result budget. No record was delivered. Read collection=layout for exact item identities and positions; the full content record was not observed."});
            if bytes(&body) > limit {
                body = Value::Null;
            }
            return ToolResultDraft {
                status: ToolResultStatus::Partial,
                model_body: body,
                truncated: true,
                continuation: None,
                evidence_arguments: arguments.into(),
            };
        }
    } else {
        let revision = raw["board"]["revision"].clone();
        let call = |collection: &str, offset: usize| {
            let mut args = json!({"operation":"read","collection":collection,"offset":offset,"length":20,"board_revision":revision});
            if collection.starts_with("candidate_")
                && raw["candidate_revision"].as_u64().unwrap_or(0) > 0
            {
                args["candidate_revision"] = raw["candidate_revision"].clone();
            }
            json!({"name":"board.author","arguments":args})
        };
        // Layout is independently readable even if a retained answer is larger
        // than a page, so new drawing placement never depends on clipped text.
        let mut calls = vec![call("layout", 0)];
        body["delivery"] = json!({"status":"partial","notice":"Only complete records are shown. Follow next_calls to recover omitted records. layout contains geometry without long answer text."});
        if let Some(board) = body["board"].as_object_mut() {
            board.remove("deliveries");
        }
        if raw["candidate"].is_object() {
            body["candidate"] = json!({"candidate_id":raw["candidate"]["candidate_id"],
                "candidate_revision":raw["candidate_revision"],"base_board_revision":raw["candidate"]["base_board_revision"],
                "previewed":raw["candidate"]["previewed"],"staged":raw["candidate"]["staged"],"content_omitted":true});
            for (collection, path) in [
                ("candidate_items", "/candidate/items"),
                ("candidate_blocks", "/candidate/explanation_blocks"),
                ("candidate_updates", "/candidate/update_items"),
                ("candidate_links", "/candidate/links"),
                ("candidate_mappings", "/candidate/mappings"),
                ("candidate_routes", "/candidate/routes"),
            ] {
                if raw
                    .pointer(path)
                    .and_then(Value::as_array)
                    .is_some_and(|items| !items.is_empty())
                {
                    calls.push(call(collection, 0));
                }
            }
        }
        body["next_calls"] = json!(calls);
        for (collection, path) in [
            ("items", "/board/items"),
            ("links", "/board/links"),
            ("mappings", "/board/mappings"),
            ("routes", "/board/routes"),
            ("understanding_items", "/understanding_items"),
            ("understanding_refs", "/understanding_refs"),
            ("formal_condition_refs", "/formal_condition_refs"),
            ("committed", "/committed"),
        ] {
            while bytes(&body) > limit {
                let Some(items) = body.pointer_mut(path).and_then(Value::as_array_mut) else {
                    break;
                };
                if items.is_empty() {
                    break;
                }
                items.pop();
                let kept = items.len();
                calls.retain(|next| next["arguments"]["collection"] != collection);
                calls.push(call(collection, kept));
                body["next_calls"] = json!(calls);
            }
        }
        if bytes(&body) > limit {
            body = json!({"status":"board_read_index","board_id":raw["board"]["board_id"],"board_revision":revision,
                "reserved_answer_item_id":raw["reserved_answer_item_id"],"candidate_revision":raw["candidate_revision"],
                "proposed_content_revision":raw["proposed_content_revision"],"answer_position":raw["answer_position"],"answer_size":raw["answer_size"],
                "notice":"The full board did not fit. Start with layout, then read the needed collections at this board_revision. No content record was delivered.",
                "collections":["committed","layout","items","links","mappings","routes","understanding_items","understanding_refs","formal_condition_refs","candidate_items","candidate_blocks","candidate_updates","candidate_links","candidate_mappings","candidate_routes"],
                "next_calls":[call("layout",0)]});
            if bytes(&body) > limit {
                return ToolResultDraft {
                    status: ToolResultStatus::Partial,
                    model_body: Value::Null,
                    truncated: true,
                    continuation: next_call(&body),
                    evidence_arguments: arguments.into(),
                };
            }
        }
    }
    ToolResultDraft {
        status: ToolResultStatus::Partial,
        continuation: next_call(&body),
        model_body: body,
        truncated: true,
        evidence_arguments: arguments.into(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthorRequest {
    /// Run-local expression planning; intercepted before the persistent board port.
    Plan {
        focus: String,
        #[serde(default)]
        needs: Vec<String>,
    },
    Catalog {
        kind: String,
        #[serde(default)]
        query: Option<String>,
        #[serde(default)]
        after: usize,
        #[serde(default)]
        limit: Option<usize>,
        #[serde(default)]
        published_book_ref: Option<base_schema::PublishedBookRef>,
    },
    SourceRead {
        source: Value,
        #[serde(default)]
        published_book_ref: Option<base_schema::PublishedBookRef>,
    },
    Read {
        #[serde(default)]
        collection: Option<ReadCollection>,
        #[serde(default)]
        offset: usize,
        #[serde(default)]
        length: Option<usize>,
        #[serde(default)]
        board_revision: Option<u32>,
        #[serde(default)]
        candidate_revision: Option<u32>,
    },
    Prepare {
        base_board_revision: u32,
        blocks: Vec<ExplanationBlock>,
        drawings: Vec<Drawing>,
        #[serde(default)]
        update_drawings: Vec<Drawing>,
        #[serde(default)]
        layout: Vec<LayoutUpdate>,
        #[serde(default)]
        imports: Vec<SourceImport>,
        #[serde(default)]
        presentations: Vec<crate::presentation::PresentationRef>,
        #[serde(default)]
        links: Vec<BoardLink>,
        #[serde(default)]
        mappings: Vec<BoardExplanationMapping>,
        #[serde(default)]
        routes: Vec<BoardRoute>,
    },
    Preview {
        candidate_revision: u32,
        #[serde(default)]
        viewport: Option<crate::presentation_preview::PreviewViewport>,
    },
    Revise {
        candidate_revision: u32,
        #[serde(default)]
        blocks: Vec<ExplanationBlock>,
        #[serde(default)]
        drawings: Vec<Drawing>,
    },
    Deliver {
        candidate_revision: u32,
    },
    Commit {
        candidate_revision: u32,
        /// Supplied only by the runtime after checking the accepted teaching text.
        #[serde(skip)]
        reviewed_delivery: Option<ReviewedDelivery>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewedDelivery {
    pub teaching_delivery_move_refs: Vec<String>,
    pub delivery_scope: Option<Value>,
}

pub struct AuthorResult {
    pub body: Value,
    pub images: Vec<crate::presentation_author::PreviewImage>,
    pub previewed_candidate: Option<String>,
    pub delivery_text: Option<String>,
    pub invalidates_delivery: bool,
    pub committed: Option<memory::boards::BoardDelivery>,
}

/// Keep the actual preview after its image has been consumed by the author.
/// Teaching selection runs before board delivery, so reviewers need this
/// proposed surface rather than a receipt that can only exist after review.
pub(crate) struct ReviewPreview {
    pub report: Value,
    pub images: Vec<crate::presentation_author::PreviewImage>,
}

impl ReviewPreview {
    /// Resolve an explicit reference to the already inspected explanation.
    /// Do not make the teacher retype a potentially long, source-bound body.
    pub fn bind_teaching_text(&self, request: &mut Value, current: &Value, inspected: bool) -> Result<(), read_tools::ToolError> {
        let candidate = &current["candidate"];
        let committed = current["committed"].as_array().into_iter().flatten()
            .any(|part| part["commit_id"] == candidate["candidate_id"]);
        let Some(revision) = request["move"].get("board_candidate_revision").cloned() else {
            return if candidate.is_object() && !committed {
                Err(error("BOARD_TEACHING_REFERENCE_REQUIRED", "A pending native board can only be selected by move.board_candidate_revision. Omit move.delivery_text; the runtime uses the candidate's complete explanation. Change content through board.author revise, then preview and inspect the new revision."))
            } else { Ok(()) };
        };
        if request["move"].get("delivery_text").is_some() {
            return Err(error("BOARD_TEACHING_REFERENCE_REQUIRED", "Do not supply a second delivery_text for a native board. Select only move.board_candidate_revision; revise the candidate's blocks before preview if its explanation needs changes."));
        }
        if revision != candidate["candidate_revision"] || revision != self.report["candidate_revision"] {
            return Err(error("BOARD_INSPECTION_REQUIRED", "Select the current inspected board_candidate_revision; a replaced candidate cannot supply the teaching text."));
        }
        let text = candidate["delivery_text"].as_str().ok_or_else(|| error("BOARD_INSPECTION_REQUIRED", "Read and preview the current board explanation before selecting it."))?;
        if self.proposed_board(current, inspected, text)?.is_null() {
            return Err(error("BOARD_INSPECTION_REQUIRED", "This board part is already committed. Select a new inspected candidate or write the final teaching text."));
        }
        request["move"]["delivery_text"] = json!(text);
        Ok(())
    }

    pub fn proposed_board(&self, current: &Value, inspected: bool, delivery_text: &str) -> Result<Value, read_tools::ToolError> {
        let candidate = &current["candidate"];
        if current["committed"].as_array().into_iter().flatten()
            .any(|part| part["commit_id"] == self.report["candidate_id"]) {
            return Ok(Value::Null);
        }
        if !inspected || candidate["previewed"] != true || self.report["status"] != "preview_ready_for_inspection"
            || candidate["candidate_id"] != self.report["candidate_id"]
            || candidate["candidate_revision"] != self.report["candidate_revision"] {
            return Err(error("BOARD_INSPECTION_REQUIRED", "Prepare and preview the current board candidate, then inspect its image in the next sampling before selecting its teaching text."));
        }
        if candidate["base_board_revision"] != current["board"]["revision"] {
            return Err(error("BOARD_REVISION_CONFLICT", "The board changed after this preview. Read the current board, prepare and preview the updated candidate before teaching selection."));
        }
        if candidate["delivery_text"].as_str() != Some(delivery_text) {
            return Err(error("BOARD_TEACHING_TEXT_MISMATCH", format!("The proposed teaching text differs from the previewed board explanation. To use this inspected explanation, select with move.board_candidate_revision={} and omit move.delivery_text; the runtime resolves its exact body. To change it, prepare revised blocks and preview them before selecting the new revision.", candidate["candidate_revision"])));
        }
        let items = |field: &str| candidate[field].as_array().into_iter().flatten().map(|item| {
            json!({"item_id":item["item_id"],"title":item["title"],"kind":item["target"]["kind"],
                "reading_text":item["readable_content"],"explanation_blocks":item["explanation_blocks"],
                "position":item["position"],"size":item["size"],"graphic":item["target"]["graphic"]})
        }).collect::<Vec<_>>();
        Ok(json!({"status":"inspected_candidate","board_id":candidate["board_id"],
            "base_board_revision":candidate["base_board_revision"],"candidate_revision":candidate["candidate_revision"],
            "delivery_text":candidate["delivery_text"],"items":items("items"),"update_items":items("update_items"),
            "links":candidate["links"],"mappings":candidate["mappings"],"routes":candidate["routes"],
            "author_preview_observations":[self.report],
            "delivery_boundary":"Proposed native board, actually previewed and inspected; not yet committed. Review this exact diagram and explanation before allowing its save. The runtime requires the selected text and previewed revision to match at delivery."}))
    }
}

impl AuthorResult {
    pub fn value(body: Value) -> Self {
        Self {
            body,
            images: vec![],
            previewed_candidate: None,
            delivery_text: None,
            invalidates_delivery: false,
            committed: None,
        }
    }
}

pub fn error(code: &str, message: impl Into<String>) -> read_tools::ToolError {
    read_tools::ToolError {
        error_code: code.into(),
        category: "validation".into(),
        message: message.into(),
    }
}
pub fn unavailable() -> read_tools::ToolError {
    error(
        "BOARD_AUTHORING_UNAVAILABLE",
        "Native board authoring requires this run's accepted board follow-up",
    )
}

/// Native geometry cannot replace the public text accepted by the teaching loop.
pub(crate) fn validate_teaching_delivery(
    tutor: Option<&Value>,
    resolved: bool,
    accepted: Option<&Value>,
    candidate_text: &str,
) -> Result<(), read_tools::ToolError> {
    if tutor
        .is_none_or(|tutor| tutor["status"] != "active" || tutor["outside_current_request"] == true)
    {
        return Ok(());
    }
    if !resolved {
        return Err(error("BOARD_TEACHING_SELECTION_REQUIRED", "Complete tutor.step select for this teaching explanation before staging the native board. Keep the exact reviewed delivery_text in the prepared blocks."));
    }
    if accepted.is_some_and(|draft| draft["delivery_scope"]["kind"] == "board_part") {
        return Err(error("BOARD_PART_NOT_FINAL", "The selected teaching expression is one board part. Commit it and continue the original request; it cannot finish the whole answer."));
    }
    if accepted.and_then(|draft| draft["text"].as_str()) != Some(candidate_text) {
        return Err(error("BOARD_TEACHING_TEXT_MISMATCH", "The native board text differs from the accepted teaching delivery. Prepare and preview the exact reviewed delivery_text, or review the revised teaching expression with tutor.step select before delivering."));
    }
    Ok(())
}

pub(crate) fn validate_teaching_commit(
    tutor: Option<&Value>,
    _resolved: bool,
    accepted: Option<&Value>,
    candidate_text: &str,
) -> Result<(), read_tools::ToolError> {
    if tutor
        .is_none_or(|tutor| tutor["status"] != "active" || tutor["outside_current_request"] == true)
    {
        return Ok(());
    }
    if accepted.is_some_and(|draft| draft["delivery_scope"]["kind"] == "board_part") {
        if accepted.and_then(|draft| draft["text"].as_str()) != Some(candidate_text) {
            return Err(error("BOARD_TEACHING_TEXT_MISMATCH", "The board part differs from the exact reviewed local teaching expression. Prepare and preview the accepted text before committing."));
        }
        return Ok(());
    }
    Err(error("BOARD_PART_SELECTION_REQUIRED", "Select and review this local expression with delivery_scope.kind=board_part before committing. A final teaching selection uses the whole-turn delivery path."))
}

pub fn spec() -> crate::ToolSpec {
    let point = json!({"type":"object","properties":{"x":{"type":"number"},"y":{"type":"number"}},"required":["x","y"],"additionalProperties":false});
    let content = json!({"type":"object","properties":{"item_id":{"type":"string","description":"For a block in this prepare, copy read.reserved_answer_item_id. A block_id is not an item_id. For retained content, copy its observed item_id."},"block_id":{"type":["string","null"],"description":"The explicit blocks[].block_id within the answer item, or an exact observed retained block ID; null addresses the whole item."},"content_revision":{"type":"integer","minimum":1,"description":"For content created by this prepare, copy read.proposed_content_revision (base_board_revision + 1), not the current board revision or candidate revision. Retained references keep their observed content_revision."}},"required":["item_id","block_id","content_revision"],"additionalProperties":false});
    let visual = json!({"oneOf":[
        {"type":"object","properties":{"kind":{"enum":["item"]},"item_id":{"type":"string"}},"required":["kind","item_id"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"enum":["link"]},"link_id":{"type":"string"}},"required":["kind","link_id"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"enum":["presentation_anchor"]},"item_id":{"type":"string"},"reference":{"type":"object","properties":{"presentation_id":{"type":"string"},"revision":{"type":"integer"}},"required":["presentation_id","revision"],"additionalProperties":false},"anchor_id":{"type":"string"}},"required":["kind","item_id","reference","anchor_id"],"additionalProperties":false}
    ]});
    let graphic = json!({"description":r"Text and shape labels support the same math delimiters as explanation blocks: $...$, $$...$$, \(...\), \[...\]. Write valid LaTeX, e.g. \(S_t = S_{t-1} + k_t v_t^{\mathsf T}\); avoid bare pseudo-math such as S_(t-1).","oneOf":[
        {"type":"object","properties":{"kind":{"enum":["text"]},"text":{"type":"string","description":"A short heading or label with only a few supporting lines; put detailed explanation in blocks and connect it with mappings."}},"required":["kind","text"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"enum":["shape"]},"shape":{"enum":["rectangle","ellipse"]},"text":{"type":"string","description":"A concise node heading followed by a few short supporting lines. Let the shape and its links carry the visual model; move long prose to blocks."},"color":{"type":"string","description":"A readable dark outline/accent color on the light canvas. Reuse a small muted palette consistently for related concepts."}},"required":["kind","shape","text","color"],"additionalProperties":false},
        {"type":"object","properties":{"kind":{"enum":["path"]},"points":{"type":"array","items":point,"description":"Local coordinates within this drawing's size, relative to its position. Canvas coordinates equal position + point. For a circle around another item, place the path's position at the circle bounds and points around 0..width / 0..height; do not repeat absolute canvas coordinates."},"color":{"type":"string"},"width":{"type":"number"}},"required":["kind","points","color","width"],"additionalProperties":false}
    ]});
    let size = json!({"type":"object","properties":{"width":{"type":"number"},"height":{"type":"number"}},"required":["width","height"],"additionalProperties":false});
    let publication = json!({"type":"object","properties":{"book_id":{"type":"string"},"publication_id":{"type":"string"}},"required":["book_id","publication_id"],"additionalProperties":false});
    let presentation_ref = json!({"type":"object","properties":{"presentation_id":{"type":"string"},"revision":{"type":"integer","minimum":1}},"required":["presentation_id","revision"],"additionalProperties":false});
    let scene_receipt = json!({"type":"object","properties":{"session_id":{"type":"string"},"turn_id":{"type":"string"},"reference":presentation_ref,"state_revision":{"type":"integer","minimum":1},"saved_state_ref":{"type":"string"}},"required":["session_id","turn_id","reference","state_revision","saved_state_ref"],"additionalProperties":false});
    let source = json!({"description":"Copy the exact source object from catalog. Historical answers and presentations resolve their original publication automatically.","oneOf":[
        json!({"type":"object","properties":{"kind":{"enum":["artifact"]},"artifact":{"enum":["pass1","discourse","book_structure"]},"entry_id":{"type":"string"},"source_revision":{"type":"string"}},"required":["kind","artifact","entry_id","source_revision"],"additionalProperties":false}),
        json!({"type":"object","properties":{"kind":{"enum":["answer"]},"session_id":{"type":"string"},"turn_id":{"type":"string"}},"required":["kind","session_id","turn_id"],"additionalProperties":false}),
        json!({"type":"object","properties":{"kind":{"enum":["presentation"]},"session_id":{"type":"string"},"turn_id":{"type":"string"},"reference":presentation_ref,"receipt":scene_receipt},"required":["kind","session_id","turn_id","reference"],"additionalProperties":false}),
        json!({"type":"object","properties":{"kind":{"enum":["prebuild"]},"object_id":{"type":"string"},"object_revision":{"type":"integer","minimum":1},"map_revision":{"type":"string"}},"required":["kind","object_id","object_revision","map_revision"],"additionalProperties":false}),
        json!({"type":"object","properties":{"kind":{"enum":["understanding"]},"evidence_id":{"type":"string"}},"required":["kind","evidence_id"],"additionalProperties":false})
    ]});
    let drawing = json!({"type":"object","properties":{"item_id":{"type":"string"},"title":{"type":"string"},"position":point,"size":size,"graphic":graphic,"based_on":{"type":"array","items":content}},"required":["item_id","title","position","size","graphic"],"additionalProperties":false});
    let extra = json!({
            "kind":{"enum":["answer","presentation","prebuild","understanding"]},
            "query":{"type":"string","description":"Search the full permitted source contents before pagination, including answers beyond the first page."},
            "after":{"type":"integer","minimum":0},
            "limit":{"type":"integer","minimum":1,"maximum":50},
            "published_book_ref":publication,
            "source":source,
            "imports":{"type":"array","items":{"type":"object","properties":{"source":source,"published_book_ref":publication,"item_id":{"type":"string"},"position":point,"size":size},"required":["source","item_id","position","size"],"additionalProperties":false},"description":"Resolve exact original sources into the private candidate; full content and provenance are server owned."},
            "update_drawings":{"type":"array","items":drawing,"description":"Explicitly replace native graphic/text/geometry of existing drawings using their exact item_id, including visual refinements to an existing board. Preserve source meaning and provenance; prior revisions stay readable."},
            "layout":{"type":"array","items":{"type":"object","properties":{"item_id":{"type":"string"},"position":point,"size":size},"required":["item_id","position","size"],"additionalProperties":false},"description":"Move or resize retained items without changing their saved content. Use aligned groups and generous gaps so relationships and labels stay readable."}
    });
    let mut spec = crate::ToolSpec { name: "board.author".into(), description: "Use this accepted board when a visual relationship helps the current request. A short explanation, correction or clarification can be answered directly without authoring a diagram. Prioritize the host-selected explanation block over the rest of a long previous answer. When drawing is useful, read the relevant board layout and exact sources. Call plan with the concrete relationship before preparing new visuals or changing the explanation strategy; consume its coach handoff in the next sampling before drawing. Plan is run-local and reused by later prepare/revise/preview calls. Layout, font or position-only adjustments do not need a new plan. Search catalog or workspace.records.read only when relevant existing material or original conversations are needed. catalog searches all permitted sources before returning a bounded page; source_read returns an exact original snapshot and prepare imports resolves that same source identity. State the current learning workspace scope and never fabricate history or understanding. Build native mind maps with text/shapes and labelled links; circle/highlight with paths; connect explanations to visuals with mappings and progressive focus/reveal routes. Update existing native drawings through update_drawings and retained geometry through layout while keeping original answers and learner quotes immutable. Read the accepted persistent board, prepare native drawings with stable explanation blocks and a local teaching route, inspect the real Reader BoardCanvas preview, then stage exactly that observed candidate for the final successful answer. read returns the reserved answer item ID, current content revision and exact retained knowledge/understanding snapshots. Each prepare replaces the candidate and invalidates its preview. New content references use base_board_revision+1; existing references retain their exact observed revision. Every explanation block has an explicit stable block_id; never infer blocks from Markdown positions. Cite new source claims only with this turn's observed source.present references. Drawing based_on references identify formal conditions or actual learner evidence; they do not establish mastery. Do not fabricate learner quotes or understanding records; use tutor.step evidence for real user expressions. Preview images must be inspected in a later model sampling before deliver. Use commit to publish one complete observed part and keep working. A committed part survives cancellation or failure; read returns its exact receipts and the next reserved answer item ID. After any parts are committed, explicitly commit every additional part and finish with references to the saved parts. Deliver keeps the whole-turn staging path only when no parts have been committed. If the board changes, read and prepare/preview again. Existing presentations use declared real anchor IDs and exact PresentationRef; this tool never changes their experimental state.".into(), parameters: json!({
        "type":"object","properties":{
            "operation":{"enum":["read","catalog","source_read","plan","prepare","revise","preview","deliver","commit"]},
            "focus":{"type":"string","minLength":1,"description":"The concrete relationship the reader should understand, especially the selected explanation's unresolved point. Call plan before designing new visuals or changing the explanation strategy."},
            "needs":{"type":"array","items":{"type":"string"},"description":"Unresolved evidence or representation needs for this explanation, not drawing coordinates."},
            "base_board_revision":{"type":"integer","minimum":1},
            "candidate_revision":{"type":"integer","minimum":1},
            "presentations":{"type":"array","items":{"type":"object","properties":{"presentation_id":{"type":"string"},"revision":{"type":"integer","minimum":1}},"required":["presentation_id","revision"],"additionalProperties":false},"description":"Exact presentation versions already delivered by presentation.author in this run. They become independent retained board items in this part; do not invent Markdown presentation markers."},
            "collection":{"enum":["committed","layout","items","links","mappings","routes","understanding_items","understanding_refs","formal_condition_refs","candidate_items","candidate_blocks","candidate_updates","candidate_links","candidate_mappings","candidate_routes"],"description":"Read complete records from one collection. layout returns every item's identity and geometry without long retained answer text. Follow next_calls to recover omitted collections."},
            "offset":{"type":"integer","minimum":0},
            "length":{"type":"integer","minimum":1,"maximum":50},
            "board_revision":{"type":"integer","minimum":1,"description":"Pin continuation pages to the observed board revision; read again after a conflict."},
            "blocks":{"type":"array","description":"The explanation needed for this request and this visual relationship. Attach this turn's returned [[source:<source_ref_id>]] markers to substantive book claims in markdown; source.present calls without markers do not create clickable citations. Explain how to read the new visual or resolve the selected point; do not repeat the previous full answer merely to attach a diagram. Keep prose here and map it to concise visual nodes.","items":{"type":"object","properties":{"block_id":{"type":"string","description":"Stable ID unique across this board. Reuse it when revising the same uncommitted part; use a new ID for a later committed part."},"markdown":{"type":"string"}},"required":["block_id","markdown"],"additionalProperties":false}},
            "drawings":{"type":"array","items":{"type":"object","properties":{"item_id":{"type":"string"},"title":{"type":"string"},"position":point,"size":{"type":"object","properties":{"width":{"type":"number"},"height":{"type":"number"}},"required":["width","height"],"additionalProperties":false},"graphic":graphic,"based_on":{"type":"array","items":content}},"required":["item_id","title","position","size","graphic"],"additionalProperties":false}},
            "links":{"type":"array","items":{"type":"object","properties":{"link_id":{"type":"string"},"from":visual,"to":visual,"label":{"type":"string"},"based_on":{"type":"array","items":content}},"required":["link_id","from","to","label"],"additionalProperties":false}},
            "mappings":{"type":"array","items":{"type":"object","properties":{"mapping_id":{"type":"string"},"explanation":content,"visual":visual},"required":["mapping_id","explanation","visual"],"additionalProperties":false}},
            "routes":{"type":"array","description":"Focus each teaching step on a small related group and its connecting links. Keep the overall relationship model coherent instead of fitting all details into every step.","items":{"type":"object","properties":{"route_id":{"type":"string"},"title":{"type":"string"},"steps":{"type":"array","items":{"type":"object","properties":{"step_id":{"type":"string"},"explanation":{"anyOf":[content,{"type":"null"}]},"reveal_targets":{"type":"array","items":visual},"focus_targets":{"type":"array","items":visual},"transition":{"enum":["instant","draw","reveal_text"]},"duration_ms":{"type":"integer","minimum":0}},"required":["step_id","explanation","reveal_targets","focus_targets","transition","duration_ms"],"additionalProperties":false}},},"required":["route_id","title","steps"],"additionalProperties":false}},
            "viewport":{"type":"object","properties":{"width":{"type":"integer","minimum":240,"maximum":1920},"height":{"type":"integer","minimum":160,"maximum":2160},"input":{"enum":["mouse","touch"]}},"required":["width","height","input"],"additionalProperties":false}
        },"required":["operation"],"additionalProperties":false,
        "anyOf":[
            {"properties":{"operation":{"enum":["read"]}},"required":["operation"]},
            {"properties":{"operation":{"enum":["prepare"]}},"required":["operation","base_board_revision","blocks","drawings"]},
            {"properties":{"operation":{"enum":["preview"]}},"required":["operation","candidate_revision"]},
            {"properties":{"operation":{"enum":["deliver"]}},"required":["operation","candidate_revision"]}
        ]
    }) };
    spec.description.push(' ');
    spec.description.push_str(SOURCE_CITATION_POLICY);
    spec.description.push_str(" Compose around a clear visual model using shapes, spatial groups and labelled links. Give related nodes consistent alignment, generous whitespace and a small muted color palette. Keep node headings short and supporting text sparse; allocate enough space for ordinary headings around 22-26 px and body text around 16-19 px at natural scale. Put long explanations in blocks instead of shrinking text to cram the whole board onto one screen. Use local route focus to make each group readable while preserving the full model's relationships. Inspect hierarchy, spacing and readable text in the real preview. Continue revising only to fix a concrete wrong relationship, overlap, unreadable text or missing requested meaning; once those issues are resolved, deliver the inspected candidate.");
    spec.description.push_str(" revise changes only the supplied existing blocks by block_id and drawings by item_id in the current uncommitted candidate, preserving every other item, link, mapping and route. Supply at least one change. It creates a new revision and invalidates the previous preview and teaching approval. Read collection=candidate_blocks for the original source-bound blocks. Preview and inspect the revised candidate before selecting it by move.board_candidate_revision; never rewrite its teaching body at select.");
    spec.parameters["properties"].as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    crate::tool_schema::complete_spec(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_board_author_requires_clickable_sources_for_book_claims() {
        let registry = crate::orchestrator::resident_tool_registry();
        let registration = registry.registration("board.author").unwrap();
        assert!(registration.spec.description.contains(SOURCE_CITATION_POLICY));
        assert!(registration.spec.parameters["properties"]["blocks"]["description"]
            .as_str().unwrap().contains("[[source:<source_ref_id>]]"));
        let modules = crate::agent_prompt::policy_modules_for_tools(&[crate::ToolSpec {
            name: "source.present".into(), description: String::new(), parameters: json!({}),
        }]);
        let policy = &modules.iter().find(|module| module.asset_id == "resident-agent.policy.source-delivery").unwrap().text;
        assert!(policy.contains("Board replies are an exception"),
            "the general optional-citation default must not contradict the board requirement");
    }

    #[test]
    fn native_plan_and_links_without_evidence_pass_the_actual_tool_contract() {
        let registry = crate::orchestrator::resident_tool_registry();
        let registration = registry.registration("board.author").unwrap();
        let plan = json!({"operation":"plan","focus":"一次写入如何改变原数组"});
        registration.validate_arguments(&plan.to_string()).unwrap();
        assert!(matches!(serde_json::from_value::<AuthorRequest>(plan).unwrap(), AuthorRequest::Plan { .. }));
        let prepare = json!({"operation":"prepare","base_board_revision":1,"blocks":[{"block_id":"example","markdown":"同一个数组元素发生变化。"}],"drawings":[],
            "links":[{"link_id":"mutation","from":{"kind":"item","item_id":"before"},"to":{"kind":"item","item_id":"after"},"label":"写入"}]});
        registration.validate_arguments(&prepare.to_string()).unwrap();
        let AuthorRequest::Prepare { links, .. } = serde_json::from_value::<AuthorRequest>(prepare).unwrap() else { panic!() };
        assert!(links[0].based_on.is_empty());
    }

    #[test]
    fn board_selection_requires_a_reference_and_never_accepts_a_second_body() {
        let preview = ReviewPreview { report:json!({"status":"preview_ready_for_inspection","candidate_id":"board:turn:1","candidate_revision":1}), images:vec![] };
        let current = json!({"board":{"revision":1},"committed":[],"candidate":{
            "board_id":"board","candidate_id":"board:turn:1","candidate_revision":1,"base_board_revision":1,
            "previewed":true,"delivery_text":"The exact explanation.","items":[],"update_items":[],"links":[],"mappings":[],"routes":[]}});
        let mut rewritten = json!({"operation":"select","move":{"delivery_text":"The exact explanation."}});
        assert_eq!(preview.bind_teaching_text(&mut rewritten, &current, true).unwrap_err().error_code, "BOARD_TEACHING_REFERENCE_REQUIRED");
        let mut duplicate = json!({"operation":"select","move":{"board_candidate_revision":1,"delivery_text":"The exact explanation."}});
        assert_eq!(preview.bind_teaching_text(&mut duplicate, &current, true).unwrap_err().error_code, "BOARD_TEACHING_REFERENCE_REQUIRED");
        let mut referenced = json!({"operation":"select","move":{"board_candidate_revision":1}});
        preview.bind_teaching_text(&mut referenced, &current, true).unwrap();
        assert_eq!(referenced["move"]["board_candidate_revision"], 1);
        assert_eq!(referenced["move"]["delivery_text"], "The exact explanation.");
    }

    #[test]
    fn review_preview_uses_only_the_current_inspected_candidate_and_public_board_content() {
        let preview = ReviewPreview { report:json!({"status":"preview_ready_for_inspection","candidate_id":"board:turn:2","candidate_revision":2,
            "observations":[{"dom":{"text":"History → State","board":{"ready":true}}}]}), images:vec![] };
        let current = json!({"board":{"revision":3},"committed":[],"candidate":{
            "board_id":"board","candidate_id":"board:turn:2","candidate_revision":2,"base_board_revision":3,"previewed":true,
            "delivery_text":"Explain the state.","items":[{"item_id":"state","title":"State","readable_content":"A fixed-size table accumulates each token.",
                "target":{"kind":"drawing","graphic":{"kind":"text","text":"History → State"}},"source_bindings":[{"text":"PRIVATE_SOURCE"}]}],
            "update_items":[],"links":[],"mappings":[],"routes":[]}});
        let board = preview.proposed_board(&current, true, "Explain the state.").unwrap();
        assert_eq!(board["status"], "inspected_candidate");
        assert_eq!(board["items"][0]["reading_text"], "A fixed-size table accumulates each token.");
        assert_eq!(board["author_preview_observations"][0], preview.report);
        assert!(!board.to_string().contains("PRIVATE_SOURCE"));
        assert!(preview.proposed_board(&current, false, "Explain the state.").is_err());
        assert_eq!(preview.proposed_board(&current, true, "A different explanation.").unwrap_err().error_code, "BOARD_TEACHING_TEXT_MISMATCH");
        let selection = json!({"operation":"select","move":{"board_candidate_revision":2}});
        let mut resolved = selection.clone();
        preview.bind_teaching_text(&mut resolved, &current, true).unwrap();
        assert_eq!(resolved["move"]["delivery_text"], "Explain the state.");
        assert_eq!(resolved["move"]["board_candidate_revision"], 2);
        assert!(preview.bind_teaching_text(&mut selection.clone(), &current, false).is_err());
        let mut conflict = selection.clone(); conflict["move"]["delivery_text"] = json!("A different explanation.");
        assert_eq!(preview.bind_teaching_text(&mut conflict, &current, true).unwrap_err().error_code, "BOARD_TEACHING_REFERENCE_REQUIRED");
        let mut stale = selection.clone(); stale["move"]["board_candidate_revision"] = json!(1);
        assert!(preview.bind_teaching_text(&mut stale, &current, true).is_err());
        for (pointer, value) in [("/candidate/previewed", json!(false)), ("/candidate/candidate_revision", json!(3)),
            ("/candidate/candidate_id", json!("board:turn:3")), ("/board/revision", json!(4))] {
            let mut changed = current.clone(); *changed.pointer_mut(pointer).unwrap() = value;
            assert!(preview.proposed_board(&changed, true, "Explain the state.").is_err(), "{pointer}");
        }
        let mut committed = current;
        committed["committed"] = json!([{"commit_id":"board:turn:2"}]);
        assert!(preview.proposed_board(&committed, true, "The final summary.").unwrap().is_null());
        assert!(preview.bind_teaching_text(&mut selection.clone(), &committed, true).is_err());
    }

    #[test]
    fn prepare_preserves_decimal_geometry_through_tagged_request_and_path() {
        // Same JSON shape sent by the real board author; integers alone do not
        // expose arbitrary_precision's internally tagged Content conversion.
        let input = r##"{"operation":"prepare","base_board_revision":2,
            "blocks":[{"block_id":"time","markdown":"分别计算时间。"}],
            "drawings":[{"item_id":"time-path","title":"两段时间","position":{"x":0.0,"y":760.5},
                "size":{"width":290.25,"height":180.75},"graphic":{"kind":"path",
                "points":[{"x":0.5,"y":1.25},{"x":120.75,"y":60.125}],"color":"#177e77","width":2.5},"based_on":[]}]}
        "##;
        let from_text = serde_json::from_str::<AuthorRequest>(input).unwrap();
        let from_value =
            serde_json::from_value::<AuthorRequest>(serde_json::from_str(input).unwrap()).unwrap();
        for request in [from_text, from_value] {
            let AuthorRequest::Prepare { drawings, .. } = request else {
                panic!()
            };
            assert_eq!(drawings[0].position, BoardPosition { x: 0.0, y: 760.5 });
            assert_eq!(
                drawings[0].size,
                BoardSize {
                    width: 290.25,
                    height: 180.75
                }
            );
            let BoardGraphic::Path { points, width, .. } = &drawings[0].graphic else {
                panic!()
            };
            assert_eq!(*width, 2.5);
            assert_eq!(points[0], memory::boards::BoardPoint { x: 0.5, y: 1.25 });
            assert_eq!(
                points[1],
                memory::boards::BoardPoint {
                    x: 120.75,
                    y: 60.125
                }
            );
        }
    }
}
