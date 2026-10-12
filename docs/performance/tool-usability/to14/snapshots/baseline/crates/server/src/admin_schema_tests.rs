use super::*;

fn old_database(root: &Path, version: i64) -> Connection {
    let db = Connection::open(root.join("control.sqlite")).unwrap();
    db.execute_batch(include_str!("tests/control_schema_v4.sql"))
        .unwrap();
    if version < 4 {
        db.execute_batch(
            "ALTER TABLE run_admissions DROP COLUMN cancel_requested;
            ALTER TABLE run_admissions DROP COLUMN key_closed;
            ALTER TABLE run_admissions DROP COLUMN unsaved;",
        )
        .unwrap();
    }
    if version < 3 {
        db.execute_batch("ALTER TABLE reader_workspaces DROP COLUMN checkpoint_seq;")
            .unwrap();
    }
    if version == 1 {
        db.execute_batch("DROP TABLE book_defaults;").unwrap();
    }
    db.pragma_update(None, "application_id", APPLICATION_ID)
        .unwrap();
    db.pragma_update(None, "user_version", version).unwrap();
    db
}

fn snapshot(db: &Connection, table: &str, columns: &str) -> Vec<Vec<rusqlite::types::Value>> {
    let mut query = db
        .prepare(&format!("SELECT {columns} FROM {table} ORDER BY rowid"))
        .unwrap();
    let count = query.column_count();
    query
        .query_map([], |r| (0..count).map(|i| r.get(i)).collect())
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

const ADMIN_TABLES: &[&str] = &[
    "admin_operations",
    "allowance_periods",
    "manual_receipts",
    "allowance_adjustments",
    "model_call_charges",
];

#[test]
fn adm1_upgrades_every_supported_schema_and_preserves_existing_data_on_reopen() {
    for version in 1..=4 {
        let root = tempfile::tempdir().unwrap();
        let db = old_database(root.path(), version);
        db.execute_batch("INSERT INTO users VALUES('A',0,7,'saved-password-hash');
            INSERT INTO users VALUES('B',1,9,NULL);
            INSERT INTO auth_sessions VALUES(X'010203','A',7,9000);
            INSERT INTO book_publications VALUES('book','pub','fixture-directory','{}');
            INSERT INTO book_grants VALUES('A','book','pub');
            INSERT INTO reader_workspaces(owner_user_id,workspace_id,book_id,publication_id,selected_chat,generation,revision,checkpoint)
                VALUES('A','scene','book','pub','chat',2,3,'saved-checkpoint');
            INSERT INTO run_admissions(owner_user_id,client_request_id,turn_id,chat_session_id,workspace_id,workspace_generation,book_id,publication_id,dispatch_state,attempt)
                VALUES('A','request','turn','chat','scene',2,'book','pub','settled',2);").unwrap();
        if version >= 2 {
            db.execute_batch("INSERT INTO book_defaults VALUES('book','pub');")
                .unwrap();
        }
        let tables = [
            ("users", "user_id,disabled,auth_epoch,password_hash"),
            ("auth_sessions", "*"), ("book_publications", "*"), ("book_grants", "*"),
            ("reader_workspaces", "owner_user_id,workspace_id,book_id,publication_id,selected_chat,generation,revision,checkpoint"),
            ("run_admissions", "owner_user_id,client_request_id,turn_id,chat_session_id,workspace_id,workspace_generation,book_id,publication_id,dispatch_state,boot_id,attempt"),
        ];
        let before: Vec<_> = tables.iter().map(|(t, c)| snapshot(&db, t, c)).collect();
        let paths = UserStoragePaths::for_service(root.path(), "A").unwrap();
        std::fs::create_dir_all(paths.history.parent().unwrap()).unwrap();
        let history = b"{\"sessions\":[{\"id\":\"chat\",\"private\":\"preserve verbatim\"}]}\n";
        std::fs::write(&paths.history, history).unwrap();
        drop(db);
        let writer = ServiceWriter::acquire(root.path()).unwrap();
        for _ in 0..2 {
            let store = ControlStore::open(writer.clone()).unwrap();
            assert_eq!(
                store
                    .connection
                    .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                    .unwrap(),
                CONTROL_SCHEMA_VERSION
            );
            for ((table, columns), rows) in tables.iter().zip(&before) {
                assert_eq!(
                    &snapshot(&store.connection, table, columns),
                    rows,
                    "schema {version}, {table}"
                );
            }
            if version >= 2 {
                assert_eq!(snapshot(&store.connection, "book_defaults", "*").len(), 1);
            }
            assert_eq!(
                store
                    .connection
                    .query_row("SELECT sum(is_admin) FROM users", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            for table in ADMIN_TABLES {
                assert!(snapshot(&store.connection, table, "*").is_empty());
            }
            assert_eq!(std::fs::read(&paths.history).unwrap(), history);
        }
    }
}

#[test]
fn adm1_failed_upgrade_rolls_back_role_and_version() {
    let root = tempfile::tempdir().unwrap();
    let db = old_database(root.path(), 4);
    // Simulate a conflicting table so the migration fails after ALTER users.
    db.execute_batch(
        "CREATE TABLE manual_receipts(keep TEXT); INSERT INTO manual_receipts VALUES('original');",
    )
    .unwrap();
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    assert!(
        matches!(ControlStore::open(writer.clone()), Err(e) if e.error_code=="CONTROL_STORAGE_UNAVAILABLE")
    );
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        4
    );
    assert!(db.prepare("SELECT is_admin FROM users").is_err());
    assert_eq!(
        db.query_row("SELECT keep FROM manual_receipts", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "original"
    );
    db.execute_batch("DROP TABLE manual_receipts").unwrap();
    assert!(ControlStore::open(writer).is_ok());
}

#[test]
fn adm1_new_schema_enforces_ownership_money_periods_and_charge_facts() {
    let root = tempfile::tempdir().unwrap();
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    let mut store = ControlStore::open(writer.clone()).unwrap();
    for user in ["operator", "A", "B"] {
        store.create_user(user).unwrap();
    }
    let db = &store.connection;
    db.execute_batch("INSERT INTO allowance_periods(period_id,user_id,starts_at,expires_at) VALUES('pA','A',100,200),('pB','B',100,200),('next','A',200,300);
        INSERT INTO admin_operations VALUES('operator','receipt-op','A','receipt','{\"amount_fen\":123,\"delta_micro_cny\":4000000}','{}',110);
        INSERT INTO admin_operations VALUES('operator','gift-op','B','gift','{}','{}',110);
        INSERT INTO manual_receipts(receipt_id,actor,operation_id,user_id,amount_fen,paid_at,external_ref,note) VALUES('r','operator','receipt-op','A',123,109,'wx-1','confirmed');
        INSERT INTO allowance_adjustments VALUES('operator','receipt-op','A','pA',4000000,'payment grant','r',110);
        INSERT INTO allowance_adjustments VALUES('operator','gift-op','B','pB',500000,'gift',NULL,110);
        INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,run_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_cost_status,evidence_kind,created_at)
            VALUES('call','logical',1,'A','pA','turn','chat','model','{}','{\"source\":\"test\"}','sent',1000000,'pending','awaiting_usage',120);").unwrap();
    for sql in [
        "UPDATE users SET is_admin=2 WHERE user_id='A'",
        "INSERT INTO allowance_periods(period_id,user_id,starts_at,expires_at) VALUES('overlap','A',199,201)",
        "UPDATE allowance_periods SET starts_at=199 WHERE period_id='next'",
        "UPDATE allowance_periods SET expires_at=starts_at WHERE period_id='pA'",
        "UPDATE manual_receipts SET user_id='B' WHERE receipt_id='r'",
        "UPDATE manual_receipts SET amount_fen=1.5 WHERE receipt_id='r'",
        "UPDATE manual_receipts SET amount_fen=0 WHERE receipt_id='r'",
        "UPDATE allowance_adjustments SET period_id='pB' WHERE user_id='A'",
        "UPDATE allowance_adjustments SET receipt_id='r' WHERE user_id='B'",
        "INSERT INTO admin_operations SELECT * FROM admin_operations WHERE operation_id='receipt-op'",
        "INSERT INTO allowance_adjustments SELECT * FROM allowance_adjustments WHERE user_id='A'",
        "UPDATE model_call_charges SET user_id='B' WHERE call_id='call'",
        "UPDATE model_call_charges SET provider_cost_micro_cny=0 WHERE call_id='call'",
        "UPDATE model_call_charges SET state='settled' WHERE call_id='call'",
        "UPDATE model_call_charges SET reserved_micro_cny=-1 WHERE call_id='call'",
        "UPDATE model_call_charges SET run_ref=NULL WHERE call_id='call'",
        "INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,run_ref,task_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_usage,provider_cost_micro_cny,provider_cost_status,account_debit_micro_cny,evidence_kind,created_at,settled_at) SELECT 'duplicate',logical_call_id,attempt,user_id,period_id,run_ref,task_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_usage,provider_cost_micro_cny,provider_cost_status,account_debit_micro_cny,evidence_kind,created_at,settled_at FROM model_call_charges",
    ] { assert!(db.execute_batch(sql).is_err(), "must reject: {sql}"); }
    db.execute_batch("INSERT INTO admin_operations VALUES('operator','second-receipt','A','receipt','{}','{}',130);").unwrap();
    assert!(db.execute_batch("INSERT INTO manual_receipts(receipt_id,actor,operation_id,user_id,amount_fen,paid_at,external_ref,note) VALUES('r2','operator','second-receipt','A',100,130,'wx-1','duplicate');").is_err());
    // Nullable external references allow manually verified payments without IDs.
    db.execute_batch("UPDATE manual_receipts SET external_ref=NULL;
        INSERT INTO manual_receipts(receipt_id,actor,operation_id,user_id,amount_fen,paid_at,note) VALUES('r2','operator','second-receipt','A',100,130,'no reference');
        UPDATE allowance_periods SET closed_at=150 WHERE period_id='pA';
        UPDATE model_call_charges SET state='pending' WHERE call_id='call';
        UPDATE model_call_charges SET state='released',account_debit_micro_cny=0,settled_at=151,evidence_kind='manual_waiver' WHERE call_id='call';").unwrap();
    // Waiving the account debit does not invent zero supplier cost.
    assert_eq!(
        db.query_row(
            "SELECT provider_cost_micro_cny FROM model_call_charges",
            [],
            |r| r.get::<_, Option<i64>>(0)
        )
        .unwrap(),
        None
    );
    let before: Vec<_> = ADMIN_TABLES.iter().map(|t| snapshot(db, t, "*")).collect();
    drop(store);
    let reopened = ControlStore::open(writer).unwrap();
    for (table, rows) in ADMIN_TABLES.iter().zip(before) {
        assert_eq!(snapshot(&reopened.connection, table, "*"), rows);
    }
}
