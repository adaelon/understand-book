use super::*;
use std::cell::RefCell;

const NOW: &str = "2026-10-27T04:00:00Z";

fn setup() -> (tempfile::TempDir, AppState, crate::agent_run::PreparedAgentChat, Value, Vec<SourceBinding>) {
    let (root, mut state, metadata) = crate::tests::board_browser_host::fixture();
    let bindings = state.user.agent_history.sessions.iter().find(|session| session.id == metadata["session_id"])
        .unwrap().turns.iter().find(|turn| turn.turn_id == metadata["turn_id"]).unwrap().source_bindings.clone();
    let board = board_api::route(&state.user, &state.workspace, "create", &json!({"title":"候选修订验收"}), NOW).unwrap();
    let prepared = prepare_agent_chat(&mut state, &json!({"message":"整理上下文状态，并保留已经画出的关系。",
        "client_request_id":"board-revise", "session_id":metadata["session_id"], "expected_chat_head":metadata["turn_id"],
        "board_follow_up":{"board_id":board["board_id"],"board_revision":board["revision"]}}).to_string(), NOW)
        .unwrap_or_else(|reply| panic!("{}", reply.body));
    (root, state, prepared, metadata, bindings)
}

fn drawing(item_id: &str, text: &str) -> Value {
    json!({"item_id":item_id,"title":text,"position":{"x":480,"y":80},"size":{"width":240,"height":120},
        "graphic":{"kind":"shape","shape":"rectangle","text":text,"color":"#177e77"},"based_on":[]})
}

fn prepare_input(board: &BoardDocument, reference: &AgentTurnRef, source: &Value) -> Value {
    let explanation = json!({"item_id":reserved_answer_item_id(&reference.turn_id),"block_id":"why","content_revision":board.revision+1});
    json!({"operation":"prepare","base_board_revision":board.revision,
        "blocks":[{"block_id":"why","markdown":"先看两段的用时。\n\n再看总时间。 [[source:source-wb1]]"},
            {"block_id":"result","markdown":"保留关系，再解释它代表的计算。"}],
        "drawings":[drawing("context","上下文"),drawing("state","固定大小状态")],
        "imports":[{"source":source,"item_id":"original-answer","position":{"x":1100,"y":80},"size":{"width":320,"height":240}}],
        "links":[{"link_id":"context-state","from":{"kind":"item","item_id":"context"},"to":{"kind":"item","item_id":"state"},"label":"更新","based_on":[]}],
        "mappings":[{"mapping_id":"why-state","explanation":explanation,"visual":{"kind":"item","item_id":"state"}}],
        "routes":[{"route_id":"explain","title":"理解状态","steps":[{"step_id":"why","explanation":explanation,
            "reveal_targets":[{"kind":"item","item_id":"state"}],"focus_targets":[{"kind":"link","link_id":"context-state"}],"transition":"draw","duration_ms":800}]}]})
}

