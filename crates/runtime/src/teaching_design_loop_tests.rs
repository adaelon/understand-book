use super::*;
include!("../../read-tools/testdata/math_book.rs");
include!("tool_argument_recovery_tests.rs");
include!("teaching_review_usage_tests.rs");
include!("local_followup_tests.rs");

#[test]
fn math_structure_source_reading_reaches_author_and_delivery() {
    let b=optimization_math_book();
    let mut store=MemoryStore::open(tmp("math-source-teaching")).unwrap();
    let mut reader=Reader::new(&b,DEFAULT_RADIUS);
    let snapshot=default_profile_snapshot(&b,&store,"t0");
    let question="1.8 是要求证明，还是已经给出证明？请直接解释题目关系。";
    let answer="习题要求利用定理 6.5 去证明推论 6.2：定理是要使用的结果，推论是待证明的结论。这句题目本身没有给出证明。";
    let mut plan=design();
    plan["target"]=serde_json::json!({"learning_focus":"习题的前提和待证结论","expected_performance":"区分引用的定理与要求证明的推论",
        "capability":"explanation","source_bindings":[{"start_lid":"1.8","end_lid":"1.9"}],"object_refs":[]});
    plan["organizing_model"]=serde_json::json!("使用定理建立待证推论。");
    plan["blocks"]=serde_json::json!([{"block_id":"exercise-link","relation":"定理是引用的结果，推论是待证结论。",
        "purpose":"理解题目任务","source_bindings":[{"start_lid":"1.8","end_lid":"1.9"}]}]);
    plan["focus_block_refs"]=serde_json::json!(["exercise-link"]);
    let mut selection=selected_body("math-explanation",answer);
    selection["move"]["target"]=plan["target"].clone();
    selection["move"]["block_refs"]=serde_json::json!(["exercise-link"]);
    selection["move"]["interaction_intent"]=serde_json::json!("direct_explanation");
    selection["move"]["intent_origin"]=serde_json::json!("current_request");
    selection["move"]["intent_quote"]=serde_json::json!("请直接解释题目关系");
    selection["move"]["prompt"]=serde_json::json!("说明习题要求。");
    selection["move"]["participation"]=serde_json::json!({"choice":"conclude","reason":"直接说明题目中的使用关系和待证任务。"});
    let adapter=ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("exercise-heading","book.text",r#"{"lid":"1.8"}"#)]),
        turn_calls(vec![call("exercise-body","book.text",r#"{"lid":"1.9"}"#)]),
        turn_calls(vec![call("related-results","book.text",r#"{"lid":"1.5","end_lid":"1.7"}"#)]),
        turn_calls(vec![call("explain-task","tutor.step",&selection.to_string())]),
        turn_final(answer),
    ],vec![accepted_review()]);
    let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(8),..Default::default()},adapter.model_runtime_profile());
    context.tool_exposure_state.activate("book.text");
    context.tutor=Some(serde_json::json!({"status":"practice","current_design":{"design_ref":"design:current","design":plan},
        "learner_context":{"current_request":question}}));
    let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
    let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),question,"t0").unwrap();
    assert!(!outcome.incomplete);
    assert_eq!(outcome.answer.as_deref(),Some(answer));
    assert_eq!(port.accepted,["select"]);
    assert!(adapter.seen_reviews.borrow().is_empty() && port.reviews.is_empty());
    let reads=serde_json::to_string(&adapter.inner.0.seen_plans.borrow().last().unwrap().ordered_messages()).unwrap();
    for relation in ["uses", "asks_to_prove", "定理 6.5"] {
        assert!(reads.contains(relation), "the author must receive actual math relationships: {relation}");
    }
}

fn traced_learning_support() -> serde_json::Value {
    serde_json::json!({"delivery_ref":"delivered:help", "answer_preview":"The catalog preserves the cards.",
        "responding_to_expressions":[{"fact_ref":"usage:before","expression_preview":"B is gone when omitted.",
            "support_delivery_refs":[],"assistance_refs":[]}],
        "learner_expressions":[{"fact_ref":"usage:after","expression_preview":"An added route item could already be in the catalog.",
            "support_delivery_refs":["delivered:help"],"assistance_refs":["help:before-input"]}]})
}

struct TeachingPort<'a> {
    inner: BorrowedResidentState<'a>,
    accepted: Vec<String>,
    selections: BTreeMap<String, serde_json::Value>,
    reviews: Vec<serde_json::Value>,
}
impl ResidentStatePort for TeachingPort<'_> {
    fn tutor_record_review(&mut self, record: serde_json::Value) -> Result<serde_json::Value, ToolError> {
        self.reviews.push(record);
        Ok(serde_json::Value::Null)
    }
    fn tutor_step(&mut self, request: serde_json::Value, _: &[SourceBinding], _: &[read_tools::EvidenceRange]) -> Result<serde_json::Value, ToolError> {
        let operation = request["operation"].as_str().unwrap();
        self.accepted.push(operation.into());
        Ok(match operation {
            "design" => serde_json::json!({"design_ref":"design:current","design":request["design"],
                "learner_context_update":{"learner_grounding":{"expressions":[{"fact_ref":"usage:before","text":"B is gone when omitted."}],"interpretations":[{"evidence_ref":"evidence:focused","interpretation":"RELATION_SCOPED_CONTEXT", "detail":"The learner's long explanation. ".repeat(500)}]}}}),
            "evidence" => serde_json::json!({"evidence_ref":request["operation_id"], "nature":"hypothesis",
                "learner_context_update":{"interpretations":[{"evidence_ref":request["operation_id"],"interpretation":request["interpretation"],"assistance_refs":["help:before-input"]}],"learner_grounding":{"expressions":[{"fact_ref":"usage:after","text":"An added route item could already be in the catalog."}],"interpretations":[{"evidence_ref":request["operation_id"],"interpretation":request["interpretation"],"fact_refs":["usage:after"]}]},"uninterpreted_fact_refs":["usage:other"],"evidence_watermark":if request["operation_id"]=="evidence:first" {1} else {2},
                    "learning_opportunity":{"block_refs":["keep-select"],"interpretation_refs":[request["operation_id"]],
                        "delivered_support":[traced_learning_support()]}}}),
            "select" => {
                let reference = if request["move"]["move_id"] == "feedback" {"selected:current".to_owned()}
                    else {format!("selected:{}", request["move"]["move_id"].as_str().unwrap())};
                let mut accepted_move = request["move"].clone();
                accepted_move["explanation"] = serde_json::json!("ACCEPTED_PRIVATE_REASONING");
                self.selections.insert(reference.clone(), accepted_move);
                let mut references: Vec<String> = serde_json::from_value(request.get("delivery_move_refs").cloned().unwrap_or(serde_json::json!([]))).unwrap();
                let mut included = BTreeSet::new();
                references.retain(|id| included.insert(id.clone()));
                if !references.contains(&reference) { references.push(reference.clone()); }
                let moves: Vec<_> = references.iter().map(|id| self.selections[id].clone()).collect();
                let texts: Vec<_> = moves.iter().map(|movement| {
                    let mut text = movement["delivery_text"].as_str().unwrap().to_owned();
                    if let Some(question) = movement["response_expectation"]["public_prompt"].as_str().filter(|question| !text.contains(*question)) {
                        text.push_str("\n\n"); text.push_str(question);
                    }
                    text
                }).collect();
                let mut result = serde_json::json!({"selected":reference,"delivery_move_refs":references,
                    "delivery_draft":{"delivery_move_refs":references,"text":texts.join("\n\n"),"moves":moves}});
                if let Some(scope) = request.get("delivery_scope") {
                    result["delivery_scope"] = scope.clone();
                    result["delivery_draft"]["delivery_scope"] = scope.clone();
                }
                result
            },
            "outside" => serde_json::json!({"outside":true}),
            _ => unreachable!(),
        })
    }
    fn author_presentation(&mut self, request: crate::presentation_author::AuthorRequest, _: &[SourceBinding], _: &[Message], _: &crate::run_context::CancellationToken) -> Result<crate::presentation_author::AuthorResult, ToolError> {
        use crate::presentation_author::{AuthorRequest, AuthorResult, PreviewImage};
        let mut result = AuthorResult {body:serde_json::Value::Null,images:vec![],previewed_candidate:None,delivered:None};
        match request {
            AuthorRequest::Write {title,..} => result.body=serde_json::json!({"status":"candidate_saved","candidate_id":format!("c{title}")}),
            AuthorRequest::Patch {candidate_id:Some(candidate_id),teaching_binding:Some(binding),..} => {
                assert_eq!(candidate_id,"c1");
                assert_eq!(binding.design_ref,"design:current");
                result.body=serde_json::json!({"status":"candidate_saved","candidate_id":"c2"});
            },
            AuthorRequest::Read {candidate_id:Some(_),file:None,length:Some(1),..} => {
                result.body=serde_json::json!({"teaching_binding":{"design_ref":"design:current","block_refs":["keep-select"],"regions":[]}});
            },
            AuthorRequest::Preview {candidate_id,..} => {
                result.body=serde_json::json!({"status":"preview_ready_for_inspection","candidate_id":candidate_id,
                    "environment_name":"desktop-content","observations":[{"step":1,"dom":{"text":format!("ACTUAL_PREVIEW_{candidate_id}")},"action":{"kind":"click","selector":"#toggle"}}],
                    "reading":{"page_state":{"route_selected":false},"text":"B remains in the catalog."}});
                result.previewed_candidate=Some(candidate_id.clone());
                result.images.push(PreviewImage {caption:"Inspect the concrete card after switching this route off.".into(),png_base64:format!("png-{candidate_id}"),candidate_id:Some(candidate_id),environment_name:Some("desktop-content".into())});
            },
            AuthorRequest::Deliver {candidate_id} => {
                let reference=crate::presentation::PresentationRef {presentation_id:"cards".into(),revision:candidate_id.strip_prefix('c').unwrap().parse().unwrap()};
                result.body=serde_json::json!({"status":"version_saved","reference":reference});
                result.delivered=Some(reference);
            },
            AuthorRequest::Read {reference:Some(reference),file:None,length:Some(1),..} => {
                result.body=serde_json::json!({"status":"version_read","reference":reference,"title":"The same retained card",
                    "readable_content":format!("SAVED_PAGE_BODY_{}",reference.revision),"initial_state":{"route_selected":true},
                    "state_contract":{"route_selected":"Whether this route selects B."},
                    "teaching_binding":{"design_ref":"design:current","block_refs":["keep-select"],"regions":[{"region_id":"catalog","block_refs":["keep-select"],"state_fields":["route_selected"],"observation":"B remains while route selection changes."}]}});
            },
            _ => panic!("unexpected presentation request"),
        }
        Ok(result)
    }
    fn submit_private<R>(&mut self, operation: impl FnOnce(&mut MemoryStore) -> R) -> Result<R, ToolError> { self.inner.submit_private(operation) }
    fn read_live_reader<R>(&mut self, operation: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> { self.inner.read_live_reader(operation) }
    fn apply_reader<R>(&mut self, operation: impl FnOnce(&mut MemoryStore, &mut Reader) -> R) -> Result<R, ToolError> { self.inner.apply_reader(operation) }
    fn reader_input(&mut self, book: &Book, question: &str) -> crate::run_context::ReaderInputSnapshot { self.inner.reader_input(book, question) }
}

struct SourcePreflightTeachingPort<'a> {
    inner: TeachingPort<'a>,
    observed_reads: RefCell<Vec<Vec<read_tools::EvidenceRange>>>,
    required: read_tools::EvidenceRange,
}
impl ResidentStatePort for SourcePreflightTeachingPort<'_> {
    fn tutor_record_review(&mut self, record: serde_json::Value) -> Result<serde_json::Value, ToolError> {
        self.inner.tutor_record_review(record)
    }
    fn tutor_step_preflight(&self, request: &serde_json::Value, _: &[SourceBinding], ranges: &[read_tools::EvidenceRange]) -> Result<serde_json::Value, crate::run_context::TeachingSelectionPreflightError> {
        self.observed_reads.borrow_mut().push(ranges.to_vec());
        if !ranges.contains(&self.required) {
            assert!(self.inner.accepted.is_empty(), "an unread candidate must not reach the selection host");
            return Err(crate::run_context::TeachingSelectionPreflightError {
                error: memory::teaching::invalid("Read the missing source paragraph before reviewing this candidate."),
                missing_source_read: Some(self.required.clone()),
            });
        }
        let mut normalized = request.clone();
        if request["operation"] == "design" { return Ok(normalized); }
        normalized["move"]["interaction_intent"] = serde_json::json!("direct_explanation");
        normalized["move"]["intent_origin"] = serde_json::json!("current_action");
        normalized["move"]["target"] = design()["target"].clone();
        Ok(normalized)
    }
    fn tutor_step(&mut self, request: serde_json::Value, evidence: &[SourceBinding], ranges: &[read_tools::EvidenceRange]) -> Result<serde_json::Value, ToolError> {
        self.inner.tutor_step(request, evidence, ranges)
    }
    fn submit_private<R>(&mut self, operation: impl FnOnce(&mut MemoryStore) -> R) -> Result<R, ToolError> { self.inner.submit_private(operation) }
    fn read_live_reader<R>(&mut self, operation: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> { self.inner.read_live_reader(operation) }
    fn apply_reader<R>(&mut self, operation: impl FnOnce(&mut MemoryStore, &mut Reader) -> R) -> Result<R, ToolError> { self.inner.apply_reader(operation) }
    fn reader_input(&mut self, book: &Book, question: &str) -> crate::run_context::ReaderInputSnapshot { self.inner.reader_input(book, question) }
}

struct StreamingTeachingAdapter(RequestPlanRecordingAdapter);
impl ModelAdapter for StreamingTeachingAdapter {
    fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, AdapterError> { self.0.complete(request) }
    fn complete_structured(&self, request: CompletionRequest) -> Result<serde_json::Value, AdapterError> { self.0.complete_structured(request) }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> { self.0.chat(request) }
    fn chat_observed(&self, request: &AgentRequestPlan, observer: &mut dyn crate::provider_stream::ModelObserver) -> Result<AssistantTurn, AdapterError> {
        let turn = self.chat(request)?;
        if let Some(text) = &turn.text { observer.observe(crate::provider_stream::ModelDelta::Text(format!("{text}\n\n"))); }
        Ok(turn)
    }
}

struct ReviewedTeachingAdapter {
    inner: StreamingTeachingAdapter,
    reviews: RefCell<VecDeque<Result<serde_json::Value, AdapterError>>>,
    explanation_reviews: RefCell<VecDeque<Result<serde_json::Value, AdapterError>>>,
    seen_reviews: RefCell<Vec<serde_json::Value>>,
    seen_explanation_reviews: RefCell<Vec<serde_json::Value>>,
    seen_review_instructions: RefCell<Vec<String>>,
    seen_review_images: RefCell<Vec<Vec<crate::presentation_author::PreviewImage>>>,
    seen_review_reasoning_efforts: RefCell<Vec<Option<String>>>,
    reported_review_usage: Option<u32>,
    reported_explanation_usage: Option<u32>,
    cancel_after_explanation: Option<crate::run_context::CancellationToken>,
    seen_purposes: RefCell<Vec<String>>,
    seen_review_requests: RefCell<Vec<CompletionRequest>>,
}
impl ReviewedTeachingAdapter {
    fn new(chats: Vec<AssistantTurn>, reviews: Vec<Result<serde_json::Value, AdapterError>>) -> Self {
        Self { inner:StreamingTeachingAdapter(RequestPlanRecordingAdapter::new(chats, vec![])),
            reviews:RefCell::new(reviews.into()), explanation_reviews:RefCell::new(VecDeque::new()),
            seen_reviews:RefCell::new(Vec::new()), seen_explanation_reviews:RefCell::new(Vec::new()),
            seen_review_instructions:RefCell::new(Vec::new()), seen_review_images:RefCell::new(Vec::new()),
            seen_review_reasoning_efforts:RefCell::new(Vec::new()), reported_review_usage:Some(171),
            reported_explanation_usage:Some(171), cancel_after_explanation:None,
            seen_purposes:RefCell::new(Vec::new()), seen_review_requests:RefCell::new(Vec::new()) }
    }
}

