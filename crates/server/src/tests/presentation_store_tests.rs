use crate::agent_run::AppStatePort;
use super::*;
use runtime::presentation::*;

#[path = "presentation_ex13_tests.rs"]
mod ex13;

fn fixture() -> (tempfile::TempDir, AppState, AgentTurnRef) {
    let root = tempfile::tempdir().unwrap();
    let name = format!("rp2-presentation-{}", root.path().file_name().unwrap().to_string_lossy());
    let mut state = state_named(&name);
    state.user.history_path = Some(root.path().join("agent-history.json"));
    let turn = next_turn(&mut state);
    (root, state, turn)
}

#[test]
fn rn1_presentation_notes_bind_saved_scene_across_both_command_paths() {
    let (root, mut state, turn) = fixture();
    let memory_path = root.path().join("memory.json");
    state.user.store = MemoryStore::open(&memory_path).unwrap();
    let candidate = create(&mut state, &turn, None, "Original scene");
    let reference = persist(&mut state, &turn, &candidate);
    finish(&mut state, &turn, &reference).unwrap();
    let scene = |count| PresentationState { values: json!({"count":count}), visible_step: Some("step-2".into()), observed_result: "Two pieces".into(), source_ref_ids: vec!["source-rp2".into()] };
    let first = state.private_context().save_presentation_state(&turn.session_id, &turn.turn_id, &reference, scene(2)).unwrap();
    let second = state.private_context().save_presentation_state(&turn.session_id, &turn.turn_id, &reference, scene(3)).unwrap();
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
    let scene = |count| PresentationState { values: json!({"count":count}), visible_step: Some("explain".into()), observed_result: "Two of three".into(), source_ref_ids: vec!["source-rp2".into()] };
    let receipt = state.private_context().save_presentation_state(&turn.session_id, &turn.turn_id, &reference, scene(2)).unwrap();
    let save = |text| json!({"type":"note","content":text,"note":{"association":{"kind":"presentation","receipt":receipt}}});
    let a = workspace_client::memory(&mut state.user, &state.workspace, "memory/save", &save("First thought"), "first").unwrap();
    let b = workspace_client::memory(&mut state.user, &state.workspace, "memory/save", &save("Second thought"), "second").unwrap();
    state.private_context().save_presentation_state(&turn.session_id, &turn.turn_id, &reference, scene(3)).unwrap();
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
    assert!(note_api::presentation(&state.user, &state.workspace, &plain, false).unwrap()["restored_state"].is_null());
    let read = post(&mut state, "/memory/presentation.read", &request.to_string());
    assert_eq!(read.status, 200, "{}", read.body);
    let observation = json!({"mem_id":a["record"]["mem_id"],"text":"Two of three","source_ref_ids":["source-rp2"]});
    assert_eq!(post(&mut state, "/memory/presentation.observe", &observation.to_string()).status, 200);
    let source = json!({"turn_id":turn.turn_id,"source_ref_id":"source-rp2","note_mem_id":a["record"]["mem_id"]});
    assert_eq!(post(&mut state, "/agent/source.resolve", &source.to_string()).status, 200);
    assert_eq!(post(&mut state, "/agent/source.open", &source.to_string()).status, 200);
    // Client-supplied version/state is not authority; the persisted note decides.
    let mut forged = request.clone(); forged["saved_state"] = json!({"reference":third_ref});
    assert_eq!(note_api::presentation(&state.user, &state.workspace, &forged, false).unwrap()["reference"], json!(reference));
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
    assert!(note_api::presentation(&state.user, &state.workspace, &request, false).is_err());
    assert!(note_api::presentation(&state.user, &state.workspace, &json!({"mem_id":b["record"]["mem_id"],"restore":true}), false).is_ok());
    state.workspace.selected_chat = None;
    assert!(note_api::follow_up(&state.user, &state.workspace, b["record"]["mem_id"].as_str().unwrap()).is_err());
}

struct TestAuthorStorage<'a> { state: &'a Arc<Mutex<AppState>> }
impl crate::presentation_author::AuthorStorage for TestAuthorStorage<'_> {
    fn with_private<R>(&self, operation: impl FnOnce(&PrivateBookContext<'_>) -> Result<R, ToolError>) -> Result<R, ToolError> {
        operation(&self.state.lock().unwrap().private_context())
    }
}

fn next_turn(state: &mut AppState) -> AgentTurnRef {
    let book_id = state.workspace.book.base.book_id.clone();
    precommit_agent_turn(
        state,
        &book_id,
        "Explain recall".into(),
        None,
        None,
        None,
        "2026-09-16T16:00:00Z",
    )
    .unwrap()
}

#[test]
fn jl9_recap_can_open_a_committed_presentation_when_terminal_save_is_interrupted() {
    let (_root, mut state) = super::session_runtime_tests::setup();
    let turn = next_turn(&mut state);
    let candidate = create(&mut state, &turn, None, "Delivered before interruption");
    let reference = persist(&mut state, &turn, &candidate);
    let request = json!({"session_id":turn.session_id,"turn_id":turn.turn_id,"reference":reference});
    assert_ne!(presentation_api::route(&state.private_context(), &request.to_string(), false).status, 200);
    crate::session_runtime::append(&mut state.user, &turn, "delivered", crate::session_event::EventBody::EffectDelivered {
        effect_id: format!("presentation:{}:{}", reference.presentation_id, reference.revision),
        effect: crate::session_event::DeliveredEffect::Presentation { reference: reference.clone() },
    }).unwrap();
    let (history, store) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    state.user.agent_history = history; state.user.session_store = store;
    let response = presentation_api::route(&state.private_context(), &request.to_string(), false);
    assert_eq!(response.status, 200, "{}", response.body);
    assert!(presentation_api::source_binding(&state.private_context(), &turn.turn_id, "source-rp2").is_some());
    let recap = json!(crate::session_recap::local(&state, &HashMap::from([("session_id".into(), turn.session_id.clone())]), "recap").unwrap());
    assert!(recap["effects"][0]["unavailable_reason"].is_null());
    assert_eq!(recap["effects"][0]["effect"]["reference"], json!(reference));
}
fn content(state: &AppState, title: &str) -> PresentationContent {
    let evidence_range = EvidenceRange {
        start_lid: "1.1".into(),
        end_lid: "1.1".into(),
        ranges: vec![],
    };
    let source = state.workspace.book
        .resolve_source(&evidence_range, "zh-CN", None)
        .unwrap();
    PresentationContent {
        animation_assets: Default::default(),
        title: title.into(),
        content_files: BTreeMap::from([(
            "index.html".into(),
            format!("<h1>{title}</h1><script>window.count=2</script>"),
        )]),
        entrypoint: "index.html".into(),
        readable_content: format!("{title}: two of three pieces of evidence"),
        source_bindings: vec![SourceBinding {
            source_ref_id: "source-rp2".into(),
            book_id: state.workspace.book.base.book_id.clone(),
            evidence_range,
            evidence_text_digest: source.evidence_text_digest,
            label_snapshot: source.label,
            preview_snapshot: source.preview,
        }],
        assumptions: vec!["Three required pieces".into()],
        state_contract: json!({"count":"integer from 0 to 3"}),
        initial_state: json!({"count":2}),
    }
}
pub(super) fn create(
    state: &mut AppState,
    turn: &AgentTurnRef,
    based_on: Option<PresentationRef>,
    title: &str,
) -> PresentationCandidate {
    let content = content(state, title);
    state.private_context().create_presentation_candidate(&turn.session_id, &turn.turn_id, based_on, content)
        .unwrap()
}
pub(super) fn persist(
    state: &mut AppState,
    turn: &AgentTurnRef,
    candidate: &PresentationCandidate,
) -> PresentationRef {
    state.private_context().persist_presentation_candidate(&turn.session_id, &turn.turn_id, &candidate.candidate_id)
        .unwrap()
}
fn outcome(reference: &PresentationRef) -> OuterOutcome {
    OuterOutcome {
        answer: Some("Recall example".into()),
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
    }
}
fn finish(
    state: &mut AppState,
    turn: &AgentTurnRef,
    reference: &PresentationRef,
) -> Result<(), ToolError> {
    let messages = state.workspace.messages.clone();
    finalize_agent_turn_completed(
        state,
        turn,
        &outcome(reference),
        &messages,
        "2026-09-16T16:01:00Z",
    )
}

