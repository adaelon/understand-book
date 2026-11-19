use super::*;

fn reader(f: &Fixture, reference: &base_schema::PublishedBookRef, scope: Option<&str>) -> Value {
    let w = f.ok(&f.a,"POST","/api/workspaces",json!({"published_book_ref":reference,
        "learning_workspace_id":scope,"attachment_id":"record-search"}));
    f.action(&f.a,&w,"record-search","chat/new",json!({}))
}

fn say(f: &Fixture, w: &Value, request: &str, message: &str) -> Value {
    let w = f.get(&f.a,w);
    let mut input = Fixture::stamp(&w,"record-search");
    input["session_id"] = w["selected_chat"].clone();
    input["client_request_id"] = json!(request);
    input["message"] = json!(message);
    let (status, result) = admit(f,&w,&input);
    assert_eq!(status,202,"{result:#}");
    f.access.runs.run_one(&f.access).unwrap();
    result
}

#[test]
fn workspace_record_search_finds_native_sessions_across_members_and_older_turns() {
    let f = Fixture::new();
    configure(&f,&Probe::default());
    let scope = f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"理解上下文关系"},"members":[f.x,f.y]}));
    let scope = scope["learning_workspace_id"].as_str().unwrap();
    let first = reader(&f,&f.x,Some(scope));
    say(&f,&first,"record-search-intro","先读例子");
    let older = say(&f,&first,"record-search-old","我原先把状态等同于上下文");
    say(&f,&first,"record-search-later","继续看后一页");
    let second = reader(&f,&f.y,Some(scope));
    let newer = say(&f,&second,"record-search-new","现在我认为状态只是上下文中的一部分");
    let handle = context(&f,&f.a).user;
    let user = handle.lock().unwrap();
    // One explanation is both a direct record and a causal source of another
    // explanation in the same teaching session. Collection must not duplicate it.
    let mut store = user.learning_store_for_workspace(Some(scope)).unwrap();
    let first_bundle = store.chat_record_bundle(first["selected_chat"].as_str().unwrap(),
        &[older["turn_id"].as_str().unwrap().to_owned()]).unwrap();
    store.append_teaching(&memory::teaching::TeachingEvent {
        event_id:"record-search-shared".into(), binding:first_bundle.facts[0].binding.clone(),
        kind:memory::teaching::TeachingFact::MessageDelivered, causal_refs:vec![],
        payload:json!({"answer":"共享出处的状态解释"}),occurred_at:"original-delivery-time".into(),
    }).unwrap();
    store.append_teaching(&memory::teaching::TeachingEvent {
        event_id:"record-search-shared-reuse".into(), binding:first_bundle.facts[0].binding.clone(),
        kind:memory::teaching::TeachingFact::MessageDelivered,causal_refs:vec!["record-search-shared".into()],
        payload:json!({"answer":"继续用共享出处说明状态"}),occurred_at:"later-delivery-time".into(),
    }).unwrap();
    let read = |request| crate::workspace_record_context::read_scoped(&user,Some(scope),&f.x.book_id,request).unwrap();
    let listed = read(json!({"operation":"list","limit":1}));
    assert_eq!(listed["total"],2);
    assert_eq!(listed["records"].as_array().unwrap().len(),1);
    assert!(listed["next_request"].is_object());
    let rest = read(listed["next_request"].clone());
    assert_ne!(listed["records"][0]["chat_session_id"],rest["records"][0]["chat_session_id"]);
    let page = read(json!({"operation":"read","chat_session_id":first["selected_chat"],"limit":1}));
    assert_eq!(page["records"][0]["user"],"先读例子");
    assert!(!page["records"].to_string().contains("我原先把状态"));
    assert_eq!(page["next_request"]["through_seq"],page["selection"]["through_seq"]);
    let found = read(json!({"operation":"search","query":"状态","limit":20}));
    let rows = found["records"].as_array().unwrap();
    let old = rows.iter().find(|row|row["kind"]=="chat_turn" && row["turn_id"]==older["turn_id"]).unwrap();
    let new = rows.iter().find(|row|row["kind"]=="chat_turn" && row["turn_id"]==newer["turn_id"]).unwrap();
    assert_eq!(old["published_book_ref"],json!(f.x));
    assert_eq!(new["published_book_ref"],json!(f.y));
    assert_ne!(old["chat_session_id"],new["chat_session_id"]);
    assert!(old["occurred_at"].is_string() && old["updated_at"].is_string());
    assert!(old["accepted_event_seq"].as_u64().unwrap() < old["event_seq"].as_u64().unwrap());
    let log = user.session_store.as_ref().unwrap().paths.session(first["selected_chat"].as_str().unwrap());
    let events = std::fs::read_to_string(log).unwrap().lines()
        .map(|line|serde_json::from_str::<Value>(line).unwrap()).collect::<Vec<_>>();
    let accepted = events.iter().find(|event|event["kind"]=="turn.accepted" && event["turn_id"]==older["turn_id"]).unwrap();
    assert_eq!(old["occurred_at"],accepted["at"]);
    assert_eq!(old["accepted_event_seq"],accepted["seq"]);
    assert_eq!(old["source_learning_workspace_id"],scope);
    assert_eq!(old["origin"],"native");
    assert_eq!(found["current_mastery"],false);
    let exact = read(old["read_request"].clone());
    assert_eq!(exact["records"],json!([old]));
    let retained = reader_service::tests::host_calls::workspace_record_context::permitted_turn(&user,Some(scope),&f.x.book_id,
        first["selected_chat"].as_str().unwrap(),older["turn_id"].as_str().unwrap()).unwrap();
    assert_eq!(retained.user_authored_text(),"我原先把状态等同于上下文");
    let one = read(json!({"operation":"search","query":"状态","limit":1}));
    assert_eq!(one["records"].as_array().unwrap().len(),1);
    assert!(one["total"].as_u64().unwrap()>=2);
    let next = read(one["next_request"].clone());
    assert_ne!(one["records"][0]["record_id"],next["records"][0]["record_id"]);
    let shared = read(json!({"operation":"search","query":"共享出处","limit":20}));
    assert_eq!(shared["records"].as_array().unwrap().iter()
        .filter(|row|row["record_id"]=="fact:record-search-shared").count(),1);
}

