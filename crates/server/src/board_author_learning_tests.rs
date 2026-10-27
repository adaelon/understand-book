use super::*;
use std::cell::RefCell;

const NOW: &str = "2026-10-26T09:00:00Z";

fn blank_run() -> (tempfile::TempDir, AppState, crate::agent_run::PreparedAgentChat, Value, Value) {
    let (root, mut state, metadata) = crate::tests::board_browser_host::fixture();
    let board = board_api::route(&state.user, &state.workspace, "create", &json!({"title":"从问题开始"}), NOW).unwrap();
    let prepared = prepare_agent_chat(&mut state, &json!({"message":"整理已有解释，画出结构后调整节点。",
        "client_request_id":"board-learning-root", "session_id":metadata["session_id"], "expected_chat_head":metadata["turn_id"],
        "board_follow_up":{"board_id":board["board_id"],"board_revision":board["revision"]}}).to_string(), NOW)
        .unwrap_or_else(|reply|panic!("{}",reply.body));
    (root,state,prepared,board,metadata)
}

#[test]
fn board_layout_read_supplies_exact_new_explanation_refs_for_mappings_and_routes() {
    let (_root, mut state, prepared, board, _metadata) = blank_run();
    let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
    let storage = LocalStorage { port: &port, scope: &prepared.scope };
    let frozen = prepared.scope.board_context.as_ref().unwrap();
    let token = CancellationToken::default();
    let run = |request: Value| execute(&storage, frozen, &prepared.turn_ref,
        serde_json::from_value(request).unwrap(), &[], &[], &token);
    // The live Agent began with this paginated layout read on an empty board.
    let observed = run(json!({"operation":"read","collection":"layout","board_revision":board["revision"]})).unwrap().body;
    assert_eq!(observed["records"], json!([]));
    assert_eq!(observed["reserved_answer_item_id"], reserved_answer_item_id(&prepared.turn_ref.turn_id));
    assert_eq!(observed["proposed_content_revision"], board["revision"].as_u64().unwrap() + 1);
    assert!(observed["answer_position"].is_object());
    assert!(observed["answer_size"].is_object());
    let explanation = json!({"item_id":observed["reserved_answer_item_id"],"block_id":"b-history",
        "content_revision":observed["proposed_content_revision"]});
    let mut request = json!({"operation":"prepare","base_board_revision":observed["board_revision"],
        "blocks":[{"block_id":"b-history","markdown":"原始表达与本次解释分开，按关系逐步讲解。"}],
        "drawings":[
            {"item_id":"probe-a","title":"上下文","position":{"x":400,"y":0},"size":{"width":160,"height":80},
                "graphic":{"kind":"shape","shape":"rectangle","text":"上下文","color":"#BFDBFE"}},
            {"item_id":"probe-b","title":"状态","position":{"x":800,"y":0},"size":{"width":160,"height":80},
                "graphic":{"kind":"shape","shape":"rectangle","text":"状态","color":"#BBF7D0"}}],
        "links":[{"link_id":"probe-link","from":{"kind":"item","item_id":"probe-a"},"to":{"kind":"item","item_id":"probe-b"},"label":"保留供后续使用","based_on":[]}],
        "mappings":[{"mapping_id":"probe-map","explanation":explanation,"visual":{"kind":"link","link_id":"probe-link"}}],
        "routes":[{"route_id":"probe-route","title":"逐步讲解","steps":[{"step_id":"history","explanation":explanation,
            "reveal_targets":[{"kind":"item","item_id":"probe-a"},{"kind":"item","item_id":"probe-b"},{"kind":"link","link_id":"probe-link"}],
            "focus_targets":[{"kind":"link","link_id":"probe-link"}],"transition":"draw","duration_ms":900}]}]});
    // Preserve the two actual failure modes as rejected, actionable requests.
    request["mappings"][0]["explanation"]["item_id"] = json!("b-history");
    let missing_item = run(request.clone()).err().unwrap();
    assert!(missing_item.message.contains(&explanation.to_string()), "{}", missing_item.message);
    request["mappings"][0]["explanation"]["item_id"] = json!("probe-a");
    let different_item = run(request.clone()).err().unwrap();
    assert!(!different_item.message.contains("Available in the proposed board"), "an existing item must not be silently redirected to another source: {}", different_item.message);
    request["mappings"][0]["explanation"] = explanation.clone();
    request["routes"][0]["steps"][0]["explanation"]["content_revision"] = observed["board_revision"].clone();
    let previous_revision = run(request.clone()).err().unwrap();
    assert!(previous_revision.message.contains(&explanation.to_string()), "{}", previous_revision.message);
    request["routes"][0]["steps"][0]["explanation"] = explanation.clone();
    let result = run(request).unwrap().body;
    assert_eq!(result["candidate_revision"], 1, "rejected candidates do not replace the current candidate");
    assert_eq!(result["explanation_refs"], json!([explanation]));
    assert_eq!(result["proposed_content_revision"], observed["proposed_content_revision"]);
    storage.with_user(|user| {
        let candidate = selected_candidate(user, &prepared.turn_ref, 1)?;
        assert_eq!(candidate.links.len(), 1);
        assert_eq!(candidate.mappings.len(), 1);
        assert_eq!(candidate.routes[0].steps.len(), 1);
        assert!(read_current(user, frozen)?.1.items.is_empty(), "prepare is private until preview and commit");
        Ok(())
    }).unwrap();
}

