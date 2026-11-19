pub use reader_service::tests::presentation_store_tests::*;
use crate::agent_run::AppStatePort;
use super::*;
use runtime::presentation::*;


#[path = "presentation_se4_tests.rs"]
mod se4;

fn learning_artifacts_with_upload_storage(condition: &str) {
    let (root, mut state, _) = fixture();
    let service_root = root.path().join("service");
    let mut users = crate::user_registry::UserRegistry::open(&service_root).unwrap();
    users.create_user("A").unwrap();
    let control = crate::control_store::ControlStore::open(users.writer()).unwrap();
    let now = crate::multi_user_host::now();
    let upload_capacity = 16 * 1024;
    if condition != "missing" {
        let end = if condition == "expired" { now - 1 } else { now + 1000 };
        control.connection.execute(
            "INSERT INTO allowance_periods(period_id,user_id,starts_at,expires_at) VALUES('p','A',?,?)",
            rusqlite::params![now - 1000, end],
        ).unwrap();
        crate::cloud_storage::tests::seed(&control.connection, "A", "p", upload_capacity, upload_capacity);
        if condition == "full" {
            let path = service_root.join("original.md");
            std::fs::write(&path, vec![b'x'; upload_capacity as usize]).unwrap();
            crate::cloud_storage::reserve(&control.connection, "A", "workspace", "original", "original.md", &path, upload_capacity, now).unwrap();
            crate::cloud_storage::uploaded(&control.connection, "A", "original", now).unwrap();
        }
    }
    let paths = crate::user_storage_paths::UserStoragePaths::for_service(&service_root, "A").unwrap();
    state.user = crate::user_registry::open_service(&paths, users.writer(), "now").unwrap();
    state.workspace.selected_chat = None;
    assert_eq!(route_agent_new(&mut state, "now").status, 200);
    let turn = next_turn(&mut state);
    let candidate = create(&mut state, &turn, None, "Saved learning page");
    let reference = persist(&mut state, &turn, &candidate);
    finish(&mut state, &turn, &reference).unwrap();
    let scene = PresentationState { observation: None, values: json!({"count":2}), visible_step: None,
        observed_result: "Two pieces".into(), source_ref_ids: vec!["source-rp2".into()] };
    let receipt = state.private_context().test_save_presentation_state(&turn.session_id, &turn.turn_id, &reference, scene.clone()).unwrap();

    // Reopen the real service user's files and read through the public presentation route.
    state.user = crate::user_registry::open_service(&paths, users.writer(), "later").unwrap();
    assert_eq!(state.private_context().test_read_presentation_state(&receipt).unwrap().state, scene);
    let response = post(&mut state, "/agent/presentation.read", &json!({
        "session_id":turn.session_id,"turn_id":turn.turn_id,"reference":reference,
    }).to_string());
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(serde_json::from_str::<Value>(&response.body).unwrap()["title"], "Saved learning page");
    let space = crate::cloud_storage::summary(&control.connection, "A", now).unwrap();
    assert_eq!(space["used_bytes"], if condition == "full" { upload_capacity } else { 0 });
    assert_eq!(space["reserved_bytes"], 0);
    assert_eq!(space["can_upload"], false);
}

#[test]
fn learning_artifacts_save_without_a_storage_plan() {
    learning_artifacts_with_upload_storage("missing");
}

#[test]
fn learning_artifacts_save_after_storage_plan_expiry() {
    learning_artifacts_with_upload_storage("expired");
}

#[test]
fn learning_artifacts_save_when_upload_space_is_full() {
    learning_artifacts_with_upload_storage("full");
}

