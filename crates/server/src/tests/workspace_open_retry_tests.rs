use super::*;

#[test]
fn mu5_open_retry_create_recovers_the_same_resident_without_consuming_capacity() {
    let f = Fixture::new();
    let created = f.create(&f.a, &f.x, "retry-page");
    // Simulate losing the response: the browser resends the original create.
    for _ in 0..6 {
        assert_eq!(f.create(&f.a, &f.x, "retry-page"), created);
    }
    let other_tab = f.create(&f.a, &f.x, "other-page");
    let other_book = f.create(&f.a, &f.y, "retry-page");
    let other_owner = f.create(&f.b, &f.x, "retry-page");
    for other in [&other_tab, &other_book, &other_owner] {
        assert_ne!(other["workspace_id"], created["workspace_id"]);
    }
}

#[test]
fn mu5_open_retry_attach_recovers_only_the_fork_of_the_exact_source() {
    let f = Fixture::new();
    let first = f.create(&f.a, &f.x, "first-page");
    let second = f.create(&f.a, &f.x, "second-page");
    let first = f.action(&f.a, &first, "first-page", "checkpoint", json!({"top_lid":"1.8"}));
    let second = f.action(&f.a, &second, "second-page", "checkpoint", json!({"top_lid":"1.20"}));
    let fork = f.action(&f.a, &first, "retry-page", "attach", json!({}));
    for _ in 0..6 {
        assert_eq!(f.action(&f.a, &first, "retry-page", "attach", json!({})), fork);
    }
    let other_source = f.action(&f.a, &second, "retry-page", "attach", json!({}));
    assert_ne!(other_source["workspace_id"], fork["workspace_id"]);
    assert_ne!(other_source["reader"]["viewport"], fork["reader"]["viewport"]);
    assert_eq!(f.action(&f.a, &second, "retry-page", "attach", json!({})), other_source);
    // Reuse still works when all four resident slots are occupied.
    assert_eq!(f.action(&f.a, &first, "retry-page", "attach", json!({})), fork);
}

#[test]
fn mu5_open_retry_does_not_reuse_an_explicit_fork_as_a_create_or_attach() {
    let f = Fixture::new();
    let source = f.create(&f.a, &f.x, "source-page");
    let explicit = f.action(&f.a, &source, "retry-page", "fork", json!({}));
    let another_explicit = f.action(&f.a, &source, "retry-page", "fork", json!({}));
    assert_ne!(explicit["workspace_id"], another_explicit["workspace_id"]);
    let attached = f.action(&f.a, &source, "retry-page", "attach", json!({}));
    assert_ne!(attached["workspace_id"], explicit["workspace_id"]);
    assert_eq!(f.action(&f.a, &source, "retry-page", "attach", json!({})), attached);
    // A fresh create cannot silently select either kind of fork at capacity.
    let rejected = f.call(&f.a, "POST", "/api/workspaces",
        json!({"published_book_ref":f.x,"attachment_id":"retry-page"}));
    assert_eq!(rejected.1["error_code"], "WORKSPACE_CAPACITY");
}

#[test]
fn mu5_open_retry_still_requires_the_publication_grant() {
    let f = Fixture::new();
    let source = f.create(&f.a, &f.x, "source-page");
    f.action(&f.a, &source, "retry-page", "attach", json!({}));
    f.access.library.lock().unwrap().revoke("A", &f.x).unwrap();
    let create = f.call(&f.a, "POST", "/api/workspaces",
        json!({"published_book_ref":f.x,"attachment_id":"source-page"}));
    assert_eq!(create.0, 404);
    let path = format!("/api/workspaces/{}/attach", source["workspace_id"].as_str().unwrap());
    assert_eq!(f.call(&f.a, "POST", &path, Fixture::stamp(&source, "retry-page")).0, 404);
    assert_eq!(f.call(&f.b, "POST", &path, Fixture::stamp(&source, "retry-page")).0, 404);
}

#[test]
fn mu5_open_retry_matches_learning_scope_and_revalidates_its_membership() {
    let mut f = Fixture::new();
    let scope = f.ok(&f.a, "POST", "/api/learning-workspaces",
        json!({"goal":{"text":"学习阅读材料"},"members":[f.x]}));
    let scope_id = scope["learning_workspace_id"].as_str().unwrap();
    let request = json!({"published_book_ref":f.x,"attachment_id":"source-page","learning_workspace_id":scope_id});
    let source = f.ok(&f.a, "POST", "/api/workspaces", request.clone());
    let unscoped = f.create(&f.a, &f.x, "source-page");
    assert_ne!(unscoped["workspace_id"], source["workspace_id"]);
    assert_eq!(f.ok(&f.a, "POST", "/api/workspaces", request.clone()), source);
    f.action(&f.a, &source, "retry-page", "attach", json!({}));
    f.control.remove_learning_workspace_member("A", scope_id, &f.x, multi_user_host::now()).unwrap();
    assert_eq!(f.call(&f.a, "POST", "/api/workspaces", request).1["error_code"], "WORKSPACE_MATERIAL_OUT_OF_SCOPE");
    let path = format!("/api/workspaces/{}/attach", source["workspace_id"].as_str().unwrap());
    assert_eq!(f.call(&f.a, "POST", &path, Fixture::stamp(&source, "retry-page")).1["error_code"], "WORKSPACE_MATERIAL_OUT_OF_SCOPE");
}