#[test]
fn presentation_versions_reopen_with_original_history_and_explicit_edit_base() {
    let (_root, mut state, first_turn) = fixture();
    let first = create(&mut state, &first_turn, None, "Original");
    let first_ref = persist(&mut state, &first_turn, &first);
    assert_eq!(first_ref.revision, 1);
    assert_eq!(persist(&mut state, &first_turn, &first), first_ref);
    finish(&mut state, &first_turn, &first_ref).unwrap();
    let original_bytes = std::fs::read(state.user.history_path.as_ref().unwrap()).unwrap();
    assert!(!String::from_utf8_lossy(&original_bytes).contains("<script>"));

    let second_turn = next_turn(&mut state);
    let second = create(
        &mut state,
        &second_turn,
        Some(first_ref.clone()),
        "Add example",
    );
    let second_ref = persist(&mut state, &second_turn, &second);
    assert_eq!(second_ref.revision, 2);
    assert_eq!(first_ref.presentation_id, second_ref.presentation_id);
    finish(&mut state, &second_turn, &second_ref).unwrap();

    // Reconstruct AppState and reload the actual persisted history and content.
    let mut reopened = state_named("rp2-reopen");
    reopened.user.history_path = state.user.history_path.clone();
    reopened.user.agent_history = load_agent_history(&reopened.user.history_path).unwrap();
    assert_eq!(
        reopened.private_context().read_presentation(&first_turn.session_id, &first_ref)
            .unwrap()
            .content,
        first.content
    );
    let version = reopened.private_context().read_presentation(&first_turn.session_id, &second_ref)
        .unwrap();
    assert_eq!(version.based_on, Some(first_ref.clone()));
    assert_eq!(version.content, second.content);
    assert_eq!(
        reopened.private_context().read_presentation_candidate(&second_turn.session_id, &second.candidate_id)
            .unwrap(),
        second
    );
    let view = session_view(&reopened.user.agent_history.sessions[0], &reopened.workspace.book);
    assert_eq!(
        view.turns[0]
            .outcome
            .as_ref()
            .unwrap()
            .answer_view
            .as_ref()
            .unwrap()
            .parts,
        outcome(&first_ref).answer_view.unwrap().parts
    );
    assert_eq!(
        view.turns[1]
            .outcome
            .as_ref()
            .unwrap()
            .answer_view
            .as_ref()
            .unwrap()
            .parts,
        outcome(&second_ref).answer_view.unwrap().parts
    );
    // Supported old-version editing: new revision records revision 1 as its base.
    let third_turn = next_turn(&mut reopened);
    let third = create(
        &mut reopened,
        &third_turn,
        Some(first_ref.clone()),
        "Edit old view",
    );
    let third_ref = persist(&mut reopened, &third_turn, &third);
    assert_eq!(third_ref.revision, 3);
    assert_eq!(
        reopened.private_context().read_presentation(&third_turn.session_id, &third_ref)
            .unwrap()
            .based_on,
        Some(first_ref)
    );
}

#[test]
fn session_management_jsonl_reopen_reads_original_presentation_content() {
    let (_root, mut state, _) = fixture();
    super::session_runtime_tests::enable_jsonl(&mut state);
    assert_eq!(route_agent_new(&mut state, "fresh").status, 200);
    let turn = next_turn(&mut state);
    let candidate = create(&mut state, &turn, None, "JSONL presentation");
    let reference = persist(&mut state, &turn, &candidate);
    finish(&mut state, &turn, &reference).unwrap();
    let (history, store) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    state.user.agent_history = history; state.user.session_store = store;
    assert_eq!(state.private_context().read_presentation(&turn.session_id, &reference).unwrap().content, candidate.content);
}

#[test]
fn presentation_candidate_is_not_a_deliverable_and_missing_revision_cannot_finalize() {
    let (_root, mut state, turn) = fixture();
    let candidate = create(&mut state, &turn, None, "Unpublished");
    let invented = PresentationRef {
        presentation_id: candidate.presentation_id.clone(),
        revision: 1,
    };
    let before = std::fs::read(state.user.history_path.as_ref().unwrap()).unwrap();
    assert_eq!(
        finish(&mut state, &turn, &invented).unwrap_err().error_code,
        "PRESENTATION_NOT_FOUND"
    );
    assert_eq!(
        state.user.agent_history.sessions[0].turns[0].status,
        AgentAssistantStatus::PendingAssistant
    );
    assert_eq!(
        std::fs::read(state.user.history_path.as_ref().unwrap()).unwrap(),
        before
    );
    let reference = persist(&mut state, &turn, &candidate);
    finish(&mut state, &turn, &reference).unwrap();
    assert_eq!(
        state.private_context().persist_presentation_candidate(
                &turn.session_id,
                &turn.turn_id,
                &candidate.candidate_id
            )
            .unwrap_err()
            .error_code,
        "PRESENTATION_TURN_NOT_PENDING"
    );
}