#[test]
fn rn1_presentation_notes_bind_saved_scene_across_both_command_paths() {
    let (root, mut state, turn) = fixture();
    let memory_path = root.path().join("memory.json");
    state.user.store = MemoryStore::open(&memory_path).unwrap();
    let candidate = create(&mut state, &turn, None, "Original scene");
    let reference = persist(&mut state, &turn, &candidate);
    finish(&mut state, &turn, &reference).unwrap();
    let scene = |count| PresentationState { observation: None, values: json!({"count":count}), visible_step: Some("step-2".into()), observed_result: "Two pieces".into(), source_ref_ids: vec!["source-rp2".into()] };
    let first = state.private_context().test_save_presentation_state(&turn.session_id, &turn.turn_id, &reference, scene(2)).unwrap();
    let second = state.private_context().test_save_presentation_state(&turn.session_id, &turn.turn_id, &reference, scene(3)).unwrap();
    let request = |receipt: &PresentationFollowUp| json!({"type":"note","content":"我的发现","note":{"association":{"kind":"presentation","receipt":receipt}}});
    // The creation association remains the explicitly selected scene after switching chats.
    route_agent_new(&mut state, "other chat");
    let saved = post(&mut state, "/memory/save", &request(&first).to_string());
    assert_eq!(saved.status, 200, "{}", saved.body);
    let saved: Value = serde_json::from_str(&saved.body).unwrap();
    let record = &saved["record"];
    assert_eq!(record["note"]["association"]["receipt"], json!(first));
    assert_eq!(record["note"]["association"]["title"], "Original scene");
    assert_eq!(record["note"]["source_bindings"][0], json!(candidate.content.source_bindings[0]));
    assert!(record["anchor"].as_object().unwrap().is_empty());
    assert_eq!(record["source_session_id"], first.session_id);
    let duplicate = workspace_client::memory(&mut state.user, &state.workspace, "memory/save", &request(&first), "retry").unwrap();
    assert_eq!(duplicate["status"], "EXISTING"); assert_eq!(duplicate["record"], *record);
    let another = workspace_client::memory(&mut state.user, &state.workspace, "memory/save", &request(&second), "second").unwrap();
    assert_ne!(another["record"]["mem_id"], record["mem_id"]);
    let edited = workspace_client::memory(&mut state.user, &state.workspace, "memory/replace", &json!({"mem_id":record["mem_id"],"content":"修订想法"}), "edited").unwrap();
    assert_eq!(edited["note"], record["note"]); assert_eq!(edited["generated_at"], record["generated_at"]);
    let before = state.user.store.recall(&RecallQuery::default());
    let mut bad = first.clone(); bad.saved_state_ref = "invented".into();
    assert_ne!(post(&mut state, "/memory/save", &request(&bad).to_string()).status, 200);
    bad = first.clone(); bad.reference.revision += 1;
    assert_ne!(post(&mut state, "/memory/save", &request(&bad).to_string()).status, 200);
    let mut empty = request(&first); empty["content"] = json!("");
    assert_ne!(post(&mut state, "/memory/save", &empty.to_string()).status, 200);
    assert_eq!(state.user.store.recall(&RecallQuery::default()), before);
    let disk = std::fs::read(&memory_path).unwrap();
    std::fs::create_dir(memory_path.with_extension("replace.tmp")).unwrap();
    let mut failed = request(&first); failed["content"] = json!("write must fail");
    assert_ne!(post(&mut state, "/memory/save", &failed.to_string()).status, 200);
    assert!(workspace_client::memory(&mut state.user, &state.workspace, "memory/replace", &json!({"mem_id":edited["mem_id"],"content":"write must fail"}), "failed").is_err());
    assert_eq!(state.user.store.recall(&RecallQuery::default()), before);
    assert_eq!(std::fs::read(memory_path).unwrap(), disk);
}

