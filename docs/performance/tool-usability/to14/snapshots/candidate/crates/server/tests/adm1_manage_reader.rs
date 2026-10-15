use server::control_store::{ControlStore, ServiceWriter};
use std::process::{Command, Output};

fn manage(root: &std::path::Path, operation: &str, user: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_manage_reader"))
        .arg(root)
        .args([operation, user])
        .output()
        .unwrap()
}

#[test]
fn adm1_offline_role_command_requires_writer_and_preserves_reader_defaults() {
    let root = tempfile::tempdir().unwrap();
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    let mut control = ControlStore::open(writer.clone()).unwrap();
    control
        .provision_password("operator", "fixture-only-password", true)
        .unwrap();
    control
        .provision_password("reader", "fixture-only-password", true)
        .unwrap();
    let busy = manage(root.path(), "grant-admin", "operator");
    assert!(!busy.status.success());
    assert!(String::from_utf8_lossy(&busy.stderr).contains("SERVICE_WRITER_BUSY"));
    drop(control);
    drop(writer);
    let granted = manage(root.path(), "grant-admin", "operator");
    assert!(
        granted.status.success(),
        "{}",
        String::from_utf8_lossy(&granted.stderr)
    );
    let db = rusqlite::Connection::open(root.path().join("control.sqlite")).unwrap();
    let role = |id: &str| {
        db.query_row("SELECT is_admin FROM users WHERE user_id=?", [id], |r| {
            r.get::<_, bool>(0)
        })
        .unwrap()
    };
    assert!(role("operator"));
    assert!(!role("reader"));
    assert!(manage(root.path(), "revoke-admin", "operator")
        .status
        .success());
    assert!(!role("operator"));
    assert!(!manage(root.path(), "grant-admin", "missing")
        .status
        .success());
}
