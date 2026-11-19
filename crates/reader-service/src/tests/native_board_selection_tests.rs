//! Ordinary board requests keep their source and question without selecting a teaching plan.
use super::*;
use std::cell::RefCell;

#[test]
fn native_board_requests_keep_the_original_question_without_a_teaching_plan() {
    for enabled in [false,true] {
        let source="Average speed is total distance divided by total time. ".repeat(4);
        let (_root,mut state)=crate::tests::tutor_loop_tests::fixture_with_book(false,Book::new(base_schema::sample_base(),&source),&source);
        let view=state.user.learning_store().unwrap().state().unwrap();
        state.user.learning_store().unwrap().mutate(&serde_json::from_value(json!({"operation_id":"historical-setting",
            "expected_revision":view.control.revision,"action":{"kind":"set_enabled","enabled":enabled}})).unwrap(),"before-upgrade").unwrap();
        assert_eq!(route_agent_new(&mut state,"now").status,200);
        let board=crate::board_api::route(&state.user,&state.workspace,"create",&json!({"title":"Journey comparison"}),"now").unwrap();
        let message="Explain why this graph includes stopped time, with a concrete example.";
        let input=json!({"message":message,"client_request_id":"board-question","session_id":state.workspace.selected_chat,"expected_chat_head":null,
            "board_follow_up":{"board_id":board["board_id"],"board_revision":board["revision"]}});
        let run=prepare_agent_chat(&mut state,&input.to_string(),"now").unwrap_or_else(|reply|panic!("{}",reply.body));
        let context=run.tutor.as_ref().unwrap();
        assert_eq!(context["status"],"observing");assert_eq!(context["current_request"],message);
        assert!(context["current_design"].is_null());assert!(run.scope.board_context.is_some());
        let design=crate::tests::teaching_design_tests::design_request("obsolete-plan",None);
        assert!(step(&state.private_context(),&run.turn_ref,design,&[],&[]).is_err());
        let app=crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
        let port=crate::agent_run::RuntimeStatePort {port:&app,turn_ref:&run.turn_ref,scope:&run.scope,
            previewed:Default::default(),animations:Default::default(),plots:Default::default()};
        port.author_board_candidate(serde_json::from_value(json!({"operation":"prepare","base_board_revision":board["revision"],
            "blocks":[{"block_id":"elapsed","markdown":"A 10-minute ride plus a 5-minute stop lasts 15 minutes; the distance stays unchanged during the stop."}],"drawings":[]})).unwrap(),
            &[],&[],&runtime::run_context::CancellationToken::default()).unwrap();
        drop(port);drop(app);
        let saved=state.user.agent_history.sessions.iter().flat_map(|session|&session.turns).find(|turn|turn.turn_id==run.turn_ref.turn_id).unwrap();
        assert!(saved.domain.board_authoring.is_some());
        let events=state.user.learning_store().unwrap().teaching_turn_events(context["binding"]["tutor_session_id"].as_str().unwrap(),&run.turn_ref.turn_id).unwrap();
        assert!(!events.iter().any(|event|matches!(event.kind,TeachingFact::DesignRevised|TeachingFact::MoveSelected)));
    }
}
