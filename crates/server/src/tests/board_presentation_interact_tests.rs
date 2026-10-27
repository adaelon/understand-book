use super::*;
use crate::agent_stream::{RunDescriptor, RunStream};
use runtime::presentation::PresentationRef;

const NOW: &str = "2026-10-25T12:00:00Z";

fn board(state: &crate::AppState, operation: &str, input: Value) -> Value {
    crate::board_api::route(&state.user, &state.workspace, operation, &input, NOW).unwrap()
}

fn retained_fixture() -> (tempfile::TempDir, crate::AppState, crate::AgentTurnRef, BoardPresentationTarget, PresentationRef, Value) {
    let (root, mut state, metadata) = crate::tests::board_browser_host::fixture();
    let created = board(&state, "create", json!({"title":"继续演示"}));
    let document = board(&state, "import", json!({"board_id":created["board_id"],
        "board_revision":created["revision"],"source":metadata["sources"]["presentation"]}));
    let item = &document["items"][0];
    let target = serde_json::from_value(json!({"board_id":document["board_id"],"item_id":item["item_id"],"board_revision":document["revision"]})).unwrap();
    let original = item["target"]["receipt"].clone();
    let reference = serde_json::from_value(metadata["reference"].clone()).unwrap();
    assert_eq!(crate::route_agent_new(&mut state, NOW).status, 200);
    assert_eq!(crate::route_agent_history_delete(&mut state, &json!({"session_id":metadata["session_id"]}).to_string(), NOW).status, 200);
    let run = crate::prepare_agent_chat(&mut state, r#"{"message":"请在这张旧演示上把回程速度调成2。"}"#, NOW).unwrap_or_else(|reply| panic!("{}", reply.body));
    assert_ne!(run.turn_ref.session_id, original["session_id"]);
    (root, state, run.turn_ref, target, reference, original)
}

fn stream(state: &mut crate::AppState, turn: &crate::AgentTurnRef) -> std::sync::Arc<RunStream> {
    let stream = RunStream::new(RunDescriptor {book_id:state.workspace.book.base.book_id.clone(),session_id:turn.session_id.clone(),turn_id:turn.turn_id.clone()});
    state.workspace.active_agent_stream = Some(std::sync::Arc::downgrade(&stream));
    stream
}

fn saved_command(state: &crate::AppState, stream: &RunStream, turn: &crate::AgentTurnRef,
    request: InteractRequest, speed: u32) -> (PresentationCommand, Value, Value) {
    let status = if matches!(&request, InteractRequest::Observe { .. }) { "observed" } else { "applied" };
    let command = prepare(&stream.presentation_commands, &state.private_context(), turn, request).unwrap();
    let target = command.board_target.as_ref().unwrap();
    let claim = json!({"operation":"claim","command_id":command.command_id,"run_turn_id":command.run_turn_id,"page_id":"retained-page"});
    assert_eq!(respond(&state.workspace, &state.private_context(), &claim).unwrap()["claimed"],true);
    let input = json!({"board_id":target.board_id,"item_id":target.item_id,
        "state":{"values":{"page":{"return_speed":speed,"revealed":false}},"visible_step":"explore",
            "observed_result":format!("回程速度：{speed} m/s"),"source_ref_ids":[]},
        "teacher_interaction":{"command_id":command.command_id,"run_turn_id":command.run_turn_id,"page_id":"retained-page","status":status,"support":[]}});
    let receipt = board(state, "presentation.state", input.clone());
    assert_eq!(board(state,"presentation.state",input),receipt,"transport retry must reuse the saved scene");
    let complete = json!({"operation":"complete","command_id":command.command_id,"run_turn_id":command.run_turn_id,
        "page_id":"retained-page","status":status,"receipt":receipt});
    assert!(is_board_command(&state.workspace,&complete));
    respond(&state.workspace,&state.private_context(),&complete).unwrap();
    let result = stream.presentation_commands.pending.lock().unwrap().result.take().unwrap();
    (command,receipt,result)
}

#[test]
fn wb3_retained_interaction_survives_deleted_chat_and_records_current_actor() {
    let (_root, mut state, turn, target, reference, original) = retained_fixture();
    let stream = stream(&mut state,&turn);
    assert!(prepare(&stream.presentation_commands,&state.private_context(),&turn,
        InteractRequest::Observe {reference:reference.clone(),board_target:None}).is_err(),"ordinary interaction keeps chat ownership");
    let (observed,receipt,result) = saved_command(&state,&stream,&turn,
        InteractRequest::Observe {reference:reference.clone(),board_target:Some(target.clone())},1);
    assert_eq!(observed.session_id,turn.session_id);
    assert_eq!(observed.content_session_id,original["session_id"]);
    assert_eq!(receipt["session_id"],original["session_id"]);
    assert_eq!(receipt["turn_id"],original["turn_id"]);
    // A subsequent human save must not replace the exact observed baseline.
    board(&state,"presentation.state",json!({"board_id":target.board_id,"item_id":target.item_id,
        "state":{"values":{"page":{"return_speed":3,"revealed":false}},"visible_step":"explore",
            "observed_result":"回程速度：3 m/s","source_ref_ids":[]}}));
    let (action,saved,result) = saved_command(&state,&stream,&turn,InteractRequest::Act {
        reference:reference.clone(),board_target:Some(target.clone()),based_on:result["saved_state_ref"].as_str().unwrap().into(),
        action:PresentationAction::SetParameters {values:BTreeMap::from([("return_speed".into(),json!(2))])}},2);
    assert_eq!(action.expected_values["return_speed"],1);
    assert_eq!(result["status"],"applied");
    let fact = state.user.learning_store_for_workspace(state.workspace.learning_workspace_id.as_deref()).unwrap()
        .teaching_event(result["teaching"]["action_ref"].as_str().unwrap()).unwrap();
    assert_eq!(fact.binding.chat_session_id,turn.session_id);
    assert_eq!(fact.binding.turn_id,turn.turn_id);
    assert_eq!(fact.payload["actor"],"teacher");
    assert!(fact.learner_expression().is_none());
    assert_eq!(saved["reference"],json!(reference));
    let reopened = board(&state,"presentation.read",json!({"board_id":target.board_id,"item_id":target.item_id}));
    assert_eq!(reopened["restored_state"]["values"]["page"]["return_speed"],2);
    let document = board(&state,"read",json!({"board_id":target.board_id}));
    assert_eq!(document["document"]["items"][0]["target"]["receipt"],original,"imported receipt stays immutable");
}

#[test]
fn wb3_retained_interaction_rejects_wrong_revision_scope_and_removed_target() {
    let (_root, mut state, turn, target, reference, _) = retained_fixture();
    let stream = stream(&mut state,&turn);
    let mut wrong_version = reference.clone(); wrong_version.revision += 1;
    assert!(prepare(&stream.presentation_commands,&state.private_context(),&turn,
        InteractRequest::Observe {reference:wrong_version,board_target:Some(target.clone())}).is_err());
    let mut wrong_board = target.clone(); wrong_board.board_revision = 1;
    assert!(prepare(&stream.presentation_commands,&state.private_context(),&turn,
        InteractRequest::Observe {reference:reference.clone(),board_target:Some(wrong_board)}).is_err());
    let workspace = state.workspace.learning_workspace_id.clone();
    state.workspace.learning_workspace_id = Some("another-private-workspace".into());
    assert!(prepare(&stream.presentation_commands,&state.private_context(),&turn,
        InteractRequest::Observe {reference:reference.clone(),board_target:Some(target.clone())}).is_err());
    state.workspace.learning_workspace_id = workspace;
    let command = prepare(&stream.presentation_commands,&state.private_context(),&turn,
        InteractRequest::Observe {reference,board_target:Some(target.clone())}).unwrap();
    board(&state,"update",json!({"board_id":target.board_id,"board_revision":target.board_revision,"remove_item_ids":[target.item_id]}));
    assert!(respond(&state.workspace,&state.private_context(),&json!({"operation":"claim","command_id":command.command_id,
        "run_turn_id":turn.turn_id,"page_id":"removed-page"})).is_err());
    assert!(board(&state,"read",json!({"board_id":target.board_id}))["document"]["items"].as_array().unwrap().is_empty());
}