#[test]
fn revise_one_block_preserves_full_graph_sources_and_durable_candidate_then_commit() {
    let (_root, mut state, prepared, metadata, bindings) = setup();
    let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
    let storage = LocalStorage { port: &port, scope: &prepared.scope };
    let frozen = prepared.scope.board_context.as_ref().unwrap();
    let token = CancellationToken::default();
    let board = storage.with_user(|user| Ok(read_current(user, frozen)?.1)).unwrap();
    let input = prepare_input(&board, &prepared.turn_ref, &metadata["sources"]["answer"]);
    execute(&storage, frozen, &prepared.turn_ref, serde_json::from_value(input).unwrap(), &bindings, &[], &token).unwrap();
    let before = storage.with_user(|user| {
        let mut candidate = selected_candidate(user, &prepared.turn_ref, 1)?;
        candidate.previewed = true;
        candidate.staged = true;
        candidate.commit_selection = Some(runtime::board_author::SelectedDelivery { teaching_delivery_move_refs:vec!["old-review".into()], delivery_scope:None });
        candidate.commit_requested_at = Some(NOW.into());
        // Existing durable board parts used this key for selection references.
        let mut legacy = serde_json::to_value(&candidate).unwrap();
        legacy["commit_review"] = legacy.as_object_mut().unwrap().remove("commit_selection").unwrap();
        candidate = serde_json::from_value(legacy).unwrap();
        assert_eq!(candidate.commit_selection.as_ref().unwrap().teaching_delivery_move_refs,["old-review"]);
        session_runtime::update_board_authoring(user, &prepared.turn_ref, NOW, candidate.clone())?;
        Ok(candidate)
    }).unwrap();
    let revised = execute(&storage, frozen, &prepared.turn_ref, serde_json::from_value(json!({"operation":"revise","candidate_revision":1,
        "blocks":[{"block_id":"result","markdown":"固定大小状态保留到目前为止的累积结果。"}]})).unwrap(), &[], &[], &token).unwrap();
    assert_eq!(revised.body["candidate_revision"], 2);
    assert!(revised.invalidates_delivery);
    let raw_blocks = execute(&storage, frozen, &prepared.turn_ref, AuthorRequest::Read {
        collection:Some(runtime::board_author::ReadCollection::CandidateBlocks),offset:0,length:Some(20),
        board_revision:Some(board.revision),candidate_revision:Some(2),
    }, &[], &[], &token).unwrap().body;
    assert_eq!(raw_blocks["records"][0]["markdown"], "先看两段的用时。\n\n再看总时间。 [[source:source-wb1]]");
    assert_eq!(raw_blocks["records"][1]["markdown"], "固定大小状态保留到目前为止的累积结果。");
    storage.with_user(|user| {
        user.agent_history = load_agent_history(&user.history_path)?;
        let candidate = selected_candidate(user, &prepared.turn_ref, 2)?;
        assert_eq!(candidate.items.len(), before.items.len());
        assert!(candidate.items.iter().any(|item| matches!(item.target, BoardTarget::Presentation{..})), "imported inline presentation remains independent");
        for old in before.items.iter().skip(1) {
            assert_eq!(candidate.items.iter().find(|item| item.item_id == old.item_id), Some(old));
        }
        assert_eq!(candidate.items[0].explanation_blocks[0], before.items[0].explanation_blocks[0], "unchanged block keeps its compiled source part");
        assert!(!candidate.items[0].explanation_blocks[0].markdown.contains("[[source:"), "BoardCanvas uses the compiled readable block while candidate_blocks exposes its original source markers");
        assert!(candidate.delivery_text.contains("[[source:source-wb1]]"), "recompilation uses raw source-bound blocks");
        assert!(candidate.answer_parts.iter().any(|part| part["kind"] == "sources"));
        assert_eq!(json!(candidate.source_bindings), json!(before.source_bindings));
        assert_eq!(candidate.links, before.links);
        assert_eq!(candidate.mappings, before.mappings);
        assert_eq!(candidate.routes, before.routes);
        assert_eq!(candidate.update_items, before.update_items);
        assert!(!candidate.previewed && !candidate.staged);
        assert!(candidate.commit_selection.is_none() && candidate.commit_requested_at.is_none());
        assert_eq!(read_current(user, frozen)?.1, board, "revision stays private before a fresh preview and commit");
        Ok(())
    }).unwrap();
    assert!(execute(&storage, frozen, &prepared.turn_ref, serde_json::from_value(json!({"operation":"revise","candidate_revision":1,
        "blocks":[{"block_id":"result","markdown":"旧候选不能覆盖新候选"}]})).unwrap(), &[], &[], &token).is_err());
    assert!(execute(&storage, frozen, &prepared.turn_ref, AuthorRequest::Preview {candidate_revision:1, viewport:None}, &[], &[], &token).is_err());
    assert_eq!(execute(&storage, frozen, &prepared.turn_ref, AuthorRequest::Deliver {candidate_revision:2}, &[], &[], &token).err().unwrap().error_code, "BOARD_PREVIEW_REQUIRED");
    storage.with_user(|user| {
        let mut candidate = selected_candidate(user, &prepared.turn_ref, 2)?;
        candidate.previewed = true; // Rendering is exercised by BoardCanvas acceptance; this regression covers persistence and the gate.
        session_runtime::update_board_authoring(user, &prepared.turn_ref, NOW, candidate)
    }).unwrap();
    execute(&storage, frozen, &prepared.turn_ref, AuthorRequest::Commit {candidate_revision:2,
        selected_delivery:Some(runtime::board_author::SelectedDelivery{teaching_delivery_move_refs:vec![],delivery_scope:None})}, &[], &[], &token).unwrap();
    storage.with_user(|user| {
        user.agent_history = load_agent_history(&user.history_path)?;
        let saved = read_current(user, frozen)?.1;
        assert_eq!(saved.links, before.links);
        assert_eq!(saved.mappings, before.mappings);
        assert_eq!(saved.routes, before.routes);
        assert_eq!(saved.items.len(), before.items.len());
        let part = &turn(user, &prepared.turn_ref)?.domain.board_commits[0];
        assert_eq!(part.candidate_revision, 2);
        assert!(part.delivery_text.contains("固定大小状态保留到目前为止的累积结果"));
        assert_eq!(json!(part.source_bindings), json!(bindings));
        Ok(())
    }).unwrap();
    let committed = execute(&storage, frozen, &prepared.turn_ref, serde_json::from_value(json!({"operation":"revise","candidate_revision":2,
        "blocks":[{"block_id":"result","markdown":"不能修改已保存部分"}]})).unwrap(), &[], &[], &token).err().unwrap();
    assert_eq!(committed.error_code, "BOARD_CANDIDATE_COMMITTED");
}

