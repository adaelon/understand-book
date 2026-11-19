//! Opt-in model-in-the-loop evaluation. Tools, admission, persistence and source checks are real.
//! A gpt-6.1-sol subagent supplies each model completion through a small file transport.
use super::*;
use base64::Engine;

pub const SOURCE: &str = "BookStructure separates candidate preservation from chapter and theme selection. The complete candidate catalog C contains every preserved candidate. Each chapter chooses accepted_stop_refs A from C, and then macro_stop_refs M from that chapter's accepted choices: M is a subset of A, and A is a subset of C. Chapter spine.key_stop_ids uses the macro choices, not all accepted choices. Theme generation searches the complete candidate catalog C; chapter macro choices are only seeds, not its search boundary. Theme stages.member_refs T may include a candidate that no chapter accepted, provided that candidate remains in C and the theme's evidence supports its role. Final BookStructure.key_stops contains the union of chapter accepted choices A and theme members T. Throughlines point to the theme members. A candidate absent from a chapter spine is therefore not necessarily deleted or unavailable to a theme. Example: preserve three candidates: Jia says a cache reuses an earlier result; Yi says a changed input can make that old result inappropriate; Bing says invalidation on update restores consistency. The chapter accepts all three but chooses only Jia and Bing for its macro spine. A correctness theme can choose Yi and Bing. Even if the chapter later does not accept Yi, a theme can still inspect and choose Yi from C. If neither chapter nor theme selects Yi, Yi remains a candidate but is not materialized into final key_stops.";

pub fn save(path: &Path, value: &Value) {
    let pending = path.with_extension("pending");
    std::fs::write(&pending, serde_json::to_vec_pretty(value).unwrap()).unwrap();
    std::fs::rename(pending, path).unwrap();
}

pub fn wait_json(path: &Path) -> Result<Value, AdapterError> {
    let started = std::time::Instant::now();
    while !path.exists() {
        if started.elapsed().as_secs() > 1200 {
            return Err(AdapterError { message:format!("Subagent response timed out: {}",path.display()),spend_stop:None });
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|e|AdapterError {message:e.to_string(),spend_stop:None})?)
        .map_err(|e|AdapterError {message:e.to_string(),spend_stop:None})
}

pub struct SubagentAdapter { directory: PathBuf, sequence: std::cell::Cell<usize> }
impl ModelAdapter for SubagentAdapter {
    fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        let value=self.complete_structured(request)?;
        Ok(ParsedResponse {sufficient:true,answer:Some(value.to_string()),citations:vec![],model_supplement:vec![]})
    }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        let number=self.sequence.get()+1; self.sequence.set(number);
        save(&self.directory.join(format!("request-{number:04}.json")), &json!({
            "kind":"structured_completion", "request":request}));
        println!("TEACHING_MODEL_STRUCTURED_REQUEST {number:04}");
        wait_json(&self.directory.join(format!("response-{number:04}.json")))
    }
    fn chat(&self, plan: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let number=self.sequence.get()+1; self.sequence.set(number);
        let mut pictures=Vec::new();
        for (index,picture) in plan.preview_images.iter().enumerate() {
            let path=self.directory.join(format!("request-{number:04}-image-{index}.png"));
            std::fs::write(&path,base64::engine::general_purpose::STANDARD.decode(&picture.png_base64).unwrap()).unwrap();
            pictures.push(json!({"path":path,"caption":picture.caption,"candidate_id":picture.candidate_id,"environment_name":picture.environment_name}));
        }
        save(&self.directory.join(format!("request-{number:04}.json")), &json!({
            "instructions":plan.instructions,"input":plan.input,"tools":plan.tools,"tool_choice":plan.tool_choice,
            "parallel_tool_calls":plan.parallel_tool_calls,"active_context":plan.active_context,"preview_images":pictures,
            "output_token_limit":plan.output_token_limit,"instruction_assets":plan.instruction_assets}));
        println!("TEACHING_MODEL_REQUEST {number:04}");
        let value=wait_json(&self.directory.join(format!("response-{number:04}.json")))?;
        let tool_calls=value["tool_calls"].as_array().into_iter().flatten().map(|call| {
            Ok(ToolCall {id:call["id"].as_str().ok_or("tool call id missing")?.into(),
                name:call["name"].as_str().ok_or("tool name missing")?.into(),
                arguments:match &call["arguments"] {Value::String(s)=>s.clone(),value=>value.to_string()}})
        }).collect::<Result<Vec<_>,&str>>().map_err(|s|AdapterError {message:s.into(),spend_stop:None})?;
        Ok(AssistantTurn {provider_continuation:None,text:value["text"].as_str().map(str::to_owned),tool_calls,usage_total_tokens:None})
    }
}