#[test]
fn presentation_failed_saves_preserve_old_version_and_history() {
    let (root, mut state, turn) = fixture();
    let first = create(&mut state, &turn, None, "Original");
    let original = persist(&mut state, &turn, &first);
    finish(&mut state, &turn, &original).unwrap();
    let turn = next_turn(&mut state);
    let edit = create(&mut state, &turn, Some(original.clone()), "Changed");
    let version_dir = root
        .path()
        .join("agent-history.presentations/versions")
        .join(&original.presentation_id);
    // A real filesystem publication failure, without mocking the storage port.
    std::fs::create_dir(version_dir.join("2.json")).unwrap();
    let before = std::fs::read(state.user.history_path.as_ref().unwrap()).unwrap();
    assert!(state.private_context().persist_presentation_candidate(&turn.session_id, &turn.turn_id, &edit.candidate_id)
        .is_err());
    assert_eq!(
        state.private_context().read_presentation(&turn.session_id, &original)
            .unwrap()
            .content,
        first.content
    );
    assert_eq!(
        std::fs::read(state.user.history_path.as_ref().unwrap()).unwrap(),
        before
    );
    std::fs::remove_dir(version_dir.join("2.json")).unwrap();
    let saved = persist(&mut state, &turn, &edit);
    assert_eq!(saved.revision, 2);

    // Content saved, but the history commit fails: pending in memory/on disk, no answer receipt.
    let temporary = agent_history_temporary_path(state.user.history_path.as_ref().unwrap());
    std::fs::create_dir(&temporary).unwrap();
    assert!(finish(&mut state, &turn, &saved).is_err());
    assert_eq!(
        state.user.agent_history.sessions[0].turns[1].status,
        AgentAssistantStatus::PendingAssistant
    );
    assert_eq!(
        std::fs::read(state.user.history_path.as_ref().unwrap()).unwrap(),
        before
    );
    std::fs::remove_dir(temporary).unwrap();
    finish(&mut state, &turn, &saved).unwrap();

    let turn = next_turn(&mut state);
    let candidates = root.path().join("agent-history.presentations/candidates");
    let parked = root.path().join("saved-candidates");
    std::fs::rename(&candidates, &parked).unwrap();
    std::fs::write(&candidates, b"blocked directory").unwrap();
    let draft = content(&state, "Cannot save");
    assert_eq!(
        state.private_context().create_presentation_candidate(
                &turn.session_id,
                &turn.turn_id,
                Some(original.clone()),
                draft
            )
            .unwrap_err()
            .error_code,
        "PRESENTATION_STORAGE_FAILED"
    );
    assert_eq!(
        state.private_context().read_presentation(&turn.session_id, &original)
            .unwrap()
            .content,
        first.content
    );
}

#[test]
fn presentation_ownership_follows_book_session_and_pending_turn() {
    let (_root, mut state, turn) = fixture();
    let candidate = create(&mut state, &turn, None, "Owned");
    let reference = persist(&mut state, &turn, &candidate);
    let next = next_turn(&mut state);
    assert_eq!(
        state.private_context().persist_presentation_candidate(
                &next.session_id,
                &next.turn_id,
                &candidate.candidate_id
            )
            .unwrap_err()
            .error_code,
        "PRESENTATION_OWNER_MISMATCH"
    );
    let other = new_agent_session(&state.workspace.book.base.book_id, "2026-09-16T17:00:00Z", 1);
    let other_id = other.id.clone();
    state.user.agent_history.sessions.push(other);
    assert_eq!(route_agent_history_select(&mut state, &json!({"session_id":other_id}).to_string()).status, 200);
    // Historical, inactive session remains readable within the same book.
    assert!(state.private_context().read_presentation(&turn.session_id, &reference)
        .is_ok());
    assert_eq!(
        state.private_context().read_presentation(&other_id, &reference)
            .unwrap_err()
            .error_code,
        "PRESENTATION_OWNER_MISMATCH"
    );
    assert_eq!(
        state.private_context().read_presentation_candidate(&other_id, &candidate.candidate_id)
            .unwrap_err()
            .error_code,
        "PRESENTATION_OWNER_MISMATCH"
    );
    let other_turn = next_turn(&mut state);
    let draft = content(&state, "Cross-session edit");
    assert_eq!(
        state.private_context().create_presentation_candidate(
                &other_id,
                &other_turn.turn_id,
                Some(reference.clone()),
                draft
            )
            .unwrap_err()
            .error_code,
        "PRESENTATION_OWNER_MISMATCH"
    );
    assert_eq!(
        finish(&mut state, &other_turn, &reference)
            .unwrap_err()
            .error_code,
        "PRESENTATION_OWNER_MISMATCH"
    );
    let mut other_base = sample_base();
    other_base.book_id = "another-book".into();
    state.workspace.book = Book::new(other_base, &"X".repeat(100)).into();
    assert_eq!(
        state.private_context().read_presentation(&turn.session_id, &reference)
            .unwrap_err()
            .error_code,
        "PRESENTATION_OWNER_MISMATCH"
    );
    let other_book_turn = next_turn(&mut state);
    assert_eq!(
        state.private_context().read_presentation(&other_book_turn.session_id, &reference)
            .unwrap_err()
            .error_code,
        "PRESENTATION_OWNER_MISMATCH"
    );
}

#[test]
fn presentation_rejects_invalid_content_and_unavailable_private_storage() {
    let (_root, mut state, turn) = fixture();
    let valid = content(&state, "Valid");
    for change in 0..4 {
        let mut draft = valid.clone();
        match change {
            0 => draft.entrypoint = "missing.html".into(),
            1 => {
                draft
                    .content_files
                    .insert("../outside.html".into(), "bad".into());
            }
            2 => draft.source_bindings[0].book_id = "other-book".into(),
            _ => draft.readable_content.clear(),
        }
        assert_eq!(
            state.private_context().create_presentation_candidate(&turn.session_id, &turn.turn_id, None, draft)
                .unwrap_err()
                .error_code,
            "PRESENTATION_INVALID"
        );
    }
    assert_eq!(
        state.private_context().read_presentation_candidate(&turn.session_id, "../history")
            .unwrap_err()
            .error_code,
        "PRESENTATION_INVALID"
    );
    state.user.history_path = None;
    assert_eq!(
        state.private_context().create_presentation_candidate(&turn.session_id, &turn.turn_id, None, valid)
            .unwrap_err()
            .error_code,
        "PRESENTATION_STORAGE_UNAVAILABLE"
    );
}