#[test]
fn rn1_answer_admission_preserves_original_turn_sources_and_publication() {
    let (_root, mut state, turn) = fixture();
    let request = json!({"type":"note","content":"","note":{"association":{"kind":"answer","session_id":turn.session_id,"turn_id":turn.turn_id},"retained_excerpt":"Delivered explanation"}});
    assert_ne!(post(&mut state, "/memory/save", &request.to_string()).status, 200);
    let sources = content(&state, "source").source_bindings;
    let stored = &mut state.user.agent_history.sessions[0].turns[0];
    stored.status = AgentAssistantStatus::Completed;
    let mut answer = outcome(&PresentationRef { presentation_id: "unused".into(), revision: 1 });
    answer.answer = Some("Delivered explanation".into());
    answer.answer_view = Some(AgentAnswerView { parts: vec![AgentAnswerPart::Markdown { text: "Delivered explanation".into() }], sources: vec![] });
    stored.outcome = Some(answer); stored.source_bindings = sources.clone();
    let saved = workspace_client::memory(&mut state.user, &state.workspace, "memory/save", &request, "created").unwrap();
    assert_eq!(saved["record"]["note"]["retained_excerpt"], json!({"kind":"assistant","text":"Delivered explanation"}));
    assert_eq!(saved["record"]["note"]["source_bindings"], json!(sources));
    assert!(saved["record"]["anchor"].as_object().unwrap().is_empty());
    let mut wrong = request.clone(); wrong["note"]["retained_excerpt"] = json!("not delivered");
    assert!(workspace_client::memory(&mut state.user, &state.workspace, "memory/save", &wrong, "wrong").is_err());
    wrong = request.clone(); wrong["note"]["association"]["turn_id"] = json!("missing");
    assert_ne!(post(&mut state, "/memory/save", &wrong.to_string()).status, 200);
    state.user.agent_history.sessions[0].turns[0].published_book_ref = Some(published_library::PublishedBookRef { book_id: state.workspace.book.base.book_id.clone(), publication_id: "original-edition".into() });
    assert!(workspace_client::memory(&mut state.user, &state.workspace, "memory/save", &request, "wrong-publication").is_err());
    let edited = workspace_client::memory(&mut state.user, &state.workspace, "memory/replace", &json!({"mem_id":saved["record"]["mem_id"],"content":"my thought"}), "edit").unwrap();
    assert_eq!(edited["note"], saved["record"]["note"]);
    assert_eq!(edited["generated_at"], "created");
}

#[test]
fn rn1_answer_excerpt_crosses_inline_sources_but_not_separate_surfaces() {
    let (_root, mut state, turn) = fixture();
    let sources = content(&state, "source").source_bindings;
    let stored = &mut state.user.agent_history.sessions[0].turns[0];
    stored.status = AgentAssistantStatus::Completed;
    let mut answer = outcome(&PresentationRef { presentation_id: "unused".into(), revision: 1 });
    answer.answer_view = Some(AgentAnswerView { parts: vec![
        AgentAnswerPart::Markdown { text: "低音以 A 为底，".into() },
        AgentAnswerPart::Sources { source_ref_ids: vec![sources[0].source_ref_id.clone()] },
        AgentAnswerPart::Markdown { text: "三音取 **C 还原**的音响。".into() },
        AgentAnswerPart::SocraticQuestion { text: "你听到了什么？".into(), purpose: None },
        AgentAnswerPart::Presentation { presentation_id: "unused".into(), revision: 1 },
        AgentAnswerPart::Markdown { text: "另一段回答。".into() },
    ], sources: vec![] });
    stored.outcome = Some(answer); stored.source_bindings = sources.clone();
    let request = |text: &str| json!({"type":"note","content":"","note":{"association":{"kind":"answer","session_id":turn.session_id,"turn_id":turn.turn_id},"retained_excerpt":text}});
    for text in ["低音以 A 为底，三音取 **C 还原**的音响。", "以 A 为底，三音取 **C", "你听到了什么？"] {
        let saved = workspace_client::memory(&mut state.user, &state.workspace, "memory/save", &request(text), "created").unwrap();
        assert_eq!(saved["record"]["note"]["retained_excerpt"], json!({"kind":"assistant","text":text}));
        assert_eq!(saved["record"]["note"]["source_bindings"], json!(sources));
    }
    for text in ["音响。你听到了什么？", "你听到了什么？另一段回答。", "伪造的解释"] {
        assert!(workspace_client::memory(&mut state.user, &state.workspace, "memory/save", &request(text), "invalid").is_err());
    }
}

