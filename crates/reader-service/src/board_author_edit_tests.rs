use super::*;
use std::cell::RefCell;

const NOW: &str = "2026-10-28T08:00:00Z";

fn setup() -> (tempfile::TempDir, AppState, crate::agent_run::PreparedAgentChat) {
    let (root, mut state, metadata) = crate::tests::board_browser_host::fixture();
    let created = board_api::route(&state.user, &state.workspace, "create", &json!({"title":"可编辑切片导图"}), NOW).unwrap();
    let store = board_api::scoped_store(&state.user,state.workspace.learning_workspace_id.as_deref()).unwrap();
    let board = store.edit(created["board_id"].as_str().unwrap(),created["revision"].as_u64().unwrap() as u32,
        serde_json::from_value(json!([
            {"kind":"create_mind_map","map":{"map_id":"slice","root_item_id":"root","anchor":{"x":500,"y":200}},"root":{"item_id":"root","title":"Rust 切片"}},
            {"kind":"insert_topic","parent_item_id":"root","index":0,"topic":{"item_id":"a","title":"借用权限"}},
            {"kind":"insert_topic","parent_item_id":"root","index":1,"topic":{"item_id":"b","title":"范围"}}
        ])).unwrap(),BoardEditActor::Human,"创建导图",NOW).unwrap();
    let prepared = prepare_agent_chat(&mut state,&json!({"message":"只在借用权限这一支补一个实际例子。",
        "client_request_id":"editable-map","session_id":metadata["session_id"],"expected_chat_head":metadata["turn_id"],
        "board_follow_up":{"board_id":board.board_id,"board_revision":board.revision,"item_id":"a"}}).to_string(),NOW)
        .unwrap_or_else(|reply|panic!("{}",reply.body));
    (root,state,prepared)
}

fn request(board: &BoardDocument, reference: &AgentTurnRef) -> AuthorRequest {
    let explanation = json!({"item_id":reserved_answer_item_id(&reference.turn_id),"block_id":"write","content_revision":board.revision+1});
    serde_json::from_value(json!({"operation":"prepare","base_board_revision":board.revision,
        "blocks":[{"block_id":"write","markdown":"可写切片借用原数组的两个位置，s[0] = 99 会改变原数组第二个元素。"}],"drawings":[],
        "read_item_ids":["a"],"operations":[{"kind":"insert_topic","parent_item_id":"a","index":0,
            "topic":{"item_id":"write-example","title":"s[0] = 99 改写原数组","based_on":[explanation]}}],
        "mappings":[{"mapping_id":"explain-write","explanation":explanation,"visual":{"kind":"item","item_id":"write-example"}}],
        "routes":[{"route_id":"write-route","title":"看同一块数组","steps":[{"step_id":"write-step","explanation":explanation,
            "reveal_targets":[{"kind":"topic_edge","child_item_id":"write-example"}],"focus_targets":[{"kind":"item","item_id":"write-example"}],"transition":"draw","duration_ms":800}]}]})).unwrap()
}

fn read() -> AuthorRequest { AuthorRequest::Read {collection:None,offset:0,length:None,board_revision:None,candidate_revision:None} }
fn review() -> runtime::board_author::SelectedDelivery { runtime::board_author::SelectedDelivery {teaching_delivery_move_refs:vec![],delivery_scope:None} }

#[test]
fn new_topic_source_requires_exact_reserved_answer_id_and_accepts_the_reported_repair() {
    let (_root,mut state,prepared)=setup();
    let port=crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
    let storage=LocalStorage{port:&port,scope:&prepared.scope};
    let frozen=prepared.scope.board_context.as_ref().unwrap();
    let token=CancellationToken::default();
    let observed=execute(&storage,frozen,&prepared.turn_ref,read(),&[],&[],&token).unwrap();
    let board=storage.with_user(|user|Ok(read_current(user,frozen)?.1)).unwrap();
    let exact=observed.body["reserved_answer_item_id"].as_str().unwrap();
    let mut input=serde_json::to_value(request(&board,&prepared.turn_ref)).unwrap();
    let reference=&mut input["operations"][0]["topic"]["based_on"][0];
    reference["item_id"]=json!(exact.replace("turn_","turn-"));
    assert_ne!(reference["item_id"],exact);
    let rejected=execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(input.clone()).unwrap(),&[],&[],&token).err().unwrap();
    assert_eq!(rejected.error_code,"INVALID_BOARD");
    assert!(rejected.message.contains(exact) && rejected.message.contains("Available in the proposed board"));
    assert_eq!(storage.with_user(|user|Ok(read_current(user,frozen)?.1)).unwrap(),board,"bad identity must never publish a partial edit");
    input["operations"][0]["topic"]["based_on"][0]["item_id"]=json!(exact);
    input["operations"][0]["topic"]["based_on"][0]["content_revision"]=observed.body["proposed_content_revision"].clone();
    execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(input).unwrap(),&[],&[],&token).unwrap();
    storage.with_user(|user| {
        let (store,current)=read_current(user,frozen)?;
        let candidate=selected_candidate(user,&prepared.turn_ref,1)?;
        let proposed=validate_current(&store,&current,&candidate)?;
        assert!(proposed.items.iter().any(|item|item.item_id=="write-example"));
        assert!(!candidate.previewed && candidate.commit_selection.is_none(),"correcting the address does not bypass preview or review");
        Ok(())
    }).unwrap();
}