#[test]
fn presentation_cancelled_run_keeps_saved_old_content_without_committing_an_answer() {
    let (_root, mut state, turn) = fixture();
    let first = create(&mut state, &turn, None, "Original");
    let original = persist(&mut state, &turn, &first);
    finish(&mut state, &turn, &original).unwrap();
    let turn = next_turn(&mut state);
    let _edit = create(
        &mut state,
        &turn,
        Some(original.clone()),
        "Cancelled candidate",
    );
    finalize_agent_turn(
        &mut state,
        &turn,
        AgentAssistantStatus::Cancelled,
        None,
        Some(AgentTurnError {
            error_code: "AGENT_RUN_CANCELLED".into(),
            category: "cancelled".into(),
            message: "Stopped".into(),
        }),
        None,
        &[],
        "2026-09-16T17:00:00Z",
    )
    .unwrap();
    let history = load_agent_history(&state.user.history_path).unwrap();
    assert!(history.sessions[0].turns[1].outcome.is_none());
    assert_eq!(
        state.private_context().read_presentation(&turn.session_id, &original)
            .unwrap()
            .content,
        first.content
    );
}

#[test]
fn presentation_public_read_observation_and_source_use_delivered_revision() {
    let (_root, mut state, turn) = fixture();
    let candidate = create(&mut state, &turn, None, "Recall");
    let reference = persist(&mut state, &turn, &candidate);
    let request =
        json!({"session_id": turn.session_id, "turn_id": turn.turn_id, "reference": reference});
    assert_ne!(
        post(&mut state, "/agent/presentation.read", &request.to_string()).status,
        200
    );
    finish(&mut state, &turn, &reference).unwrap();
    let response = post(&mut state, "/agent/presentation.read", &request.to_string());
    assert_eq!(response.status, 200);
    let public: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(public["reference"]["revision"], 1);
    assert!(public.get("source_bindings").is_none());
    assert!(public.to_string().find("evidence_range").is_none());
    assert_ne!(public["sources"][0]["label"], "1.1");

    let mut observed = request.clone();
    observed["text"] = json!("Recall is 2/3; an unrelated document changes nothing.");
    observed["source_ref_ids"] = json!(["source-rp2"]);
    assert_eq!(
        post(
            &mut state,
            "/agent/presentation.observe",
            &observed.to_string()
        )
        .status,
        200
    );
    observed["text"] = json!("Probability ratio = 1.1; the chart updates with the slider.");
    assert_eq!(
        post(
            &mut state,
            "/agent/presentation.observe",
            &observed.to_string()
        )
        .status,
        200
    );
    for text in [
        "See LID 1.1",
        "Internal position 1.1",
        "Claim [[source:invented]]",
    ] {
        observed["text"] = json!(text);
        assert_ne!(
            post(
                &mut state,
                "/agent/presentation.observe",
                &observed.to_string()
            )
            .status,
            200,
            "{text}"
        );
    }
    observed["text"] = json!("Recall is 1");
    observed["source_ref_ids"] = json!(["invented"]);
    assert_ne!(
        post(
            &mut state,
            "/agent/presentation.observe",
            &observed.to_string()
        )
        .status,
        200
    );
    let source = json!({"turn_id": turn.turn_id, "source_ref_id": "source-rp2"});
    assert_eq!(
        post(&mut state, "/agent/source.resolve", &source.to_string()).status,
        200
    );
    assert_eq!(
        post(&mut state, "/agent/source.open", &source.to_string()).status,
        200
    );
    let mut wrong = request.clone();
    wrong["turn_id"] = json!("not-this-turn");
    assert_ne!(
        post(&mut state, "/agent/presentation.read", &wrong.to_string()).status,
        200
    );
}

#[test]
fn presentation_semantic_failure_cannot_commit_and_source_id_cannot_change_meaning() {
    let (_root, mut state, turn) = fixture();
    let candidate = create(&mut state, &turn, None, "See LID 1.1");
    let reference = persist(&mut state, &turn, &candidate);
    assert_eq!(
        finish(&mut state, &turn, &reference)
            .unwrap_err()
            .error_code,
        "PRESENTATION_PUBLIC_CONTENT_INVALID"
    );
    let candidate = create(&mut state, &turn, None, "Good content");
    let reference = persist(&mut state, &turn, &candidate);
    let mut answer = outcome(&reference);
    let mut conflicting = candidate.content.source_bindings[0].clone();
    conflicting.preview_snapshot = "Different evidence".into();
    answer.source_bindings.push(conflicting);
    let messages = state.workspace.messages.clone();
    assert!(finalize_agent_turn_completed(
        &mut state,
        &turn,
        &answer,
        &messages,
        "2026-09-17T01:00:00Z"
    )
    .is_err());
    assert_eq!(
        state.user.agent_history.sessions[0].turns[0].status,
        AgentAssistantStatus::PendingAssistant
    );
}

/// Browser acceptance host: real private files, answer compiler and Reader routes.
#[test]
#[ignore = "starts a bounded HTTP fixture for playwright/agent-presentation.spec.ts"]
fn presentation_browser_host() {
    run_presentation_browser_host(false, None);
}

#[test]
#[ignore = "starts a bounded HTTP fixture for playwright/agent-presentation-ex1.spec.ts"]
fn presentation_ex1_browser_host() {
    run_presentation_browser_host(true, None);
}

#[test]
#[ignore = "EX10 saved candidate through real Reader state and follow-up routes"]
fn presentation_ex10_browser_host() {
    run_presentation_browser_host(false, Some(std::env::var("EX10_CONTENT").expect("EX10_CONTENT")));
}

#[test]
#[ignore = "RN2 real persistence browser acceptance host"]
fn rn2_browser_host() {
    run_presentation_browser_host(false, None);
}

