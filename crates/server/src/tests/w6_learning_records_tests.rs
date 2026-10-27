use super::*;
use runtime::run_context::ResidentStatePort;

#[path = "workspace_record_search_tests.rs"]
mod workspace_record_search_tests;

pub(super) fn reference_learning(f: &Fixture, chat: &str, operation: &str) -> (String, String) {
    let handle = context(f, &f.a).user;
    let publication = f.access.library.lock().unwrap().load("A", &f.y).unwrap();
    let range = EvidenceRange { start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![] };
    let resolved = publication.book.resolve_source(&range, "zh-CN", None).unwrap();
    let binding = SourceBinding { music: None, media: None, source_ref_id:format!("w6-source-{operation}"), book_id:f.y.book_id.clone(),
        publication_id:Some(f.y.publication_id.clone()),source_revision:Some(publication.book.source_fingerprint().into()),
        evidence_range:Some(range),evidence_text_digest:resolved.evidence_text_digest,
        label_snapshot:resolved.label,preview_snapshot:resolved.preview };
    let mut user = handle.lock().unwrap();
    let session = user.agent_history.sessions.iter().find(|session|session.id == chat).unwrap();
    let turn = session.turns.last().unwrap();
    let turn_ref = AgentTurnRef {session_id:chat.into(),turn_id:turn.turn_id.clone(),user_turn_ordinal:turn.user_turn_ordinal};
    crate::session_runtime::append(&mut user, &turn_ref, "source", crate::session_event::EventBody::SourcesBound {bindings:vec![binding.clone()]}).unwrap();
    let session = user.agent_history.sessions.iter().find(|session|session.id == chat).unwrap();
    let turn = session.turns.last().unwrap();
    crate::cross_book::record_delivery(&user, session, turn, turn.outcome.as_ref().unwrap(), "delivery").unwrap();
    let private = PrivateBookContext {user:&user,learning_workspace_id:session.learning_workspace_id.as_deref(),book:&publication.book,
        book_dir:publication.directory(),messages:&session.messages,selected_chat:Some(chat)};
    let evidence = crate::cross_book::learning(&private,&turn_ref,&f.y,&json!({"operation":"observe","operation_id":operation,
        "learner_quote":turn.user_authored_text(),"learning_focus":"参考资料的速度关系","expected_performance":"解释变化关系",
        "capability":"explanation","source_ref_ids":[binding.source_ref_id],"interpretation":"用户正在比较速度与变化",
        "teaching_implication":"从原表达继续讨论"}),&[binding],"observed").unwrap();
    (evidence["evidence_ref"].as_str().unwrap().into(), turn_ref.turn_id)
}

