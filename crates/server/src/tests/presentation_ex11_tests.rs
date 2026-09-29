//! EX11 formal library supply and actual model diagnostics.
use super::*;
use std::sync::atomic::Ordering::SeqCst;
fn evidence() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/performance/ex11-local-demonstrations")
}
fn save_json(path: impl AsRef<std::path::Path>, value: &Value) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn diagnostic_state() -> AppState {
    let mut state = state_named("ex11-diagnostic");
    // The shared unit-test fixture appends an uncovered source tail. Natural
    // authoring may search the book, so use a valid complete leaf here.
    let source = "X".repeat(100);
    state.book = Book::new(base_schema::sample_base(), &source).into();
    state.reader = Reader::new(&state.book, DEFAULT_RADIUS);
    std::fs::write(state.book_dir.join("source.txt"), source).unwrap();
    state
}
fn copy_revision_store(source: &std::path::Path, target: &std::path::Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_revision_store(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), destination).unwrap();
        }
    }
}

/// Observe the production adapter without changing requests or returned calls.
struct Ex11RecordingAdapter {
    inner: Ed3RecordingAdapter,
    started: std::time::Instant,
    completions: std::sync::atomic::AtomicUsize,
}
impl ModelAdapter for Ex11RecordingAdapter {
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile { self.inner.model_runtime_profile() }
    fn set_run_cancellation(&self, token: CancellationToken) { self.inner.set_run_cancellation(token); }
    fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, runtime::AdapterError> { self.inner.complete(request) }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, runtime::AdapterError> {
        let ordinal = self.completions.fetch_add(1, SeqCst);
        save_json(self.inner.evidence.join(format!("completion-{ordinal:02}-request.json")), &json!({
            "system":request.system,"user":request.user,"output_token_limit":request.output_token_limit,
            "reasoning_effort":request.reasoning_effort,"elapsed_ms":self.started.elapsed().as_millis(),
        }));
        let mut usage = None;
        let mut raw_text = String::new();
        let result = self.inner.inner.complete_structured_observed(request, &mut |delta| match delta {
            runtime::provider_stream::ModelDelta::Usage(value) => usage = Some(value),
            runtime::provider_stream::ModelDelta::Text(value) => raw_text.push_str(&value),
            _ => {},
        });
        save_json(self.inner.evidence.join(format!("completion-{ordinal:02}-response.json")), &json!({
            "response":result.as_ref().ok(),"error":result.as_ref().err().map(|e|&e.message),
            "usage":usage,"raw_text":raw_text,"elapsed_ms":self.started.elapsed().as_millis(),
        }));
        result
    }
    fn chat(&self, request: &runtime::AgentRequestPlan) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
        let ordinal = self.inner.requests.load(SeqCst);
        save_json(self.inner.evidence.join(format!("timing-{ordinal:02}-start.json")), &json!({
            "elapsed_ms":self.started.elapsed().as_millis(), "output_token_limit":request.output_token_limit,
            "runtime_profile":request.runtime_profile,
        }));
        let result = self.inner.chat(request);
        save_json(self.inner.evidence.join(format!("timing-{ordinal:02}-end.json")), &json!({
            "elapsed_ms":self.started.elapsed().as_millis(), "error":result.as_ref().err().map(|error| &error.message),
        }));
        result
    }
}