fn run_presentation_browser_host(ex1_gold: bool, ex10_view: Option<String>) {
    let (_root, mut state, turn) = fixture();
    state.user.store = MemoryStore::open(_root.path().join("memory.json")).unwrap();
    let mut draft = content(&state, if ex1_gold { "同样的损失，不同的路" } else { "证据召回率" });
    if ex1_gold {
        draft.readable_content = "一维模型 L(w)=(w−2)²，w₀=0，真实迭代满足 eₖ₊₁=(1−2η)eₖ。η=0.2 同侧接近；η=0.8 跨越 2 交替接近；η=1.1 交替远离。η=0.2/0.8 的距离和损失序列相同，位置与带符号误差显示方向差别。过渡实心点是视觉插值，不是新的算法迭代。可播放、暂停、回退、定位、改 η 和先预测后揭示；当前具体数值以保存的现场为准。".into();
        draft.source_bindings.clear();
        draft.assumptions = vec!["仅适用这一维二次模型、固定学习率、无动量和噪声".into()];
        draft.state_contract = json!({
            "eta": "fixed learning rate 0.1..1.1, step 0.05",
            "semantic_state": "completed gradient descent iteration 0..5",
            "transition_progress": "visual interpolation after completed iteration in [0,1)",
            "reveal_state": "whether the current next-step prediction was revealed",
            "prediction": "same, cross or null for current next-step prediction"
        });
        draft.initial_state = json!({"eta":0.8,"semantic_state":0,"transition_progress":0,"reveal_state":false,"prediction":null});
    } else {
        draft.readable_content = "需要三处证据，找到两处时召回率为 2/3；加入无关材料不改变召回率，补齐第三处后为 1。 [[source:source-rp2]]".into();
    }
    draft.content_files.insert(
        "index.html".into(),
        std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(if ex1_gold { "../../docs/performance/ex1-learning-rate.html" } else { "tests/fixtures/presentation-answer.html" }),
        )
        .unwrap(),
    );
    if let Some(path) = ex10_view {
        let view: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        draft.title = view["title"].as_str().unwrap().into();
        draft.content_files = serde_json::from_value(view["content_files"].clone()).unwrap();
        draft.animation_assets = serde_json::from_value(view.get("animation_assets").cloned().unwrap_or_else(||json!({}))).unwrap();
        draft.entrypoint = view["entrypoint"].as_str().unwrap().into();
        draft.readable_content = view["readable_content"].as_str().unwrap().into();
        draft.state_contract = view["state_contract"].clone();
        draft.initial_state = view["initial_state"].clone();
        draft.source_bindings.clear();
        draft.assumptions = vec![];
    }
    let candidate = state.private_context().create_presentation_candidate(&turn.session_id, &turn.turn_id, None, draft)
        .unwrap();
    let reference = persist(&mut state, &turn, &candidate);
    finish(&mut state, &turn, &reference).unwrap();
    let (turn, reference) = if std::env::var_os("RN2_BROWSER").is_some() {
        let next = next_turn(&mut state);
        let mut draft = candidate.content.clone();
        draft.content_files.get_mut("index.html").unwrap().push_str("<img id=\"retained-chart\" src=\"assets/chart.svg\" alt=\"证据图\">");
        draft.content_files.insert("assets/chart.svg".into(), "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"20\"><rect width=\"40\" height=\"20\" fill=\"coral\"/></svg>".into());
        let candidate = state.private_context().create_presentation_candidate(&next.session_id, &next.turn_id, Some(reference), draft).unwrap();
        let reference = persist(&mut state, &next, &candidate);
        finish(&mut state, &next, &reference).unwrap();
        (next, reference)
    } else { (turn, reference) };
    let port = std::env::var("PRESENTATION_TEST_PORT").unwrap_or_else(|_| "4175".into());
    let server = tiny_http::Server::http(format!("127.0.0.1:{port}")).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    state.services.adapter = Box::new(ChatRecordingAdapter {
        seen_messages: seen.clone(),
    });
    let mut fail_state = false;
    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    println!("RP3_BROWSER_READY");
    while std::time::Instant::now() < deadline {
        let Some(mut request) = server.recv_timeout(Duration::from_secs(1)).unwrap() else {
            continue;
        };
        let path = request.url().to_string();
        let reply = if path == "/fixture" {
            ok_json(
                &json!({"session_id": turn.session_id, "turn_id": turn.turn_id, "reference": reference, "outcome": outcome(&reference)}),
            )
        } else if path == "/requests" {
            ok_json(&*seen.lock().unwrap())
        } else if path == "/version-three" {
            let next = next_turn(&mut state);
            let candidate = create(&mut state, &next, Some(reference.clone()), "Version three");
            let latest = persist(&mut state, &next, &candidate);
            finish(&mut state, &next, &latest).unwrap();
            ok_json(&json!({"reference":latest}))
        } else if path == "/reopen" {
            let history_path = state.user.history_path.clone();
            state = state_named("rp6-reopened-browser");
            state.user.history_path = history_path;
            state.user.agent_history = load_agent_history(&state.user.history_path).unwrap();
            state.user.store = MemoryStore::open(_root.path().join("memory.json")).unwrap();
            state.services.adapter = Box::new(ChatRecordingAdapter {
                seen_messages: seen.clone(),
            });
            ok_json(&json!({"ok":true}))
        } else if path == "/reset-tutor" {
            let path = _root.path().join("learning.db");
            if path.exists() { std::fs::remove_file(path).unwrap(); }
            ok_json(&json!({"ok":true}))
        } else if path == "/reset-scene" {
            let directory = _root.path().join("agent-history.presentations/states");
            if directory.exists() {
                std::fs::remove_dir_all(directory).unwrap();
            }
            ok_json(&json!({"ok":true}))
        } else if path == "/fail-next-state" {
            fail_state = true;
            ok_json(&json!({"ok":true}))
        } else if path == "/agent/presentation.state.save" && fail_state {
            fail_state = false;
            err_reply(&ToolError {
                error_code: "PRESENTATION_STORAGE_FAILED".into(),
                category: "internal".into(),
                message: "验收：现场写入失败".into(),
            })
        } else if path == "/stop" {
            request
                .respond(tiny_http::Response::from_string("stopped"))
                .unwrap();
            break;
        } else {
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body).unwrap();
            route(&mut state, Req { method: request.method().as_str(), url: &path, body: &body, now: "browser-test" })
        };
        request
            .respond(
                tiny_http::Response::from_string(reply.body)
                    .with_status_code(reply.status)
                    .with_header(
                        tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                    ),
            )
            .unwrap();
    }
}

fn save_scene(
    state: &mut AppState,
    turn: &AgentTurnRef,
    reference: &PresentationRef,
    count: u32,
) -> PresentationFollowUp {
    let response = post(state, "/agent/presentation.state.save", &json!({
        "session_id":turn.session_id,"turn_id":turn.turn_id,"reference":reference,
        "state":{"values":{"count":count},"visible_step":"compare","observed_result":format!("Found {count} of 3 required pieces"),"source_ref_ids":["source-rp2"]}
    }).to_string());
    assert_eq!(response.status, 200, "{}", response.body);
    serde_json::from_str(&response.body).unwrap()
}

