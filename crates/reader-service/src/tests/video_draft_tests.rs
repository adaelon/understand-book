use super::*;
use crate::video_draft;

pub fn media_learning_state(root: &Path, book: Book) -> AppState {
    let mut state = state_named(&format!("media-learning-{}", uuid::Uuid::now_v7()));
    state.user.store = MemoryStore::open(root.join("memory.json")).unwrap();
    state.user.history_path = Some(root.join("history.json"));
    state.workspace.book = Arc::new(book);
    state.workspace.book_dir = root.into();
    state.workspace.reader = Reader::new(&state.workspace.book, DEFAULT_RADIUS);
    assert_eq!(post(&mut state,"/agent/new","{}").status, 200);
    let mut store = state.user.learning_store().unwrap();
    store.mutate(&memory::learning::TutorMutation { operation_id:"enable-media".into(), expected_revision:0,
        action:memory::learning::TutorAction::SetEnabled {enabled:true} }, "now").unwrap();
    let before = store.state().unwrap();
    store.mutate(&memory::learning::TutorMutation { operation_id:"old-source-stage".into(), expected_revision:before.control.revision,
        action:memory::learning::TutorAction::Start { user_intent:"Learn the previous book".into(), explicit_constraints:vec![],
            material_scope:vec![memory::learning::TutorMaterial {source_id:"previous-book".into(),scope_refs:vec![],role:memory::learning::TutorMaterialRole::Primary}],
            default_teaching_intent:None, learning_purpose:None } }, "now").unwrap();
    state
}

pub fn media_purpose_input(state: &AppState, key: &str) -> Value {
    let head = state.user.agent_history.sessions.iter().find(|s| Some(s.id.as_str()) == state.workspace.selected_chat.as_deref())
        .and_then(|s| s.turns.last()).map(|turn| &turn.turn_id);
    json!({"message":"","entry_action":"learn_with_purpose","learning_purpose":"understand",
        "source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint(),
        "session_id":state.workspace.selected_chat,"expected_chat_head":head,"client_request_id":key})
}

pub fn media_continue_input(state: &AppState, turn: &AgentTurnRef) -> Value {
    json!({"message":"","entry_action":"continue_answer","answer_ref":{"turn_id":turn.turn_id},
        "session_id":state.workspace.selected_chat,"expected_chat_head":turn.turn_id,
        "client_request_id":format!("continue-{}",turn.turn_id)})
}