#[test]
fn prepare_can_reposition_its_reserved_answer_after_overlap_without_bypassing_preview() {
    let (_root, mut state, prepared) = setup();
    let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
    let storage = LocalStorage {port:&port,scope:&prepared.scope};
    let frozen = prepared.scope.board_context.as_ref().unwrap();
    let token = CancellationToken::default();
    let board = storage.with_user(|user|Ok(read_current(user,frozen)?.1)).unwrap();
    let original_request = request(&board, &prepared.turn_ref);
    let mut repair = serde_json::to_value(&original_request).unwrap();
    execute(&storage,frozen,&prepared.turn_ref,original_request,&[],&[],&token).unwrap();
    let old = storage.with_user(|user|Ok(selected_candidate(user,&prepared.turn_ref,1)?)).unwrap();
    // Exact repair attempted in the failed Rust replay after the real preview
    // reported overlap between two left-hand topics and the reserved answer.
    repair["layout"] = json!([{"item_id":reserved_answer_item_id(&prepared.turn_ref.turn_id),
        "position":{"x":40,"y":700},"size":{"width":340,"height":196}}]);
    let result = execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(repair).unwrap(),&[],&[],&token).unwrap();
    assert_eq!(result.body["candidate_revision"],2);
    assert!(result.invalidates_delivery);
    storage.with_user(|user| {
        let candidate = selected_candidate(user,&prepared.turn_ref,2)?;
        assert_eq!(candidate.operations,old.operations);
        assert_eq!(candidate.delivery_text,old.delivery_text);
        assert_eq!(candidate.items[0].item_id,old.items[0].item_id);
        assert_eq!(candidate.items[0].position,BoardPosition{x:40.0,y:700.0});
        assert_eq!(candidate.items[0].size,BoardSize{width:340.0,height:196.0});
        assert!(candidate.update_items.is_empty(),"new answer geometry is not a retained-item mutation");
        assert!(!candidate.previewed && !candidate.staged && candidate.commit_selection.is_none());
        let current = read_current(user,frozen)?.1;
        assert_eq!(current,board,"repair stays private until the fresh preview and commit");
        let projected = project(&current,&candidate)?;
        assert_eq!(projected.items.iter().find(|item|item.item_id==candidate.items[0].item_id).unwrap().position,candidate.items[0].position);
        Ok(())
    }).unwrap();
    let blocked = execute(&storage,frozen,&prepared.turn_ref,AuthorRequest::Commit{candidate_revision:2,selected_delivery:Some(review())},&[],&[],&token).err().unwrap();
    assert_eq!(blocked.error_code,"BOARD_PREVIEW_REQUIRED");
}

