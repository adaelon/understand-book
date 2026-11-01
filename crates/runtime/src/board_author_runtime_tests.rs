use super::*;
use serde_json::{json, Value};
use std::collections::VecDeque;

struct ReadProgressPort {
    store: MemoryStore,
    reader: Reader,
    results: VecDeque<Value>,
    reads: usize,
}
impl ResidentStatePort for ReadProgressPort {
    fn board_authoring_available(&self) -> bool { true }
    fn workspace_records_available(&self) -> bool { true }
    fn author_board(&mut self, _: crate::board_author::AuthorRequest, _: &[SourceBinding],
        _: &[Message], _: &crate::run_context::CancellationToken) -> Result<crate::board_author::AuthorResult, ToolError> {
        self.reads += 1;
        Ok(crate::board_author::AuthorResult::value(self.results.pop_front().unwrap()))
    }
    fn workspace_records(&mut self, request: Value) -> Result<Value, ToolError> {
        if request["operation"] == "list" { return Ok(json!({"records":[],"next":null})); }
        self.reads += 1;
        Ok(self.results.pop_front().unwrap())
    }
    fn submit_private<R>(&mut self, f: impl FnOnce(&mut MemoryStore) -> R) -> Result<R, ToolError> { Ok(f(&mut self.store)) }
    fn read_live_reader<R>(&mut self, f: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> { Ok(f(&self.reader)) }
    fn apply_reader<R>(&mut self, f: impl FnOnce(&mut MemoryStore, &mut Reader) -> R) -> Result<R, ToolError> { Ok(f(&mut self.store, &mut self.reader)) }
    fn reader_input(&mut self, book: &Book, question: &str) -> crate::run_context::ReaderInputSnapshot {
        crate::run_context::ReaderInputSnapshot::capture(book, &self.reader, question)
    }
}

#[test]
fn board_and_workspace_distinct_reads_continue_past_the_stalled_batch_threshold() {
    for scenario in ["catalog", "source_read", "pages", "workspace_records"] {
        let b = book();
        let mut requests = Vec::new();
        let mut results = VecDeque::new();
        for index in 0..4 {
            let source = json!({"kind":"answer","session_id":"original-chat","turn_id":format!("turn-{index}")});
            let (tool, args, result) = match scenario {
                "catalog" => ("board.author", json!({"operation":"catalog","kind":"answer","after":index,"limit":1}),
                    json!({"status":"board_catalog","book_id":"book","published_book_ref":null,"items":[{"source":source}],"total":4,"next":index+1})),
                "source_read" => ("board.author", json!({"operation":"source_read","source":source}),
                    json!({"status":"board_source","source":source,"published_book_ref":null,"item":{"readable_content":format!("original answer {index}")}})),
                "pages" => {
                    let collection = if index < 2 { "layout" } else { "items" };
                    let offset = index % 2;
                    ("board.author", json!({"operation":"read","collection":collection,"board_revision":1,"offset":offset,"length":1}),
                        json!({"status":"board_page","board_id":"board-read","board_revision":1,"candidate_revision":0,
                            "collection":collection,"offset":offset,"length":1,"total":2,"records":[{"item_id":format!("item-{offset}")}],"next_calls":[]}))
                }
                _ => ("workspace.records.read", json!({"operation":"read","chat_session_id":"original-chat","after":index,"limit":1}),
                    json!({"learning_workspace_id":"workspace","selection":{"chat_session_id":"original-chat","through_seq":8},
                        "records":[{"kind":"chat_turn","record_id":format!("chat_turn:turn-{index}"),"chat_session_id":"original-chat","through_seq":8}],"next":index+1})),
            };
            requests.push(turn_calls(vec![call(&format!("read-{index}"), tool, &args.to_string())]));
            results.push_back(result);
        }
        requests.push(turn_final("不同原始记录已经读到，接下来可以整理。"));
        let adapter = RequestPlanRecordingAdapter::new(requests, vec![]);
        let mut port = ReadProgressPort { store: MemoryStore::open(tmp(&format!("board-read-progress-{scenario}"))).unwrap(),
            reader: Reader::new(&b, 1), results, reads: 0 };
        let snapshot = default_profile_snapshot(&b, &port.store, "t0");
        let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
        let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(),
            None, &mut EphemeralCompactionCheckpointSink::default(), "整理画板和原始学习记录", "t0").unwrap();
        assert_eq!(port.reads, 4, "{scenario}: distinct reads were stopped before all records arrived");
        assert!(!outcome.incomplete, "{scenario}: {:?}", outcome.warning);
        assert_eq!(context.progress_ledger.read_observations.len(), 4, "{scenario}");
    }
}