#[test]
fn w6_legacy_record_links_keep_original_jsonl_reference_sources_and_revision_chain() {
    let (f,w,mut input,probe) = setup();
    input["message"] = json!("我把速度和变化率混在了一起");
    assert_eq!(admit(&f,&w,&input).0,202);
    f.access.runs.run_one(&f.access).unwrap();
    let chat = w["selected_chat"].as_str().unwrap().to_owned();
    let (evidence,turn_id) = reference_learning(&f,&chat,"original");
    let handle = context(&f,&f.a).user;
    let (path,before,original_evidence) = {
        let user = handle.lock().unwrap();
        let mut store = user.learning_store().unwrap();
        store.correct_evidence(&evidence,"w6-first-correction","我说的是加速度").unwrap();
        store.correct_evidence("correction:w6-first-correction","w6-second-correction","更确切地说，是瞬时加速度").unwrap();
        let path = user.session_store.as_ref().unwrap().paths.session(&chat);
        let bytes = std::fs::read(&path).unwrap();
        // This is the supported pre-workspace JSONL shape: no scope is fabricated on load.
        let created:Value = serde_json::from_str(std::str::from_utf8(&bytes).unwrap().lines().next().unwrap()).unwrap();
        assert!(created["payload"].get("learning_workspace_id").is_none());
        (path,bytes,json!(store.evidence(&evidence).unwrap()))
    };
    let target = f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"明确加速度关系"},"members":[f.x]}));
    let scope = target["learning_workspace_id"].as_str().unwrap();
    let list = f.ok(&f.a,"GET","/api/me/learning-records",Value::Null);
    let item = list["records"].as_array().unwrap().iter().find(|record|record["session_id"] == chat).unwrap();
    assert!(item["source_learning_workspace_id"].is_null());
    let seq = item["through_seq"].as_u64().unwrap();
    let url = format!("/api/me/learning-records/{chat}?through_seq={seq}");
    let detail = f.ok(&f.a,"GET",&url,Value::Null);
    assert_eq!(detail["turns"].as_array().unwrap().len(),1);
    assert_eq!(detail["recap"]["sources"][0]["published_book_ref"],json!(f.y));
    assert!(detail["recap"]["sources"][0]["unavailable_reason"].is_null());
    assert_eq!(detail["sources"][0]["published_book_ref"],json!(f.y));
    assert!(detail["sources"][0]["view"]["source_url"].as_str().unwrap().contains(&f.y.publication_id));
    assert_eq!(detail["learning"]["evidence"].as_array().unwrap().len(),3);
    assert!(detail["learning"]["evidence"].as_array().unwrap().iter().any(|e|e["supersedes"] == "correction:w6-first-correction"));
    assert_eq!(f.call(&f.b,"GET",&url,Value::Null).0,404);
    let link_url = format!("/api/learning-workspaces/{scope}/records");
    let request = json!({"session_id":chat,"through_seq":seq,"goal_revision":1});
    let first = f.ok(&f.a,"POST",&link_url,request.clone());
    assert_eq!(first["selection"]["turn_ids"],json!([turn_id]));
    assert_eq!(f.ok(&f.a,"POST",&link_url,request.clone()),first);
    assert_eq!(f.call(&f.b,"POST",&link_url,request).0,404);
    let linked = f.ok(&f.a,"GET",&link_url,Value::Null);
    assert_eq!(linked["records"].as_array().unwrap().len(),1);
    assert_eq!(linked["records"][0]["through_seq"],seq);
    {
        let user = handle.lock().unwrap();
        let original = user.agent_history.sessions.iter().find(|session|session.id==chat).unwrap();
        assert!(original.learning_workspace_id.is_none());
        assert_eq!(json!(user.learning_store().unwrap().evidence(&evidence).unwrap()),original_evidence);
        let target = user.learning_store_for_workspace(Some(scope)).unwrap();
        assert!(target.evidence(&evidence).is_err());
        assert_eq!(target.linked_record_bundles().unwrap()[0].evidence.len(),3);
    }
    assert_eq!(std::fs::read(&path).unwrap(),before);
    f.access.library.lock().unwrap().revoke("A",&f.y).unwrap();
    let unavailable = f.ok(&f.a,"GET",&url,Value::Null);
    assert_eq!(unavailable["sources"][0]["quote"],detail["sources"][0]["quote"]);
    assert!(unavailable["sources"][0]["view"].is_null());
    assert!(unavailable["recap"]["sources"][0]["unavailable_reason"].is_string());
    assert_eq!(std::fs::read(&path).unwrap(),before);
    drop(handle);
    let f = reopen(f,&probe);
    assert_eq!(f.ok(&f.a,"GET",&link_url,Value::Null)["records"],linked["records"]);
    assert_eq!(f.ok(&f.a,"GET",&url,Value::Null)["learning"],unavailable["learning"]);
    assert_eq!(std::fs::read(&path).unwrap(),before);
}

