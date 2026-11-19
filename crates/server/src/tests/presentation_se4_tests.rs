//! Feedback uses real private files and the local/network request dispatchers.
use super::*;

fn delivered_fixture() -> (tempfile::TempDir, AppState, AgentTurnRef, PresentationRef) {
    let (root, mut state, turn) = fixture();
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    let candidate = create(&mut state, &turn, None, "Feedback example");
    let reference = persist(&mut state, &turn, &candidate);
    finish(&mut state, &turn, &reference).unwrap();
    (root, state, turn, reference)
}

fn target(turn: &AgentTurnRef, reference: &PresentationRef) -> Value {
    json!({"session_id":turn.session_id,"turn_id":turn.turn_id,"reference":reference})
}

fn feedback(state: &mut AppState, operation: &str, input: &Value) -> Value {
    let reply = post(state, &format!("/agent/presentation.feedback.{operation}"), &input.to_string());
    assert_eq!(reply.status, 200, "{operation}: {}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}

fn rejected(state: &mut AppState, operation: &str, input: &Value) -> Value {
    let reply = post(state, &format!("/agent/presentation.feedback.{operation}"), &input.to_string());
    assert_ne!(reply.status, 200, "{operation}: {}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}

#[test]
fn se4_feedback_lifecycle_retries_and_reopen_preserve_one_current_choice() {
    let (_root, mut state, turn, reference) = delivered_fixture();
    let request = target(&turn, &reference);
    let original = state.private_context().read_presentation(&turn.session_id, &reference).unwrap();
    let history = std::fs::read(state.user.history_path.as_ref().unwrap()).unwrap();
    let memories = state.user.store.recall(&RecallQuery::default());
    let empty = feedback(&mut state, "read", &request);
    assert_eq!(empty["feedback_revision"], 0);
    assert!(empty["selection"].is_null());
    assert!(empty["created_at"].is_null());
    let mut set = request.clone(); set["selection"] = json!({"choice":"like"});
    let first = feedback(&mut state, "set", &set);
    assert_eq!(first["feedback_revision"], 1);
    assert_eq!(first["selection"], json!({"choice":"like","reasons":[],"comment":""}));
    assert!(first["saved_state"].is_null());
    let path = state.user.test_presentation_root().unwrap().join("feedback").join(&reference.presentation_id).join("1.json");
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(feedback(&mut state, "set", &set), first);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(std::fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);

    let mut reopened = state_named(&format!("se4-reopen-{}", uuid::Uuid::now_v7()));
    reopened.user.history_path = state.user.history_path.clone();
    reopened.user.agent_history = load_agent_history(&reopened.user.history_path).unwrap();
    assert_eq!(feedback(&mut reopened, "read", &request), first);
    set["selection"] = json!({"choice":"dislike","reasons":["unclear","hard_to_read"],"comment":"这一步没有讲清楚。"});
    let changed = feedback(&mut reopened, "set", &set);
    assert_eq!(changed["feedback_revision"], 2);
    assert_eq!(changed["selection"], set["selection"]);
    assert_eq!(changed["created_at"], first["created_at"]);
    assert_eq!(feedback(&mut reopened, "read", &request), changed);
    let cleared = feedback(&mut reopened, "clear", &request);
    assert_eq!(cleared["feedback_revision"], 3);
    assert!(cleared["selection"].is_null());
    assert_eq!(feedback(&mut reopened, "clear", &request), cleared);
    assert_eq!(feedback(&mut reopened, "read", &request), cleared);
    assert_eq!(feedback(&mut reopened, "set", &set)["feedback_revision"], 4);
    assert_eq!(state.private_context().read_presentation(&turn.session_id, &reference).unwrap(), original);
    assert_eq!(std::fs::read(state.user.history_path.as_ref().unwrap()).unwrap(), history);
    assert_eq!(state.user.store.recall(&RecallQuery::default()), memories);
    assert_eq!(state.user.agent_history.sessions[0].turns.len(), 1);
}

#[test]
fn se4_feedback_exact_snapshot_rejects_late_changed_cleared_and_other_version_receipts() {
    let (_root, mut state, turn, reference) = delivered_fixture();
    let request = target(&turn, &reference);
    let mut set = request.clone(); set["selection"] = json!({"choice":"like"});
    let first = feedback(&mut state, "set", &set);
    let saved = save_scene(&mut state, &turn, &reference, 1);
    let newer = save_scene(&mut state, &turn, &reference, 2);
    let mut attach = request.clone(); attach["feedback_revision"] = first["feedback_revision"].clone(); attach["saved_state"] = json!(saved);
    let attached = feedback(&mut state, "attach_state", &attach);
    assert_eq!(attached["saved_state"], json!(saved));
    assert_eq!(attached["feedback_revision"], first["feedback_revision"]);
    assert_eq!(feedback(&mut state, "attach_state", &attach), attached);
    assert_eq!(feedback(&mut state, "set", &set), attached, "retry preserves the exact scene");
    let mut late = attach.clone(); late["saved_state"] = json!(newer);
    assert_eq!(rejected(&mut state, "attach_state", &late)["error_code"], "PRESENTATION_FEEDBACK_STATE_CONFLICT");
    set["selection"] = json!({"choice":"dislike","comment":"操作不便"});
    let changed = feedback(&mut state, "set", &set);
    assert!(changed["saved_state"].is_null());
    assert_eq!(rejected(&mut state, "attach_state", &attach)["error_code"], "PRESENTATION_FEEDBACK_STALE");
    attach["feedback_revision"] = changed["feedback_revision"].clone();
    for field in ["saved_state_ref", "state_revision", "turn_id", "session_id", "reference"] {
        let mut wrong = attach.clone();
        wrong["saved_state"][field] = match field {
            "state_revision" => json!(99), "reference" => json!({"presentation_id":reference.presentation_id,"revision":2}), _ => json!("missing"),
        };
        rejected(&mut state, "attach_state", &wrong);
    }
    let cleared = feedback(&mut state, "clear", &request);
    assert_eq!(rejected(&mut state, "attach_state", &attach)["error_code"], "PRESENTATION_FEEDBACK_STALE");
    assert_eq!(feedback(&mut state, "read", &request), cleared);
    let reselected = feedback(&mut state, "set", &set);
    assert!(reselected["feedback_revision"].as_u64().unwrap() > changed["feedback_revision"].as_u64().unwrap());
    assert_eq!(rejected(&mut state, "attach_state", &attach)["error_code"], "PRESENTATION_FEEDBACK_STALE");

    let second_turn = next_turn(&mut state);
    let candidate = create(&mut state, &second_turn, Some(reference.clone()), "Second revision");
    let second = persist(&mut state, &second_turn, &candidate);
    finish(&mut state, &second_turn, &second).unwrap();
    let second_request = target(&second_turn, &second);
    assert_eq!(feedback(&mut state, "read", &second_request)["feedback_revision"], 0);
    let mut second_set = second_request.clone(); second_set["selection"] = json!({"choice":"like"});
    let second_feedback = feedback(&mut state, "set", &second_set);
    attach = second_request.clone(); attach["feedback_revision"] = second_feedback["feedback_revision"].clone(); attach["saved_state"] = json!(saved);
    rejected(&mut state, "attach_state", &attach);
    assert_eq!(feedback(&mut state, "read", &request), reselected);
    assert_eq!(feedback(&mut state, "read", &second_request), second_feedback);
}

#[test]
fn se4_feedback_access_requires_current_book_session_and_delivered_turn() {
    let (_root, mut state, turn) = fixture();
    let candidate = create(&mut state, &turn, None, "Not yet delivered");
    let reference = persist(&mut state, &turn, &candidate);
    let mut request = target(&turn, &reference); request["selection"] = json!({"choice":"like"});
    for operation in ["read", "set", "clear"] { rejected(&mut state, operation, &request); }
    finish(&mut state, &turn, &reference).unwrap();
    let first = feedback(&mut state, "set", &request);
    for field in ["session_id", "turn_id", "reference"] {
        let mut wrong = request.clone();
        wrong[field] = if field == "reference" { json!({"presentation_id":reference.presentation_id,"revision":9}) } else { json!("missing") };
        for operation in ["read", "set", "clear"] { rejected(&mut state, operation, &wrong); }
    }
    let mut wrong = request.clone(); wrong["user_id"] = json!("someone-else");
    assert_eq!(rejected(&mut state, "set", &wrong)["error_code"], "PRESENTATION_FEEDBACK_INVALID");
    for selection in [json!({"choice":"unknown"}), json!({"choice":"like","reasons":["unknown"]}), Value::Null] {
        wrong = request.clone(); wrong["selection"] = selection;
        rejected(&mut state, "set", &wrong);
    }
    let method = route(&mut state, Req { method:"GET", url:"/agent/presentation.feedback.set", body:&request.to_string(), now:"now" });
    assert_eq!(method.status, 405);
    route_agent_new(&mut state, "another chat");
    assert_eq!(feedback(&mut state, "read", &target(&turn, &reference)), first, "local historical delivery stays accessible");
    let old_book = state.workspace.book.clone();
    let mut other = sample_base(); other.book_id = "other-book".into();
    state.workspace.book = Book::new(other, &"X".repeat(100)).into();
    rejected(&mut state, "set", &request);
    state.workspace.book = old_book;
    assert_eq!(post(&mut state, "/agent/history/delete", &json!({"session_id":turn.session_id}).to_string()).status, 200);
    for operation in ["read", "set", "clear"] { rejected(&mut state, operation, &request); }
}

fn usage(path: &std::path::Path) -> (u64, usize) {
    let mut result = (0, 0);
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            let nested = usage(&entry.path()); result.0 += nested.0; result.1 += nested.1;
        } else { result.0 += entry.metadata().unwrap().len(); result.1 += 1; }
    }
    result
}

#[test]
fn se4_feedback_storage_failure_and_replacement_quota_keep_committed_choice() {
    let (_root, mut state, turn, reference) = delivered_fixture();
    let request = target(&turn, &reference);
    let mut set = request.clone(); set["selection"] = json!({"choice":"like"});
    let root = state.user.test_presentation_root().unwrap();
    std::fs::write(root.join("feedback"), "blocked directory").unwrap();
    assert_eq!(rejected(&mut state, "set", &set)["error_code"], "PRESENTATION_STORAGE_FAILED");
    std::fs::remove_file(root.join("feedback")).unwrap();
    assert_eq!(feedback(&mut state, "read", &request)["feedback_revision"], 0);
    let first = feedback(&mut state, "set", &set);
    let path = root.join("feedback").join(&reference.presentation_id).join("1.json");
    let initial = std::fs::read(&path).unwrap();
    let (bytes, files) = usage(&root);
    state.user.presentation_limits = Some((bytes + 16, files));
    // Replacing the one record is allowed at the file limit, and charges only its new size.
    set["selection"] = json!({"choice":"dislike"});
    let changed = feedback(&mut state, "set", &set);
    assert_eq!(changed["feedback_revision"], 2);
    assert_eq!(usage(&root).1, files);
    let committed = std::fs::read(&path).unwrap();
    set["selection"]["comment"] = json!("x".repeat(200));
    assert_eq!(rejected(&mut state, "set", &set)["error_code"], "PRESENTATION_STORAGE_LIMIT");
    assert_eq!(std::fs::read(&path).unwrap(), committed);
    assert_eq!(feedback(&mut state, "read", &request), changed);
    // Snapshot failure is independent of the already saved vote.
    state.user.presentation_limits = None;
    let saved = save_scene(&mut state, &turn, &reference, 1);
    let (bytes, files) = usage(&root); state.user.presentation_limits = Some((bytes, files));
    let mut attach = request.clone(); attach["feedback_revision"] = changed["feedback_revision"].clone(); attach["saved_state"] = json!(saved);
    assert_eq!(rejected(&mut state, "attach_state", &attach)["error_code"], "PRESENTATION_STORAGE_LIMIT");
    assert_eq!(feedback(&mut state, "read", &request), changed);
    state.user.presentation_limits = None;
    assert_eq!(feedback(&mut state, "attach_state", &attach)["saved_state"], attach["saved_state"]);
    assert_eq!(first["feedback_revision"], 1); assert_ne!(initial, committed);
}

#[test]
fn se4_feedback_network_users_workspaces_linked_views_and_publication_access() {
    use crate::tests::mu5_tests::Fixture;
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "parent");
    let w = f.action(&f.a, &w, "parent", "chat/new", json!({}));
    let port = f.run(&w, "parent");
    let reference = PresentationRef { presentation_id:"se4-shared-id".into(), revision:1 };
    let turn = AgentTurnRef { session_id:port.scope.chat_session_id.clone(), turn_id:port.scope.turn_id.clone(), user_turn_ordinal:1 };
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let session = handle.lock().unwrap().agent_history.sessions.iter().find(|s| s.id == turn.session_id).unwrap().clone();
    // Two real user stores intentionally share all object IDs, as imported histories can.
    for user_id in ["A", "B"] {
        let handle = f.access.users.lock().unwrap().get(user_id, "now").unwrap();
        let mut user = handle.lock().unwrap();
        if user_id == "B" {
            let user = &mut *user;
            let mut empty = session.clone(); empty.turns.clear();
            user.session_store.as_mut().unwrap().create(&mut user.agent_history, empty).unwrap();
            user.session_store.as_mut().unwrap().accept(&mut user.agent_history, session.clone(), "now").unwrap();
        }
        let mut completed = session.clone();
        completed.turns[0].status = AgentAssistantStatus::Completed;
        completed.turns[0].outcome = Some(outcome(&reference));
        let version = AgentPresentation {
            reference:reference.clone(), candidate_id:"candidate-se4".into(),
            owner:PresentationOwner {book_id:f.x.book_id.clone(),session_id:turn.session_id.clone()},
            created_by_turn_id:turn.turn_id.clone(), based_on:None,
            content:PresentationContent { action_contract: vec![], teaching_binding: None,
                method_refs:None, method_observations:Default::default(), animation_assets:Default::default(),
                title:"A feedback scene".into(),content_files:BTreeMap::from([("index.html".into(),"<p>Scene</p>".into())]),
                entrypoint:"index.html".into(),readable_content:"A scene".into(),source_bindings:vec![],assumptions:vec![],
                state_contract:json!({}),initial_state:json!({}),
            },
        };
        let path = user.test_presentation_root().unwrap().join("versions/se4-shared-id/1.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(&version).unwrap()).unwrap();
        reader_service::tests::host_calls::session_runtime::finish(&mut user, &turn, &completed, "finished").unwrap();
    }
    drop(port);
    let request = target(&turn, &reference);
    let mut set = request.clone(); set["selection"] = json!({"choice":"like"});
    let a = f.ok(&f.a, "POST", "/api/agent/presentation.feedback.set", set.clone());
    assert_eq!(f.ok(&f.b,"POST","/api/me/presentation.feedback.read",request.clone())["feedback_revision"],0);
    set["selection"] = json!({"choice":"dislike","reasons":["hard_to_operate"]});
    let b = f.ok(&f.b,"POST","/api/me/presentation.feedback.set",set.clone());
    assert_eq!(b["feedback_revision"],1);
    assert_eq!(f.ok(&f.a,"POST","/api/me/presentation.feedback.read",request.clone()),a);
    let original_scene = f.get(&f.a, &w);
    let w = f.action(&f.a, &w,"parent","presentation/feedback/read",request.clone());
    assert_eq!(w["result"],a);
    assert_eq!(w["revision"],original_scene["revision"]);
    assert_eq!(w["reader"],original_scene["reader"]);
    let linked = f.action(&f.a,&w,"parent","linked/attach",request.clone());
    let attachment = linked["attachment_id"].as_str().unwrap();
    let linked_vote = f.action(&f.a,&w,attachment,"presentation/feedback/set",set.clone());
    assert_eq!(linked_vote["result"]["feedback_revision"],2);
    let mut save = request.clone(); save["state"] = json!({"values":{"count":1},"visible_step":"first","observed_result":"One","source_ref_ids":[]});
    let w = f.action(&f.a,&w,attachment,"presentation/save",save);
    let mut attach = request.clone(); attach["saved_state"] = w["presentation"].clone(); attach["feedback_revision"] = json!(2);
    let with_scene = f.action(&f.a,&w,attachment,"presentation/feedback/attach_state",attach.clone());
    assert_eq!(with_scene["result"]["saved_state"],w["presentation"]);
    let cleared = f.action(&f.a,&w,attachment,"presentation/feedback/clear",request.clone());
    assert_eq!(cleared["result"]["feedback_revision"],3);
    assert_eq!(f.call(&f.a,"POST","/api/me/presentation.feedback.attach_state",attach).0,409);
    let wrong = f.create(&f.a,&f.y,"wrong-book");
    let mut input = Fixture::stamp(&wrong,"wrong-book"); input.as_object_mut().unwrap().extend(set.as_object().unwrap().clone());
    assert_ne!(f.call(&f.a,"POST",&format!("/api/workspaces/{}/presentation/feedback/set",wrong["workspace_id"].as_str().unwrap()),input).0,200);
    assert_eq!(f.call(&f.a,"GET","/api/me/presentation.feedback.set",set.clone()).0,405);

    let Fixture {root,access,control,x,y,a,b:token_b} = f;
    drop(handle); drop(access); drop(control);
    let users = crate::user_registry::UserRegistry::open(root.path()).unwrap();
    let control = crate::control_store::ControlStore::open(users.writer()).unwrap();
    let f = Fixture {root,access:Arc::new(crate::authorization::Authorization::new(users).unwrap()),control,x,y,a,b:token_b};
    assert_eq!(f.ok(&f.a,"POST","/api/me/presentation.feedback.read",request.clone()),cleared["result"]);
    assert_eq!(f.ok(&f.b,"POST","/api/me/presentation.feedback.read",request.clone()),b);
    assert_eq!(f.ok(&f.a,"POST","/api/me/presentation.feedback.clear",request.clone()),cleared["result"]);
    f.access.library.lock().unwrap().revoke("A",&f.x).unwrap();
    for operation in ["read","set","clear"] {
        assert_eq!(f.call(&f.a,"POST",&format!("/api/me/presentation.feedback.{operation}"),set.clone()).0,404);
    }
    assert_eq!(f.ok(&f.b,"POST","/api/me/presentation.feedback.read",request.clone()),b);
    assert_eq!(f.call(&f.b,"DELETE",&format!("/api/me/chats/{}",turn.session_id),json!({})).0,200);
    assert_eq!(f.call(&f.b,"POST","/api/me/presentation.feedback.read",request).0,404);
}