#[test]
fn unrelated_human_edit_rebases_exact_operations_and_requires_a_fresh_preview_then_undo_keeps_human_work() {
    let (_root,mut state,prepared) = setup();
    let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
    let storage = LocalStorage {port:&port,scope:&prepared.scope};
    let frozen = prepared.scope.board_context.as_ref().unwrap();
    let token = CancellationToken::default();
    let board = storage.with_user(|user|Ok(read_current(user,frozen)?.1)).unwrap();
    execute(&storage,frozen,&prepared.turn_ref,request(&board,&prepared.turn_ref),&[],&[],&token).unwrap();
    let old = storage.with_user(|user| {
        let mut candidate = selected_candidate(user,&prepared.turn_ref,1)?;
        candidate.previewed=true; candidate.staged=true; candidate.commit_selection=Some(review()); candidate.commit_requested_at=Some(NOW.into());
        session_runtime::update_board_authoring(user,&prepared.turn_ref,NOW,candidate.clone())?;
        let (store,board)=read_current(user,frozen)?;
        store.edit(&board.board_id,board.revision,vec![BoardEditOperation::UpdateTopic {item_id:"b".into(),title:Some("我补充的范围说明".into()),style:None}],BoardEditActor::Human,"人工改名",NOW)?;
        Ok(candidate)
    }).unwrap();
    let observed=execute(&storage,frozen,&prepared.turn_ref,read(),&[],&[],&token).unwrap();
    assert!(observed.invalidates_delivery);
    assert_eq!(observed.body["candidate_change"],"rebased");
    assert_eq!(observed.body["candidate"]["candidate_revision"],2);
    assert!(observed.body["candidate"].get("edit_base").is_none(),"private semantic baseline is not duplicated in model context");
    let unpreviewed=execute(&storage,frozen,&prepared.turn_ref,AuthorRequest::Commit {candidate_revision:2,selected_delivery:Some(review())},&[],&[],&token).err().unwrap();
    assert_eq!(unpreviewed.error_code,"BOARD_PREVIEW_REQUIRED");
    storage.with_user(|user| {
        user.agent_history=load_agent_history(&user.history_path)?;
        let mut candidate=selected_candidate(user,&prepared.turn_ref,2)?;
        assert_eq!(candidate.delivery_text,old.delivery_text);
        assert_eq!(candidate.items[0].item_id,old.items[0].item_id);
        assert!(!candidate.previewed && !candidate.staged && candidate.commit_selection.is_none());
        assert!(candidate.commit_requested_at.is_none());
        assert_eq!(candidate.mappings[0].explanation.content_revision,candidate.base_board_revision+1);
        let BoardEditOperation::InsertTopic {topic,..}=&candidate.operations[0] else {panic!()};
        assert_eq!(topic.item_id,"write-example");
        assert_eq!(topic.based_on[0].content_revision,candidate.base_board_revision+1);
        let board=read_current(user,frozen)?.1;
        assert!(!board.items.iter().any(|item|item.item_id=="write-example"));
        // Actual browser rendering has separate acceptance coverage; this test exercises the durable save boundary.
        candidate.previewed=true;
        session_runtime::update_board_authoring(user,&prepared.turn_ref,NOW,candidate)
    }).unwrap();
    execute(&storage,frozen,&prepared.turn_ref,AuthorRequest::Commit {candidate_revision:2,selected_delivery:Some(review())},&[],&[],&token).unwrap();
    storage.with_user(|user| {
        let (store,saved)=read_current(user,frozen)?;
        assert!(matches!(&saved.items.iter().find(|item|item.item_id=="write-example").unwrap().target,BoardTarget::Topic {..}));
        assert_eq!(saved.edit_history.transactions.last().unwrap().actor,BoardEditActor::Ai);
        let undone=store.undo(&saved.board_id,saved.revision,NOW)?;
        assert!(!undone.items.iter().any(|item|item.item_id=="write-example" || item.item_id==old.items[0].item_id));
        assert_eq!(undone.items.iter().find(|item|item.item_id=="b").unwrap().title,"我补充的范围说明");
        assert_eq!(undone.deliveries.len(),1);
        assert!(!undone.mappings.iter().any(|mapping|mapping.mapping_id=="explain-write"));
        Ok(())
    }).unwrap();
    let retried=execute(&storage,frozen,&prepared.turn_ref,AuthorRequest::Commit {candidate_revision:2,selected_delivery:Some(review())},&[],&[],&token).unwrap();
    assert_eq!(retried.body["already_committed"],true);
    assert!(!storage.with_user(|user|Ok(read_current(user,frozen)?.1)).unwrap().items.iter().any(|item|item.item_id=="write-example"),"retry never recreates the undone delivery");
}

