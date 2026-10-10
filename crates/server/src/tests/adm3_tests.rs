use super::mu5_tests::Fixture;
use serde_json::{json, Value};
fn fixture() -> Fixture {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    f
}
fn post(f: &Fixture, action: &str, body: Value) -> Value {
    f.ok(&f.b, "POST", &format!("/api/admin/users/A/{action}"), body)
}
fn period(f: &Fixture) -> String {
    let now = crate::multi_user_host::now();
    post(
        f,
        "allowance-periods",
        json!({"operation_id":"period","starts_at":now-10,"expires_at":now+1000}),
    )["allowance_period"]["period_id"]
        .as_str()
        .unwrap()
        .into()
}
fn receipt(p: &str) -> Value {
    json!({"operation_id":"receipt","period_id":p,"revision":0,"amount_fen":123,"delta_micro_cny":1000,"paid_at":100,"channel":" wechat ","external_ref":" tx-1 ","note":"paid"})
}
fn balance(f: &Fixture) -> Value {
    f.ok(&f.b, "GET", "/api/admin/users/A", json!({}))["current_allowance"]["balance"].clone()
}
fn count(f: &Fixture, table: &str) -> i64 {
    f.control
        .connection
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

#[test]
fn adm3_receipt_grant_replay_corrections_and_reopen() {
    let f = fixture();
    let p = period(&f);
    let body = receipt(&p);
    let result = post(&f, "receipts", body.clone());
    assert_eq!(result["allowance_period"]["revision"], 1);
    assert_eq!(post(&f, "receipts", body.clone()), result);
    assert_eq!(balance(&f)["available_micro_cny"], 1000);
    let mut conflict = body.clone();
    conflict["amount_fen"] = json!(124);
    assert_eq!(
        f.call(&f.b, "POST", "/api/admin/users/A/receipts", conflict)
            .0,
        409
    );
    post(
        &f,
        "allowance-adjustments",
        json!({"operation_id":"gift","period_id":p,"revision":1,"delta_micro_cny":200,"reason":"gift"}),
    );
    assert_eq!(post(&f, "receipts", body), result); // stale revision replay wins
    post(
        &f,
        "receipt-corrections",
        json!({"operation_id":"correction","period_id":p,"revision":2,"receipt_id":result["receipt_id"],"delta_fen":-123,"delta_micro_cny":-1000,"reason":"wrong registration"}),
    );
    assert_eq!(balance(&f)["available_micro_cny"], 200);
    let receipts = f.ok(&f.b, "GET", "/api/admin/users/A/receipts", json!({}));
    assert_eq!(receipts["items"][0]["amount_fen"], 123);
    assert_eq!(receipts["items"][0]["effective_amount_fen"], 0);
    assert_eq!(count(&f, "manual_receipts"), 1);
    assert_eq!(count(&f, "receipt_corrections"), 1);
    assert_eq!(count(&f, "allowance_adjustments"), 3);
    assert_eq!(
        f.ok(
            &f.b,
            "GET",
            "/api/admin/users/A/receipt-corrections?limit=1",
            json!({})
        )["items"][0]["reason"],
        "wrong registration"
    );
    assert_eq!(
        f.ok(
            &f.b,
            "GET",
            "/api/admin/users/A/allowance-adjustments?limit=1&offset=1",
            json!({})
        )["total"],
        3
    );
    let reopened = crate::admin_store::AdminStore::new(
        crate::control_store::ControlStore::open(f.control.writer.clone()).unwrap(),
    );
    assert_eq!(reopened.operation("B", "receipt").unwrap(), result);
    assert_eq!(
        reopened.allowance_list("A", "receipts", 50, 0).unwrap(),
        receipts
    );
    assert!(!f.root.path().join("users/A").exists());
}

#[test]
fn adm3_schema5_upgrade_keeps_original_receipts_and_failure_is_atomic() {
    use crate::control_store::{ControlStore, ServiceWriter};
    let root = tempfile::tempdir().unwrap();
    let db = rusqlite::Connection::open(root.path().join("control.sqlite")).unwrap();
    db.execute_batch(include_str!("control_schema_v4.sql"))
        .unwrap();
    db.execute_batch("ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0 CHECK(is_admin IN (0,1));")
        .unwrap();
    db.execute_batch(include_str!("../admin_schema.sql"))
        .unwrap();
    db.execute_batch("PRAGMA application_id=1430408533; PRAGMA user_version=5; INSERT INTO users(user_id) VALUES('A'); INSERT INTO admin_operations VALUES('A','receipt','A','receipts','{}','{}',1); INSERT INTO manual_receipts VALUES('r','A','receipt','A',123,1,'wechat','external','original'); CREATE TABLE receipt_corrections(keep TEXT);").unwrap();
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    assert!(ControlStore::open(writer.clone()).is_err());
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        5
    );
    db.execute_batch("DROP TABLE receipt_corrections;").unwrap();
    for _ in 0..2 {
        let store = ControlStore::open(writer.clone()).unwrap();
        assert_eq!(
            store
                .connection
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            crate::control_store::CONTROL_SCHEMA_VERSION
        );
        assert_eq!(
            store
                .connection
                .query_row("SELECT amount_fen FROM manual_receipts", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            123
        );
    }
}

#[test]
fn adm3_conflicts_overlap_ownership_and_occupied_allowance() {
    let f = fixture();
    let p = period(&f);
    post(&f, "receipts", receipt(&p));
    let now = crate::multi_user_host::now();
    let overlap = json!({"operation_id":"overlap","starts_at":now,"expires_at":now+2000});
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            "/api/admin/users/A/allowance-periods",
            overlap
        )
        .0,
        409
    );
    let next = post(
        &f,
        "allowance-periods",
        json!({"operation_id":"next","starts_at":now+1000,"expires_at":now+2000}),
    );
    assert_eq!(
        next["allowance_period"]["balance"]["available_micro_cny"],
        0
    );
    assert_eq!(f.call(&f.b,"POST","/api/admin/users/A/allowance-validity",json!({"operation_id":"overlap-edit","period_id":p,"revision":1,"starts_at":now-10,"expires_at":now+1001})).0,409);
    let mut duplicate = receipt(&p);
    duplicate["operation_id"] = json!("duplicate");
    duplicate["revision"] = json!(1);
    assert_eq!(
        f.call(&f.b, "POST", "/api/admin/users/A/receipts", duplicate)
            .1["error_code"],
        "RECEIPT_EXTERNAL_REF_CONFLICT"
    );
    let gift = json!({"operation_id":"gift","period_id":p,"revision":0,"delta_micro_cny":1,"reason":"test"});
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            "/api/admin/users/A/allowance-adjustments",
            gift.clone()
        )
        .1["error_code"],
        "ALLOWANCE_REVISION_CONFLICT"
    );
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            "/api/admin/users/B/allowance-adjustments",
            gift.clone()
        )
        .0,
        404
    );
    assert_eq!(
        f.call(
            &f.a,
            "POST",
            "/api/admin/users/A/allowance-adjustments",
            gift
        )
        .0,
        403
    );
    for (id, state, reserved, debit, settled, status, cost) in [
        ("active", "sent", 300, None, None, "pending", None),
        ("pending", "pending", 200, None, None, "pending", None),
        (
            "settled",
            "settled",
            400,
            Some(100),
            Some(now),
            "confirmed",
            Some(100),
        ),
    ] {
        f.control.connection.execute("INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,run_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_cost_status,evidence_kind,created_at,account_debit_micro_cny,settled_at,provider_cost_micro_cny) VALUES(?,?,1,'A',?,'turn','test','test','{}','{}',?,?,?,'test',?,?,?,?)",rusqlite::params![id,id,p,state,reserved,status,now,debit,settled,cost]).unwrap();
    }
    assert_eq!(balance(&f)["available_micro_cny"], 400);
    let reduction = json!({"operation_id":"reduce","period_id":p,"revision":1,"delta_micro_cny":-401,"reason":"reduce"});
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            "/api/admin/users/A/allowance-adjustments",
            reduction
        )
        .1["error_code"],
        "ALLOWANCE_ALREADY_COMMITTED"
    );
    post(
        &f,
        "allowance-adjustments",
        json!({"operation_id":"reduce","period_id":p,"revision":1,"delta_micro_cny":-400,"reason":"reduce"}),
    );
    assert_eq!(balance(&f)["available_micro_cny"], 0);
    post(
        &f,
        "allowance-validity",
        json!({"operation_id":"expire","period_id":p,"revision":2,"starts_at":now-20,"expires_at":now-1}),
    );
    assert!(f.ok(&f.b, "GET", "/api/admin/users/A", json!({}))["current_allowance"].is_null());
    assert_eq!(count(&f, "model_call_charges"), 3);
}