#[test]
fn video_learning_purpose_admission_freezes_scope_and_continues_original_material() {
    for selected in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let book = super::tutor_tests::video_tutor_foundation(root.path());
        let mut state = media_learning_state(root.path(), book);
        let mut input = media_purpose_input(&state, "video-purpose");
        if selected {
            let mut d = draft(&state.workspace, "video-purpose-draft", 492000.0);
            d["context"]["image"] = question_image();
            video_draft::command(&mut state.workspace, &state.user,
                &json!({"window_id":"learning-purpose","action":"save","draft":d})).unwrap();
            input["video_draft_id"] = json!("video-purpose-draft");
            input["video_window_id"] = json!("learning-purpose");
        }
        let first = prepare_agent_chat(&mut state, &input.to_string(), "first").unwrap_or_else(|r| panic!("{}",r.body));
        assert_eq!(first.tutor.as_ref().unwrap()["status"], "observing");
        let session_id = first.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_owned();
        assert_eq!(first.tutor.as_ref().unwrap()["learning_purpose"],"understand");
        assert_eq!(first.tutor.as_ref().unwrap()["binding"]["source_id"],state.workspace.book.base.book_id);
        assert_eq!(first.scope.reader_input.as_ref().unwrap().video_question.is_some(), selected);
        let saved = state.user.agent_history.sessions.iter().flat_map(|s| &s.turns).find(|t| t.turn_id == first.turn_ref.turn_id).unwrap();
        assert!(saved.domain.interaction.as_ref().unwrap().source_bindings.is_empty(), "a video learning scope must not fabricate observed text evidence");
        assert!(saved.domain.interaction.as_ref().unwrap().accepted_request.contains(if selected {"视频片段"} else {"当前视频"}));
        let store = state.user.learning_store().unwrap();
        assert!(store.coach_turn_dependency(&first.turn_ref.session_id, &first.turn_ref.turn_id).unwrap().is_none_or(|dependency| dependency.job_ref.is_none()));
        drop(store);
        super::tutor_loop_tests::finish(&mut state, &first.turn_ref, &super::tutor_loop_tests::outcome(None));
        assert!(state.user.learning_store().unwrap().coach_job_for_source(&session_id,memory::tutor_coach::CoachJobOrigin::AfterTurn,&first.turn_ref.turn_id).unwrap().is_none(),"a purpose button alone supplies no authored learner expression");
        let next_input = media_continue_input(&state, &first.turn_ref);
        let next = prepare_agent_chat(&mut state, &next_input.to_string(), "next").unwrap_or_else(|r| panic!("{}",r.body));
        assert_eq!(next.tutor.as_ref().unwrap()["binding"]["tutor_session_id"], session_id);
        assert_eq!(json!(next.scope.reader_input.as_ref().unwrap().video_question), json!(first.scope.reader_input.as_ref().unwrap().video_question));
        assert!(state.user.learning_store().unwrap().coach_turn_dependency(&next.turn_ref.session_id, &next.turn_ref.turn_id).unwrap().is_none_or(|dependency| dependency.job_ref.is_none()));
        super::tutor_loop_tests::finish(&mut state, &next.turn_ref, &super::tutor_loop_tests::outcome(None));
        let again_input = media_continue_input(&state, &next.turn_ref);
        let again = prepare_agent_chat(&mut state, &again_input.to_string(), "again").unwrap_or_else(|r| panic!("{}",r.body));
        assert_eq!(again.tutor.as_ref().unwrap()["binding"]["tutor_session_id"], session_id);
        assert_eq!(json!(again.scope.reader_input.as_ref().unwrap().video_question), json!(first.scope.reader_input.as_ref().unwrap().video_question));
        if selected {
            super::tutor_loop_tests::finish(&mut state, &again.turn_ref, &super::tutor_loop_tests::outcome(None));
            let whole_input = media_purpose_input(&state, "video-whole-after-clip");
            let whole = prepare_agent_chat(&mut state, &whole_input.to_string(), "whole").unwrap_or_else(|r| panic!("{}",r.body));
            assert_eq!(whole.tutor.as_ref().unwrap()["binding"]["tutor_session_id"], session_id,
                "changing a clip goal to the whole video must start the requested scope");
        }
    }
}

pub fn fixture(dir: &Path) -> Book {
    std::fs::create_dir_all(dir.join("media")).unwrap();
    let video = json!({"media_id":"v","media_revision":"1"});
    let transcript = json!({"transcript_id":"t","transcript_revision":"1","video":video});
    let location = json!({"video":video,"start_ms":0,"end_ms":510000});
    let base = json!({"book_id":"vi6","lid_nodes":[{"lid":"1","path":[1],"kind":"paragraph","span":{"start":0,"end":8},"children":[]}],"graph_nodes":[],"graph_edges":[]});
    std::fs::write(dir.join("base.json"), base.to_string()).unwrap();
    std::fs::write(dir.join("source.txt"), "A frame.").unwrap();
    std::fs::write(dir.join("media/original.mp4"), b"fixture media").unwrap();
    std::fs::write(dir.join("raw.json"), "{}").unwrap();
    std::fs::write(dir.join("video_source_manifest.json"),json!({"version":"video_source_manifest.v1","material":{"book_id":"vi6","video":video,"transcript":transcript},"source":{"video":video,"imported_path":"media/original.mp4","original_path":"fixture.mp4","duration_ms":510000,"width":1920,"height":1080},"canonical_text_path":"source.txt","transcript_map_path":"transcript_map.json","coverage":[{"capability":"transcript","accepted":[location],"pending":[],"failed":[]}],"previous_book_id":null}).to_string()).unwrap();
    std::fs::write(dir.join("transcript_map.json"),json!({"transcript":transcript,"raw_result_path":"raw.json","machine_transcription":{"engine":"fixture","model":"fixture","language":"en"},"segments":[{"segment_id":"s","text":"A frame.","location":location,"text_range":{"lid":"1","range":{"start":0,"end":8}},"words":[]}]}).to_string()).unwrap();
    std::fs::write(dir.join("publication.json"),json!({"version":"publication.v1","reference":{"book_id":"vi6","publication_id":"original"},"source_fingerprint":"fixture-existing-contract","files":{},"capabilities":{},"artifact_versions":{},"teaching_readiness":{},"build_readiness":{},"resident_bytes":0}).to_string()).unwrap();
    Book::load(dir.to_str().unwrap()).unwrap()
}
pub fn draft(w: &reader_workspace::ReaderWorkspace, id: &str, time: f64) -> Value {
    let (m, _) = w.book.load_video_source(&w.book_dir).unwrap();
    json!({"message":"Explain this code", "context":{"draft_id":id,"publication":video_draft::publication(w).unwrap(),"material":m.material,
        "frame":{"video":m.source.video,"actual_pts_ms":time,"width":1920,"height":1080},"region":null,"range":null}})
}
pub fn question_image() -> Value {
    use base64::Engine;
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes,2,1);
        encoder.set_color(png::ColorType::Rgb);
        encoder.write_header().unwrap().write_image_data(&[255,0,0,0,255,0]).unwrap();
    }
    json!({"png_base64":base64::engine::general_purpose::STANDARD.encode(bytes),"width":2,"height":1,"origin":"upload"})
}

