use super::*;

#[test]
fn tutor_source_api_restores_each_book_inside_and_outside_learning_workspaces() {
    let f = Fixture::new();
    let (workspace, _) = learning_scene(&f, "联合学习", vec![]);
    for scope in [None, Some(workspace.as_str())] {
        let query = scope.map(|id| format!("learning_workspace_id={id}&")).unwrap_or_default();
        for (key, source, intent) in [("a", &f.x.book_id, "理解计算图"), ("b", &f.y.book_id, "理解资源核算")] {
            let path = format!("/api/tutor/state?{query}source_id={source}");
            let before = f.ok(&f.a, "GET", &path, Value::Null);
            assert!(before["control"]["current_tutor_session_id"].is_null());
            let state = f.ok(&f.a, "POST", &format!("/api/tutor/mutate?{query}source_id={source}"), json!({
                "operation_id":key, "expected_revision":before["control"]["revision"],
                "action":{"kind":"start", "user_intent":intent, "explicit_constraints":[],
                    "material_scope":[{"source_id":source,"scope_refs":[],"role":"primary"}], "default_teaching_intent":null}
            }));
            assert_eq!(state["control"]["current_tutor_session_id"], format!("tutor-{key}"));
        }
        for (key, source, intent) in [("a", &f.x.book_id, "理解计算图"), ("b", &f.y.book_id, "理解资源核算")] {
            let restored = f.ok(&f.a, "GET", &format!("/api/tutor/state?{query}source_id={source}"), Value::Null);
            let id = format!("tutor-{key}");
            assert_eq!(restored["control"]["current_tutor_session_id"], id);
            assert_eq!(restored["sessions"][&id]["user_intent"], intent);
            assert_eq!(restored["sessions"].as_object().unwrap().len(), 2);
        }
        let empty = f.ok(&f.a, "GET", &format!("/api/tutor/state?{query}source_id=unread-book"), Value::Null);
        assert!(empty["control"]["current_tutor_session_id"].is_null());
    }
}

#[derive(Clone)]
struct GoalProbe {
    calls: Arc<AtomicUsize>,
    requests: Arc<Mutex<Vec<String>>>,
    update: Option<Value>,
}
impl ModelAdapter for GoalProbe {
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        super::super::tutor_loop_tests::fixture_coach_guidance(&request)
            .ok_or_else(||AdapterError {spend_stop:None,message:"Only coach guidance is expected".into()})
    }
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        Err(AdapterError { spend_stop:None, message:"Only the resident request is expected".into() })
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let count = self.calls.fetch_add(1, Ordering::SeqCst);
        self.requests.lock().unwrap().push(request.ordered_messages().iter()
            .filter_map(|message|message.content.as_deref()).collect::<Vec<_>>().join("\n"));
        if count == 0 {
            if let Some(update) = &self.update {
                return Ok(AssistantTurn { provider_continuation:None, text:None,
                    tool_calls:vec![runtime::ToolCall {id:"goal-change".into(),name:"workspace.goal.update".into(),arguments:update.to_string()}],usage_total_tokens:Some(1) });
            }
        }
        Ok(AssistantTurn {provider_continuation:None,text:Some("已按这次请求处理。".into()),tool_calls:vec![],usage_total_tokens:Some(1)})
    }
}

fn learning_scene(f: &Fixture, text: &str, constraints: Vec<&str>) -> (String, Value) {
    let workspace = f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":text,"constraints":constraints},"members":[f.x,f.y]}));
    let id = workspace["learning_workspace_id"].as_str().unwrap().to_owned();
    let scene = f.ok(&f.a,"POST","/api/workspaces",json!({"published_book_ref":f.x,"attachment_id":"page","learning_workspace_id":id}));
    (id, f.action(&f.a,&scene,"page","chat/new",json!({})))
}
fn send(f: &Fixture, scene: &Value, key: &str, message: &str) -> Value {
    let mut input = Fixture::stamp(scene,"page");
    input["session_id"] = scene["selected_chat"].clone();
    input["client_request_id"] = json!(key);
    input["message"] = json!(message);
    let (status, result) = admit(f,scene,&input);
    assert_eq!(status,202,"{result}");
    result
}

