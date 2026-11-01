use super::*;
use super::mu5_tests::Fixture;

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
