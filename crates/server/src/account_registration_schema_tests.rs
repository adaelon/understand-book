use super::*;
use rusqlite::types::Value;

// Build historical schemas from the frozen v4 fixture, never from today's users.
fn old_database(root: &Path, version: i64) -> Connection {
    let db = Connection::open(root.join("control.sqlite")).unwrap();
    db.execute_batch(include_str!("tests/control_schema_v4.sql"))
        .unwrap();
    db.execute_batch("ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0 CHECK(is_admin IN (0,1));")
        .unwrap();
    db.execute_batch(include_str!("admin_schema.sql")).unwrap();
    if version >= 6 {
        db.execute_batch(include_str!("receipt_corrections.sql"))
            .unwrap();
    }
    if version >= 7 {
        db.execute_batch(include_str!("charge_settlement.sql"))
            .unwrap();
    }
    if version >= 8 {
        db.execute_batch(include_str!("usage_schema.sql")).unwrap();
    }
    db.pragma_update(None, "application_id", APPLICATION_ID)
        .unwrap();
    db.pragma_update(None, "user_version", version).unwrap();
    db
}

fn rows(db: &Connection, sql: &str) -> Vec<Vec<Value>> {
    let mut query = db.prepare(sql).unwrap();
    let count = query.column_count();
    query
        .query_map([], |r| (0..count).map(|i| r.get(i)).collect())
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn schema_definitions(db: &Connection) -> Vec<(String, String)> {
    db.prepare("SELECT name,sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY name")
        .unwrap()
        .query_map([], |r| {
            let sql: String = r.get(1)?;
            // ALTER TABLE changes whitespace in the stored CREATE TABLE text.
            Ok((
                r.get(0)?,
                sql.chars().filter(|c| !c.is_whitespace()).collect(),
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn assert_constraint(db: &Connection, sql: &str) {
    let err = db.execute_batch(sql).expect_err(sql);
    assert_eq!(
        err.sqlite_error_code(),
        Some(rusqlite::ErrorCode::ConstraintViolation),
        "{sql}: {err}"
    );
}

#[test]
fn inv1_upgrades_preserve_old_fields_and_match_fresh_schema_on_reopen() {
    let fresh_root = tempfile::tempdir().unwrap();
    let fresh = ControlStore::open(ServiceWriter::acquire(fresh_root.path()).unwrap()).unwrap();
    assert_eq!(CONTROL_SCHEMA_VERSION, 9);
    let expected_schema = schema_definitions(&fresh.connection);
    for version in 5..=8 {
        let root = tempfile::tempdir().unwrap();
        let db = old_database(root.path(), version);
        db.execute_batch("INSERT INTO users VALUES('admin',0,7,'saved-admin-hash',1),('reader',0,3,'saved-reader-hash',0),('disabled',1,9,NULL,0);
            INSERT INTO auth_sessions VALUES(X'010203','reader',3,9000);
            INSERT INTO book_publications VALUES('book','pub','fixture-directory','{}');
            INSERT INTO book_grants VALUES('reader','book','pub');
            INSERT INTO book_defaults VALUES('book','pub');
            INSERT INTO reader_workspaces(owner_user_id,workspace_id,book_id,publication_id,selected_chat,generation,revision,checkpoint)
                VALUES('reader','scene','book','pub','chat',2,3,'saved-checkpoint');
            INSERT INTO run_admissions(owner_user_id,client_request_id,turn_id,chat_session_id,workspace_id,workspace_generation,book_id,publication_id,dispatch_state)
                VALUES('reader','request','turn','chat','scene',2,'book','pub','settled');
            INSERT INTO admin_operations VALUES('admin','receipt','reader','receipt','{}','{}',110);
            INSERT INTO manual_receipts VALUES('r','admin','receipt','reader',123,109,'wechat','wx-1','confirmed');
            INSERT INTO allowance_periods(period_id,user_id,starts_at,expires_at) VALUES('p','reader',100,200);
            INSERT INTO allowance_adjustments VALUES('admin','receipt','reader','p',4000000,'payment grant','r',110);
            INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,run_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_cost_status,evidence_kind,created_at)
                VALUES('call','logical',1,'reader','p','turn','chat','model','{}','{}','pending',1000000,'pending','awaiting_usage',120);")
            .unwrap();
        if version >= 6 {
            db.execute_batch("INSERT INTO admin_operations VALUES('admin','correction','reader','correction','{}','{}',121);
                INSERT INTO receipt_corrections VALUES('admin','correction','reader','r',-1,'corrected',121);").unwrap();
        }
        if version >= 7 {
            db.execute_batch("INSERT INTO charge_reports VALUES('call','{}','accepted',122);
                INSERT INTO admin_operations VALUES('admin','reconcile','reader','reconcile','{}','{}',123);
                INSERT INTO charge_reconciliations VALUES('admin','reconcile','reader','call','{}','{}',0,NULL,'note','evidence',123);").unwrap();
        }
        if version >= 8 {
            db.execute_batch(
                "INSERT INTO admin_usage_events VALUES('reader','read','book:pub',124)",
            )
            .unwrap();
        }
        // Save every pre-existing column value, including credentials and ADM facts.
        let tables: Vec<String> = db
            .prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap().query_map([], |r| r.get(0)).unwrap()
            .collect::<rusqlite::Result<_>>().unwrap();
        let before: Vec<_> = tables
            .iter()
            .map(|table| {
                let columns: Vec<String> = db
                    .prepare(&format!("PRAGMA table_info({table})"))
                    .unwrap()
                    .query_map([], |r| r.get(1))
                    .unwrap()
                    .collect::<rusqlite::Result<_>>()
                    .unwrap();
                let sql = format!("SELECT {} FROM {table} ORDER BY rowid", columns.join(","));
                let values = rows(&db, &sql);
                (sql, values)
            })
            .collect();
        let paths =
            UserStoragePaths::for_service(&root.path().canonicalize().unwrap(), "reader").unwrap();
        std::fs::create_dir_all(paths.history.parent().unwrap()).unwrap();
        let history = b"{\"sessions\":[{\"id\":\"chat\"}]}\n";
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
                9
            );
            assert_eq!(
                schema_definitions(&store.connection),
                expected_schema,
                "schema {version}"
            );
            for (sql, values) in &before {
                assert_eq!(
                    &rows(&store.connection, sql),
                    values,
                    "schema {version}: {sql}"
                );
            }
            assert_eq!(
                rows(
                    &store.connection,
                    "SELECT email,email_verified_at FROM users"
                ),
                vec![vec![Value::Null, Value::Null]; 3]
            );
            assert_eq!(store.user_paths("reader").unwrap().history, paths.history);
            assert_eq!(std::fs::read(&paths.history).unwrap(), history);
        }
    }
}

#[test]
fn inv1_email_invite_and_request_constraints_persist_on_reopen() {
    for upgrade in [false, true] {
        let root = tempfile::tempdir().unwrap();
        if upgrade {
            drop(old_database(root.path(), 8));
        }
        let writer = ServiceWriter::acquire(root.path()).unwrap();
        let store = ControlStore::open(writer.clone()).unwrap();
        let db = &store.connection;
        db.execute_batch("INSERT INTO users(user_id) VALUES('admin'),('A'),('B');
            UPDATE users SET email='reader@example.com',email_verified_at=100 WHERE user_id='A';
            INSERT INTO invite_batches VALUES('admin','batch',3,100);
            INSERT INTO beta_invites(invite_id,code,actor,operation_id,state,created_at) VALUES
                ('i1','ABCDEABCDEABCDEABCDE','admin','batch','unused',100),
                ('i2','BCDEFBCDEFBCDEFBCDEF','admin','batch','unused',100),
                ('i3','CDEFGCDEFGCDEFGCDEFG','admin','batch','unused',100);
            UPDATE beta_invites SET state='used',used_by='A',used_at=110 WHERE invite_id='i2';
            UPDATE beta_invites SET state='disabled',disabled_at=120 WHERE invite_id='i3';
            INSERT INTO registration_requests(request_id,email,password_hash,invite_id,verification_code,expires_at) VALUES
                ('r1','new@example.com','argon2-fixture','i1','012345',1000),
                ('r2','new@example.com','argon2-fixture','i1','234567',1000);
            INSERT INTO email_binding_requests(request_id,owner_user_id,auth_epoch,email,verification_code,expires_at) VALUES('bind','B',3,'new@example.com','345678',1000);
            INSERT INTO password_reset_requests(token,owner_user_id,auth_epoch,expires_at) VALUES('random-token','A',7,2000);")
            .unwrap();
        for sql in [
            "UPDATE users SET email='reader@example.com',email_verified_at=101 WHERE user_id='B'",
            "UPDATE users SET email='Reader@example.com',email_verified_at=101 WHERE user_id='B'",
            "UPDATE users SET email=' reader@example.com ',email_verified_at=101 WHERE user_id='B'",
            "UPDATE users SET email='',email_verified_at=101 WHERE user_id='B'",
            "UPDATE users SET email='b@example.com' WHERE user_id='B'",
            "UPDATE users SET email_verified_at=101 WHERE user_id='B'",
            "INSERT INTO invite_batches VALUES('admin','batch',3,100)",
            "INSERT INTO invite_batches VALUES('missing','batch',1,100)",
            "UPDATE invite_batches SET count=0",
            "UPDATE invite_batches SET count=101",
            "UPDATE beta_invites SET code='ABCDEABCDEABCDEABCDE' WHERE invite_id='i2'",
            "UPDATE beta_invites SET operation_id='missing' WHERE invite_id='i1'",
            "UPDATE beta_invites SET state='invalid' WHERE invite_id='i1'",
            "UPDATE beta_invites SET state='used' WHERE invite_id='i1'",
            "UPDATE beta_invites SET state='used',used_by='A' WHERE invite_id='i1'",
            "UPDATE beta_invites SET state='used',used_at=120 WHERE invite_id='i1'",
            "UPDATE beta_invites SET used_at=120 WHERE invite_id='i1'",
            "UPDATE beta_invites SET used_by='A' WHERE invite_id='i1'",
            "UPDATE beta_invites SET disabled_at=120 WHERE invite_id='i1'",
            "UPDATE beta_invites SET state='disabled' WHERE invite_id='i1'",
            "UPDATE beta_invites SET disabled_at=120 WHERE invite_id='i2'",
            "UPDATE beta_invites SET used_by='missing' WHERE invite_id='i2'",
            "UPDATE beta_invites SET used_at=120 WHERE invite_id='i3'",
            "UPDATE beta_invites SET used_by='A' WHERE invite_id='i3'",
            "UPDATE registration_requests SET invite_id='missing' WHERE request_id='r1'",
            "UPDATE registration_requests SET password_hash=NULL WHERE request_id='r1'",
            "UPDATE registration_requests SET verification_code=NULL WHERE request_id='r1'",
            "UPDATE registration_requests SET completed_user_id='A' WHERE request_id='r1'",
            "UPDATE registration_requests SET failed_attempts=-1 WHERE request_id='r1'",
            "UPDATE email_binding_requests SET owner_user_id='missing'",
            "UPDATE email_binding_requests SET verification_code=NULL",
            "UPDATE email_binding_requests SET failed_attempts=-1",
            "UPDATE password_reset_requests SET owner_user_id='missing'",
            "INSERT INTO password_reset_requests SELECT * FROM password_reset_requests",
        ] {
            assert_constraint(db, sql);
        }
        // Two pending requests can share an email and invite; neither reserves them.
        assert_eq!(
            rows(db, "SELECT state FROM beta_invites WHERE invite_id='i1'"),
            vec![vec![Value::Text("unused".into())]]
        );
        db.execute_batch("UPDATE users SET email='new@example.com',email_verified_at=130 WHERE user_id='B';
            UPDATE beta_invites SET state='used',used_by='B',used_at=130 WHERE invite_id='i1';
            UPDATE registration_requests SET password_hash=NULL,verification_code=NULL,completed_user_id='B' WHERE request_id='r1';
            UPDATE registration_requests SET failed_attempts=2 WHERE request_id='r2';
            UPDATE email_binding_requests SET failed_attempts=1,used_at=140;
            UPDATE password_reset_requests SET used_at=150;").unwrap();
        assert_constraint(
            db,
            "UPDATE registration_requests SET completed_user_id='missing' WHERE request_id='r1'",
        );
        let tables = [
            "users",
            "invite_batches",
            "beta_invites",
            "registration_requests",
            "email_binding_requests",
            "password_reset_requests",
        ];
        let before: Vec<_> = tables
            .iter()
            .map(|t| rows(db, &format!("SELECT * FROM {t} ORDER BY rowid")))
            .collect();
        drop(store);
        let reopened = ControlStore::open(writer).unwrap();
        for (table, expected) in tables.iter().zip(before) {
            assert_eq!(
                rows(
                    &reopened.connection,
                    &format!("SELECT * FROM {table} ORDER BY rowid")
                ),
                expected
            );
        }
        // Completed receipts and used requests remain deletable by later cleanup.
        reopened.connection.execute_batch("DELETE FROM registration_requests WHERE request_id='r1'; DELETE FROM email_binding_requests; DELETE FROM password_reset_requests;").unwrap();
        assert_eq!(
            rows(
                &reopened.connection,
                "SELECT used_by FROM beta_invites WHERE invite_id='i1'"
            ),
            vec![vec![Value::Text("B".into())]]
        );
    }
}

#[test]
fn inv1_failed_upgrade_rolls_back_email_tables_and_version() {
    let root = tempfile::tempdir().unwrap();
    let db = old_database(root.path(), 8);
    db.execute_batch("INSERT INTO users(user_id) VALUES('old'); CREATE TABLE password_reset_requests(keep TEXT); INSERT INTO password_reset_requests VALUES('original');").unwrap();
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    assert!(
        matches!(ControlStore::open(writer.clone()), Err(e) if e.error_code == "CONTROL_STORAGE_UNAVAILABLE")
    );
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        8
    );
    assert!(db.prepare("SELECT email FROM users").is_err());
    assert!(db.prepare("SELECT * FROM invite_batches").is_err());
    assert_eq!(
        rows(&db, "SELECT keep FROM password_reset_requests"),
        vec![vec![Value::Text("original".into())]]
    );
    db.execute_batch("DROP TABLE password_reset_requests")
        .unwrap();
    let store = ControlStore::open(writer).unwrap();
    assert_eq!(
        rows(&store.connection, "SELECT user_id,email FROM users"),
        vec![vec![Value::Text("old".into()), Value::Null]]
    );
}
