use super::*;
#[path = "mcp_pack_tests.rs"]
mod packs;
use crate::{
    auth::AuthService,
    control_store::{ServiceWriter, CONTROL_SCHEMA_VERSION},
    mcp_access::McpAccess,
};
use std::sync::{
    atomic::{AtomicI64, Ordering},
    Barrier,
};

fn timestamp(value: &str) -> i64 {
    OffsetDateTime::parse(value, &Rfc3339)
        .unwrap()
        .unix_timestamp()
}
fn reference() -> PublishedBookRef {
    PublishedBookRef {
        book_id: "book".into(),
        publication_id: "publication".into(),
    }
}
struct Fixture {
    usage: McpUsage,
    db: ControlStore,
    now: Arc<AtomicI64>,
    _root: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let writer = ServiceWriter::acquire(root.path()).unwrap();
        let mut db = ControlStore::open(writer.clone()).unwrap();
        for user in ["alice", "bob", "admin"] {
            db.create_user(user).unwrap();
        }
        db.connection
            .execute("UPDATE users SET is_admin=1 WHERE user_id='admin'", [])
            .unwrap();
        let now = Arc::new(AtomicI64::new(timestamp("2026-10-11T09:00:00+08:00")));
        let usage = Self::service(&writer, &now);
        Self {
            usage,
            db,
            now,
            _root: root,
        }
    }
    fn service(writer: &Arc<ServiceWriter>, now: &Arc<AtomicI64>) -> McpUsage {
        let clock = now.clone();
        let mut usage = McpUsage::new(ControlStore::open(writer.clone()).unwrap());
        usage.clock = Arc::new(move || clock.load(Ordering::SeqCst));
        usage
    }
    fn at(&self, value: &str) {
        self.now.store(timestamp(value), Ordering::SeqCst);
    }
    fn login(&mut self, user: &str) -> Principal {
        self.db
            .provision_password(user, "fixture-only-password", false)
            .unwrap();
        let auth = AuthService::new(ControlStore::open(self.db.writer.clone()).unwrap());
        let now = self.now.load(Ordering::SeqCst);
        let token = auth
            .login(user, "fixture-only-password", None, now)
            .unwrap();
        auth.authenticate(&token, now).unwrap()
    }
    fn count(&self, table: &str) -> i64 {
        self.db
            .connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }
    fn consume(&self) -> Consumption {
        self.usage
            .consume_success("alice", "book_text", &reference())
            .unwrap()
    }
}

#[test]
fn mcp_usage_first_month_is_full_account_owned_and_history_is_minimal() {
    let f = Fixture::new();
    let initial = f.usage.status("alice").unwrap();
    assert_eq!(
        (
            initial.free_granted,
            initial.free_used,
            initial.free_remaining
        ),
        (1000, 0, 1000)
    );
    assert_eq!(initial.free_resets_at, "2026-11-01T00:00:00+08:00");
    assert_eq!(f.count("mcp_usage"), 0);
    let first = f.consume();
    let second = f
        .usage
        .consume_success("alice", "book_search_text", &reference())
        .unwrap();
    assert_ne!(first.call_id, second.call_id);
    assert_eq!(first.charged_units, 1);
    assert!(
        matches!(first.charged_from, DebitSource::FreeMonth { ref month_hk } if month_hk == "2026-10")
    );
    assert_eq!(second.usage.free_remaining, 998);
    assert_eq!(f.usage.status("bob").unwrap().free_remaining, 1000);
    let row: (String, String, String, String, i64, String, String, i64) = f.db.connection.query_row(
        "SELECT user_id,tool,book_id,publication_id,units,debit_source,debit_ref,completed_at FROM mcp_usage WHERE call_id=?",
        [&second.call_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?)),
    ).unwrap();
    assert_eq!(
        row,
        (
            "alice".into(),
            "book_search_text".into(),
            "book".into(),
            "publication".into(),
            1,
            "free_month".into(),
            "2026-10".into(),
            f.now.load(Ordering::SeqCst)
        )
    );
    let columns: Vec<String> =
        f.db.connection
            .prepare("PRAGMA table_info(mcp_usage)")
            .unwrap()
            .query_map([], |r| r.get(1))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
    assert_eq!(
        columns,
        [
            "call_id",
            "user_id",
            "tool",
            "book_id",
            "publication_id",
            "units",
            "debit_source",
            "debit_ref",
            "completed_at"
        ]
    );
    assert_eq!(f.count("manual_receipts"), 0);
    assert_eq!(f.count("model_call_charges"), 0);
    assert_eq!(f.count("reader_workspaces"), 0);
}

