//! Compiled assets through real Server Author and Resident request assembly.
//! The Provider boundary records actual plans and returns controlled tool calls.
use super::*;
use std::{cell::RefCell, collections::VecDeque};

fn invoke(port: &mut impl ResidentStatePort, args: Value) -> Result<AuthorResult, ToolError> {
    port.author_presentation(
        serde_json::from_value(args).unwrap(),
        &[],
        &[],
        &CancellationToken::default(),
    )
}

#[test]
fn se2_author_reads_registered_files_and_reports_exact_errors() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(RefCell::new(&mut state));
    let scope = app.with_app(|s| crate::run_scope::RunScope::capture(s, &turn, "se2", None, None));
    let mut port = RuntimeStatePort {
        scope: &scope,
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    assert_eq!(
        invoke(
            &mut port,
            json!({"operation":"read_method","method_id":"missing","revision":1})
        )
        .err()
        .unwrap()
        .error_code,
        "PRESENTATION_METHOD_NOT_FOUND"
    );
    for method in runtime::presentation_methods::catalog()["methods"]
        .as_array()
        .unwrap()
    {
        let base = json!({"operation":"read_method","method_id":method["id"],"revision":method["revision"]});
        let card = invoke(&mut port, base.clone()).unwrap();
        assert_eq!(card.body["file"], "card.md");
        assert_eq!(card.body["files"], method["files"]);
        assert_eq!(card.body["entrypoint"], method["entrypoint"]);
        assert!(
            card.images.is_empty()
                && card.delivered.is_none()
                && card.previewed_candidate.is_none()
        );
        for file in method["files"].as_array().unwrap() {
            let expected = std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../..")
                    .join(method["root"].as_str().unwrap())
                    .join(file.as_str().unwrap()),
            )
            .unwrap();
            let mut offset = 0;
            let mut reconstructed = String::new();
            loop {
                let mut args = base.clone();
                args["file"] = file.clone();
                args["offset"] = json!(offset);
                args["length"] = json!(333);
                let result = invoke(&mut port, args).unwrap().body;
                assert_eq!(result["method_id"], method["id"]);
                assert_eq!(result["revision"], method["revision"]);
                assert_eq!(result["total_characters"], expected.chars().count());
                let text = result["text"].as_str().unwrap();
                reconstructed.push_str(text);
                assert_eq!(result["chunk_characters"], text.chars().count());
                assert_eq!(result["offset"], offset);
                let end = offset + text.chars().count();
                assert_eq!(result["end_offset"], end);
                match result["next_offset"].as_u64() {
                    Some(next) => {
                        assert_eq!(next as usize, end, "explicit page length must continue at the actual returned end");
                        assert!(end < expected.chars().count());
                        offset = next as usize;
                    }
                    None => {
                        assert_eq!(end, expected.chars().count(), "only the whole file's EOF ends method pagination");
                        break;
                    }
                }
            }
            assert_eq!(reconstructed, expected);
        }
        for (field, value, code) in [
            ("revision", json!(999), "PRESENTATION_METHOD_NOT_FOUND"),
            (
                "file",
                json!("missing.html"),
                "PRESENTATION_METHOD_FILE_NOT_FOUND",
            ),
            ("offset", json!(999999), "PRESENTATION_AUTHORING_FAILED"),
            ("length", json!(0), "PRESENTATION_AUTHORING_FAILED"),
        ] {
            let mut args = base.clone();
            args[field] = value;
            assert_eq!(invoke(&mut port, args).err().unwrap().error_code, code);
        }
        let mut eof = base.clone();
        eof["offset"] = card.body["total_characters"].clone();
        let eof = invoke(&mut port, eof).unwrap().body;
        assert_eq!(eof["text"], "");
        assert_eq!(eof["next_offset"], Value::Null);
    }
    assert!(port.previewed.is_empty() && port.plots.is_empty() && port.animations.is_empty());
}

struct RecordingProvider {
    calls: RefCell<VecDeque<runtime::ToolCall>>,
    plans: RefCell<Vec<AgentRequestPlan>>,
}
impl ModelAdapter for RecordingProvider {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        unreachable!("no model helpers")
    }
    fn chat(&self, plan: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        self.plans.borrow_mut().push(plan.clone());
        let call = self.calls.borrow_mut().pop_front();
        Ok(AssistantTurn {
            provider_continuation: None,
            text: call.is_none().then(|| "方法资料已读取。".into()),
            tool_calls: call.into_iter().collect(),
            usage_total_tokens: None,
        })
    }
}

fn call(id: impl Into<String>, tool: &str, args: Value) -> runtime::ToolCall {
    runtime::ToolCall {
        id: id.into(),
        name: tool.into(),
        arguments: args.to_string(),
    }
}

fn body(plan: &AgentRequestPlan, id: &str) -> Value {
    let messages = plan.ordered_messages();
    let message = messages
        .iter()
        .find(|m| m.tool_call_id.as_deref() == Some(id))
        .expect("read result reaches next Provider sampling");
    let envelope: Value = serde_json::from_str(message.content.as_deref().unwrap()).unwrap();
    assert_eq!(envelope["status"], "ok", "{envelope}");
    envelope["model_body"].clone()
}

