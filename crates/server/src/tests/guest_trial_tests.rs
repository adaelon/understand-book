use super::mu5_tests::Fixture;
use crate::{auth, multi_user_host::{self, Headers, HttpReply, Site}};
use rusqlite::params;
use serde_json::{json, Value};

fn setup() -> (Fixture, String) {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    let batch = f.ok(&f.b, "POST", "/api/admin/invite-batches", json!({"operation_id":"guest-fixture","count":2}));
    (f, batch["invites"][0]["code"].as_str().unwrap().to_owned())
}
fn redeem(f: &Fixture, code: &str, previous: Option<&str>) -> HttpReply {
    let mut headers = vec![("Host".into(), "reader.example".into()), ("Origin".into(), "https://reader.example".into())];
    if let Some(token) = previous { headers.push(("Cookie".into(), format!("{}={token}", auth::COOKIE))); }
    multi_user_host::dispatch(&f.access, &Site::new("https://reader.example").unwrap(), "POST", "/api/auth/guest", &Headers(headers),
        &json!({"invite_code":code}).to_string(), multi_user_host::now())
}
fn identity(reply: &HttpReply) -> (String, Value) {
    let value: Value = serde_json::from_slice(&reply.body).unwrap();
    assert_eq!(reply.status, 200, "{value}");
    let token = auth::token_from_cookie(reply.cookie.as_deref().unwrap()).unwrap().to_owned();
    (token, value)
}
fn guest_count(f: &Fixture) -> i64 {
    f.control.connection.query_row("SELECT count(*) FROM users WHERE user_id LIKE 'guest_%'", [], |r| r.get(0)).unwrap()
}

#[test]
fn guest_trial_replaces_inherited_default_grants_with_its_selected_materials() {
    let (f, code) = setup();
    // The deployed trigger can both duplicate a selected grant and inherit a
    // project book or old publication outside the guest's material selection.
    f.control.connection.execute("INSERT INTO default_book_grants VALUES(?,?)", params![f.x.book_id,f.x.publication_id]).unwrap();
    for (book, publication) in [("understandbook", "project"), (f.x.book_id.as_str(), "old")] {
        f.control.connection.execute("INSERT INTO book_publications(book_id,publication_id,directory,manifest) VALUES(?,?,?,'{}')", params![book,publication,format!("/unused/{publication}")]).unwrap();
        f.control.connection.execute("INSERT INTO default_book_grants VALUES(?,?)", params![book,publication]).unwrap();
    }
    let (token, guest) = identity(&redeem(&f, &code, None));
    let books = f.ok(&token, "GET", "/api/library", json!({}));
    assert_eq!(books["books"].as_array().unwrap().len(), 2);
    assert!(books["books"].as_array().unwrap().iter().all(|b| b["is_default"] == true && b["published_book_ref"]["book_id"] != "understandbook"));
    let owner = guest["user_id"].as_str().unwrap();
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM book_grants WHERE owner_user_id=? AND publication_id IN ('project','old')", [owner], |r|r.get::<_,i64>(0)).unwrap(), 0);
    assert_eq!(f.ok(&token, "GET", "/api/account/allowance", json!({}))["current_allowance"]["balance"]["available_micro_cny"], 5_000_000);
}