#[test]
fn same_topic_conflict_preserves_reader_and_private_suggestion_without_rendering_or_committing() {
    let (_root,mut state,prepared)=setup();
    let port=crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
    let storage=LocalStorage {port:&port,scope:&prepared.scope};
    let frozen=prepared.scope.board_context.as_ref().unwrap();
    let token=CancellationToken::default();
    let board=storage.with_user(|user|Ok(read_current(user,frozen)?.1)).unwrap();
    execute(&storage,frozen,&prepared.turn_ref,request(&board,&prepared.turn_ref),&[],&[],&token).unwrap();
    storage.with_user(|user| {
        let (store,board)=read_current(user,frozen)?;
        store.edit(&board.board_id,board.revision,vec![BoardEditOperation::UpdateTopic {item_id:"a".into(),title:Some("用户刚刚重写了权限".into()),style:None}],BoardEditActor::Human,"改名",NOW)?;
        Ok(())
    }).unwrap();
    let read=execute(&storage,frozen,&prepared.turn_ref,read(),&[],&[],&token).unwrap();
    assert_eq!(read.body["candidate_change"],"pending_changes");
    assert!(read.body["candidate"]["conflict"].is_object());
    let preview=execute(&storage,frozen,&prepared.turn_ref,AuthorRequest::Preview {candidate_revision:1,viewport:None},&[],&[],&token).unwrap();
    assert_eq!(preview.body["status"],"candidate_pending_changes");
    assert!(preview.images.is_empty());
    let error=execute(&storage,frozen,&prepared.turn_ref,AuthorRequest::Commit {candidate_revision:1,selected_delivery:Some(review())},&[],&[],&token).err().unwrap();
    assert_eq!(error.error_code,"BOARD_PREVIEW_REQUIRED");
    let saved=storage.with_user(|user|Ok(read_current(user,frozen)?.1)).unwrap();
    assert_eq!(saved.items.iter().find(|item|item.item_id=="a").unwrap().title,"用户刚刚重写了权限");
    assert!(!saved.items.iter().any(|item|item.item_id=="write-example"));
    assert!(saved.deliveries.is_empty());
    let blocked=execute(&storage,frozen,&prepared.turn_ref,request(&saved,&prepared.turn_ref),&[],&[],&token).err().unwrap();
    assert_eq!(blocked.error_code,"BOARD_EDIT_PENDING_READER","a same-run retry cannot erase the conflict without the reader choosing regeneration");
    // Finishing the operational notice must retain the suggestion without
    // automatically appending either that notice or the rejected AI candidate.
    drop(storage);
    drop(port);
    let mut outcome=state.user.agent_history.sessions.iter().flat_map(|session|&session.turns)
        .find_map(|turn|turn.outcome.clone()).unwrap();
    outcome.answer=Some("本次 AI 改图尚未应用。你的修改和待处理建议已保留，可按当前内容重新生成。".into());
    outcome.answer_view.as_mut().unwrap().parts=vec![AgentAnswerPart::Markdown{text:outcome.answer.clone().unwrap()}];
    outcome.source_bindings.clear(); outcome.effects.clear(); outcome.teaching_delivery_move_refs.clear();
    let messages=state.workspace.messages.clone();
    finalize_agent_turn_completed(&mut state,&prepared.turn_ref,&outcome,&messages,NOW).unwrap();
    let completed=state.user.agent_history.sessions.iter().find(|session|session.id==prepared.turn_ref.session_id).unwrap()
        .turns.iter().find(|turn|turn.turn_id==prepared.turn_ref.turn_id).unwrap();
    assert_eq!(completed.status,AgentAssistantStatus::Completed);
    assert!(completed.domain.board_authoring.as_ref().unwrap().conflict.is_some());
    assert!(completed.board_delivery.is_none() && completed.board_delivery_error.is_none());
    assert!(crate::board_delivery::pending_error(completed).is_none());
    let store=board_api::scoped_store(&state.user,state.workspace.learning_workspace_id.as_deref()).unwrap();
    assert_eq!(store.read(&saved.board_id).unwrap(),saved);
    let conversation=board_api::route(&state.user,&state.workspace,"conversation",&json!({"board_id":saved.board_id}),NOW).unwrap();
    let displayed=conversation["sessions"].as_array().unwrap().iter().flat_map(|session|session["turns"].as_array().unwrap())
        .find(|turn|turn["turn_id"]==prepared.turn_ref.turn_id).unwrap();
    assert!(displayed["pending_board_edit"].is_object(),"the completed notice keeps the actionable suggestion visible");
    let source=json!({"session_id":prepared.turn_ref.session_id,"turn_id":prepared.turn_ref.turn_id,
        "candidate_id":displayed["pending_board_edit"]["candidate_id"],"candidate_revision":1});
    let regenerated=prepare_agent_chat(&mut state,&json!({"message":"基于当前内容完成这次未应用的修改，保留冲突期间的人工编辑。",
        "client_request_id":"regeneration-preserves-human","session_id":prepared.turn_ref.session_id,"expected_chat_head":prepared.turn_ref.turn_id,
        "board_follow_up":{"board_id":saved.board_id,"board_revision":saved.revision,"item_id":"a","regenerate_from":source}}).to_string(),NOW)
        .unwrap_or_else(|reply|panic!("{}",reply.body));
    let frozen=regenerated.scope.board_context.as_ref().unwrap();
    assert!(frozen.regeneration.is_some());
    let frozen_wire=serde_json::to_value(frozen).unwrap();
    let reopened:board_api::FrozenBoardFollowUp=serde_json::from_value(frozen_wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(&reopened).unwrap(),frozen_wire,"boxing private context leaves persisted JSON unchanged");
    let frozen=&reopened;
    let port=crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
    let storage=LocalStorage{port:&port,scope:&regenerated.scope};
    let good=request(&saved,&regenerated.turn_ref);
    let mut bad=serde_json::to_value(&good).unwrap();
    bad["operations"].as_array_mut().unwrap().push(json!({"kind":"update_topic","item_id":"a","title":"借用权限"}));
    let rejected=execute(&storage,frozen,&regenerated.turn_ref,serde_json::from_value(bad).unwrap(),&[],&[],&token).err().unwrap();
    assert_eq!(rejected.error_code,"BOARD_REGENERATION_PROTECTED","the replay's old requested title cannot overwrite the reader on a new run");
    execute(&storage,frozen,&regenerated.turn_ref,good,&[],&[],&token).unwrap();
    storage.with_user(|user| {
        let (store,current)=read_current(user,frozen)?;
        let candidate=selected_candidate(user,&regenerated.turn_ref,1)?;
        let candidate_wire=serde_json::to_value(&candidate).unwrap();
        let mut candidate:Candidate=serde_json::from_value(candidate_wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(&candidate).unwrap(),candidate_wire);
        let projected=validate_current(&store,&current,&candidate)?;
        assert_eq!(projected.items.iter().find(|item|item.item_id=="a").unwrap().title,"用户刚刚重写了权限");
        assert!(projected.items.iter().any(|item|item.item_id=="write-example"));
        candidate.operations.push(BoardEditOperation::UpdateTopic{item_id:"a".into(),title:Some("借用权限".into()),style:None});
        assert_eq!(validate_current(&store,&current,&candidate).unwrap_err().error_code,"BOARD_REGENERATION_PROTECTED","the final transaction projection retains the same protection");
        Ok(())
    }).unwrap();
}

