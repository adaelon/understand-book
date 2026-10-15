use super::*;
use crate::{
    admin_store::{AdminStore, Command},
    mcp_pack::{Correction, Mutation, Purchase},
};

fn admin(f: &Fixture) -> AdminStore {
    AdminStore::new(ControlStore::open(f.db.writer.clone()).unwrap())
}
fn buy(f: &Fixture, op: &str, count: i64, reference: Option<&str>) -> Result<Value, ToolError> {
    admin(f)
        .apply(
            "admin",
            op,
            "alice",
            Command::Mcp(Mutation::Purchase(Purchase {
                pack_count: count,
                paid_at: 123,
                channel: " wechat ".into(),
                external_ref: reference.map(str::to_owned),
                note: " paid ".into(),
            })),
            f.now.load(Ordering::SeqCst),
        )
        .map(|v| v.0)
}
fn correct(
    f: &Fixture,
    op: &str,
    receipt: &Value,
    revision: i64,
    fen: i64,
    units: i64,
) -> Result<Value, ToolError> {
    admin(f)
        .apply(
            "admin",
            op,
            "alice",
            Command::Mcp(Mutation::Correction(Correction {
                receipt_id: receipt["receipt_id"].as_str().unwrap().into(),
                revision,
                delta_fen: fen,
                delta_units: units,
                reason: " recording error ".into(),
            })),
            f.now.load(Ordering::SeqCst),
        )
        .map(|v| v.0)
}
fn exhaust_free(f: &Fixture) {
    f.usage.status("alice").unwrap();
    f.db.connection
        .execute(
            "UPDATE mcp_free_months SET used_units=granted_units WHERE user_id='alice'",
            [],
        )
        .unwrap();
}

#[test]
fn mcp_pack_purchase_replay_conflicts_rollback_and_restart() {
    let f = Fixture::new();
    let first = buy(&f, "buy", 2, Some(" tx-1 ")).unwrap();
    assert_eq!(first["call_pack"]["price_fen"], 2000);
    assert_eq!(first["call_pack"]["total_units"], 2000);
    assert_eq!(
        first["call_pack"]["expires_at"],
        "2027-10-11T09:00:00+08:00"
    );
    assert_eq!(buy(&f, "buy", 2, Some("tx-1")).unwrap(), first);
    f.at("2026-11-01T00:00:00+08:00");
    assert_eq!(buy(&f, "buy", 2, Some("tx-1")).unwrap(), first);
    assert_eq!(
        buy(&f, "buy", 3, Some("tx-1")).unwrap_err().error_code,
        "ADMIN_OPERATION_CONFLICT"
    );
    assert_eq!(
        buy(&f, "dup", 1, Some("tx-1")).unwrap_err().error_code,
        "RECEIPT_EXTERNAL_REF_CONFLICT"
    );
    for count in [0, -1, i64::MAX] {
        assert!(buy(&f, "bad", count, None).is_err());
    }
    f.db.connection.execute_batch("CREATE TRIGGER fail_pack BEFORE INSERT ON mcp_call_packs BEGIN SELECT RAISE(ABORT,'pack failure'); END;").unwrap();
    assert!(buy(&f, "rollback", 1, Some("tx-2")).is_err());
    for table in ["admin_operations", "manual_receipts", "mcp_call_packs"] {
        assert_eq!(f.count(table), 1);
    }
    for table in [
        "allowance_periods",
        "allowance_adjustments",
        "model_call_charges",
    ] {
        assert_eq!(f.count(table), 0);
    }
    let root = f._root;
    let clock = f.now;
    drop(f.usage);
    drop(f.db);
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    let reopened = Fixture::service(&writer, &clock);
    assert_eq!(reopened.status("alice").unwrap().paid_remaining, 2000);
    assert_eq!(
        AdminStore::new(ControlStore::open(writer).unwrap())
            .operation("admin", "buy")
            .unwrap(),
        first
    );
}

