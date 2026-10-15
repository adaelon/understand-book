use super::*;

#[test]
fn focus_visible_position_rest_persists_exact_range_and_rejects_invalid_without_mutation() {
    let mut state = state_named("fp5-visible-position");
    let dir = tempfile::tempdir().unwrap();
    state.workspace.session_path = Some(dir.path().join("session.json"));
    let lid = state.workspace.reader.viewport().top_lid;
    let text = state.workspace.book.text(&lid, None).unwrap();
    let first = text.chars().next().unwrap().to_string();
    let position = json!({"lid":lid,"text_position":{"start":0,"end":first.encode_utf16().count(),"text":first},
        "ranges":[{"lid":lid,"start":0,"end":text.encode_utf16().count()}]});
    let reply = post(&mut state, "/reader/position", &position.to_string());
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert_eq!(state.workspace.reader.viewport().visible_lids, vec![lid.clone()]);
    let saved = load_session(&state.workspace.session_path).unwrap();
    let stored = saved.visible_position_for_dir(&path_string(&state.workspace.book_dir)).unwrap();
    assert_eq!(serde_json::to_value(stored).unwrap(), position);
    let mut restarted = Reader::new(&state.workspace.book, DEFAULT_RADIUS);
    restarted.restore_visible_position(&state.workspace.book, stored);
    assert_eq!(reader_state_response(&state.workspace.book, &restarted)["visible_position"], position);
    let revision = state.workspace.reader.revision();
    let mut invalid = position.clone(); invalid["ranges"][0]["end"] = json!(text.encode_utf16().count() + 1);
    assert_eq!(post(&mut state, "/reader/position", &invalid.to_string()).status, 400);
    assert_eq!(state.workspace.reader.revision(), revision);
    assert_eq!(reader_state_response(&state.workspace.book, &state.workspace.reader)["visible_position"], position);
}

#[test]
fn focus_visible_position_network_checkpoint_round_trip() {
    let f = mu5_tests::Fixture::new();
    let w = f.create(&f.a, &f.x, "fp5");
    let position = json!({"lid":"1.29","ranges":[{"lid":"1.29","start":2,"end":6}]});
    let moved = f.action(&f.a, &w, "fp5", "reader/position", position.clone());
    assert_eq!(moved["reader"]["visible_position"], position);
    assert_eq!(moved["reader"]["viewport"]["visible_lids"], json!(["1.29"]));
    assert_eq!(moved["reader"]["layout"], w["reader"]["layout"]);
    let restored = f.get(&f.a, &moved);
    assert_eq!(restored["reader"]["visible_position"], position);
}