#[test]
fn mcp_usage_hong_kong_rollover_and_unused_gifts_do_not_carry() {
    let mut f = Fixture::new();
    f.at("2026-10-31T23:59:59+08:00");
    let admin = f.login("admin");
    let receipt = f
        .usage
        .gift(&admin, "gift-1", "alice", 50, " beta reading ")
        .unwrap();
    assert_eq!(receipt["parameters"]["reason"], "beta reading");
    assert_eq!(f.consume().usage.free_remaining, 1049);
    f.at("2026-11-01T00:00:00+08:00");
    let current = f.usage.status("alice").unwrap();
    assert_eq!(
        (current.month_hk.as_str(), current.free_remaining),
        ("2026-11", 1000)
    );
    assert_eq!(
        f.usage
            .gift(&admin, "gift-1", "alice", 50, "beta reading")
            .unwrap(),
        receipt
    );
    assert_eq!(f.usage.status("alice").unwrap().free_remaining, 1000);
    assert_eq!(f.consume().usage.free_remaining, 999);
    assert_eq!(f.db.connection.query_row("SELECT granted_units-used_units FROM mcp_free_months WHERE user_id='alice' AND month_hk='2026-10'", [], |r|r.get::<_,i64>(0)).unwrap(), 1049);
    assert_eq!(f.count("mcp_free_gifts"), 1);
    for (at, month, reset) in [
        (
            "2026-12-31T16:00:00Z",
            "2027-01",
            "2027-02-01T00:00:00+08:00",
        ),
        (
            "2028-02-29T15:59:59Z",
            "2028-02",
            "2028-03-01T00:00:00+08:00",
        ),
        (
            "2028-02-29T16:00:00Z",
            "2028-03",
            "2028-04-01T00:00:00+08:00",
        ),
    ] {
        f.at(at);
        let status = f.usage.status("alice").unwrap();
        assert_eq!(
            (
                status.month_hk.as_str(),
                status.free_resets_at.as_str(),
                status.free_remaining
            ),
            (month, reset, 1000)
        );
    }
}

#[test]
fn mcp_usage_debit_samples_clock_inside_write_transaction_after_preflight() {
    let mut f = Fixture::new();
    f.at("2026-10-31T23:59:59+08:00");
    assert_eq!(f.usage.status("alice").unwrap().month_hk, "2026-10");
    let path = f.db.writer.root().join("control.sqlite");
    f.usage.clock = Arc::new(move || {
        let db = Connection::open(&path).unwrap();
        db.busy_timeout(std::time::Duration::ZERO).unwrap();
        let err = db
            .execute_batch("BEGIN IMMEDIATE")
            .expect_err("debit must already own the write transaction");
        assert_eq!(
            err.sqlite_error_code(),
            Some(rusqlite::ErrorCode::DatabaseBusy)
        );
        timestamp("2026-11-01T00:00:00+08:00")
    });
    let debit = f.consume();
    assert_eq!(
        (debit.usage.month_hk.as_str(), debit.usage.free_remaining),
        ("2026-11", 999)
    );
    assert_eq!(f.db.connection.query_row("SELECT used_units FROM mcp_free_months WHERE user_id='alice' AND month_hk='2026-10'", [], |r|r.get::<_,i64>(0)).unwrap(), 0);
}

#[test]
fn mcp_usage_two_connections_compete_for_last_unit_without_overdraft() {
    let f = Fixture::new();
    for _ in 0..999 {
        f.consume();
    }
    let services = [
        Fixture::service(&f.db.writer, &f.now),
        Fixture::service(&f.db.writer, &f.now),
    ];
    let barrier = Arc::new(Barrier::new(2));
    let workers: Vec<_> = services
        .into_iter()
        .map(|usage| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                assert_eq!(usage.status("alice").unwrap().free_remaining, 1);
                barrier.wait();
                usage.consume_success("alice", "book_text", &reference())
            })
        })
        .collect();
    let results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let failure = results.into_iter().find_map(Result::err).unwrap();
    assert_eq!(failure.error_code, "MCP_QUOTA_EXHAUSTED");
    assert_eq!(f.usage.status("alice").unwrap().free_remaining, 0);
    assert_eq!(f.count("mcp_usage"), 1000);
}