#[test]
fn revise_updates_candidate_and_retained_drawings_and_rejects_bad_changes_atomically() {
    let (_root, mut state, prepared, metadata, bindings) = setup();
    let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
    let storage = LocalStorage { port: &port, scope: &prepared.scope };
    let frozen = prepared.scope.board_context.as_ref().unwrap();
    let token = CancellationToken::default();
    let board = storage.with_user(|user| {
        let (store, mut board) = read_current(user, frozen)?;
        let revision = board.revision;
        board.items.push(BoardItem {item_id:"retained-native".into(),reply_to_item_id:None,block_id:None,explanation_blocks:vec![],title:"旧图形".into(),
            readable_content:"旧图形".into(),position:BoardPosition{x:0.0,y:500.0},size:BoardSize{width:240.0,height:120.0},created_at:NOW.into(),source_bindings:vec![],
            target:BoardTarget::Drawing{graphic:BoardGraphic::Text{text:"旧图形".into()},session_id:"original-chat".into(),turn_id:"original-turn".into(),based_on:vec![]}});
        store.update(board, revision, NOW)
    }).unwrap();
    let mut input = prepare_input(&board, &prepared.turn_ref, &metadata["sources"]["answer"]);
    input["update_drawings"] = json!([drawing("retained-native", "准备中的图形")]);
    execute(&storage, frozen, &prepared.turn_ref, serde_json::from_value(input).unwrap(), &bindings, &[], &token).unwrap();
    let before = storage.with_user(|user| selected_candidate(user, &prepared.turn_ref, 1)).unwrap();
    for request in [
        json!({"operation":"revise","candidate_revision":1}),
        json!({"operation":"revise","candidate_revision":1,"blocks":[{"block_id":"result","markdown":"保留关系，再解释它代表的计算。"}]}),
        json!({"operation":"revise","candidate_revision":1,"drawings":[drawing("state","固定大小状态")]}),
        json!({"operation":"revise","candidate_revision":1,"blocks":[{"block_id":"missing-block","markdown":"新正文"}]}),
        json!({"operation":"revise","candidate_revision":1,"blocks":[{"block_id":"result","markdown":""}]}),
        json!({"operation":"revise","candidate_revision":1,"blocks":[{"block_id":"result","markdown":"未绑定 [[source:unobserved]]"}]}),
        json!({"operation":"revise","candidate_revision":1,"drawings":[drawing("state","先改正确图形"),drawing("missing-drawing","未知图形")]}),
        json!({"operation":"revise","candidate_revision":1,"drawings":[drawing("original-answer","原答案不允许改成图形")]}),
    ] {
        let rejected = execute(&storage, frozen, &prepared.turn_ref, serde_json::from_value(request.clone()).unwrap(), &[], &[], &token).err().unwrap();
        if request["blocks"][0]["markdown"] == "保留关系，再解释它代表的计算。" || request["drawings"][0]["graphic"]["text"] == "固定大小状态" {
            assert_eq!(rejected.error_code,"BOARD_CANDIDATE_UNCHANGED");
        }
        if request["blocks"][0]["block_id"] == "missing-block" { assert!(rejected.message.contains("missing-block")); }
        if request["drawings"][1]["item_id"] == "missing-drawing" { assert!(rejected.message.contains("missing-drawing")); }
        let after = storage.with_user(|user| selected_candidate(user, &prepared.turn_ref, 1)).unwrap();
        assert_eq!(json!(after), json!(before), "rejected revision must leave the complete candidate intact");
    }
    execute(&storage, frozen, &prepared.turn_ref, serde_json::from_value(json!({"operation":"revise","candidate_revision":1,
        "drawings":[drawing("state","新的状态解释"),drawing("retained-native","更新后的已有图形")]})).unwrap(), &[], &[], &token).unwrap();
    storage.with_user(|user| {
        let candidate = selected_candidate(user, &prepared.turn_ref, 2)?;
        assert_eq!(candidate.delivery_text, before.delivery_text, "drawing-only revise keeps exact explanation body");
        assert_eq!(candidate.items.iter().find(|item|item.item_id=="state").unwrap().readable_content,"新的状态解释");
        let retained = candidate.update_items.iter().find(|item|item.item_id=="retained-native").unwrap();
        assert_eq!(retained.readable_content,"更新后的已有图形");
        assert!(matches!(&retained.target,BoardTarget::Drawing{session_id,turn_id,..} if session_id=="original-chat" && turn_id=="original-turn"));
        assert_eq!(candidate.links, before.links);
        assert_eq!(candidate.mappings, before.mappings);
        assert_eq!(candidate.routes, before.routes);
        let (store, mut changed) = read_current(user, frozen)?;
        let revision = changed.revision;
        changed.title = "用户修改画板".into();
        store.update(changed, revision, NOW)?;
        Ok(())
    }).unwrap();
    let conflict = execute(&storage, frozen, &prepared.turn_ref, serde_json::from_value(json!({"operation":"revise","candidate_revision":2,
        "drawings":[drawing("state","不能覆盖外部修改")]})).unwrap(), &[], &[], &token).err().unwrap();
    assert_eq!(conflict.error_code,"BOARD_REVISION_CONFLICT");
    assert_eq!(storage.with_user(|user| selected_candidate(user,&prepared.turn_ref,2)).unwrap().items.iter().find(|item|item.item_id=="state").unwrap().readable_content,"新的状态解释");
}