fn run(message: &str, calls: VecDeque<runtime::ToolCall>) -> Vec<AgentRequestPlan> {
    let expected_reads = calls.iter().filter(|call| call.arguments.contains("read_method")).count();
    let exposed = !calls.is_empty();
    let root = tempfile::tempdir().unwrap();
    let mut state = state_named("se2-provider");
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    state.user.history_path = Some(root.path().join("history.json"));
    let prepared = prepare_agent_chat(
        &mut state,
        &json!({"message":message}).to_string(),
        "2026-10-16T00:00:00Z",
    )
    .unwrap_or_else(|r| panic!("{}", r.body));
    let adapter = RecordingProvider {
        calls: RefCell::new(calls),
        plans: RefCell::new(vec![]),
    };
    let app = BorrowedAppPort(RefCell::new(&mut state));
    let report =
        crate::agent_run::execute_prepared(&app, &adapter, prepared, CancellationToken::default());
    assert_eq!(report.reply.status, 200, "{}", report.reply.body);
    assert!(
        adapter.calls.borrow().is_empty(),
        "Resident stopped before the selected reads completed"
    );
    // SE3 retains read-only runs independently of candidates and declarations.
    app.with_app(|s| {
        let session = s.user.agent_history.sessions.last().unwrap();
        let turn = session.turns.last().unwrap();
        let observations = s.private_context().presentation_method_observations(&session.id, &turn.turn_id).unwrap();
        assert_eq!(observations.catalog.is_some(), exposed);
        assert_eq!(observations.reads.len(), expected_reads);
    });
    adapter.plans.into_inner()
}

fn record(name: &str, plans: &[AgentRequestPlan]) {
    if let Some(root) = std::env::var_os("SE2_OUTPUT") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        let values: Vec<_> = plans.iter().map(|p| json!({"instructions":p.instructions,
            "messages":p.ordered_messages(),"instruction_assets":p.instruction_assets,"tools":p.tools})).collect();
        std::fs::write(root.join(name), serde_json::to_vec_pretty(&values).unwrap()).unwrap();
    }
}

#[test]
fn se2_actual_provider_inputs_contain_only_requested_method_source() {
    let plain = run(
        "用一句话解释：当前状态加更新量得到下一状态。",
        VecDeque::new(),
    );
    assert_eq!(plain.len(), 1);
    assert!(!plain[0]
        .tools
        .iter()
        .any(|t| t.name == "presentation.author"));
    assert!(!serde_json::to_string(&plain[0].ordered_messages())
        .unwrap()
        .contains("Presentation method directory:"));
    record("ordinary-provider-inputs.json", &plain);

    // A real author selects a method before reading it. Keep each selected unit in
    // its own run so library growth does not consume the run's finite tool budget.
    for method in runtime::presentation_methods::catalog()["methods"]
        .as_array()
        .unwrap()
    {
        let mut calls = VecDeque::from([call(
            "discover",
            "tool.search",
            json!({
            "task":"read explanation method assets", "required_capabilities":["presentation_authoring"],
            "scope":"passage", "operation":"explain", "effect_mode":"read_only", "max_results":1}),
        )]);
        let mut expected = Vec::new();
        for registered_file in method["files"].as_array().unwrap() {
            let file = registered_file.as_str().unwrap();
            let (_, source) = runtime::presentation_methods::source(
                method["id"].as_str().unwrap(),
                method["revision"].as_u64().unwrap(),
                file,
            )
            .unwrap();
            let chars: Vec<_> = source.chars().collect();
            for offset in (0..chars.len()).step_by(4000) {
                let id = format!("read-{}", expected.len());
                let mut args = json!({"operation":"read_method","method_id":method["id"],"revision":method["revision"],"offset":offset,"length":4000});
                if file != "card.md" {
                    args["file"] = json!(file);
                }
                calls.push_back(call(&id, "presentation.author", args));
                expected.push((
                    id,
                    file,
                    offset,
                    chars[offset..(offset + 4000).min(chars.len())]
                        .iter()
                        .collect::<String>(),
                ));
            }
        }
        let plans = run(
            "读取解释方法目录和选中方法的卡片与源码，供之后适配使用。",
            calls,
        );
        assert_eq!(plans.len(), expected.len() + 2);
        assert!(serde_json::to_string(&plans[1].ordered_messages())
            .unwrap()
            .contains("Presentation method directory:"));
        assert!(!serde_json::to_string(&plans[1].ordered_messages())
            .unwrap()
            .contains("function renderAt"));
        for (index, (id, file, offset, text)) in expected.iter().enumerate() {
            let observed = body(&plans[index + 2], id);
            assert_eq!(observed["status"], "method_read");
            assert_eq!(observed["file"], *file);
            assert_eq!(observed["offset"], *offset);
            assert_eq!(observed["text"], *text);
            assert_eq!(observed["end_offset"], offset + text.chars().count());
            if index == 0 {
                let wire = serde_json::to_string(&plans[index + 2].ordered_messages()).unwrap();
                assert!(
                    !wire.contains("function renderAt"),
                    "unselected example source leaked with the card"
                );
                assert!(
                    !wire.contains("## Author 内容字段"),
                    "unselected guide leaked with the card"
                );
            }
        }
        record(&format!("author-{}-provider-inputs.json", method["id"].as_str().unwrap()), &plans);
        eprintln!(
            "SE2 Provider boundary: {} method reads across {} actual requests",
            expected.len(),
            plans.len()
        );
    }
}
