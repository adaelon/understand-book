use super::*;

#[test]
fn board_source_publication_legacy_network_sources_reopen_without_rewriting_saved_board() {
    let f = crate::tests::mu5_tests::Fixture::new();
    let w = f.create(&f.a, &f.x, "legacy-source");
    let created = f.action(&f.a, &w, "legacy-source", "boards/create", json!({"title":"Legacy sources"}));
    let imported = f.action(&f.a, &w, "legacy-source", "boards/import", json!({
        "board_id":created["result"]["board_id"],"board_revision":1,
        "source":{"kind":"prebuild","object_id":"speed","object_revision":1,"map_revision":"v1"}
    }));
    let mut document: BoardDocument = convert(imported["result"].clone()).unwrap();
    document.board_id = "legacy-source-board".into();
    document.revision = 1;
    document.deliveries.clear();
    document.items[0].source_bindings[0].publication_id = None;
    document.items[0].source_bindings[0].source_revision = None;
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let store = scoped_store(&handle.lock().unwrap(), None).unwrap();
    store.create(document.clone()).unwrap();
    let request = json!({"board_id":document.board_id,"item_id":document.items[0].item_id,
        "source_ref_id":document.items[0].source_bindings[0].source_ref_id});
    let resolved = f.action(&f.a, &w, "legacy-source", "boards/source.resolve", request.clone());
    assert_eq!(resolved["result"]["stale"], false);
    assert_eq!(resolved["result"]["can_open_in_reader"], true);
    let read = f.action(&f.a, &w, "legacy-source", "boards/source.read", request.clone());
    assert_eq!(read["result"]["published_book_ref"], json!(f.x));
    assert!(!read["result"]["segments"].as_array().unwrap().is_empty());
    assert_eq!(read["reader"], w["reader"]);
    assert_eq!(read["revision"], w["revision"]);
    assert_eq!(store.read(&document.board_id).unwrap(), document);

    // An explicit original version and a different book must never inherit this board's version.
    for (id, book, publication) in [
        ("explicit-source-board", f.x.book_id.clone(), Some("unavailable-original".to_owned())),
        ("foreign-source-board", "another-book".to_owned(), None),
    ] {
        let mut other = document.clone();
        other.board_id = id.into();
        other.items[0].source_bindings[0].book_id = book;
        other.items[0].source_bindings[0].publication_id = publication;
        store.create(other).unwrap();
        let mut input = json!({"attachment_id":"legacy-source","generation":w["generation"],"expected_revision":w["revision"]});
        input.as_object_mut().unwrap().extend(request.as_object().unwrap().clone());
        input["board_id"] = json!(id);
        let (status, _) = f.call(&f.a, "POST", &format!("/api/workspaces/{}/boards/source.resolve", w["workspace_id"].as_str().unwrap()), input);
        assert_eq!(status, 404);
    }
}
