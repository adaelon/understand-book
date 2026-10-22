use super::*;
include!("../../read-tools/testdata/math_book.rs");

#[test]
fn math_structure_source_reading_reaches_active_tutor_review_and_delivery() {
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
    context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":plan},
        "learner_context":{"current_request":question}}));
    let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
    let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),question,"t0").unwrap();
    assert!(!outcome.incomplete);
    assert_eq!(outcome.answer.as_deref(),Some(answer));
    assert_eq!(port.accepted,["select"]);
    let reviews=adapter.seen_reviews.borrow();
    assert_eq!(reviews.len(),1);
    let math=reviews[0]["current_source_evidence"][0]["math_context"].as_array().unwrap();
    let exercise=math.iter().find(|i|i["label"]=="习题 6.3").unwrap();
    assert_eq!(exercise["references"][0]["relation"],"uses");
    assert_eq!(exercise["references"][1]["relation"],"asks_to_prove");
    assert!(math.iter().any(|i|i["label"]=="定理 6.5"), "{}",serde_json::to_string(&outcome.trace).unwrap());
    assert!(adapter.seen_review_instructions.borrow()[0].contains("不能把题目要求当成已给出的证明"));
    assert_eq!(port.reviews[0]["source_evidence"],reviews[0]["current_source_evidence"]);
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
                serde_json::json!({"selected":reference,"delivery_move_refs":references,
                    "delivery_draft":{"delivery_move_refs":references,"text":texts.join("\n\n"),"moves":moves}})
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
    seen_reviews: RefCell<Vec<serde_json::Value>>,
    seen_review_instructions: RefCell<Vec<String>>,
    seen_review_images: RefCell<Vec<Vec<crate::presentation_author::PreviewImage>>>,
}
impl ReviewedTeachingAdapter {
    fn new(chats: Vec<AssistantTurn>, reviews: Vec<Result<serde_json::Value, AdapterError>>) -> Self {
        Self { inner:StreamingTeachingAdapter(RequestPlanRecordingAdapter::new(chats, vec![])),
            reviews:RefCell::new(reviews.into()), seen_reviews:RefCell::new(Vec::new()),
            seen_review_instructions:RefCell::new(Vec::new()), seen_review_images:RefCell::new(Vec::new()) }
    }
}
impl ModelAdapter for ReviewedTeachingAdapter {
    fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, AdapterError> { self.inner.complete(request) }
    fn complete_structured(&self, request: CompletionRequest) -> Result<serde_json::Value, AdapterError> {
        if request.system.starts_with("Teaching delivery acceptance review.") {
            assert_eq!(request.output_token_limit, Some(32_768), "reasoning must leave room for the verdict");
            let packet: serde_json::Value = serde_json::from_str(&request.user).unwrap();
            self.seen_reviews.borrow_mut().push(packet.clone());
            self.seen_review_instructions.borrow_mut().push(request.system);
            self.seen_review_images.borrow_mut().push(request.images);
            self.reviews.borrow_mut().pop_front().expect("unexpected teaching review")
                .map(|review|review_fixture(review, &packet))
        } else { self.inner.complete_structured(request) }
    }
    fn complete_structured_observed(&self, request: CompletionRequest, observer: &mut dyn crate::provider_stream::ModelObserver) -> Result<serde_json::Value, AdapterError> {
        let review = request.system.starts_with("Teaching delivery acceptance review.");
        let result = self.complete_structured(request);
        if review { observer.observe(crate::provider_stream::ModelDelta::Usage(crate::provider_stream::ModelUsage {total_tokens:Some(171), ..Default::default()})); }
        result
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> { self.inner.chat(request) }
    fn chat_observed(&self, request: &AgentRequestPlan, observer: &mut dyn crate::provider_stream::ModelObserver) -> Result<AssistantTurn, AdapterError> { self.inner.chat_observed(request,observer) }
}

fn accepted_review() -> Result<serde_json::Value, AdapterError> { Ok(serde_json::json!({"decision":"accept","revision":null,"learner_claims":[]})) }

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
fn exam_observing_answer_is_revised_and_reviewed_without_a_teaching_selection_or_stream_leak() {
    let b = book();
    let mut store = MemoryStore::open(tmp("exam-observing-review")).unwrap();
    let mut reader = Reader::new(&b,DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b,&store,"t0");
    let question = "Explain this route directly, without a diagnostic.";
    let bad = "UNREVIEWED_EXAM_DRAFT";
    let good = "This route selects some retained cards; an omitted card can still be in the catalog.";
    let adapter = ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("read","book.text",r#"{"lid":"1.1"}"#)]),
        turn_final(bad),turn_final(good),
    ],vec![Ok(serde_json::json!({"decision":"revise","revision":"Explain the requested distinction within the current exam scope.","learner_claims":[]})),accepted_review()]);
    let sink = std::sync::Arc::new(TeachingStreamSink::default());
    let mut context = RunContext::new(new_session(),OuterConfig {max_turns:Some(8),..Default::default()},adapter.model_runtime_profile());
    context.events = crate::run_events::RunEvents::new(Some(sink.clone()));
    context.workspace_goal = Some(exam_workspace(3));
    let grounding = serde_json::json!({"expressions":[{"fact_ref":"user:question","text":question}],"interpretations":[],"delivered_support":[]});
    context.tutor = Some(serde_json::json!({"status":"observing","learner_context":{"current_request":question,"learner_grounding":grounding}}));
    let observed = crate::run_events::ObservedAdapter {inner:&adapter,events:context.events.clone(),
        cancellation:context.cancellation.clone(),runtime_profile:context.runtime_profile.clone()};
    let mut port = TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:vec![]};
    let outcome = run_context(&b,&mut port,&observed,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),question,"t0").unwrap();
    assert!(!outcome.incomplete);
    assert_eq!(outcome.answer.as_deref(),Some(good));
    assert_eq!(outcome.turns,5,"three main samplings and two reviews share the turn budget");
    assert!(outcome.tokens_spent>=342);
    assert!(port.accepted.is_empty() && port.selections.is_empty());
    assert!(context.teaching_delivery_draft.is_none() && outcome.teaching_delivery_move_refs.is_empty());
    assert_eq!(port.reviews.len(),2);
    let packets = adapter.seen_reviews.borrow();
    assert_eq!(packets[0]["proposed_delivery"]["text"],bad);
    assert_eq!(packets[1]["proposed_delivery"]["text"],good);
    assert!(packets[0]["current_design"].is_null());
    assert_eq!(packets[0]["proposed_presentations"],serde_json::json!([]));
    assert_eq!(packets[0]["learner_grounding"],grounding);
    assert_eq!(packets[0]["frozen_input"]["workspace_goal"]["revision"],3);
    assert_eq!(packets[0]["frozen_input"]["exam_calendar"]["remaining_days"],1);
    assert_eq!(packets[0]["frozen_input"]["constraints"],serde_json::json!(exam_workspace(3).goal.constraints));
    assert!(!packets[0]["current_source_evidence"].as_array().unwrap().is_empty());
    assert_eq!(packets[1]["review_context"]["prior_reviews"][0]["decision"],"revise");
    let plans = adapter.inner.0.seen_plans.borrow();
    assert!(plans[2].ordered_messages().iter().filter_map(|m|m.content.as_deref()).any(|text|
        text.contains("agent.exam_answer_revision") && text.contains(bad) && text.contains("current exam scope")));
    assert!(!serde_json::to_string(&context.messages).unwrap().contains(bad),"private rejected draft must not become delivered conversation history");
    let patches = serde_json::to_string(&*sink.0.lock().unwrap()).unwrap();
    assert!(patches.contains(good));
    assert!(!patches.contains(bad),"even the first streamed delta must wait for review");
}