#[test]
fn video_question_image_invalid_upload_preserves_saved_pixels_and_image_only_draft() {
    let root = tempfile::tempdir().unwrap(); let book = fixture(root.path());
    let mut state = state_named("video-question-image");
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    state.workspace.book = Arc::new(book); state.workspace.book_dir = root.path().into();
    let mut d = draft(&state.workspace,"image-only",15466.6875);
    d["message"] = json!(""); d["context"]["image"] = question_image();
    let save = |state: &mut AppState, draft: &Value| video_draft::command(&mut state.workspace,&state.user,&json!({"window_id":"a","action":"save","draft":draft}));
    save(&mut state,&d).unwrap();
    for bad in [json!({"png_base64":"broken","width":2,"height":1,"origin":"upload"}),
        { let mut image=question_image();image["width"]=json!(3);image }] {
        let mut invalid=d.clone(); invalid["context"]["image"]=bad;
        assert_eq!(save(&mut state,&invalid).unwrap_err().error_code,"VIDEO_DRAFT_INVALID");
        assert_eq!(serde_json::to_value(state.workspace.video_drafts["a"].context.image.as_ref().unwrap()).unwrap(),question_image());
    }
}
#[test]
fn video_draft_local_failure_restore_handoff_and_history() {
    let root = tempfile::tempdir().unwrap(); let book = fixture(root.path());
    let mut state = state_named("vi6-local");
    // Memory and the adjacent learning.db must belong to this fixture.
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    state.workspace.book = Arc::new(book); state.workspace.book_dir = root.path().into();
    state.workspace.reader = Reader::new(&state.workspace.book, DEFAULT_RADIUS);
    state.workspace.session_path = Some(root.path().join("session.json"));
    state.workspace.restore_chat(&mut state.user.agent_history, "t0");
    let mut d = draft(&state.workspace, "d1", 492000.0);
    d["context"]["image"] = question_image();
    let command = json!({"window_id":"a","action":"save","draft":d});
    let reply = route(&mut state, Req { method:"POST", url:"/reader/video_draft",body:&command.to_string(),now:"t0" });
    assert_eq!(reply.status,200,"{}",reply.body);
    assert_eq!(video_draft::load(&state.workspace.session_path)["a"].context.frame.actual_pts_ms,492000.0);
    let other = draft(&state.workspace,"d2",507000.0);
    video_draft::command(&mut state.workspace,&state.user,&json!({"window_id":"b","action":"save","draft":other})).unwrap();
    let input = json!({"message":"Explain this code","video_window_id":"a","video_draft_id":"d1"});
    let mut invalid = input.clone(); invalid["video_draft_id"]=json!("missing");
    assert!(prepare_agent_chat(&mut state,&invalid.to_string(),"t1").is_err());
    assert_eq!(state.workspace.video_drafts["a"].context.frame.actual_pts_ms,492000.0);
    let prepared = prepare_agent_chat(&mut state,&input.to_string(),"t2").unwrap_or_else(|r|panic!("{}",r.body));
    assert_eq!(prepared.scope.reader_input.as_ref().unwrap().video_question.as_ref().unwrap().frame.actual_pts_ms,492000.0);
    assert_eq!(prepared.scope.reader_input.as_ref().unwrap().video_question.as_ref().unwrap().publication.publication_id,"original");
    assert_eq!(serde_json::to_value(prepared.scope.reader_input.as_ref().unwrap().video_question.as_ref().unwrap().image.as_ref().unwrap()).unwrap(),question_image());
    let session = state.user.agent_history.sessions.iter().find(|s|s.id==prepared.turn_ref.session_id).unwrap();
    let restored: AgentChatSession = serde_json::from_str(&serde_json::to_string(session).unwrap()).unwrap();
    assert_eq!(turn_view(&prepared.scope.reading_book(),&restored.turns[0]).video_question.as_ref().unwrap().frame.actual_pts_ms,492000.0);
    assert_eq!(restored.turns[0].published_book_ref.as_ref().unwrap().publication_id,"original");
    assert_eq!(serde_json::to_value(turn_view(&prepared.scope.reading_book(),&restored.turns[0]).video_question.unwrap().image.unwrap()).unwrap(),question_image());
    assert!(video_draft::command(&mut state.workspace,&state.user,&json!({"window_id":"a","action":"get"})).unwrap()["draft"].is_null());
    assert!(!video_draft::load(&state.workspace.session_path).contains_key("a"));
    assert_eq!(state.workspace.video_drafts["b"].context.frame.actual_pts_ms,507000.0);
}

