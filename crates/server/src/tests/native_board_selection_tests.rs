use super::*;
use std::cell::RefCell;

const NOW: &str = "2026-10-27T09:00:00Z";
const BODY: &str = "A whole trip includes its stopped time. Which interval belongs to the whole trip?";

fn range() -> EvidenceRange { EvidenceRange {start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![]} }

fn setup(message:&str, mode:Option<&str>, preferences:bool) -> (tempfile::TempDir,AppState,crate::agent_run::PreparedAgentChat,Value,Value) {
    let source="Average speed is total distance divided by total time. Equal distances need not take equal times. The total time must be positive. ".repeat(3);
    let (root,mut state)=crate::tests::tutor_loop_tests::fixture_with_book(false,Book::new(base_schema::sample_base(),&source),&source);
    if let Some(mode)=mode {
        let view=state.user.learning_store().unwrap().state().unwrap();
        state.user.learning_store().unwrap().mutate(&serde_json::from_value(json!({"operation_id":"native-mode","expected_revision":view.control.revision,
            "action":{"kind":"start","user_intent":"Understand the whole journey","explicit_constraints":[],"material_scope":[{"source_id":state.workspace.book.base.book_id,"scope_refs":[],"role":"primary"}],"default_teaching_intent":mode}})).unwrap(),NOW).unwrap();
    }
    if preferences {
        let stated=prepare_agent_chat(&mut state,r#"{"message":"I prefer to try one useful inference before a complete answer. I also prefer limited diagrams that leave a small judgment for me."}"#,NOW)
            .unwrap_or_else(|error|panic!("{}",error.body));
        let mut outcome=crate::tests::tutor_loop_tests::outcome(None);
        outcome.answer=Some("I will keep those teaching preferences.".into());
        outcome.answer_view.as_mut().unwrap().parts=vec![AgentAnswerPart::Markdown{text:outcome.answer.clone().unwrap()}];
        crate::tests::tutor_loop_tests::finish(&mut state,&stated.turn_ref,&outcome);
        for (key,value) in [("first","Let me try one useful inference before a complete answer"),("second","Use limited diagrams that leave a small judgment for me") ] {
            state.user.store.create_profile_fact(memory::CreateProfileFact {
                scope:memory::ProfileScope::Book{book_id:state.workspace.book.base.book_id.clone()},applicability:memory::Applicability::Any,
                payload:memory::ProfilePayload::ExplanationPreference(memory::PreferenceClaim{key:key.into(),value:value.into()}),
                source:memory::FactSource::UserStated,evidence:vec![memory::EvidenceRef::Turn{session_id:stated.turn_ref.session_id.clone(),turn_id:stated.turn_ref.turn_id.clone()}],
                confidence:None,sensitivity:memory::Sensitivity::Normal,valid_until:None,
            },NOW).unwrap();
        }
    }
    assert_eq!(route_agent_new(&mut state,NOW).status,200);
    let board=crate::board_api::route(&state.user,&state.workspace,"create",&json!({"title":"Native selection"}),NOW).unwrap();
    let head=state.user.agent_history.sessions.iter().find(|session|Some(session.id.as_str())==state.workspace.selected_chat.as_deref())
        .and_then(|session|session.turns.last()).map(|turn|&turn.turn_id);
    let input=json!({"message":message,"client_request_id":"native-selection-setup",
        "session_id":state.workspace.selected_chat,"expected_chat_head":head,
        "board_follow_up":{"board_id":board["board_id"],"board_revision":board["revision"]}});
    let run=prepare_agent_chat(&mut state,&input.to_string(),NOW).unwrap_or_else(|error|panic!("{}",error.body));
    let accepted=step(&state.private_context(),&run.turn_ref,crate::tests::teaching_design_tests::design_request("native-design",None),&[],&[range()]).unwrap();
    let request=attach_candidate(&mut state,&run,&board["revision"],&accepted["design_ref"]);
    (root,state,run,accepted,request)
}

fn attach_candidate(state:&mut AppState,run:&crate::agent_run::PreparedAgentChat,board_revision:&Value,design_ref:&Value) -> Value {
    {
        let app=crate::agent_run::BorrowedAppPort(RefCell::new(&mut *state));
        let port=crate::agent_run::RuntimeStatePort {port:&app,turn_ref:&run.turn_ref,scope:&run.scope,
            previewed:Default::default(),animations:Default::default(),plots:Default::default()};
        port.author_board_candidate(serde_json::from_value(json!({"operation":"prepare","base_board_revision":board_revision,
            "blocks":[{"block_id":"canvas-only-state","markdown":BODY}],"drawings":[]})).unwrap(),&[],&[],&runtime::run_context::CancellationToken::default()).unwrap();
    }
    let mut candidate=state.user.agent_history.sessions.iter().find(|session|session.id==run.turn_ref.session_id).unwrap().turns.iter()
        .find(|turn|turn.turn_id==run.turn_ref.turn_id).unwrap().domain.board_authoring.clone().unwrap();
    candidate.previewed=true; // Actual image inspection is a runtime gate; this fixture exercises the host's authoritative metadata.
    crate::session_runtime::update_board_authoring(&mut state.user,&run.turn_ref,NOW,candidate.clone()).unwrap();
    json!({"operation":"select","native_board_selection":{"candidate_id":candidate.candidate_id,"candidate_revision":candidate.candidate_revision},
        "move":{"design_ref":design_ref,"delivery_text":candidate.delivery_text}})
}

#[test]
fn native_board_selection_preflight_binds_real_candidate_design_focus_and_typed_move() {
    let (_root,state,run,accepted,mut request)=setup("Explain the complete graph directly.",None,false);
    request["move"]["interaction_intent"]=json!("direct_explanation");
    let normalized=preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[range()]).unwrap();
    assert!(normalized.get("native_board_selection").is_none());
    let movement:TeachingMove=serde_json::from_value(normalized["move"].clone()).unwrap();
    assert_eq!(movement.design_ref.as_deref(),accepted["design_ref"].as_str());
    assert_eq!(movement.block_refs,vec!["total-time"]);
    assert!(!movement.block_refs.iter().any(|id|id=="canvas-only-state"));
    let mut expected_target=accepted["design"]["target"].clone();
    let focus=movement.target.as_ref().unwrap().focus_ref.as_ref().expect("accepted relationship identity");
    expected_target["focus_ref"]=json!(focus);
    assert_eq!(json!(movement.target),expected_target);
    assert_eq!(movement.capability,accepted["design"]["target"]["capability"]);
    assert_eq!(movement.object_ids,Vec::<String>::new());
    assert_eq!(movement.prompt,accepted["design"]["target"]["learning_focus"]);
    assert_eq!(movement.interaction_intent,"direct_explanation");
    assert_eq!(movement.intent_origin,"current_request");
    assert_eq!(movement.kind,"explain");
    assert_eq!(movement.delivery_text.as_deref(),Some(BODY));
    assert!(matches!(movement.participation,Some(runtime::tutor::TeachingParticipation::Support{learner_inference:None,..})));
    let mut part=request.clone();
    part["delivery_scope"]=json!({"kind":"board_part","relation":"The stopped interval belongs to the whole journey.","remaining_request":"Relate this interval to the other graph branches."});
    let normalized_part=preflight_step(&state.private_context(),&run.turn_ref,&part,&[],&[range()]).unwrap();
    assert_eq!(normalized_part["move"]["participation"]["choice"],"support");
    assert_eq!(normalized_part["delivery_scope"],part["delivery_scope"]);
    part["finish"]=json!(true);
    assert!(preflight_step(&state.private_context(),&run.turn_ref,&part,&[],&[range()]).is_err());
    let selected=step(&state.private_context(),&run.turn_ref,normalized.clone(),&[],&[range()]).unwrap();
    let saved=event(&state.private_context(),selected["selected"].as_str().unwrap()).unwrap();
    assert_eq!(saved.payload["move"],normalized["move"]);
    assert!(saved.payload["move"].get("candidate_id").is_none());
    assert!(saved.payload["move"].get("board_candidate_revision").is_none());
    assert!(saved.payload.get("native_board_selection").is_none());
    let mut stale=request; stale["native_board_selection"]["candidate_id"]=json!("board:another:1");
    assert!(preflight_step(&state.private_context(),&run.turn_ref,&stale,&[],&[range()]).is_err());
}

