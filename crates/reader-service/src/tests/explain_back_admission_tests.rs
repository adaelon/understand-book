use super::*;

pub(crate) fn original_answer(state: &mut AppState) -> AgentTurnRef {
    cited_answer(state, json!({"message":"Why include stopped time?", "question_quote":quote()}))
}

fn cited_answer(state: &mut AppState, input: Value) -> AgentTurnRef {
    let original = prepare(state, input);
    let range = EvidenceRange { start_lid: "1.1".into(), end_lid: "1.2".into(), ranges: vec![] };
    let source = state.workspace.book.resolve_source(&range, "en", None).unwrap();
    let mut answer = super::super::tutor_loop_tests::outcome(None);
    let view = answer.answer_view.as_mut().unwrap();
    view.parts.push(AgentAnswerPart::Sources { source_ref_ids: vec!["src".into()] });
    view.sources.push(runtime::orchestrator::AgentAnswerSource { source_ref_id: "src".into(), label: source.label.clone() });
    answer.source_bindings.push(SourceBinding { source_ref_id: "src".into(), book_id: state.workspace.book.base.book_id.clone(),
        publication_id: None, source_revision: None, evidence_range: Some(range), evidence_text_digest: source.evidence_text_digest,
        media: None, music: None, label_snapshot: source.label, preview_snapshot: source.preview });
    super::super::tutor_loop_tests::finish(state, &original.turn_ref, &answer);
    original.turn_ref
}

#[test]
fn explain_back_overview_does_not_promote_the_ui_instruction_to_a_question() {
    for (purpose, message, expected) in [
        ("discover", "", ""),
        ("understand", "我还没有具体困惑。请先从所选内容中选出一个值得理解的重点，说明它是什么、为什么重要，再带我逐步理解。", ""),
        ("understand", "I do not have a specific question yet. Choose one key idea from the selected content, explain what it is and why it matters, then guide me through it.", ""),
        ("understand", "Why include rest time?", "Why include rest time?"),
    ] {
        let (_root, mut state) = fixture(true);
        let mut input = selection_input(&state, "guide");
        input["entry_action"] = json!("learn_with_purpose");
        input["learning_purpose"] = json!(purpose);
        input["message"] = json!(message);
        let source = cited_answer(&mut state, input);
        let next = validate_agent_input(&mut state.user, &state.workspace, &explanation(&source).to_string()).unwrap_or_else(|r| panic!("{}",r.body));
        assert_eq!(next.interaction.unwrap().accepted_request, expected);
    }
}

pub(crate) fn explanation(source: &AgentTurnRef) -> Value {
    json!({"message":"Stopped time still counts in total time.", "client_request_id":"expression-1", "expression_id":"expression-1",
        "operation_id":"initial", "entry_action":"explain_back", "session_id":source.session_id,
        "expected_chat_head":source.turn_id,"answer_ref":{"turn_id":source.turn_id}})
}

#[test]
fn explain_back_admission_pins_citations_and_retries_same_stored_expression() {
    let (root, mut state) = fixture(false);
    std::fs::remove_file(root.path().join("discourse_index.json")).unwrap();
    let source = original_answer(&mut state);
    let initial = explanation(&source);
    let run = prepare(&mut state, initial.clone());
    let chat = &state.user.agent_history.sessions[0];
    let expression = chat.turns.last().unwrap();
    let action = expression.domain.interaction.as_ref().unwrap();
    assert_eq!(action.accepted_request, "Why include stopped time?");
    assert!(action.answer_text.is_none());
    assert_eq!(action.source_bindings[0].start_lid, "1.1");
    assert_eq!(action.source_bindings[0].end_lid, "1.1");
    assert_eq!(action.user_supplement, expression.user);
    assert_eq!(run.agent_message, expression.user);
    assert_eq!(expression.domain.teach_back_attempts.len(), 1);
    let error = AgentTurnError { error_code:"TEST_GENERATION_FAILED".into(), category:"unavailable".into(), message:"generation failed".into() };
    finalize_agent_turn(&mut state, &run.turn_ref, AgentAssistantStatus::Failed, None, Some(error), None, &[], "1001").unwrap();
    let mut retry = initial.clone();
    retry["operation_id"] = json!("retry-operation");
    retry["retry_of"] = json!(run.turn_ref.turn_id);
    retry["expected_chat_head"] = retry["retry_of"].clone();
    let mut changed = retry.clone();
    changed["message"] = json!("Different explanation");
    assert!(crate::interaction_input::local_repeat(&mut state, &changed.to_string()).is_err());
    let accepted = prepare_agent_chat(&mut state, &retry.to_string(), "1002").unwrap_or_else(|r| panic!("{}", r.body));
    assert_eq!(accepted.turn_ref.turn_id,run.turn_ref.turn_id);
    assert_eq!(accepted.turn_ref.user_turn_ordinal,run.turn_ref.user_turn_ordinal);
    for input in [initial, retry] {
        assert!(crate::interaction_input::local_repeat(&mut state, &input.to_string()).unwrap().is_some());
    }
    let paths = crate::session_store::SessionPaths::from_history(state.user.history_path.as_ref().unwrap());
    let (_, recovered) = crate::session_store::SessionStore::open(paths).unwrap();
    assert_eq!(recovered.sessions[0].turns.len(),2);
    assert_eq!(recovered.sessions[0].turns[1].domain.teach_back_attempts.len(),2);
    let binding = state.user.learning_store().unwrap().teaching_event(&format!("binding:{}",run.turn_ref.turn_id)).unwrap();
    assert_eq!(binding.occurred_at, "1000");
}

#[test]
fn explain_back_admission_rejects_media_cross_book_missing_citations_and_changed_revision() {
    let (root, mut state) = fixture(false);
    let source = original_answer(&mut state);
    let request = explanation(&source);
    for filename in ["video_source_manifest.json", "music_source_manifest.json"] {
        let path = root.path().join(filename);
        std::fs::write(&path, "{}").unwrap();
        assert!(validate_agent_input(&mut state.user, &state.workspace, &request.to_string()).is_err());
        std::fs::remove_file(path).unwrap();
    }
    let original = state.user.agent_history.sessions[0].turns[0].clone();
    for cross_book in [false,true] {
        let turn = &mut state.user.agent_history.sessions[0].turns[0];
        *turn = original.clone();
        if cross_book { turn.source_bindings[0].book_id = "other".into(); }
        else { turn.outcome.as_mut().unwrap().answer_view.as_mut().unwrap().sources.clear(); }
        assert!(!turn_view(&state.workspace.book, turn).can_explain_back);
        assert!(validate_agent_input(&mut state.user, &state.workspace, &request.to_string()).is_err());
    }
    state.user.agent_history.sessions[0].turns[0] = original;
    state.workspace.book = Arc::new(Book::new(state.workspace.book.base.clone(), &format!("{SOURCE}\nNew edition")));
    let err = validate_agent_input(&mut state.user, &state.workspace, &request.to_string()).err().unwrap();
    assert!(err.body.contains("INTERACTION_STALE"));
}
