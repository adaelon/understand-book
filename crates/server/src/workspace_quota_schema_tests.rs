use super::*;
use rusqlite::types::Value;

/// Construct a real historical database without today's tables or columns.
pub(crate) fn historical_database(root: &Path, version: i64) -> Connection {
    assert!((12..=14).contains(&version));
    let db = Connection::open(root.join("control.sqlite")).unwrap();
    db.pragma_update(None, "foreign_keys", "ON").unwrap();
    db.execute_batch(include_str!("tests/control_schema_v4.sql"))
        .unwrap();
    db.execute_batch("ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0 CHECK(is_admin IN (0,1));").unwrap();
    for script in [
        include_str!("admin_schema.sql"),
        include_str!("receipt_corrections.sql"),
        include_str!("charge_settlement.sql"),
        include_str!("usage_schema.sql"),
    ] {
        db.execute_batch(script).unwrap();
    }
    db.execute_batch(
        "ALTER TABLE users ADD COLUMN email TEXT CHECK(email IS NULL OR (email=lower(trim(email)) AND length(email)>0));
         ALTER TABLE users ADD COLUMN email_verified_at INTEGER CHECK((email IS NULL) = (email_verified_at IS NULL));",
    ).unwrap();
    for script in [
        include_str!("account_registration.sql"),
        include_str!("mcp_access.sql"),
        include_str!("mcp_usage.sql"),
        include_str!("mcp_packs.sql"),
    ] {
        db.execute_batch(script).unwrap();
    }
    if version >= 13 {
        db.execute_batch(include_str!("default_book_grants.sql"))
            .unwrap();
    }
    if version >= 14 {
        db.execute_batch(include_str!("publication_operations.sql"))
            .unwrap();
    }
    db.pragma_update(None, "application_id", APPLICATION_ID)
        .unwrap();
    db.pragma_update(None, "user_version", version).unwrap();
    db
}

fn rows(db: &Connection, table: &str, columns: &str) -> Vec<Vec<Value>> {
    let mut query = db
        .prepare(&format!("SELECT {columns} FROM {table} ORDER BY rowid"))
        .unwrap();
    let count = query.column_count();
    query
        .query_map([], |row| (0..count).map(|index| row.get(index)).collect())
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn seed_cost_facts(db: &Connection) {
    db.execute_batch(
        "INSERT INTO users(user_id,is_admin,password_hash) VALUES('admin',1,'saved-admin-hash'),('A',0,'saved-reader-hash');
         INSERT INTO auth_sessions VALUES(X'010203','A',0,9000);
         INSERT INTO book_publications VALUES('book','pub','fixture-directory','{}');
         INSERT INTO book_grants VALUES('A','book','pub');
         INSERT INTO book_defaults VALUES('book','pub');
         INSERT INTO reader_workspaces(owner_user_id,workspace_id,book_id,publication_id,selected_chat,checkpoint)
             VALUES('A','scene','book','pub','chat','saved-checkpoint');
         INSERT INTO allowance_periods(period_id,user_id,starts_at,expires_at,revision) VALUES('period','A',100,200,3);
         INSERT INTO admin_operations VALUES
             ('admin','payment','A','receipt','{\"amount_fen\":123}','{\"receipt_id\":\"receipt\"}',110),
             ('admin','correction','A','receipt_correction','{}','{}',111),
             ('admin','reconcile','A','charge_reconciliation','{}','{}',125);
         INSERT INTO manual_receipts(receipt_id,actor,operation_id,user_id,amount_fen,paid_at,external_ref,note)
             VALUES('receipt','admin','payment','A',123,109,'paid-1','confirmed receipt');
         INSERT INTO allowance_adjustments VALUES('admin','payment','A','period',4000000,'payment grant','receipt',110);
         INSERT INTO receipt_corrections VALUES('admin','correction','A','receipt',-1,'correct receipt',111);
         INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,run_ref,task_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_usage,provider_cost_micro_cny,provider_cost_status,account_debit_micro_cny,evidence_kind,created_at,settled_at,revision,needs_reconciliation,send_identity,send_outcome)
             VALUES('settled-call','logical-1',1,'A','period','turn',NULL,'chat','model','{}','{}','settled',1000,'{\"tokens\":11}',123,'confirmed',123,'usage',120,121,2,0,'{\"provider\":\"fixture\"}','{\"status\":\"completed\"}'),
                   ('pending-call','logical-2',2,'A','period',NULL,'memory-task','memory_review','model','{}','{}','pending',250,NULL,NULL,'pending',NULL,'awaiting_usage',122,NULL,1,1,'{\"provider\":\"fixture\"}',NULL);
         INSERT INTO charge_reports VALUES
             ('settled-call','{\"status\":\"completed\"}','accepted',121),
             ('pending-call','{\"status\":\"unknown\"}','conflict',123);
         INSERT INTO charge_reconciliations VALUES('admin','reconcile','A','settled-call','{\"cost\":120}','{\"cost\":123}',3,3,'provider correction','provider receipt',125);",
    ).unwrap();
}