#[test]
fn board_catalog_searches_full_answer_beyond_first_page_and_imports_exact_origin() {
    let (_root, mut state, metadata) = crate::tests::board_browser_host::fixture();
    let original = state.user.agent_history.sessions.iter().find(|session|session.id == metadata["session_id"])
        .unwrap().turns.iter().find(|turn|turn.turn_id == metadata["turn_id"]).unwrap().clone();
    let book_id = state.workspace.book.base.book_id.clone();
    let mut oldest = None;
    for index in 0..24 {
        let reference = precommit_agent_turn(&mut state, &book_id, format!("原问题 {index}"), None, None, None, NOW).unwrap();
        let mut outcome = original.outcome.clone().unwrap();
        let text = if index == 0 { format!("{}末尾检索词", "原始完整解释。".repeat(50)) } else { format!("其他解释 {index}") };
        outcome.answer = Some(text.clone());
        outcome.answer_view.as_mut().unwrap().parts = vec![AgentAnswerPart::Markdown{text}];
        let messages = state.workspace.messages.clone();
        finalize_agent_turn_completed(&mut state, &reference, &outcome, &messages, NOW).unwrap();
        if index == 0 { oldest = Some(reference); }
    }
    let page = board_api::catalog(&state.user,&state.workspace,&json!({"kind":"answer","limit":5})).unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(),5);
    assert_eq!(page["next"],5);
    assert!(page["total"].as_u64().unwrap() > 20);
    let found = board_api::catalog(&state.user,&state.workspace,&json!({"kind":"answer","query":"末尾检索词","limit":5})).unwrap();
    assert_eq!(found["total"],1);
    let source:board_api::BoardImportSource=serde_json::from_value(found["items"][0]["source"].clone()).unwrap();
    let imported=board_api::import_item(&state.user,&state.workspace,source,NOW).unwrap();
    let oldest=oldest.unwrap();
    assert!(imported.readable_content.ends_with("末尾检索词"));
    assert!(imported.readable_content.chars().count()>160);
    assert!(matches!(imported.target,BoardTarget::Answer{ref session_id,ref turn_id,..} if session_id==&oldest.session_id && turn_id==&oldest.turn_id));
    let actual=state.user.agent_history.sessions.iter().find(|session|session.id==oldest.session_id).unwrap().turns.iter().find(|turn|turn.turn_id==oldest.turn_id).unwrap();
    assert_eq!(imported.source_bindings,note_api::bindings(&actual.source_bindings));
}

