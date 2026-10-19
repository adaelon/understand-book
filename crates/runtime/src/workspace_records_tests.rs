use super::*;
use serde_json::{json, Value};

struct LinkedHistory<'a> {
    inner: BorrowedResidentState<'a>,
    reads: Vec<Value>,
}
impl ResidentStatePort for LinkedHistory<'_> {
    fn workspace_records_available(&self) -> bool { true }
    fn workspace_records(&mut self, request: Value) -> Result<Value, ToolError> {
        self.reads.push(request.clone());
        Ok(if request["operation"] == "list" {
            json!({"records":[{"selection":{"source_learning_workspace_id":null,"chat_session_id":"original-chat","turn_ids":["original-turn"]}}],"next":null})
        } else {
            assert_eq!(request["chat_session_id"], "original-chat");
            json!({"selection":{"source_learning_workspace_id":null,"chat_session_id":"original-chat"},"historical":true,"current_mastery":false,
                "records":[{"kind":"fact","record":{"event_id":"original-fact","learner_quote":"我能展开链式法则，但尚不会写反向传播"}}],"next":null})
        })
    }
    fn submit_private<R>(&mut self, operation: impl FnOnce(&mut MemoryStore) -> R) -> Result<R,ToolError> { self.inner.submit_private(operation) }
    fn read_live_reader<R>(&mut self, operation: impl FnOnce(&Reader) -> R) -> Result<R,ToolError> { self.inner.read_live_reader(operation) }
    fn apply_reader<R>(&mut self, operation: impl FnOnce(&mut MemoryStore,&mut Reader) -> R) -> Result<R,ToolError> { self.inner.apply_reader(operation) }
    fn reader_input(&mut self, book:&Book, question:&str)->crate::run_context::ReaderInputSnapshot { self.inner.reader_input(book,question) }
}

#[test]
fn w6_selected_history_enters_actual_model_read_loop_without_enabling_teaching() {
    let book = book();
    let mut store = MemoryStore::open(tmp("w6-history-context")).unwrap();
    let mut reader = Reader::new(&book, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&book, &store, "t0");
    let adapter = RequestPlanRecordingAdapter::new(vec![
        turn_calls(vec![call("prior", "workspace.records.read", r#"{"operation":"read","chat_session_id":"original-chat","limit":8}"#)]),
        turn_final("原记录中你能展开链式法则，但还没有写过反向传播；这次先结合当前例子继续。"),
    ], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    let mut port = LinkedHistory { inner: BorrowedResidentState { store: &mut store, reader: &mut reader }, reads:vec![] };
    let result = run_context(&book,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),"结合我带入的旧学习记录继续","t0").unwrap();
    assert!(!result.incomplete);
    assert_eq!(port.reads.len(),2);
    assert_eq!(port.reads[0]["operation"],"list");
    assert_eq!(port.reads[1]["operation"],"read");
    assert!(context.tutor.is_none(),"reading old records cannot resume the old stage");
    let requests = adapter.seen_plans.borrow();
    assert!(requests[0].tools.iter().any(|tool| tool.name == "workspace.records.read"));
    let first = json!(requests[0].ordered_messages()).to_string();
    assert!(first.contains("workspace_records.v1") && first.contains("original-chat"));
    assert!(first.contains("not current mastery"));
    let following = json!(requests[1].ordered_messages()).to_string();
    assert!(following.contains("我能展开链式法则，但尚不会写反向传播"));
    assert!(following.contains("original-fact") && following.contains("source_learning_workspace_id"));
}