#[test]
fn implicit_topic_context_allows_unrelated_style_but_keeps_explicit_reads_and_style_writes_strict() {
    for case in ["implicit_context", "explicit_read", "style_write", "title_changed"] {
        let (_root,mut state,prepared)=setup();
        let port=crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
        let storage=LocalStorage {port:&port,scope:&prepared.scope};
        let frozen=prepared.scope.board_context.as_ref().unwrap();
        let token=CancellationToken::default();
        let board=storage.with_user(|user|Ok(read_current(user,frozen)?.1)).unwrap();
        let mut input=serde_json::to_value(request(&board,&prepared.turn_ref)).unwrap();
        if case!="explicit_read" { input["read_item_ids"]=json!([]); }
        if case=="style_write" {
            input["operations"].as_array_mut().unwrap().push(json!({"kind":"update_topic","item_id":"a","style":{"color":"#2563eb","bold":false}}));
        }
        execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(input).unwrap(),&[],&[],&token).unwrap();
        let human_style=BoardTopicStyle {color:Some("#15803d".into()),bold:true};
        storage.with_user(|user| {
            let (store,board)=read_current(user,frozen)?;
            store.edit(&board.board_id,board.revision,vec![BoardEditOperation::UpdateTopic {item_id:"a".into(),
                title:(case=="title_changed").then(||"用户刚改的借用权限".into()),style:(case!="title_changed").then(||human_style.clone())}],BoardEditActor::Human,"人工编辑",NOW)?;
            Ok(())
        }).unwrap();
        let observed=execute(&storage,frozen,&prepared.turn_ref,read(),&[],&[],&token).unwrap();
        if case!="implicit_context" {
            assert_eq!(observed.body["candidate_change"],"pending_changes","{case}");
            assert!(observed.body["candidate"]["conflict"].is_object(),"{case}");
            continue;
        }
        assert_eq!(observed.body["candidate_change"],"rebased");
        storage.with_user(|user| {
            let mut candidate=selected_candidate(user,&prepared.turn_ref,2)?;
            assert!(!candidate.read_item_ids.iter().any(|id|id=="a"),"implicit topic context must not become an explicit full-item read dependency");
            assert!(!candidate.previewed && candidate.commit_selection.is_none());
            candidate.previewed=true; // This test covers the transaction boundary; real preview is exercised in acceptance.
            session_runtime::update_board_authoring(user,&prepared.turn_ref,NOW,candidate)
        }).unwrap();
        execute(&storage,frozen,&prepared.turn_ref,AuthorRequest::Commit {candidate_revision:2,selected_delivery:Some(review())},&[],&[],&token).unwrap();
        let saved=storage.with_user(|user|Ok(read_current(user,frozen)?.1)).unwrap();
        let BoardTarget::Topic {children,style,..}=&saved.items.iter().find(|item|item.item_id=="a").unwrap().target else {panic!()};
        assert_eq!(style,&human_style,"the human style survives AI insertion through the real store");
        assert_eq!(children,&["write-example"]);
    }
}