#[test]
fn w6_selection_keeps_unselected_chat_out_and_checks_current_goal_before_linking() {
    let (f,w,mut input,_) = setup();
    input["message"] = json!("选定这段学习记录");
    assert_eq!(admit(&f,&w,&input).0,202);
    f.access.runs.run_one(&f.access).unwrap();
    let first_chat = w["selected_chat"].as_str().unwrap().to_owned();
    reference_learning(&f,&first_chat,"selected");
    let detail = f.ok(&f.a,"GET",&format!("/api/me/learning-records/{first_chat}"),Value::Null);
    let target = f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"学习速度"}}));
    let scope = target["learning_workspace_id"].as_str().unwrap();
    let link_url = format!("/api/learning-workspaces/{scope}/records");
    let request = json!({"session_id":first_chat,"through_seq":detail["through_seq"],"goal_revision":1});
    f.ok(&f.a,"POST",&format!("/api/learning-workspaces/{scope}/goal"),json!({"goal":{"text":"只复习加速度"}}));
    let rejected = f.call(&f.a,"POST",&link_url,request.clone());
    assert_eq!(rejected.1["error_code"],"WORKSPACE_GOAL_REVISION_CONFLICT");
    assert!(f.ok(&f.a,"GET",&link_url,Value::Null)["records"].as_array().unwrap().is_empty());
    let w = f.get(&f.a,&w);
    let w = f.action(&f.a,&w,"page","chat/new",json!({}));
    let second_chat = w["selected_chat"].as_str().unwrap().to_owned();
    let mut input = Fixture::stamp(&w,"page");
    input["session_id"] = json!(second_chat); input["client_request_id"] = json!("w6-unselected"); input["message"] = json!("这段没有选择带入");
    assert_eq!(admit(&f,&w,&input).0,202);
    f.access.runs.run_one(&f.access).unwrap();
    let (unselected,_) = reference_learning(&f,&second_chat,"unselected");
    let mut request = request; request["goal_revision"] = json!(2);
    f.ok(&f.a,"POST",&link_url,request);
    let handle = context(&f,&f.a).user;
    let user = handle.lock().unwrap();
    let target = user.learning_store_for_workspace(Some(scope)).unwrap();
    let bundles = target.linked_record_bundles().unwrap();
    assert_eq!(bundles.len(),1);
    assert!(bundles[0].facts.iter().all(|fact|fact.binding.chat_session_id == first_chat));
    assert!(bundles[0].evidence.iter().all(|evidence|evidence.evidence_id != unselected));
    assert!(target.current_interpretations(&f.y.book_id,"irrelevant",0,20).unwrap().is_empty());
    drop(user);
    f.ok(&f.a,"POST",&format!("/api/learning-workspaces/{scope}/members"),json!({"published_book_ref":f.x}));
    let target_reader = f.ok(&f.a,"POST","/api/workspaces",json!({"published_book_ref":f.x,"learning_workspace_id":scope,"attachment_id":"target"}));
    let target_reader = f.action(&f.a,&target_reader,"target","chat/new",json!({}));
    let mut port = f.run(&target_reader,"target");
    assert!(port.workspace_records_available());
    let listed = port.workspace_records(json!({"operation":"list"})).unwrap();
    let records = listed["records"].as_array().unwrap();
    assert_eq!(records.len(),2);
    assert!(records.iter().any(|record|record["chat_session_id"] == first_chat && record["origin"] == "linked"));
    assert!(records.iter().any(|record|record["chat_session_id"] == target_reader["selected_chat"] && record["origin"] == "native"));
    assert!(records.iter().all(|record|record["chat_session_id"] != second_chat));
    let read = port.workspace_records(json!({"operation":"read","chat_session_id":first_chat,"limit":20})).unwrap();
    assert!(read["selection"]["source_learning_workspace_id"].is_null());
    assert_eq!(read["records"][0]["user"],"选定这段学习记录");
    assert_eq!(read["current_mastery"],false);
    assert!(port.workspace_records(json!({"operation":"read","chat_session_id":second_chat})).is_err());
}