#[test]
fn native_board_selection_requires_real_mode_and_preserves_invitation_semantics() {
    let (_root,state,run,_accepted,mut request)=setup("Let me try one inference after limited help.",None,false);
    let missing=preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[range()]).unwrap_err();
    assert!(missing.error.message.contains("interaction_intent"),"{}",missing.error.message);
    request["move"]["interaction_intent"]=json!("guided_inquiry");
    let missing_source=preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[]).unwrap_err();
    assert_eq!(missing_source.missing_source_read,Some(range()));
    request["move"]["response_expectation"]=json!({"public_prompt":"Which interval belongs to the whole trip?"});
    let inconsistent=preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[range()]).unwrap_err();
    assert!(inconsistent.error.message.contains("新") || inconsistent.error.message.contains("invite"),"{}",inconsistent.error.message);
    request["move"]["participation"]=json!({"choice":"invite","reason":"The current request explicitly asks to try an inference.",
        "support":"The diagram supplies the journey boundary.","learner_inference":"Identify the interval counted as elapsed time."});
    let normalized=preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[range()]).unwrap();
    assert_eq!(normalized["move"]["participation"],request["move"]["participation"]);
    assert_eq!(normalized["move"]["response_expectation"]["public_prompt"],request["move"]["response_expectation"]["public_prompt"]);
    assert_eq!(normalized["move"]["response_expectation"]["block_refs"],json!(["total-time"]));
    assert_eq!(normalized["move"]["intent_origin"],"current_request");
    assert_eq!(normalized["move"]["intent_quote"],"Let me try one inference after limited help.");
}

