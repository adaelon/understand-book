use super::*;
use serde_json::{json, Value};

struct PreviewPort {
    store: MemoryStore,
    reader: Reader,
    previews: usize,
    deliveries: usize,
    omit_first_preview: bool,
}

impl PreviewPort {
    fn new(book: &Book, name: &str) -> Self {
        Self { store: MemoryStore::open(tmp(name)).unwrap(), reader: Reader::new(book, 1),
            previews: 0, deliveries: 0, omit_first_preview: false }
    }

    fn images(&mut self, candidate_id: &str) -> Vec<crate::presentation_author::PreviewImage> {
        self.previews += 1;
        let count = if self.omit_first_preview && self.previews == 1 { 256 } else { 1 };
        (0..count).map(|index| crate::presentation_author::PreviewImage {
            caption: format!("Actual candidate preview {index}"), png_base64: "png".into(),
            candidate_id: Some(candidate_id.into()), environment_name: Some("desktop".into()),
        }).collect()
    }
}

impl ResidentStatePort for PreviewPort {
    fn board_authoring_available(&self) -> bool { true }
    fn author_board(&mut self, request: crate::board_author::AuthorRequest, _: &[SourceBinding],
        _: &[Message], _: &crate::run_context::CancellationToken) -> Result<crate::board_author::AuthorResult, ToolError> {
        use crate::board_author::{AuthorRequest, AuthorResult};
        match request {
            AuthorRequest::Preview { candidate_revision: 1, .. } => {
                let mut result = AuthorResult::value(json!({"status":"preview_ready_for_inspection",
                    "candidate_id":"board:run:1","candidate_revision":1}));
                result.previewed_candidate = Some("board:run:1".into());
                result.images = self.images("board:run:1");
                result.invalidates_delivery = true;
                Ok(result)
            }
            AuthorRequest::Read { .. } => Ok(AuthorResult::value(json!({"candidate":{
                "candidate_id":"board:run:1","candidate_revision":1,"delivery_text":"The inspected board explanation."}}))),
            AuthorRequest::Deliver { candidate_revision: 1 } => {
                self.deliveries += 1;
                let mut result = AuthorResult::value(json!({"status":"delivery_staged",
                    "candidate_id":"board:run:1","candidate_revision":1}));
                result.delivery_text = Some("The inspected board explanation.".into());
                Ok(result)
            }
            _ => panic!("unexpected board operation"),
        }
    }
    fn author_presentation(&mut self, request: crate::presentation_author::AuthorRequest, _: &[SourceBinding],
        _: &[Message], _: &crate::run_context::CancellationToken) -> Result<crate::presentation_author::AuthorResult, ToolError> {
        use crate::presentation_author::{AuthorRequest, AuthorResult};
        match request {
            AuthorRequest::Preview { candidate_id, .. } => Ok(AuthorResult {
                body: json!({"status":"preview_ready_for_inspection","candidate_id":candidate_id}),
                images: self.images(&candidate_id), previewed_candidate: Some(candidate_id), delivered: None,
            }),
            AuthorRequest::Deliver { .. } => {
                self.deliveries += 1;
                let reference = crate::presentation::PresentationRef { presentation_id:"recovered-page".into(), revision:1 };
                Ok(AuthorResult { body:json!({"status":"version_saved","reference":reference}), images:vec![],
                    previewed_candidate:None, delivered:Some(reference) })
            }
            _ => panic!("unexpected presentation operation"),
        }
    }
    fn submit_private<R>(&mut self, f: impl FnOnce(&mut MemoryStore) -> R) -> Result<R, ToolError> { Ok(f(&mut self.store)) }
    fn read_live_reader<R>(&mut self, f: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> { Ok(f(&self.reader)) }
    fn apply_reader<R>(&mut self, f: impl FnOnce(&mut MemoryStore, &mut Reader) -> R) -> Result<R, ToolError> { Ok(f(&mut self.store, &mut self.reader)) }
    fn reader_input(&mut self, book: &Book, question: &str) -> crate::run_context::ReaderInputSnapshot {
        crate::run_context::ReaderInputSnapshot::capture(book, &self.reader, question)
    }
}

fn execute_preview_run(book: &Book, port: &mut PreviewPort, adapter: &RequestPlanRecordingAdapter,
    context: &mut RunContext) -> Result<OuterOutcome, ToolError> {
    let snapshot = default_profile_snapshot(book, &port.store, "t0");
    run_context(book, port, adapter, context, &snapshot, &ResidentTurnResources::default(),
        None, &mut EphemeralCompactionCheckpointSink::default(), "Show the explanation on the board", "t0")
}

#[test]
fn native_preview_at_turn_limit_uses_board_delivery_for_non_teaching_states() {
    for state in ["none", "paused", "completed", "observing", "reference", "outside"] {
        let b = book();
        let mut port = PreviewPort::new(&b, &format!("native-preview-grace-{state}"));
        let adapter = RequestPlanRecordingAdapter::new(vec![
            turn_calls(vec![call("preview", "board.author", r#"{"operation":"preview","candidate_revision":1}"#)]),
            turn_calls(vec![call("deliver", "board.author", r#"{"operation":"deliver","candidate_revision":1}"#)]),
        ], vec![]);
        let mut context = RunContext::new(new_session(), OuterConfig { max_turns:Some(1), ..Default::default() }, adapter.model_runtime_profile());
        if state != "none" {
            context.tutor = Some(if state == "outside" { json!({"status":"active","outside_current_request":true}) }
                else { json!({"status":state}) });
            context.tutor_resolved = true;
        }
        let outcome = execute_preview_run(&b, &mut port, &adapter, &mut context).unwrap();
        assert_eq!(port.deliveries, 1, "{state}");
        assert!(!outcome.incomplete, "{state}");
        assert_eq!(outcome.turns, 2, "one reserved delivery sample: {state}");
        assert_eq!(outcome.answer.as_deref(), Some("The inspected board explanation."));
        let plans = adapter.seen_plans.borrow();
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[1].tools.iter().map(|tool| tool.name.as_str()).collect::<Vec<_>>(), ["board.author"]);
        assert_eq!(plans[1].tools[0].parameters["properties"]["candidate_revision"]["enum"], json!([1]));
    }
}

#[test]
fn native_board_distinct_viewports_progress_but_identical_previews_remain_bounded() {
    for distinct in [true, false] {
        let b = book();
        let mut port = PreviewPort::new(&b, &format!("native-viewport-progress-{distinct}"));
        let mut chats = (0..4).map(|index| turn_calls(vec![call(&format!("preview-{index}"), "board.author",
            &json!({"operation":"preview","candidate_revision":1,"viewport":{
                "width":if distinct {400+index*200} else {400},"height":600,"input":"mouse"}}).to_string())])).collect::<Vec<_>>();
        chats.push(turn_calls(vec![call("deliver", "board.author", r#"{"operation":"deliver","candidate_revision":1}"#)]));
        let adapter = RequestPlanRecordingAdapter::new(chats, vec![]);
        let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
        let outcome = execute_preview_run(&b, &mut port, &adapter, &mut context);
        if distinct {
            assert!(!outcome.unwrap().incomplete);
            assert_eq!((port.previews, port.deliveries), (4, 1));
        } else {
            assert!(outcome.is_err(), "Repeated observations must not claim new work");
            assert!(port.previews < 4);
            assert_eq!(port.deliveries, 0);
        }
    }
}

#[test]
fn omitted_preview_can_be_observed_before_retrying_the_same_delivery() {
    for native in [true, false] {
        let b = book();
        let mut port = PreviewPort::new(&b, &format!("omitted-preview-recovery-{native}"));
        port.omit_first_preview = true;
        let (tool, preview, deliver) = if native {
            ("board.author", json!({"operation":"preview","candidate_revision":1}), json!({"operation":"deliver","candidate_revision":1}))
        } else {
            ("presentation.author", json!({"operation":"preview","candidate_id":"candidate"}), json!({"operation":"deliver","candidate_id":"candidate"}))
        };
        let mut chats = vec![];
        if !native {
            chats.push(turn_calls(vec![call("discover", "tool.search", r#"{"task":"interactive HTML","required_capabilities":["presentation_authoring"],"scope":"passage","operation":"explain","effect_mode":"read_only","max_results":1}"#)]));
        }
        chats.extend([
            turn_calls(vec![call("preview", tool, &preview.to_string())]),
            turn_calls(vec![call("unseen-deliver", tool, &deliver.to_string())]),
            turn_calls(vec![call("preview-again", tool, &preview.to_string())]),
            turn_calls(vec![call("observed-deliver", tool, &deliver.to_string())]),
            turn_final("The inspected explanation is saved."),
        ]);
        let adapter = RequestPlanRecordingAdapter::new(chats, vec![]);
        let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
        let outcome = execute_preview_run(&b, &mut port, &adapter, &mut context).unwrap();
        assert!(!outcome.incomplete, "native={native}: {:?}", outcome.warning);
        assert_eq!((port.previews, port.deliveries), (2, 1), "native={native}");
        let feedback: Value = serde_json::from_str(context.messages.iter()
            .find(|message| message.tool_call_id.as_deref() == Some("unseen-deliver"))
            .and_then(|message| message.content.as_deref()).unwrap()).unwrap();
        assert!(feedback.to_string().contains(if native { "BOARD_INSPECTION_REQUIRED" } else { "PRESENTATION_INSPECTION_REQUIRED" }));
        let plans = adapter.seen_plans.borrow();
        assert!(plans.iter().flat_map(AgentRequestPlan::ordered_messages).any(|message|
            message.content.as_deref().is_some_and(|text| text.contains("images omitted by the active image budget"))));
        assert!(context.unobserved_preview_environments.is_empty());
    }
}