#[test]
fn exam_review_does_not_change_non_exam_or_stale_calendar_answers_or_double_review_active_teaching() {
    for case in ["no_exam","stale_calendar","changed_goal","active"] {
        let b = book();
        let mut store = MemoryStore::open(tmp(&format!("exam-review-{case}"))).unwrap();
        let mut reader = Reader::new(&b,DEFAULT_RADIUS);
        let snapshot = default_profile_snapshot(&b,&store,"t0");
        let active = case=="active";
        let chats = if active {vec![turn_calls(vec![call("select","tutor.step",&select_args())]),turn_final("FREE_FINAL_NOT_SELECTED")]}
            else {vec![turn_final("Ordinary answer remains unchanged.")]};
        let adapter = ReviewedTeachingAdapter::new(chats,if active {vec![accepted_review()]} else {vec![]});
        let mut context = RunContext::new(new_session(),OuterConfig {max_turns:Some(6),..Default::default()},adapter.model_runtime_profile());
        let mut workspace = exam_workspace(if case=="stale_calendar" {2} else {3});
        if case=="no_exam" { workspace.exam_calendar=None; }
        if case=="changed_goal" { context.accepted_workspace_goal=Some(base_schema::WorkspaceGoal {revision:4,..workspace.goal.clone()}); }
        context.workspace_goal=Some(workspace);
        context.tutor=Some(if active {serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}})}
            else {serde_json::json!({"status":"observing","learner_context":{}})});
        let mut port = TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:vec![]};
        let outcome = run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"Explain this relation.","t0").unwrap();
        assert!(!outcome.incomplete);
        assert_eq!(adapter.seen_reviews.borrow().len(),usize::from(active));
        assert_eq!(port.accepted,if active {vec!["select"]} else {vec![]});
        if active {
            assert_ne!(outcome.answer.as_deref(),Some("FREE_FINAL_NOT_SELECTED"));
            assert_eq!(adapter.seen_reviews.borrow()[0]["frozen_input"]["exam_calendar"]["workspace_goal_revision"],3);
        } else { assert_eq!(outcome.answer.as_deref(),Some("Ordinary answer remains unchanged.")); }
    }
}

