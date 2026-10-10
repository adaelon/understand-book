//! One retained EX13.6 continuation through the production author/preview loop.
//! The local recording relay enforces the approved monetary limit per HTTP call.
use super::*;

struct ContinuationAdapter(RecordingAdapter);

impl ModelAdapter for ContinuationAdapter {
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile { self.0.profile.clone() }
    fn set_run_cancellation(&self, token: CancellationToken) { self.0.inner.set_run_cancellation(token); }
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, runtime::AdapterError> {
        Err(runtime::AdapterError { spend_stop: None, message: "EX13.6 uses only recorded chat and structured requests".into() })
    }
    fn complete_structured(&self, req: CompletionRequest) -> Result<Value, runtime::AdapterError> {
        self.complete_structured_observed(req, &mut runtime::provider_stream::ignore)
    }
    fn complete_structured_observed(&self, mut req: CompletionRequest, observer: &mut dyn ModelObserver) -> Result<Value, runtime::AdapterError> {
        // Admission also calls the memory-intent classifier, whose ordinary
        // request has no max_tokens. Bound that auxiliary call for this run.
        if req.output_token_limit.is_none() { req.output_token_limit = Some(16_384); }
        self.0.complete_structured_observed(req, observer)
    }
    fn chat(&self, req: &runtime::AgentRequestPlan) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
        self.chat_observed(req, &mut runtime::provider_stream::ignore)
    }
    fn chat_observed(&self, req: &runtime::AgentRequestPlan, observer: &mut dyn ModelObserver) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
        let n = self.0.chats.fetch_add(1, SeqCst);
        let images: Vec<_> = req.preview_images.iter().enumerate().map(|(i, image)| {
            let file = format!("image-{n:02}-{i:02}.png");
            std::fs::write(self.0.root.join(&file), base64::engine::general_purpose::STANDARD.decode(&image.png_base64).unwrap()).unwrap();
            json!({"file":file,"caption":image.caption,"candidate_id":image.candidate_id,"environment":image.environment_name})
        }).collect();
        // ordered_messages includes EX13 sampling-position guidance and state.
        save(self.0.root.join(format!("request-{n:02}.json")), &json!({
            "instructions":req.instructions,"instruction_assets":req.instruction_assets,
            "messages":req.ordered_messages(),"images":images,"runtime_profile":req.runtime_profile,
            "tools":req.tools.iter().map(|t|json!({"name":t.name,"description":t.description,"parameters":t.parameters})).collect::<Vec<_>>(),
            "output_token_limit":req.output_token_limit,"elapsed_ms":self.0.started.elapsed().as_millis()}));
        let start = Instant::now(); let mut usage = None;
        let result = self.0.inner.chat_observed(req, &mut |delta: ModelDelta| {
            if let ModelDelta::Usage(value) = &delta { usage = Some(value.clone()); }
            observer.observe(delta);
        });
        save(self.0.root.join(format!("response-{n:02}.json")), &json!({
            "text":result.as_ref().ok().and_then(|t|t.text.as_ref()),
            "tool_calls":result.as_ref().ok().map(|t|&t.tool_calls),"usage":usage,
            "error":result.as_ref().err().map(|e|&e.message),"duration_ms":start.elapsed().as_millis()}));
        result
    }
}

