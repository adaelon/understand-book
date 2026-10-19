use super::*;
use crate::{control_store::{ControlStore, ServiceWriter}, online_publication::PublicationRequest,
    published_library::PublishedLibrary};
use super::mu3_tests::material;

fn request(input: &Path) -> PublicationRequest {
    PublicationRequest { operation_id: "publish-one".into(), input_directory: input.to_owned(), grant_users: vec!["A".into(), "B".into()] }
}

#[test]
fn online_publication_commits_grants_together_and_retry_survives_reopen() {
    let root = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    material(input.path(), "online", b"image");
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    let mut control = ControlStore::open(writer.clone()).unwrap();
    for user in ["A", "B", "C"] { control.create_user(user).unwrap(); }
    let mut reader = PublishedLibrary::new(ControlStore::open(writer.clone()).unwrap());
    let old = reader.publish(input.path()).unwrap();
    reader.grant("A", &old.reference).unwrap();
    let pinned = reader.load("A", &old.reference).unwrap();
    let mut publisher = PublishedLibrary::new(control);
    let request = request(input.path());
    let new = publisher.publish_online(&request).unwrap();
    assert_ne!(new.reference, old.reference);
    assert_eq!(reader.default_ref("online").unwrap(), new.reference);
    for user in ["A", "B"] { assert!(reader.load(user, &new.reference).is_ok()); }
    assert!(reader.load("C", &new.reference).is_err());
    assert_eq!(pinned.reference, old.reference);
    assert_eq!(pinned.asset("assets/images/same.png").unwrap().body, b"image");
    let mut changed = request.clone(); changed.grant_users = vec!["C".into()];
    assert_eq!(publisher.publish_online(&changed).unwrap_err().error_code, "PUBLICATION_OPERATION_CONFLICT");
    drop(publisher);
    // A retry is a lookup even if the upload was cleaned up, and must not regrant revoked access.
    reader.revoke("B", &new.reference).unwrap();
    drop(input);
    let mut reopened = PublishedLibrary::new(ControlStore::open(writer.clone()).unwrap());
    assert_eq!(reopened.publish_online(&request).unwrap().reference, new.reference);
    assert!(reader.load("B", &new.reference).is_err());
    let mut control = ControlStore::open(writer).unwrap();
    control.create_user("future").unwrap();
    assert!(reader.list("future").unwrap().is_empty());
    assert_eq!(control.connection.query_row("SELECT count(*) FROM book_publications", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
}

#[test]
fn online_publication_failed_grant_rolls_back_publication_default_and_receipt() {
    let root = tempfile::tempdir().unwrap(); let input = tempfile::tempdir().unwrap();
    material(input.path(), "atomic", b"image");
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    let mut control = ControlStore::open(writer.clone()).unwrap();
    for user in ["A", "B"] { control.create_user(user).unwrap(); }
    let mut publisher = PublishedLibrary::new(ControlStore::open(writer).unwrap());
    let old = publisher.publish(input.path()).unwrap();
    control.connection.execute_batch("CREATE TRIGGER fail_second_grant BEFORE INSERT ON book_grants WHEN NEW.owner_user_id='B' BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    let request = request(input.path());
    assert!(publisher.publish_online(&request).is_err());
    assert_eq!(publisher.default_ref("atomic").unwrap(), old.reference);
    for (table, expected) in [("book_publications", 1), ("book_grants", 0), ("publication_operations", 0)] {
        assert_eq!(control.connection.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get::<_, i64>(0)).unwrap(), expected);
    }
    control.connection.execute_batch("DROP TRIGGER fail_second_grant").unwrap();
    assert!(publisher.publish_online(&request).is_ok());
}

#[test]
fn online_publication_schema13_upgrade_preserves_existing_catalog() {
    let root = tempfile::tempdir().unwrap(); let input = tempfile::tempdir().unwrap();
    material(input.path(), "upgrade", b"image");
    let connection = crate::control_store::workspace_quota_schema_tests::historical_database(root.path(), 13);
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    let mut control = ControlStore { connection, writer: writer.clone() };
    control.create_user("A").unwrap();
    let mut library = PublishedLibrary::new(control);
    let old = library.publish(input.path()).unwrap(); library.grant("A", &old.reference).unwrap();
    drop(library);
    let control = ControlStore::open(writer).unwrap();
    assert_eq!(control.connection.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), crate::control_store::CONTROL_SCHEMA_VERSION);
    assert_eq!(control.connection.query_row("SELECT count(*) FROM publication_operations", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    let library = PublishedLibrary::new(control);
    assert_eq!(library.default_ref("upgrade").unwrap(), old.reference);
    assert_eq!(library.list("A").unwrap().len(), 1);
}

#[cfg(unix)]
#[test]
fn online_publication_socket_keeps_http_readable_during_import_and_replays_after_restart() {
    use crate::{authorization::Authorization, multi_user_host::{self, Site}, user_registry::UserRegistry};
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap(); let input = tempfile::tempdir().unwrap();
    material(input.path(), "live", b"image");
    let registry = UserRegistry::open(root.path()).unwrap();
    let mut control = ControlStore::open(registry.writer()).unwrap();
    for user in ["A", "B"] { control.provision_password(user, "fixture-password", true).unwrap(); }
    let access = Arc::new(Authorization::new(registry).unwrap());
    let start = || multi_user_host::start_with_access("127.0.0.1:0".parse().unwrap(), Arc::new(Site::new("https://reader.example").unwrap()), access.clone()).unwrap();
    let server = start();
    let token = access.auth.login("A", "fixture-password", None, multi_user_host::now()).unwrap();
    let get_catalog = || ureq::get(&format!("{}/api/library", server.url)).set("Host", "reader.example")
        .set("Cookie", &format!("{}={token}", crate::auth::COOKIE)).timeout(std::time::Duration::from_secs(2)).call().unwrap().into_json::<Value>().unwrap();
    assert!(get_catalog()["books"].as_array().unwrap().is_empty());
    assert_eq!(std::fs::metadata(root.path().join("publish.sock")).unwrap().permissions().mode() & 0o777, 0o600);
    control.connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    let request = request(input.path());
    let (root_path, sent) = (root.path().to_owned(), request.clone());
    let import = std::thread::spawn(move || crate::online_publication::submit(&root_path, &sent));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !root.path().join("library/published/live").exists() {
        assert!(std::time::Instant::now() < deadline, "import never reached registration");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // Import is blocked at its metadata write, while the same service still answers readers.
    assert!(!import.is_finished());
    assert!(get_catalog()["books"].as_array().unwrap().is_empty());
    control.connection.execute_batch("COMMIT").unwrap();
    let published = import.join().unwrap().unwrap();
    assert_eq!(get_catalog()["books"][0]["published_book_ref"], json!(published.reference));
    server.shutdown();
    assert!(!root.path().join("publish.sock").exists());
    let server = start();
    assert_eq!(crate::online_publication::submit(root.path(), &request).unwrap().reference, published.reference);
    server.shutdown();
}