#[test]
fn exam_failed_review_and_exhausted_budget_cannot_generate_or_stream_an_unreviewed_fallback() {
    for case in ["review_failure","no_review_budget","revision_budget","tool_budget","invalid_citation"] {
        let b = book();
        let mut store = MemoryStore::open(tmp(&format!("exam-review-stop-{case}"))).unwrap();
        let mut reader = Reader::new(&b,DEFAULT_RADIUS);
        let snapshot = default_profile_snapshot(&b,&store,"t0");
        let bad = if case=="invalid_citation" {"UNREVIEWED_CITATION [[source:invented]]"} else {"UNREVIEWED_EXAM_FAILURE"};
        let mut chats = if case=="tool_budget" {vec![turn_calls(vec![call("read","book.text",r#"{"lid":"1.1"}"#)])]} else {vec![turn_final(bad)]};
        chats.push(turn_final("FALLBACK_GENERATION_MUST_NOT_RUN"));
        let reviews = match case {
            "review_failure"=>vec![Err(AdapterError {spend_stop:None,message:"Review provider unavailable".into()})],
            "revision_budget"=>vec![Ok(serde_json::json!({"decision":"revise","revision":"Explain the missing relationship.","learner_claims":[]}))],
            "invalid_citation"=>vec![accepted_review()],
            _=>vec![],
        };
        let adapter = ReviewedTeachingAdapter::new(chats,reviews);
        let limit = if matches!(case,"no_review_budget"|"tool_budget") {1} else if case=="revision_budget" {2} else {6};
        let mut context = RunContext::new(new_session(),OuterConfig {max_turns:Some(limit),..Default::default()},adapter.model_runtime_profile());
        let sink = std::sync::Arc::new(TeachingStreamSink::default());
        context.events = crate::run_events::RunEvents::new(Some(sink.clone()));
        context.workspace_goal=Some(exam_workspace(3));
        context.tutor=Some(serde_json::json!({"status":"observing","learner_context":{}}));
        let observed = crate::run_events::ObservedAdapter {inner:&adapter,events:context.events.clone(),
            cancellation:context.cancellation.clone(),runtime_profile:context.runtime_profile.clone()};
        let mut port = TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:vec![]};
        let outcome = run_context(&b,&mut port,&observed,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"Explain 1.1 directly.","t0").unwrap();
        assert!(outcome.incomplete,"{case}");
        assert_eq!(adapter.inner.0.seen_plans.borrow().len(),1,"{case}: no extra finalization, repair, or fallback generation");
        assert!(!outcome.answer.as_deref().unwrap().contains("UNREVIEWED"));
        assert!(port.accepted.is_empty());
        let patches = serde_json::to_string(&*sink.0.lock().unwrap()).unwrap();
        assert!(!patches.contains("UNREVIEWED") && !patches.contains("FALLBACK_GENERATION"),"{case}");
        if case=="tool_budget" { assert!(!outcome.source_bindings.is_empty(),"observed original text remains available"); }
    }
}

#[test]
fn teaching_writing_rules_reach_each_enabled_main_request_once() {
    let b = book();
    let profile = ModelRuntimeProfile::fallback("shared-writing", ProviderToolProtocol::Native);
    let writing = include_str!("tutor_writing_rules.md");
    for status in ["active", "observing", "reference", "paused", "inactive"] {
        let tutor = serde_json::json!({"status":status,"learner_context":{"current_request":"Explain this relation."}});
        let registry = resident_tool_registry().with_tutor_context(Some(&tutor));
        let mut exposure = ToolExposureState::default();
        exposure.activate_tutor();
        let messages = vec![Message::system(crate::tutor::instructions(&tutor)), Message::user("Explain this relation.")];
        let (_, plan) = build_sample_request(&messages, &mut ContextFragmentLedger::default(), &b,
            &mut ActiveToolResultLedger::default(), None, COMPACTION_CONSUMPTION_WRAPPER, &registry, &profile,
            ToolPermissions::default(), &exposure, ArtifactExposureContext::no_overlay(), EvidenceState::UserProvided,
            &[], None, false, Some(&tutor), None).unwrap();
        let enabled = usize::from(matches!(status, "active" | "observing" | "reference"));
        let occurrences: usize = plan.ordered_messages().iter().filter_map(|message|message.content.as_deref())
            .map(|content|content.matches(writing).count()).sum();
        assert_eq!(occurrences, enabled, "{status}: the shared body must reach the actual main request once");
        assert_eq!(plan.instruction_assets.iter().filter(|asset|asset.asset_id == "resident-agent.tutor.writing").count(), enabled);
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
    assert_eq!(adapter.seen_reviews.borrow().as_slice(), &[packet]);
    assert!(adapter.seen_review_images.borrow()[0].is_empty());
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
    let question = packet["frozen_input"]["current_request"].as_str()
        .or_else(||packet["frozen_input"]["learner_request"].as_str())
        .or_else(||packet["frozen_input"]["current_action"]["accepted_request"].as_str())
        .or_else(||packet["original_request"].as_str()).unwrap();
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
    review
}

#[test]
fn teaching_review_fixture_fills_only_missing_relation_fields() {
    let packet=serde_json::json!({"original_request":"Explain the interval.","proposed_delivery":{"text":"The end time minus the start time is the interval."}});
    let explicit=serde_json::json!({"decision":"accept","revision":null,"findings":[],
        "request_resolution":{"status":"unresolved","question_quote":"Explain the interval.","reason":"The actual answer is missing.",
            "requested_relation":"ORIGINAL_REQUESTED_RELATION","delivered_relation":"ORIGINAL_DELIVERED_RELATION","answer_quotes":[]}});
    assert_eq!(review_fixture(explicit.clone(),&packet),explicit);
    let filled=review_fixture(serde_json::json!({"decision":"accept","revision":null}),&packet);
    assert_eq!(filled["request_resolution"]["answer_quotes"],serde_json::json!([packet["proposed_delivery"]["text"]]));
    assert!(filled["request_resolution"]["requested_relation"].as_str().unwrap().contains("Explain the interval."));
}

#[test]
fn teaching_grounding_rejects_review_citations_to_delivery_or_invented_words_before_selection() {
    for failure in ["delivery", "invented_quote", "missing_basis", "missing_claims"] {
        let b = book();
        let mut store = MemoryStore::open(tmp(&format!("teaching-grounding-{failure}"))).unwrap();
        let mut reader = Reader::new(&b, DEFAULT_RADIUS);
        let snapshot = default_profile_snapshot(&b, &store, "t0");
        let bad = selected_body("unsupported", "You already understand that vectors only order candidates.");
        let good = selected_body("explanation", "A document's short retrieval text is converted to a vector. The query is converted too, and their comparison helps retrieve candidate documents.");
        let mut claim = serde_json::json!({"claim":"The learner already understands the role of vectors.",
            "basis":[{"fact_ref":"user:question","quote":"How is Embedding used?"}]});
        match failure {
            "delivery" => claim["basis"][0]["fact_ref"] = serde_json::json!("delivery:page"),
            "invented_quote" => claim["basis"][0]["quote"] = serde_json::json!("I understand vectors now."),
            "missing_basis" => claim["basis"] = serde_json::json!([]),
            _ => {},
        }
        let mut verdict = serde_json::json!({"decision":"accept","revision":null,"learner_claims":[claim]});
        if failure == "missing_claims" { verdict.as_object_mut().unwrap().remove("learner_claims"); }
        let adapter = ReviewedTeachingAdapter::new(vec![
            turn_calls(vec![call("bad", "tutor.step", &bad.to_string())]),
            turn_calls(vec![call("good", "tutor.step", &good.to_string())]),
            turn_final("Use the accepted explanation."),
        ], vec![Ok(verdict), accepted_review()]);
        let grounding = serde_json::json!({"expressions":[{"fact_ref":"user:question","text":"How is Embedding used?"}],
            "interpretations":[],"delivered_support":[{"delivery_ref":"delivery:page","answer":"Vectors only order candidates."}]});
        let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
        context.tutor = Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},
            "learner_context":{"learner_grounding":grounding}}));
        let mut port = TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
        let outcome = run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"How is Embedding used?","t0").unwrap();
        assert_eq!(port.accepted,["select"],"a reviewer cannot authorize a learner claim using a delivery or fabricated words");
        assert!(!port.selections.contains_key("selected:unsupported"));
        assert_eq!(outcome.answer.as_deref(),good["move"]["delivery_text"].as_str());
        assert_eq!(adapter.seen_reviews.borrow()[0]["learner_grounding"],grounding);
        assert!(tool_result(&context.messages,"bad").contains("TEACHING_INVALID"));
    }
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
    context.tutor = Some(serde_json::json!({"status":"active","learner_context":{"current_request":"FROZEN_INPUT_STAYS_ONCE"}}));
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
        context.tutor = Some(if outside { serde_json::json!({"status":"active","learner_context":{}}) } else { serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}) });
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
        let resolved = ResolvedInteraction { action, learning_purpose:None, user_supplement:String::new(), accepted_request:"这一步没跟上".into(), answer_ref:None, answer_text:None, source_excerpt:None, source_bindings:Vec::new(), target:None, expected_chat_head:None, presentation_follow_up:None };
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
    context.tutor = Some(serde_json::json!({"status":"active","learner_context":{
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
        assert_eq!(current.matches("A retained card can serve different selections.").count(),1);
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
    context.tutor = Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
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
                "source_turn_ref":"turn:prior","guidance":{"sentinel":"private-coach-guidance"}})))
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
    context.tutor = Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
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
    assert!(serde_json::to_string(&plans[0].ordered_messages()).unwrap().contains("private-coach-guidance"));
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
            "design" => turns.push(turn_calls(vec![call("redesign", "tutor.step", &serde_json::json!({"operation":"design","operation_id":"revised","design":design()}).to_string())])),
            "outside" => turns.push(turn_calls(vec![call("outside", "tutor.step", r#"{"operation":"outside"}"#)])),
            _ => {},
        }
        if change != "outside" { turns.push(turn_calls(vec![call("replace", "tutor.step", &replacement.to_string())])); }
        turns.push(turn_final("ORDINARY_FINAL_TEXT"));
        let adapter = RequestPlanRecordingAdapter::new(turns, vec![]);
        let mut context = RunContext::new(new_session(), OuterConfig { max_turns:Some(8), ..Default::default() }, adapter.model_runtime_profile());
        context.tutor = Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
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
    let mut context = RunContext::new(new_session(), OuterConfig { max_turns:Some(2), ..Default::default() }, adapter.model_runtime_profile());
    context.events = crate::run_events::RunEvents::new(Some(sink.clone()));
    context.tutor = Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
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
fn teaching_review_editorial_advice_reaches_author_without_forcing_another_review() {
    for adopt in [false, true] {
        let b=book();
        let mut store=MemoryStore::open(tmp(if adopt {"teaching-advice-adopt"} else {"teaching-advice-keep"})).unwrap();
        let mut reader=Reader::new(&b,DEFAULT_RADIUS);
        let snapshot=default_profile_snapshot(&b,&store,"t0");
        let first=selected_body("first","The catalog keeps A and B. This route uses A.");
        let improved=selected_body("improved","This route uses A; B remains in the catalog for another route.");
        let advice=serde_json::json!({"location":"opening comparison","evidence":"The catalog keeps A and B. This route uses A.",
            "learning_impact":"Keeping B in the same sentence reduces the need to remember the earlier list.",
            "suggested_change":"Place the route's selection and B's retained status together."});
        let mut reviews=vec![Ok(serde_json::json!({"decision":"accept","revision":null,"learner_claims":[],
            "reading_path":{"actual_sequence":"Read the catalog, then compare this route.",
                "understanding_gap":"The comparison requires remembering the earlier list.",
                "proposed_adjustment":"Show the two statuses together."},"improvements":[advice]}))];
        let mut turns=vec![turn_calls(vec![call("first","tutor.step",&first.to_string())])];
        if adopt {
            turns.push(turn_calls(vec![call("improved","tutor.step",&improved.to_string())]));
            reviews.push(accepted_review());
        }
        turns.push(turn_final("FINAL_MUST_USE_SELECTED_BODY"));
        let adapter=ReviewedTeachingAdapter::new(turns,reviews);
        let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(8),..Default::default()},adapter.model_runtime_profile());
        context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
        let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:vec![]};
        let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"Does B remain available when this route uses only A?","t0").unwrap();
        assert_eq!(port.reviews.len(),if adopt {2} else {1},"advice cannot force a review loop");
        assert_eq!(port.reviews[0]["review"]["decision"],"accept");
        assert!(port.reviews[0]["review"]["revision"].is_null());
        let plans=adapter.inner.0.seen_plans.borrow();
        let receipt:serde_json::Value=serde_json::from_str(plans[1].input.iter()
            .find(|message|message.tool_call_id.as_deref()==Some("first")).unwrap().content.as_deref().unwrap()).unwrap();
        assert_eq!(receipt["model_body"]["editorial_feedback"]["improvements"][0],advice);
        assert!(receipt["model_body"].get("delivery_draft").is_none());
        assert!(receipt["model_body"]["editorial_feedback"].get("agent_work").is_none());
        assert_eq!(outcome.answer.as_deref(),if adopt {improved["move"]["delivery_text"].as_str()} else {first["move"]["delivery_text"].as_str()});
        if adopt {
            assert_eq!(adapter.seen_reviews.borrow()[1]["review_context"]["prior_reviews"][0]["improvements"][0],advice);
        }
    }
}