#[test]
#[ignore = "Requires a gpt-6.1-sol subagent and fresh TEACHING_LIVE_OUTPUT"]
fn teaching_design_live_subagent_conversation() {
    let directory=PathBuf::from(std::env::var("TEACHING_LIVE_OUTPUT").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    assert!(!directory.join("request-0001.json").exists(),"Use a fresh output directory");
    let resumed = std::env::var("TEACHING_LIVE_STATE").ok().map(PathBuf::from);
    let (state_directory, mut state) = if let Some(root) = &resumed {
        (root.clone(), reopen_live_state(root))
    } else {
        let (base, source) = if let Ok(book_directory) = std::env::var("TEACHING_LIVE_BOOK_DIR") {
            let root = PathBuf::from(book_directory);
            (serde_json::from_slice(&std::fs::read(root.join("base.json")).unwrap()).unwrap(),
                std::fs::read_to_string(root.join("source.txt")).unwrap())
        } else {
            let mut base=sample_base(); base.book_id="bookstructure-teaching-design-fixture".into();
            base.graph_nodes.clear(); base.graph_edges.clear();
            for node in &mut base.lid_nodes {node.span=Span {start:0,end:SOURCE.encode_utf16().count()};}
            (base, SOURCE.to_owned())
        };
        let (root,mut state)=super::tutor_loop_tests::fixture_with_book(false,Book::new(base,&source),&source);
        super::session_runtime_tests::enable_jsonl(&mut state);
        assert_eq!(route_agent_new(&mut state,"teaching-live").status,200);
        (root.keep(), state)
    };
    let first_turn=std::env::var("TEACHING_LIVE_START_TURN").ok().map(|s|s.parse::<usize>().unwrap()).unwrap_or(1);
    if resumed.is_some() {
        save(&directory.join("recovered-history.json"), &json!(state.user.agent_history));
    }
    save(&directory.join("configuration.json"),&json!({"model":"gpt-6.1-sol subagent","transport":"file ModelAdapter; real tools and persistence",
        "source":std::fs::read_to_string(state_directory.join("source.txt")).unwrap(),
        "book_directory":std::env::var("TEACHING_LIVE_BOOK_DIR").ok(),
        "source_basis":std::env::var("TEACHING_LIVE_SOURCE_BASIS").ok(),
        "learner":"simulated expressions; see experiment.md for how inputs were produced; not human learning evidence",
        "teaching_mode":"normal session default; no test-imposed guided inquiry","state_directory":state_directory,
        "resumed_state":resumed,"first_turn":first_turn}));
    let adapter=SubagentAdapter {directory:directory.clone(),sequence:Default::default()};
    for index in first_turn..=12 {
        let input=wait_json(&directory.join(format!("input-{index:02}.json"))).unwrap();
        if input["stop"]==true {break;}
        let mut before_results=std::collections::BTreeMap::new();
        for action in input["before"].as_array().into_iter().flatten() {
            let reply=post(&mut state,action["path"].as_str().unwrap(),&action["body"].to_string());
            assert_eq!(reply.status,200,"{}",reply.body);
            let name=action["name"].as_str().unwrap_or("action");
            let result:Value=serde_json::from_str(&reply.body).unwrap();
            save(&directory.join(format!("before-{index:02}-{name}.json")),&result);
            before_results.insert(name.to_owned(),result);
        }
        let mut request=input.get("request").unwrap_or(&input).clone();
        if let Some(name)=input["use_before_receipt"].as_str() {
            request["presentation_follow_up"]=before_results.get(name).expect("saved receipt result").clone();
        }
        let request_id=if resumed.is_some() {format!("live-retry-{index:02}")} else {format!("live-{index:02}")};
        let turn_started=std::time::Instant::now();
        let run=prepare_agent_chat(&mut state,&request.to_string(),&request_id).unwrap_or_else(|r|panic!("{}",r.body));
        let turn=run.turn_ref.clone();
        let tutor=run.tutor.clone();
        // Capture the exact submitted expression and frozen help before this turn can add support.
        save(&directory.join(format!("input-context-{index:02}.json")), &json!({
            "input":input,"submitted_request":request,"turn_ref":{"session_id":turn.session_id,"turn_id":turn.turn_id},"tutor_at_input":tutor,
            "learner_origin":"simulated reader; not human learning evidence"}));
        let result=agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)),&adapter,run,Default::default());
        let saved=state.user.agent_history.sessions.iter().find(|s|s.id==turn.session_id).unwrap().turns.iter().find(|t|t.turn_id==turn.turn_id).unwrap();
        let deliveries=state.user.learning_store().unwrap().teaching_deliveries(&turn.session_id,&turn.turn_id).unwrap();
        save(&directory.join(format!("turn-{index:02}.json")),&json!({"http_status":result.reply.status,"reply":serde_json::from_str::<Value>(&result.reply.body).unwrap(),
            "turn":saved,"tutor_at_input":tutor,"deliveries":deliveries,"model_calls_total":adapter.sequence.get(),
            "elapsed_ms":turn_started.elapsed().as_millis(),"usage_note":"File subagent transport does not report provider tokens; turn usage contains host estimates when available."}));
        // Read persisted facts, designs, moves and interpretations, including superseded judgments.
        // References can therefore be followed without reconstructing them from model prose.
        if let Some(session)=tutor.as_ref().and_then(|t|t["binding"]["tutor_session_id"].as_str()) {
            let store=state.user.learning_store().unwrap();
            let mut trace=Vec::new();
            let mut after=0;
            loop {
                let page=store.teaching_events(session,after,100).unwrap();
                if page.is_empty() {break;}
                after=page.last().unwrap().0;
                trace.extend(page);
            }
            let mut evidence=Vec::new();
            after=0;
            loop {
                let page=store.evidence_page(&state.workspace.book.base.book_id,after,100).unwrap();
                if page.is_empty() {break;}
                after=page.last().unwrap().0;
                evidence.extend(page);
            }
            save(&directory.join(format!("learning-trace-{index:02}.json")),&json!({
                "tutor_session_id":session,"teaching_events":trace,"learning_evidence":evidence}));
        }
        if let Some(answer)=saved.outcome.as_ref().and_then(|o|o.answer.as_ref()) {std::fs::write(directory.join(format!("answer-{index:02}.md")),answer).unwrap();}
        // Capture delivered page source for content and browser inspection, never synthesize a page.
        for delivery in &deliveries {
            if let Ok(reference)=serde_json::from_value::<runtime::presentation::PresentationRef>(delivery.payload["move"]["presentation"].clone()) {
                let version=state.private_context().read_presentation(&turn.session_id,&reference).unwrap();
                save(&directory.join(format!("page-{index:02}.json")),&json!(version));
                std::fs::write(directory.join(format!("page-{index:02}.html")),crate::presentation_author::preview_document(&version.content)).unwrap();
            }
        }
        println!("TEACHING_USER_TURN {index:02} status={}",result.reply.status);
        assert_eq!(result.reply.status,200,"{}",result.reply.body);
    }
    save(&directory.join("complete.json"),&json!({"model_calls":adapter.sequence.get(),"finished":true}));
}

