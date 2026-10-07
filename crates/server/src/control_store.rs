//! Service metadata only. The held OS lock covers database and private JSON writers.
use crate::user_storage_paths::{error, validate_user_id, UserStoragePaths};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

pub const CONTROL_SCHEMA_VERSION: i64 = 4;
const APPLICATION_ID: i64 = 0x55424d55;

pub struct ServiceWriter {
    root: PathBuf,
    // Never unlink the lock file: a second inode would allow two writers.
    _lock: File,
}

impl ServiceWriter {
    /// Used by the service and offline maintenance tools, before opening any store.
    pub fn acquire(root: &Path) -> Result<Arc<Self>, ToolError> {
        let writer = Self::acquire_maintenance(root)?;
        if writer.root.join("maintenance.json").exists() {
            return Err(error("SERVICE_MAINTENANCE_INCOMPLETE", "conflict", "Resume the recorded offline maintenance before starting this root"));
        }
        Ok(writer)
    }

    pub(crate) fn acquire_maintenance(root: &Path) -> Result<Arc<Self>, ToolError> {
        if !root.is_absolute() {
            return Err(error(
                "SERVICE_ROOT_INVALID",
                "validation",
                "Service root must be an absolute local directory",
            ));
        }
        memory::ReaderPrivateStorageGate::create_dir_all(root).map_err(|_| storage())?;
        let root = root.canonicalize().map_err(|_| storage())?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("service.lock"))
            .map_err(|_| storage())?;
        lock.try_lock().map_err(|_| {
            error(
                "SERVICE_WRITER_BUSY",
                "conflict",
                "Service root already has a writer or cannot be locked",
            )
        })?;
        memory::ReaderPrivateStorageGate::secure_directory(&root)?;
        memory::ReaderPrivateStorageGate::secure_file(&root.join("service.lock"))?;
        Ok(Arc::new(Self { root, _lock: lock }))
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub(crate) fn paths(&self, owner: &str) -> Result<UserStoragePaths, ToolError> {
        UserStoragePaths::for_service(&self.root, owner)
    }
}

pub struct ControlStore {
    pub(crate) connection: Connection,
    pub(crate) writer: Arc<ServiceWriter>,
}

fn storage() -> ToolError {
    error(
        "CONTROL_STORAGE_UNAVAILABLE",
        "unavailable",
        "Service metadata could not be read or committed",
    )
}
fn schema() -> ToolError {
    error(
        "CONTROL_SCHEMA_INCOMPATIBLE",
        "conflict",
        "Service schema requires a compatible binary or an explicit migration",
    )
}