#[test]
fn native_board_selection_uses_default_and_real_preference_without_fabricating_request_provenance() {
    let (_root,state,run,_accepted,mut request)=setup("Continue the graph.",Some("guided_inquiry"),false);
    request["move"]["interaction_intent"]=json!("guided_inquiry");
    let normal=preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[range()]).unwrap();
    assert_eq!(normal["move"]["intent_origin"],"session_default");
    assert!(normal["move"]["intent_quote"].is_null());
    let (_root,state,run,_accepted,mut request)=setup("Explain the complete diagram directly, without a new task.",Some("guided_inquiry"),false);
    request["move"]["interaction_intent"]=json!("direct_explanation");
    let override_request=preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[range()]).unwrap();
    assert_eq!(override_request["move"]["intent_origin"],"current_request");
    assert_eq!(override_request["move"]["intent_quote"],"Explain the complete diagram directly, without a new task.");
    let (_root,state,run,_accepted,mut request)=setup("Show the relationships in this graph.",None,true);
    let preferences=run.tutor.as_ref().unwrap()["confirmed_preferences"].as_array().unwrap();
    assert_eq!(preferences.len(),2);
    for preference in preferences {
        request["move"]["interaction_intent"]=json!("guided_inquiry");
        request["move"]["preference_ref"]=preference["fact_id"].clone();
        let normalized=preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[range()]).unwrap();
        assert_eq!(normalized["move"]["intent_origin"],"confirmed_preference");
        assert_eq!(normalized["move"]["preference_ref"],preference["fact_id"]);
        assert!(normalized["move"]["intent_quote"].is_null());
    }
    request["move"]["preference_ref"]=json!("preference-not-confirmed");
    assert!(preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[range()]).is_err());
}

#[test]
fn native_board_selection_known_action_needs_only_the_candidate_and_binds_no_user_quote() {
    let (_root,state,run,_accepted,request)=setup("Explain the graph.",None,false);
    // The prepare_select API consumes an authoritative frozen TeachingEvent.
    // Public admission forbids combining board_follow_up with another UI entry;
    // exercise this shared host contract without manufacturing that public run.
    let mut frozen=bound(&state.private_context(),&run.turn_ref.turn_id).unwrap().unwrap();
    frozen.payload["current_request"]=json!("");
    frozen.payload["current_action"]=json!({"action":{"origin":"ui_entry","entry_action":"explain_selection"},
        "user_supplement":"","accepted_request":"Explain this selected relationship","answer_ref":null,"answer_text":null,
        "source_excerpt":null,"source_bindings":[],"target":null,"expected_chat_head":null,"presentation_follow_up":null});
    let prepared=prepare_select(&state.private_context(),&run.turn_ref,&frozen,&request,&[],&[range()]).unwrap();
    assert_eq!(prepared.movement.interaction_intent,"direct_explanation");
    assert_eq!(prepared.movement.intent_origin,"current_action");
    assert!(prepared.movement.intent_quote.is_none());
    assert_eq!(prepared.movement.kind,"explain");
    assert_eq!(prepared.movement.delivery_text.as_deref(),Some(BODY));
    assert_eq!(prepared.movement.block_refs,vec!["total-time"]);
}