#[test]
fn rn1_original_selection_and_body_placement_keep_distinct_content_identity() {
    let (_root, mut state, _) = fixture();
    let selection = json!({"status":"resolved","raw_quote":"XX","resolved_quote":"XX","ranges":[{"lid":"1.1","range":{"start":0,"end":2}}]});
    let request = json!({"type":"note","content":"my thought","selection_context":selection,"note":{"association":{"kind":"selection"},"retained_excerpt":"XX"}});
    let saved = post(&mut state, "/memory/save", &request.to_string());
    assert_eq!(saved.status, 200, "{}", saved.body);
    let saved: Value = serde_json::from_str(&saved.body).unwrap();
    assert_eq!(saved["record"]["content"], "my thought");
    assert_eq!(saved["record"]["note"]["retained_excerpt"]["kind"], "original");
    let edited = post(&mut state, "/memory/replace", &json!({"mem_id":saved["record"]["mem_id"],"content":"edited"}).to_string());
    assert_eq!(edited.status, 200, "{}", edited.body);
    let edited: Value = serde_json::from_str(&edited.body).unwrap();
    assert_eq!(edited["selection_context"], selection); assert_eq!(edited["note"], saved["record"]["note"]);
    let mut wrong_selection = selection.clone(); wrong_selection["ranges"][0]["lid"] = json!("missing-lid");
    let wrong = json!({"mem_id":edited["mem_id"],"content":"not committed","selection_context":wrong_selection});
    assert_ne!(post(&mut state, "/memory/replace", &wrong.to_string()).status, 200);
    assert!(workspace_client::memory(&mut state.user, &state.workspace, "memory/replace", &wrong, "invalid").is_err());
    let source = state.workspace.book.source_fingerprint();
    let placement = json!({"kind":"lid_block","source_fingerprint":source,"lid":"1.1"});
    let placed = post(&mut state, "/memory/save", &json!({"type":"note","content":"placed","note_placement":placement,"note":{"association":{"kind":"body_placement"}}}).to_string());
    assert_eq!(placed.status, 200, "{}", placed.body);
    let placed: Value = serde_json::from_str(&placed.body).unwrap();
    assert_eq!(placed["record"]["note"]["association"]["placement"], placement);
}
#[test]
fn rn2_retained_version_state_resources_sources_and_current_chat_survive_delete_and_reopen() {
    let (root, mut state, first_turn) = fixture();
    let memory_path = root.path().join("memory.json");
    state.user.store = MemoryStore::open(&memory_path).unwrap();
    let first = create(&mut state, &first_turn, None, "Version one");
    let first_ref = persist(&mut state, &first_turn, &first);
    finish(&mut state, &first_turn, &first_ref).unwrap();
    let turn = next_turn(&mut state);
    let mut draft = content(&state, "Recorded version two");
    draft.content_files.insert("assets/chart.svg".into(), "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"5\"/></svg>".into());
    let second = state.private_context().create_presentation_candidate(&turn.session_id, &turn.turn_id, Some(first_ref), draft.clone()).unwrap();
    let reference = persist(&mut state, &turn, &second);
    assert_eq!(reference.revision, 2);
    finish(&mut state, &turn, &reference).unwrap();
    let scene = |count| PresentationState { observation: None, values: json!({"count":count}), visible_step: Some("explain".into()), observed_result: "Two of three".into(), source_ref_ids: vec!["source-rp2".into()] };
    let receipt = state.private_context().test_save_presentation_state(&turn.session_id, &turn.turn_id, &reference, scene(2)).unwrap();
    let save = |text| json!({"type":"note","content":text,"note":{"association":{"kind":"presentation","receipt":receipt}}});
    let a = workspace_client::memory(&mut state.user, &state.workspace, "memory/save", &save("First thought"), "first").unwrap();
    let b = workspace_client::memory(&mut state.user, &state.workspace, "memory/save", &save("Second thought"), "second").unwrap();
    state.private_context().test_save_presentation_state(&turn.session_id, &turn.turn_id, &reference, scene(3)).unwrap();
    let third_turn = next_turn(&mut state);
    let third = create(&mut state, &third_turn, Some(reference.clone()), "Version three");
    let third_ref = persist(&mut state, &third_turn, &third);
    finish(&mut state, &third_turn, &third_ref).unwrap();
    assert_eq!(third_ref.revision, 3);
    let deleted = post(&mut state, "/agent/history/delete", &json!({"session_id":turn.session_id}).to_string());
    assert_eq!(deleted.status, 200, "{}", deleted.body);
    let history_path = state.user.history_path.clone();
    state.user.agent_history = load_agent_history(&history_path).unwrap();
    state.user.store = MemoryStore::open(&memory_path).unwrap();
    assert!(!state.user.agent_history.sessions.iter().any(|s| s.id == turn.session_id));
    let request = json!({"mem_id":a["record"]["mem_id"],"restore":true});
    let restored = workspace_client::read(&state.user, &state.workspace, "memory/presentation.read", &request, "read").unwrap().unwrap();
    assert_eq!(restored["reference"], json!(reference));
    assert_eq!(restored["restored_state"], json!(scene(2)));
    assert_eq!(restored["content_files"], json!(draft.content_files));
    assert_eq!(restored["sources"][0]["source_ref_id"], "source-rp2");
    let mut plain = request.clone(); plain["restore"] = json!(false);
    assert!(reader_service::tests::host_calls::note_api::presentation(&state.user, &state.workspace, &plain, false).unwrap()["restored_state"].is_null());
    let read = post(&mut state, "/memory/presentation.read", &request.to_string());
    assert_eq!(read.status, 200, "{}", read.body);
    let observation = json!({"mem_id":a["record"]["mem_id"],"text":"Two of three","source_ref_ids":["source-rp2"]});
    assert_eq!(post(&mut state, "/memory/presentation.observe", &observation.to_string()).status, 200);
    let source = json!({"turn_id":turn.turn_id,"source_ref_id":"source-rp2","note_mem_id":a["record"]["mem_id"]});
    assert_eq!(post(&mut state, "/agent/source.resolve", &source.to_string()).status, 200);
    assert_eq!(post(&mut state, "/agent/source.open", &source.to_string()).status, 200);
    // Client-supplied version/state is not authority; the persisted note decides.
    let mut forged = request.clone(); forged["saved_state"] = json!({"reference":third_ref});
    assert_eq!(reader_service::tests::host_calls::note_api::presentation(&state.user, &state.workspace, &forged, false).unwrap()["reference"], json!(reference));
    let follow = json!({"message":"Explain recorded result","note_mem_id":a["record"]["mem_id"]});
    let selected = state.workspace.selected_chat.clone().unwrap();
    let validated = validate_agent_input(&mut state.user, &state.workspace, &follow.to_string()).map_err(|e| e.body).unwrap();
    assert_eq!(validated.presentation_follow_up, Some(receipt.clone()));
    assert!(validated.presentation_context.unwrap().contains("\"count\":2"));
    assert_eq!(state.workspace.selected_chat.as_ref(), Some(&selected));
    let pending = precommit_agent_turn(&mut state, &draft.source_bindings[0].book_id, "follow".into(), None, None, Some(receipt.clone()), "now").unwrap();
    let scope = crate::run_scope::RunScope::capture(&state, &pending, "follow", None, Some(receipt.clone()));
    let shared = Arc::new(Mutex::new(state));
    // The original files can be read on demand from the new conversation's committed receipt.
    let result = crate::presentation_author::AuthorSession {
        storage: &TestAuthorStorage { state: &shared }, turn_ref: &pending,
        previewed: &mut Default::default(), animations: &mut Default::default(), plots: &mut Default::default(), sandbox: None,
    }.author(runtime::presentation_author::AuthorRequest::Read { reference: Some(reference.clone()), candidate_id: None, file: Some("readable_content".into()), offset: 0, length: None }, &[], &[], &runtime::run_context::CancellationToken::default()).unwrap();
    assert!(result.body.to_string().contains("Recorded version two"));
    drop(scope);
    let mut state = shared.lock().unwrap();
    state.user.store.delete(a["record"]["mem_id"].as_str().unwrap()).unwrap();
    assert!(reader_service::tests::host_calls::note_api::presentation(&state.user, &state.workspace, &request, false).is_err());
    assert!(reader_service::tests::host_calls::note_api::presentation(&state.user, &state.workspace, &json!({"mem_id":b["record"]["mem_id"],"restore":true}), false).is_ok());
    state.workspace.selected_chat = None;
    assert!(reader_service::tests::host_calls::note_api::follow_up(&state.user, &state.workspace, b["record"]["mem_id"].as_str().unwrap()).is_err());
}

