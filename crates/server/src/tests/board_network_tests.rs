use super::*;
use super::mu5_tests::Fixture;

#[test]
fn board_search_scopes_personal_notes_before_ranking_and_limit() {
    let f = Fixture::new();
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let (visible, outside) = {
        let mut user = handle.lock().unwrap();
        let content = |text: String| serde_json::from_value(json!({"content":text,"authorship":"user"})).unwrap();
        let mut outside = String::new();
        for index in 0..101 {
            let note = user.store.create_personal_note(content("entropy".into()), &format!("outside-{index}"), "1").unwrap();
            outside = note.note_id;
        }
        let visible = user.store.create_personal_note(content("待补充的卡片".into()), "visible", "1").unwrap();
        (visible.note_id, outside)
    };
    let first = f.create(&f.a, &f.x, "scoped-search");
    let board = f.action(&f.a, &first, "scoped-search", "boards/create", json!({"title":"范围内卡片"}))["result"].clone();
    let add = |window: &Value, attachment: &str, board: &Value, note_id: &str| {
        f.action(&f.a, window, attachment, "boards/import", json!({
            "board_id":board["board_id"],"board_revision":board["revision"],
            "source":{"kind":"personal_note","reference":{"note_id":note_id}},
        }))["result"].clone()
    };
    let board = add(&first, "scoped-search", &board, &visible);
    let other = f.create(&f.a, &f.y, "outside-search");
    let other_board = f.action(&f.a, &other, "outside-search", "boards/create", json!({"title":"其他资料"}))["result"].clone();
    add(&other, "outside-search", &other_board, &outside);
    {
        let mut user = handle.lock().unwrap();
        user.store.revise_personal_note(&visible, 1, serde_json::from_value(json!({
            "content":format!("entropy {}", "context ".repeat(60)), "authorship":"user",
        })).unwrap(), "visible-revision", "2").unwrap();
        let global = user.store.search_personal_notes("entropy", None, 100).unwrap();
        assert_eq!(global.len(), 100);
        assert!(!global.iter().any(|hit| hit.reference.note_id == visible), "fixture must put the board note beyond the global cutoff");
    }
    let found = f.action(&f.a, &first, "scoped-search", "boards/search", json!({"query":"entropy","limit":1}));
    let hits = found["result"]["items"].as_array().unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["board_id"], board["board_id"]);
    assert_eq!(hits[0]["item_id"], board["items"][0]["item_id"]);
    assert!(hits[0]["snippet"].as_str().unwrap().contains("entropy"));

    let scope = f.ok(&f.a, "POST", "/api/learning-workspaces", json!({"goal":{"text":"跨资料检索"},"members":[f.x,f.y]}))["learning_workspace_id"].clone();
    let open = |reference: &base_schema::PublishedBookRef, attachment: &str| f.ok(&f.a, "POST", "/api/workspaces", json!({
        "published_book_ref":reference,"learning_workspace_id":scope,"attachment_id":attachment,
    }));
    let scoped_first = open(&f.x, "scope-first");
    let scoped_board = f.action(&f.a, &scoped_first, "scope-first", "boards/create", json!({"title":"共同目标"}))["result"].clone();
    let scoped_board = add(&scoped_first, "scope-first", &scoped_board, &visible);
    let scoped_other = open(&f.y, "scope-other");
    let across_books = f.action(&f.a, &scoped_other, "scope-other", "boards/search", json!({"query":"entropy","limit":1}));
    assert_eq!(across_books["result"]["items"][0]["board_id"], scoped_board["board_id"]);
}