#[test]
fn board_root_candidate_imports_sources_and_atomically_edits_native_drawings_and_layout() {
    let (_root, mut state, prepared, board, metadata)=blank_run();
    let port=crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
    let storage=LocalStorage{port:&port,scope:&prepared.scope};
    let frozen=prepared.scope.board_context.as_ref().unwrap();
    assert!(frozen.selected_item.is_none());
    let token=CancellationToken::default();
    let source=execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(json!({"operation":"source_read","source":metadata["sources"]["answer"]})).unwrap(),&[],&[],&token).unwrap().body;
    let scene=execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(json!({"operation":"source_read","source":metadata["sources"]["presentation"]})).unwrap(),&[],&[],&token).unwrap().body;
    assert_eq!(scene["source"]["receipt"],scene["item"]["target"]["receipt"]);
    storage.with_user(|user| {
        let source:board_api::BoardImportSource=serde_json::from_value(scene["source"].clone()).unwrap();
        let item=board_api::import_item(user,&storage.source_workspace(None)?,source,NOW)?;
        assert_eq!(json!(item.target),scene["item"]["target"]); Ok(())
    }).unwrap();
    let request:AuthorRequest=serde_json::from_value(json!({"operation":"prepare","base_board_revision":board["revision"],
        "blocks":[{"block_id":"root-explanation","markdown":"把原解释放在右侧，再看时间关系。"}],
        "imports":[{"source":metadata["sources"]["answer"],"item_id":"retained-original","position":{"x":900,"y":40},"size":{"width":320,"height":220}}],
        "drawings":[{"item_id":"native-time","title":"时间关系","position":{"x":480,"y":40},"size":{"width":250,"height":160},
            "graphic":{"kind":"shape","shape":"rectangle","text":"总时间 = 两段相加","color":"#177e77"},"based_on":[]}]})).unwrap();
    execute(&storage,frozen,&prepared.turn_ref,request,&[],&[],&token).unwrap();
    let first=storage.with_user(|user| {
        let mut candidate=selected_candidate(user,&prepared.turn_ref,1)?;
        let original=candidate.items.iter().find(|item|item.item_id=="retained-original").unwrap();
        assert_eq!(json!(original.target),source["item"]["target"]);
        assert_eq!(json!(original.source_bindings),source["item"]["source_bindings"]);
        assert!(candidate.items.iter().any(|item|matches!(item.target,BoardTarget::Presentation{..})),"inline presentation is retained independently");
        assert!(candidate.items.iter().all(|item|item.reply_to_item_id.is_none()));
        candidate.previewed=true; // The real rendering/inspection gate has separate preview tests.
        session_runtime::update_board_authoring(user,&prepared.turn_ref,NOW,candidate)
    }).unwrap();
    let _=first;
    let review=||Some(runtime::board_author::ReviewedDelivery{teaching_delivery_move_refs:vec![],delivery_scope:None});
    let committed=execute(&storage,frozen,&prepared.turn_ref,AuthorRequest::Commit{candidate_revision:1,reviewed_delivery:review()},&[],&[],&token).unwrap().committed.unwrap();
    assert!(committed.parent_item_id.is_none());
    let before=storage.with_user(|user|Ok(read_current(user,frozen)?.1)).unwrap();
    let original=before.items.iter().find(|item|item.item_id=="retained-original").unwrap().clone();
    let mut request=json!({"operation":"prepare","base_board_revision":before.revision,
        "blocks":[{"block_id":"revised-explanation","markdown":"将时间关系改为两个分支，并移动原解释。"}],"drawings":[],
        "update_drawings":[{"item_id":"native-time","title":"时间分支","position":{"x":500,"y":400},"size":{"width":280,"height":170},
            "graphic":{"kind":"shape","shape":"ellipse","text":"去程时间 + 回程时间","color":"#2563eb"},"based_on":[]}],
        "layout":[{"item_id":"retained-original","position":{"x":1000,"y":300},"size":{"width":340,"height":240}}]});
    request["update_drawings"][0]["item_id"]=json!("retained-original");
    assert!(execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(request.clone()).unwrap(),&[],&[],&token).is_err(),"retained source text cannot become a drawing edit");
    request["update_drawings"][0]["item_id"]=json!("native-time");
    execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(request).unwrap(),&[],&[],&token).unwrap();
    let candidate_revision=storage.with_user(|user| {
        let mut candidate=turn(user,&prepared.turn_ref)?.domain.board_authoring.clone().unwrap();
        let projected=project(&before,&candidate)?;
        assert_eq!(projected.items.iter().find(|item|item.item_id=="retained-original").unwrap().target,original.target);
        assert_eq!(read_current(user,frozen)?.1,before,"prepare/preview projection does not publish edits");
        let revision=candidate.candidate_revision; candidate.previewed=true;
        session_runtime::update_board_authoring(user,&prepared.turn_ref,NOW,candidate)?; Ok(revision)
    }).unwrap();
    let saved=execute(&storage,frozen,&prepared.turn_ref,AuthorRequest::Commit{candidate_revision,reviewed_delivery:review()},&[],&[],&token).unwrap().committed.unwrap();
    storage.with_user(|user| {
        let (store,after)=read_current(user,frozen)?;
        assert_eq!(after.revision,before.revision+1);
        assert_eq!(saved.item_ids.iter().filter(|id|id.as_str()=="native-time").count(),1);
        let moved=after.items.iter().find(|item|item.item_id=="retained-original").unwrap();
        assert_eq!(moved.target,original.target); assert_eq!(moved.readable_content,original.readable_content);
        assert_eq!(moved.position,BoardPosition{x:1000.0,y:300.0});
        let edited=after.items.iter().find(|item|item.item_id=="native-time").unwrap();
        assert_eq!(edited.readable_content,"去程时间 + 回程时间");
        assert_eq!(store.read_revision(&before.board_id,before.revision)?,before);
        Ok(())
    }).unwrap();
}