#[test]
fn selected_subtree_rejects_unrelated_semantic_updates_and_topic_coordinates() {
    let (_root,mut state,prepared)=setup();
    let port=crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
    let storage=LocalStorage {port:&port,scope:&prepared.scope};
    let frozen=prepared.scope.board_context.as_ref().unwrap();
    let token=CancellationToken::default();
    let board=storage.with_user(|user|Ok(read_current(user,frozen)?.1)).unwrap();
    let mut outside=serde_json::to_value(request(&board,&prepared.turn_ref)).unwrap();
    outside["operations"][0]=json!({"kind":"update_topic","item_id":"b","title":"越界修改"});
    outside["mappings"]=json!([]);outside["routes"]=json!([]);
    let error=execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(outside).unwrap(),&[],&[],&token).err().unwrap();
    assert_eq!(error.error_code,"BOARD_EDIT_OUTSIDE_SELECTION");
    let mut geometry=serde_json::to_value(request(&board,&prepared.turn_ref)).unwrap();
    geometry["layout"]=json!([{"item_id":"a","position":{"x":200,"y":300},"size":{"width":300,"height":200}}]);
    assert!(execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(geometry).unwrap(),&[],&[],&token).err().unwrap().message.contains("shared layout"));
    assert!(storage.with_user(|user|Ok(turn(user,&prepared.turn_ref)?.domain.board_authoring.is_none())).unwrap());
}

#[test]
fn shared_note_revision_invalidates_candidate_even_when_text_and_board_revision_are_unchanged() {
    use memory::workspace_notes::{WorkspaceNote,WorkspaceNoteAuthor,WorkspaceNoteStore};
    let (_root,mut state,prepared)=setup();
    let port=crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
    let storage=LocalStorage {port:&port,scope:&prepared.scope};
    let frozen=prepared.scope.board_context.as_ref().unwrap();
    let token=CancellationToken::default();
    let board=storage.with_user(|user| {
        let notes=WorkspaceNoteStore::open(user.history_path.as_ref().unwrap().parent().unwrap(),frozen.learning_workspace_id.as_deref(),true)?;
        notes.create(WorkspaceNote {note_id:"permission-note".into(),learning_workspace_id:frozen.learning_workspace_id.clone(),title:"借用笔记".into(),content:"可写借用可以改变元素".into(),
            revision:1,created_at:NOW.into(),updated_at:NOW.into(),author:WorkspaceNoteAuthor::User,updated_by:WorkspaceNoteAuthor::User,origin:None,source_bindings:vec![]})?;
        let (store,mut board)=read_current(user,frozen)?;
        let revision=board.revision;
        board.items.push(BoardItem {item_id:"support-note".into(),reply_to_item_id:None,block_id:None,explanation_blocks:vec![],title:"".into(),readable_content:"".into(),
            position:BoardPosition {x:900.0,y:700.0},size:BoardSize {width:300.0,height:200.0},created_at:NOW.into(),source_bindings:vec![],target:BoardTarget::Note {note_id:"permission-note".into(),missing:false}});
        store.update(board,revision,NOW)
    }).unwrap();
    let mut input=serde_json::to_value(request(&board,&prepared.turn_ref)).unwrap();
    input["read_item_ids"]=json!(["a","support-note"]);
    execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(input).unwrap(),&[],&[],&token).unwrap();
    storage.with_user(|user| {
        let mut candidate=selected_candidate(user,&prepared.turn_ref,1)?;
        assert_eq!(candidate.note_revisions.get("permission-note"),Some(&1));
        candidate.previewed=true;
        session_runtime::update_board_authoring(user,&prepared.turn_ref,NOW,candidate)?;
        let notes=WorkspaceNoteStore::open(user.history_path.as_ref().unwrap().parent().unwrap(),frozen.learning_workspace_id.as_deref(),true)?;
        notes.update("permission-note",1,"借用笔记","可写借用可以改变元素",WorkspaceNoteAuthor::User,NOW)?;
        assert_eq!(read_current(user,frozen)?.1.revision,board.revision);
        Ok(())
    }).unwrap();
    let blocked=execute(&storage,frozen,&prepared.turn_ref,AuthorRequest::Commit {candidate_revision:1,selected_delivery:Some(review())},&[],&[],&token).err().unwrap();
    assert_eq!(blocked.error_code,"BOARD_EDIT_CONFLICT");
    let observed=execute(&storage,frozen,&prepared.turn_ref,read(),&[],&[],&token).unwrap();
    assert_eq!(observed.body["candidate_change"],"pending_changes");
    assert_eq!(observed.body["board"]["revision"],board.revision);
    assert_eq!(observed.body["candidate"]["previewed"],false);
}

