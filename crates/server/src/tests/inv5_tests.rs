use super::mu5_tests::Fixture;
use crate::{
    auth,
    multi_user_host::{self, Headers, Site},
};
use serde_json::{json, Value};

#[test]
fn inv5_host_returns_resolved_identity_and_search_preserves_private_owner() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    f.control
        .connection
        .execute(
            "UPDATE users SET email='a+tag@example.com',email_verified_at=1 WHERE user_id='A'",
            [],
        )
        .unwrap();
    let w = f.create(&f.a, &f.x, "original");
    for name in ["A", " A+Tag@Example.COM "] {
        let reply = multi_user_host::dispatch(
            &f.access,
            &Site::new("https://reader.example").unwrap(),
            "POST",
            "/api/auth/login",
            &Headers(vec![
                ("Host".into(), "reader.example".into()),
                ("Origin".into(), "https://reader.example".into()),
            ]),
            &json!({"username":name,"password":"fixture-only-password"}).to_string(),
            multi_user_host::now(),
        );
        assert_eq!(reply.status, 200);
        let identity: Value = serde_json::from_slice(&reply.body).unwrap();
        assert_eq!(identity["user_id"], "A");
        assert_eq!(identity["email"], "a+tag@example.com");
        let cookie = reply.cookie.unwrap();
        let token = auth::token_from_cookie(&cookie).unwrap();
        let me = f.ok(token, "GET", "/api/auth/me", json!({}));
        assert_eq!(me["user_id"], "A");
        assert_eq!(me["email"], identity["email"]);
        assert_eq!(me["capabilities"]["admin"], false);
        assert_eq!(f.get(token, &w)["workspace_id"], w["workspace_id"]);
        assert_eq!(f.call(token, "GET", "/api/admin/users", json!({})).0, 403);
        assert_eq!(
            f.ok(token, "GET", "/api/library", json!({}))["books"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
    let me = f.ok(&f.b, "GET", "/api/auth/me", json!({}));
    assert!(me["email"].is_null());
    assert_eq!(me["capabilities"]["admin"], true);
    let results = f.ok(
        &f.b,
        "GET",
        "/api/admin/users?search=TAG%40EXAMPLE.COM",
        json!({}),
    );
    assert_eq!(results["total"], 1);
    assert_eq!(results["users"][0]["user_id"], "A");
    assert_eq!(results["users"][0]["email"], "a+tag@example.com");
    assert_eq!(
        f.ok(&f.b, "GET", "/api/admin/users/A", json!({}))["email"],
        "a+tag@example.com"
    );
    assert!(!f.root.path().join("users/a+tag@example.com").exists());
    assert_ne!(
        f.call(
            &f.b,
            "GET",
            &format!("/api/workspaces/{}", w["workspace_id"].as_str().unwrap()),
            json!({})
        )
        .0,
        200
    );
}