#[test]
fn mcp_pack_free_first_calendar_expiry_and_cross_month_carry() {
    let f = Fixture::new();
    f.at("2028-02-29T23:30:00+08:00");
    let pack = buy(&f, "leap", 1, None).unwrap();
    assert_eq!(pack["call_pack"]["expires_at"], "2029-02-28T23:30:00+08:00");
    assert!(matches!(
        f.consume().charged_from,
        DebitSource::FreeMonth { .. }
    ));
    assert_eq!(f.usage.status("alice").unwrap().paid_remaining, 1000);
    exhaust_free(&f);
    // Empty successful search has the same one-unit debit as any content result.
    let debit = f
        .usage
        .consume_success("alice", "book_search_text", &reference())
        .unwrap();
    assert!(
        matches!(debit.charged_from,DebitSource::CallPack{ref pack_id} if pack_id==pack["call_pack"]["pack_id"].as_str().unwrap())
    );
    assert_eq!(debit.usage.paid_remaining, 999);
    assert_eq!(f.usage.status("bob").unwrap().paid_remaining, 0);
    f.at("2028-03-01T00:00:00+08:00");
    let status = f.usage.status("alice").unwrap();
    assert_eq!((status.free_remaining, status.paid_remaining), (1000, 999));
    let debit = f.consume();
    assert_eq!(
        (debit.usage.free_remaining, debit.usage.paid_remaining),
        (999, 999)
    );
    f.at("2029-02-28T23:29:59+08:00");
    exhaust_free(&f);
    assert_eq!(f.consume().usage.paid_remaining, 998);
    f.at("2029-02-28T23:30:00+08:00");
    let status = f.usage.status("alice").unwrap();
    assert_eq!(status.paid_remaining, 0);
    assert_eq!(status.call_packs[0].remaining_units, 998);
    assert_eq!(status.paid_next_expires_at, None);
    assert_eq!(
        f.usage
            .consume_success("alice", "book_text", &reference())
            .unwrap_err()
            .error_code,
        "MCP_QUOTA_EXHAUSTED"
    );
    f.at("2026-01-31T12:34:56+08:00");
    assert_eq!(
        buy(&f, "month-end", 1, None).unwrap()["call_pack"]["expires_at"],
        "2027-01-31T12:34:56+08:00"
    );
}

#[test]
fn mcp_pack_earliest_expiry_then_opening_order_and_expired_exclusion() {
    let f = Fixture::new();
    let late = buy(&f, "late", 1, None).unwrap();
    f.at("2026-09-01T09:00:00+08:00");
    let early = buy(&f, "early", 1, None).unwrap();
    let tied = buy(&f, "tied", 1, None).unwrap();
    f.at("2026-10-11T09:00:00+08:00");
    exhaust_free(&f);
    assert_eq!(
        f.usage
            .status("alice")
            .unwrap()
            .paid_next_expires_at
            .as_deref(),
        Some("2027-09-01T09:00:00+08:00")
    );
    correct(&f, "withdraw", &early, 0, 0, -999).unwrap();
    assert!(
        matches!(f.consume().charged_from,DebitSource::CallPack{ref pack_id} if pack_id==early["call_pack"]["pack_id"].as_str().unwrap())
    );
    assert!(
        matches!(f.consume().charged_from,DebitSource::CallPack{ref pack_id} if pack_id==tied["call_pack"]["pack_id"].as_str().unwrap())
    );
    f.at("2027-09-01T09:00:00+08:00");
    exhaust_free(&f);
    assert!(
        matches!(f.consume().charged_from,DebitSource::CallPack{ref pack_id} if pack_id==late["call_pack"]["pack_id"].as_str().unwrap())
    );
}