#[test]
fn presentation_rp6_reopens_exact_version_and_requested_snapshot() {
    let (_root, mut state, turn) = fixture();
    let candidate = create(&mut state, &turn, None, "Original");
    let original = persist(&mut state, &turn, &candidate);
    finish(&mut state, &turn, &original).unwrap();
    let saved = save_scene(&mut state, &turn, &original, 1);
    save_scene(&mut state, &turn, &original, 2);
    let next = next_turn(&mut state);
    let candidate = create(&mut state, &next, Some(original.clone()), "New version");
    let new = persist(&mut state, &next, &candidate);
    finish(&mut state, &next, &new).unwrap();
    save_scene(&mut state, &next, &new, 3);
    let mut reopened = state_named("rp6-reopened");
    reopened.user.history_path = state.user.history_path.clone();
    reopened.user.agent_history = load_agent_history(&reopened.user.history_path).unwrap();
    let mut request =
        json!({"session_id":turn.session_id,"turn_id":turn.turn_id,"reference":original});
    let read = |state: &mut AppState, request: &Value| -> Value {
        let reply = post(state, "/agent/presentation.read", &request.to_string());
        assert_eq!(reply.status, 200, "{}", reply.body);
        serde_json::from_str(&reply.body).unwrap()
    };
    let latest = read(&mut reopened, &request);
    assert_eq!(latest["restored_state"]["values"]["count"], 2);
    assert_eq!(latest["reference"], json!(original));
    request["saved_state"] = json!(saved);
    let exact = read(&mut reopened, &request);
    assert_eq!(exact["restored_state"]["values"]["count"], 1);
    assert_eq!(exact["restored_state_revision"], saved.state_revision);
    request["reference"] = json!(new);
    request["turn_id"] = json!(next.turn_id);
    assert_ne!(
        post(
            &mut reopened,
            "/agent/presentation.read",
            &request.to_string()
        )
        .status,
        200
    );
}

#[test]
#[ignore = "requires installed Chromium/Edge"]
fn presentation_rp6_edit_rehearses_inherited_state_and_delivers_new_revision() {
    use crate::agent_run::{BorrowedAppPort, RuntimeStatePort};
    use runtime::{
        presentation_author::AuthorRequest,
        run_context::{CancellationToken, ResidentStatePort},
    };
    let (_root, mut state, first) = fixture();
    let mut draft = content(&state, "Original");
    draft.state_contract = json!({"count":"integer evidence count 0..3"});
    let candidate = state.private_context().create_presentation_candidate(&first.session_id, &first.turn_id, None, draft.clone())
        .unwrap();
    let reference = persist(&mut state, &first, &candidate);
    finish(&mut state, &first, &reference).unwrap();
    state.private_context().save_presentation_state(
            &first.session_id,
            &first.turn_id,
            &reference,
            PresentationState {
                values: json!({"page":{"count":1},"controls":[]}),
                visible_step: None,
                observed_result: "Found one piece".into(),
                source_ref_ids: vec![],
            },
        )
        .unwrap();
    let turn = next_turn(&mut state);
    let updated;
    {
        let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
        let scope = app.with_app(|state| crate::run_scope::RunScope::capture(state, &turn, "test", None, None));
        let mut port = RuntimeStatePort { scope: &scope,
            port: &app,
            turn_ref: &turn,
            previewed: Default::default(),
            animations: Default::default(), plots: Default::default(),
        };
        let cancel = CancellationToken::default();
        let read = port
            .author_presentation(
                AuthorRequest::Read {
                    reference: Some(reference.clone()),
                    candidate_id: None,
                    length: None,
                    file: None,
                    offset: 0,
                },
                &[],
                &[],
                &cancel,
            )
            .unwrap();
        assert_eq!(read.body["title"], "Original");
        let write = port.author_presentation(AuthorRequest::Write {
            new_object: false,
            libraries: vec![],
            based_on:Some(reference.clone()), state_contract:draft.state_contract.clone(), initial_state:json!({"count":2}),
            title:"Added example".into(), readable_content:"Updated example with inherited count".into(), source_ref_ids:vec![], assumptions:vec![],
            asset_refs:vec![],
            html:"<h1>Added example</h1><output id='result'></output><script>document.querySelector('#result').textContent='Found '+window.presentation.initialState.count;window.presentation.registerStateRestorer(()=>{});</script>".into(),
        }, &[], &[], &cancel).unwrap();
        let candidate_id = write.body["candidate_id"].as_str().unwrap().to_string();
        let preview = port
            .author_presentation(
                AuthorRequest::Preview {
                read_selector: None,
                    width: None,
                    viewport: None,
                    candidate_id: candidate_id.clone(),
                    actions: vec![],
                },
                &[],
                &[],
                &cancel,
            )
            .unwrap();
        assert_eq!(
            preview.body["status"], "preview_ready_for_inspection",
            "{}",
            preview.body
        );
        assert!(preview.body["observations"][0]["dom"]["text"]
            .as_str()
            .unwrap()
            .contains("Found 1"));
        updated = port
            .author_presentation(AuthorRequest::Deliver { candidate_id }, &[], &[], &cancel)
            .unwrap()
            .delivered
            .unwrap();
    }
    finish(&mut state, &turn, &updated).unwrap();
    assert_eq!(updated.presentation_id, reference.presentation_id);
    assert_eq!(updated.revision, 2);
    assert_eq!(
        state.private_context().read_presentation(&first.session_id, &reference)
            .unwrap()
            .content,
        draft
    );
    assert_eq!(
        state.private_context().read_presentation(&turn.session_id, &updated)
            .unwrap()
            .based_on,
        Some(reference)
    );
}