#[test]
fn w1_q1_q2_schema14_upgrade_preserves_cost_receipts_and_reopens() {
    let root = tempfile::tempdir().unwrap();
    let db = historical_database(root.path(), 14);
    seed_cost_facts(&db);
    let tables = [
        "users",
        "auth_sessions",
        "book_publications",
        "book_grants",
        "book_defaults",
        "reader_workspaces",
        "allowance_periods",
        "admin_operations",
        "manual_receipts",
        "allowance_adjustments",
        "receipt_corrections",
        "model_call_charges",
        "charge_reports",
        "charge_reconciliations",
    ];
    let before: Vec<_> = tables
        .iter()
        .map(|table| {
            let query = db.prepare(&format!("SELECT * FROM {table}")).unwrap();
            let columns = std::iter::once("rowid")
                .chain(query.column_names())
                .collect::<Vec<_>>()
                .join(",");
            (*table, columns.clone(), rows(&db, table, &columns))
        })
        .collect();
    drop(db);
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    for _ in 0..2 {
        let store = ControlStore::open(writer.clone()).unwrap();
        assert_eq!(
            store
                .connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            CONTROL_SCHEMA_VERSION
        );
        for (table, columns, expected) in &before {
            assert_eq!(
                &rows(&store.connection, table, columns),
                expected,
                "historical {table}"
            );
        }
        assert_eq!(store.connection.query_row(
            "SELECT cost_owner,parent_task_ref,learning_workspace_id,related_learning_workspace_ids FROM model_call_charges WHERE call_id='settled-call'",
            [], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, String>(3)?)),
        ).unwrap(), ("account".into(), "turn".into(), None, "[]".into()));
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT parent_task_ref FROM model_call_charges WHERE call_id='pending-call'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "memory-task"
        );
        assert_eq!(
            store
                .connection
                .query_row("SELECT count(*) FROM allowance_period_bases", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert!(store.list_learning_workspaces("A").unwrap().is_empty());
        assert_eq!(
            store
                .connection
                .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            0
        );
    }
}

#[test]
fn w1_q1_q2_failed_schema15_upgrade_rolls_back_new_tables_and_old_facts() {
    let root = tempfile::tempdir().unwrap();
    let db = historical_database(root.path(), 14);
    seed_cost_facts(&db);
    let charges = rows(&db, "model_call_charges", "*");
    // A conflicting table fails after workspace DDL, proving the whole upgrade rolls back.
    db.execute_batch(
        "CREATE TABLE allowance_bases(keep TEXT); INSERT INTO allowance_bases VALUES('original');",
    )
    .unwrap();
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    assert!(
        matches!(ControlStore::open(writer.clone()), Err(error) if error.error_code == "CONTROL_STORAGE_UNAVAILABLE")
    );
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        14
    );
    assert_eq!(rows(&db, "model_call_charges", "*"), charges);
    assert!(db.prepare("SELECT * FROM learning_workspaces").is_err());
    assert_eq!(
        rows(&db, "allowance_bases", "*"),
        vec![vec![Value::Text("original".into())]]
    );
    db.execute_batch("DROP TABLE allowance_bases").unwrap();
    assert!(ControlStore::open(writer).is_ok());
}