#[test]
fn mcp_usage_restart_and_replaced_credentials_share_original_month() {
    let mut f = Fixture::new();
    let alice = f.login("alice");
    let access = McpAccess::new(ControlStore::open(f.db.writer.clone()).unwrap());
    let now = f.now.load(Ordering::SeqCst);
    let old = access.create(&alice, "first client", now).unwrap();
    let identity = access
        .authenticate(Some(&format!("Bearer {}", old.token)))
        .unwrap();
    f.usage
        .consume_success(identity.user_id(), "book_text", &reference())
        .unwrap();
    access
        .revoke(&alice, &old.credential.credential_id, now)
        .unwrap();
    let new = access.create(&alice, "replacement", now).unwrap();
    let identity = access
        .authenticate(Some(&format!("Bearer {}", new.token)))
        .unwrap();
    // Drop every store and writer, then reacquire the service lock as a restart does.
    let root = f._root;
    let clock = f.now;
    drop(f.usage);
    drop(f.db);
    drop(access);
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    let reopened = Fixture::service(&writer, &clock);
    assert_eq!(
        reopened.status(identity.user_id()).unwrap().free_remaining,
        999
    );
    assert_eq!(
        reopened
            .consume_success(identity.user_id(), "book_text", &reference())
            .unwrap()
            .usage
            .free_remaining,
        998
    );
    let db = ControlStore::open(writer).unwrap();
    assert_eq!(
        db.connection
            .query_row("SELECT count(*) FROM mcp_free_months", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn mcp_usage_failed_debits_leave_no_consumption_or_partial_grant() {
    let f = Fixture::new();
    for name in ["book_list", "mcp_usage", "book_query", "unknown"] {
        assert_eq!(
            f.usage
                .consume_success("alice", name, &reference())
                .unwrap_err()
                .error_code,
            "MCP_USAGE_INPUT_INVALID"
        );
    }
    assert_eq!(
        f.usage
            .consume_success("missing", "book_text", &reference())
            .unwrap_err()
            .error_code,
        "USER_UNAVAILABLE"
    );
    let empty = PublishedBookRef {
        book_id: String::new(),
        publication_id: "publication".into(),
    };
    assert_eq!(
        f.usage
            .consume_success("alice", "book_text", &empty)
            .unwrap_err()
            .error_code,
        "MCP_USAGE_INPUT_INVALID"
    );
    f.db.connection
        .execute("UPDATE users SET disabled=1 WHERE user_id='bob'", [])
        .unwrap();
    assert_eq!(
        f.usage.status("bob").unwrap_err().error_code,
        "USER_UNAVAILABLE"
    );
    assert_eq!(
        f.usage
            .consume_success("bob", "book_text", &reference())
            .unwrap_err()
            .error_code,
        "USER_UNAVAILABLE"
    );
    assert_eq!(f.count("mcp_free_months"), 0);
    f.db.connection.execute_batch("CREATE TRIGGER fail_usage BEFORE INSERT ON mcp_usage BEGIN SELECT RAISE(ABORT,'injected usage failure'); END;").unwrap();
    assert_eq!(
        f.usage
            .consume_success("alice", "book_text", &reference())
            .unwrap_err()
            .error_code,
        "MCP_USAGE_STORAGE_UNAVAILABLE"
    );
    assert_eq!(f.count("mcp_free_months"), 0);
    assert_eq!(f.usage.status("alice").unwrap().free_remaining, 1000);
    assert_eq!(
        f.usage
            .consume_success("alice", "book_text", &reference())
            .unwrap_err()
            .error_code,
        "MCP_USAGE_STORAGE_UNAVAILABLE"
    );
    assert_eq!(f.usage.status("alice").unwrap().free_remaining, 1000);
    assert_eq!(f.count("mcp_usage"), 0);
    f.db.connection
        .execute_batch("DROP TRIGGER fail_usage")
        .unwrap();
    assert_eq!(f.consume().usage.free_remaining, 999);
}

#[test]
fn mcp_usage_gifts_require_admin_and_replay_business_fields_atomically() {
    let mut f = Fixture::new();
    let admin = f.login("admin");
    let bob = f.login("bob");
    assert_eq!(
        f.usage
            .gift(&bob, "gift", "alice", 50, "reading")
            .unwrap_err()
            .error_code,
        "ADMIN_REQUIRED"
    );
    for (units, reason) in [(0, "reason"), (-1, "reason"), (1, " ")] {
        assert_eq!(
            f.usage
                .gift(&admin, "bad", "alice", units, reason)
                .unwrap_err()
                .error_code,
            "MCP_USAGE_INPUT_INVALID"
        );
    }
    let first = f
        .usage
        .gift(&admin, "gift", "alice", 50, " reading ")
        .unwrap();
    assert_eq!(
        f.usage
            .gift(&admin, "gift", "alice", 50, "reading")
            .unwrap(),
        first
    );
    assert_eq!(f.usage.status("alice").unwrap().free_remaining, 1050);
    for (owner, units, reason) in [
        ("alice", 51, "reading"),
        ("alice", 50, "changed"),
        ("bob", 50, "reading"),
    ] {
        assert_eq!(
            f.usage
                .gift(&admin, "gift", owner, units, reason)
                .unwrap_err()
                .error_code,
            "ADMIN_OPERATION_CONFLICT"
        );
    }
    assert_eq!(
        f.usage
            .gift(&admin, "overflow", "alice", i64::MAX, "reason")
            .unwrap_err()
            .error_code,
        "MCP_USAGE_INPUT_INVALID"
    );
    f.db.connection.execute_batch("CREATE TRIGGER fail_gift BEFORE INSERT ON mcp_free_gifts BEGIN SELECT RAISE(ABORT,'injected gift failure'); END;").unwrap();
    for owner in ["alice", "bob"] {
        assert_eq!(
            f.usage
                .gift(&admin, "rollback", owner, 25, "reason")
                .unwrap_err()
                .error_code,
            "MCP_USAGE_STORAGE_UNAVAILABLE"
        );
    }
    assert_eq!(f.usage.status("alice").unwrap().free_remaining, 1050);
    assert_eq!(f.count("mcp_free_months"), 1);
    assert_eq!(f.count("mcp_free_gifts"), 1);
    assert_eq!(f.count("admin_operations"), 1);
    assert_eq!(f.count("manual_receipts"), 0);
    assert_eq!(f.count("allowance_adjustments"), 0);
    f.db.connection
        .execute_batch("DROP TRIGGER fail_gift; UPDATE users SET is_admin=0 WHERE user_id='admin';")
        .unwrap();
    assert_eq!(
        f.usage
            .gift(&admin, "gift", "alice", 50, "reading")
            .unwrap_err()
            .error_code,
        "ADMIN_REQUIRED"
    );
    f.db.connection
        .execute_batch("UPDATE users SET is_admin=1,auth_epoch=auth_epoch+1 WHERE user_id='admin';")
        .unwrap();
    assert_eq!(
        f.usage
            .gift(&admin, "gift", "alice", 50, "reading")
            .unwrap_err()
            .error_code,
        "AUTH_REQUIRED"
    );
}

#[test]
fn mcp_usage_schema_10_upgrade_preserves_existing_data_and_rolls_back() {
    for fail in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let writer = ServiceWriter::acquire(root.path()).unwrap();
        let db = Connection::open(root.path().join("control.sqlite")).unwrap();
        for sql in [
            include_str!("control_schema.sql"),
            include_str!("admin_schema.sql"),
            include_str!("receipt_corrections.sql"),
            include_str!("charge_settlement.sql"),
            include_str!("usage_schema.sql"),
            include_str!("account_registration.sql"),
            include_str!("mcp_access.sql"),
        ] {
            db.execute_batch(sql).unwrap();
        }
        db.execute_batch("INSERT INTO users(user_id,auth_epoch) VALUES('alice',7);
            INSERT INTO mcp_credentials VALUES('credential','alice',7,X'1234','client',100,NULL);
            INSERT INTO reader_workspaces(owner_user_id,workspace_id,checkpoint) VALUES('alice','scene','saved');").unwrap();
        db.pragma_update(None, "application_id", 0x55424d55)
            .unwrap();
        db.pragma_update(None, "user_version", 10).unwrap();
        if fail {
            db.execute_batch("CREATE INDEX mcp_usage_user_time ON users(user_id)")
                .unwrap();
        }
        drop(db);
        let result = ControlStore::open(writer.clone());
        if fail {
            assert_eq!(
                result.err().unwrap().error_code,
                "CONTROL_STORAGE_UNAVAILABLE"
            );
            let db = Connection::open(root.path().join("control.sqlite")).unwrap();
            assert_eq!(
                db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                    .unwrap(),
                10
            );
            assert_eq!(db.query_row("SELECT count(*) FROM sqlite_schema WHERE type='table' AND name IN ('mcp_free_months','mcp_free_gifts','mcp_usage')", [], |r|r.get::<_,i64>(0)).unwrap(), 0);
        } else {
            drop(result.unwrap());
            let reopened = ControlStore::open(writer).unwrap();
            let db = &reopened.connection;
            assert_eq!(
                db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                    .unwrap(),
                CONTROL_SCHEMA_VERSION
            );
            assert_eq!(
                db.query_row(
                    "SELECT auth_epoch FROM users WHERE user_id='alice'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                7
            );
            let credential: (String,i64,Vec<u8>,String,i64,Option<i64>) = db.query_row("SELECT owner_user_id,auth_epoch,token_digest,label,created_at,revoked_at FROM mcp_credentials WHERE credential_id='credential'", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).unwrap();
            assert_eq!(
                credential,
                (
                    "alice".into(),
                    7,
                    vec![0x12, 0x34],
                    "client".into(),
                    100,
                    None
                )
            );
            assert_eq!(
                db.query_row("SELECT checkpoint FROM reader_workspaces", [], |r| r
                    .get::<_, String>(0))
                    .unwrap(),
                "saved"
            );
            assert_eq!(
                db.query_row("SELECT count(*) FROM mcp_free_months", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
    }
}