#[test]
fn old_candidate_history_remains_readable_without_reconstructing_raw_source_blocks() {
    let (_root, mut state, prepared, metadata, bindings) = setup();
    let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
    let storage = LocalStorage { port: &port, scope: &prepared.scope };
    let frozen = prepared.scope.board_context.as_ref().unwrap();
    let token = CancellationToken::default();
    let board = storage.with_user(|user| Ok(read_current(user,frozen)?.1)).unwrap();
    execute(&storage, frozen, &prepared.turn_ref,
        serde_json::from_value(prepare_input(&board,&prepared.turn_ref,&metadata["sources"]["answer"])).unwrap(), &bindings, &[], &token).unwrap();
    let original = storage.with_user(|user| {
        let candidate = selected_candidate(user,&prepared.turn_ref,1)?;
        let mut old_record = json!(candidate);
        old_record.as_object_mut().unwrap().remove("explanation_blocks");
        let legacy: Candidate = serde_json::from_value(old_record).unwrap();
        session_runtime::update_board_authoring(user,&prepared.turn_ref,NOW,legacy)?;
        user.agent_history = load_agent_history(&user.history_path)?;
        Ok(candidate)
    }).unwrap();
    let legacy = storage.with_user(|user| selected_candidate(user,&prepared.turn_ref,1)).unwrap();
    assert_eq!(legacy.delivery_text,original.delivery_text);
    assert_eq!(legacy.answer_parts,original.answer_parts);
    assert_eq!(legacy.items,original.items);
    assert!(legacy.explanation_blocks.is_empty());
    execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(json!({"operation":"revise","candidate_revision":1,
        "drawings":[drawing("state","只改旧候选的图形")]})).unwrap(),&[],&[],&token).unwrap();
    let blocked = execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(json!({"operation":"revise","candidate_revision":2,
        "blocks":[{"block_id":"result","markdown":"不能从显示正文猜测来源和分块"}]})).unwrap(),&[],&[],&token).err().unwrap();
    assert!(blocked.message.contains("original explanation blocks"));
    let saved = storage.with_user(|user| selected_candidate(user,&prepared.turn_ref,2)).unwrap();
    assert_eq!(saved.delivery_text,original.delivery_text);
    assert_eq!(saved.answer_parts,original.answer_parts);
    assert_eq!(saved.items[0],original.items[0]);
}