#[test]
fn cross_book_presentation_sources_survive_delivery_note_reuse_and_learning() {
    let (root, mut state, turn) = fixture();
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    let mut draft = content(&state, "Rust 借用补讲");
    let source = &mut draft.source_bindings[0];
    source.book_id = "reference-rust".into();
    source.publication_id = Some("rust-edition-1".into());
    source.source_revision = Some("rust-source-revision".into());
    source.label_snapshot = "《Rust》 · 借用".into();
    let sources = draft.source_bindings.clone();
    let candidate = state.private_context().create_presentation_candidate(&turn.session_id, &turn.turn_id, None, draft).unwrap();
    let reference = persist(&mut state, &turn, &candidate);
    finish(&mut state, &turn, &reference).unwrap();
    assert_eq!(state.user.agent_history.sessions[0].turns[0].source_bindings, sources);
    crate::teaching::record_user_delivery(&state.user, &turn, "now").unwrap();
    crate::teaching::record_user_delivery(&state.user, &turn, "retry").unwrap();
    assert_eq!(state.user.learning_store().unwrap().teaching_recent("observation:reference-rust", 10).unwrap().len(), 1);
    let request = json!({"session_id":turn.session_id,"turn_id":turn.turn_id,"reference":reference});
    let public = post(&mut state, "/agent/presentation.read", &request.to_string());
    assert_eq!(public.status, 200, "{}", public.body);
    let public: Value = serde_json::from_str(&public.body).unwrap();
    assert_eq!(public["sources"][0]["label"], sources[0].label_snapshot);
    let receipt = state.private_context().test_save_presentation_state(&turn.session_id, &turn.turn_id, &reference,
        PresentationState {observation:None,values:json!({"count":2}),visible_step:None,observed_result:"补讲".into(),source_ref_ids:vec!["source-rp2".into()]}).unwrap();
    let note = workspace_client::memory(&mut state.user, &state.workspace, "memory/save",
        &json!({"type":"note","content":"借用时仍有所有者","note":{"association":{"kind":"presentation","receipt":receipt}}}), "saved").unwrap();
    assert_eq!(note["record"]["note"]["source_bindings"], json!(sources));
    let next = next_turn(&mut state);
    finish(&mut state, &next, &reference).unwrap();
    crate::teaching::record_user_delivery(&state.user, &next, "later").unwrap();
    assert_eq!(state.user.agent_history.sessions[0].turns[1].source_bindings, sources);
    let facts = state.user.learning_store().unwrap().teaching_recent("observation:reference-rust", 10).unwrap();
    assert_eq!(facts.len(), 2);
    assert!(facts.iter().all(|f| f.binding.source_revision == "rust-source-revision"));
    let history = load_agent_history(&state.user.history_path).unwrap();
    assert_eq!(history.sessions[0].turns[1].source_bindings, sources);
}