#[test]
fn deleted_selected_branch_stays_deleted_and_exposes_a_pending_suggestion() {
    let (_root,mut state,prepared)=setup();
    let port=crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
    let storage=LocalStorage {port:&port,scope:&prepared.scope};
    let frozen=prepared.scope.board_context.as_ref().unwrap();
    let token=CancellationToken::default();
    let board=storage.with_user(|user|Ok(read_current(user,frozen)?.1)).unwrap();
    execute(&storage,frozen,&prepared.turn_ref,request(&board,&prepared.turn_ref),&[],&[],&token).unwrap();
    storage.with_user(|user| {
        let (store,board)=read_current(user,frozen)?;
        store.edit(&board.board_id,board.revision,vec![BoardEditOperation::RemoveSubtree {item_id:"a".into()}],BoardEditActor::Human,"删除分支",NOW)?;
        Ok(())
    }).unwrap();
    let observed=execute(&storage,frozen,&prepared.turn_ref,read(),&[],&[],&token).unwrap();
    assert_eq!(observed.body["candidate_change"],"pending_changes");
    assert!(observed.body["candidate"]["conflict"].is_object());
    let preview=execute(&storage,frozen,&prepared.turn_ref,AuthorRequest::Preview {candidate_revision:1,viewport:None},&[],&[],&token).unwrap();
    assert_eq!(preview.body["status"],"candidate_pending_changes");
    assert!(!observed.body["board"]["items"].as_array().unwrap().iter().any(|item|item["item_id"]=="a" || item["item_id"]=="write-example"));
}