#[test]
fn board_library_lists_owned_boards_across_materials_and_workspaces_without_a_reader_stamp() {
    let f = Fixture::new();
    assert_eq!(f.ok(&f.a, "GET", "/api/me/boards", Value::Null)["items"], json!([]));
    let first = f.create(&f.a, &f.x, "board-library-first");
    let unscoped = f.action(&f.a, &first, "board-library-first", "boards/create", json!({"title":"理解第一份资料"}))["result"].clone();
    let learning = f.ok(&f.a, "POST", "/api/learning-workspaces", json!({"goal":{"text":"跨资料理解"},"members":[f.x,f.y]}))["learning_workspace_id"].clone();
    let second = f.ok(&f.a, "POST", "/api/workspaces", json!({"published_book_ref":f.y,"attachment_id":"board-library-second","learning_workspace_id":learning}));
    let scoped = f.action(&f.a, &second, "board-library-second", "boards/create", json!({"title":"比较两个概念"}))["result"].clone();
    let library = f.ok(&f.a, "GET", "/api/me/boards", Value::Null);
    let items = library["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|item| item["board_id"] == unscoped["board_id"] && item["learning_workspace_id"].is_null()));
    assert!(items.iter().any(|item| item["board_id"] == scoped["board_id"] && item["learning_workspace_id"] == learning && item["publication_id"] == f.y.publication_id));
    assert!(items.iter().all(|item| item.get("items").is_none()));
    assert_eq!(f.ok(&f.b, "GET", "/api/me/boards", Value::Null)["items"], json!([]));
}

#[test]
fn mind_map_network_edits_use_board_revision_and_survive_another_window() {
    let f = Fixture::new();
    let first = f.create(&f.a, &f.x, "mind-map-first");
    let created = f.action(&f.a, &first, "mind-map-first", "boards/create", json!({"title":"可编辑导图"}));
    let id = created["result"]["board_id"].clone();
    let edited = f.action(&f.a, &created, "mind-map-first", "boards/edit", json!({
        "board_id":id,"board_revision":1,"action":"创建主题","operations":[
            {"kind":"create_mind_map","map":{"map_id":"map","root_item_id":"root","anchor":{"x":0,"y":0}},"root":{"item_id":"root","title":"Rust"}}
        ]
    }));
    assert_eq!(edited["result"]["revision"], 2);
    let second = f.create(&f.a, &f.x, "mind-map-second");
    let reopened = f.action(&f.a, &second, "mind-map-second", "boards/read", json!({"board_id":id}));
    assert_eq!(reopened["result"]["document"]["mind_maps"][0]["root_item_id"], "root");
    let undo = f.action(&f.a, &reopened, "mind-map-second", "boards/undo", json!({"board_id":id,"board_revision":2}));
    assert!(undo["result"]["items"].as_array().unwrap().is_empty());
    let redo = f.action(&f.a, &undo, "mind-map-second", "boards/redo", json!({"board_id":id,"board_revision":3}));
    assert_eq!(redo["result"]["items"][0]["item_id"], "root");
}

#[test]
fn wb1_network_boards_reopen_across_reader_windows_without_crossing_private_scope() {
    let f = Fixture::new();
    let learning = f.ok(&f.a, "POST", "/api/learning-workspaces", json!({"goal":{"text":"理解概念"},"members":[f.x,f.y]}))["learning_workspace_id"].clone();
    let open = |attachment: &str| f.ok(&f.a, "POST", "/api/workspaces", json!({"published_book_ref":f.x,"attachment_id":attachment,"learning_workspace_id":learning}));
    let first = open("first");
    let created = f.action(&f.a, &first, "first", "boards/create", json!({"title":"同一个知识点"}));
    let board = created["result"].clone();
    assert_eq!(board["learning_workspace_id"], learning);
    assert_ne!(board["board_id"], first["workspace_id"]);
    let second = open("second");
    let read = f.action(&f.a, &second, "second", "boards/read", json!({"board_id":board["board_id"]}));
    assert_eq!(read["result"]["document"], board);
    let changed = f.action(&f.a, &second, "second", "boards/update", json!({"board_id":board["board_id"],"board_revision":1,"title":"重开后继续"}));
    assert_eq!(changed["result"]["revision"], 2);
    let first = f.action(&f.a, &created, "first", "reader/goto", json!({"lid":"1.5"}));
    let mut stale = Fixture::stamp(&first, "first");
    stale["board_id"] = board["board_id"].clone(); stale["board_revision"] = json!(1); stale["title"] = json!("不能覆盖");
    let path = format!("/api/workspaces/{}/boards/update", first["workspace_id"].as_str().unwrap());
    let (status, error) = f.call(&f.a, "POST", &path, stale);
    assert_eq!(status, 409); assert_eq!(error["error_code"], "BOARD_REVISION_CONFLICT");
    let latest = f.action(&f.a, &first, "first", "boards/read", json!({"board_id":board["board_id"]}));
    assert_eq!(latest["result"]["document"]["title"], "重开后继续");
    let another_learning = f.ok(&f.a, "POST", "/api/learning-workspaces", json!({"goal":{"text":"另一目标"},"members":[f.x]}))["learning_workspace_id"].clone();
    let other_scope = f.ok(&f.a, "POST", "/api/workspaces", json!({"published_book_ref":f.x,"attachment_id":"other-scope","learning_workspace_id":another_learning}));
    let other_user = f.create(&f.b, &f.x, "other-user");
    for (token, window, attachment) in [(&f.a, &other_scope, "other-scope"), (&f.b, &other_user, "other-user")] {
        let mut input = Fixture::stamp(window, attachment); input["board_id"] = board["board_id"].clone();
        assert_ne!(f.call(token,"POST",&format!("/api/workspaces/{}/boards/read",window["workspace_id"].as_str().unwrap()),input).0,200);
        assert!(f.action(token,window,attachment,"boards/list",json!({}))["result"]["items"].as_array().unwrap().is_empty());
    }
    f.access.library.lock().unwrap().revoke("A", &f.x).unwrap();
    let mut input = Fixture::stamp(&first,"first"); input["board_id"] = board["board_id"].clone();
    assert_ne!(f.call(&f.a,"POST",&format!("/api/workspaces/{}/boards/read",first["workspace_id"].as_str().unwrap()),input).0,200);
}
