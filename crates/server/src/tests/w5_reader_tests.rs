use super::*;
use super::mu5_tests::Fixture;

#[test]
fn w5_workspace_material_positions_restore_without_crossing_scope_after_reopen() {
    let f = Fixture::new();
    let a = f.ok(&f.a, "POST", "/api/learning-workspaces", json!({
        "goal":{"text":"学习深度学习"}, "members":[f.x,f.y]
    }))["learning_workspace_id"].clone();
    let b = f.ok(&f.a, "POST", "/api/learning-workspaces", json!({
        "goal":{"text":"期末考试冲刺"}, "members":[f.x]
    }))["learning_workspace_id"].clone();
    let w = f.ok(&f.a, "POST", "/api/workspaces", json!({
        "published_book_ref":f.x,"attachment_id":"page","learning_workspace_id":a
    }));
    assert_eq!(w["learning_workspace_goal"]["text"], "学习深度学习");
    let empty = f.action(&f.a, &w, "page", "chat/history", json!({}))["result"].clone();
    assert_eq!(empty["current"]["learning_workspace_id"], a);
    assert_eq!(empty["active_session_id"], "");
    assert!(empty["current"]["turns"].as_array().unwrap().is_empty());
    let w = f.action(&f.a, &w, "page", "chat/new", json!({}));
    let a_chat = w["selected_chat"].clone();
    let w = f.action(&f.a, &w, "page", "reader/goto", json!({"lid":"1.20"}));
    let w = f.action(&f.a, &w, "page", "book/open", json!({"published_book_ref":f.y}));
    assert_eq!(w["selected_chat"], a_chat);
    let w = f.action(&f.a, &w, "page", "reader/goto", json!({"lid":"1.10"}));
    let w = f.action(&f.a, &w, "page", "book/open", json!({"published_book_ref":f.x,"learning_workspace_id":b}));
    assert!(w["reader"]["selection"].is_null(), "A's same-book selection must not become B's initial position");
    assert_eq!(w["learning_workspace_goal"]["text"], "期末考试冲刺");
    let empty = f.action(&f.a, &w, "page", "chat/history", json!({}))["result"].clone();
    assert_eq!(empty["current"]["learning_workspace_id"], b);
    let w = f.action(&f.a, &w, "page", "reader/goto", json!({"lid":"1.5"}));
    assert_eq!(w["learning_workspace_positions"][a.as_str().unwrap()], json!(f.y));
    assert_eq!(w["learning_workspace_positions"][b.as_str().unwrap()], json!(f.x));
    let w = f.action(&f.a, &w, "page", "book/open", json!({"published_book_ref":f.y,"learning_workspace_id":a}));
    assert_eq!(w["selected_chat"], a_chat);
    assert_eq!(w["reader"]["selection"], "1.10");
    let w = f.action(&f.a, &w, "page", "book/open", json!({"published_book_ref":f.x}));
    assert_eq!(w["reader"]["selection"], "1.20");

    // A new scene also must not seed an unseen workspace from another scope.
    let c = f.ok(&f.a, "POST", "/api/learning-workspaces", json!({
        "goal":{"text":"读书笔记"},"members":[f.x]
    }))["learning_workspace_id"].clone();
    let fresh = f.ok(&f.a, "POST", "/api/workspaces", json!({
        "published_book_ref":f.x,"attachment_id":"fresh","learning_workspace_id":c
    }));
    assert!(fresh["reader"]["selection"].is_null());

    let principal = f.access.auth.authenticate(&f.a, crate::multi_user_host::now()).unwrap();
    let context = f.access.context(principal, crate::multi_user_host::now()).unwrap();
    let user = context.user.lock().unwrap();
    let mut reopened = crate::workspace_registry::WorkspaceRegistry::new(
        crate::control_store::ControlStore::open(f.access.users.lock().unwrap().writer()).unwrap());
    let restored = reopened.scene("A", w["workspace_id"].as_str().unwrap(), &user, &f.access.library).unwrap();
    let scene = restored.lock().unwrap();
    assert_eq!(scene.workspace.reader.checkpoint().selection.as_deref(), Some("1.20"));
    let positions = &scene.workspace.learning_reading_positions;
    assert_eq!(positions.iter().find(|p| p.learning_workspace_id == b.as_str().unwrap()).unwrap().reader.selection.as_deref(), Some("1.5"));
    assert_eq!(positions.iter().find(|p| p.learning_workspace_id == a.as_str().unwrap() && p.published_book_ref == f.y).unwrap().reader.selection.as_deref(), Some("1.10"));
}

#[test]
fn w5_explicit_null_returns_to_legacy_reading_without_adopting_workspace_chat() {
    let f = Fixture::new();
    let a = f.ok(&f.a, "POST", "/api/learning-workspaces", json!({"goal":{"text":"学习"},"members":[f.x]}));
    let w = f.ok(&f.a, "POST", "/api/workspaces", json!({
        "published_book_ref":f.x,"attachment_id":"page","learning_workspace_id":a["learning_workspace_id"]
    }));
    let w = f.action(&f.a, &w, "page", "chat/new", json!({}));
    let w = f.action(&f.a, &w, "page", "book/open", json!({"published_book_ref":f.x,"learning_workspace_id":null}));
    assert!(w["learning_workspace_id"].is_null());
    assert!(w["learning_workspace_goal"].is_null());
    assert!(w["selected_chat"].is_null());
    let legacy = f.ok(&f.a, "POST", "/api/workspaces", json!({"published_book_ref":f.x,"attachment_id":"legacy","learning_workspace_id":null}));
    assert!(legacy["learning_workspace_id"].is_null());
}