#[test]
fn mcp_pack_corrections_preserve_consumption_and_separate_cash_and_units() {
    let f = Fixture::new();
    let pack = buy(&f, "buy", 1, None).unwrap();
    exhaust_free(&f);
    let consumed = f.consume();
    assert_eq!(
        correct(&f, "overdraw", &pack, 0, -1000, -1000)
            .unwrap_err()
            .error_code,
        "MCP_PACK_ALREADY_CONSUMED"
    );
    assert_eq!(
        correct(&f, "overrefund", &pack, 0, -1001, 0)
            .unwrap_err()
            .error_code,
        "RECEIPT_CORRECTION_EXCEEDS_AMOUNT"
    );
    let correction = correct(&f, "correction", &pack, 0, -500, -500).unwrap();
    assert_eq!(
        correct(&f, "correction", &pack, 0, -500, -500).unwrap(),
        correction
    );
    assert_eq!(
        correct(&f, "correction", &pack, 0, -499, -500)
            .unwrap_err()
            .error_code,
        "ADMIN_OPERATION_CONFLICT"
    );
    assert_eq!(
        correct(&f, "stale", &pack, 0, 1, 0).unwrap_err().error_code,
        "MCP_PACK_REVISION_CONFLICT"
    );
    correct(&f, "cash", &pack, 1, -500, 0).unwrap();
    assert_eq!(f.usage.status("alice").unwrap().paid_remaining, 499);
    correct(&f, "units", &pack, 2, 0, -499).unwrap();
    assert_eq!(f.usage.status("alice").unwrap().paid_remaining, 0);
    correct(&f, "restore", &pack, 3, 0, 10).unwrap();
    assert_eq!(f.usage.status("alice").unwrap().paid_remaining, 10);
    let receipts = admin(&f)
        .allowance_list("alice", "receipts", 50, 0)
        .unwrap();
    assert_eq!(receipts["items"][0]["entitlement_kind"], "mcp_call_pack");
    assert_eq!(receipts["items"][0]["period_id"], Value::Null);
    assert_eq!(receipts["items"][0]["amount_fen"], 1000);
    assert_eq!(receipts["items"][0]["effective_amount_fen"], 0);
    assert_eq!(f.count("mcp_usage"), 1);
    assert_eq!(f.count("receipt_corrections"), 2);
    assert_eq!(f.count("mcp_pack_adjustments"), 3);
    assert_eq!(f.count("allowance_adjustments"), 0);
    let saved: (String, String) =
        f.db.connection
            .query_row(
                "SELECT debit_source,debit_ref FROM mcp_usage WHERE call_id=?",
                [consumed.call_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(
        saved,
        (
            "call_pack".into(),
            pack["call_pack"]["pack_id"].as_str().unwrap().into()
        )
    );
}

#[test]
fn mcp_pack_debit_and_correction_failures_rollback_all_facts() {
    let f = Fixture::new();
    let pack = buy(&f, "buy", 1, None).unwrap();
    exhaust_free(&f);
    f.db.connection.execute_batch("CREATE TRIGGER fail_usage BEFORE INSERT ON mcp_usage BEGIN SELECT RAISE(ABORT,'usage failure'); END;").unwrap();
    assert!(f
        .usage
        .consume_success("alice", "book_text", &reference())
        .is_err());
    assert_eq!(f.usage.status("alice").unwrap().paid_remaining, 1000);
    assert_eq!(f.count("mcp_usage"), 0);
    f.db.connection.execute_batch("CREATE TRIGGER fail_correction BEFORE INSERT ON mcp_pack_adjustments BEGIN SELECT RAISE(ABORT,'correction failure'); END;").unwrap();
    assert!(correct(&f, "rollback", &pack, 0, -1000, -1000).is_err());
    assert_eq!(f.count("admin_operations"), 1);
    assert_eq!(f.count("receipt_corrections"), 0);
    assert_eq!(f.usage.status("alice").unwrap().call_packs[0].revision, 0);
}

#[test]
fn mcp_pack_last_paid_unit_is_not_double_spent() {
    let f = Fixture::new();
    let pack = buy(&f, "buy", 1, None).unwrap();
    correct(&f, "leave-one", &pack, 0, 0, -999).unwrap();
    exhaust_free(&f);
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
                assert_eq!(usage.status("alice").unwrap().paid_remaining, 1);
                barrier.wait();
                usage.consume_success("alice", "book_text", &reference())
            })
        })
        .collect();
    let results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .into_iter()
            .find_map(Result::err)
            .unwrap()
            .error_code,
        "MCP_QUOTA_EXHAUSTED"
    );
    assert_eq!(f.count("mcp_usage"), 1);
    assert_eq!(f.usage.status("alice").unwrap().paid_remaining, 0);
}

#[test]
fn mcp_pack_schema11_migration_preserves_free_history_and_is_atomic() {
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
            include_str!("mcp_usage.sql"),
        ] {
            db.execute_batch(sql).unwrap();
        }
        db.execute_batch("PRAGMA application_id=1430408533; PRAGMA user_version=11;
            INSERT INTO users(user_id) VALUES('alice');
            INSERT INTO mcp_free_months VALUES('alice','2026-10',1000,1);
            INSERT INTO mcp_usage VALUES('call','alice','book_text','book','pub',1,'free_month','2026-10',100);").unwrap();
        if fail {
            db.execute_batch("CREATE TABLE mcp_usage_next(keep TEXT)")
                .unwrap();
        }
        drop(db);
        let result = ControlStore::open(writer);
        let db = Connection::open(root.path().join("control.sqlite")).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT debit_ref FROM mcp_usage WHERE call_id='call'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "2026-10"
        );
        assert_eq!(
            db.query_row("SELECT used_units FROM mcp_free_months", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            if fail { 11 } else { CONTROL_SCHEMA_VERSION }
        );
        if fail {
            assert!(result.is_err());
            assert_eq!(
                db.query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE name='mcp_call_packs'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                0
            );
        } else {
            let store = result.unwrap();
            assert!(store.connection.execute("INSERT INTO mcp_usage(call_id,user_id,tool,book_id,publication_id,units,debit_source,debit_ref,completed_at) VALUES('bad','alice','book_text','book','pub',1,'call_pack','missing',100)",[]).is_err());
        }
    }
}