#[test]
fn workspace_record_search_keeps_linked_prefix_and_original_interpretation_chain() {
    let f = Fixture::new();
    configure(&f,&Probe::default());
    let source = reader(&f,&f.x,None);
    say(&f,&source,"record-search-selected","我把速度和变化率混在了一起");
    let chat = source["selected_chat"].as_str().unwrap();
    let (evidence, turn) = reference_learning(&f,chat,"record-search-observed");
    let original = f.ok(&f.a,"GET",&format!("/api/me/learning-records/{chat}"),Value::Null);
    let scope = f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"复习变化率"},"members":[f.x]}));
    let scope = scope["learning_workspace_id"].as_str().unwrap();
    let request = json!({"session_id":chat,"through_seq":original["through_seq"],"goal_revision":1});
    let link = format!("/api/learning-workspaces/{scope}/records");
    f.ok(&f.a,"POST",&link,request.clone());
    f.ok(&f.a,"POST",&link,request);
    let later = say(&f,&source,"record-search-unselected-later","速度之后这条未被选入");
    let unlinked = reader(&f,&f.x,None);
    say(&f,&unlinked,"record-search-unlinked","速度的未授权聊天");
    let other_scope = f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"另一个主题"},"members":[f.x]}));
    let other = reader(&f,&f.x,other_scope["learning_workspace_id"].as_str());
    say(&f,&other,"record-search-other-scope","速度的其他工作区记录");
    let handle = context(&f,&f.a).user;
    let user = handle.lock().unwrap();
    let mut store = user.learning_store().unwrap();
    let corrected = store.correct_evidence(&evidence,"record-search-correction","我问的是瞬时速度").unwrap();
    let read = |request| crate::workspace_record_context::read_scoped(&user,Some(scope),&f.x.book_id,request);
    let found = read(json!({"operation":"search","query":"速度","limit":20})).unwrap();
    let rows = found["records"].as_array().unwrap();
    assert!(!found.to_string().contains("未被选入"));
    assert!(!found.to_string().contains("未授权聊天"));
    assert!(!found.to_string().contains("其他工作区记录"));
    assert_eq!(rows.iter().filter(|row|row["kind"]=="chat_turn").count(),1);
    assert!(rows.iter().any(|row|row["kind"]=="fact" && row["record"]["payload"]["response"]=="我把速度和变化率混在了一起"));
    assert!(rows.iter().any(|row|row["kind"]=="interpretation" && row["record"]["evidence_id"]==evidence
        && row["record"]["interpretation"]=="用户正在比较速度与变化"));
    assert!(rows.iter().any(|row|row["kind"]=="interpretation" && row["record"]["supersedes"]==evidence
        && row["record"]["evidence_id"]==corrected.evidence_id));
    let turn = rows.iter().find(|row|row["kind"]=="chat_turn" && row["turn_id"]==turn).unwrap();
    assert_eq!(turn["origin"],"linked");
    assert_eq!(turn["through_seq"],original["through_seq"]);
    assert!(turn["source_learning_workspace_id"].is_null());
    let ids = rows.iter().map(|row|row["record_id"].as_str().unwrap()).collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(),rows.len(),"same linked source must not be returned twice");
    assert!(read(json!({"operation":"read","chat_session_id":unlinked["selected_chat"]})).is_err());
    assert!(read(json!({"operation":"read","chat_session_id":other["selected_chat"]})).is_err());
    assert!(read(json!({"operation":"read","chat_session_id":chat,"through_seq":999999})).is_err());
    assert!(reader_service::tests::host_calls::workspace_record_context::permitted_turn(&user,Some(scope),&f.x.book_id,
        chat,later["turn_id"].as_str().unwrap()).is_err());
    let exact = read(turn["read_request"].clone()).unwrap();
    assert_eq!(exact["records"],json!([turn]));
}

#[test]
fn workspace_record_search_without_workspace_stays_in_current_book() {
    let f = Fixture::new();
    configure(&f,&Probe::default());
    let first = reader(&f,&f.x,None);
    say(&f,&first,"record-search-legacy-a","速度的当前书聊天");
    let second = reader(&f,&f.y,None);
    say(&f,&second,"record-search-legacy-b","速度的其他书聊天");
    let handle = context(&f,&f.a).user;
    let user = handle.lock().unwrap();
    let result = crate::workspace_record_context::read_scoped(&user,None,&f.x.book_id,
        json!({"operation":"search","query":"速度","limit":20})).unwrap();
    assert!(result.to_string().contains("当前书聊天"));
    assert!(!result.to_string().contains("其他书聊天"));
    assert!(crate::workspace_record_context::read_scoped(&user,None,&f.x.book_id,
        json!({"operation":"read","chat_session_id":second["selected_chat"]})).is_err());
}