#[test]
fn conversion_waits_for_explicit_confirmation_after_the_chat_finishes_and_preserves_originals() {
    for action in ["accept", "dismiss", "stale"] {
    let (_root,mut state,metadata)=crate::tests::board_browser_host::fixture();
    let mut outcome=state.user.agent_history.sessions.iter().find(|session|session.id==metadata["session_id"])
        .unwrap().turns.iter().find(|turn|turn.turn_id==metadata["turn_id"]).unwrap().outcome.clone().unwrap();
    outcome.answer=Some("重组预览已准备好，请查看后确认。".into());
    outcome.answer_view.as_mut().unwrap().parts=vec![AgentAnswerPart::Markdown {text:outcome.answer.clone().unwrap()}];
    outcome.source_bindings.clear();
    outcome.effects.clear();
    let created=board_api::route(&state.user,&state.workspace,"create",&json!({"title":"旧板书重组"}),NOW).unwrap();
    let original=board_api::route(&state.user,&state.workspace,"import",&json!({"board_id":created["board_id"],"board_revision":created["revision"],"source":metadata["sources"]["prebuild"]}),NOW).unwrap();
    let prepared=prepare_agent_chat(&mut state,&json!({"message":"将选中内容整理成思维导图，保留原材料。","client_request_id":"conversion-test",
        "session_id":metadata["session_id"],"expected_chat_head":metadata["turn_id"],"board_follow_up":{"board_id":original["board_id"],"board_revision":original["revision"],
        "item_id":original["items"][0]["item_id"],"intent":"convert_to_mind_map"}}).to_string(),NOW).unwrap_or_else(|reply|panic!("{}",reply.body));
    let candidate_id;
    {
        let port=crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
        let storage=LocalStorage {port:&port,scope:&prepared.scope};
        let frozen=prepared.scope.board_context.as_ref().unwrap();
        assert!(frozen.requires_conversion_confirmation);
        let token=CancellationToken::default();
        execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(json!({"operation":"prepare","base_board_revision":original["revision"],
            "blocks":[{"block_id":"converted-explanation","markdown":"把总路程与总时间分别列出，再比较它们的比值。"}],"drawings":[],"operations":[
                {"kind":"create_mind_map","map":{"map_id":"converted","root_item_id":"converted-root","anchor":{"x":700,"y":100}},"root":{"item_id":"converted-root","title":"平均速度"}},
                {"kind":"insert_topic","parent_item_id":"converted-root","index":0,"topic":{"item_id":"distance","title":"总路程"}}
            ]})).unwrap(),&[],&[],&token).unwrap();
        candidate_id=storage.with_user(|user| {
            let mut candidate=selected_candidate(user,&prepared.turn_ref,1)?;
            candidate.previewed=true;
            session_runtime::update_board_authoring(user,&prepared.turn_ref,NOW,candidate.clone())?;
            Ok(candidate.candidate_id)
        }).unwrap();
        let waiting=execute(&storage,frozen,&prepared.turn_ref,AuthorRequest::Commit {candidate_revision:1,selected_delivery:Some(review())},&[],&[],&token).unwrap();
        assert_eq!(waiting.body["status"],"candidate_confirmation_required");
        assert!(waiting.committed.is_none());
        assert_eq!(json!(storage.with_user(|user|Ok(read_current(user,frozen)?.1)).unwrap()),original);
    }
    let messages=state.workspace.messages.clone();
    finalize_agent_turn_completed(&mut state,&prepared.turn_ref,&outcome,&messages,NOW).unwrap();
    assert!(crate::board_delivery::pending_error(&state.user.agent_history.sessions.iter().find(|session|session.id==prepared.turn_ref.session_id).unwrap()
        .turns.iter().find(|turn|turn.turn_id==prepared.turn_ref.turn_id).unwrap()).is_none());
    let input=json!({"board_id":original["board_id"],"session_id":prepared.turn_ref.session_id,"turn_id":prepared.turn_ref.turn_id,
        "candidate_id":candidate_id,"candidate_revision":1});
    let preview=candidate_preview(&state.user,&state.workspace,&input).unwrap();
    assert_eq!(preview["document"]["mind_maps"][0]["root_item_id"],"converted-root");
    assert_eq!(preview["document"]["items"][0],original["items"][0]);
    if action != "accept" {
        let store=board_api::scoped_store(&state.user,state.workspace.learning_workspace_id.as_deref()).unwrap();
        if action=="stale" {
            let mut board=store.read(original["board_id"].as_str().unwrap()).unwrap();
            let revision=board.revision;
            board.title="确认预览期间用户重命名了画板".into();
            store.update(board,revision,NOW).unwrap();
            let error=candidate_confirmation(&mut state.user,&state.workspace,&input,true,NOW).err().unwrap();
            assert_eq!(error.error_code,"BOARD_REVISION_CONFLICT");
        }
        let dismissed=candidate_confirmation(&mut state.user,&state.workspace,&input,false,NOW).unwrap();
        assert_eq!(dismissed["status"],"candidate_dismissed");
        crate::board_delivery::ensure(&mut state.user,&prepared.turn_ref,NOW).unwrap();
        let unchanged=store.read(original["board_id"].as_str().unwrap()).unwrap();
        assert!(unchanged.deliveries.is_empty() && unchanged.mind_maps.is_empty());
        assert_eq!(json!(unchanged.items),original["items"]);
        continue;
    }
    let accepted=candidate_confirmation(&mut state.user,&state.workspace,&input,true,NOW).unwrap();
    assert_eq!(accepted["status"],"board_part_committed");
    assert_eq!(accepted["document"]["items"][0],original["items"][0]);
    assert_eq!(accepted["document"]["deliveries"].as_array().unwrap().len(),1);
    let repeated=candidate_confirmation(&mut state.user,&state.workspace,&input,true,NOW).unwrap();
    assert_eq!(repeated["already_committed"],true);
    assert_eq!(accepted["document"],repeated["document"]);
    }
}