#[test]
fn w4_goal_replays_chain_rule_exam_temporary_question_and_explicit_redirection() {
    for (case, goal, constraints, message, revision) in [
        ("chain-rule", "学习深度学习，能够实现基本模型", vec![], "请补充链式法则，再回到反向传播。", 1),
        ("exam", "机器学习期末考试冲刺", vec!["考试只剩三天", "以给定考纲为准"], "先安排薄弱点的复习，深度以应试够用为准。", 1),
        ("temporary", "学习深度学习", vec![], "临时插问：这段历史背景是什么？工作区目标不变。", 1),
        ("redirect", "学习深度学习", vec![], "把工作区目标改为机器学习期末考试冲刺，考试只剩三天。", 2),
    ] {
        let f = Fixture::new();
        let (id, scene) = learning_scene(&f,goal,constraints.clone());
        let probe = GoalProbe {calls:Default::default(),requests:Default::default(),
            update:(revision == 2).then(||json!({"basis_quote":message,"expected_revision":1,
                "goal":{"text":"机器学习期末考试冲刺","constraints":["考试只剩三天"]}}))};
        let provider = probe.clone();
        f.access.runs.configure_fake(move ||Box::new(provider.clone()));
        let admitted = send(&f,&scene,case,message);
        // The result is bound to the admitted workspace even after the Reader moves.
        let other = f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"阅读文学"},"members":[f.x]}));
        let switched = f.action(&f.a,&scene,"page","book/open",json!({"published_book_ref":f.x,"learning_workspace_id":other["learning_workspace_id"]}));
        assert!(f.access.runs.run_one(&f.access).unwrap());
        assert_eq!(f.get(&f.a,&switched)["learning_workspace_id"],other["learning_workspace_id"]);
        let actual = f.ok(&f.a,"GET",&format!("/api/learning-workspaces/{id}"),Value::Null);
        assert_eq!(actual["goal"]["revision"],revision,"{case}: {actual}");
        assert_eq!(actual["goal_history"].as_array().unwrap().len(),revision as usize);
        let requests = probe.requests.lock().unwrap();
        assert!(requests[0].contains("workspace_goal.v1"),"{case}");
        assert!(requests[0].contains(goal),"{case}");
        assert!(requests[0].contains(message),"{case}");
        for constraint in constraints { assert!(requests[0].contains(constraint),"{case}"); }
        if revision == 2 {
            assert_eq!(actual["goal"]["text"],"机器学习期末考试冲刺");
            assert_eq!(actual["goal"]["constraints"],json!(["考试只剩三天"]));
            assert!(requests.last().unwrap().contains("\"run_goal_revision\":1"));
            assert!(requests.last().unwrap().contains("\"revision\":2"));
        } else { assert_eq!(actual["goal"]["text"],goal); }
        let account = context(&f,&f.a);
        let user = account.user.lock().unwrap();
        let session = user.agent_history.sessions.iter().find(|session|session.id == scene["selected_chat"]).unwrap();
        let turn = session.turns.iter().find(|turn|turn.turn_id == admitted["turn_id"]).unwrap();
        assert_eq!(turn.status,AgentAssistantStatus::Completed,"{case}: {:?}",turn.error);
        let frozen = &user.session_store.as_ref().unwrap().logs[&session.id].projection.inputs[&turn.turn_id];
        assert_eq!(frozen.learning_workspace_goal.as_ref().unwrap().revision,1);
        assert_eq!(frozen.learning_workspace_id.as_deref(),Some(id.as_str()));
        assert_eq!(turn.domain.workspace_goal_update.as_ref().map(|goal|goal.revision), (revision == 2).then_some(2));
        assert_eq!(turn.goal_ref.as_ref().unwrap().id,session.goals.first().unwrap().id);
        assert_eq!(session.goals.first().unwrap().origin_turn_id,turn.turn_id);
        assert!(!user.learning_store_for_workspace(Some(&id)).unwrap().state().unwrap().control.enabled);
        drop(user);
        drop(account);
        drop(requests);
        if revision == 2 {
            let f = reopen(f,&Probe::default());
            let handle = context(&f,&f.a);
            let user = handle.user.lock().unwrap();
            let session = user.agent_history.sessions.iter().find(|session|session.id == scene["selected_chat"]).unwrap();
            let turn = session.turns.iter().find(|turn|turn.turn_id == admitted["turn_id"]).unwrap();
            assert_eq!(turn.domain.workspace_goal_update.as_ref().unwrap().revision,2);
            assert_eq!(turn.domain.scene.as_ref().unwrap().workspace_goal.as_ref().unwrap().revision,1);
        }
    }
}