#[test]
fn teaching_review_rejects_before_host_selection_and_delivers_only_the_revision() {
    let b = book();
    let mut store = MemoryStore::open(tmp("teaching-review-revision")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let rejected = selected_body("broad", "UNACCEPTED_BRANCHES_AND_COUNTERFACTUALS");
    let revised = selected_body("focused", "The route leaves B out, while the retained catalog still has B.");
    let adapter = ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("broad", "tutor.step", &rejected.to_string())]),
        turn_calls(vec![call("focused", "tutor.step", &revised.to_string())]),
        turn_final("FREE_FINAL_MUST_NOT_ADD_THE_COUNTERFACTUAL"),
    ], vec![Ok(serde_json::json!({"decision":"accept","revision":null,
        "request_resolution":{"status":"unresolved","question_quote":"I can follow B in only one list.",
            "reason":"Keep the catalog and this route fixed; the answer has not supplied the requested distinction, so omit the later changed-condition case."},
        "findings":[],"agent_work":{"understanding":[{"statement":"This draft's private memory has an invalid citation.",
            "sources":[{"source_id":b.base.book_id,"source_revision":b.source_fingerprint(),"lid":"1.9","quote":"Not read at this position."}]}],"open_questions":[]}})), accepted_review()]);
    let sink = std::sync::Arc::new(TeachingStreamSink::default());
    let mut context = RunContext::new(new_session(), OuterConfig {max_turns:Some(8), ..Default::default()}, adapter.model_runtime_profile());
    context.events = crate::run_events::RunEvents::new(Some(sink.clone()));
    let mut chapter_design = design();
    chapter_design["organizing_model"] = serde_json::json!("Why retain a card that this route leaves out? The learner currently sees only one list.");
    let support_move = serde_json::json!({"move_id":"help","design_ref":"design:current","block_refs":["keep-select"],
        "object_ids":["catalog"],"kind":"explain","interaction_intent":"direct_explanation",
        "intent_origin":"current_request","intent_quote":"I can follow B in only one list.","preference_ref":null,
        "responds_to":{"fact_refs":["usage:before"]},"interpretation_refs":[],"presentation":null});
    let mut historical_move = support_move.clone();
    historical_move["prompt"] = serde_json::json!("UNDELIVERED_OLD_PLAN");
    historical_move["target"] = serde_json::json!({"expected_performance":"ALREADY_MASTERED_PLAN"});
    historical_move["explanation"] = serde_json::json!("PRIVATE_WORKED_SOLUTION");
    let grounding = serde_json::json!({"expressions":[
        {"fact_ref":"usage:before","text":"B is gone when omitted.","support_delivery_refs":[]},
        {"fact_ref":"usage:after","text":"I can follow B in only one list.","support_delivery_refs":["delivered:help"],
            "assistance_refs":["help:before-input"],"scene":{"visible_step":"Only route A"},"presentations":[{"presentation_id":"old-page"}]}],
        "interpretations":[{"evidence_ref":"reading:before","fact_refs":["usage:before"],"interpretation":"A revisable reading."}],
        "delivered_support":[{"delivery_ref":"delivered:help","kind":"message_delivered","move":historical_move,
            "answer":"The catalog preserves the cards.","text":"The route selects some cards.",
            "scene":{"observed_result":"B remains in the catalog."},"presentation":{"presentation_id":"old-page"},
            "presentations":[{"readable_content":"Catalog: A, B. Route: A."}],"feedback":{"text":"This was assisted."}}],
        "unknown_when_absent":true,"unavailable_basis_refs":[]});
    let workflow = serde_json::json!({"pending_response_invitation":{"event_id":"invitation:prior"},
        "pending_activity":{"activity_ref":"activity:prior"},"pending_assessment_refs":["assessment:prior"],
        "learning_opportunity":{"design_ref":"design:current","focus_block_refs":["keep-select"],
            "return_context":{"open_question_refs":["usage:before"],"scene":{"visible_step":"Earlier catalog"}}}});
    let mut learner = workflow.clone();
    learner["current_request"] = serde_json::json!("I can follow B in only one list.");
    learner["recent_facts"] = serde_json::json!([{"text":"OLD_TRACE_COPY"}]);
    learner["current_target"] = serde_json::json!({"expected_performance":"OLD_TARGET_COPY"});
    learner["learner_grounding"] = grounding.clone();
    learner["learning_opportunity"]["delivered_support"] = serde_json::json!([traced_learning_support()]);
    context.tutor = Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":chapter_design},
        "chapter_scope":{"source_id":"source","chapter_lid":"1","title":"Retained cards and routes"},
        "material_scope":[{"source_id":"source","scope_refs":["1"],"role":"primary"}],
        "learner_context":learner}));
    let mut port = TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader}, accepted:Vec::new(),selections:Default::default(),reviews:Vec::new()};
    let outcome = run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),"I can follow B in only one list.","t0").unwrap();
    assert_eq!(port.accepted,["select"],"the rejected selection must never reach the host");
    assert!(!port.selections.contains_key("selected:broad"));
    assert_eq!(port.reviews.len(),2);
    assert_eq!(port.reviews[0]["review"]["reported_decision"],"accept");
    assert_eq!(port.reviews[0]["agent_work_warnings"].as_array().unwrap().len(),1);
    assert!(port.reviews[0]["review"]["agent_work"]["understanding"].as_array().unwrap().is_empty());
    assert_eq!(port.reviews[0]["review"]["decision"],"revise",
        "an admitted unresolved request must be persisted and routed as a revision before host selection");
    assert_eq!(port.reviews[1]["review"]["decision"],"accept");
    assert_eq!(outcome.answer.as_deref(),revised["move"]["delivery_text"].as_str());
    assert_eq!(outcome.teaching_delivery_move_refs,["selected:focused"]);
    assert_eq!(outcome.turns,5,"both isolated review calls consume the model-turn budget");
    assert!(outcome.tokens_spent>=342,"the two review usage snapshots must be included");
    let requests = adapter.seen_reviews.borrow();
    assert_eq!(requests.len(),2);
    assert_eq!(requests[0]["original_request"],"I can follow B in only one list.");
    assert_eq!(requests[0]["learner_view"],workflow,"review keeps workflow context without another learner/help history");
    let mut expected_grounding = grounding.clone();
    expected_grounding["delivered_support"][0]["move"] = support_move;
    assert_eq!(requests[0]["learner_grounding"],expected_grounding,
        "one canonical projection preserves original words, input-time help, scenes and associations verbatim");
    assert!(requests[0]["frozen_input"].get("recent_facts").is_none());
    for omitted in ["UNDELIVERED_OLD_PLAN","ALREADY_MASTERED_PLAN","PRIVATE_WORKED_SOLUTION","OLD_TRACE_COPY","OLD_TARGET_COPY"] {
        assert!(!requests[0].to_string().contains(omitted),"{omitted}");
    }
    assert_eq!(context.tutor.as_ref().unwrap()["learner_context"]["learner_grounding"],grounding,
        "review projection must not rewrite stored or working learner data");
    assert_eq!(requests[0]["frozen_input"]["chapter_scope"]["chapter_lid"],"1");
    assert_eq!(requests[0]["frozen_input"]["material_scope"][0]["scope_refs"],serde_json::json!(["1"]));
    assert_eq!(requests[0]["current_design"]["design"]["organizing_model"],chapter_design["organizing_model"]);
    assert_eq!(requests[0]["current_design"]["design"]["blocks"][0]["purpose"],chapter_design["blocks"][0]["purpose"]);
    let review_context = &requests[1]["review_context"];
    assert_eq!(review_context["round"],2);
    assert!(review_context["prior_reviews"][0]["revision"].as_str().unwrap().contains("Keep the catalog and this route fixed"));
    let change = &review_context["changes_since_previous_review"][0];
    assert_eq!(change["path"],"delivery.text");
    assert_eq!(change["before"],rejected["move"]["delivery_text"]);
    assert_eq!(change["after"],revised["move"]["delivery_text"]);
    assert!(review_context["prior_reviews"][0].get("proposed_delivery").is_none(),"prior packets must not be repeated");
    let public = serde_json::to_string(&context.messages).unwrap();
    assert!(public.contains("Keep the catalog and this route fixed"),"the next selection must receive the concrete revision");
    assert!(!public.contains("UNACCEPTED_BRANCHES_AND_COUNTERFACTUALS"));
    let patches = serde_json::to_string(&*sink.0.lock().unwrap()).unwrap();
    assert!(patches.contains("retained catalog still has B"));
    assert!(!patches.contains("UNACCEPTED_BRANCHES") && !patches.contains("FREE_FINAL_MUST_NOT"));
}