fn continuation(root: &Path) -> (AppState, Value, Value) {
    let mut state = setup_run(root, "mechanism");
    let history = evidence().join("mechanism-staged-feedback5/history.json");
    std::fs::copy(&history, root.join("history.json")).unwrap();
    state.user.agent_history = load_agent_history(&state.user.history_path).unwrap();
    let session = state.user.agent_history.sessions.last().unwrap();
    assert!(session.compaction_checkpoint.is_some());
    runtime::compaction::validate_persisted_checkpoint(&session.messages, session.compaction_checkpoint.as_ref().unwrap()).unwrap();
    let goal = session.goals.iter().find(|g| g.status == runtime::goal::GoalStatus::Open).unwrap();
    let goal_id = goal.id.clone();
    let session_id = session.id.clone();
    state.workspace.messages = session.messages.clone();
    state.workspace.selected_chat = Some(session.id.clone());
    let version = json_file(evidence().join("mechanism-staged-feedback3/version.json"));
    let mut patch = json_file(evidence().join("local-correction/patch.json"));
    assert_eq!(version["reference"], patch["reference"]);
    assert_eq!(patch["reference"], json!({"presentation_id":"presentation-1790920818511542500-6","revision":1}));
    let mut html = version["content"]["content_files"]["index.html"].as_str().unwrap().to_owned();
    for edit in patch["edits"].as_array().unwrap() {
        let old = edit["old_text"].as_str().unwrap();
        assert_eq!(html.matches(old).count(), 1, "frozen patch base mismatch");
        html = html.replacen(old, edit["new_text"].as_str().unwrap(), 1);
    }
    let expected = json_file(evidence().join("local-correction/content.json"));
    assert_eq!(html, expected["content_files"]["index.html"]);
    assert_eq!(patch["readable_content"], expected["readable_content"]);
    let supplemental = json_file(repo().join("docs/performance/presentation-context-ex13/ex13-6/supplemental-edit.json"));
    assert_eq!(html.matches(supplemental["old_text"].as_str().unwrap()).count(), 1);
    patch["edits"].as_array_mut().unwrap().push(supplemental);
    save(root.join("patch.json"), &patch);
    let target = root.join("history.presentations/versions").join(version["reference"]["presentation_id"].as_str().unwrap());
    std::fs::create_dir_all(&target).unwrap(); save(target.join("1.json"), &version);
    let observations = json_file(evidence().join("mechanism-staged-feedback3/independent/observations.json"));
    assert_eq!(observations["reference"], version["reference"]);
    let scene = observations["records"][0]["observations"].as_array().unwrap().iter()
        .find(|o| o["label"] == "default_calculator").unwrap();
    let reference = serde_json::from_value(version["reference"].clone()).unwrap();
    let receipt = state.private_context().save_presentation_state(&session_id,
        version["created_by_turn_id"].as_str().unwrap(), &reference,
        runtime::presentation::PresentationState {
            values:json!({"page":scene["state"]["values"],"controls":[]}),
            visible_step:None,observed_result:scene["selected_text"].as_str().unwrap().into(),source_ref_ids:vec![],
        }).unwrap();
    save(root.join("saved-observation.json"), &json!({"source":"mechanism-staged-feedback3/independent/observations.json:960/default_calculator","receipt":receipt,"state":scene["state"]}));
    let message = format!("继续当前未完成 Goal，完成原定局部纠错并交付同一对象的新 revision。上一运行因余额不足中断，现恢复。先用 goal.update working.items 记录读取、修改、验证、交付四项进展，保留原要求。以下是已离线验证、尚未原生交付的精确补丁；请重新发现工具，读取指定版本的相关内容，核对所需来源后，通过 presentation.author.patch 原样应用这些替换和 readable_content。保留原作品完整范围，不另建对象。\n{}\n\n在当前运行中预览三环境并检查返回图片：默认数值、B=8、权重减半；练习 1-3 默认 B*=105.5、读取减半 B*=52.7 且 B=64 计算18.11ms；聚焦 ctl-N 时标签/控件不被总览遮挡；练习1-4四档实测表齐全。可在一次 preview 中使用多个动作并读取相应结果区。确认结果后 deliver 并在回答挂载新版本，更新工作计划。关键控件为 #btn-B8、#btn-bwhalf、#btn-halve、#ctl-N，区域为 .summary、#s7、#measured-ratios。只完成这次修订。", patch);
    (state, json!({"message":message,"question_anchor_lid":"3.6.1","goal_id":goal_id,"presentation_follow_up":receipt}), version)
}

#[test]
#[ignore = "EX13.6 archived local publication; offline admission only"]
fn ex13_continuation_restores_goal_checkpoint_and_exact_patch() {
    let root = tempfile::tempdir().unwrap();
    let (mut state, input, _) = continuation(root.path());
    let goal = state.user.agent_history.sessions[0].goals.last().unwrap().clone();
    let prepared = prepare_agent_chat(&mut state, &input.to_string(), "2026-10-02T14:00:00Z")
        .unwrap_or_else(|r|panic!("{}", r.body));
    let turn = state.user.agent_history.sessions[0].turns.iter().find(|t| t.turn_id == prepared.turn_ref.turn_id).unwrap();
    assert_eq!(turn.goal_ref.as_ref().map(|r| &r.id), Some(&goal.id));
    assert_eq!(state.user.agent_history.sessions[0].goals.last().unwrap().requirements, goal.requirements);
    assert!(state.user.agent_history.sessions[0].compaction_checkpoint.is_some());
    runtime::compaction::validate_persisted_checkpoint(&prepared.messages,
        state.user.agent_history.sessions[0].compaction_checkpoint.as_ref().unwrap()).unwrap();
    let mut replay = prepared.messages.clone();
    runtime::presentation_author::redact_history(&mut replay);
    runtime::tool_exposure::redact_history(&mut replay);
    runtime::compaction::validate_persisted_checkpoint(&replay,
        state.user.agent_history.sessions[0].compaction_checkpoint.as_ref().unwrap()).unwrap();
}