#[test]
fn guest_trial_opens_only_inviter_default_materials_and_grants_five_yuan_once() {
    let (f, code) = setup();
    // Both an old publication and a privately prepared publication are reachable
    // in the same registry, but must not enter a new guest's demonstration library.
    for (book, publication, owner) in [("understandbook", "project", "B"), ("private-material", "private", "A"), (f.x.book_id.as_str(), "old", "B")] {
        f.control.connection.execute("INSERT INTO book_publications(book_id,publication_id,directory,manifest) VALUES(?,?,?,'{}')", params![book, publication, format!("/unused/{publication}")]).unwrap();
        f.control.connection.execute("INSERT OR IGNORE INTO book_defaults VALUES(?,?)", params![book,publication]).unwrap();
        f.control.connection.execute("INSERT INTO book_grants VALUES(?,?,?)", params![owner,book,publication]).unwrap();
    }
    let (token, guest) = identity(&redeem(&f, &code.to_lowercase(), None));
    assert_eq!(guest["is_guest"], true);
    assert!(guest["email"].is_null());
    let me = f.ok(&token, "GET", "/api/auth/me", json!({}));
    assert_eq!(me["is_guest"], true);
    assert_eq!(me["capabilities"]["admin"], false);
    let books = f.ok(&token, "GET", "/api/library", json!({}));
    assert_eq!(books["books"].as_array().unwrap().len(), 2);
    assert!(books["books"].as_array().unwrap().iter().all(|b| b["is_default"] == true));
    let owner = guest["user_id"].as_str().unwrap();
    let allowance = f.ok(&token, "GET", "/api/account/allowance", json!({}));
    assert_eq!(allowance["status"], "active");
    assert_eq!(allowance["current_allowance"]["balance"]["available_micro_cny"], 5_000_000);
    assert_eq!(allowance["current_allowance"]["balance"]["allowance_basis"]["basis_micro_cny"], 5_000_000);
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM manual_receipts WHERE user_id=?", [owner], |r| r.get::<_,i64>(0)).unwrap(), 0);
    let workspace = f.create(&token, &f.x, "guest-reader");
    f.action(&token, &workspace, "guest-reader", "reader/note", json!({"lid":"1.1","text":"guest note"}));
    assert_eq!(guest_count(&f), 1);
    let (again, same) = identity(&redeem(&f, &code, Some(&token)));
    assert_eq!(token, again);
    assert_eq!(guest, same);
    assert_eq!(f.ok(&again, "GET", "/api/account/allowance", json!({})), allowance);
    let period = allowance["current_allowance"]["period_id"].as_str().unwrap();
    f.control.connection.execute("INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,task_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_cost_micro_cny,provider_cost_status,account_debit_micro_cny,evidence_kind,created_at,settled_at) VALUES('guest-usage','guest-usage',1,?1,?2,'fixture-task','answer','fixture','{}','{}','settled',0,1250000,'confirmed',1250000,'fixture',10,11)", params![owner, period]).unwrap();
    let (again, _) = identity(&redeem(&f, &code, Some(&token)));
    assert_eq!(f.ok(&again, "GET", "/api/account/allowance", json!({}))["current_allowance"]["balance"]["available_micro_cny"], 3_750_000);
    let later = multi_user_host::now() + 13 * 60 * 60;
    assert_eq!(f.access.auth.authenticate(&again, later).unwrap().user_id(), owner);
    assert_eq!(redeem(&f, &code, None).status, 409);
    assert_eq!(redeem(&f, &code, Some(&f.a)).status, 409);
    let batch = f.ok(&f.b, "GET", "/api/admin/invite-batches/guest-fixture", json!({}));
    let second_code = batch["invites"].as_array().unwrap().iter().find(|i| i["state"] == "unused").unwrap()["code"].as_str().unwrap();
    let (second, other) = identity(&redeem(&f, second_code, None));
    assert_ne!(guest["user_id"], other["user_id"]);
    assert_eq!(f.call(&second, "GET", &format!("/api/workspaces/{}",workspace["workspace_id"].as_str().unwrap()), json!({})).0, 404);
    assert_eq!(f.ok(&second, "GET", "/api/account/allowance", json!({}))["current_allowance"]["balance"]["available_micro_cny"], 5_000_000);
    assert_eq!(guest_count(&f), 2);
    f.ok(&token, "POST", "/api/auth/logout", json!({}));
    assert_eq!(redeem(&f, &code, Some(&token)).status, 409);
}