#[test]
fn w4_goal_edit_stops_old_continuation_and_next_question_uses_new_goal_without_a_stage() {
    let f = Fixture::new();
    let (id, scene) = learning_scene(&f,"学习深度学习",vec![]);
    configure(&f,&Probe::default());
    let path = format!("/api/tutor/mutate?learning_workspace_id={id}");
    let stage = f.ok(&f.a,"POST",&path,json!({"operation_id":"old-stage","expected_revision":0,"action":{"kind":"start","user_intent":"补链式法则","explicit_constraints":[],"material_scope":[{"source_id":f.x.book_id,"scope_refs":[],"role":"primary"}],"default_teaching_intent":null}}));
    let old_id = stage["control"]["current_tutor_session_id"].as_str().unwrap();
    assert_eq!(stage["sessions"][old_id]["workspace_goal_revision"],1);
    let first = send(&f,&scene,"before-change","继续解释链式法则");
    f.ok(&f.a,"POST",&format!("/api/learning-workspaces/{id}/goal"),json!({"goal":{"text":"机器学习期末考试冲刺","constraints":["考试只剩三天"]}}));
    let stopped = f.ok(&f.a,"GET",&format!("/api/tutor/state?learning_workspace_id={id}"),json!({}));
    assert!(stopped["control"]["current_tutor_session_id"].is_null());
    assert_eq!(stopped["sessions"][old_id]["status"],"paused");
    let (status, error) = f.call(&f.a,"POST",&path,json!({"operation_id":"resume-old","expected_revision":stopped["control"]["revision"],"action":{"kind":"resume","session_id":old_id}}));
    assert_eq!(status,409,"{error}");
    assert_eq!(error["error_code"],"TUTOR_WORKSPACE_GOAL_CHANGED");
    assert!(f.access.runs.run_one(&f.access).unwrap());
    let current_scene = f.get(&f.a,&scene);
    let next = send(&f,&current_scene,"after-change","先复习考试中的薄弱点");
    let user = context(&f,&f.a);
    let user = user.user.lock().unwrap();
    let session = user.agent_history.sessions.iter().find(|session|session.id == scene["selected_chat"]).unwrap();
    let original = session.turns.iter().find(|turn|turn.turn_id == first["turn_id"]).unwrap();
    let current = session.turns.iter().find(|turn|turn.turn_id == next["turn_id"]).unwrap();
    let inputs = &user.session_store.as_ref().unwrap().logs[&session.id].projection.inputs;
    assert_eq!(inputs[&original.turn_id].learning_workspace_goal.as_ref().unwrap().revision,1);
    assert_eq!(inputs[&current.turn_id].learning_workspace_goal.as_ref().unwrap().revision,2);
    let learning = user.learning_store_for_workspace(Some(&id)).unwrap();
    let publication = f.access.library.lock().unwrap().load("A",&f.x).unwrap();
    assert!(!teaching::turn_active(&crate::PrivateBookContext {user:&user,learning_workspace_id:Some(&id),book:&publication.book,
        book_dir:publication.directory(),messages:&[],selected_chat:None},&original.turn_id).unwrap());
    let state = learning.state().unwrap();
    assert!(state.control.current_tutor_session_id.is_none());
    let bound = learning.teaching_event(&format!("binding:{}",current.turn_id)).unwrap();
    assert_eq!(bound.payload["workspace_goal_revision"],2);
    assert_eq!(bound.binding.turn_id,current.turn_id);
    let continuity=&bound.payload["learner_context"]["learning_opportunity"]["continuity"];
    assert_eq!(continuity["scope"]["workspace_goal_revision"],2);
    assert_eq!(continuity["scope"]["tutor_session_id"],bound.binding.tutor_session_id);
    assert_eq!(bound.payload["status"],"observing");
    assert!(continuity["previous_work"].as_array().unwrap().iter()
        .any(|row|row["binding"]["turn_id"]==original.turn_id),"the new goal retains the actual earlier learning record");
    let original_bound=learning.teaching_event(&format!("binding:{}",original.turn_id)).unwrap();
    assert_eq!(original_bound.payload["workspace_goal_revision"],1,"historical work keeps its accepted goal revision");
    assert!(continuity["scope"]["design_ref"].is_null());
}