#[test]
fn video_draft_range_edits_restore_and_handoff_independent_region() {
    let root = tempfile::tempdir().unwrap(); let book = fixture(root.path());
    let mut state = state_named("vi8b-local");
    // Memory and the adjacent learning.db must belong to this fixture.
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    state.workspace.book = Arc::new(book); state.workspace.book_dir = root.path().into();
    state.workspace.reader = Reader::new(&state.workspace.book, DEFAULT_RADIUS);
    state.workspace.session_path = Some(root.path().join("session.json"));
    state.workspace.restore_chat(&mut state.user.agent_history, "t0");
    let mut d = draft(&state.workspace, "range", 492000.0);
    d["message"] = json!("");
    d["context"]["range"] = json!({"video":d["context"]["material"]["video"],"start_ms":400000,"end_ms":510000});
    let save = |state: &mut AppState, d: &Value| route(state, Req { method:"POST", url:"/reader/video_draft",
        body: &json!({"window_id":"a","action":"save","draft":d}).to_string(), now:"t0" });
    assert_eq!(save(&mut state, &d).status, 200, "range-only is content; it can exceed one read batch");
    let region = json!({"frame":d["context"]["frame"],"rect_normalized":[0.125,0.25,0.5,0.25]});
    d["context"]["region"] = region.clone(); d["context"]["range"]["start_ms"] = json!(410000);
    assert_eq!(save(&mut state, &d).status, 200);
    d["context"]["range"] = Value::Null;
    assert_eq!(save(&mut state, &d).status, 200, "region keeps binding on range cancellation");
    d["context"]["range"] = json!({"video":d["context"]["material"]["video"],"start_ms":0,"end_ms":90000});
    assert_eq!(save(&mut state, &d).status, 200, "range may be independent of focused frame");
    for (start, end) in [(90000, 90000), (90000, 0), (-1, 90000), (0, 510001)] {
        let mut invalid = d.clone(); invalid["context"]["range"]["start_ms"] = json!(start); invalid["context"]["range"]["end_ms"] = json!(end);
        assert_ne!(save(&mut state, &invalid).status, 200);
    }
    let mut invalid = d.clone(); invalid["context"]["range"]["video"]["media_id"] = json!("other");
    assert_ne!(save(&mut state, &invalid).status, 200);
    let restored = video_draft::load(&state.workspace.session_path);
    assert_eq!(restored["a"].context.range.as_ref().unwrap().end_ms, 90000.0);
    assert_eq!(serde_json::to_value(&restored["a"].context.region).unwrap(), region);
    let input = json!({"message":"Explain process","video_window_id":"a","video_draft_id":"range"});
    let prepared = prepare_agent_chat(&mut state, &input.to_string(), "t1").unwrap_or_else(|r| panic!("{}", r.body));
    let context = prepared.scope.reader_input.as_ref().unwrap().video_question.as_ref().unwrap();
    assert_eq!(context.range, restored["a"].context.range); assert_eq!(context.frame.actual_pts_ms, 492000.0);
    assert!(video_draft::load(&state.workspace.session_path).is_empty());
}