fn captured_review_packet(request: &CompletionRequest) -> serde_json::Value {
    let mut packet: serde_json::Value = serde_json::from_str(&request.user).unwrap();
    if let Some(followup) = &request.followup_user {
        packet["review_format_repair"] = serde_json::from_str(followup.strip_prefix("review_format_repair:\n").unwrap()).unwrap();
    }
    packet
}

impl ModelAdapter for ReviewedTeachingAdapter {
    fn set_model_purpose(&self, purpose: &str) { self.seen_purposes.borrow_mut().push(purpose.to_owned()); }
    fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, AdapterError> { self.inner.complete(request) }
    fn complete_structured(&self, request: CompletionRequest) -> Result<serde_json::Value, AdapterError> {
        if request.system.contains("Teaching delivery acceptance review.") {
            self.seen_review_requests.borrow_mut().push(request.clone());
            let packet = captured_review_packet(&request);
            self.seen_reviews.borrow_mut().push(packet.clone());
            self.seen_review_instructions.borrow_mut().push(request.system);
            self.seen_review_images.borrow_mut().push(request.images);
            self.seen_review_reasoning_efforts.borrow_mut().push(request.reasoning_effort);
            self.reviews.borrow_mut().pop_front().expect("unexpected teaching review")
                .map(|review|review_fixture(review, &packet))
        } else if request.system.contains("Teaching explanation acceptance review.") {
            self.seen_review_requests.borrow_mut().push(request.clone());
            let packet = captured_review_packet(&request);
            self.seen_explanation_reviews.borrow_mut().push(packet.clone());
            let standard = serde_json::json!({"original_request":packet["current_request"],
                "proposed_delivery":{"text":packet["current_delivery"]},"open_issues":packet["open_issues"],
                "revision_reference":packet["revision_reference"]});
            let result = self.explanation_reviews.borrow_mut().pop_front().unwrap_or_else(accepted_review)
                .map(|review|review_fixture(review, &standard));
            if let Some(cancellation) = &self.cancel_after_explanation { cancellation.cancel(); }
            result
        } else { self.inner.complete_structured(request) }
    }
    fn complete_structured_observed(&self, request: CompletionRequest, observer: &mut dyn crate::provider_stream::ModelObserver) -> Result<serde_json::Value, AdapterError> {
        let review = request.system.contains("Teaching delivery acceptance review.");
        let explanation = request.system.contains("Teaching explanation acceptance review.");
        let result = self.complete_structured(request);
        if review { observer.observe(crate::provider_stream::ModelDelta::Usage(crate::provider_stream::ModelUsage {total_tokens:self.reported_review_usage, ..Default::default()})); }
        if explanation { observer.observe(crate::provider_stream::ModelDelta::Usage(crate::provider_stream::ModelUsage {total_tokens:self.reported_explanation_usage, ..Default::default()})); }
        result
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> { self.inner.chat(request) }
    fn chat_observed(&self, request: &AgentRequestPlan, observer: &mut dyn crate::provider_stream::ModelObserver) -> Result<AssistantTurn, AdapterError> { self.inner.chat_observed(request,observer) }
}

fn accepted_review() -> Result<serde_json::Value, AdapterError> { Ok(serde_json::json!({"decision":"accept","revision":null,"learner_claims":[]})) }

#[test]
fn native_board_candidate_is_the_complete_public_delivery() {
    let text = "The catalog keeps B while this route leaves B out.";
    let invitation = "Could another route still use B?";
    let mut selection = selected_body("native", text);
    selection["move"]["board_candidate_revision"] = serde_json::json!(1);
    selection["move"]["response_expectation"] = serde_json::json!({"public_prompt":invitation,"block_refs":["keep-select"]});
    let missing_invitation = proposed_teaching_delivery(&selection, &BTreeMap::new(), true).unwrap_err();
    assert_eq!(missing_invitation.error_code, "BOARD_CANDIDATE_REVISION_REQUIRED");
    assert!(missing_invitation.message.contains("revise"));

    selection["move"]["delivery_text"] = serde_json::json!(format!("{text}\n\n{invitation}"));
    let complete = proposed_teaching_delivery(&selection, &BTreeMap::new(), true).unwrap();
    assert_eq!(complete["text"], selection["move"]["delivery_text"], "A bound board must not receive text after inspection");

    selection["delivery_move_refs"] = serde_json::json!(["selected:earlier"]);
    let accepted = BTreeMap::from([("selected:earlier".into(), selected_body("earlier", "An earlier explanation.")["move"].clone())]);
    let retained = proposed_teaching_delivery(&selection, &accepted, true).unwrap_err();
    assert_eq!(retained.error_code, "BOARD_CANDIDATE_REVISION_REQUIRED");
    assert!(retained.message.contains("revise"));
}

#[test]
fn pending_native_board_exposes_only_the_candidate_selection_contract() {
    let pending = serde_json::json!({"status":"practice","native_board_candidate_pending":true});
    for spec in [crate::tutor::spec_for_context(&pending).unwrap(), crate::tutor::model_spec_for_context(&pending).unwrap()] {
        assert!(spec.parameters["properties"].get("delivery_move_refs").is_none());
        let movement = &spec.parameters["properties"]["move"];
        assert_eq!(movement["required"], serde_json::json!(["board_candidate_revision","interaction_intent"]));
        for field in ["move_id","design_ref","block_refs","target","object_ids","capability","kind","prompt","intent_origin","intent_quote","coach_ref"] {
            assert!(movement["properties"].get(field).is_none(), "The host must bind {field}, rather than asking the model to copy it");
        }
        assert!(movement["properties"].get("participation").is_some(), "The model retains real optional reasoning-allocation choices");
        assert!(movement["properties"].get("delivery_text").is_none(), "A second public body must not be an advertised native input");
        assert!(movement.get("anyOf").is_none());
    }
    let resolved = serde_json::json!({"status":"practice","native_board_candidate_pending":true,"current_action":{
        "action":{"origin":"ui_entry","entry_action":"explain_selection"},"user_supplement":"","accepted_request":"Explain this selection.",
        "answer_ref":null,"answer_text":null,"source_excerpt":null,"source_bindings":[],"target":null,"expected_chat_head":null,"presentation_follow_up":null}});
    for spec in [crate::tutor::spec_for_context(&resolved).unwrap(), crate::tutor::model_spec_for_context(&resolved).unwrap()] {
        assert_eq!(spec.parameters["properties"]["move"]["required"], serde_json::json!(["board_candidate_revision"]));
        assert!(spec.parameters["properties"]["move"]["properties"].get("interaction_intent").is_none());
    }
    let committed = serde_json::json!({"status":"practice","native_board_candidate_pending":false});
    assert!(crate::tutor::model_spec_for_context(&committed).unwrap().parameters["properties"]["move"]["properties"].get("delivery_text").is_some(),
        "After committing a part, ordinary final teaching text is available again");
}

#[test]
fn native_board_revision_feedback_has_bounded_cross_tool_recovery() {
    let mut guard = ProgressPhaseGuard::default();
    guard.observe_error("tutor.step", &serde_json::json!({"error_code":"BOARD_CANDIDATE_REVISION_REQUIRED"}).to_string());
    assert!(guard.recovery_available(), "The author must be allowed to revise the previewed board");
    assert!(guard.argument_correction_tools.contains("board.author"));
    assert!(guard.argument_correction_tools.contains("tutor.step"));
    guard.recovery_used = true;
    guard.observe_error("board.author", &serde_json::json!({"error_code":"BOARD_CANDIDATE_REVISION_REQUIRED"}).to_string());
    assert!(!guard.recovery_available(), "Repeating an invalid revision must not renew recovery indefinitely");
    guard.observe_batch("candidate:1", "candidate:2");
    assert!(!guard.blocks("candidate:2"), "A real replacement candidate resets stalled progress");
}

#[test]
fn native_board_failure_keeps_the_cause_without_internal_correction_schema() {
    let original = ToolError {error_code:"TOOL_ARGUMENTS_INVALID".into(),category:"validation".into(),
        message:"$.responds_to is not allowed\nPlace $.responds_to at $.move.responds_to\nSelected operation schema: {}".into()};
    let final_error = unfinished_native_board(Some("board:run:1"), None, Some(&original)).unwrap();
    assert_eq!(final_error.error_code, original.error_code);
    assert_eq!(final_error.message, "$.responds_to is not allowed 本轮画板尚未保存，任务未完成。");
    assert!(original.message.contains("$.move.responds_to"), "The model's full correction feedback stays available");
}

fn compact_board_selection(selection: &serde_json::Value, revision: u32) -> serde_json::Value {
    let mut compact = serde_json::json!({"operation":"select","move":{"board_candidate_revision":revision,
        "interaction_intent":selection["move"]["interaction_intent"]}});
    for field in ["finish", "delivery_scope"] {
        if let Some(value) = selection.get(field) { compact[field] = value.clone(); }
    }
    if selection["move"]["participation"]["choice"] != "support" {
        compact["move"]["participation"] = selection["move"]["participation"].clone();
    }
    for field in ["response_expectation", "responds_to", "interpretation_refs", "continuation"] {
        if let Some(value) = selection["move"].get(field) { compact["move"][field] = value.clone(); }
    }
    if let Some(expectation) = compact["move"].get_mut("response_expectation").and_then(serde_json::Value::as_object_mut) { expectation.remove("block_refs"); }
    compact
}

#[test]
fn native_board_preview_reaches_teaching_selection_before_any_commit() {
    struct BoardTeachingPort<'a> { inner: TeachingPort<'a>, text: String, deliveries: usize, revision: u32, fail_staging: bool, resolved_intent: Option<&'static str>, actual_request: &'static str }
    impl ResidentStatePort for BoardTeachingPort<'_> {
        fn board_authoring_available(&self) -> bool { true }
        fn author_board(&mut self, request: crate::board_author::AuthorRequest, _: &[SourceBinding], _: &[Message], _: &crate::run_context::CancellationToken) -> Result<crate::board_author::AuthorResult, ToolError> {
            use crate::board_author::{AuthorRequest, AuthorResult};
            let candidate_id = format!("board:run:{}", self.revision);
            match request {
                AuthorRequest::Revise { candidate_revision, .. } if candidate_revision != self.revision =>
                    Err(crate::board_author::error("BOARD_CANDIDATE_CHANGED", "Read the current candidate before revising it.")),
                AuthorRequest::Prepare { blocks, .. } | AuthorRequest::Revise { blocks, .. } => {
                    self.revision += 1;
                    if !blocks.is_empty() {
                        self.text = blocks.iter().map(|b| b.markdown.as_str()).collect::<Vec<_>>().join("\n\n");
                    }
                    let mut result = AuthorResult::value(serde_json::json!({"status":"candidate_prepared",
                        "candidate_id":format!("board:run:{}",self.revision),"candidate_revision":self.revision}));
                    result.invalidates_delivery = true;
                    Ok(result)
                }
                AuthorRequest::Preview { .. } => {
                    let mut result = AuthorResult::value(serde_json::json!({"status":"preview_ready_for_inspection","candidate_id":candidate_id,"candidate_revision":self.revision,
                        "observations":[{"dom":{"text":"B remains in the catalog.","board":{"ready":true}},"issues":[]}]}));
                    result.previewed_candidate = Some(candidate_id.clone()); result.invalidates_delivery = true;
                    result.images.push(crate::presentation_author::PreviewImage {caption:"Actual native diagram".into(),png_base64:"board-png".into(),candidate_id:Some(candidate_id.clone()),environment_name:Some("desktop-content".into())});
                    Ok(result)
                }
                AuthorRequest::Read { .. } => Ok(AuthorResult::value(serde_json::json!({"board":{"revision":1},"committed":[],
                    "candidate":{"board_id":"board","candidate_id":candidate_id,"candidate_revision":self.revision,"base_board_revision":1,"previewed":true,
                        "delivery_text":self.text,"items":[{"item_id":"catalog","readable_content":"B remains in the catalog.","target":{"kind":"drawing","graphic":{"kind":"text","text":"Catalog → B"}}}],"links":[],"routes":[]}}))),
                AuthorRequest::Deliver { .. } => {
                    assert_eq!(self.inner.accepted, ["select"], "selection must precede the actual save");
                    if self.fail_staging { return Err(crate::board_author::error("BOARD_REVISION_CONFLICT", "The board changed after teaching was selected.")); }
                    self.deliveries += 1;
                    let mut result = AuthorResult::value(serde_json::json!({"status":"delivery_staged"}));
                    result.delivery_text = Some(self.text.clone()); Ok(result)
                }
                _ => panic!("unexpected native board operation"),
            }
        }
        fn tutor_step_preflight(&self, request: &serde_json::Value, _: &[SourceBinding], _: &[read_tools::EvidenceRange]) -> Result<serde_json::Value, crate::run_context::TeachingSelectionPreflightError> {
            let mut normalized = request.clone();
            if request["operation"] == "select" {
                let native = &request["native_board_selection"];
                assert_eq!(native["candidate_revision"], self.revision, "Only the actual bound candidate reaches native expansion");
                let movement = &mut normalized["move"];
                movement["move_id"] = serde_json::json!(format!("native:{}:design:current", native["candidate_id"].as_str().unwrap()));
                movement["design_ref"] = serde_json::json!("design:current");
                movement["block_refs"] = design()["focus_block_refs"].clone();
                movement["target"] = design()["target"].clone();
                movement["capability"] = movement["target"]["capability"].clone();
                movement["object_ids"] = serde_json::json!([]);
                movement["kind"] = serde_json::json!("explain");
                movement["prompt"] = movement["target"]["learning_focus"].clone();
                if let Some(intent) = self.resolved_intent {
                    movement["interaction_intent"] = serde_json::json!(intent);
                    movement["intent_origin"] = serde_json::json!("current_action");
                } else if movement["interaction_intent"] == "neutral" {
                    movement["intent_origin"] = serde_json::json!("neutral");
                } else {
                    movement["intent_origin"] = serde_json::json!("current_request");
                    movement["intent_quote"] = serde_json::json!(self.actual_request);
                }
                if movement.get("participation").is_none() {
                    movement["participation"] = serde_json::json!({"choice":"support","reason":"Publish the inspected support for the current accepted focus.",
                        "support":design()["organizing_model"],"learner_inference":null});
                }
                if movement["response_expectation"].is_object() {
                    movement["response_expectation"]["block_refs"] = design()["focus_block_refs"].clone();
                }
                for binding in normalized["move"]["target"]["source_bindings"].as_array_mut().into_iter().flatten() {
                    binding["source_id"] = serde_json::json!("source:fixture");
                    binding["source_revision"] = serde_json::json!("revision:fixture");
                }
                serde_json::from_value::<crate::tutor::TeachingMove>(normalized["move"].clone()).map_err(|error|
                    crate::run_context::TeachingSelectionPreflightError {error:memory::teaching::invalid(error.to_string()),missing_source_read:None})?;
                normalized.as_object_mut().unwrap().remove("native_board_selection");
            }
            Ok(normalized)
        }
        fn tutor_step(&mut self, request: serde_json::Value, evidence: &[SourceBinding], ranges: &[read_tools::EvidenceRange]) -> Result<serde_json::Value, ToolError> {
            if request["operation"] == "select" {
                serde_json::from_value::<crate::tutor::TeachingMove>(request["move"].clone()).expect("The real typed host rejects runtime-only native reference fields");
            }
            self.inner.tutor_step(request, evidence, ranges)
        }
        fn tutor_record_review(&mut self, record: serde_json::Value) -> Result<serde_json::Value, ToolError> { self.inner.tutor_record_review(record) }
        fn submit_private<R>(&mut self, operation: impl FnOnce(&mut MemoryStore) -> R) -> Result<R, ToolError> { self.inner.submit_private(operation) }
        fn read_live_reader<R>(&mut self, operation: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> { self.inner.read_live_reader(operation) }
        fn apply_reader<R>(&mut self, operation: impl FnOnce(&mut MemoryStore, &mut Reader) -> R) -> Result<R, ToolError> { self.inner.apply_reader(operation) }
        fn reader_input(&mut self, book: &Book, question: &str) -> crate::run_context::ReaderInputSnapshot { self.inner.reader_input(book, question) }
    }
    // The real incident first omitted the body, then selected a different body;
    // its subsequent prepare was rejected as no progress. Recover that sequence.
    for mode in ["presentation_reference", "reference", "resolved_reference", "guided_request", "repair", "explicit", "conflict", "invitation_revision", "changed_after_selection", "failed_after_selection", "unselected_part_commit", "misplaced_responds_to", "misplaced_invitation", "grace_accept", "grace_wrong_call"] {
    let b = book(); let mut store = MemoryStore::open(tmp(&format!("native-board-teaching-review-{mode}"))).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS); let snapshot = default_profile_snapshot(&b, &store, "t0");
    let plain_text = "The catalog retains B even when this route does not select B.";
    let invitation = "Could another route still use B?";
    let text = if matches!(mode, "invitation_revision" | "guided_request" | "misplaced_invitation") { format!("{plain_text}\n\n{invitation}") } else { plain_text.into() };
    let actual_request = if mode == "guided_request" {"Explain only the prerequisite, then let me try first."} else {"Explain why B remains available and draw it on the board."};
    let mut selection = selected_body("board", &text);
    if matches!(mode, "invitation_revision" | "guided_request" | "misplaced_invitation") {
        selection["move"]["response_expectation"] = serde_json::json!({"public_prompt":invitation,"block_refs":["keep-select"]});
        selection["move"]["participation"] = serde_json::json!({"choice":"invite","reason":"Leave the changed-route application for the learner.",
            "support":"The board supplies catalog retention.","learner_inference":"Judge whether another route can use B."});
        if mode == "guided_request" { selection["move"]["interaction_intent"] = serde_json::json!("guided_inquiry"); }
    }
    if mode == "unselected_part_commit" {
        selection["delivery_scope"] = serde_json::json!({"kind":"board_part","relation":"The catalog retains B.","remaining_request":"Explain how a different route selects it."});
    }
    let mut chats = vec![turn_calls(vec![call("preview", "board.author", r#"{"operation":"preview","candidate_revision":1}"#)])];
    let revision = if matches!(mode, "repair" | "invitation_revision") {
        if mode == "repair" {
            let mut missing = selection.clone(); missing["move"].as_object_mut().unwrap().remove("delivery_text");
            chats.push(turn_calls(vec![call("missing-body", "tutor.step", &missing.to_string())]));
            chats.push(turn_calls(vec![call("different-body", "tutor.step", &selected_body("board", "A changed explanation.").to_string())]));
        } else {
            let first = compact_board_selection(&selection, 1);
            chats.push(turn_calls(vec![call("review-first-board", "tutor.step", &first.to_string())]));
        }
        chats.push(turn_calls(vec![call("repair", "board.author", &serde_json::json!({"operation":"revise","candidate_revision":1,
            "blocks":[{"block_id":"retained-card","markdown":text}],"drawings":[]}).to_string())]));
        chats.push(turn_calls(vec![call("preview-repair", "board.author", r#"{"operation":"preview","candidate_revision":2}"#)]));
        2
    } else { 1 };
    if mode == "explicit" {
        chats.push(turn_calls(vec![call("forbidden-text", "tutor.step", &selection.to_string())]));
    }
    if matches!(mode, "presentation_reference" | "reference" | "conflict" | "grace_accept") { selection["finish"] = serde_json::json!(true); }
    selection = compact_board_selection(&selection, revision);
    if mode == "resolved_reference" { selection["move"].as_object_mut().unwrap().remove("interaction_intent"); }
    if matches!(mode, "misplaced_responds_to" | "misplaced_invitation") {
        selection["finish"] = serde_json::json!(true);
        selection["move"]["responds_to"] = serde_json::json!({"fact_refs":["usage:before"],"disposition":"addressed"});
        let mut misplaced = selection.clone();
        for field in ["responds_to", "participation", "response_expectation"] {
            if let Some(value) = misplaced["move"].as_object_mut().unwrap().remove(field) {
                misplaced[field] = value;
            }
        }
        chats.push(turn_calls(vec![call("misplaced-fields", "tutor.step", &misplaced.to_string())]));
    }
    if mode == "grace_wrong_call" {
        chats.push(turn_calls(vec![call("wrong-grace-tool", "board.author", r#"{"operation":"deliver","candidate_revision":1}"#)]));
    } else {
        chats.push(turn_calls(vec![call("select", "tutor.step", &selection.to_string())]));
    }
    if matches!(mode, "changed_after_selection" | "failed_after_selection") {
        chats.push(turn_calls(vec![call("changed-board", "board.author", &serde_json::json!({"operation":"revise","candidate_revision":if mode == "failed_after_selection" {999} else {1},
            "blocks":[{"block_id":"retained-card","markdown":"A replacement which has not been selected."}]}).to_string())]));
    }
    if mode == "unselected_part_commit" {
        chats.push(turn_calls(vec![call("change-diagram-only", "board.author", &serde_json::json!({"operation":"revise","candidate_revision":1,
            "drawings":[{"item_id":"catalog","title":"Revised catalog diagram","position":{"x":0,"y":0},"size":{"width":200,"height":100},"graphic":{"kind":"text","text":"Changed relation"}}]}).to_string())]));
        chats.push(turn_calls(vec![call("preview-changed-diagram", "board.author", r#"{"operation":"preview","candidate_revision":2}"#)]));
        chats.push(turn_calls(vec![call("commit-unselected-diagram", "board.author", r#"{"operation":"commit","candidate_revision":2}"#)]));
    }
    chats.push(turn_final(&text));
    if mode == "conflict" { chats.push(turn_final(&text)); }
    let adapter = ReviewedTeachingAdapter::new(chats, vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig {
        max_turns: mode.starts_with("grace_").then_some(1), ..Default::default()
    }, adapter.model_runtime_profile());
    let mut current_design = design();
    if mode == "presentation_reference" {
        current_design["representation"] = serde_json::json!({"kind":"presentation",
            "reason":"Keep the catalog and route visible together.","observation":"B stays in the catalog while the route changes."});
    }
    context.tutor = Some(serde_json::json!({"status":"practice","current_design":{"design_ref":"design:current","design":current_design},"learner_context":{}}));
    if mode == "resolved_reference" {
        context.tutor.as_mut().unwrap()["current_action"] = serde_json::json!({"action":{"origin":"ui_entry","entry_action":"explain_selection"},
            "user_supplement":"","accepted_request":"Explain this selection.","answer_ref":null,"answer_text":null,"source_excerpt":null,
            "source_bindings":[],"target":null,"expected_chat_head":null,"presentation_follow_up":null});
    }
    let mut port = BoardTeachingPort {inner:TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},
        accepted:vec![],selections:Default::default(),reviews:vec![]},text:if mode == "invitation_revision" {plain_text.into()} else { text.clone() },deliveries:0,revision:1,fail_staging:mode == "conflict",resolved_intent:if mode == "resolved_reference" {Some("direct_explanation")} else {None},actual_request};
    let outcome = run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),actual_request,"t0");
    if matches!(mode, "grace_wrong_call") {
        let outcome = outcome.unwrap();
        assert!(outcome.incomplete, "The existing text fallback cannot complete a rejected board");
        assert_eq!(outcome.warning.as_deref(), Some("BOARD_DELIVERY_GRACE_REQUIRED"));
        assert_eq!(port.deliveries, 0, "Reserved delivery must not publish an unaccepted candidate");
        assert!(port.inner.accepted.is_empty() && context.board_delivery_draft.is_none());
        assert!(adapter.seen_reviews.borrow().is_empty());
        let plans = adapter.inner.0.seen_plans.borrow();
        assert_eq!(plans.len(), 3, "The failed reserved selection permits only the existing tool-free final fallback");
        assert!(plans[2].tools.is_empty(), "Fallback must not renew the authoring budget");
        continue;
    }
    if mode == "conflict" {
        let error = outcome.unwrap_err();
        assert_eq!(error.error_code, "BOARD_REVISION_CONFLICT");
        assert!(error.message.contains("未完成"));
        assert_eq!(port.deliveries, 0);
        continue;
    }
    if mode == "changed_after_selection" {
        assert_eq!(outcome.unwrap_err().error_code, "BOARD_DELIVERY_REQUIRED");
        assert!(context.board_delivery_draft.is_none(), "Changing the accepted candidate must invalidate staging");
        assert_eq!(port.deliveries, 1);
        continue;
    }
    if mode == "failed_after_selection" {
        assert_eq!(outcome.unwrap_err().error_code, "BOARD_CANDIDATE_CHANGED");
        assert_eq!(port.deliveries, 1, "An earlier stage must not hide a failed required revision");
        continue;
    }
    if mode == "unselected_part_commit" {
        assert_eq!(outcome.unwrap_err().error_code, "BOARD_TEACHING_SELECTION_REQUIRED");
        assert_eq!(port.deliveries, 0, "A changed diagram must not reuse an older part's selected text");
        assert_eq!(port.text, text, "This regression changes only the diagram, keeping the accepted body identical");
        continue;
    }
    let outcome = outcome.unwrap();
    assert!(!outcome.incomplete, "{:?}", outcome.warning);
    assert_eq!(port.deliveries, 1);
    assert!(adapter.seen_reviews.borrow().is_empty() && port.inner.reviews.is_empty());
    if mode == "grace_accept" {
        assert_eq!(adapter.inner.0.seen_plans.borrow().len(), 2, "Preview gets exactly one reserved selection");
        assert_eq!(port.inner.accepted, ["select"]);
        assert_eq!(outcome.turns, 2, "one preview sampling and one reserved selection");
    }
    if matches!(mode, "misplaced_responds_to" | "misplaced_invitation") {
        let feedback = tool_result(&context.messages, "misplaced-fields");
        assert_eq!(tool_result_error_code(&feedback).as_deref(), Some("TOOL_ARGUMENTS_INVALID"));
        assert!(feedback.contains("$.move.responds_to"), "The rejected call must identify the correct field location: {feedback}");
        if mode == "misplaced_invitation" {
            for field in ["participation", "response_expectation"] {
                assert!(feedback.contains(&format!("$.move.{field}")), "One correction must cover all misplaced teaching fields: {feedback}");
            }
        }
        let plans = adapter.inner.0.seen_plans.borrow();
        let correction = serde_json::to_string(&plans[2].ordered_messages()).unwrap();
        assert!(correction.contains("$.move.responds_to"), "The model must consume the correction before retrying");
        assert_eq!(port.inner.accepted, ["select"], "The invalid call must never execute or save");
        assert_eq!(port.revision, 1, "Correcting metadata must reuse the inspected candidate");
        assert_eq!(port.inner.selections[&outcome.teaching_delivery_move_refs[0]]["responds_to"], selection["move"]["responds_to"]);
    }
    let inspected=&context.board_review_preview.as_ref().unwrap().report;
    assert_eq!(inspected["candidate_revision"],revision);
    assert_eq!(inspected["observations"][0]["dom"]["text"],"B remains in the catalog.");
    assert!(adapter.inner.0.seen_plans.borrow().iter().any(|plan|
        plan.preview_images.iter().any(|image|image.png_base64=="board-png")),"author must actually observe the board");
    assert!(adapter.seen_explanation_reviews.borrow().is_empty());
    let selected=&port.inner.selections[&outcome.teaching_delivery_move_refs[0]];
    assert_eq!(selected["delivery_text"],text);
    assert_eq!(selected["block_refs"],design()["focus_block_refs"]);
    assert_eq!(selected["prompt"],design()["target"]["learning_focus"]);
    if mode=="guided_request" {
        assert_eq!(selected["interaction_intent"],"guided_inquiry");
        assert_eq!(selected["participation"]["choice"],"invite");
        assert_eq!(selected["intent_quote"],actual_request);
    }
    assert_eq!(outcome.answer.as_deref(), Some(text.as_str()));
    }
}

fn exam_workspace(calendar_revision: u32) -> crate::workspace_goal::WorkspaceGoalContext {
    crate::workspace_goal::WorkspaceGoalContext {
        learning_workspace_id:"exam:private".into(),
        goal:base_schema::WorkspaceGoal {revision:3,text:"Understand the catalog and route selection.".into(),
            constraints:vec!["Only route selection is on this exam; catalog maintenance is excluded.".into()],created_at:0},
        exam_calendar:Some(serde_json::json!({"workspace_goal_revision":calendar_revision,"current_local_date":"2026-10-21",
            "exam_date":"2026-10-22","remaining_days":1})),
    }
}

#[test]
fn teaching_writing_rules_reach_each_enabled_main_request_once() {
    let b = book();
    let profile = ModelRuntimeProfile::fallback("shared-writing", ProviderToolProtocol::Native);
    let writing = include_str!("tutor_writing_rules.md");
    for status in ["practice", "observing", "reference", "paused", "inactive"] {
        let tutor = serde_json::json!({"status":status,"learner_context":{"current_request":"Explain this relation."}});
        let registry = resident_tool_registry().with_tutor_context(Some(&tutor));
        let mut exposure = ToolExposureState::default();
        exposure.activate_tutor();
        let messages = vec![Message::system(crate::tutor::instructions(&tutor)), Message::user("Explain this relation.")];
        let (_, plan) = build_sample_request(&messages, &mut ContextFragmentLedger::default(), &b,
            &mut ActiveToolResultLedger::default(), None, COMPACTION_CONSUMPTION_WRAPPER, &registry, &profile,
            ToolPermissions::default(), &exposure, ArtifactExposureContext::no_overlay(), EvidenceState::UserProvided,
            &[], None, false, Some(&tutor), None, None, None).unwrap();
        let enabled = usize::from(matches!(status, "practice" | "observing" | "reference"));
        let occurrences: usize = plan.ordered_messages().iter().filter_map(|message|message.content.as_deref())
            .map(|content|content.matches(writing).count()).sum();
        assert_eq!(occurrences, enabled, "{status}: the shared body must reach the actual main request once");
        assert_eq!(plan.instruction_assets.iter().filter(|asset|asset.asset_id == "resident-agent.tutor.writing").count(), enabled);
        let explanation_occurrences: usize = plan.ordered_messages().iter().filter_map(|message|message.content.as_deref())
            .map(|content|content.matches(include_str!("tutor_explanation_principle.md")).count()).sum();
        assert_eq!(explanation_occurrences, 1, "{status}: the explanation responsibility must reach ordinary and active requests once");
        assert_eq!(plan.instruction_assets.iter().filter(|asset|asset.asset_id == "resident-agent.teaching.explanation").count(), 1);
        let starting_point_occurrences: usize = plan.ordered_messages().iter().filter_map(|message|message.content.as_deref())
            .map(|content|content.matches(crate::agent_prompt::LEARNER_STARTING_POINT).count()).sum();
        assert_eq!(starting_point_occurrences, 1, "{status}: the learner starting point is independent of Tutor activation");
    }
}

#[test]
fn teaching_review_receives_the_same_single_shared_writing_rules() {
    let adapter = ReviewedTeachingAdapter::new(vec![], vec![accepted_review()]);
    let packet = serde_json::json!({"original_request":"Explain this relation.",
        "proposed_delivery":{"text":"The selected material remains in the retained collection."}});
    crate::tutor::review_delivery(&adapter,
        ModelRuntimeProfile::fallback("shared-writing-review", ProviderToolProtocol::Native), packet.clone(), Vec::new()).unwrap();
    let captured = adapter.seen_review_instructions.borrow();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0], crate::tutor::review_instructions());
    assert_eq!(captured[0].matches(include_str!("tutor_review_rules.md")).count(), 1);
    assert_eq!(captured[0].matches(include_str!("tutor_writing_rules.md")).count(), 1);
    assert_eq!(captured[0].matches(include_str!("tutor_review_examples.md")).count(), 1);
    assert_eq!(captured[0].matches(include_str!("tutor_explanation_principle.md")).count(), 1);
    assert_eq!(adapter.seen_reviews.borrow().as_slice(), &[packet]);
    assert!(adapter.seen_review_images.borrow()[0].is_empty());
}

#[test]
fn text_review_checks_sources_and_preserves_verified_work_without_author_teaching_claims() {
    let source = serde_json::json!([{"source_id":"book","source_revision":"current",
        "segments":[{"lid":"1.1","text":"The retained collection contains all accepted items."}]}]);
    let work = serde_json::json!({"understanding":[{"statement":"Accepted items remain in the collection.",
        "sources":[{"source_id":"book","source_revision":"current","lid":"1.1",
            "quote":"contains all accepted items"}]}],"open_questions":["Explain how the next selection uses the retained collection."]});
    let mut verdict = accepted_review().unwrap();
    verdict["agent_work"] = work.clone();
    let adapter = ReviewedTeachingAdapter::new(vec![], vec![Ok(verdict)]);
    let packet = serde_json::json!({"original_request":"Continue explaining.",
        "frozen_input":{"has_board_context":true,"learning_purpose":"understand","user_intent":"Explain how the retained collection is used."},
        "current_design":{"design_ref":"d1","design":{"target":{"expected_performance":"Follow an item into the next selection."},
            "organizing_model":"Trace one retained item through selection.","blocks":[{"block_id":"b1","relation":"retention supports later selection"}]}},
        "proposed_delivery":{"text":"The selected material remains in the retained collection.",
            "moves":[{"target":{"source_bindings":[]},"block_refs":["b1"]}]},
        "current_source_evidence":source});
    let (mut review, _) = crate::tutor::review_delivery(&adapter,
        ModelRuntimeProfile::fallback("deepseek-v4-flash", ProviderToolProtocol::Native), packet, Vec::new()).unwrap();
    let captured = &adapter.seen_reviews.borrow()[0];
    assert_eq!(captured["frozen_input"]["learning_purpose"], "understand");
    assert_eq!(captured["frozen_input"]["has_board_context"], true,
        "direct board text keeps its board scope in the actual factual review request");
    assert!(adapter.seen_review_instructions.borrow()[0].contains("读过原文或调用 source.present 不能代替正文实际附引用"));
    assert!(captured.get("local_design").is_none(), "the factual reviewer must not inherit the author's claim of having explained the relationship");
    assert_eq!(adapter.seen_review_instructions.borrow()[0], crate::tutor::text_review_instructions());
    assert_eq!(adapter.seen_review_instructions.borrow()[0].matches(include_str!("tutor_explanation_principle.md")).count(), 0,
        "explanation sufficiency is judged by the isolated explanation reviewer");
    assert!(crate::tutor::retain_grounded_review_work(&mut review, &source).is_empty());
    assert_eq!(review["agent_work"], work, "text review must retain grounded knowledge and unfinished explanation work");
}

#[test]
fn teaching_review_uses_low_reasoning_only_for_deepseek() {
    for (model, expected_effort) in [("deepseek-v4-pro", Some("low")), ("review-other-model", None)] {
        let adapter = ReviewedTeachingAdapter::new(vec![], vec![accepted_review()]);
        let packet = serde_json::json!({"original_request":"Explain this relation.",
            "proposed_delivery":{"text":"The selected material remains in the retained collection."}});
        crate::tutor::review_delivery(&adapter,
            ModelRuntimeProfile::fallback(model, ProviderToolProtocol::Native), packet.clone(), Vec::new()).unwrap();
        assert_eq!(adapter.seen_review_reasoning_efforts.borrow().as_slice(), &[expected_effort.map(str::to_owned)], "{model}");
        assert_eq!(adapter.seen_reviews.borrow().as_slice(), &[packet]);
        assert_eq!(adapter.seen_review_instructions.borrow()[0], crate::tutor::review_instructions());
    }
}

#[test]
fn teaching_review_screenshot_budget_preserves_visible_omissions() {
    let adapter = ReviewedTeachingAdapter::new(vec![], vec![accepted_review()]);
    let packet = serde_json::json!({"original_request":"Explain this relation.",
        "proposed_delivery":{"text":"B remains while the selected route changes."}});
    let mut profile = ModelRuntimeProfile::fallback("review-image-budget", ProviderToolProtocol::Native);
    profile.output_reserve_tokens = 32_768;
    let messages = vec![Message::system(crate::tutor::review_instructions()), Message::user(packet.to_string())];
    let base = AgentRequestPlan::for_ad_hoc(profile.clone(), &messages, &[]);
    profile.context_window_tokens = base.active_context.estimated_input_tokens
        + profile.output_reserve_tokens + profile.safety_margin_tokens + 4096 + 1024;
    let images = (1..=3).map(|index| crate::presentation_author::PreviewImage {
        caption:format!("Actual viewport {index}"), png_base64:format!("png-{index}"),
        candidate_id:Some("candidate".into()), environment_name:Some(format!("view-{index}")),
    }).collect();
    crate::tutor::review_delivery(&adapter, profile, packet, images).unwrap();
    let seen = adapter.seen_review_images.borrow();
    assert_eq!(seen[0].len(), 1);
    assert_eq!(seen[0][0].png_base64, "png-3");
    let packets = adapter.seen_reviews.borrow();
    let notice = packets[0]["visual_observation_notice"].as_str().unwrap();
    assert!(notice.contains("2 images omitted") && notice.contains("candidate/view-1") && notice.contains("candidate/view-2"));
    assert!(!packets[0].to_string().contains("png-"), "PNG payloads belong in image blocks, not textual evidence");
}

#[test]
fn teaching_review_format_repair_keeps_frozen_text_and_image_evidence() {
    let adapter = ReviewedTeachingAdapter::new(vec![], vec![accepted_review(), accepted_review()]);
    let packet = serde_json::json!({"original_request":"Explain this relation.",
        "proposed_delivery":{"text":"B remains while the selected route changes."}});
    let images = vec![crate::presentation_author::PreviewImage {
        caption:"The actual changed route".into(), png_base64:"png-observation".into(),
        candidate_id:Some("candidate".into()), environment_name:Some("desktop".into()),
    }];
    let prepared = crate::tutor::prepare_delivery_review(adapter.model_runtime_profile(), packet, images, false);
    prepared.complete(&adapter, None).unwrap();
    let repair = serde_json::json!({"validation_error":"The answer quote is not exact.",
        "previous_review":{"decision":"accept"},"instruction":"Repair your review using the unchanged evidence."});
    prepared.complete(&adapter, Some(&repair)).unwrap();
    let requests = adapter.seen_review_requests.borrow();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].system, requests[1].system);
    assert_eq!(requests[0].user, requests[1].user);
    assert_eq!(requests[0].followup_user, None);
    assert_eq!(requests[1].followup_user.as_deref(), Some(format!("review_format_repair:\n{repair}").as_str()));
    assert_eq!(serde_json::to_value(&requests[0].images).unwrap(), serde_json::to_value(&requests[1].images).unwrap());
    assert_eq!(requests[0].images.len(), 1);
    let native = requests.iter().map(|request| crate::native_completion_request_projection(
        "fixture", &request.system, request)).collect::<Vec<_>>();
    let messages = requests.iter().map(|request| crate::deepseek_messages::completion_projection(
        "deepseek-flash", request).unwrap()).collect::<Vec<_>>();
    for (protocol, bodies) in [("chat_completions", native), ("messages", messages)] {
        let original = bodies[0]["messages"].as_array().unwrap();
        let repaired = bodies[1]["messages"].as_array().unwrap();
        assert_eq!(repaired.len(), original.len() + 1);
        assert_eq!(&repaired[..original.len()], original.as_slice(),
            "{protocol}: text, image captions and image blocks retain their exact wire positions");
        assert_eq!(serde_json::to_vec(original).unwrap(), serde_json::to_vec(&repaired[..original.len()]).unwrap());
        assert_eq!(bodies[0]["system"], bodies[1]["system"]);
        assert!(repaired.last().unwrap().to_string().contains("review_format_repair"));
        let evidence = &original.last().unwrap()["content"];
        assert!(evidence.as_array().unwrap().iter().any(|block|
            matches!(block["type"].as_str(), Some("image" | "image_url"))),
            "{protocol}: the checked prefix must contain an actual image block");
        let mut tracker = crate::request_diagnostics::RequestTracker::default();
        tracker.observe("review", 1, std::sync::Arc::new(bodies[0].clone()));
        let diagnostics = tracker.observe("review", 2, std::sync::Arc::new(bodies[1].clone()));
        assert_eq!(diagnostics.messages_append_only, Some(true));
        assert_eq!(diagnostics.unchanged_prefix_messages, Some(original.len() as u32));
        assert_eq!((diagnostics.previous_image_count, diagnostics.image_count), (Some(1), 1));
        assert_eq!(diagnostics.first_changed_message.unwrap().fields,
            vec![crate::request_diagnostics::MessageChangeField::Added]);
    }
}

#[test]
fn teaching_review_format_repair_budget_cannot_drop_evidence_or_send_an_oversized_request() {
    let adapter = ReviewedTeachingAdapter::new(vec![], vec![accepted_review()]);
    let packet = serde_json::json!({"original_request":"Explain this relation.",
        "proposed_delivery":{"text":"B remains while the selected route changes."}});
    let mut profile = adapter.model_runtime_profile();
    let messages = vec![Message::system(crate::tutor::review_instructions()), Message::user(packet.to_string())];
    let base = AgentRequestPlan::for_ad_hoc(profile.clone(), &messages, &[]);
    profile.context_window_tokens = base.active_context.estimated_input_tokens
        + profile.output_reserve_tokens + profile.safety_margin_tokens + 4096 + 512;
    let images = vec![crate::presentation_author::PreviewImage {caption:"Essential observation".into(),
        png_base64:"png-kept".into(),candidate_id:Some("candidate".into()),environment_name:Some("desktop".into())}];
    let prepared = crate::tutor::prepare_delivery_review(profile, packet, images, false);
    prepared.complete(&adapter, None).unwrap();
    let failure = prepared.complete(&adapter, Some(&serde_json::json!({"validation_error":"large diagnostic ".repeat(4000)}))).unwrap_err();
    assert!(failure.error.message.contains("exceeds model context budget"));
    assert_eq!(failure.reported_usage, None);
    assert_eq!(adapter.seen_review_requests.borrow().len(), 1, "the failed admission sends no second request");
    assert_eq!(adapter.seen_review_requests.borrow()[0].images.len(), 1, "evidence selected for the first attempt remains frozen");
}

#[test]
fn teaching_review_examples_cannot_supply_current_answer_quotes() {
    let packet = serde_json::json!({"original_request":"Explain the current relation.",
        "proposed_delivery":{"text":"B remains while the selected route changes."}});
    let mut review = review_fixture(accepted_review().unwrap(), &packet);
    review["request_resolution"]["answer_quotes"] = serde_json::json!(["模型 45 秒＋其他 15 秒"]);
    assert!(crate::tutor::normalize_delivery_review(&mut review, &packet).is_err(),
        "a calibration example is not evidence from the actual delivery");
}

// Supply the current review contract for fixtures whose subject is another
// part of the teaching loop. Explicit contract fields are never overwritten.
pub(super) fn review_fixture(mut review: serde_json::Value, packet: &serde_json::Value) -> serde_json::Value {
    let question = packet["original_request"].as_str()
        .or_else(||packet["frozen_input"]["current_request"].as_str())
        .or_else(||packet["frozen_input"]["learner_request"].as_str())
        .or_else(||packet["frozen_input"]["current_action"]["accepted_request"].as_str())
        .unwrap();
    if review.get("request_resolution").is_none() {
        review["request_resolution"] = serde_json::json!({
            "status":if review["decision"]=="revise" {"unresolved"} else {"answered"},
            "question_quote":question,
            "reason":review["revision"].as_str().unwrap_or("The supplied example answers the fixture's current request.")});
    }
    if let Some(resolution)=review["request_resolution"].as_object_mut() {
        let unresolved=resolution.get("status").is_some_and(|status|status=="unresolved");
        let delivered=packet["proposed_delivery"]["text"].as_str().unwrap_or_default();
        resolution.entry("requested_relation").or_insert_with(||serde_json::json!(format!("Explain the relationship requested by: {question}")));
        resolution.entry("delivered_relation").or_insert_with(||serde_json::json!(if unresolved {
            "The fixture draft still lacks the relationship identified in the revision."
        } else { delivered }));
        resolution.entry("answer_quotes").or_insert_with(||if unresolved {serde_json::json!([])}else{serde_json::json!([delivered])});
    }
    if review.get("findings").is_none() { review["findings"] = serde_json::json!([]); }
    if review.get("issue_updates").is_none() {
        if let Some(issues)=packet["open_issues"].as_array().filter(|issues|!issues.is_empty()) {
            let status=if review["decision"]=="revise" {"unresolved"} else {"resolved"};
            review["issue_updates"]=serde_json::json!(issues.iter().map(|issue|serde_json::json!({
                "issue_ref":issue["issue_ref"],"status":status,
                "reason":"The fixture reviewer explicitly carries or resolves this prior issue for the supplied decision."
            })).collect::<Vec<_>>());
        }
    }
    if review.get("revision_comparison").is_none() {
        if let Some(reference) = packet["revision_reference"]["text"].as_str().filter(|text| !text.trim().is_empty()) {
            review["revision_comparison"] = serde_json::json!({"reference_quotes":[reference],
                "answer_quotes":[packet["proposed_delivery"]["text"]],"lost_relation":null,
                "reason":"The fixture preserves the original explanatory role; its scripted request disposition controls other gaps."});
        }
    }
    review
}

#[test]
fn teaching_review_fixture_fills_only_missing_relation_fields() {
    let packet=serde_json::json!({"original_request":"Explain the interval.",
        "frozen_input":{"current_request":"An older request still in the fixture's learning context."},
        "proposed_delivery":{"text":"The end time minus the start time is the interval."}});
    let explicit=serde_json::json!({"decision":"accept","revision":null,"findings":[],
        "request_resolution":{"status":"unresolved","question_quote":"Explain the interval.","reason":"The actual answer is missing.",
            "requested_relation":"ORIGINAL_REQUESTED_RELATION","delivered_relation":"ORIGINAL_DELIVERED_RELATION","answer_quotes":[]}});
    assert_eq!(review_fixture(explicit.clone(),&packet),explicit);
    let filled=review_fixture(serde_json::json!({"decision":"accept","revision":null}),&packet);
    assert_eq!(filled["request_resolution"]["answer_quotes"],serde_json::json!([packet["proposed_delivery"]["text"]]));
    assert_eq!(filled["request_resolution"]["question_quote"],packet["original_request"]);
    assert!(filled["request_resolution"]["requested_relation"].as_str().unwrap().contains("Explain the interval."));
}

#[test]
fn teaching_grounding_keeps_unknown_teachable_and_acknowledges_actual_confirmation() {
    let grounding = serde_json::json!({"expressions":[{"fact_ref":"user:confirmation","text":"So the input is a prebuilt intermediate representation, right?"}],"interpretations":[]});
    let confirmed = serde_json::json!({"decision":"accept","revision":null,"learner_claims":[{
        "claim":"The learner connects the vector input with the intermediate representation.",
        "basis":[{"fact_ref":"user:confirmation","quote":"the input is a prebuilt intermediate representation"}]}]});
    crate::tutor::validate_review_grounding(&confirmed,&grounding).unwrap();
    crate::tutor::validate_review_grounding(&accepted_review().unwrap(),&serde_json::json!({"expressions":[]})).unwrap();
    let view = crate::tutor::learner_view(&serde_json::json!({"learner_context":{
        "entries":[{"state":"mastered"}],"interpretations":[{"interpretation":"Already understands because the page was delivered."}],
        "learner_grounding":grounding}}));
    assert!(view.get("entries").is_none() && view.get("interpretations").is_none());
    assert_eq!(view["learner_grounding"],grounding,"the shared original-word basis replaces a competing aggregate learner profile");
}

#[test]
fn teaching_grounding_accepts_original_user_corrections_without_treating_interpretations_as_words() {
    let grounding = serde_json::json!({"expressions":[{"kind":"learner_correction","evidence_ref":"correction:user",
        "text":"I copied an earlier worked example."}],
        "interpretations":[{"evidence_ref":"interpretation:teacher","interpretation":"The learner understands this independently."}]});
    let mut review = serde_json::json!({"decision":"accept","revision":null,"learner_claims":[{
        "claim":"The learner says the earlier answer was copied.","basis":[{
            "evidence_ref":"correction:user","quote":"I copied an earlier worked example"}]}]});
    crate::tutor::validate_review_grounding(&review,&grounding).unwrap();
    review["learner_claims"][0]["basis"][0] = serde_json::json!({
        "fact_ref":"correction:user","quote":"I copied an earlier worked example"});
    assert!(crate::tutor::validate_review_grounding(&review,&grounding).is_err());
    review["learner_claims"][0]["basis"][0] = serde_json::json!({
        "evidence_ref":"interpretation:teacher","quote":"The learner understands this independently."});
    assert!(crate::tutor::validate_review_grounding(&review,&grounding).is_err());
}

fn selected_body(id: &str, text: &str) -> serde_json::Value {
    let mut selection: serde_json::Value = serde_json::from_str(&select_args()).unwrap();
    selection["move"]["move_id"] = serde_json::json!(id);
    selection["move"]["delivery_text"] = serde_json::json!(text);
    selection
}
#[derive(Default)]
struct TeachingStreamSink(std::sync::Mutex<Vec<crate::answer_stream::AnswerPatch>>);
impl crate::run_events::RunEventSink for TeachingStreamSink {
    fn emit(&self, _: crate::run_events::RuntimeEvent) {}
    fn answer_patch(&self, patch: crate::answer_stream::AnswerPatch) { self.0.lock().unwrap().push(patch); }
}

fn design() -> serde_json::Value {
    serde_json::json!({"target":{"learning_focus":"Keep and select","expected_performance":"Distinguish retained material from a selected route","capability":"explanation","source_bindings":[{"start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]},
        "representation":{"kind":"text","reason":"A concrete card comparison is enough for this request."},
        "organizing_model":"A retained card can serve different selections.","blocks":[{"block_id":"keep-select","relation":"Not selected does not imply deleted.","purpose":"Find material for another purpose.","source_bindings":[{"start_lid":"1.1","end_lid":"1.1"}]}],
        "focus_block_refs":["keep-select"],"revision_reason":"The learner asks whether an omitted card still exists."})
}
fn select_args() -> String {
    serde_json::json!({"operation":"select","move":{"move_id":"feedback","design_ref":"design:current","block_refs":["keep-select"],"object_ids":[],"target":design()["target"],"capability":"explanation","kind":"explain","interaction_intent":"neutral","intent_origin":"neutral",
        "participation":{"choice":"support","reason":"The learner asks whether omitted material remains available; make retention distinct from route selection.","support":"Retaining a card and selecting it for a route are separate decisions.","learner_inference":null},
        "delivery_text":"保留材料和为一条路线选用材料是两个决定。",
        "prompt":"The same card remains in the retained set while this route omits it."}}).to_string()
}

#[test]
fn application_repeated_source_read_returns_to_design_and_selected_card() {
    // Reader trace: an answer continuation, an actual paragraph read and a repeat
    // previously fell through to available_answer.v2 with the grading key in prose.
    let source = "是鸟也，海运则将徙于南冥。";
    let mut base = sample_base();
    for node in &mut base.lid_nodes { node.span.start = 0; node.span.end = source.chars().count(); }
    let book = Book::new(base,source);
    let mut store = MemoryStore::open(tmp("application-source-read-recovery")).unwrap();
    let mut reader = Reader::new(&book,DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&book,&store,"t0");
    let prior = "鹏的迁徙需要海动风起的条件。";
    let case = "湖边的鹭群每年等浅滩露出后才迁到岛上。今年浅滩还在水下。";
    let mut design = design();
    design["target"]["learning_focus"] = serde_json::json!("行动所需的外部条件");
    let selection = serde_json::json!({"operation":"select","application":{"situation":case,
        "question":"你预计鹭群接下来会怎样行动？","options":["继续等待","立即迁徙"],"correct_option":1,
        "solution":"应等待浅滩露出。","hint":"留意水位。"}});
    let args = r#"{"lid":"1.1"}"#;
    let adapter = ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("read","book.text",args)]), turn_calls(vec![call("repeat","book.text",args)]),
        turn_calls(vec![call("plan","tutor.step",&serde_json::json!({"operation":"design","operation_id":"lesson","design":design}).to_string())]),
        turn_calls(vec![call("select","tutor.step",&selection.to_string())]),
    ],vec![]);
    let mut context = RunContext::new(new_session(),OuterConfig{max_turns:Some(8),..Default::default()},adapter.model_runtime_profile());
    context.tool_exposure_state.activate("book.text");
    context.tutor = Some(serde_json::json!({"status":"practice","learning_purpose":"practice","application_requested":true,
        "learner_context":{"current_request":"根据这段回答给我一道应用题。","previous_answer":prior,
            "current_target":{"source_bindings":[{"source_id":book.base.book_id,"source_revision":book.source_fingerprint(),"start_lid":"1.1","end_lid":"1.1"}]}}}));
    let mut port = TeachingPort{inner:BorrowedResidentState{store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:vec![]};
    let outcome = run_context(&book,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),"根据这段回答给我一道应用题。","now").unwrap();
    assert!(tool_result(&context.messages,"repeat").contains("AGENT_NO_PROGRESS"));
    assert_eq!(port.accepted,["design","select"]);
    assert_eq!(outcome.teaching_delivery_move_refs,["selected:application"]);
    assert!(!outcome.incomplete);
    assert_eq!(outcome.answer.as_deref(),Some("请在练习卡中选择你的答案。"));
    let card = &port.selections["selected:application"];
    assert_eq!(card["assessment"]["rule"]["correct"],serde_json::json!(["A"]));
    assert!(card["prompt"].as_str().unwrap().contains(case));
    assert!(!crate::tutor::public_arguments(&selection.to_string()).contains("correct_option"));
    for (plan, operation) in adapter.inner.0.seen_plans.borrow().iter().skip(2).zip(["design","select"]) {
        assert_eq!(plan.tools.len(),1);
        assert_eq!(plan.tools[0].name,"tutor.step");
        assert_eq!(plan.tools[0].parameters["properties"]["operation"]["enum"],serde_json::json!([operation]));
    }
}

#[test]
fn teaching_design_loop_direct_reply_does_not_adopt_or_require_a_saved_plan() {
    for saved_plan in [serde_json::Value::Null,serde_json::json!({"design_ref":"design:current","design":design()})] {
        let b=book();
        let mut store=MemoryStore::open(tmp("teaching-direct-reply")).unwrap();
        let mut reader=Reader::new(&b,DEFAULT_RADIUS);
        let snapshot=default_profile_snapshot(&b,&store,"t0");
        let text="The card remains available even when this route omits it.";
        let mut selection=selected_body("direct",text);
        selection["move"].as_object_mut().unwrap().remove("design_ref");
        selection["move"].as_object_mut().unwrap().remove("block_refs");
        selection["finish"]=serde_json::json!(true);
        let adapter=ReviewedTeachingAdapter::new(vec![turn_calls(vec![call("direct-select","tutor.step",&selection.to_string())])],vec![]);
        let mut context=RunContext::new(new_session(),OuterConfig{max_turns:Some(3),..Default::default()},adapter.model_runtime_profile());
        context.tutor=Some(serde_json::json!({"status":"practice","current_design":saved_plan,"learner_context":{"current_request":"Explain whether the card remains."}}));
        let mut port=TeachingPort{inner:BorrowedResidentState{store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:vec![]};
        let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"Explain whether the card remains.","t0").unwrap();
        assert!(!outcome.incomplete);
        assert_eq!(outcome.answer.as_deref(),Some(text));
        assert_eq!(port.accepted,["select"]);
        assert!(port.selections["selected:direct"]["design_ref"].is_null());
        assert!(adapter.seen_reviews.borrow().is_empty());
    }
}

#[test]
fn teaching_design_loop_consumes_design_before_selection_and_holds_unready_stream() {
    let b = book();
    let mut store = MemoryStore::open(tmp("teaching-design-stream")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let adapter = StreamingTeachingAdapter(RequestPlanRecordingAdapter::new(vec![
        turn_final("UNREADY_EXPLANATION_MUST_NOT_STREAM"),
        turn_calls(vec![call("design", "tutor.step", &serde_json::json!({"operation":"design","operation_id":"d1","design":design()}).to_string()), call("early-select", "tutor.step", &select_args())]),
        turn_calls(vec![call("select", "tutor.step", &select_args())]),
        turn_final("保留材料和为一条路线选用材料是两个决定。"),
    ], vec![]));
    let sink = std::sync::Arc::new(TeachingStreamSink::default());
    let mut context = RunContext::new(new_session(), OuterConfig { max_turns: Some(8), ..Default::default() }, adapter.model_runtime_profile());
    context.events = crate::run_events::RunEvents::new(Some(sink.clone()));
    context.tutor = Some(serde_json::json!({"status":"practice","learner_context":{"current_request":"FROZEN_INPUT_STAYS_ONCE"}}));
    let mut port = TeachingPort { inner: BorrowedResidentState { store: &mut store, reader: &mut reader }, accepted: Vec::new(), selections:Default::default(),reviews:Vec::new() };
    let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), "为什么没选到的材料还在？", "t0").unwrap();
    assert_eq!(port.accepted, ["design", "select"], "same-batch selection must never reach storage");
    assert_eq!(outcome.teaching_delivery_move_refs, ["selected:current"]);
    assert!(context.messages.iter().any(|message| message.tool_call_id.as_deref() == Some("early-select") && message.content.as_ref().is_some_and(|text| text.contains("TEACHING_DESIGN_REQUIRES_NEXT_SAMPLING"))));
    let plans = adapter.0.seen_plans.borrow();
    let after_design = serde_json::to_string(&plans[2].ordered_messages()).unwrap();
    assert!(after_design.contains("current_design"));
    assert!(after_design.contains("A retained card can serve different selections."));
    assert!(after_design.contains("RELATION_SCOPED_CONTEXT"));
    let receipt:serde_json::Value=serde_json::from_str(plans[2].input.iter().find(|message|message.tool_call_id.as_deref()==Some("design")).unwrap().content.as_deref().unwrap()).unwrap();
    assert_eq!(receipt["model_body"]["design_ref"],"design:current");
    assert!(receipt["model_body"].get("learner_context_update").is_none());
    assert_eq!(after_design.matches("FROZEN_INPUT_STAYS_ONCE").count(), 1);
    let patches = serde_json::to_string(&*sink.0.lock().unwrap()).unwrap();
    assert!(!patches.contains("UNREADY_EXPLANATION_MUST_NOT_STREAM"));
    assert!(patches.contains("保留材料"));
    let restored: OuterOutcome = serde_json::from_value(serde_json::to_value(&outcome).unwrap()).unwrap();
    assert_eq!(restored.teaching_delivery_move_refs, outcome.teaching_delivery_move_refs);
}

#[test]
fn teaching_design_loop_reuses_frozen_design_and_outside_does_not_require_design() {
    for outside in [false, true] {
        let b = book();
        let mut store = MemoryStore::open(tmp(if outside { "teaching-outside" } else { "teaching-reuse" })).unwrap();
        let mut reader = Reader::new(&b, DEFAULT_RADIUS);
        let snapshot = default_profile_snapshot(&b, &store, "t0");
        let args = if outside { serde_json::json!({"operation":"outside"}).to_string() } else { select_args() };
        let adapter = RequestPlanRecordingAdapter::new(vec![turn_calls(vec![call("resolve", "tutor.step", &args)]), turn_final("继续回应当前请求。")], vec![]);
        let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
        context.tutor = Some(if outside { serde_json::json!({"status":"practice","learner_context":{}}) } else { serde_json::json!({"status":"practice","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}) });
        let mut port = TeachingPort { inner: BorrowedResidentState { store: &mut store, reader: &mut reader }, accepted: Vec::new(), selections:Default::default(),reviews:Vec::new() };
        let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
            &mut EphemeralCompactionCheckpointSink::default(), "请完整说明。", "t0").unwrap();
        assert_eq!(port.accepted, [if outside { "outside" } else { "select" }]);
        assert_eq!(outcome.teaching_delivery_move_refs.is_empty(), outside);
    }
}

#[test]
fn teaching_design_loop_help_and_continuation_do_not_lock_a_teaching_method() {
    use crate::tutor_continuation::*;
    for action in [InteractionAction::UiEntry { entry_action: EntryAction::ClarifyAnswer },
        InteractionAction::UiChoice { teaching_choice: TeachingChoice { delivery_ref:"delivery".into(), option_id:"help".into() }, move_kind:ContinuationMoveKind::Explain }] {
        let resolved = ResolvedInteraction { action, teach_back_reply: None, course: None, learning_purpose:None, user_supplement:String::new(), accepted_request:"这一步没跟上".into(), answer_ref:None, answer_text:None, source_excerpt:None, source_bindings:Vec::new(), target:None, expected_chat_head:None, presentation_follow_up:None, note_ref:None };
        let context = serde_json::json!({"current_action":resolved});
        assert_eq!(crate::tutor::action_intent(&context), None);
        assert!(resolved.validates_current_action("guided_inquiry", "compare"));
        assert!(resolved.validates_current_action("direct_explanation", "explain"));
    }
}

#[test]
fn teaching_design_loop_refreshes_interpretation_after_same_batch_design_and_revision() {
    let b = book();
    let mut store = MemoryStore::open(tmp("teaching-interpretation-refresh")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let evidence = |id: &str, interpretation: &str| serde_json::json!({
        "operation":"evidence","operation_id":id,"nature":"hypothesis","interpretation":interpretation,
        "target":design()["target"],"fact_refs":["usage:current"],"teaching_implication":"Compare retained material and route selection.",
    }).to_string();
    let adapter = RequestPlanRecordingAdapter::new(vec![
        turn_calls(vec![call("design", "tutor.step", &serde_json::json!({"operation":"design","operation_id":"d1","design":design()}).to_string()),
            call("evidence-first", "tutor.step", &evidence("evidence:first","CURRENT_INTERPRETATION_AFTER_DESIGN"))]),
        turn_calls(vec![call("evidence-revised", "tutor.step", &evidence("evidence:revised","REVISED_INTERPRETATION"))]),
        turn_calls(vec![call("select", "tutor.step", &select_args())]),
        turn_final("继续使用保存的材料，比较不同路线的选择。"),
    ], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig { max_turns:Some(8), ..Default::default() }, adapter.model_runtime_profile());
    context.tutor = Some(serde_json::json!({"status":"practice","learner_context":{
        "current_request":"FROZEN_INPUT_STAYS_ONCE", "recent_facts":[{"assistance_refs":["help:before-input"]}],
        "interpretations":[{"interpretation":"OUTDATED_INITIAL_INTERPRETATION"}],"evidence_watermark":0,
        "learning_opportunity":{"interpretation_refs":["OUTDATED_OPPORTUNITY"]},
        "uninterpreted_fact_refs":["usage:current","usage:other"],
    }}));
    let mut port = TeachingPort { inner: BorrowedResidentState { store:&mut store, reader:&mut reader }, accepted:Vec::new(), selections:Default::default(),reviews:Vec::new() };
    run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),"没选到就不能用了？","t0").unwrap();
    assert_eq!(port.accepted,["design","evidence","evidence","select"],"{:?}",context.messages);
    let plans = adapter.seen_plans.borrow();
    for (index, expected, watermark) in [(1,"CURRENT_INTERPRETATION_AFTER_DESIGN",1),(2,"REVISED_INTERPRETATION",2)] {
        let systems:Vec<_> = plans[index].ordered_messages().into_iter().filter(|message|message.role==Role::System).collect();
        let current = systems.iter().filter_map(|message|message.content.as_deref()).rfind(|text|text.starts_with("runtime_state_snapshot.v1")).unwrap();
        assert!(current.contains(expected), "the next model sampling must consume the accepted interpretation");
        assert!(current.contains(r#""uninterpreted_fact_refs":["usage:other"]"#),"the current view must exclude facts already covered by accepted evidence");
        let frozen = systems.iter().filter_map(|message|message.content.as_deref()).find(|text|text.contains("key=agent.tutor_context")).unwrap();
        assert!(!frozen.contains("uninterpreted_fact_refs"),"evidence coverage is derived state, not a frozen input fact");
        assert!(!frozen.contains("learning_opportunity"),"reasoning responsibility must be decided from current support and expression, not a frozen opportunity snapshot");
        assert!(current.contains(if index==1 {r#""interpretation_refs":["evidence:first"]"#} else {r#""interpretation_refs":["evidence:revised"]"#}));
        assert!(!current.contains("OUTDATED_OPPORTUNITY"));
        for preserved in ["usage:before", "B is gone when omitted.", "usage:after", "An added route item could already be in the catalog.", "help:before-input"] {
            assert!(current.contains(preserved), "the next sampling must retain each expression and its own help conditions: {preserved}");
        }
        assert!(!current.contains("OUTDATED_INITIAL_INTERPRETATION"));
        assert!(!current.contains("RELATION_SCOPED_CONTEXT"), "design-time understanding must not shadow newer evidence");
        if index==2 { assert!(!current.contains("CURRENT_INTERPRETATION_AFTER_DESIGN"), "revised understanding replaces the authoritative view"); }
        assert_eq!(serde_json::to_string(&systems).unwrap().matches("FROZEN_INPUT_STAYS_ONCE").count(),1);
        assert!(current.contains("agent.teaching_design"), "the unchanged design remains selected in active_keys");
        assert_eq!(systems.iter().filter_map(|message|message.content.as_deref())
            .map(|text|text.matches("A retained card can serve different selections.").count()).sum::<usize>(),1,
            "the unchanged design body stays available without being appended again");
        let receipt_id=if index==1 {"evidence-first"} else {"evidence-revised"};
        let receipt:serde_json::Value=serde_json::from_str(plans[index].input.iter().find(|m|m.tool_call_id.as_deref()==Some(receipt_id)).unwrap().content.as_deref().unwrap()).unwrap();
        assert!(receipt["model_body"].get("learner_context_update").is_none(),"receipts must not retain obsolete snapshots in history");
        assert_eq!(receipt["model_body"]["evidence_ref"],if watermark==1 {"evidence:first"} else {"evidence:revised"});
    }
    let tutor=context.tutor.as_ref().unwrap();
    assert_eq!(tutor["learner_context"]["evidence_watermark"],2);
    assert_eq!(tutor["learner_context"]["uninterpreted_fact_refs"],serde_json::json!(["usage:other"]));
    assert_eq!(tutor["learner_context"]["recent_facts"],serde_json::json!([{"assistance_refs":["help:before-input"]}]));
    assert_eq!(tutor["learner_context"]["interpretations"][0]["assistance_refs"],serde_json::json!(["help:before-input"]));
}

#[test]
fn teaching_delivery_consumes_accepted_text_and_keeps_reasoning_private() {
    let b = book();
    let mut store = MemoryStore::open(tmp("teaching-accepted-delivery")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let mut selection: serde_json::Value = serde_json::from_str(&select_args()).unwrap();
    selection["move"]["delivery_text"] = serde_json::json!("The retained cards are A and B. This route uses A.");
    selection["move"]["participation"] = serde_json::json!({"choice":"invite","reason":"The learner can distinguish the two lists in one concrete case.","support":"Both lists and their different purposes are visible.","learner_inference":"Judge whether another route can still consider B."});
    selection["move"]["response_expectation"] = serde_json::json!({"public_prompt":"Can another route still consider B? What lets you decide?","block_refs":["keep-select"]});
    let canonical = "The retained cards are A and B. This route uses A.\n\nCan another route still consider B? What lets you decide?";
    let adapter = StreamingTeachingAdapter(RequestPlanRecordingAdapter::new(vec![
        turn_calls(vec![call("select", "tutor.step", &selection.to_string())]),
        turn_final("FREE_EXPANSION_SOLVES_B: yes, another route can use B because it remains retained."),
    ], vec![]));
    let sink = std::sync::Arc::new(TeachingStreamSink::default());
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    context.events = crate::run_events::RunEvents::new(Some(sink.clone()));
    context.tutor = Some(serde_json::json!({"status":"practice","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port = TeachingPort { inner: BorrowedResidentState { store: &mut store, reader: &mut reader }, accepted:Vec::new(), selections:Default::default(),reviews:Vec::new() };
    let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), "I can only follow A so far.", "t0").unwrap();
    assert_eq!(outcome.answer.as_deref(), Some(canonical));
    assert_eq!(outcome.teaching_delivery_move_refs, ["selected:current"]);
    assert_eq!(outcome.answer.as_ref().unwrap().matches("Can another route").count(), 1);
    let plans = adapter.0.seen_plans.borrow();
    let next_sampling = serde_json::to_string(&plans[1].ordered_messages()).unwrap();
    assert!(next_sampling.contains("ACCEPTED_PRIVATE_REASONING"));
    assert!(next_sampling.contains("Judge whether another route can still consider B."));
    let receipt: serde_json::Value = serde_json::from_str(plans[1].input.iter().find(|message| message.tool_call_id.as_deref()==Some("select")).unwrap().content.as_deref().unwrap()).unwrap();
    assert!(receipt["model_body"].get("delivery_draft").is_none());
    let public = serde_json::to_string(&context.messages).unwrap();
    assert!(!public.contains("ACCEPTED_PRIVATE_REASONING") && !public.contains("delivery_draft"));
    assert!(!public.contains("FREE_EXPANSION_SOLVES_B"));
    let patches = serde_json::to_string(&*sink.0.lock().unwrap()).unwrap();
    assert!(patches.contains("Can another route still consider B?"));
    assert!(!patches.contains("FREE_EXPANSION_SOLVES_B"), "the unaccepted inference must never stream before replacement");
}

#[test]
fn teaching_selection_automatically_binds_the_current_private_coach_guidance() {
    struct CoachedTeachingPort<'a> { inner: TeachingPort<'a> }
    impl ResidentStatePort for CoachedTeachingPort<'_> {
        fn tutor_coach_context(&mut self) -> Result<Option<serde_json::Value>, ToolError> {
            Ok(Some(serde_json::json!({"status":"ready","coach_ref":"coach:source-turn",
                "source_turn_ref":"turn:prior","guidance":{"sentinel":"private-coach-guidance","observation_receipts":[{"evidence_ref":"private-observation-receipt","fact_ref":"prior-expression"}]}})))
        }
        fn tutor_step(&mut self, request: serde_json::Value, evidence: &[SourceBinding], ranges: &[read_tools::EvidenceRange]) -> Result<serde_json::Value, ToolError> {
            self.inner.tutor_step(request, evidence, ranges)
        }
        fn tutor_record_review(&mut self, record: serde_json::Value) -> Result<serde_json::Value, ToolError> { self.inner.tutor_record_review(record) }
        fn submit_private<R>(&mut self, operation: impl FnOnce(&mut MemoryStore) -> R) -> Result<R, ToolError> { self.inner.submit_private(operation) }
        fn read_live_reader<R>(&mut self, operation: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> { self.inner.read_live_reader(operation) }
        fn apply_reader<R>(&mut self, operation: impl FnOnce(&mut MemoryStore, &mut Reader) -> R) -> Result<R, ToolError> { self.inner.apply_reader(operation) }
        fn reader_input(&mut self, book: &Book, question: &str) -> crate::run_context::ReaderInputSnapshot { self.inner.reader_input(book, question) }
    }
    let b = book();
    let mut store = MemoryStore::open(tmp("teaching-coach-selection")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let selection = selected_body("coached", "The catalog retains B even when this route does not select B.");
    assert!(selection["move"].get("coach_ref").is_none(), "the teacher does not choose the coach decision");
    let adapter = StreamingTeachingAdapter(RequestPlanRecordingAdapter::new(vec![
        turn_calls(vec![call("select", "tutor.step", &selection.to_string())]),
        turn_final("Use the accepted explanation."),
    ], vec![]));
    let sink = std::sync::Arc::new(TeachingStreamSink::default());
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    context.events = crate::run_events::RunEvents::new(Some(sink.clone()));
    context.tutor = Some(serde_json::json!({"status":"practice","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port = CoachedTeachingPort { inner: TeachingPort {
        inner: BorrowedResidentState { store: &mut store, reader: &mut reader },
        accepted:Vec::new(), selections:Default::default(), reviews:Vec::new(),
    }};
    let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), "Explain why B remains available.", "t0").unwrap();
    assert_eq!(port.inner.accepted, ["select"]);
    assert_eq!(port.inner.selections["selected:coached"]["coach_ref"], "coach:source-turn");
    assert_eq!(outcome.answer.as_deref(), selection["move"]["delivery_text"].as_str());
    let plans = adapter.0.seen_plans.borrow();
    let sampled=serde_json::to_string(&plans[0].ordered_messages()).unwrap();
    assert!(!sampled.contains("private-coach-guidance"));
    assert!(sampled.contains("private-observation-receipt"));
    let public = serde_json::to_string(&context.messages).unwrap();
    let patches = serde_json::to_string(&*sink.0.lock().unwrap()).unwrap();
    for private in ["coach:source-turn", "private-coach-guidance"] {
        assert!(!public.contains(private), "private coach metadata cannot enter public tool history");
        assert!(!patches.contains(private), "private coach metadata cannot enter answer streaming");
    }
}

#[test]
fn teaching_delivery_reselection_design_and_outside_replace_the_accepted_draft() {
    for change in ["select", "design", "outside"] {
        let b = book();
        let mut store = MemoryStore::open(tmp(&format!("teaching-delivery-{change}"))).unwrap();
        let mut reader = Reader::new(&b, DEFAULT_RADIUS);
        let snapshot = default_profile_snapshot(&b, &store, "t0");
        let mut first: serde_json::Value = serde_json::from_str(&select_args()).unwrap();
        first["move"]["delivery_text"] = serde_json::json!("SUPERSEDED_ACCEPTED_BODY");
        let mut replacement = first.clone();
        replacement["move"]["move_id"] = serde_json::json!("replacement");
        replacement["move"]["delivery_text"] = serde_json::json!("CURRENT_ACCEPTED_BODY");
        let mut turns = vec![turn_calls(vec![call("first", "tutor.step", &first.to_string())])];
        match change {
            "design" => {
                let mut revised = design();
                revised["previous_ref"] = serde_json::json!("design:current");
                turns.push(turn_calls(vec![call("redesign", "tutor.step", &serde_json::json!({"operation":"design","operation_id":"revised","design":revised}).to_string())]));
            }
            "outside" => turns.push(turn_calls(vec![call("outside", "tutor.step", r#"{"operation":"outside"}"#)])),
            _ => {},
        }
        if change != "outside" { turns.push(turn_calls(vec![call("replace", "tutor.step", &replacement.to_string())])); }
        turns.push(turn_final("ORDINARY_FINAL_TEXT"));
        let adapter = RequestPlanRecordingAdapter::new(turns, vec![]);
        let mut context = RunContext::new(new_session(), OuterConfig { max_turns:Some(8), ..Default::default() }, adapter.model_runtime_profile());
        context.tutor = Some(serde_json::json!({"status":"practice","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
        let mut port = TeachingPort { inner: BorrowedResidentState { store:&mut store, reader:&mut reader }, accepted:Vec::new(), selections:Default::default(),reviews:Vec::new() };
        let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
            &mut EphemeralCompactionCheckpointSink::default(), "Only this part is clear.", "t0").unwrap();
        assert_eq!(outcome.answer.as_deref(), Some(if change == "outside" {"ORDINARY_FINAL_TEXT"} else {"CURRENT_ACCEPTED_BODY"}));
        let plans = adapter.seen_plans.borrow();
        let current_snapshot = |index: usize| plans[index].ordered_messages().into_iter()
            .filter_map(|message| message.content).rfind(|text| text.starts_with("runtime_state_snapshot.v1")).unwrap();
        if change != "select" {
            let after_clear = current_snapshot(2);
            assert!(after_clear.contains(r#""delivery_draft":null"#), "{change} must clear the working draft before another sampling");
            assert!(!after_clear.contains("SUPERSEDED_ACCEPTED_BODY"));
        }
        if change != "outside" {
            let latest = current_snapshot(plans.len()-1);
            assert!(latest.contains("CURRENT_ACCEPTED_BODY"));
            assert!(!latest.contains("SUPERSEDED_ACCEPTED_BODY"));
            if change == "design" { assert!(!context.accepted_teaching_moves.contains_key("selected:current")); }
        } else {
            assert!(context.teaching_delivery_draft.is_none());
            assert!(context.accepted_teaching_moves.is_empty());
            assert!(outcome.teaching_delivery_move_refs.is_empty());
        }
    }
}

#[test]
fn teaching_delivery_budget_finalization_delivers_the_accepted_body() {
    let b = book();
    let mut store = MemoryStore::open(tmp("teaching-delivery-budget")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let adapter = StreamingTeachingAdapter(RequestPlanRecordingAdapter::new(vec![
        turn_calls(vec![call("select", "tutor.step", &select_args())]),
        turn_final("BUDGET_FREE_EXPANSION_MUST_NOT_REPLACE_SUPPORT"),
    ], vec![]));
    let sink = std::sync::Arc::new(TeachingStreamSink::default());
    let mut context = RunContext::new(new_session(), OuterConfig { max_turns:Some(1), ..Default::default() }, adapter.model_runtime_profile());
    context.events = crate::run_events::RunEvents::new(Some(sink.clone()));
    context.tutor = Some(serde_json::json!({"status":"practice","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port = TeachingPort { inner: BorrowedResidentState { store:&mut store, reader:&mut reader }, accepted:Vec::new(), selections:Default::default(),reviews:Vec::new() };
    let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), "The route leaves B out, but I am unsure what that means.", "t0").unwrap();
    assert_eq!(outcome.answer.as_deref(), Some("保留材料和为一条路线选用材料是两个决定。"));
    assert_eq!(outcome.teaching_delivery_move_refs, ["selected:current"]);
    assert!(!outcome.incomplete && outcome.warning.is_none(), "accepted canonical delivery remains deliverable when tool budget ends");
    let plans = adapter.0.seen_plans.borrow();
    assert!(plans[1].tools.is_empty());
    assert!(serde_json::to_string(&plans[1].ordered_messages()).unwrap().contains("ACCEPTED_PRIVATE_REASONING"));
    let patches = serde_json::to_string(&*sink.0.lock().unwrap()).unwrap();
    assert!(patches.contains("保留材料"));
    assert!(!patches.contains("BUDGET_FREE_EXPANSION"));
}

#[test]
fn teaching_selection_preserves_the_explicit_retained_set_even_after_a_replacement() {
    let b = book();
    let mut store = MemoryStore::open(tmp("teaching-review-retained-set")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let first = selected_body("first","FIRST_PREMISE");
    let second = selected_body("second","SUPERSEDED_SECOND_SCOPE");
    let mut third = selected_body("third","THIRD_SMALL_CONNECTION");
    third["delivery_move_refs"] = serde_json::json!(["selected:first","selected:first"]);
    let adapter = ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("first","tutor.step",&first.to_string())]),
        turn_calls(vec![call("second","tutor.step",&second.to_string())]),
        turn_calls(vec![call("third","tutor.step",&third.to_string())]),
        turn_final("Do not expand the accepted composition."),
    ],vec![accepted_review(),accepted_review(),accepted_review()]);
    let mut context = RunContext::new(new_session(),OuterConfig {max_turns:Some(10),..Default::default()},adapter.model_runtime_profile());
    context.tutor=Some(serde_json::json!({"status":"practice","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:Vec::new(),selections:Default::default(),reviews:Vec::new()};
    let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),"I can follow only the first premise.","t0").unwrap();
    assert!(adapter.seen_reviews.borrow().is_empty() && port.reviews.is_empty());
    assert_eq!(context.teaching_delivery_draft.as_ref().unwrap()["moves"].as_array().unwrap().len(),2);
    assert_eq!(outcome.answer.as_deref(),Some("FIRST_PREMISE\n\nTHIRD_SMALL_CONNECTION"));
    assert_eq!(outcome.teaching_delivery_move_refs,["selected:first","selected:third"]);
}

#[test]
fn teaching_preflight_missing_read_recovers_before_saving_the_normalized_move() {
    let b=book();
    let mut store=MemoryStore::open(tmp("teaching-preflight-read-recovery")).unwrap();
    let mut reader=Reader::new(&b,DEFAULT_RADIUS);
    let snapshot=default_profile_snapshot(&b,&store,"t0");
    let mut selection=selected_body("source-repaired","The retained catalog still contains the card omitted from this route.");
    selection["move"].as_object_mut().unwrap().remove("target");
    let adapter=ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("missing-read","tutor.step",&selection.to_string())]),
        turn_calls(vec![call("read-source","book.text",r#"{"lid":"1.1"}"#)]),
        turn_calls(vec![call("repaired","tutor.step",&selection.to_string())]),
        turn_final("Use the accepted explanation."),
    ],vec![accepted_review()]);
    let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(8),..Default::default()},adapter.model_runtime_profile());
    context.tutor=Some(serde_json::json!({"status":"practice","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port=SourcePreflightTeachingPort {
        inner:TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:Vec::new(),selections:Default::default(),reviews:Vec::new()},
        observed_reads:RefCell::new(Vec::new()),
        required:read_tools::EvidenceRange {start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:Vec::new()},
    };
    let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),"Explain why the omitted card remains available.","t0").unwrap();
    assert!(!outcome.incomplete,"{outcome:?}");
    assert_eq!(outcome.answer.as_deref(),selection["move"]["delivery_text"].as_str());
    assert_eq!(outcome.teaching_delivery_move_refs,["selected:source-repaired"]);
    assert_eq!(port.inner.accepted,["select"],"the rejected candidate must never commit");
    let reads=port.observed_reads.borrow();
    assert_eq!(reads.len(),2);
    assert!(reads[0].is_empty(),"a source locator is not evidence that its text has been read");
    assert!(reads[1].iter().any(|range|range.start_lid=="1.1" && range.end_lid=="1.1"));
    let missing:serde_json::Value=serde_json::from_str(&tool_result(&context.messages,"missing-read")).unwrap();
    assert_eq!(missing["missing_source_read"],serde_json::json!({"start_lid":"1.1","end_lid":"1.1"}));
    assert!(tool_result_error_code(&tool_result(&context.messages,"read-source")).is_none(),"the host-reported missing range must authorize book.text");
    assert!(context.locator_ledger.origins("1.1").unwrap().contains(&LocatorOrigin::TeachingValidation));
    assert!(adapter.seen_reviews.borrow().is_empty() && port.inner.reviews.is_empty());
    let accepted=&port.inner.selections["selected:source-repaired"];
    assert_eq!(accepted["interaction_intent"],"direct_explanation");
    assert_eq!(accepted["intent_origin"],"current_action");
    assert_eq!(accepted["target"],design()["target"]);
    assert_eq!(accepted["delivery_text"],selection["move"]["delivery_text"]);
}

#[test]
fn teaching_design_missing_read_after_early_authoring_is_real_locator_progress() {
    let b=book_leaves(3);
    let mut store=MemoryStore::open(tmp("teaching-design-read-recovery")).unwrap();
    let mut reader=Reader::new(&b,1);
    let snapshot=default_profile_snapshot(&b,&store,"t0");
    let request=serde_json::json!({"operation":"design","operation_id":"d1","design":design()}).to_string();
    let adapter=ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("early-author","presentation.author",r#"{"operation":"prepare"}"#)]),
        turn_calls(vec![call("missing-read","tutor.step",&request)]),
        turn_calls(vec![call("read-source","book.text",r#"{"lid":"1.3"}"#)]),
        turn_calls(vec![call("repaired-design","tutor.step",&request)]),
        turn_calls(vec![call("select","tutor.step",&select_args())]),
        turn_final("Deliver the accepted explanation."),
    ],vec![accepted_review()]);
    let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(10),..Default::default()},adapter.model_runtime_profile());
    context.tutor=Some(serde_json::json!({"status":"practice","learner_context":{}}));
    let mut port=SourcePreflightTeachingPort {
        inner:TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:Vec::new(),selections:Default::default(),reviews:Vec::new()},
        observed_reads:RefCell::new(Vec::new()),
        required:read_tools::EvidenceRange {start_lid:"1.3".into(),end_lid:"1.3".into(),ranges:Vec::new()},
    };
    let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),"那Embedding在哪里会被用到呢，怎么用呢，给我个演示页吧","t0").unwrap();
    assert!(tool_result_error_code(&tool_result(&context.messages,"early-author")).is_some(),"authoring must wait for the design");
    let missing:serde_json::Value=serde_json::from_str(&tool_result(&context.messages,"missing-read")).unwrap();
    assert_eq!(missing["missing_source_read"]["start_lid"],"1.3");
    assert!(context.locator_ledger.origins("1.3").unwrap().contains(&LocatorOrigin::TeachingValidation));
    assert!(tool_result_error_code(&tool_result(&context.messages,"read-source")).is_none());
    assert_eq!(port.inner.accepted,["design","select"]);
    assert!(!outcome.incomplete,"{outcome:?}");
    assert!(adapter.seen_reviews.borrow().is_empty());
}

#[test]
fn teaching_unknown_reference_never_select_the_candidate() {
    for failure in ["unknown_reference"] {
        let b=book();
        let mut store=MemoryStore::open(tmp(&format!("teaching-review-{failure}"))).unwrap();
        let mut reader=Reader::new(&b,DEFAULT_RADIUS);
        let snapshot=default_profile_snapshot(&b,&store,"t0");
        let mut selection=selected_body("unreviewed","UNREVIEWED_CANDIDATE_MUST_STAY_PRIVATE");
        if failure=="unknown_reference" { selection["delivery_move_refs"]=serde_json::json!(["move:not-accepted"]); }
        let reviews=vec![];
        let adapter=ReviewedTeachingAdapter::new(vec![
            turn_calls(vec![call("candidate","tutor.step",&selection.to_string())]),
            turn_calls(vec![call("outside","tutor.step",r#"{"operation":"outside"}"#)]),
            turn_final("Ordinary response after leaving this teaching attempt."),
        ],reviews);
        let sink=std::sync::Arc::new(TeachingStreamSink::default());
        let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(8),..Default::default()},adapter.model_runtime_profile());
        context.events=crate::run_events::RunEvents::new(Some(sink.clone()));
        context.tutor=Some(serde_json::json!({"status":"practice","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
        let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:Vec::new(),selections:Default::default(),reviews:Vec::new()};
        let result=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"Only this fragment is clear.","t0");
        let outcome=result.unwrap();
        assert!(outcome.teaching_delivery_move_refs.is_empty());
        assert!(!outcome.answer.as_deref().unwrap_or_default().contains("UNREVIEWED_CANDIDATE"));
        assert!(!port.accepted.iter().any(|operation|operation=="select"),"{failure}");
        assert!(port.selections.is_empty() && context.accepted_teaching_moves.is_empty());
        assert!(context.teaching_delivery_draft.is_none());
        assert!(adapter.seen_reviews.borrow().is_empty());
        assert!(!serde_json::to_string(&*sink.0.lock().unwrap()).unwrap().contains("UNREVIEWED_CANDIDATE"));
    }
}


fn presentation_design() -> serde_json::Value {
    let mut design=design();
    // Page authoring consumes the normalized stored design, unlike the raw
    // source ranges accepted by the tutor.step design input fixture above.
    let b=book();
    let source=serde_json::json!({"source_id":b.base.book_id,"source_revision":b.source_fingerprint(),"start_lid":"1.1","end_lid":"1.1"});
    design["target"]["source_bindings"]=serde_json::json!([source]);
    design["blocks"][0]["source_bindings"]=serde_json::json!([source]);
    design["representation"]=serde_json::json!({"kind":"presentation","reason":"The learner loses the same object across two selections.","observation":"Keep B visible in the catalog while changing this route's selection."});
    design
}

#[test]
fn teaching_representation_activates_authoring_without_a_user_demo_request() {
    for (label, current, create) in [("accepted",None,true),("reused",Some(presentation_design()),false),("text",Some(design()),false)] {
        let b=book();
        let mut store=MemoryStore::open(tmp(&format!("teaching-representation-{label}"))).unwrap();
        let mut reader=Reader::new(&b,DEFAULT_RADIUS);
        let snapshot=default_profile_snapshot(&b,&store,"t0");
        let mut turns=vec![];
        if create { turns.push(turn_calls(vec![call("design","tutor.step",&serde_json::json!({"operation":"design","operation_id":"d1","design":presentation_design()}).to_string())])); }
        turns.push(turn_calls(vec![call("outside","tutor.step",r#"{"operation":"outside"}"#)]));
        turns.push(turn_final("The separate request is complete."));
        let adapter=RequestPlanRecordingAdapter::new(turns,vec![]);
        let mut context=RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
        context.tutor=Some(serde_json::json!({"status":"practice","current_design":current.map(|design|serde_json::json!({"design_ref":"design:current","design":design})),"learner_context":{}}));
        let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
        run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"I can follow B in one list, but lose it in the other.","t0").unwrap();
        let plans=adapter.seen_plans.borrow();
        assert_eq!(plans[usize::from(create)].tools.iter().any(|tool|tool.name=="presentation.author"),create);
        if create { assert!(!plans[0].tools.iter().any(|tool|tool.name=="presentation.author")); }
        if create {
            let instructions=serde_json::to_string(&plans[usize::from(create)].ordered_messages()).unwrap();
            assert!(instructions.contains("resident-agent.presentation.phase.local"),"an accepted teaching design supplies the framework for local authoring");
            assert!(!instructions.contains("resident-agent.presentation.phase.global"));
        }
    }
}

#[test]
fn teaching_presentation_requirement_uses_actual_prior_delivery_not_saved_versions() {
    for delivered in [false,true] {
        let b=book();
        let mut store=MemoryStore::open(tmp(if delivered {"teaching-page-feedback"} else {"teaching-page-not-delivered"})).unwrap();
        let mut reader=Reader::new(&b,DEFAULT_RADIUS);
        let snapshot=default_profile_snapshot(&b,&store,"t0");
        let mut turns=vec![turn_calls(vec![call("select","tutor.step",&select_args())])];
        if !delivered { turns.push(turn_calls(vec![call("outside","tutor.step",r#"{"operation":"outside"}"#)])); }
        turns.push(turn_final("Only the selected support is delivered."));
        let adapter=ReviewedTeachingAdapter::new(turns,if delivered {vec![accepted_review()]} else {vec![]});
        let mut context=RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
        context.tutor=Some(serde_json::json!({"status":"practice","current_design":{"design_ref":"design:current","design":presentation_design()},
            "learner_context":{"learning_opportunity":{"delivered_support":if delivered {serde_json::json!([{"design_ref":"design:current","presentation":{"presentation_id":"prior","revision":1}}])} else {serde_json::json!([])}}}}));
        context.delivered_presentations.push(crate::presentation::PresentationRef {presentation_id:"saved-only".into(),revision:1});
        let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
        let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"I think B stays, but I'm unsure why.","t0").unwrap();
        assert_eq!(port.accepted.iter().any(|operation|operation=="select"),delivered);
        assert!(adapter.seen_reviews.borrow().is_empty());
        if delivered { assert!(!outcome.answer_view.unwrap().parts.iter().any(|part|matches!(part,AgentAnswerPart::Presentation {..})),"feedback need not repeat or regenerate the page"); }
    }
}

#[test]
fn teaching_page_observation_and_exact_selected_revision_survive_both_finish_paths() {
    for budget in [false,true] {
        let b=book();
        let mut store=MemoryStore::open(tmp(if budget {"teaching-page-budget"} else {"teaching-page-final"})).unwrap();
        let mut reader=Reader::new(&b,DEFAULT_RADIUS);
        let snapshot=default_profile_snapshot(&b,&store,"t0");
        let mut turns=vec![];
        for revision in [1,2] {
            let candidate=format!("c{revision}");
            let write=serde_json::json!({"operation":"write","title":revision.to_string(),"html":"<p id=\"catalog\">B</p>","readable_content":"AUTHOR_CLAIM_MUST_NOT_REPLACE_SAVED_CONTENT",
                "teaching_binding":{"design_ref":"design:current","block_refs":["keep-select"],"regions":[]}});
            turns.push(turn_calls(vec![call(&format!("write-{revision}"),"presentation.author",&write.to_string())]));
            turns.push(turn_calls(vec![call(&format!("preview-{revision}"),"presentation.author",&serde_json::json!({"operation":"preview","candidate_id":candidate}).to_string())]));
            turns.push(turn_calls(vec![call(&format!("save-{revision}"),"presentation.author",&serde_json::json!({"operation":"deliver","candidate_id":candidate}).to_string())]));
            let mut selection=selected_body(&format!("page-{revision}"),"The page keeps B in place while one route changes.");
            if !budget && revision==2 { selection["finish"]=serde_json::json!(true); }
            selection["move"]["presentation"]=serde_json::json!({"presentation_id":"cards","revision":revision});
            turns.push(turn_calls(vec![call(&format!("select-{revision}"),"tutor.step",&selection.to_string())]));
        }
        turns.push(turn_final("FREE_FINAL_MUST_NOT_REINTRODUCE_THE_OLD_PAGE"));
        turns.insert(0,turn_calls(vec![call("discover-page","tool.search",r#"{"task":"author the presentation for this request","required_capabilities":["presentation_authoring"],"scope":"passage","operation":"explain","effect_mode":"read_only","max_results":3}"#)]));
        let adapter=ReviewedTeachingAdapter::new(turns,vec![Ok(serde_json::json!({"decision":"revise","revision":"The page changes two conditions at once. Keep one route fixed and revise the page."})),accepted_review()]);
        let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(if budget {9} else {16}),..Default::default()},adapter.model_runtime_profile());
        context.tutor=Some(serde_json::json!({"status":"practice","current_design":{"design_ref":"design:current","design":presentation_design()},"learner_context":{}}));
        let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
        let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"B seems gone from this route, but I can't follow where it went.","t0").unwrap();
        assert_eq!(port.accepted,["select","select"]);
        let page_refs:Vec<_>=outcome.answer_view.unwrap().parts.into_iter().filter_map(|part|match part {AgentAnswerPart::Presentation {presentation_id,revision}=>Some((presentation_id,revision)),_=>None}).collect();
        assert_eq!(page_refs,[("cards".into(),2)],"a superseded page must not appear in either finalization path");
        assert!(!outcome.incomplete);
        assert_eq!(context.delivered_presentations.len(),2,"both versions really were saved before selection");
        let plans=adapter.inner.0.seen_plans.borrow();
        let saved=tool_result(&plans[4].input,"save-1");
        assert!(saved.contains("tutor.step") && saved.contains("operation select") && saved.contains("cards"),
            "the delivered-page receipt must direct the next sampling to teaching selection: {saved}");
        assert!(adapter.seen_reviews.borrow().is_empty() && port.reviews.is_empty());
        assert!(adapter.seen_explanation_reviews.borrow().is_empty());
        for revision in [1,2] {
            assert!(plans.iter().any(|plan|plan.preview_images.iter().any(|image|
                image.png_base64==format!("png-c{revision}"))),"the author must observe each actual version");
            let reference=crate::presentation::PresentationRef {presentation_id:"cards".into(),revision};
            let observations=&context.presentation_delivery_observations[&serde_json::to_string(&reference).unwrap()];
            assert_eq!(observations[0]["observations"][0]["dom"]["text"],format!("ACTUAL_PREVIEW_c{revision}"));
        }
        if !budget { assert_eq!(plans.len(),9,"page finish needs no copy-only author sampling"); }
        if budget { assert!(adapter.inner.0.seen_plans.borrow().last().unwrap().tools.is_empty()); }
    }
}

#[test]
fn historical_design_requires_a_live_representation_choice_before_generation() {
    let mut old=design(); old.as_object_mut().unwrap().remove("representation");
    let tutor=serde_json::json!({"status":"practice","current_design":{"design_ref":"old","design":old}});
    let error=require_sampled_teaching_design(Some(&tutor),Some("old"),Some("old")).unwrap_err();
    assert!(error.message.contains("representation"));
}

#[test]
fn teaching_embedding_page_focus_then_missing_operation_gets_one_real_repair() {
    for repair in [false,true] {
        let b=book();
        let mut store=MemoryStore::open(tmp(&format!("teaching-embedding-author-repair-{repair}"))).unwrap();
        let mut reader=Reader::new(&b,DEFAULT_RADIUS);
        let snapshot=default_profile_snapshot(&b,&store,"t0");
        let good=serde_json::json!({"operation":"write","title":"1","html":"<p id=\"catalog\">Embedding candidate</p>","readable_content":"Embedding orders candidates.",
            "teaching_binding":{"design_ref":"design:current","block_refs":["keep-select"],"regions":[]}});
        let mut outside=good.clone();outside["teaching_binding"]["block_refs"]=serde_json::json!(["provider-and-cache"]);
        let mut missing=good.clone();missing["operation"]=serde_json::Value::Null;
        let mut turns=vec![turn_calls(vec![call("focus-error","presentation.author",&outside.to_string())]),
            turn_calls(vec![call("missing-operation","presentation.author",&missing.to_string())])];
        if repair {
            turns.push(turn_calls(vec![call("corrected","presentation.author",&good.to_string())]));
            turns.push(turn_calls(vec![call("preview","presentation.author",r#"{"operation":"preview","candidate_id":"c1"}"#)]));
            turns.push(turn_calls(vec![call("save","presentation.author",r#"{"operation":"deliver","candidate_id":"c1"}"#)]));
            let mut select=selected_body("embedding","Embedding orders candidates; inspect their source before deciding.");
            select["move"]["presentation"]=serde_json::json!({"presentation_id":"cards","revision":1});
            turns.push(turn_calls(vec![call("select","tutor.step",&select.to_string())]));
            turns.push(turn_final("Deliver the accepted page."));
        } else {
            let mut invalid=missing.clone();invalid["title"]=serde_json::json!("still missing operation");
            turns.push(turn_calls(vec![call("still-invalid","presentation.author",&invalid.to_string())]));
        }
        turns.insert(0,turn_calls(vec![call("discover-page","tool.search",r#"{"task":"author the presentation for this request","required_capabilities":["presentation_authoring"],"scope":"passage","operation":"explain","effect_mode":"read_only","max_results":3}"#)]));
        let adapter=ReviewedTeachingAdapter::new(turns,if repair {vec![accepted_review()]} else {vec![]});
        let mut context=RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
        context.tutor=Some(serde_json::json!({"status":"practice","current_design":{"design_ref":"design:current","design":presentation_design()},"learner_context":{}}));
        let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
        let result=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"那Embedding在哪里会被用到呢，怎么用呢，给我个演示页吧","t0");
        assert!(tool_result(&context.messages,"focus-error").contains("TEACHING_INVALID"));
        assert!(tool_result(&context.messages,"missing-operation").contains("PRESENTATION_ARGUMENTS_INVALID"));
        if repair {
            let outcome=result.unwrap();
            assert!(!outcome.incomplete);
            assert!(outcome.answer_view.unwrap().parts.iter().any(|part|matches!(part,AgentAnswerPart::Presentation {..})));
        } else {
            let outcome=result.unwrap();
            assert!(outcome.incomplete);
            assert_eq!(outcome.warning.as_deref(),Some("AGENT_NO_PROGRESS"));
            assert!(outcome.teaching_delivery_move_refs.is_empty());
            assert!(outcome.answer.as_deref().is_some_and(|answer|!answer.trim().is_empty()));
            assert!(!outcome.answer_view.unwrap().parts.iter().any(|part|matches!(part,AgentAnswerPart::Presentation {..})));
            assert!(outcome.delivery_diagnostics.as_ref().unwrap().initial.issues[0]
                .trigger_value.as_ref().unwrap().contains("operation"));
            assert!(port.accepted.is_empty());
        }
    }
}

#[test]
fn teaching_review_truncated_response_retains_cause_without_private_reasoning() {
    let response=serde_json::json!({"choices":[{"message":{"content":null,"reasoning_content":"PRIVATE_CHAIN_OF_THOUGHT"},"finish_reason":"length"}],
        "usage":{"completion_tokens":4096,"completion_tokens_details":{"reasoning_tokens":4096}}});
    let error=crate::response_message_content(&response).unwrap_err();
    assert!(error.message.contains("finish_reason=length") && error.message.contains("4096"));
    assert!(!error.message.contains("PRIVATE_CHAIN_OF_THOUGHT"));
}

#[test]
fn teaching_page_patch_inherits_saved_binding_but_not_a_replaced_design() {
    for replaced in [false,true] {
        let b=book();
        let mut store=MemoryStore::open(tmp(&format!("teaching-page-patch-binding-{replaced}"))).unwrap();
        let mut reader=Reader::new(&b,DEFAULT_RADIUS);
        let snapshot=default_profile_snapshot(&b,&store,"t0");
        let mut turns=vec![turn_calls(vec![call("patch","presentation.author",r#"{"operation":"patch","candidate_id":"c1","edits":[{"old_text":"overflow: hidden","new_text":"overflow: auto"}]}"#)])];
        if !replaced {
            turns.push(turn_calls(vec![call("preview","presentation.author",r#"{"operation":"preview","candidate_id":"c2"}"#)]));
            turns.push(turn_calls(vec![call("deliver","presentation.author",r#"{"operation":"deliver","candidate_id":"c2"}"#)]));
            let mut select=selected_body("patched-page","The corrected page keeps the same teaching relationship.");
            select["move"]["presentation"]=serde_json::json!({"presentation_id":"cards","revision":2});
            turns.push(turn_calls(vec![call("select","tutor.step",&select.to_string())]));
            turns.push(turn_final("Deliver the inspected page."));
        }
        turns.insert(0,turn_calls(vec![call("discover-page","tool.search",r#"{"task":"author the presentation for this request","required_capabilities":["presentation_authoring"],"scope":"passage","operation":"explain","effect_mode":"read_only","max_results":3}"#)]));
        let adapter=ReviewedTeachingAdapter::new(turns,if replaced {vec![]} else {vec![accepted_review()]});
        let mut context=RunContext::new(new_session(),OuterConfig {max_turns:if replaced {Some(2)} else {None},..Default::default()},adapter.model_runtime_profile());
        context.tutor=Some(serde_json::json!({"status":"practice","current_design":{"design_ref":if replaced {"design:new"} else {"design:current"},"design":presentation_design()},"learner_context":{}}));
        let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
        let result=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"Fix the page layout and show the same relationship.","t0");
        if replaced {
            let error=result.unwrap_err();
            assert_eq!(error.error_code,TURN_LIMIT_EXCEEDED);
            assert!(error.message.contains("TEACHING_DESIGN_REQUIRES_NEXT_SAMPLING"));
            assert_eq!(adapter.inner.0.seen_plans.borrow().len(), 2);
            assert!(port.accepted.is_empty());
        } else {
            let outcome=result.unwrap();
            assert!(!outcome.incomplete);
            assert_eq!(context.presentation_teaching_designs.get("c2").map(String::as_str),Some("design:current"));
            assert!(outcome.answer_view.unwrap().parts.iter().any(|p|matches!(p,AgentAnswerPart::Presentation {revision:2,..})));
        }
    }
}

#[test]
#[ignore = "exports local semantic fixtures using the production text projection; no model calls"]
fn export_explanation_review_probe() {
    use serde_json::{Value, json};
    let directory = std::path::PathBuf::from(std::env::var("TUTOR_EXPLANATION_PROBE_DIR").unwrap());
    let manifest: Value = serde_json::from_slice(&std::fs::read(directory.join("cases/manifest.json")).unwrap()).unwrap();
    std::fs::write(directory.join("text-review-system.txt"), crate::tutor::text_review_instructions()).unwrap();
    let mut inputs = Vec::new();
    for case in manifest["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let relative = case["raw_packet_file"].as_str().unwrap();
        let mut packet: Value = serde_json::from_slice(&std::fs::read(directory.join("cases").join(relative)).unwrap()).unwrap();
        assert!(crate::tutor_review_input::uses_text_review(&packet), "{id}: must exercise the actual text path");
        // The saved-event fixture calls this field `grounding`; the live
        // review packet uses `learner_grounding`. Preserve the actual history
        // before applying the production projection.
        if let Some(grounding) = packet.as_object_mut().unwrap().remove("grounding") {
            packet["learner_grounding"] = grounding;
        }
        packet["learner_grounding"] = teaching_review_grounding(&packet["learner_grounding"]);
        let projected = crate::tutor_review_input::project_text(packet);
        let packet_file = format!("{id}-packet.json");
        std::fs::write(directory.join(&packet_file), serde_json::to_vec_pretty(&projected).unwrap()).unwrap();
        inputs.push(json!({"id":id,"expected":case["expected"],"packet_file":packet_file,
            "frozen_system_file":"text-review-system.txt"}));
    }
    let context = json!({"status":"practice","learner_context":{}});
    let teacher = explanation_probe_instructions(Some(&context));
    std::fs::write(directory.join("teacher-system.txt"), teacher).unwrap();
    std::fs::write(directory.join("inputs.json"), serde_json::to_vec_pretty(&json!({"cases":inputs})).unwrap()).unwrap();
}

#[test]
#[ignore = "exports production independent-review semantic fixtures without replacing earlier probe evidence; no model calls"]
fn export_independent_explanation_review_probe() {
    use serde_json::{Value, json};
    let directory = std::path::PathBuf::from(std::env::var("TUTOR_EXPLANATION_PROBE_DIR").unwrap());
    let manifest: Value = serde_json::from_slice(&std::fs::read(directory.join("cases/manifest.json")).unwrap()).unwrap();
    let output = directory.join("final");
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(output.join("factual-review-system.txt"), crate::tutor::text_review_instructions()).unwrap();
    std::fs::write(output.join("explanation-review-system.txt"), crate::tutor::explanation_review_instructions()).unwrap();
    let mut inputs = Vec::new();
    for case in manifest["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let relative = case["raw_packet_file"].as_str().unwrap();
        let mut packet: Value = serde_json::from_slice(&std::fs::read(directory.join("cases").join(relative)).unwrap()).unwrap();
        assert!(crate::tutor_review_input::uses_text_review(&packet), "{id}: must exercise the actual text path");
        if let Some(grounding) = packet.as_object_mut().unwrap().remove("grounding") {
            packet["learner_grounding"] = grounding;
        }
        packet["learner_grounding"] = teaching_review_grounding(&packet["learner_grounding"]);
        let explanation = crate::tutor_review_input::project_explanation(&packet);
        let factual = crate::tutor_review_input::project_text(packet);
        let factual_file = format!("{id}-factual-packet.json");
        let explanation_file = format!("{id}-explanation-packet.json");
        std::fs::write(output.join(&factual_file), serde_json::to_vec_pretty(&factual).unwrap()).unwrap();
        std::fs::write(output.join(&explanation_file), serde_json::to_vec_pretty(&explanation).unwrap()).unwrap();
        inputs.push(json!({"id":id,"expected":case["expected"],
            "factual":{"packet_file":factual_file,"frozen_system_file":"factual-review-system.txt"},
            "explanation":{"packet_file":explanation_file,"frozen_system_file":"explanation-review-system.txt"}}));
    }
    let context = json!({"status":"practice","learner_context":{}});
    let teacher = explanation_probe_instructions(Some(&context));
    std::fs::write(output.join("teacher-system.txt"), teacher).unwrap();
    std::fs::write(output.join("ordinary-system.txt"), explanation_probe_instructions(None)).unwrap();
    std::fs::write(output.join("inputs.json"), serde_json::to_vec_pretty(&json!({"cases":inputs})).unwrap()).unwrap();
}

fn explanation_probe_instructions(tutor: Option<&serde_json::Value>) -> String {
    let mut modules = crate::agent_prompt::policy_modules_for_tools(&[]);
    if let Some(context) = tutor {
        modules.extend(crate::tutor::instruction_modules(context, &[], None));
    }
    AgentRequestPlan::for_agent_turn_with_modules(
        ModelRuntimeProfile::fallback("explanation-probe", ProviderToolProtocol::Native),
        &[Message::system(crate::agent_prompt::BASE_INSTRUCTIONS)], &[], &modules,
    ).instructions
}

include!("teaching_patch_tests.rs");

#[test]
fn exam_explanations_use_the_same_author_loop_without_teacher_content_review() {
    for status in ["practice", "observing"] {
        for calendar_revision in [2, 3] {
            let b=book();
            let mut store=MemoryStore::open(tmp(&format!("exam-author-{status}-{calendar_revision}"))).unwrap();
            let mut reader=Reader::new(&b,DEFAULT_RADIUS);
            let snapshot=default_profile_snapshot(&b,&store,"t0");
            let answer="The route selects retained cards; an omitted card can remain in the catalog.";
            let mut selection=selected_body("exam",answer);
            selection["finish"]=serde_json::json!(true);
            let turns=if status=="practice" {
                vec![turn_calls(vec![call("select","tutor.step",&selection.to_string())])]
            } else { vec![turn_final(answer)] };
            let adapter=ReviewedTeachingAdapter::new(turns,vec![]);
            let mut context=RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
            context.workspace_goal=Some(exam_workspace(calendar_revision));
            context.tutor=Some(serde_json::json!({"status":status,
                "current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
            let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},
                accepted:vec![],selections:Default::default(),reviews:vec![]};
            let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
                &mut EphemeralCompactionCheckpointSink::default(),"Explain this relationship for my exam.","t0").unwrap();
            assert!(!outcome.incomplete);
            assert_eq!(outcome.answer.as_deref(),Some(answer));
            assert_eq!(outcome.turns,1);
            assert!(port.reviews.is_empty() && adapter.seen_reviews.borrow().is_empty());
            assert!(adapter.seen_explanation_reviews.borrow().is_empty());
        }
    }
}
