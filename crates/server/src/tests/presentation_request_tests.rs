use super::*;
use runtime::goal::{GoalStatus, GoalVerification};

#[test]
fn presentation_request_reaches_the_goal_and_survives_storage() {
    for logged in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut state = state_named("presentation-request");
        if logged {
            state.user.history_path = Some(dir.path().join("agent-history.json"));
            let paths = session_store::SessionPaths::from_history(state.user.history_path.as_ref().unwrap());
            std::fs::create_dir_all(&paths.root).unwrap();
            let (store, history) = session_store::SessionStore::open(paths).unwrap();
            state.user.session_store = Some(store);
            state.user.agent_history = history;
        }
        assert_eq!(route_agent_new(&mut state, "1").status, 200);
        let prepared = prepare_agent_chat(&mut state,
            &json!({"message":"解释一下这里", "presentation_requested":true}).to_string(), "2")
            .unwrap_or_else(|reply| panic!("{}", reply.body));
        let goal = prepared.goal.unwrap();
        assert!(goal.objective_gap(0, 0).is_some(), "a prose answer must leave an objective gap");
        assert!(goal.objective_gap(1, 0).is_none());
        let restored = if logged { session_store::load_chat_storage(&state.user.history_path).unwrap().0 }
            else { state.user.agent_history.clone() };
        let session = restored.sessions.iter().find(|s| s.id == prepared.scope.chat_session_id).unwrap();
        assert_eq!(session.goals[0], goal);
        assert_eq!(session.turns[0].user, "解释一下这里");
        assert_eq!(session.turns[0].goal_ref.as_ref().unwrap().revision, goal.revision);
    }
}

#[test]
fn presentation_request_adds_delivery_to_an_open_goal_without_losing_content() {
    let mut session = new_agent_session("request", "1", 0);
    append_pending_agent_turn(&mut session, "解释一下这里".into(), None, None, None, None, None, None, None, None, false, "1").unwrap();
    assert_eq!(session.goals[0].requirements[0].verification, GoalVerification::Content);
    let id = session.goals[0].id.clone();
    append_pending_agent_turn(&mut session, "换个方式演示".into(), None, None, None, None, Some(&id), None, None, None, true, "2").unwrap();
    let goal = &session.goals[0];
    assert_eq!(goal.status, GoalStatus::Open);
    assert_eq!(goal.requirements.len(), 2);
    assert_eq!(goal.requirements[0].description, "解释一下这里");
    assert_eq!(goal.requirements[1].verification, GoalVerification::PresentationDelivery);
    assert_eq!(goal.requirements[1].basis_turn_id, session.turns[1].turn_id);
    assert!(goal.objective_gap(0, 0).is_some());
    assert_eq!(session.turns[1].goal_ref.as_ref().unwrap().revision, goal.revision);
}

#[test]
fn presentation_request_is_frozen_for_retries_and_validated_before_acceptance() {
    let plain = json!({"message":"解释一下这里"});
    let page = json!({"message":"解释一下这里", "presentation_requested":true});
    let frozen = crate::run_admission::normalize("workspace", &page).unwrap();
    assert_eq!(frozen["presentation_requested"], true);
    assert_ne!(frozen, crate::run_admission::normalize("workspace", &plain).unwrap());
    let mut state = state_named("presentation-request-invalid");
    let invalid = json!({"message":"解释一下这里", "presentation_requested":"true"});
    assert!(crate::run_admission::normalize("workspace", &invalid).is_err());
    let before = state.user.agent_history.sessions.clone();
    let reply = match prepare_agent_chat(&mut state, &invalid.to_string(), "1") {
        Err(reply) => reply,
        Ok(_) => panic!("invalid output selection was accepted"),
    };
    assert!(reply.body.contains("PRESENTATION_REQUEST_INVALID"));
    assert_eq!(serde_json::to_value(&state.user.agent_history.sessions).unwrap(), serde_json::to_value(before).unwrap());
}

#[test]
fn presentation_request_network_queue_keeps_output_choice_on_retry() {
    use super::mu5_tests::Fixture;
    let mut f = Fixture::new();
    let provider = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", provider.server_addr().to_ip().unwrap());
    super::adm5_tests::seed(&mut f, &url, 1_000_000);
    f.access.runs.configure(runtime::ProviderConfig::from_values("native", "fixture-key", url, "fixture-model").unwrap()).unwrap();
    let workspace = f.create(&f.a, &f.x, "page");
    let workspace = f.action(&f.a, &workspace, "page", "chat/new", json!({}));
    let mut input = Fixture::stamp(&workspace, "page");
    input["session_id"] = workspace["selected_chat"].clone();
    input["client_request_id"] = json!("presentation-request");
    input["message"] = json!("解释一下这里");
    input["presentation_requested"] = json!(true);
    let path = format!("/api/workspaces/{}/agent/runs", workspace["workspace_id"].as_str().unwrap());
    let (status, accepted) = f.call(&f.a, "POST", &path, input.clone());
    assert_eq!(status, 202, "{accepted}");
    let (status, retry) = f.call(&f.a, "POST", &path, input.clone());
    assert_eq!(status, 202, "{retry}");
    assert_eq!(retry["turn_id"], accepted["turn_id"]);
    let handle = f.access.users.lock().unwrap().get("A", "3").unwrap();
    {
        let user = handle.lock().unwrap();
        let session = user.agent_history.sessions.iter().find(|s| s.id == input["session_id"]).unwrap();
        assert_eq!(session.turns.len(), 1);
        assert!(session.goals[0].objective_gap(0, 0).is_some());
        let restored = session_store::load_chat_storage(&user.history_path).unwrap().0;
        assert_eq!(restored.sessions.iter().find(|s| s.id == session.id).unwrap().goals, session.goals);
    }
    input["presentation_requested"] = json!(false);
    assert_eq!(f.call(&f.a, "POST", &path, input).0, 409, "retry identity must retain the output choice");
    assert!(provider.try_recv().unwrap().is_none(), "admission does not start a model request");
}