#[test]
fn w6_other_workspace_links_freeze_chat_prefix_until_another_explicit_selection() {
    let f = Fixture::new();
    let probe = Probe::default(); configure(&f,&probe);
    let source = f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"学习深度学习"},"members":[f.x]}));
    let target = f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"期末冲刺"}}));
    let source_id = source["learning_workspace_id"].as_str().unwrap();
    let target_id = target["learning_workspace_id"].as_str().unwrap();
    let w = f.ok(&f.a,"POST","/api/workspaces",json!({"published_book_ref":f.x,"learning_workspace_id":source_id,"attachment_id":"page"}));
    let w = f.action(&f.a,&w,"page","chat/new",json!({}));
    let chat = w["selected_chat"].as_str().unwrap();
    let mut input = Fixture::stamp(&w,"page");
    input["session_id"] = json!(chat); input["client_request_id"] = json!("w6-scoped-first"); input["message"] = json!("第一段原记录");
    assert_eq!(admit(&f,&w,&input).0,202); f.access.runs.run_one(&f.access).unwrap();
    let detail_url = format!("/api/me/learning-records/{chat}");
    let old = f.ok(&f.a,"GET",&detail_url,Value::Null);
    assert_eq!(old["turns"][0]["workspace_goal"]["text"],"学习深度学习");
    assert_eq!(old["turns"][0]["workspace_goal"]["revision"],1);
    assert!(old["turns"][0]["accepted_workspace_goal"].is_null());
    assert!(old["turns"][0].get("question_quote").is_some());
    let selected = json!({"session_id":chat,"through_seq":old["through_seq"],"goal_revision":1});
    let link_url = format!("/api/learning-workspaces/{target_id}/records");
    let link = f.ok(&f.a,"POST",&link_url,selected.clone());
    assert_eq!(link["selection"]["source_learning_workspace_id"],source_id);
    assert_ne!(f.call(&f.a,"POST",&format!("/api/learning-workspaces/{source_id}/records"),selected.clone()).0,200);
    f.ok(&f.a,"POST",&format!("/api/learning-workspaces/{source_id}/goal"),json!({"goal":{"text":"改为学习线性代数"}}));
    let w = f.get(&f.a,&w);
    let mut later = Fixture::stamp(&w,"page");
    later["session_id"] = json!(chat); later["client_request_id"] = json!("w6-scoped-later"); later["message"] = json!("稍后新增的原记录");
    assert_eq!(admit(&f,&w,&later).0,202); f.access.runs.run_one(&f.access).unwrap();
    let current = f.ok(&f.a,"GET",&detail_url,Value::Null);
    assert_eq!(current["turns"].as_array().unwrap().len(),2);
    assert_eq!(current["turns"][0]["workspace_goal"]["text"],"学习深度学习");
    assert_eq!(current["turns"][1]["workspace_goal"]["text"],"改为学习线性代数");
    let frozen = f.ok(&f.a,"GET",&format!("{detail_url}?through_seq={}",old["through_seq"]),Value::Null);
    assert_eq!(frozen["turns"],old["turns"]);
    assert_eq!(f.ok(&f.a,"POST",&link_url,selected),link);
    let mut updated = json!({"session_id":chat,"through_seq":current["through_seq"],"goal_revision":1});
    let expanded = f.ok(&f.a,"POST",&link_url,updated.take());
    assert_eq!(expanded["selection"]["turn_ids"].as_array().unwrap().len(),2);
    assert_eq!(expanded["linked_at"],link["linked_at"]);
    assert_eq!(f.ok(&f.a,"GET",&link_url,Value::Null)["records"].as_array().unwrap().len(),1);
    let invalid = f.call(&f.a,"GET",&format!("{detail_url}?through_seq=999999"),Value::Null);
    assert_eq!(invalid.1["error_code"],"INVALID_RECAP_RANGE");
}

#[test]
fn w6_old_source_without_publication_keeps_saved_excerpt_without_guessing_current_book() {
    let (f,w,input,probe) = setup();
    assert_eq!(admit(&f,&w,&input).0,202); f.access.runs.run_one(&f.access).unwrap();
    let chat = w["selected_chat"].as_str().unwrap().to_owned();
    let publication = f.access.library.lock().unwrap().load("A",&f.x).unwrap();
    let range = EvidenceRange {start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![]};
    let source = publication.book.resolve_source(&range,"zh-CN",None).unwrap();
    let handle = context(&f,&f.a).user;
    let path = {
        let mut user = handle.lock().unwrap();
        let session = user.agent_history.sessions.iter().find(|session|session.id == chat).unwrap();
        let turn = &session.turns[0];
        let turn_ref = AgentTurnRef {session_id:chat.clone(),turn_id:turn.turn_id.clone(),user_turn_ordinal:turn.user_turn_ordinal};
        crate::session_runtime::append(&mut user,&turn_ref,"old-source",crate::session_event::EventBody::SourcesBound {bindings:vec![SourceBinding { music: None, media: None,
            source_ref_id:"old-original-source".into(),book_id:f.x.book_id.clone(),publication_id:None,source_revision:None,
            evidence_range:Some(range),evidence_text_digest:source.evidence_text_digest,label_snapshot:source.label.clone(),preview_snapshot:source.preview.clone(),
        }]}).unwrap();
        user.session_store.as_ref().unwrap().paths.session(&chat)
    };
    drop(handle);
    // Represent the supported old JSONL shape before reopening: it did not record a publication.
    let mut legacy = String::new();
    for line in std::fs::read_to_string(&path).unwrap().lines() {
        let mut event:Value = serde_json::from_str(line).unwrap();
        if event["kind"] == "turn.accepted" { event["payload"]["turn"].as_object_mut().unwrap().remove("published_book_ref"); }
        legacy.push_str(&event.to_string()); legacy.push('\n');
    }
    std::fs::write(&path,&legacy).unwrap();
    let f = reopen(f,&probe);
    let detail = f.ok(&f.a,"GET",&format!("/api/me/learning-records/{chat}"),Value::Null);
    assert_eq!(detail["sources"][0]["label"],source.label);
    assert_eq!(detail["sources"][0]["quote"],source.preview);
    assert!(detail["sources"][0]["published_book_ref"].is_null());
    assert!(detail["sources"][0]["view"].is_null());
    assert!(detail["sources"][0]["unavailable_reason"].is_string());
    assert!(detail["recap"]["sources"][0]["published_book_ref"].is_null());
    assert_eq!(std::fs::read_to_string(&path).unwrap(),legacy);
    assert!(f.access.library.lock().unwrap().load("A",&f.x).is_ok(),"a readable current publication must not be guessed as the original");
}