#[test]
fn ex11_library_write_read_revision_and_size_contract() {
    let (_temp, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(), plots: Default::default(),
    };
    let cancel = CancellationToken::default();
    let raw = json!({"operation":"write","title":"point","html":"<script>window.scene=Konva.version</script>","readable_content":"point", "libraries":["konva","konva"]}).to_string();
    let written = port
        .author_presentation(serde_json::from_str(&raw).unwrap(), &[], &[], &cancel)
        .unwrap();
    assert_eq!(written.body["libraries"][0]["version"], "10.7.0");
    let id = written.body["candidate_id"].as_str().unwrap();
    let candidate = app
        .with_app(|s| s.read_presentation_candidate(&turn.session_id, id))
        .unwrap();
    let html = &candidate.content.content_files["index.html"];
    assert_eq!(html.matches("data-presentation-library").count(), 1);
    assert!(html.find("data-presentation-library").unwrap() < html.find("window.scene").unwrap());
    assert_eq!(candidate.content.content_files.len(), 3);
    let path = written.body["libraries"][0]["path"].as_str().unwrap();
    let source = &candidate.content.content_files[path];
    assert!(source.len() > 190_000);
    assert!(!raw.contains(source));
    assert!(!written.body.to_string().contains(source));
    let reference = app
        .with_app(|s| s.persist_presentation_candidate(&turn.session_id, &turn.turn_id, id))
        .unwrap();
    // Link the version to completed history so subsequent reads use the actual ownership path.
    app.with_app(|s| {
        let outcome = OuterOutcome {
            answer: Some("point".into()),
            answer_view: Some(AgentAnswerView {
                parts: vec![AgentAnswerPart::Presentation {
                    presentation_id: reference.presentation_id.clone(),
                    revision: reference.revision,
                }],
                sources: vec![],
            }),
            incomplete: false,
            warning: None,
            turns: 1,
            tokens_spent: 0,
            effects: vec![],
            trace: vec![],
            profile_usage: Default::default(),
            memory_updates: vec![],
            source_bindings: vec![],
            delivery_diagnostics: None,
            request_audit: Default::default(),
        };
        finalize_agent_turn_completed(
            s,
            &turn,
            &outcome,
            &s.messages.clone(),
            "2026-09-28T04:00:00Z",
        )
        .unwrap();
    });
    for file in [
        None,
        Some(path.to_owned()),
        Some(
            written.body["libraries"][0]["license_path"]
                .as_str()
                .unwrap()
                .to_owned(),
        ),
    ] {
        let read = port
            .author_presentation(
                AuthorRequest::Read {
                    reference: reference.clone(),
                    file: file.clone(),
                    offset: 0,
                },
                &[],
                &[],
                &cancel,
            )
            .unwrap();
        assert!(!read.body.to_string().contains(source));
        assert_eq!(read.body["libraries"][0]["version"], "10.7.0");
        if file.is_some() {
            assert!(read.body.get("text").is_none());
        } else {
            assert_eq!(read.body["text"], *html);
        }
    }
    let revision_turn = app.with_app(|s| {
        let book = s.book.base.book_id.clone();
        precommit_agent_turn(
            s,
            &book,
            "revise".into(),
            None,
            None,
            None,
            "2026-09-28T04:01:00Z",
        )
        .unwrap()
    });
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &revision_turn,
        previewed: Default::default(),
        animations: Default::default(), plots: Default::default(),
    };
    for libraries in [json!(["konva"]), json!([])] {
        let revised = port.author_presentation(serde_json::from_value(json!({"operation":"write","title":"revised", "based_on":reference,"libraries":libraries,"html":html,"readable_content":"point"})).unwrap(),&[],&[],&cancel).unwrap();
        let next = app
            .with_app(|s| {
                s.read_presentation_candidate(
                    &turn.session_id,
                    revised.body["candidate_id"].as_str().unwrap(),
                )
            })
            .unwrap();
        let count = libraries.as_array().unwrap().len();
        assert_eq!(
            next.content.content_files["index.html"]
                .matches("data-presentation-library")
                .count(),
            count
        );
        assert_eq!(next.content.content_files.len(), 1 + 2 * count);
        assert_eq!(
            app.with_app(|s| s.read_presentation(&turn.session_id, &reference))
                .unwrap()
                .content,
            candidate.content
        );
    }
    let error = port.author_presentation(serde_json::from_value(json!({"operation":"write","title":"large","html":" ".repeat(900_000),"readable_content":"point","libraries":["konva"]})).unwrap(),&[],&[],&cancel).err().unwrap();
    assert!(error.message.contains("1 MiB"));
}
#[test]
#[ignore = "EX11 production Konva through author, three real previews and persisted version"]
fn ex11_preflight() {
    let (_temp, mut state, turn) = setup();
    let root = evidence().join("ex11.1");
    std::fs::create_dir_all(&root).unwrap();
    state.history_path = Some(root.join("history.json"));
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(), plots: Default::default(),
    };
    let raw = std::fs::read_to_string(evidence().join("scene.html")).unwrap();
    let html = raw;
    std::fs::write(root.join("raw.html"), &html).unwrap();
    let cancellation = CancellationToken::default();
    let written = port
        .author_presentation(
            AuthorRequest::Write {
                libraries: vec![PresentationLibrary::Konva],
                based_on: None,
                title: "拖动点与线段".into(),
                html,
                readable_content: "拖动蓝点，线段和位置读数同步变化。".into(),
                state_contract: json!({"x":"point horizontal coordinate in scene units"}),
                initial_state: json!({"x":80}),
                asset_refs: vec![],
                source_ref_ids: vec![],
                assumptions: vec![],
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
    save_json(root.join("write.json"), &written.body);
    let candidate_id = written.body["candidate_id"].as_str().unwrap().to_owned();
    for (name, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
        let result = port
            .author_presentation(
                AuthorRequest::Preview {
                    candidate_id: candidate_id.clone(),
                    width: None,
                    viewport: Some(viewport),
                    read_selector: Some("#readout".into()),
                    actions: vec![],
                },
                &[],
                &[],
                &cancellation,
            )
            .unwrap();
        save_json(root.join(format!("preview-{name}.json")), &result.body);
        assert!(
            matches!(
                result.body["status"].as_str(),
                Some("preview_ready_for_inspection" | "preview_environment_recorded")
            ),
            "{}",
            result.body
        );
    }
    let result = port
        .author_presentation(
            AuthorRequest::Deliver { candidate_id },
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
    save_json(root.join("deliver.json"), &result.body);
    let reference = result.delivered.unwrap();
    app.with_app(|state| {
        let outcome = OuterOutcome {
            answer: Some("拖动点与线段".into()),
            answer_view: Some(AgentAnswerView {
                parts: vec![AgentAnswerPart::Presentation {
                    presentation_id: reference.presentation_id.clone(),
                    revision: reference.revision,
                }],
                sources: vec![],
            }),
            incomplete: false,
            warning: None,
            turns: 1,
            tokens_spent: 0,
            effects: vec![],
            trace: vec![],
            profile_usage: Default::default(),
            memory_updates: vec![],
            source_bindings: vec![],
            delivery_diagnostics: None,
            request_audit: Default::default(),
        };
        finalize_agent_turn_completed(
            state,
            &turn,
            &outcome,
            &state.messages.clone(),
            "2026-09-28T04:00:00Z",
        )
        .unwrap();
        let version = state
            .read_presentation(&turn.session_id, &reference)
            .unwrap();
        save_json(
            root.join("content.json"),
            &serde_json::to_value(version.content).unwrap(),
        );
        let response = crate::presentation_api::route(
            state,
            &json!({"session_id":turn.session_id,"turn_id":turn.turn_id,"reference":reference})
                .to_string(),
            false,
        );
        assert_eq!(response.status, 200, "{}", response.body);
        std::fs::write(root.join("view.json"), response.body).unwrap();
    });
}

#[test]
#[ignore = "EX11 actual production method and fixed-library diagnostic"]
fn ex11_agent_diagnostic() {
    let root = std::path::PathBuf::from(std::env::var("EX11_RUN_DIR").expect("EX11_RUN_DIR"));
    // Never overwrite an original sample.
    std::fs::create_dir(&root).unwrap();
    let input_path = std::env::var("EX11_INPUT_FILE").map(std::path::PathBuf::from)
        .unwrap_or_else(|_| evidence().join("input.json"));
    let input: Value =
        serde_json::from_str(&std::fs::read_to_string(input_path).unwrap())
            .unwrap();
    save_json(root.join("input.json"), &input);
    let message = input["message"].as_str().unwrap();
    let mut state = diagnostic_state();
    state.history_path = Some(root.join("history.json"));
    let config = ProviderConfig::from_env().unwrap();
    let model = config.model.clone();
    let method = runtime::agent_prompt::policy_modules_for_tools(&[spec()]).into_iter()
        .find(|module| module.asset_id == "resident-agent.skill.presentation-method").unwrap().revision;
    let started = std::time::Instant::now();
    let adapter = Ex11RecordingAdapter { started, completions: Default::default(), inner: Ed3RecordingAdapter {
        inner: ProviderRegistry::adapter_from_config(config),
        baseline: false,
        evidence: root.clone(),
        requests: Default::default(),
    }};
    let prepared = prepare_agent_chat(
        &mut state,
        &json!({"message":message}).to_string(),
        "2026-09-28T04:00:00Z",
    )
    .unwrap_or_else(|reply| panic!("prepare: {}", reply.body));
    let turn_ref = prepared.turn_ref.clone();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let report =
        crate::agent_run::execute_prepared(&app, &adapter, prepared, CancellationToken::default());
    std::fs::write(root.join("outcome.json"), &report.reply.body).unwrap();
    save_json(
        root.join("summary.json"),
        &json!({"method":method,"model":model,"status":report.reply.status,
        "elapsed_ms":started.elapsed().as_millis(),"model_requests":adapter.inner.requests.load(SeqCst),"structured_completions":adapter.completions.load(SeqCst)}),
    );
    if report.reply.status == 200 {
        let outcome: OuterOutcome = serde_json::from_str(&report.reply.body).unwrap();
        if let Some(reference) = outcome.answer_view.and_then(|view| {
            view.parts.into_iter().find_map(|part| match part {
                AgentAnswerPart::Presentation {
                    presentation_id,
                    revision,
                } => Some(runtime::presentation::PresentationRef {
                    presentation_id,
                    revision,
                }),
                _ => None,
            })
        }) {
            app.with_app(|state| {
                let version = state.read_presentation(&turn_ref.session_id,&reference).unwrap();
                save_json(root.join("content.json"), &serde_json::to_value(version.content).unwrap());
                let response = crate::presentation_api::route(state,&json!({"session_id":turn_ref.session_id,"turn_id":turn_ref.turn_id,"reference":reference}).to_string(),false);
                std::fs::write(root.join("view.json"),response.body).unwrap();
            });
        }
    }
    assert_eq!(report.reply.status, 200, "provider/runtime failure retained in outcome.json: {}", report.reply.body);
}

#[test]
#[ignore = "EX11 follow-up exercises actual read, managed metadata and based_on model history"]
fn ex11_agent_revision() {
    let base = std::path::PathBuf::from(std::env::var("EX11_BASE_DIR").unwrap());
    let root = base.join(std::env::var("EX11_REVISION_NAME").unwrap_or_else(|_| "revision".into()));
    std::fs::create_dir(&root).unwrap();
    let mut state = diagnostic_state();
    state.history_path = Some(base.join("history.json"));
    state.agent_history = load_agent_history(&state.history_path).unwrap();
    std::fs::copy(base.join("history.json"), root.join("history-before.json")).unwrap();
    // Keep the original run immutable; continuation history belongs to this revision.
    copy_revision_store(&base.join("history.presentations"), &root.join("history.presentations"));
    state.history_path = Some(root.join("history.json"));
    let view: Value =
        serde_json::from_str(&std::fs::read_to_string(std::path::PathBuf::from(std::env::var("EX11_REFERENCE_DIR").unwrap_or_else(|_| base.to_string_lossy().into_owned())).join("view.json")).unwrap()).unwrap();
    let message = format!("请修订已交付页面 {}：先读取页面与 libraries/konva-10.7.0.min.js 的托管元数据，再把标题加上‘（修订）’，保留交互与布局。实际调用 write(based_on) 并完成三视口预览和交付。",view["reference"]);
    let message = match std::env::var("EX11_REVISION_INPUT") {
        Ok(path) => format!("修订确切版本 {}。{}", view["reference"], std::fs::read_to_string(path).unwrap()),
        Err(_) => message,
    };
    save_json(root.join("input.json"), &json!({"message":message}));
    let config = ProviderConfig::from_env().unwrap();
    let model = config.model.clone();
    let adapter = Ex11RecordingAdapter { started: std::time::Instant::now(), completions: Default::default(), inner: Ed3RecordingAdapter {
        inner: ProviderRegistry::adapter_from_config(config),
        baseline: false,
        evidence: root.clone(),
        requests: Default::default(),
    }};
    let prepared = prepare_agent_chat(
        &mut state,
        &json!({"message":message}).to_string(),
        "2026-09-28T05:00:00Z",
    )
    .unwrap_or_else(|r| panic!("{}", r.body));
    let turn_ref = prepared.turn_ref.clone();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let started = std::time::Instant::now();
    let report =
        crate::agent_run::execute_prepared(&app, &adapter, prepared, CancellationToken::default());
    std::fs::write(root.join("outcome.json"), &report.reply.body).unwrap();
    save_json(
        root.join("summary.json"),
        &json!({"model":model,"elapsed_ms":started.elapsed().as_millis(),"model_requests":adapter.inner.requests.load(SeqCst),"structured_completions":adapter.completions.load(SeqCst)}),
    );
    assert_eq!(report.reply.status, 200, "provider/runtime failure retained in outcome.json: {}", report.reply.body);
    let outcome: OuterOutcome = serde_json::from_str(&report.reply.body).unwrap();
    let reference = outcome
        .answer_view
        .and_then(|v| {
            v.parts.into_iter().find_map(|p| match p {
                AgentAnswerPart::Presentation {
                    presentation_id,
                    revision,
                } => Some(runtime::presentation::PresentationRef {
                    presentation_id,
                    revision,
                }),
                _ => None,
            })
        })
        .expect("revision was not delivered; inspect retained evidence");
    app.with_app(|s| {
        let version = s.read_presentation(&turn_ref.session_id,&reference).unwrap();
        save_json(root.join("content.json"),&serde_json::to_value(&version.content).unwrap());
        let response = crate::presentation_api::route(s,&json!({"session_id":turn_ref.session_id,"turn_id":turn_ref.turn_id,"reference":reference}).to_string(),false);
        std::fs::write(root.join("view.json"),response.body).unwrap();
        // Library presence is audited against the original artifact. A transfer
        // sample may legitimately use SVG rather than the EX11.2 Konva fixture.
    });
}
