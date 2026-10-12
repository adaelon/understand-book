//! Release rehearsal: real SQLite and the production backup/restore entry points.
use super::mu5_tests::Fixture;
use crate::{
    control_store::{ControlStore, ServiceWriter},
    reader_maintenance::{backup_service, restore_service},
};
use rusqlite::{types::Value as SqlValue, Connection};
use serde_json::json;
use std::{collections::BTreeMap, fs, path::Path};

fn rows(db: &Connection, table: &str, columns: &str) -> Vec<Vec<SqlValue>> {
    let mut stmt = db
        .prepare(&format!("SELECT {columns} FROM {table} ORDER BY rowid"))
        .unwrap();
    let n = stmt.column_count();
    stmt.query_map([], |r| (0..n).map(|i| r.get(i)).collect())
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn adm10_old_snapshot_upgrades_only_the_restored_copy() {
    let temp = tempfile::tempdir().unwrap();
    let snapshot = temp.path().join("old-snapshot");
    let old = snapshot.join("service");
    fs::create_dir_all(old.join("users/reader/memory")).unwrap();
    fs::create_dir_all(old.join("books/book/pub")).unwrap();
    fs::write(old.join("books/book/pub/publication.json"), "{}").unwrap();
    fs::write(
        old.join("users/reader/memory/agent-history.json"),
        b"old private bytes\n",
    )
    .unwrap();
    let source = temp.path().join("old-live");
    let db = Connection::open(old.join("control.sqlite")).unwrap();
    db.execute_batch(include_str!("control_schema_v4.sql"))
        .unwrap();
    db.execute_batch(
        "PRAGMA application_id=1430408533; PRAGMA user_version=4;
        INSERT INTO users VALUES('reader',0,7,'old-password-hash');
        INSERT INTO auth_sessions VALUES(X'0102','reader',7,9000);",
    )
    .unwrap();
    db.execute(
        "INSERT INTO book_publications VALUES('book','pub',?,'{}')",
        [source.join("books/book/pub").to_string_lossy().as_ref()],
    )
    .unwrap();
    db.execute_batch("
        INSERT INTO book_grants VALUES('reader','book','pub');
        INSERT INTO book_defaults VALUES('book','pub');
        INSERT INTO reader_workspaces(owner_user_id,workspace_id,book_id,publication_id,selected_chat,checkpoint)
          VALUES('reader','scene','book','pub','chat','saved-checkpoint');").unwrap();
    let preserved = [
        "auth_sessions",
        "book_grants",
        "book_defaults",
        "reader_workspaces",
    ];
    let before: Vec<_> = preserved.iter().map(|t| rows(&db, t, "*")).collect();
    let users = rows(&db, "users", "user_id,disabled,auth_epoch,password_hash");
    drop(db);
    // Frozen MU9 snapshot-v1 shape from the old release; no new-binary backup of an old schema.
    let inventory: BTreeMap<_, _> = files(&snapshot)
        .into_iter()
        .map(|(p, b)| (p, b.len()))
        .collect();
    fs::write(
        snapshot.join("snapshot.json"),
        json!({"version":1,"metadata":{
        "kind":"service","source_root":source,"control_schema":4},"files":inventory})
        .to_string(),
    )
    .unwrap();
    let snapshot_bytes = files(&snapshot);
    let restored = temp.path().join("candidate");
    restore_service(&snapshot, &restored).unwrap();
    let writer = ServiceWriter::acquire(&restored).unwrap();
    let control = ControlStore::open(writer).unwrap();
    assert_eq!(
        control
            .connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        crate::control_store::CONTROL_SCHEMA_VERSION
    );
    assert_eq!(
        rows(
            &control.connection,
            "users",
            "user_id,disabled,auth_epoch,password_hash"
        ),
        users
    );
    for (table, expected) in preserved.iter().zip(before) {
        assert_eq!(rows(&control.connection, table, "*"), expected, "{table}");
    }
    for table in [
        "allowance_periods",
        "manual_receipts",
        "model_call_charges",
        "admin_usage_events",
    ] {
        assert!(
            rows(&control.connection, table, "*").is_empty(),
            "must not invent old activity or charges"
        );
    }
    assert_eq!(
        rows(&control.connection, "users", "is_admin"),
        vec![vec![SqlValue::Integer(0)]]
    );
    let directory: String = control
        .connection
        .query_row("SELECT directory FROM book_publications", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        Path::new(&directory),
        restored.canonicalize().unwrap().join("books/book/pub")
    );
    assert_eq!(
        fs::read(restored.join("users/reader/memory/agent-history.json")).unwrap(),
        b"old private bytes\n"
    );
    assert_eq!(files(&snapshot), snapshot_bytes);
}

#[test]
fn adm10_backup_restores_admin_money_usage_private_files_and_unknown_reservations() {
    use runtime::{model_spend::*, provider_stream::ModelUsage};
    let mut f = Fixture::new();
    let period = super::adm5_tests::seed(&mut f, "fixture", 10_000_000);
    f.create(&f.a, &f.x, "rehearsal");
    let now = crate::multi_user_host::now();
    crate::admin_usage::record(&f.control.connection, "A", "question", "q", now).unwrap();
    crate::admin_usage::record(&f.control.connection, "A", "completed", "q", now).unwrap();
    let payment = json!({"operation_id":"adm10-pay","period_id":period,"revision":1,
        "amount_fen":1234,"delta_micro_cny":2_000_000,"paid_at":now,"channel":"wechat","note":"fixture only"});
    let receipt = f.ok(&f.b, "POST", "/api/admin/users/A/receipts", payment.clone());
    assert_eq!(
        f.ok(&f.b, "POST", "/api/admin/users/A/receipts", payment),
        receipt
    );
    f.ok(&f.b,"POST","/api/admin/users/A/receipt-corrections",json!({"operation_id":"adm10-correction",
        "receipt_id":receipt["receipt_id"],"period_id":period,"revision":2,"delta_micro_cny":0,"delta_fen":-34,"reason":"fixture"}));
    for (name, tokens) in [
        ("settled", Some(51)),
        ("pending", None),
        ("waived", None),
        ("sent", None),
    ] {
        let scope = ChargeScope::ReaderRun {
            user_id: "A".into(),
            run_ref: format!("adm10-{name}"),
        };
        let port = f
            .access
            .spend
            .port(scope.clone(), f.x.clone(), Default::default());
        let id = SendIdentity {
            call_id: name.into(),
            logical_call_id: name.into(),
            attempt: 1,
            scope: Some(scope),
            purpose: "outer".into(),
            model: "fixture-model".into(),
            provider: "fixture".into(),
        };
        port.before_send(
            &id,
            &json!({"model":"fixture-model","messages":[],"max_tokens":100}),
        )
        .unwrap();
        if name != "sent" {
            port.after_send(
                &id,
                &SendOutcome {
                    evidence: SendEvidence::Response,
                    usage: tokens.map(|n| ModelUsage {
                        input_tokens: Some(n),
                        cached_input_tokens: Some(0),
                        output_tokens: Some(0),
                        ..Default::default()
                    }),
                    succeeded: tokens.is_some(),
                },
            )
            .unwrap();
        }
    }
    let waived = f.ok(&f.b, "GET", "/api/admin/charges/waived", json!({}));
    f.ok(&f.b,"POST","/api/admin/charges/waived/reconcile",json!({"operation_id":"adm10-waive",
        "revision":waived["revision"],"waive_account":true,"reason":"fixture","evidence":"cost unknown"}));
    let tables = [
        "users",
        "auth_sessions",
        "book_grants",
        "book_defaults",
        "reader_workspaces",
        "admin_operations",
        "allowance_periods",
        "allowance_adjustments",
        "manual_receipts",
        "receipt_corrections",
        "model_call_charges",
        "charge_reports",
        "charge_reconciliations",
        "admin_usage_events",
    ];
    let expected: Vec<_> = tables
        .iter()
        .map(|t| rows(&f.control.connection, t, "*"))
        .collect();
    let root = f.root.path().to_owned();
    let x = f.x.clone();
    // Retain the source directory after dropping all writer handles.
    let Fixture {
        root: source_guard,
        access,
        control,
        ..
    } = f;
    drop(access);
    drop(control);
    let private = files(&root.join("users"));
    let rates = fs::read(root.join("model-rates.json")).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let backup = temp.path().join("backup");
    backup_service(&root, &backup).unwrap();
    let restored = temp.path().join("restored");
    restore_service(&backup, &restored).unwrap();
    let restored_files = files(&restored.join("users"));
    assert_eq!(
        restored_files.keys().collect::<Vec<_>>(),
        private.keys().collect::<Vec<_>>()
    );
    for (path, bytes) in &private {
        if bytes.starts_with(b"SQLite format 3\0") {
            // SQLite backup changes physical headers; compare every application table instead.
            let old = Connection::open(root.join("users").join(path)).unwrap();
            let new = Connection::open(restored.join("users").join(path)).unwrap();
            let mut q = old.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap();
            let names = q
                .query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            for table in names {
                assert_eq!(
                    rows(&new, &table, "*"),
                    rows(&old, &table, "*"),
                    "{path}:{table}"
                );
            }
        } else {
            assert!(
                restored_files[path] == *bytes,
                "private file changed: {path}"
            );
        }
    }
    assert_eq!(fs::read(restored.join("model-rates.json")).unwrap(), rates);
    let writer = ServiceWriter::acquire(&restored).unwrap();
    let control = ControlStore::open(writer.clone()).unwrap();
    for (table, expected) in tables.iter().zip(expected) {
        assert_eq!(rows(&control.connection, table, "*"), expected, "{table}");
    }
    let mut library = crate::published_library::PublishedLibrary::new(
        ControlStore::open(writer.clone()).unwrap(),
    );
    let publication = library.load("A", &x).unwrap();
    assert!(publication
        .directory()
        .starts_with(restored.canonicalize().unwrap()));
    let spend = crate::model_spend_store::ModelSpendStore::new(ControlStore::open(writer).unwrap());
    spend.recover().unwrap();
    let charges = rows(
        &control.connection,
        "model_call_charges",
        "call_id,state,provider_cost_micro_cny,account_debit_micro_cny,reserved_micro_cny",
    );
    let get = |id: &str| {
        charges
            .iter()
            .find(|r| r[0] == SqlValue::Text(id.into()))
            .unwrap()
    };
    assert_eq!(get("settled")[2], SqlValue::Integer(51));
    assert_eq!(get("sent")[1], SqlValue::Text("pending".into()));
    assert_eq!(get("sent")[2], SqlValue::Null);
    assert_eq!(get("pending")[1], SqlValue::Text("pending".into()));
    assert_eq!(get("waived")[2], SqlValue::Null);
    assert_eq!(get("waived")[3], SqlValue::Integer(0));
    spend.recover().unwrap();
    assert_eq!(
        rows(
            &control.connection,
            "model_call_charges",
            "call_id,state,provider_cost_micro_cny,account_debit_micro_cny,reserved_micro_cny"
        ),
        charges
    );
    drop(source_guard);
}