#[test]
fn om2_continuity_selected_prefix_keeps_origin_and_latest_correction_after_reopen() {
    let (f,w,mut input,probe)=setup();
    input["message"]=json!("我是在看过例子之后才说出速度关系的");
    assert_eq!(admit(&f,&w,&input).0,202);
    f.access.runs.run_one(&f.access).unwrap();
    let chat=w["selected_chat"].as_str().unwrap().to_owned();
    let (evidence,turn_id)=reference_learning(&f,&chat,"om2-selected");
    let handle=context(&f,&f.a).user;
    let original={
        let user=handle.lock().unwrap();
        let mut store=user.learning_store().unwrap();
        let original=store.evidence(&evidence).unwrap();
        store.correct_evidence(&evidence,"om2-linked-correction","这次表达得到过示例帮助，不代表独立运用。").unwrap();
        original
    };
    drop(handle);
    let prefix=f.ok(&f.a,"GET",&format!("/api/me/learning-records/{chat}"),Value::Null);
    let target=f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"从先前作品继续学习"},"members":[f.x]}));
    let scope=target["learning_workspace_id"].as_str().unwrap().to_owned();
    f.ok(&f.a,"POST",&format!("/api/learning-workspaces/{scope}/records"),
        json!({"session_id":chat,"through_seq":prefix["through_seq"],"goal_revision":1}));
    let w=f.get(&f.a,&w);
    let mut later=Fixture::stamp(&w,"page");
    later["session_id"]=json!(chat); later["client_request_id"]=json!("om2-after-prefix");
    later["message"]=json!("这条新作答没有被选入");
    assert_eq!(admit(&f,&w,&later).0,202); f.access.runs.run_one(&f.access).unwrap();
    let (excluded,_)=reference_learning(&f,&chat,"om2-excluded");
    let f=reopen(f,&probe);
    let publication=f.access.library.lock().unwrap().load("A",&f.x).unwrap();
    let handle=context(&f,&f.a).user;
    let user=handle.lock().unwrap();
    user.learning_store_for_workspace(Some(&scope)).unwrap().sync_workspace_goal(1,"om2-goal").unwrap();
    let private=PrivateBookContext {user:&user,learning_workspace_id:Some(&scope),book:&publication.book,
        book_dir:publication.directory(),messages:&[],selected_chat:None};
    let turn=AgentTurnRef {session_id:"om2-target-chat".into(),turn_id:"om2-target-turn".into(),user_turn_ordinal:1};
    let prepared=teaching::freeze_prepare(&private,&turn,&json!({"message":"继续原来的作品"}),None,"om2").unwrap();
    let view=&prepared.context.as_ref().unwrap()["learner_context"]["learning_opportunity"]["continuity"];
    let work=view["previous_work"].as_array().unwrap().iter()
        .find(|row|row["fact_refs"]==json!([original.action_ref])).unwrap();
    assert_eq!(work["selection"]["turn_ids"],json!([turn_id]));
    assert_eq!(work["selection"]["through_seq"],prefix["through_seq"]);
    assert!(work["selection"]["source_learning_workspace_id"].is_null());
    assert_eq!(work["published_book_ref"],json!(f.y));
    assert_eq!(work["source_refs"][0]["publication_id"],f.y.publication_id);
    assert_eq!(work["evidence_refs"],json!(["correction:om2-linked-correction"]));
    assert_eq!(work["historical"],true);
    assert_eq!(work["current_mastery"],false);
    assert!(!view.to_string().contains(&excluded));
    assert!(!view.to_string().contains("这条新作答没有被选入"),"{view:#}");
    assert!(view["teaching_implications"].as_array().unwrap().iter().any(|row|
        row["evidence_ref"]=="correction:om2-linked-correction" && row["historical"]==true));
    let read=crate::workspace_record_context::read(&user,&scope,
        json!({"operation":"read","chat_session_id":chat,"limit":20})).unwrap();
    assert!(read["records"].as_array().unwrap().iter().any(|row|row["kind"]=="fact"
        && row["record"]["event_id"]==original.action_ref && row["record"]["payload"]["response"]==input["message"]));
    assert_eq!(user.learning_store().unwrap().evidence(&evidence).unwrap(),original);
    assert!(user.learning_store_for_workspace(Some(&scope)).unwrap().evidence(&evidence).is_err());
    if let Ok(output)=std::env::var("OM2_OUTPUT") {
        std::fs::write(Path::new(&output).join("linked-context.json"),serde_json::to_vec_pretty(view).unwrap()).unwrap();
    }
    drop(user);
    let other=f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"另一个目标"},"members":[f.x]}));
    let user=handle.lock().unwrap();
    let private=PrivateBookContext {user:&user,learning_workspace_id:other["learning_workspace_id"].as_str(),
        book:&publication.book,book_dir:publication.directory(),messages:&[],selected_chat:None};
    let prepared=teaching::freeze_prepare(&private,&turn,&json!({"message":"新任务"}),None,"om2-other").unwrap();
    let view=&prepared.context.as_ref().unwrap()["learner_context"]["learning_opportunity"]["continuity"];
    assert!(view["previous_work"].as_array().unwrap().iter().all(|row|row["historical"]==false));
    assert!(view["teaching_implications"].as_array().unwrap().is_empty());
}

