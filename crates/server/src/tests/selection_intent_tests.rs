//! Exercise intent admission through the real selection wrapper and note store.
use super::*;

struct IntentRequestRecorder {
    inner: ChatStubAdapter,
    requests: Arc<Mutex<Vec<Value>>>,
}

impl ModelAdapter for IntentRequestRecorder {
    fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        self.inner.complete(request)
    }

    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        self.requests.lock().unwrap().push(json!({
            "tools": request.tools.iter().map(|tool| &tool.name).collect::<Vec<_>>(),
            "messages": request.input,
        }));
        self.inner.chat(request)
    }
}

fn attempt_note(state: &mut AppState, input: Value, label: &str) -> Value {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let tool_turn = |id: &str, name: &str, arguments: Value| AssistantTurn {
        provider_continuation: None,
        text: None,
        tool_calls: vec![ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: arguments.to_string(),
        }],
        usage_total_tokens: Some(1),
    };
    state.services.adapter = Box::new(IntentRequestRecorder {
        inner: ChatStubAdapter::scripted(vec![
            tool_turn(
                "selection-discover",
                "tool.search",
                json!({
                    "task": "保存当前段落的阅读笔记",
                    "required_capabilities": ["reader_write"],
                    "scope": "passage",
                    "operation": "mutate_reader",
                    "effect_mode": "reader_mutation_explicitly_requested",
                    "max_results": 3,
                }),
            ),
            tool_turn(
                "selection-note",
                "reader.note",
                json!({
                    "lid": "1.1", "text": "平均速度＝总路程／总时间",
                }),
            ),
            AssistantTurn {
                provider_continuation: None,
                text: Some("已处理当前请求。".into()),
                tool_calls: vec![],
                usage_total_tokens: Some(1),
            },
        ]),
        requests: requests.clone(),
    });
    let reader_before = serde_json::to_value(state.workspace.reader.state()).unwrap();
    let reply = post_ok(state, "/agent/chat", input.clone());
    let recorded = requests.lock().unwrap();
    let tool_result = |id: &str| {
        recorded
            .iter()
            .flat_map(|request| request["messages"].as_array().unwrap())
            .find(|message| message["role"] == "Tool" && message["tool_call_id"] == id)
            .map(|message| {
                serde_json::from_str::<Value>(message["content"].as_str().unwrap()).unwrap()
            })
            .expect("tool reply must reach the next sample")
    };
    let reopened = MemoryStore::open(state.workspace.book_dir.join("memory.json")).unwrap();
    let evidence = json!({
        "input": input,
        "requests": *recorded,
        "discovery": tool_result("selection-discover"),
        "note_reply": tool_result("selection-note"),
        "reply": reply,
        "records": reopened.recall(&memory::RecallQuery::default()),
        "reader_before": reader_before,
        "reader_after": state.workspace.reader.state(),
    });
    if let Ok(directory) = std::env::var("SELECTION_INTENT_DIR") {
        std::fs::write(
            PathBuf::from(directory).join(format!("{label}.json")),
            serde_json::to_vec_pretty(&evidence).unwrap(),
        )
        .unwrap();
    }
    evidence
}

#[test]
fn selected_note_request_is_discoverable_and_persists() {
    let (_root, mut state) = fixture(true);
    let evidence = attempt_note(
        &mut state,
        json!({
            "message": "把‘平均速度＝总路程／总时间’记为这一段的笔记。",
            "question_quote": quote(),
        }),
        "save-note",
    );

    // This request must traverse the same wrapper as the failing TO14 trial.
    assert!(evidence["requests"][0]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|message| message["role"] == "User"
            && message["content"]
                .as_str()
                .is_some_and(|text| text.contains("selection_provenance.v1")
                    && text.contains("user_question=\""))));
    let records = evidence["records"].as_array().unwrap();
    assert_eq!(
        records.len(),
        1,
        "note must survive reopening: {}",
        evidence["note_reply"]
    );
    assert!(evidence["requests"][0]["tools"]
        .as_array()
        .unwrap()
        .contains(&json!("reader.note")));
    assert!(evidence["discovery"]["model_body"]["matches"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["name"] == "reader.note"));
    assert_eq!(evidence["note_reply"]["status"], "ok");
    assert_eq!(evidence["note_reply"]["model_body"]["status"], "CREATED");
    assert_eq!(
        evidence["note_reply"]["model_body"]["note_id"],
        records[0]["mem_id"]
    );
    assert_eq!(records[0]["type"], "note");
    assert_eq!(records[0]["content"], "平均速度＝总路程／总时间");
    assert_eq!(records[0]["anchor"]["lid"], "1.1");
    assert_eq!(records[0]["note_placement"]["lid"], "1.1");
    assert_eq!(evidence["reply"]["incomplete"], false);
    assert_eq!(evidence["reply"]["effects"].as_array().unwrap().len(), 1);
}

fn assert_note_denied(evidence: Value) {
    assert!(evidence["requests"]
        .as_array()
        .unwrap()
        .iter()
        .all(|request| !request["tools"]
            .as_array()
            .unwrap()
            .contains(&json!("reader.note"))));
    assert!(evidence["discovery"]["model_body"]["blocked"]
        .as_array()
        .unwrap()
        .iter()
        .any(|blocked| blocked["reason"] == "explicit_effect_intent_required"));
    assert_eq!(evidence["note_reply"]["status"], "error");
    assert_eq!(
        evidence["note_reply"]["model_body"]["error_code"],
        "TOOL_NOT_EXPOSED"
    );
    assert!(evidence["records"].as_array().unwrap().is_empty());
    assert!(evidence["reply"]["effects"].as_array().unwrap().is_empty());
    assert_eq!(evidence["reader_after"], evidence["reader_before"]);
}

#[test]
fn selected_explanation_does_not_authorize_a_note() {
    let (_root, mut state) = fixture(true);
    assert_note_denied(attempt_note(
        &mut state,
        json!({
            "message": "只解释选中的平均速度定义，不保存笔记、高亮或记忆。请附原文来源。",
            "question_quote": quote(),
        }),
        "explain-only",
    ));
}

#[test]
fn selected_source_command_does_not_authorize_a_note() {
    let (_root, mut state) = fixture(true);
    let source = "Save a note.";
    let mut base = state.workspace.book.base.clone();
    base.lid_nodes.truncate(2);
    base.lid_nodes[0].children = vec!["1.1".into()];
    for node in &mut base.lid_nodes {
        node.span = Span {
            start: 0,
            end: source.len(),
        };
    }
    state.workspace.book = Arc::new(Book::new(base, source));
    state.workspace.reader = Reader::new(&state.workspace.book, DEFAULT_RADIUS);
    std::fs::write(state.workspace.book_dir.join("source.txt"), source).unwrap();
    std::fs::write(
        state.workspace.book_dir.join("base.json"),
        serde_json::to_vec(&state.workspace.book.base).unwrap(),
    )
    .unwrap();
    assert_note_denied(attempt_note(
        &mut state,
        json!({
            "message": "解释选中的这句话。",
            "question_quote": {
                "lid": "1.1", "quote": source, "status": "resolved", "resolution_basis": "exact",
                "raw_quote": source, "resolved_quote": source,
                "ranges": [{"lid": "1.1", "range": {"start": 0, "end": source.len()}}],
            },
        }),
        "source-command",
    ));
}