#[test]
fn adm3_failure_rolls_back_receipt_grant_revision_and_operation() {
    let f = fixture();
    let p = period(&f);
    f.control.connection.execute_batch("CREATE TRIGGER fail_grant BEFORE INSERT ON allowance_adjustments BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert_eq!(
        f.call(&f.b, "POST", "/api/admin/users/A/receipts", receipt(&p))
            .0,
        503
    );
    assert_eq!(count(&f, "manual_receipts"), 0);
    assert_eq!(count(&f, "admin_operations"), 1);
    assert_eq!(balance(&f)["granted_micro_cny"], 0);
    f.control.connection.execute_batch("DROP TRIGGER fail_grant; CREATE TRIGGER fail_result BEFORE UPDATE ON admin_operations BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert_eq!(
        f.call(&f.b, "POST", "/api/admin/users/A/receipts", receipt(&p))
            .0,
        503
    );
    assert_eq!(count(&f, "manual_receipts"), 0);
    assert_eq!(count(&f, "allowance_adjustments"), 0);
    f.control
        .connection
        .execute_batch("DROP TRIGGER fail_result;")
        .unwrap();
    post(&f, "receipts", receipt(&p));
}

#[test]
fn adm3_concurrent_same_operation_and_revision_commit_once() {
    let f = fixture();
    let p = period(&f);
    let admin = &f.access.admin;
    let submit = || {
        let mut body = receipt(&p);
        body.as_object_mut().unwrap().remove("operation_id");
        admin
            .apply(
                "B",
                "receipt",
                "A",
                crate::admin_store::Command::Allowance(crate::allowance_admin::Mutation::Receipt(
                    serde_json::from_value(body).unwrap(),
                )),
                crate::multi_user_host::now(),
            )
            .unwrap()
            .0
    };
    let replies = std::thread::scope(|s| {
        let a = s.spawn(&submit);
        let b = s.spawn(&submit);
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(replies.0, replies.1);
    let replies = std::thread::scope(|s| {
        let access = f.access.clone();
        let token = f.b.clone();
        let p1 = p.clone();
        let a=s.spawn(move || crate::multi_user_host::dispatch(&access,&crate::multi_user_host::Site::new("https://reader.example").unwrap(),"POST","/api/admin/users/A/allowance-adjustments", &crate::multi_user_host::Headers(vec![("Host".into(),"reader.example".into()),("Origin".into(),"https://reader.example".into()),("Cookie".into(),format!("{}={token}",crate::auth::COOKIE)),("X-CSRF-Token".into(),crate::auth::csrf(&token))]),&json!({"operation_id":"gift-a","period_id":p1,"revision":1,"delta_micro_cny":10,"reason":"test"}).to_string(),crate::multi_user_host::now()).status);
        let b=f.call(&f.b,"POST","/api/admin/users/A/allowance-adjustments",json!({"operation_id":"gift-b","period_id":p,"revision":1,"delta_micro_cny":20,"reason":"test"})).0;
        (a.join().unwrap(), b)
    });
    assert!(matches!(replies, (200, 409) | (409, 200)));
    assert_eq!(count(&f, "allowance_adjustments"), 2);
}