impl ControlStore {
    pub fn open(writer: Arc<ServiceWriter>) -> Result<Self, ToolError> {
        // Only one connection belongs to this store; future additional connections must use this setup.
        if rusqlite::version_number() < 3_051_003 {
            return Err(error(
                "SQLITE_VERSION_UNSUPPORTED",
                "unavailable",
                "Service WAL requires SQLite 3.51.3 or later",
            ));
        }
        let mut connection =
            Connection::open(writer.root.join("control.sqlite")).map_err(|_| storage())?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|_| storage())?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(|_| storage())?;
        let application: i64 = connection
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .map_err(|_| storage())?;
        let tables: i64 = connection.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'", [], |r| r.get(0)).map_err(|_| storage())?;
        if (version == 0 && (tables != 0 || application != 0))
            || (version != 0
                && (!(1..=CONTROL_SCHEMA_VERSION).contains(&version) || application != APPLICATION_ID))
        {
            return Err(schema());
        }
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|_| storage())?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|_| storage())?;
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(|_| storage())?;
        let journal: String = connection
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .map_err(|_| storage())?;
        let sync: i64 = connection
            .pragma_query_value(None, "synchronous", |r| r.get(0))
            .map_err(|_| storage())?;
        let foreign: i64 = connection
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .map_err(|_| storage())?;
        if journal != "wal" || sync != 2 || foreign != 1 {
            return Err(storage());
        }
        if version == 0 {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| storage())?;
            tx.execute_batch(include_str!("control_schema.sql"))
                .map_err(|_| storage())?;
            tx.pragma_update(None, "application_id", APPLICATION_ID)
                .map_err(|_| storage())?;
            tx.pragma_update(None, "user_version", CONTROL_SCHEMA_VERSION)
                .map_err(|_| storage())?;
            tx.commit().map_err(|_| storage())?;
        }
        if version == 1 || version == 2 {
            let tx = connection.transaction().map_err(|_| storage())?;
            if version == 1 {
                tx.execute_batch(include_str!("publication_defaults.sql")).map_err(|_| storage())?;
            }
            tx.execute_batch("ALTER TABLE reader_workspaces ADD COLUMN checkpoint_seq INTEGER NOT NULL DEFAULT 0;").map_err(|_| storage())?;
            tx.pragma_update(None, "user_version", 3).map_err(|_| storage())?;
            tx.commit().map_err(|_| storage())?;
        }
        if (1..=3).contains(&version) {
            let tx = connection.transaction().map_err(|_| storage())?;
            tx.execute_batch(include_str!("run_admission_schema.sql")).map_err(|_| storage())?;
            tx.pragma_update(None, "user_version", CONTROL_SCHEMA_VERSION).map_err(|_| storage())?;
            tx.commit().map_err(|_| storage())?;
        }
        memory::ReaderPrivateStorageGate::secure_file(&writer.root.join("control.sqlite"))?;
        eprintln!("Service SQLite {}: WAL, synchronous=FULL, foreign_keys=ON, busy_timeout=5000ms, schema={CONTROL_SCHEMA_VERSION}", rusqlite::version());
        Ok(Self { connection, writer })
    }

    /// Trusted identity provisioning without login credentials. Administrators use
    /// provision_password to create an account that can sign in.
    pub fn create_user(&mut self, owner: &str) -> Result<(), ToolError> {
        validate_user_id(owner)?;
        self.connection
            .execute("INSERT INTO users(user_id) VALUES(?)", [owner])
            .map_err(|e| {
                if e.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) {
                    error("USER_ALREADY_EXISTS", "conflict", "User already exists")
                } else {
                    storage()
                }
            })?;
        Ok(())
    }

    pub(crate) fn user_paths(&self, owner: &str) -> Result<UserStoragePaths, ToolError> {
        validate_user_id(owner)?;
        let enabled: Option<bool> = self
            .connection
            .query_row(
                "SELECT disabled=0 FROM users WHERE user_id=?",
                params![owner],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| storage())?;
        if enabled != Some(true) {
            return Err(error(
                "USER_UNAVAILABLE",
                "not_found",
                "User is unavailable",
            ));
        }
        self.writer.paths(owner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mu2_writer_child() {
        let Some(root) = std::env::var_os("MU2_LOCK_TEST_ROOT") else {
            return;
        };
        if std::env::var_os("MU2_LOCK_EXIT").is_some() {
            let _writer = ServiceWriter::acquire(Path::new(&root)).unwrap();
            std::process::exit(0); // no Drop: OS must release the lock
        }
        assert!(
            matches!(ServiceWriter::acquire(Path::new(&root)), Err(e) if e.error_code == "SERVICE_WRITER_BUSY")
        );
    }

    #[test]
    fn mu2_exclusive_writer_survives_handles_and_releases_on_process_exit() {
        let root = tempfile::tempdir().unwrap();
        let mut registry = crate::user_registry::UserRegistry::open(root.path()).unwrap();
        registry.create_user("A").unwrap();
        let user = registry.get("A", "1").unwrap();
        drop(registry);
        assert!(ServiceWriter::acquire(root.path()).is_err());
        let child = || {
            let mut c = std::process::Command::new(std::env::current_exe().unwrap());
            c.args([
                "--exact",
                "control_store::tests::mu2_writer_child",
                "--nocapture",
            ])
            .env("MU2_LOCK_TEST_ROOT", root.path());
            c
        };
        assert!(child().status().unwrap().success());
        let learning = user.lock().unwrap().learning_store().unwrap();
        drop(user);
        assert!(ServiceWriter::acquire(root.path()).is_err());
        drop(learning);
        assert!(child()
            .env("MU2_LOCK_EXIT", "1")
            .status()
            .unwrap()
            .success());
        assert!(ServiceWriter::acquire(root.path()).is_ok());
    }

    #[test]
    fn mu2_control_schema_pragmas_owner_constraints_and_reopen() {
        let root = tempfile::tempdir().unwrap();
        let writer = ServiceWriter::acquire(root.path()).unwrap();
        let mut store = ControlStore::open(writer.clone()).unwrap();
        eprintln!(
            "SQLite {} / WAL / FULL / foreign_keys=ON / busy_timeout=5000",
            rusqlite::version()
        );
        for user in ["A", "B"] {
            store.create_user(user).unwrap();
        }
        store
            .connection
            .execute_batch(
                "INSERT INTO book_publications VALUES('book','pub','test-directory','{}');
          INSERT INTO reader_workspaces(owner_user_id,workspace_id) VALUES('A','workspace');
          INSERT INTO reader_workspaces(owner_user_id,workspace_id) VALUES('B','workspace');",
            )
            .unwrap();
        for owner in ["A", "B"] {
            store.connection.execute("INSERT INTO run_admissions(owner_user_id,client_request_id,turn_id,chat_session_id,workspace_id,workspace_generation,book_id,publication_id,dispatch_state) VALUES(?,'key','turn','chat','workspace',0,'book','pub','preparing')", [owner]).unwrap();
        }
        assert!(store
            .connection
            .execute(
                "INSERT INTO run_admissions SELECT * FROM run_admissions WHERE owner_user_id='A'",
                []
            )
            .is_err());
        assert!(store
            .connection
            .execute("UPDATE run_admissions SET workspace_id='missing'", [])
            .is_err());
        assert!(store
            .connection
            .execute(
                "INSERT INTO reader_workspaces(owner_user_id,workspace_id) VALUES('missing','w')",
                []
            )
            .is_err());
        assert_eq!(
            store
                .connection
                .pragma_query_value(None, "busy_timeout", |r| r.get::<_, i64>(0))
                .unwrap(),
            5000
        );
        drop(store);
        let store = ControlStore::open(writer).unwrap();
        assert_eq!(
            store
                .connection
                .query_row("SELECT count(*) FROM run_admissions", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            2
        );
    }

    #[test]
    fn mu2_control_future_or_unversioned_foreign_schema_is_not_rewritten() {
        for future in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("control.sqlite");
            let connection = Connection::open(&path).unwrap();
            if future {
                connection.pragma_update(None, "user_version", 999).unwrap();
            } else {
                connection.execute_batch("CREATE TABLE foreign_data(value TEXT); INSERT INTO foreign_data VALUES('keep');").unwrap();
            }
            drop(connection);
            let before = std::fs::read(&path).unwrap();
            let writer = ServiceWriter::acquire(root.path()).unwrap();
            assert!(
                matches!(ControlStore::open(writer), Err(e) if e.error_code == "CONTROL_SCHEMA_INCOMPATIBLE")
            );
            assert_eq!(std::fs::read(path).unwrap(), before);
        }
    }

    #[test]
    fn mu2_database_busy_returns_storage_error_without_partial_user() {
        let root = tempfile::tempdir().unwrap();
        let writer = ServiceWriter::acquire(root.path()).unwrap();
        let mut store = ControlStore::open(writer.clone()).unwrap();
        let blocker = Connection::open(writer.root.join("control.sqlite")).unwrap();
        blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
        let start = std::time::Instant::now();
        assert_eq!(
            store.create_user("A").unwrap_err().error_code,
            "CONTROL_STORAGE_UNAVAILABLE"
        );
        assert!(
            start.elapsed() < Duration::from_secs(7),
            "busy wait exceeded the configured bound"
        );
        blocker.execute_batch("ROLLBACK").unwrap();
        store.create_user("A").unwrap();
        assert!(store.user_paths("A").is_ok());
    }
}
