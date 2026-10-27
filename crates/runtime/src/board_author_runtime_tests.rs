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
                reviewed_delivery, ..
            } => {
                assert!(
                    reviewed_delivery.is_some(),
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
        context.board_reviewed_candidate_id = Some(if changed_diagram { "board:run:0" } else { "board:run:1" }.into());
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
    context.board_reviewed_candidate_id = Some("board:run:1".into());
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