#[test]
fn presentation_rp6_author_reads_old_code_and_inherits_only_compatible_frozen_parameters() {
    use crate::agent_run::{BorrowedAppPort, RuntimeStatePort};
    use runtime::{
        presentation_author::AuthorRequest,
        run_context::{CancellationToken, ResidentStatePort},
    };
    let (root, mut state, first) = fixture();
    let mut draft = content(&state, "Original");
    draft.state_contract = json!({"count":"integer evidence count 0..3", "unit":"seconds", "mode":"display mode", "shape":"shape"});
    draft
        .content_files
        .insert("details.txt".into(), "参数说明".repeat(1200));
    let original = state.private_context().create_presentation_candidate(&first.session_id, &first.turn_id, None, draft.clone())
        .unwrap();
    let reference = persist(&mut state, &first, &original);
    finish(&mut state, &first, &reference).unwrap();
    let scene = |count| PresentationState {
        values: json!({"page":{"count":count,"unit":9,"mode":7,"shape":{"old":true}},"controls":[]}),
        visible_step: Some("explain".into()),
        observed_result: "Recall experiment".into(),
        source_ref_ids: vec![],
    };
    let receipt = state.private_context().save_presentation_state(&first.session_id, &first.turn_id, &reference, scene(1))
        .unwrap();
    let prepared = prepare_agent_chat(
        &mut state,
        &json!({"message":"Add an example", "presentation_follow_up":receipt}).to_string(),
        "now",
    )
    .unwrap_or_else(|r| panic!("{}", r.body));
    assert!(prepared.agent_message.contains(&reference.presentation_id));
    state.private_context().save_presentation_state(&first.session_id, &first.turn_id, &reference, scene(3))
        .unwrap();
    let turn = prepared.turn_ref;
    let id;
    {
        let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
        let scope = app.with_app(|state| crate::run_scope::RunScope::capture(state, &turn, "test", None, None));
        let mut port = RuntimeStatePort { scope: &scope,
            port: &app,
            turn_ref: &turn,
            previewed: Default::default(),
            animations: Default::default(), plots: Default::default(),
        };
        let read = port
            .author_presentation(
                AuthorRequest::Read {
                    reference: Some(reference.clone()),
                    candidate_id: None,
                    length: None,
                    file: None,
                    offset: 0,
                },
                &[],
                &[],
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(read.body["text"], draft.content_files["index.html"]);
        assert_eq!(
            read.body["total_characters"],
            draft.content_files["index.html"].chars().count()
        );
        assert_eq!(read.body["chunk_characters"], draft.content_files["index.html"].chars().count());
        assert_eq!(read.body["source_ref_ids"], json!(["source-rp2"]));
        assert!(read.body.get("source_bindings").is_none());
        let mut offset = 0;
        let mut restored = String::new();
        loop {
            let chunk = port
                .author_presentation(
                    AuthorRequest::Read {
                        reference: Some(reference.clone()),
                    candidate_id: None,
                    length: None,
                        file: Some("details.txt".into()),
                        offset,
                    },
                    &[],
                    &[],
                    &CancellationToken::default(),
                )
                .unwrap();
            restored.push_str(chunk.body["text"].as_str().unwrap());
            let Some(next) = chunk.body["next_offset"].as_u64() else {
                break;
            };
            offset = next as usize;
        }
        assert_eq!(restored, draft.content_files["details.txt"]);
        let written = port.author_presentation(AuthorRequest::Write {
            new_object: false,
            libraries: vec![],
            based_on:Some(reference.clone()), title:"Added example".into(), html:"<p>New example</p>".into(), readable_content:"New example".into(),
            source_ref_ids:vec!["source-rp2".into()], assumptions:vec![],
            asset_refs:vec![],
            state_contract:json!({"count":"integer evidence count 0..3", "unit":"milliseconds", "mode":"display mode", "shape":"shape"}),
            initial_state:json!({"count":2,"unit":100,"mode":"text","shape":{},"new":5}),
        }, &[], &[], &CancellationToken::default()).unwrap();
        assert_eq!(
            written.body["initial_state"],
            json!({"count":1,"unit":100,"mode":"text","shape":{},"new":5})
        );
        let patch_request = json!({"operation":"patch","reference":reference,
            "edits":[{"old_text":"window.count=2","new_text":"window.count=2;/* scoped change */"}],
            "state_contract":{"count":"integer evidence count 0..3", "unit":"milliseconds", "mode":"display mode", "shape":"shape"},
            "initial_state":{"count":2,"unit":100,"mode":"text","shape":{},"new":5}});
        let patched = port.author_presentation(serde_json::from_value(patch_request.clone()).unwrap(), &[], &[], &CancellationToken::default()).unwrap();
        assert_eq!(patched.body["initial_state"], written.body["initial_state"], "patch inherits the frozen receipt, not a later saved scene");
        let mut wrong = patch_request;
        wrong["reference"] = json!({"presentation_id":reference.presentation_id,"revision":reference.revision+1});
        assert!(port.author_presentation(serde_json::from_value(wrong).unwrap(), &[], &[], &CancellationToken::default()).is_err());
        id = written.body["candidate_id"].as_str().unwrap().to_string();
    }
    let candidate = state.private_context().read_presentation_candidate(&turn.session_id, &id)
        .unwrap();
    assert_eq!(candidate.based_on, Some(reference.clone()));
    assert_eq!(candidate.content.source_bindings, draft.source_bindings);
    let updated = persist(&mut state, &turn, &candidate);
    finish(&mut state, &turn, &updated).unwrap();
    assert_eq!(updated.revision, 2);
    assert_eq!(
        state.private_context().read_presentation(&first.session_id, &reference)
            .unwrap()
            .content,
        draft
    );
    let pending = next_turn(&mut state);
    let failed = create(&mut state, &pending, Some(reference.clone()), "Failed edit");
    // Actual version-write failure leaves both committed answers intact.
    let directory = root
        .path()
        .join("agent-history.presentations/versions")
        .join(&reference.presentation_id)
        .join("3.json");
    std::fs::create_dir(&directory).unwrap();
    assert!(state.private_context().persist_presentation_candidate(&pending.session_id, &pending.turn_id, &failed.candidate_id)
        .is_err());
    assert_eq!(
        state.private_context().read_presentation(&first.session_id, &reference)
            .unwrap()
            .content,
        draft
    );
    assert_eq!(
        state.private_context().read_presentation(&turn.session_id, &updated)
            .unwrap()
            .reference,
        updated
    );
}

#[test]
fn presentation_follow_up_freezes_old_version_and_exact_state_in_model_and_history() {
    let (_root, mut state, first) = fixture();
    let candidate = create(&mut state, &first, None, "Original");
    let original = persist(&mut state, &first, &candidate);
    finish(&mut state, &first, &original).unwrap();
    let saved = save_scene(&mut state, &first, &original, 2);
    let newer_state = save_scene(&mut state, &first, &original, 3);
    assert_eq!(newer_state.state_revision, saved.state_revision + 1);
    let second = next_turn(&mut state);
    let edit = create(&mut state, &second, Some(original.clone()), "Changed");
    let updated = persist(&mut state, &second, &edit);
    finish(&mut state, &second, &updated).unwrap();
    let updated_scene = save_scene(&mut state, &second, &updated, 1);
    assert_eq!(updated_scene.state_revision, 1);
    let seen = Arc::new(Mutex::new(Vec::new()));
    state.services.adapter = Box::new(ChatRecordingAdapter {
        seen_messages: seen.clone(),
    });
    let response = post(
        &mut state,
        "/agent/chat",
        &json!({"message":"Explain this result", "presentation_follow_up":saved}).to_string(),
    );
    assert_eq!(response.status, 200, "{}", response.body);
    let requests = seen.lock().unwrap();
    let user = requests[0]
        .iter()
        .rev()
        .find(|message| message.role == runtime::Role::User)
        .unwrap()
        .content
        .as_ref()
        .unwrap();
    assert!(user.contains("Found 2 of 3 required pieces"), "{user}");
    assert!(user.contains(&saved.saved_state_ref));
    assert!(!user.contains(&newer_state.saved_state_ref));
    assert!(!user.contains("<script>"));
    let disk = load_agent_history(&state.user.history_path).unwrap();
    assert_eq!(
        disk.sessions[0]
            .turns
            .last()
            .unwrap()
            .presentation_follow_up
            .as_ref(),
        Some(&saved)
    );
    assert!(disk.sessions[0].messages.iter().any(|message| message
        .content
        .as_ref()
        .is_some_and(|s| s.contains(&saved.saved_state_ref))));
    let mut reopened = state_named("rp5-reopened");
    reopened.user.history_path = state.user.history_path.clone();
    reopened.user.agent_history = disk;
    let history = route(&mut reopened, Req { method: "GET", url: "/agent/history", body: "", now: "now" });
    assert_eq!(history.status, 200, "{}", history.body);
    let history: Value = serde_json::from_str(&history.body).unwrap();
    assert_eq!(history["current"]["turns"].as_array().unwrap().last().unwrap()["presentation_follow_up"], json!(saved));
    assert_eq!(
        reopened.private_context().read_presentation_state(&saved)
            .unwrap()
            .state
            .values,
        json!({"count":2})
    );
}

#[test]
fn presentation_follow_up_rejects_receipt_mixup_before_precommit() {
    let (_root, mut state, first) = fixture();
    let candidate = create(&mut state, &first, None, "Original");
    let original = persist(&mut state, &first, &candidate);
    finish(&mut state, &first, &original).unwrap();
    let saved = save_scene(&mut state, &first, &original, 2);
    let second = next_turn(&mut state);
    let edit = create(&mut state, &second, Some(original.clone()), "Changed");
    let updated = persist(&mut state, &second, &edit);
    finish(&mut state, &second, &updated).unwrap();
    let before = std::fs::read(state.user.history_path.as_ref().unwrap()).unwrap();
    for kind in 0..5 {
        let mut wrong = saved.clone();
        match kind {
            0 => {
                wrong.reference = updated.clone();
                wrong.turn_id = second.turn_id.clone();
            }
            1 => wrong.state_revision += 1,
            2 => wrong.saved_state_ref = "absent".into(),
            3 => wrong.turn_id = second.turn_id.clone(),
            _ => wrong.session_id = "other-session".into(),
        }
        assert!(prepare_agent_chat(
            &mut state,
            &json!({"message":"Explain", "presentation_follow_up":wrong}).to_string(),
            "now"
        )
        .is_err());
        assert_eq!(
            std::fs::read(state.user.history_path.as_ref().unwrap()).unwrap(),
            before
        );
    }
    let other = new_agent_session(&state.workspace.book.base.book_id, "later", 5);
    let other_id = other.id.clone();
    state.user.agent_history.sessions.push(other);
    assert_eq!(route_agent_history_select(&mut state, &json!({"session_id":other_id}).to_string()).status, 200);
    assert!(presentation_api::follow_up_context(&state.private_context(), &saved).is_err());
}

#[test]
fn presentation_state_save_failure_and_invalid_semantics_return_no_receipt() {
    let (root, mut state, turn) = fixture();
    let candidate = create(&mut state, &turn, None, "Original");
    let reference = persist(&mut state, &turn, &candidate);
    let mut request = json!({"session_id":turn.session_id,"turn_id":turn.turn_id,"reference":reference,
        "state":{"values":{"count":2},"visible_step":null,"observed_result":"Found two pieces","source_ref_ids":[]}});
    assert_ne!(
        post(
            &mut state,
            "/agent/presentation.state.save",
            &request.to_string()
        )
        .status,
        200
    );
    finish(&mut state, &turn, &reference).unwrap();
    request["state"]["observed_result"] = json!("Internal position 1.1");
    assert_ne!(
        post(
            &mut state,
            "/agent/presentation.state.save",
            &request.to_string()
        )
        .status,
        200
    );
    request["state"]["observed_result"] = json!("Found two pieces");
    std::fs::write(
        root.path().join("agent-history.presentations/states"),
        b"blocked directory",
    )
    .unwrap();
    let failed = post(
        &mut state,
        "/agent/presentation.state.save",
        &request.to_string(),
    );
    assert_ne!(failed.status, 200);
    let error: Value = serde_json::from_str(&failed.body).unwrap();
    assert_eq!(error["error_code"], "PRESENTATION_STORAGE_FAILED");
    assert!(error.get("saved_state_ref").is_none());
    assert_eq!(state.user.agent_history.sessions[0].turns.len(), 1);
}

#[test]
fn jl5_presentation_delivery_and_frozen_followup_survive_scene_change() {
    let (_root, mut state, _) = fixture();
    super::session_runtime_tests::enable_jsonl(&mut state);
    assert_eq!(route_agent_new(&mut state, "fresh").status, 200);
    let turn = next_turn(&mut state);
    let candidate = create(&mut state, &turn, None, "Original JSONL version");
    assert!(state.user.agent_history.sessions[0].turns[0].domain.effects.is_empty());
    let original = persist(&mut state, &turn, &candidate);
    assert!(state.user.agent_history.sessions[0].turns[0].domain.effects.is_empty());
    finish(&mut state, &turn, &original).unwrap();
    let saved = save_scene(&mut state, &turn, &original, 1);
    let prepared = prepare_agent_chat(&mut state, &json!({"message":"Explain this state","presentation_follow_up":saved}).to_string(), "later")
        .unwrap_or_else(|r| panic!("{}", r.body));
    save_scene(&mut state, &turn, &original, 3);
    state.workspace.generation += 1;
    let second = create(&mut state, &prepared.turn_ref, Some(original.clone()), "Second JSONL version");
    let second_ref = persist(&mut state, &prepared.turn_ref, &second);
    finish(&mut state, &prepared.turn_ref, &second_ref).unwrap();
    let (history, store) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    state.user.agent_history = history; state.user.session_store = store;
    let session = state.user.agent_history.sessions.iter().find(|s| s.id == turn.session_id).unwrap();
    assert_eq!(session.turns[1].presentation_follow_up.as_ref().unwrap(), &saved);
    assert_ne!(session.turns[1].domain.scene.as_ref().unwrap().generation, state.workspace.generation);
    assert!(matches!(&session.turns[0].domain.effects[0].effect, crate::session_event::DeliveredEffect::Presentation { reference } if reference == &original));
    assert_eq!(state.private_context().read_presentation(&turn.session_id, &original).unwrap().content, candidate.content);
    assert_eq!(state.private_context().read_presentation(&turn.session_id, &second_ref).unwrap().content, second.content);
    let bytes = std::fs::read_to_string(&state.user.session_store.as_ref().unwrap().logs[&turn.session_id].path).unwrap();
    assert!(!bytes.contains("<h1>"));
    let recap = json!(crate::session_recap::local(&state, &HashMap::from([("session_id".into(), turn.session_id.clone())]), "recap").unwrap());
    assert_eq!(recap["effects"][0]["effect"]["reference"], json!(original));
    assert!(recap["effects"][0]["unavailable_reason"].is_null());
    assert_eq!(recap["effects"][1]["effect"]["reference"], json!(second_ref));
}