#[test]
fn native_board_selection_inherits_the_delivered_local_target_before_deriving_metadata() {
    let source="Average speed is total distance divided by total time. Equal distances need not take equal times. The total time must be positive. ".repeat(3);
    let (_root,mut state)=crate::tests::tutor_loop_tests::fixture_with_book(false,Book::new(base_schema::sample_base(),&source),&source);
    let first=prepare_agent_chat(&mut state,r#"{"message":"Connect the distance and all elapsed time in this trip."}"#,NOW).unwrap_or_else(|error|panic!("{}",error.body));
    let mut design_request=crate::tests::teaching_design_tests::design_request("native-two-relationships",None);
    let mut distance=design_request["design"]["blocks"][0].clone();
    distance["block_id"]=json!("distance");distance["relation"]=json!("The numerator counts the whole-trip distance.");
    design_request["design"]["blocks"].as_array_mut().unwrap().push(distance);
    design_request["design"]["focus_block_refs"]=json!(["total-time","distance"]);
    let accepted=step(&state.private_context(),&first.turn_ref,design_request,&[],&[range()]).unwrap();
    let mut full_target=Value::Null;
    let mut prior=Some(first);
    for (index,blocks,label) in [(0,json!(["total-time","distance"]),"Count the stopped interval in this whole trip"),
        (1,json!(["total-time"]),"Inspect only the stopped interval")] {
        let run=prior.take().unwrap_or_else(||prepare_agent_chat(&mut state,r#"{"message":"Help with this stopped interval."}"#,NOW).unwrap_or_else(|error|panic!("{}",error.body)));
        let mut local=crate::tests::teaching_design_tests::select_request(&format!("native-prior-local-{index}"),accepted["design_ref"].as_str().unwrap(),false);
        local["move"]["block_refs"]=blocks;
        local["move"]["target"]["learning_focus"]=json!(label);
        local["move"]["target"]["expected_performance"]=json!("Identify the interval between departure and arrival");
        local["move"]["target"]["capability"]=json!("local-explanation");local["move"]["capability"]=json!("local-explanation");
        let selected=step(&state.private_context(),&run.turn_ref,local,&[],&[range()]).unwrap();
        let saved=event(&state.private_context(),selected["selected"].as_str().unwrap()).unwrap();
        if index==0 { full_target=saved.payload["move"]["target"].clone(); }
        let mut outcome=crate::tests::tutor_loop_tests::outcome(None);
        outcome.answer=selected["delivery_draft"]["text"].as_str().map(str::to_owned);
        outcome.answer_view.as_mut().unwrap().parts=vec![AgentAnswerPart::Markdown{text:outcome.answer.clone().unwrap()}];
        outcome.teaching_delivery_move_refs=serde_json::from_value(selected["delivery_move_refs"].clone()).unwrap();
        crate::tests::tutor_loop_tests::finish(&mut state,&run.turn_ref,&outcome);
        assert_eq!(state.user.learning_store().unwrap().teaching_deliveries(&run.turn_ref.session_id,&run.turn_ref.turn_id).unwrap().len(),1,"actual completed support, not a selected draft");
    }
    let board=crate::board_api::route(&state.user,&state.workspace,"create",&json!({"title":"Retain the whole local scope"}),NOW).unwrap();
    let head=state.user.agent_history.sessions.iter().find(|session|Some(session.id.as_str())==state.workspace.selected_chat.as_deref())
        .and_then(|session|session.turns.last()).map(|turn|&turn.turn_id);
    let input=json!({"message":"Connect both relationships in the graph.",
        "client_request_id":"native-full-focus-follow-up",
        "session_id":state.workspace.selected_chat,"expected_chat_head":head,
        "board_follow_up":{"board_id":board["board_id"],"board_revision":board["revision"]}});
    let run=prepare_agent_chat(&mut state,&input.to_string(),NOW).unwrap_or_else(|error|panic!("{}",error.body));
    let mut request=attach_candidate(&mut state,&run,&board["revision"],&accepted["design_ref"]);
    request["move"]["interaction_intent"]=json!("neutral");
    let normalized=preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[range()]).unwrap();
    assert_eq!(normalized["move"]["target"],full_target,"the latest partial delivery must not hide an earlier same-design delivery covering the complete focus");
    assert_eq!(normalized["move"]["capability"],"local-explanation");
    assert_eq!(normalized["move"]["prompt"],"Count the stopped interval in this whole trip");
    assert_eq!(normalized["move"]["object_ids"],json!([]));
    assert_ne!(normalized["move"]["target"],accepted["design"]["target"]);
}