#[test]
fn teaching_review_real_revisions_after_bad_arguments_keep_making_progress() {
    let b=book();
    let mut store=MemoryStore::open(tmp("teaching-review-real-progress")).unwrap();
    let mut reader=Reader::new(&b,DEFAULT_RADIUS);
    let snapshot=default_profile_snapshot(&b,&store,"t0");
    let texts = ["The page includes B at setting 3, and keeps nine cards.",
        "The page includes B at setting 2, and keeps nine cards.",
        "The page includes B at setting 2, and keeps ten cards.",
        "At setting 2 the route includes B; the catalog keeps all ten cards."];
    let mut chats=vec![turn_calls(vec![call("missing-operation","tutor.step",r#"{}"#)])];
    for (index,text) in texts.iter().enumerate() {
        chats.push(turn_calls(vec![call(&format!("draft-{index}"),"tutor.step",&selected_body("same-move",text).to_string())]));
    }
    chats.push(turn_final("Use the revised delivery."));
    let revisions=["1. Setting 2 already includes B. 2. The catalog has ten cards, not nine.",
        "The setting is fixed. The catalog count remains nine; correct it to ten.",
        "The new phrase mixes the selected route with the retained catalog. Name them separately."];
    let mut reviews=revisions.iter().map(|revision|Ok(serde_json::json!({"decision":"revise","revision":revision}))).collect::<Vec<_>>();
    reviews.push(accepted_review());
    let adapter=ReviewedTeachingAdapter::new(chats,reviews);
    let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(12),..Default::default()},adapter.model_runtime_profile());
    context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
    let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),"Explain the page's actual settings.","t0").unwrap();
    assert!(!outcome.incomplete);
    assert_eq!(outcome.answer.as_deref(),Some(texts[3]));
    assert_eq!(port.accepted,["select"]);
    assert!(tool_result(&context.messages,"missing-operation").contains("TOOL_ARGUMENTS_INVALID"));
    let reviewed=adapter.seen_reviews.borrow();
    assert_eq!(reviewed.len(),4,"each actual revision must reach review after the argument error");
    assert_eq!(reviewed[3]["review_context"]["prior_reviews"].as_array().unwrap().len(),3);
    assert_eq!(reviewed[1]["review_context"]["changes_since_previous_review"][0]["before"],"3");
    assert_eq!(reviewed[1]["review_context"]["changes_since_previous_review"][0]["after"],"2");
    assert!(reviewed[1]["review_context"]["changes_since_previous_review"][0]["context_before"].as_str().unwrap().contains("setting"));
}