#[test]
fn w4_goal_commit_failure_stops_planning_and_next_turn_uses_committed_revision() {
    let f = Fixture::new();
    let (id, scene) = learning_scene(&f,"学习深度学习",vec![]);
    let path = format!("/api/tutor/mutate?learning_workspace_id={id}");
    let old = f.ok(&f.a,"POST",&path,json!({"operation_id":"initial-stage","expected_revision":0,"action":{"kind":"start","user_intent":"补链式法则","explicit_constraints":[],"material_scope":[{"source_id":f.x.book_id,"scope_refs":[],"role":"primary"}],"default_teaching_intent":null}}));
    let old_stage = old["control"]["current_tutor_session_id"].as_str().unwrap().to_owned();
    let message = "把工作区目标改为机器学习期末考试冲刺，考试只剩三天。";
    let probe = GoalProbe {calls:Default::default(),requests:Default::default(),update:Some(json!({"basis_quote":message,"expected_revision":1,
        "goal":{"text":"机器学习期末考试冲刺","constraints":["考试只剩三天"]}}))};
    let adapter = probe.clone();
    f.access.runs.configure_fake(move ||Box::new(adapter.clone()));
    let admitted = send(&f,&scene,"partial-goal-commit",message);
    // This in-process fixture must drive the host-owned bootstrap dependency.
    let handle = f.access.users.lock().unwrap().get_for_recovery("A","1000").unwrap();
    while crate::teaching_coach::tick_network(&f.access,&handle,&runtime::run_context::CancellationToken::default()).unwrap() {}
    drop(handle);
    let learning_path = f.access.users.lock().unwrap().paths("A").unwrap().learning;
    let db = rusqlite::Connection::open(learning_path).unwrap();
    db.execute_batch("CREATE TRIGGER fail_goal_receipt BEFORE INSERT ON tutor_events WHEN json_extract(NEW.request,'$.action.kind')='sync_workspace_goal' AND json_extract(NEW.request,'$.action.revision')=2 BEGIN SELECT RAISE(ABORT,'fixture goal receipt failure'); END;").unwrap();
    assert!(f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(probe.calls.load(Ordering::SeqCst),1,"no model may plan again after the partial commit");
    let result = f.access.runs.status(&context(&f,&f.a),admitted["turn_id"].as_str().unwrap()).unwrap();
    assert_eq!(result["turn"]["status"],"failed","{result}");
    assert_eq!(result["turn"]["error"]["error_code"],"WORKSPACE_GOAL_COMMIT_INCOMPLETE");
    assert_eq!(f.ok(&f.a,"GET",&format!("/api/learning-workspaces/{id}"),Value::Null)["goal"]["revision"],2);
    db.execute_batch("DROP TRIGGER fail_goal_receipt;").unwrap();
    drop(db);
    let f = reopen(f,&Probe::default());
    let restored = f.ok(&f.a,"GET",&format!("/api/tutor/state?learning_workspace_id={id}"),json!({}));
    assert_eq!(restored["workspace_goal_revision"],2);
    assert!(restored["control"]["current_tutor_session_id"].is_null());
    assert_eq!(restored["sessions"][&old_stage]["status"],"paused");
    let scene = f.action(&f.a,&f.get(&f.a,&scene),"page","attach",json!({}));
    let next = send(&f,&scene,"after-partial-commit","按已保存的期末目标继续");
    let account = context(&f,&f.a);
    let user = account.user.lock().unwrap();
    let session = user.agent_history.sessions.iter().find(|session|session.id == scene["selected_chat"]).unwrap();
    let original = session.turns.iter().find(|turn|turn.turn_id == admitted["turn_id"]).unwrap();
    assert!(original.domain.workspace_goal_update.is_none(),"the current goal is not invented as a receipt for the failed turn");
    assert_eq!(original.domain.scene.as_ref().unwrap().workspace_goal.as_ref().unwrap().revision,1);
    let input = &user.session_store.as_ref().unwrap().logs[&session.id].projection.inputs[next["turn_id"].as_str().unwrap()];
    assert_eq!(input.learning_workspace_goal.as_ref().unwrap().revision,2);
    let state = user.learning_store_for_workspace(Some(&id)).unwrap().state().unwrap();
    assert!(state.control.current_tutor_session_id.is_none());
    assert_eq!(state.workspace_goal_revision,Some(2));
}