#[test]
fn board_and_workspace_repeated_observations_do_not_claim_progress() {
    let mut ledger = TurnProgressLedger::default();
    let observations = [
        ("board.author", json!({"operation":"catalog","kind":"answer"}), json!({"status":"board_catalog","book_id":"book","items":[{"source":{"kind":"answer","session_id":"chat","turn_id":"turn"}}]})),
        ("board.author", json!({"operation":"source_read","source":{"kind":"answer","session_id":"chat","turn_id":"turn"}}), json!({"status":"board_source","source":{"kind":"answer","session_id":"chat","turn_id":"turn"},"item":{"item_id":"temporary-1","readable_content":"original"}})),
        ("board.author", json!({"operation":"read","collection":"layout","offset":0,"length":1}), json!({"status":"board_page","board_id":"board","board_revision":1,"candidate_revision":0,"collection":"layout","offset":0,"records":[{"item_id":"item"}]})),
        ("workspace.records.read", json!({"operation":"read","chat_session_id":"chat","after":0,"limit":1}), json!({"selection":{"chat_session_id":"chat","through_seq":8},"records":[{"record_id":"chat_turn:turn","chat_session_id":"chat","through_seq":8}]})),
    ];
    for (tool, args, body) in observations {
        ledger.observe_read(tool, &args.to_string(), &body.to_string());
        let before = ledger.clone();
        let mut repeated = args.clone();
        // Different page-size/default spelling, same delivered records.
        if args["operation"] == "catalog" || tool == "workspace.records.read" { repeated["limit"] = json!(20); }
        if tool == "board.author" && args["operation"] == "read" { repeated["length"] = json!(20); }
        let mut repeated_body = body.clone();
        if repeated_body["item"].is_object() { repeated_body["item"]["item_id"] = json!("temporary-2"); }
        ledger.observe_read(tool, &repeated.to_string(), &repeated_body.to_string());
        assert_eq!(ledger, before, "{tool}: rereading the same source/page is not progress");
    }
    let before = ledger.clone();
    for body in [json!({"error_code":"READ_FAILED"}), json!({"projection":"omitted"})] {
        for tool in ["board.author", "workspace.records.read"] {
            ledger.observe_read(tool, r#"{"operation":"read"}"#, &body.to_string());
        }
    }
    ledger.observe_read("board.author", r#"{"operation":"read"}"#, &json!({"status":"board_read_budget","records":[]}).to_string());
    assert_eq!(ledger, before, "failed or undisclosed records are not progress");
}

struct Port {
    store: MemoryStore,
    reader: Reader,
    candidate_text: String,
    deliveries: usize,
    commits: usize,
}

struct ReferenceRepairPort {
    inner: Port,
    attempts: usize,
    correction_fails: bool,
}
impl ResidentStatePort for ReferenceRepairPort {
    fn board_authoring_available(&self)->bool {true}
    fn author_board(&mut self, request:crate::board_author::AuthorRequest, bindings:&[SourceBinding], messages:&[Message], cancellation:&crate::run_context::CancellationToken)
        ->Result<crate::board_author::AuthorResult,ToolError> {
        match request {
            crate::board_author::AuthorRequest::Prepare {..} => {
                self.attempts+=1;
                if self.attempts==1 {return Err(crate::board_author::error("BOARD_AUTHORING_INVALID","Explanation failed the ordinary answer compiler: UNKNOWN_SOURCE_REF source_ref_typo"))}
                if self.attempts==2 || self.correction_fails {return Err(crate::board_author::error("INVALID_BOARD",
                    "The referenced board item is unavailable. Available in the proposed board: {\"item_id\":\"item-board-turn_run\",\"block_id\":\"example\",\"content_revision\":14}. Use this exact content reference for that content."))}
                Ok(crate::board_author::AuthorResult::value(json!({"status":"candidate_prepared","candidate_id":"board:run:1","candidate_revision":1})))
            },
            crate::board_author::AuthorRequest::Preview {..} => {
                let mut result=crate::board_author::AuthorResult::value(json!({"status":"candidate_previewed","candidate_id":"board:run:1","candidate_revision":1}));
                result.previewed_candidate=Some("board:run:1".into());
                result.images.push(crate::presentation_author::PreviewImage{caption:"Native preview fixture".into(),png_base64:"png".into(),candidate_id:Some("board:run:1".into()),environment_name:Some("fixture".into())});
                Ok(result)
            },
            other=>self.inner.author_board(other,bindings,messages,cancellation),
        }
    }
    fn submit_private<R>(&mut self,f:impl FnOnce(&mut MemoryStore)->R)->Result<R,ToolError>{self.inner.submit_private(f)}
    fn read_live_reader<R>(&mut self,f:impl FnOnce(&Reader)->R)->Result<R,ToolError>{self.inner.read_live_reader(f)}
    fn apply_reader<R>(&mut self,f:impl FnOnce(&mut MemoryStore,&mut Reader)->R)->Result<R,ToolError>{self.inner.apply_reader(f)}
    fn reader_input(&mut self,book:&Book,question:&str)->crate::run_context::ReaderInputSnapshot{self.inner.reader_input(book,question)}
}

#[test]
fn native_board_reference_repair_gets_one_recovery_after_source_correction_then_commits_or_stops() {
    for correction_fails in [false,true] {
        let b=book();
        let prepare=|source:&str,item_id:&str|json!({"operation":"prepare","base_board_revision":13,
            "blocks":[{"block_id":"example","markdown":format!("Example [[source:{source}]]")}],"drawings":[],
            "operations":[{"kind":"insert_topic","parent_item_id":"mutable","index":0,"topic":{"item_id":"example-topic","title":"s[0] = 99",
                "based_on":[{"item_id":item_id,"block_id":"example","content_revision":14}]}}]});
        let mut steps=vec![
            turn_calls(vec![call("source-typo","board.author",&prepare("source_ref_typo","item-board-turn-run").to_string())]),
            turn_calls(vec![call("source-corrected","board.author",&prepare("source_ref_exact","item-board-turn-run").to_string())]),
            turn_calls(vec![call("reference-repair","board.author",&prepare("source_ref_exact",if correction_fails {"item-board-turn-run-again"} else {"item-board-turn_run"}).to_string())]),
        ];
        if !correction_fails {
            steps.push(turn_calls(vec![call("preview","board.author",r#"{"operation":"preview","candidate_revision":1}"#)]));
            steps.push(turn_calls(vec![call("commit","board.author",r#"{"operation":"commit","candidate_revision":1}"#)]));
        }
        steps.push(turn_final(if correction_fails {"The example was not applied."} else {"The example has been saved."}));
        let adapter=RequestPlanRecordingAdapter::new(steps,vec![]);
        let mut port=ReferenceRepairPort{inner:Port{store:MemoryStore::open(tmp(&format!("board-reference-repair-{correction_fails}"))).unwrap(),reader:Reader::new(&b,1),candidate_text:"The checked example.".into(),deliveries:0,commits:0},attempts:0,correction_fails};
        let snapshot=default_profile_snapshot(&b,&port.inner.store,"t0");
        let mut context=RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
        // The request already consumed its expression-coach handoff, exactly
        // as in replay-06. The test exercises the remaining tool protocol.
        context.native_board_coach=Some(json!({"status":"ready"}));
        let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"Add the example without changing the current title","t0").unwrap();
        assert_eq!(port.attempts,3,"the first exact reference repair must reach the host");
        assert!(tool_result(&context.messages,"source-typo").contains("UNKNOWN_SOURCE_REF"));
        assert!(tool_result(&context.messages,"source-corrected").contains("INVALID_BOARD"));
        assert_eq!(port.inner.commits,usize::from(!correction_fails));
        assert_eq!(outcome.incomplete,correction_fails);
        if correction_fails {
            assert_eq!(outcome.warning.as_deref(),Some("AGENT_NO_PROGRESS"),"another invalid reference cannot renew the recovery budget");
            assert_eq!(adapter.seen_plans.borrow().len(),4,"stop after the one correction and its honest finalization");
        } else {
            assert!(context.inspected_presentations.contains("board:run:1"),"the successful repair still needs a preview sampled before commit");
            assert!(tool_result(&context.messages,"commit").contains("board_part_committed"));
        }
    }
}

#[test]
fn native_board_conflict_finishes_with_host_notice_without_selecting_unreviewed_tutor_content() {
    for scenario in ["preview", "read", "page"] {
        let b = book();
        let conflict = json!({"current_board_revision":10,"item_ids":["mutable"],"message":"The reader renamed this topic."});
        let (request, result) = match scenario {
            "preview" => (json!({"operation":"preview","candidate_revision":1}),
                json!({"status":"candidate_pending_changes","candidate_id":"board:run:1","candidate_revision":1,"conflict":conflict})),
            "read" => (json!({"operation":"read"}),
                json!({"status":"board_read","candidate_id":"board:run:1","candidate":{"conflict":conflict}})),
            _ => (json!({"operation":"read","collection":"items","offset":0,"length":10}),
                json!({"status":"board_page","candidate_id":"board:run:1","candidate_conflict":conflict,"records":[]})),
        };
        let adapter = RequestPlanRecordingAdapter::new(vec![turn_calls(vec![call("check-current", "board.author", &request.to_string())])],vec![]);
        let mut port = ReadProgressPort {store:MemoryStore::open(tmp(&format!("board-conflict-notice-{scenario}"))).unwrap(),reader:Reader::new(&b,1),results:VecDeque::from([result]),reads:0};
        let snapshot = default_profile_snapshot(&b,&port.store,"t0");
        let mut context = RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
        context.tutor=Some(json!({"status":"active","current_design":{"design_ref":"design:unreviewed"}}));
        context.board_candidate_id=Some("board:run:1".into());
        context.teaching_delivery_draft=Some(json!({"text":"UNREVIEWED candidate explanation","moves":[]}));
        context.teaching_delivery_move_refs=vec!["unreviewed-move".into()];
        let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"Rename this branch and add an example","t0").unwrap();
        assert_eq!(port.reads,1,"the persisted conflict must not prompt another authoring attempt");
        let text=outcome.answer.as_deref().unwrap();
        assert!(text.contains("尚未应用") && text.contains("按当前内容重新生成"));
        assert!(!text.contains("UNREVIEWED"));
        assert!(!outcome.incomplete);
        assert!(outcome.teaching_delivery_move_refs.is_empty());
        assert!(!context.tutor_resolved,"the operational notice does not claim teaching approval");
        assert!(context.board_delivery_draft.is_none());
        assert!(context.board_candidate_id.is_none());
    }
}

impl ResidentStatePort for Port {
    fn board_authoring_available(&self) -> bool {
        true
    }
    fn author_board(
        &mut self,
        request: crate::board_author::AuthorRequest,
        _: &[SourceBinding],
        _: &[Message],
        _: &crate::run_context::CancellationToken,
    ) -> Result<crate::board_author::AuthorResult, ToolError> {
        let mut result = crate::board_author::AuthorResult::value(serde_json::Value::Null);
        match request {
            crate::board_author::AuthorRequest::Read { .. } => {
                result.body = serde_json::json!({
                "candidate":{"candidate_revision":1,"delivery_text":self.candidate_text}})
            }
            crate::board_author::AuthorRequest::Deliver { .. } => {
                self.deliveries += 1;
                result.body = serde_json::json!({"status":"delivery_staged","candidate_id":"board:run:1","candidate_revision":1});
                result.delivery_text = Some(self.candidate_text.clone());
            }
            crate::board_author::AuthorRequest::Commit {
                selected_delivery, ..
            } => {
                assert!(
                    selected_delivery.is_some(),
                    "the runtime supplies the review after its guards"
                );
                self.commits += 1;
                let delivery = memory::boards::BoardDelivery {
                    board_id: "board-test".into(),
                    commit_id: Some("board:run:1".into()),
                    before_revision: 1,
                    after_revision: 2,
                    item_ids: vec!["part-one".into()],
                    block_ids: vec!["block-one".into()],
                    origin_session_id: "session".into(),
                    origin_turn_id: "run".into(),
                    parent_item_id: Some("condition".into()),
                };
                result.body = serde_json::json!({"status":"board_part_committed","candidate_revision":1,"candidate_id":"board:run:1","delivery":delivery});
                result.committed = Some(delivery);
            }
            _ => panic!("unexpected author operation"),
        }
        Ok(result)
    }
    fn submit_private<R>(&mut self, f: impl FnOnce(&mut MemoryStore) -> R) -> Result<R, ToolError> {
        Ok(f(&mut self.store))
    }
    fn read_live_reader<R>(&mut self, f: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> {
        Ok(f(&self.reader))
    }
    fn apply_reader<R>(
        &mut self,
        f: impl FnOnce(&mut MemoryStore, &mut Reader) -> R,
    ) -> Result<R, ToolError> {
        Ok(f(&mut self.store, &mut self.reader))
    }
    fn reader_input(
        &mut self,
        book: &Book,
        question: &str,
    ) -> crate::run_context::ReaderInputSnapshot {
        crate::run_context::ReaderInputSnapshot::capture(book, &self.reader, question)
    }
}

#[test]
fn native_delivery_cannot_replace_reviewed_teaching_text_or_stage_before_that_check() {
    let accepted = "This is the accepted explanation, kept with its actual evidence.";
    for (changed, changed_diagram) in [(false, false), (true, false), (false, true)] {
        let b = book();
        let mut port = Port {
            store: MemoryStore::open(tmp("board-teaching-contract")).unwrap(),
            reader: Reader::new(&b, 1),
            candidate_text: if changed {
                "This replacement was never selected or reviewed."
            } else {
                accepted
            }
            .into(),
            deliveries: 0,
            commits: 0,
        };
        let adapter = RequestPlanRecordingAdapter::new(
            vec![
                turn_calls(vec![call(
                    "native-deliver",
                    "board.author",
                    r#"{"operation":"deliver","candidate_revision":1}"#,
                )]),
                turn_final(accepted),
            ],
            vec![],
        );
        let snapshot = default_profile_snapshot(&b, &port.store, "t0");
        let mut context = RunContext::new(
            new_session(),
            OuterConfig::default(),
            adapter.model_runtime_profile(),
        );
        context.tutor = Some(
            serde_json::json!({"status":"active","current_design":{"design_ref":"design:reviewed"}}),
        );
        context.tutor_resolved = true;
        context.teaching_delivery_draft = Some(serde_json::json!({"text":accepted,"moves":[]}));
        // This test starts after a real preview has been sampled; the browser
        // acceptance separately exercises creation and image observation.
        context.board_candidate_id = Some("board:run:1".into());
        context.board_selected_candidate_id = Some(if changed_diagram { "board:run:0" } else { "board:run:1" }.into());
        context.inspected_presentations.insert("board:run:1".into());
        let outcome = run_context(
            &b,
            &mut port,
            &adapter,
            &mut context,
            &snapshot,
            &ResidentTurnResources::default(),
            None,
            &mut EphemeralCompactionCheckpointSink::default(),
            "Use the reviewed explanation on this board",
            "t0",
        );
        assert_eq!(
            port.deliveries,
            usize::from(!changed && !changed_diagram),
            "a changed draft must be rejected before host staging"
        );
        if changed || changed_diagram {
            let error = outcome.unwrap_err();
            assert_eq!(error.error_code, if changed_diagram { "BOARD_TEACHING_SELECTION_REQUIRED" } else { "BOARD_TEACHING_TEXT_MISMATCH" });
            assert!(error.message.contains("未完成"), "A rejected board may not finish as plain text");
            assert!(context
                .messages
                .iter()
                .any(
                    |message| message.tool_call_id.as_deref() == Some("native-deliver")
                        && message
                            .content
                            .as_deref()
                            .is_some_and(|text| text.contains(if changed_diagram { "BOARD_TEACHING_SELECTION_REQUIRED" } else { "BOARD_TEACHING_TEXT_MISMATCH" }))
                ));
            assert!(context.board_delivery_draft.is_none());
        } else {
            assert_eq!(outcome.unwrap().answer.as_deref(), Some(accepted));
        }
    }
}

#[test]
fn native_commit_keeps_the_model_running_and_never_becomes_the_final_draft() {
    let b = book();
    let mut port = Port {
        store: MemoryStore::open(tmp("board-local-commit")).unwrap(),
        reader: Reader::new(&b, 1),
        candidate_text: "The first complete local explanation.".into(),
        deliveries: 0,
        commits: 0,
    };
    let adapter = RequestPlanRecordingAdapter::new(
        vec![
            turn_calls(vec![call(
                "commit-one",
                "board.author",
                r#"{"operation":"commit","candidate_revision":1}"#,
            )]),
            turn_final("The first part is saved; this is the later final response."),
        ],
        vec![],
    );
    let snapshot = default_profile_snapshot(&b, &port.store, "t0");
    let mut context = RunContext::new(
        new_session(),
        OuterConfig::default(),
        adapter.model_runtime_profile(),
    );
    context.board_candidate_id = Some("board:run:1".into());
    context.inspected_presentations.insert("board:run:1".into());
    let outcome = run_context(
        &b,
        &mut port,
        &adapter,
        &mut context,
        &snapshot,
        &ResidentTurnResources::default(),
        None,
        &mut EphemeralCompactionCheckpointSink::default(),
        "Explain this in useful board parts",
        "t0",
    ).unwrap();
    assert_eq!(port.commits, 1);
    assert_eq!(port.deliveries, 0);
    assert!(context.board_delivery_draft.is_none());
    assert_eq!(
        outcome.answer.as_deref(),
        Some("The first part is saved; this is the later final response.")
    );
    assert!(
        outcome.turns >= 2,
        "commit must return to a later model sampling"
    );
}

#[test]
fn native_commit_requires_reviewed_local_scope_before_host_side_effects() {
    let accepted = "The selected complete teaching explanation.";
    let b = book();
    let mut port = Port {
        store: MemoryStore::open(tmp("board-commit-scope")).unwrap(),
        reader: Reader::new(&b, 1),
        candidate_text: accepted.into(),
        deliveries: 0,
        commits: 0,
    };
    let adapter = RequestPlanRecordingAdapter::new(
        vec![
            turn_calls(vec![call(
                "commit-with-final-scope",
                "board.author",
                r#"{"operation":"commit","candidate_revision":1}"#,
            )]),
            turn_final(accepted),
        ],
        vec![],
    );
    let snapshot = default_profile_snapshot(&b, &port.store, "t0");
    let mut context = RunContext::new(
        new_session(),
        OuterConfig::default(),
        adapter.model_runtime_profile(),
    );
    context.tutor = Some(
        serde_json::json!({"status":"active","current_design":{"design_ref":"design:reviewed"}}),
    );
    context.tutor_resolved = true;
    context.teaching_delivery_draft = Some(serde_json::json!({"text":accepted,"moves":[]}));
    context.board_candidate_id = Some("board:run:1".into());
    context.board_selected_candidate_id = Some("board:run:1".into());
    context.inspected_presentations.insert("board:run:1".into());
    let outcome = run_context(
        &b,
        &mut port,
        &adapter,
        &mut context,
        &snapshot,
        &ResidentTurnResources::default(),
        None,
        &mut EphemeralCompactionCheckpointSink::default(),
        "Explain this on the board",
        "t0",
    );
    assert_eq!(
        port.commits, 0,
        "a whole-request selection cannot silently become an earlier local delivery"
    );
    let error = outcome.unwrap_err();
    assert_eq!(error.error_code, "BOARD_PART_SELECTION_REQUIRED");
    assert!(error.message.contains("未完成"));
    assert!(context
        .messages
        .iter()
        .any(
            |message| message.tool_call_id.as_deref() == Some("commit-with-final-scope")
                && message
                    .content
                    .as_deref()
                    .is_some_and(|body| body.contains("BOARD_PART_SELECTION_REQUIRED"))
        ));
}

#[test]
fn stale_native_draft_is_excluded_after_teaching_selection_changes() {
    let adapter = RequestPlanRecordingAdapter::new(vec![], vec![]);
    let mut context = RunContext::new(
        new_session(),
        OuterConfig::default(),
        adapter.model_runtime_profile(),
    );
    context.tutor = Some(serde_json::json!({"status":"active"}));
    context.tutor_resolved = true;
    context.teaching_delivery_draft =
        Some(serde_json::json!({"text":"Corrected teaching interpretation"}));
    context.board_delivery_draft = Some("Earlier board interpretation".into());
    assert!(context.accepted_board_text().is_none(), "normal finalization and failure fallback must preserve the current reviewed teaching draft");
    context.board_delivery_draft = Some("Corrected teaching interpretation".into());
    assert_eq!(
        context.accepted_board_text(),
        Some("Corrected teaching interpretation")
    );
    context.tutor_resolved = false;
    assert!(
        context.accepted_board_text().is_none(),
        "a later unfinished teaching revision removes delivery eligibility"
    );
}