#[test]
#[ignore = "EX13.6 paid continuation: invoke only via run.py with approved budget"]
fn ex13_live_continuation() {
    let root = PathBuf::from(std::env::var("EX13_RUN_DIR").unwrap());
    let relay = std::env::var("EX13_RECORDING_RELAY").unwrap();
    assert!(relay.starts_with("http://127.0.0.1:"));
    std::fs::create_dir(&root).unwrap();
    let (mut state, input, base) = continuation(&root);
    save(root.join("input.json"), &input);
    let mut profile: runtime::ModelRuntimeProfile = serde_json::from_value(json_file(evidence().join("provider-profile.json"))).unwrap();
    // Retain the archived operating profile; execute the current EX13.5 rules.
    profile.compaction.prompt_asset.text_override = Some(runtime::compaction::COMPACTION_GENERATION_PROMPT.into());
    let config = ProviderConfig::from_values("native", "local-recording-relay", relay, "deepseek-v4-flash").unwrap();
    let adapter = ContinuationAdapter(RecordingAdapter {
        inner:ProviderRegistry::adapter_from_config_with_runtime_profile(config, Some(profile.clone())), profile,
        baseline:false,root:root.clone(),chats:AtomicUsize::new(0),completions:AtomicUsize::new(0),started:Instant::now()});
    let mut prepared = prepare_agent_chat(&mut state, &input.to_string(), "2026-10-02T14:00:00Z")
        .unwrap_or_else(|r|panic!("prepare: {}",r.body));
    prepared.scope.provider_binding = adapter.model_runtime_profile();
    let original = json_file(evidence().join("../presentation-staged-authoring-baseline/original-turn.json"));
    prepared.scope.reader_input = serde_json::from_value(original["admission_input"]["reader"].clone()).unwrap();
    save(root.join("admitted.json"), &json!({"messages":prepared.messages,"message":prepared.agent_message,
        "reader":prepared.scope.reader_input,"profile":adapter.model_runtime_profile(),"base":base["reference"],
        "goals_before":state.user.agent_history.sessions[0].goals}));
    let turn = prepared.turn_ref.clone();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let report = crate::agent_run::execute_prepared(&app, &adapter, prepared, CancellationToken::default());
    std::fs::write(root.join("outcome.json"), &report.reply.body).unwrap();
    save(root.join("summary.json"), &json!({"status":report.reply.status,
        "elapsed_ms":adapter.0.started.elapsed().as_millis(),"model_requests":adapter.0.chats.load(SeqCst),
        "structured_completions":adapter.0.completions.load(SeqCst)}));
    app.with_app(|state| {
        save(root.join("goals-after.json"), &json!(state.user.agent_history.sessions[0].goals));
        if let Ok(outcome) = serde_json::from_str::<OuterOutcome>(&report.reply.body) {
            for part in outcome.answer_view.into_iter().flat_map(|v|v.parts) {
                if let AgentAnswerPart::Presentation {presentation_id, revision} = part {
                    let reference = runtime::presentation::PresentationRef {presentation_id, revision};
                    let version = state.private_context().read_presentation(&turn.session_id, &reference).unwrap();
                    save(root.join("version.json"), &json!(version));
                    save(root.join("content.json"), &json!(version.content));
                    assert_eq!(json!(reference.presentation_id), base["reference"]["presentation_id"]);
                    assert_eq!(reference.revision, 2);
                }
            }
        }
    });
    assert_eq!(report.reply.status, 200, "{}", report.reply.body);
    assert!(root.join("version.json").exists(), "run did not deliver the target revision");
}