#[test]
fn om2_continuity_same_workspace_recovers_prior_work_in_a_new_chat() {
    let f=Fixture::new();
    let probe=Probe::default(); configure(&f,&probe);
    let scope=f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"理解平均速度"},"members":[f.x,f.y]}));
    let scope=scope["learning_workspace_id"].as_str().unwrap().to_owned();
    let w=f.ok(&f.a,"POST","/api/workspaces",json!({"published_book_ref":f.x,"learning_workspace_id":scope,"attachment_id":"page"}));
    let w=f.action(&f.a,&w,"page","chat/new",json!({}));
    let mut input=Fixture::stamp(&w,"page");
    input["session_id"]=w["selected_chat"].clone(); input["client_request_id"]=json!("om2-workspace-first");
    input["message"]=json!("我把路程除以包括停车的总时间");
    let (status,first)=admit(&f,&w,&input); assert_eq!(status,202); f.access.runs.run_one(&f.access).unwrap();
    let other=f.ok(&f.a,"POST","/api/workspaces",json!({"published_book_ref":f.y,"learning_workspace_id":scope,"attachment_id":"page"}));
    let other=f.action(&f.a,&other,"page","chat/new",json!({}));
    let mut other_input=Fixture::stamp(&other,"page");
    other_input["session_id"]=other["selected_chat"].clone(); other_input["client_request_id"]=json!("om2-other-book");
    other_input["message"]=json!("这是另一份资料的作品");
    assert_eq!(admit(&f,&other,&other_input).0,202); f.access.runs.run_one(&f.access).unwrap();
    let f=reopen(f,&probe);
    let w=f.action(&f.a,&f.get(&f.a,&w),"page","attach",json!({}));
    let w=f.action(&f.a,&w,"page","chat/new",json!({}));
    let mut input=Fixture::stamp(&w,"page");
    input["session_id"]=w["selected_chat"].clone(); input["client_request_id"]=json!("om2-workspace-second");
    input["message"]=json!("从我先前的计算继续");
    let (status,second)=admit(&f,&w,&input); assert_eq!(status,202);
    let handle=context(&f,&f.a).user;
    let user=handle.lock().unwrap();
    let store=user.learning_store_for_workspace(Some(&scope)).unwrap();
    let bound=store.teaching_event(&format!("binding:{}",second["turn_id"].as_str().unwrap())).unwrap();
    let view=&bound.payload["learner_context"]["learning_opportunity"]["continuity"];
    assert_eq!(view["scope"]["learning_workspace_id"],scope);
    assert_eq!(view["scope"]["workspace_goal_revision"],1);
    assert!(view["previous_work"].as_array().unwrap().iter().any(|row|
        row["binding"]["turn_id"]==first["turn_id"] && row["learner_quote_excerpt"]["text"]=="我把路程除以包括停车的总时间"));
    assert!(view["previous_work"].as_array().unwrap().iter().all(|row|row["historical"]==false));
    assert!(!view.to_string().contains("这是另一份资料的作品"));
}
