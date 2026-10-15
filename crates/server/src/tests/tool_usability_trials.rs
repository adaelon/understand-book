//! Explicit TO14 trials. The same test-only adapter is applied to both source snapshots.
use super::*;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::time::Instant;

struct CodexTrialAdapter {
    directory: PathBuf,
    ordinal: AtomicUsize,
}
impl CodexTrialAdapter {
    fn sample(&self, value: Value) -> Result<Value, AdapterError> {
        let index = self.ordinal.fetch_add(1, Ordering::SeqCst);
        let path = self.directory.join(format!("request-{index:02}.json"));
        std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        let script = std::env::var("TO14_SAMPLE_SCRIPT").expect("TO14_SAMPLE_SCRIPT");
        let status = std::process::Command::new("node").arg(script).arg(&self.directory).arg(index.to_string()).status()
            .map_err(|e| AdapterError { message:e.to_string(), spend_stop:None })?;
        let output: Value = serde_json::from_slice(&std::fs::read(self.directory.join(format!("response-{index:02}.json")))
            .map_err(|e| AdapterError {message:e.to_string(),spend_stop:None})?).unwrap();
        if !status.success() || output.get("error").is_some() {
            return Err(AdapterError { message:output["error"].as_str().unwrap_or("CODEX_SAMPLE_FAILED").into(),spend_stop:None });
        }
        Ok(output)
    }
}
impl ModelAdapter for CodexTrialAdapter {
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile {
        runtime::ModelRuntimeProfile::fallback("gpt-6.1-sol", runtime::ProviderToolProtocol::Native)
    }
    fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        let response = self.sample(json!({"kind":"completion","request":request}))?;
        Ok(ParsedResponse { sufficient:true, answer:response["text"].as_str().map(str::to_owned), citations:vec![], model_supplement:vec![] })
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let mut input = serde_json::to_value(&request.input).unwrap();
        for message in input.as_array_mut().into_iter().flatten() {
            if let Some(fields) = message.as_object_mut() { fields.remove("provider_continuation"); }
        }
        let response = self.sample(json!({"instructions":request.instructions,"input":input,"tools":request.tools,
            "instruction_assets":request.instruction_assets,"runtime_profile":request.runtime_profile,
            "active_context":request.active_context,"output_token_limit":request.output_token_limit,
            "tool_choice":request.tool_choice,"parallel_tool_calls":request.parallel_tool_calls}))?;
        let index = self.ordinal.load(Ordering::SeqCst);
        let calls = response["tool_calls"].as_array().unwrap().iter().enumerate().map(|(i, call)| ToolCall {
            id:format!("to14-{index}-{i}"), name:call["name"].as_str().unwrap().into(), arguments:call["arguments"].as_str().unwrap().into(),
        }).collect();
        Ok(AssistantTurn { provider_continuation:None, text:response["text"].as_str().map(str::to_owned),
            tool_calls:calls, usage_total_tokens:response["usage_total_tokens"].as_u64().map(|n|n as u32) })
    }
}

#[test]
#[ignore = "TO14 frozen Codex trial; requires TO14_RUN_DIR, TO14_CASE and isolated Codex adapter"]
fn to14_resident_trial() {
    let directory = PathBuf::from(std::env::var("TO14_RUN_DIR").expect("TO14_RUN_DIR"));
    assert!(!directory.join("result.json").exists(), "preserve completed and failed trials");
    std::fs::create_dir_all(&directory).unwrap();
    let case = std::env::var("TO14_CASE").expect("TO14_CASE");
    let (_root, mut state) = fixture(true);
    let input = match case.as_str() {
        "tutor-continue" => {
            let original: Value = serde_json::from_slice(&std::fs::read(std::env::var("TO14_ORIGINAL_ANSWER").unwrap()).unwrap()).unwrap();
            state.services.adapter = Box::new(ChatStubAdapter::scripted(vec![
                AssistantTurn { provider_continuation:None,text:None,usage_total_tokens:Some(0),tool_calls:vec![ToolCall {
                    id:"fixed-source".into(),name:"book.text".into(),arguments:json!({"lid":"1.1","end_lid":"1.1"}).to_string() }] },
                AssistantTurn { provider_continuation:None,text:Some(original["first"]["answer"].as_str().unwrap().into()),usage_total_tokens:Some(0),tool_calls:vec![] },
            ]));
            let first = selection_input(&state,"fixed-first"); post_ok(&mut state,"/agent/chat",first);
            let original = state.user.agent_history.sessions[0].turns[0].turn_id.clone();
            enable(&mut state);
            json!({"message":"","session_id":state.workspace.selected_chat,"client_request_id":"fixed-next",
                "entry_action":"continue_answer","answer_ref":{"turn_id":original},"expected_chat_head":original})
        }
        "save-note" => json!({"message":"把‘平均速度＝总路程／总时间’记为这一段的笔记。","question_quote":quote()}),
        "explain-only" => json!({"message":"只解释选中的平均速度定义，不保存笔记、高亮或记忆。请附原文来源。","question_quote":quote()}),
        _ => panic!("unknown frozen case"),
    };
    let before = serde_json::to_value(state.workspace.reader.state()).unwrap();
    std::fs::write(directory.join("input.json"),serde_json::to_vec_pretty(&json!({"case":case,"input":input,"source":SOURCE,
        "base":state.workspace.book.base,"reader_before":before})).unwrap()).unwrap();
    state.services.adapter = Box::new(CodexTrialAdapter { directory:directory.clone(),ordinal:AtomicUsize::new(0) });
    let started = Instant::now();
    let reply = post(&mut state,"/agent/chat",&input.to_string());
    let session = &state.user.agent_history.sessions[0];
    let turn = session.turns.last().unwrap();
    let store = state.user.learning_store().unwrap();
    let deliveries = store.teaching_deliveries(&session.id,&turn.turn_id).unwrap();
    let tutor_session = store.state().unwrap().control.current_tutor_session_id;
    let events = tutor_session.map(|id|store.teaching_turn_events(&id,&turn.turn_id).unwrap()).unwrap_or_default();
    let reopened = MemoryStore::open(_root.path().join("memory.json")).unwrap();
    let records = reopened.recall(&memory::RecallQuery::default());
    let result = json!({"case":case,"http_status":reply.status,"elapsed_ms":started.elapsed().as_millis(),
        "reply":serde_json::from_str::<Value>(&reply.body).unwrap_or(json!(reply.body)),"turn":turn,
        "deliveries":deliveries,"events":events,"reopened_records":records,"reader_after":state.workspace.reader.state()});
    std::fs::write(directory.join("result.json"),serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    println!("TO14 case={case} status={} output={}",reply.status,directory.display());
}