#[test]
fn board_source_import_preserves_explicit_linked_prefix_and_original_presentation_owner() {
    let (_root,mut state,metadata)=crate::tests::board_browser_host::fixture();
    let original_book=state.workspace.book.base.book_id.clone();
    let mut outcome=state.user.agent_history.sessions.iter().find(|s|s.id==metadata["session_id"]).unwrap().turns.iter().find(|t|t.turn_id==metadata["turn_id"]).unwrap().outcome.clone().unwrap();
    let owner=PresentationOwner{book_id:original_book.clone(),session_id:metadata["session_id"].as_str().unwrap().into()};
    let old_reference:PresentationRef=serde_json::from_value(metadata["reference"].clone()).unwrap();
    let content=presentation_store::PresentationStore::for_user(&state.user).unwrap().read_version(&owner,&old_reference).unwrap().content;
    let (history,store)=crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    state.user.agent_history=history; state.user.session_store=store;
    assert_eq!(crate::route_agent_new(&mut state,NOW).status,200);
    let source=precommit_agent_turn(&mut state,&original_book,"原始跨资料问题".into(),None,None,None,NOW).unwrap();
    let candidate=state.private_context().create_presentation_candidate(&source.session_id,&source.turn_id,None,content).unwrap();
    let reference=state.private_context().persist_presentation_candidate(&source.session_id,&source.turn_id,&candidate.candidate_id).unwrap();
    outcome.answer_view.as_mut().unwrap().parts=vec![AgentAnswerPart::Presentation{presentation_id:reference.presentation_id.clone(),revision:reference.revision}];
    outcome.answer=Some("原解释与完整演示".into());
    outcome.answer_view.as_mut().unwrap().parts.insert(0,AgentAnswerPart::Markdown{text:"原解释与完整演示".into()});
    let messages=state.workspace.messages.clone();
    finalize_agent_turn_completed(&mut state,&source,&outcome,&messages,NOW).unwrap();
    let session_id=source.session_id.as_str(); let turn_id=source.turn_id.as_str();
    let original=state.user.agent_history.sessions.iter().find(|s|s.id==session_id).unwrap().turns.iter().find(|t|t.turn_id==turn_id).unwrap().clone();
    let through=state.user.session_store.as_ref().unwrap().logs[session_id].projection.through_seq;
    state.workspace.learning_workspace_id=Some("linked-board-learning".into());
    state.user.learning_store_for_workspace(state.workspace.learning_workspace_id.as_deref()).unwrap().link_records(
        &memory::learning_records::LearningRecordSelection{source_learning_workspace_id:None,chat_session_id:session_id.into(),through_seq:through,turn_ids:vec![turn_id.into()]},NOW).unwrap();
    let original_book=state.workspace.book.base.book_id.clone();
    // A target board belongs to another source publication. The retained version
    // still belongs to the original book/session after the source chat disappears.
    let mut base=state.workspace.book.base.clone(); base.book_id="target-board-source".into();
    state.workspace.book=std::sync::Arc::new(Book::new(base,"目标资料"));
    let board=board_api::route(&state.user,&state.workspace,"create",&json!({"title":"跨工作区资料"}),NOW).unwrap();
    let cached=state.user.agent_history.sessions.iter_mut().find(|s|s.id==session_id).unwrap().turns.iter_mut().find(|t|t.turn_id==turn_id).unwrap();
    cached.user="较晚缓存内容不能替换导入前缀".into();
    let board=board_api::route(&state.user,&state.workspace,"import",&json!({"board_id":board["board_id"],"board_revision":board["revision"],"source":{"kind":"answer","session_id":session_id,"turn_id":turn_id}}),NOW).unwrap();
    let answer=board["items"].as_array().unwrap().iter().find(|item|item["target"]["kind"]=="answer").unwrap();
    assert_eq!(answer["target"]["question"],original.user);
    assert_eq!(answer["target"]["answer_parts"],json!(original.outcome.as_ref().unwrap().answer_view.as_ref().unwrap().parts));
    let presentation=board["items"].as_array().unwrap().iter().find(|item|item["target"]["kind"]=="presentation").unwrap();
    assert_eq!(presentation["target"]["source_book_id"],original_book);
    state.user.agent_history.sessions.clear();
    let (_,_,_,version,_)=board_api::retained(&state.user,&state.workspace,&json!({"board_id":board["board_id"],"item_id":presentation["item_id"]})).unwrap();
    assert_eq!(version.owner.book_id,original_book);
}