// Restore a kept live fixture through the same JSONL recovery as local startup.
#[test]
#[ignore = "Requires a fresh gpt-6.1-sol review of the recorded overextended opening"]
fn teaching_delivery_review_live_scope_regression() {
    let directory=PathBuf::from(std::env::var("TEACHING_LIVE_OUTPUT").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    assert!(!directory.join("request-0001.json").exists(),"Use a fresh output directory");
    let original:Value=serde_json::from_slice(&std::fs::read(std::env::var("TEACHING_REVIEW_PACKET").unwrap()).unwrap()).unwrap();
    let packet:Value=serde_json::from_str(original["request"]["user"].as_str().unwrap()).unwrap();
    let adapter=SubagentAdapter {directory:directory.clone(),sequence:Default::default()};
    let (verdict,usage)=runtime::tutor::review_delivery(&adapter,adapter.model_runtime_profile(),packet,Vec::new()).unwrap();
    save(&directory.join("review-result.json"),&json!({"verdict":verdict,"estimated_or_reported_tokens":usage}));
    save(&directory.join("complete.json"),&json!({"finished":true,"model_calls":adapter.sequence.get()}));
    assert_eq!(verdict["decision"],"revise","the recorded opening adds a condition before its prerequisite is usable");
}

// Restore a kept live fixture through the same JSONL recovery as local startup.
// A new request retries an interrupted turn; its lost in-memory loop is not replayed.
pub fn reopen_live_state(root: &Path) -> AppState {
    let base=serde_json::from_slice(&std::fs::read(root.join("base.json")).unwrap()).unwrap();
    let source=std::fs::read_to_string(root.join("source.txt")).unwrap();
    let book=Book::new(base,&source);
    let reader=Reader::new(&book,DEFAULT_RADIUS);
    let history_path=Some(root.join("history.json"));
    let (mut history,mut session_store)=crate::session_store::load_chat_storage(&history_path).unwrap();
    crate::session_runtime::recover(session_store.as_mut().unwrap(),&mut history,&Default::default()).unwrap();
    let mut user=crate::user_runtime::UserRuntime::local(
        MemoryStore::open(root.join("memory.json")).unwrap(),history_path,history,Some(root.join("private-build-intents")));
    user.session_store=session_store;
    let mut state=AppState {
        services:crate::service_state::ServiceState { provider_binding: None, stage_processes: Default::default(),desktop_host:false,reader_only:false,library_root:None,adapter:Box::new(crate::UnconfiguredAdapter)},
        user,mcp_artifact_read_port:None,visitor_sessions:crate::mcp::VisitorSessions::default(),
        workspace:crate::reader_workspace::ReaderWorkspace::local(root.to_path_buf(),book.into(),reader,new_session(),None),
    };
    state.workspace.restore_chat(&mut state.user.agent_history,"teaching-live-retry");
    state
}