#[test]
fn guest_trial_rolls_back_code_account_grant_and_session_on_failure() {
    let (f, code) = setup();
    f.control.connection.execute_batch("CREATE TRIGGER fail_guest_material BEFORE INSERT ON book_grants WHEN NEW.owner_user_id LIKE 'guest_%' BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert_eq!(redeem(&f, &code, None).status, 503);
    assert_eq!(guest_count(&f), 0);
    for table in ["allowance_periods", "allowance_adjustments", "admin_operations"] {
        assert_eq!(f.control.connection.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get::<_,i64>(0)).unwrap(), 0);
    }
    f.control.connection.execute_batch("DROP TRIGGER fail_guest_material").unwrap();
    identity(&redeem(&f, &code, None));
    assert_eq!(guest_count(&f), 1);
}

#[test]
fn guest_trial_missing_materials_keeps_code_unused_and_disabled_codes_grant_nothing() {
    let (f, code) = setup();
    f.control.connection.execute("DELETE FROM book_grants WHERE owner_user_id='B'", []).unwrap();
    let reply = redeem(&f, &code, None);
    assert_eq!(reply.status, 503);
    assert_eq!(serde_json::from_slice::<Value>(&reply.body).unwrap()["error_code"], "GUEST_MATERIALS_UNAVAILABLE");
    f.control.connection.execute("INSERT INTO book_grants VALUES('B',?,?)", params![f.x.book_id,f.x.publication_id]).unwrap();
    identity(&redeem(&f, &code, None));
    let batch = f.ok(&f.b, "GET", "/api/admin/invite-batches/guest-fixture", json!({}));
    let unused = batch["invites"].as_array().unwrap().iter().find(|i| i["state"] == "unused").unwrap();
    f.ok(&f.b, "POST", &format!("/api/admin/invites/{}/disable", unused["invite_id"].as_str().unwrap()), json!({}));
    assert_eq!(redeem(&f, unused["code"].as_str().unwrap(), None).status, 409);
    assert_eq!(redeem(&f, "not-a-code", None).status, 400);
    assert_eq!(guest_count(&f), 1);
}

#[test]
fn guest_trial_simultaneous_redemption_creates_one_account_and_one_grant() {
    let (f, code) = setup();
    let statuses: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2).map(|_| {
            let access = f.access.clone(); let code = code.clone();
            scope.spawn(move || multi_user_host::dispatch(&access, &Site::new("https://reader.example").unwrap(), "POST", "/api/auth/guest",
                &Headers(vec![("Host".into(),"reader.example".into()),("Origin".into(),"https://reader.example".into())]),
                &json!({"invite_code":code}).to_string(), multi_user_host::now()).status)
        }).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(statuses.iter().filter(|&&s| s == 200).count(), 1);
    assert_eq!(statuses.iter().filter(|&&s| s == 409).count(), 1);
    assert_eq!(guest_count(&f), 1);
    assert_eq!(f.control.connection.query_row("SELECT sum(delta_micro_cny) FROM allowance_adjustments", [], |r| r.get::<_,i64>(0)).unwrap(), 5_000_000);
}

#[test]
#[ignore = "real HTTP fixture for guest-trial.spec.ts; no model calls"]
fn guest_trial_browser_host() {
    let (f, code) = setup();
    let batch = f.ok(&f.b, "GET", "/api/admin/invite-batches/guest-fixture", json!({}));
    let second = batch["invites"][1]["code"].as_str().unwrap();
    let server = multi_user_host::start_with_access("127.0.0.1:4196".parse().unwrap(),
        std::sync::Arc::new(Site::new("https://reader.example").unwrap()), f.access.clone()).unwrap();
    let control = tiny_http::Server::http("127.0.0.1:4197").unwrap();
    println!("GUEST_TRIAL_BROWSER_READY");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
    while std::time::Instant::now() < deadline {
        let Some(request) = control.recv_timeout(std::time::Duration::from_secs(1)).unwrap() else { continue; };
        if request.url() == "/stop" { request.respond(tiny_http::Response::from_string("stopped")).unwrap(); break; }
        request.respond(tiny_http::Response::from_string(json!({"code":code,"second_code":second}).to_string())
            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())).unwrap();
    }
    server.shutdown();
}