#[test]
fn teaching_review_renamed_and_cycled_drafts_do_not_restart_progress() {
    let b=book();
    let mut store=MemoryStore::open(tmp("teaching-review-repeated-content")).unwrap();
    let mut reader=Reader::new(&b,DEFAULT_RADIUS);
    let snapshot=default_profile_snapshot(&b,&store,"t0");
    let mut chats=Vec::new();
    for (index,text) in ["Original incorrect page claim.","Revised but still incorrect page claim.",
        "Original incorrect page claim.","Original incorrect page claim.","Original incorrect page claim."].iter().enumerate() {
        let mut selection=selected_body(&format!("new-id-{index}"),text);
        selection["move"]["prompt"]=serde_json::json!(format!("Different private explanation {index}"));
        chats.push(turn_calls(vec![call(&format!("draft-{index}"),"tutor.step",&selection.to_string())]));
    }
    let adapter=ReviewedTeachingAdapter::new(chats,vec![
        Ok(serde_json::json!({"decision":"revise","revision":"Correct the original page claim."})),
        Ok(serde_json::json!({"decision":"revise","revision":"The revised claim still contradicts the inspected state."})),
        Ok(serde_json::json!({"decision":"revise","revision":"The original claim still contradicts the inspected state."})),
        Ok(serde_json::json!({"decision":"revise","revision":"The original claim is unchanged."})),
        Ok(serde_json::json!({"decision":"revise","revision":"The original claim is still unchanged."})),
    ]);
    let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(20),..Default::default()},adapter.model_runtime_profile());
    context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
    let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),"Explain the actual page.","t0").unwrap();
    assert!(outcome.incomplete);
    assert!(port.accepted.is_empty());
    assert_eq!(adapter.seen_reviews.borrow().len(),5,"revision history must reach the reviewer until the no-progress guard stops unchanged work");
    assert!(tool_result(&context.messages,"draft-4").contains("still unchanged"));
    assert!(outcome.turns<20,"unchanged drafts must terminate before exhausting the model budget");
}

#[test]
fn teaching_review_same_draft_after_source_read_receives_the_new_evidence() {
    let b=book();
    let mut store=MemoryStore::open(tmp("teaching-review-new-source-evidence")).unwrap();
    let mut reader=Reader::new(&b,DEFAULT_RADIUS);
    let snapshot=default_profile_snapshot(&b,&store,"t0");
    let selection=selected_body("same-draft","The retained catalog contains the card omitted from this route.");
    let adapter=ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("before-read","tutor.step",&selection.to_string())]),
        turn_calls(vec![call("read","book.text",r#"{"lid":"1.1"}"#)]),
        turn_calls(vec![call("after-read","tutor.step",&selection.to_string())]),
        turn_final("Use the supported delivery."),
    ],vec![Ok(serde_json::json!({"decision":"revise","revision":"Supply the actual source evidence for the retained card before accepting this claim."})),accepted_review()]);
    let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(12),..Default::default()},adapter.model_runtime_profile());
    context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
    let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),"Read the source and explain the retained card.","t0").unwrap();
    assert!(!outcome.incomplete,"{outcome:?}");
    assert_eq!(port.accepted,["select"]);
    let reviews=adapter.seen_reviews.borrow();
    assert_eq!(reviews.len(),2);
    assert_eq!(reviews[0]["proposed_delivery"],reviews[1]["proposed_delivery"]);
    assert!(reviews[0]["current_source_evidence"].as_array().unwrap().is_empty());
    assert_eq!(reviews[1]["current_source_evidence"].as_array().unwrap().len(),1);
    assert_eq!(reviews[1]["current_source_evidence"][0]["segments"][0]["text"],"X".repeat(100));
    assert!(reviews[1]["presented_source_references"].as_array().unwrap().is_empty(),"reading alone must supply the review, without a citation-card operation");
    assert_eq!(reviews[1]["review_context"]["changes_since_previous_review"],serde_json::json!([]));
    assert!(reviews[1]["review_context"]["prior_reviews"][0]["revision"].as_str().unwrap().contains("source evidence"));
}

#[test]
fn teaching_review_checks_the_explicit_retained_set_even_after_a_replacement() {
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
    context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:Vec::new(),selections:Default::default(),reviews:Vec::new()};
    let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),"I can follow only the first premise.","t0").unwrap();
    let reviews=adapter.seen_reviews.borrow();
    assert_eq!(reviews[1]["proposed_delivery"]["text"],"SUPERSEDED_SECOND_SCOPE");
    assert_eq!(reviews[2]["proposed_delivery"]["text"],"FIRST_PREMISE\n\nTHIRD_SMALL_CONNECTION");
    assert_eq!(reviews[2]["proposed_delivery"]["delivery_move_refs"],serde_json::json!(["selected:first"]));
    assert_eq!(reviews[2]["proposed_delivery"]["moves"].as_array().unwrap().len(),2);
    assert_eq!(outcome.answer.as_deref(),Some("FIRST_PREMISE\n\nTHIRD_SMALL_CONNECTION"));
    assert_eq!(outcome.teaching_delivery_move_refs,["selected:first","selected:third"]);
}

#[test]
fn teaching_preflight_missing_read_recovers_before_one_review_of_the_normalized_move() {
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
    context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
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
    let reviews=adapter.seen_reviews.borrow();
    assert_eq!(reviews.len(),1,"deterministic rejection must precede the model review");
    let reviewed=&reviews[0]["proposed_delivery"]["moves"][0];
    assert_eq!(reviewed["interaction_intent"],"direct_explanation");
    assert_eq!(reviewed["intent_origin"],"current_action");
    assert_eq!(reviewed["target"],design()["target"]);
    let accepted=&port.inner.selections["selected:source-repaired"];
    for field in ["interaction_intent","intent_origin","target","delivery_text"] {
        assert_eq!(accepted[field],reviewed[field],"review and commit must consume the same normalized {field}");
    }
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
    context.tutor=Some(serde_json::json!({"status":"active","learner_context":{}}));
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
    assert_eq!(adapter.seen_reviews.borrow().len(),1);
}