#[test]
fn video_draft_region_edit_preserves_binding_and_handoff() {
    let root = tempfile::tempdir().unwrap(); let book = fixture(root.path());
    let mut state = state_named("vi8a-local");
    // Memory and the adjacent learning.db must belong to this fixture.
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    state.workspace.book = Arc::new(book); state.workspace.book_dir = root.path().into();
    state.workspace.reader = Reader::new(&state.workspace.book, DEFAULT_RADIUS);
    state.workspace.session_path = Some(root.path().join("session.json"));
    state.workspace.restore_chat(&mut state.user.agent_history, "t0");
    let mut d = draft(&state.workspace, "selected", 492000.0);
    let save = |state: &mut AppState, d: &Value| route(state, Req { method:"POST", url:"/reader/video_draft",
        body: &json!({"window_id":"a","action":"save","draft":d}).to_string(), now:"t0" });
    assert_eq!(save(&mut state, &d).status, 200);
    let region = json!({"frame":d["context"]["frame"],"rect_normalized":[0.125,0.25,0.5,0.25]});
    d["context"]["region"] = region.clone();
    assert_eq!(save(&mut state, &d).status, 200);
    d["context"]["region"] = Value::Null;
    assert_eq!(save(&mut state, &d).status, 200, "text-only draft keeps its frame");
    d["message"] = json!("");
    assert_ne!(save(&mut state, &d).status, 200, "empty draft must use release");
    d["context"]["region"] = region.clone();
    assert_eq!(save(&mut state, &d).status, 200, "selection-only draft is content");
    let restored = video_draft::load(&state.workspace.session_path);
    assert_eq!(serde_json::to_value(&restored["a"].context.region).unwrap(), region);
    for field in ["draft_id", "publication", "material", "frame"] {
        let mut changed = d.clone();
        match field {
            "draft_id" => changed["context"][field] = json!("new-draft"),
            "publication" => changed["context"][field]["publication_id"] = json!("other"),
            "material" => changed["context"][field]["transcript"]["transcript_revision"] = json!("2"),
            "frame" => {
                changed["context"]["frame"]["actual_pts_ms"] = json!(507000.0);
                changed["context"]["region"]["frame"] = changed["context"]["frame"].clone();
            },
            _ => changed["context"][field] = json!({"video":d["context"]["frame"]["video"],"start_ms":0,"end_ms":1000}),
        }
        assert_ne!(save(&mut state, &changed).status, 200, "must not rebind {field}");
    }
    let mut changed = d.clone(); changed["context"]["region"]["rect_normalized"] = json!([0.9,0.0,0.5,0.5]);
    assert_ne!(save(&mut state, &changed).status, 200);
    let input = json!({"message":"Explain selected code","video_window_id":"a","video_draft_id":"selected"});
    let prepared = prepare_agent_chat(&mut state, &input.to_string(), "t1").unwrap_or_else(|r| panic!("{}", r.body));
    assert_eq!(serde_json::to_value(&prepared.scope.reader_input.as_ref().unwrap().video_question.as_ref().unwrap().region).unwrap(), region);
    assert!(video_draft::load(&state.workspace.session_path).is_empty());
}