#[test]
fn teaching_review_failure_unknown_reference_and_missing_budget_never_select_the_candidate() {
    for failure in ["provider","malformed","budget","unknown_reference"] {
        let b=book();
        let mut store=MemoryStore::open(tmp(&format!("teaching-review-{failure}"))).unwrap();
        let mut reader=Reader::new(&b,DEFAULT_RADIUS);
        let snapshot=default_profile_snapshot(&b,&store,"t0");
        let mut selection=selected_body("unreviewed","UNREVIEWED_CANDIDATE_MUST_STAY_PRIVATE");
        if failure=="unknown_reference" { selection["delivery_move_refs"]=serde_json::json!(["move:not-accepted"]); }
        let reviews=match failure {
            "provider"=>vec![Err(AdapterError {spend_stop:None,message:"review provider unavailable".into()})],
            "malformed"=>vec![Ok(serde_json::json!({"decision":"revise","revision":""}))],
            _=>vec![],
        };
        let adapter=ReviewedTeachingAdapter::new(vec![
            turn_calls(vec![call("candidate","tutor.step",&selection.to_string())]),
            turn_calls(vec![call("outside","tutor.step",r#"{"operation":"outside"}"#)]),
            turn_final("Ordinary response after leaving this teaching attempt."),
        ],reviews);
        let sink=std::sync::Arc::new(TeachingStreamSink::default());
        let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(if failure=="budget" {1} else {8}),..Default::default()},adapter.model_runtime_profile());
        context.events=crate::run_events::RunEvents::new(Some(sink.clone()));
        context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
        let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:Vec::new(),selections:Default::default(),reviews:Vec::new()};
        let result=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"Only this fragment is clear.","t0");
        let outcome=result.unwrap();
        assert!(outcome.teaching_delivery_move_refs.is_empty());
        if failure=="budget" {
            assert!(outcome.incomplete);
            assert_eq!(outcome.warning.as_deref(),Some("TEACHING_INVALID"));
            assert!(outcome.answer.as_deref().is_some_and(|answer|!answer.trim().is_empty()));
        }
        if failure=="malformed" { assert!(outcome.tokens_spent>=171,"malformed verdict usage is still counted"); }
        assert!(!outcome.answer.as_deref().unwrap_or_default().contains("UNREVIEWED_CANDIDATE"));
        assert!(!port.accepted.iter().any(|operation|operation=="select"),"{failure}");
        assert!(port.selections.is_empty() && context.accepted_teaching_moves.is_empty());
        assert!(context.teaching_delivery_draft.is_none());
        assert_eq!(adapter.seen_reviews.borrow().len(),usize::from(matches!(failure,"provider"|"malformed")));
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
        context.tutor=Some(serde_json::json!({"status":"active","current_design":current.map(|design|serde_json::json!({"design_ref":"design:current","design":design})),"learner_context":{}}));
        let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
        run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"I can follow B in one list, but lose it in the other.","t0").unwrap();
        let plans=adapter.seen_plans.borrow();
        assert_eq!(plans[usize::from(create)].tools.iter().any(|tool|tool.name=="presentation.author"),label!="text");
        if create { assert!(!plans[0].tools.iter().any(|tool|tool.name=="presentation.author")); }
        if label!="text" {
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
        context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":presentation_design()},
            "learner_context":{"learning_opportunity":{"delivered_support":if delivered {serde_json::json!([{"design_ref":"design:current","presentation":{"presentation_id":"prior","revision":1}}])} else {serde_json::json!([])}}}}));
        context.delivered_presentations.push(crate::presentation::PresentationRef {presentation_id:"saved-only".into(),revision:1});
        let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
        let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"I think B stays, but I'm unsure why.","t0").unwrap();
        assert_eq!(port.accepted.iter().any(|operation|operation=="select"),delivered);
        assert_eq!(adapter.seen_reviews.borrow().len(),usize::from(delivered));
        if delivered { assert!(!outcome.answer_view.unwrap().parts.iter().any(|part|matches!(part,AgentAnswerPart::Presentation {..})),"feedback need not repeat or regenerate the page"); }
    }
}

#[test]
fn teaching_page_review_reads_saved_semantics_and_only_accepted_revision_is_attached() {
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
            selection["move"]["presentation"]=serde_json::json!({"presentation_id":"cards","revision":revision});
            turns.push(turn_calls(vec![call(&format!("select-{revision}"),"tutor.step",&selection.to_string())]));
        }
        turns.push(turn_final("FREE_FINAL_MUST_NOT_REINTRODUCE_THE_OLD_PAGE"));
        let adapter=ReviewedTeachingAdapter::new(turns,vec![Ok(serde_json::json!({"decision":"revise","revision":"The page changes two conditions at once. Keep one route fixed and revise the page."})),accepted_review()]);
        let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(if budget {10} else {16}),..Default::default()},adapter.model_runtime_profile());
        context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":presentation_design()},"learner_context":{}}));
        let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
        let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"B seems gone from this route, but I can't follow where it went.","t0").unwrap();
        assert_eq!(port.accepted,["select"]);
        let page_refs:Vec<_>=outcome.answer_view.unwrap().parts.into_iter().filter_map(|part|match part {AgentAnswerPart::Presentation {presentation_id,revision}=>Some((presentation_id,revision)),_=>None}).collect();
        assert_eq!(page_refs,[("cards".into(),2)],"a saved but rejected page must not appear in either finalization path");
        assert!(!outcome.incomplete);
        assert_eq!(context.delivered_presentations.len(),2,"both versions really were saved before selection");
        let plans=adapter.inner.0.seen_plans.borrow();
        let saved=tool_result(&plans[3].input,"save-1");
        assert!(saved.contains("tutor.step") && saved.contains("operation select") && saved.contains("cards"),
            "the delivered-page receipt must direct the next sampling to teaching selection: {saved}");
        let rejected=tool_result(&plans[4].input,"select-1");
        assert!(rejected.contains("Keep one route fixed") && rejected.contains("index.html") && rejected.contains("cards"),
            "the concrete review must be preserved with an exact source-read handoff: {rejected}");
        let reviews=adapter.seen_reviews.borrow();
        let review_images=adapter.seen_review_images.borrow();
        assert!(plans[3].preview_images.is_empty(), "the author already consumed its image queue before select");
        for (index,revision) in [(0,1),(1,2)] {
            let page=&reviews[index]["proposed_presentations"][0];
            assert_eq!(page["reference"]["revision"],revision);
            assert_eq!(page["author_claims"]["readable_content"],format!("SAVED_PAGE_BODY_{revision}"));
            assert_eq!(page["author_claims"]["initial_state"]["route_selected"],true);
            assert_eq!(page["teaching_binding"]["regions"][0]["region_id"],"catalog");
            assert_eq!(page["author_preview_observations"][0]["observations"][0]["dom"]["text"],format!("ACTUAL_PREVIEW_c{revision}"));
            assert_eq!(review_images[index].len(),1);
            let image=&review_images[index][0];
            assert_eq!(image.png_base64,format!("png-c{revision}"));
            assert_eq!(image.candidate_id.as_deref(),Some(format!("c{revision}").as_str()));
            assert!(image.caption.contains(&format!("\"revision\":{revision}")) && image.caption.contains("cards"));
            assert!(image.caption.contains("after switching this route off"));
        }
        let changes=reviews[1]["review_context"]["changes_since_previous_review"].as_array().unwrap();
        assert!(changes.iter().any(|change|change["path"]=="delivery.presentations[0].readable_content"
            && change["before"]=="1" && change["after"]=="2"),"page changes must come from the saved page, not the teacher's description");
        assert!(!changes.iter().any(|change|change["path"].as_str().unwrap().contains("reference")),"a version ID alone is not a content revision");
        if budget { assert!(adapter.inner.0.seen_plans.borrow().last().unwrap().tools.is_empty()); }
    }
}

#[test]
fn historical_design_requires_a_live_representation_choice_before_generation() {
    let mut old=design(); old.as_object_mut().unwrap().remove("representation");
    let tutor=serde_json::json!({"status":"active","current_design":{"design_ref":"old","design":old}});
    let error=require_sampled_teaching_design(Some(&tutor),Some("old"),Some("old")).unwrap_err();
    assert!(error.message.contains("representation"));
}

#[test]
fn teaching_review_provider_retry_keeps_the_draft_and_real_failure() {
    for recover in [false, true] {
        let b=book();
        let mut store=MemoryStore::open(tmp(&format!("teaching-review-provider-retry-{recover}"))).unwrap();
        let mut reader=Reader::new(&b,DEFAULT_RADIUS);
        let snapshot=default_profile_snapshot(&b,&store,"t0");
        let select=selected_body("embedding","UNREVIEWED_EMBEDDING_TEXT");
        let mut chats=vec![turn_calls(vec![call("first","tutor.step",&select.to_string())]),
            turn_calls(vec![call("retry","tutor.step",&select.to_string())])];
        if recover { chats.push(turn_final("Use the accepted draft.")); }
        let failed=|| Err(AdapterError {spend_stop:None,message:"模型达到单次输出上限（finish_reason=length，completion_tokens=4096，reasoning_tokens=4096）".into()});
        let adapter=ReviewedTeachingAdapter::new(chats,vec![failed(),if recover {accepted_review()} else {failed()}]);
        let sink=std::sync::Arc::new(TeachingStreamSink::default());
        let mut context=RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
        context.events=crate::run_events::RunEvents::new(Some(sink.clone()));
        context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
        let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
        let result=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"Embedding具体会被怎么用呢，给我一个演示页看看","t0");
        assert_eq!(adapter.seen_reviews.borrow().len(),2);
        assert_eq!(adapter.seen_reviews.borrow()[0]["proposed_delivery"],adapter.seen_reviews.borrow()[1]["proposed_delivery"]);
        if recover {
            assert_eq!(result.unwrap().answer.as_deref(),Some("UNREVIEWED_EMBEDDING_TEXT"));
            assert_eq!(port.accepted,["select"]);
        } else {
            let outcome=result.unwrap();
            assert!(outcome.incomplete);
            assert_eq!(outcome.warning.as_deref(),Some("TEACHING_REVIEW_FAILED"));
            assert!(outcome.teaching_delivery_move_refs.is_empty());
            assert!(outcome.answer.as_deref().is_some_and(|answer|!answer.trim().is_empty() && !answer.contains("UNREVIEWED_EMBEDDING_TEXT")));
            let failure=tool_result(&context.messages,"first");
            assert!(failure.contains("PROVIDER_ERROR") && failure.contains("finish_reason=length"));
            assert!(port.accepted.is_empty());
            assert!(!serde_json::to_string(&*sink.0.lock().unwrap()).unwrap().contains("UNREVIEWED_EMBEDDING_TEXT"));
            let plans=adapter.inner.0.seen_plans.borrow();
            let fallback=plans.last().unwrap();
            assert!(fallback.tools.is_empty());
            assert!(!serde_json::to_string(&fallback.ordered_messages()).unwrap().contains("UNREVIEWED_EMBEDDING_TEXT"),
                "an unaccepted private draft must not become fallback input");
        }
    }
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
        let adapter=ReviewedTeachingAdapter::new(turns,if repair {vec![accepted_review()]} else {vec![]});
        let mut context=RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
        context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":presentation_design()},"learner_context":{}}));
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
            assert_eq!(outcome.warning.as_deref(),Some("PRESENTATION_ARGUMENTS_INVALID"));
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
        let adapter=ReviewedTeachingAdapter::new(turns,if replaced {vec![]} else {vec![accepted_review()]});
        let mut context=RunContext::new(new_session(),OuterConfig {max_turns:if replaced {Some(1)} else {None},..Default::default()},adapter.model_runtime_profile());
        context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":if replaced {"design:new"} else {"design:current"},"design":presentation_design()},"learner_context":{}}));
        let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:Vec::new()};
        let result=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"Fix the page layout and show the same relationship.","t0");
        if replaced {
            let outcome=result.unwrap();
            assert!(outcome.incomplete);
            assert_eq!(outcome.warning.as_deref(),Some("TEACHING_DESIGN_REQUIRES_NEXT_SAMPLING"));
            assert!(outcome.teaching_delivery_move_refs.is_empty());
            assert!(outcome.answer.as_deref().is_some_and(|answer|!answer.trim().is_empty()));
            assert!(!outcome.answer_view.unwrap().parts.iter().any(|part|matches!(part,AgentAnswerPart::Presentation {..})));
            assert!(port.accepted.is_empty());
        } else {
            let outcome=result.unwrap();
            assert!(!outcome.incomplete);
            assert_eq!(context.presentation_teaching_designs.get("c2").map(String::as_str),Some("design:current"));
            assert!(outcome.answer_view.unwrap().parts.iter().any(|p|matches!(p,AgentAnswerPart::Presentation {revision:2,..})));
        }
    }
}
