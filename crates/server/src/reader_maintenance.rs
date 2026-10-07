//! Offline MU9 operations. A held ServiceWriter and a durable incomplete marker
//! keep a partially imported/restored root out of all normal service entry points.
use crate::{
    control_store::{ControlStore, ServiceWriter, CONTROL_SCHEMA_VERSION},
    published_library::PublishedBookRef,
    user_storage_paths::validate_user_id,
};
use memory::ReaderPrivateStorageGate as Gate;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn tool<T>(result: std::result::Result<T, read_tools::ToolError>) -> Result<T> {
    result.map_err(|e| format!("{}: {}", e.error_code, e.message).into())
}
fn read_json(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    Gate::create_dir_all(path.parent().ok_or("Missing parent")?)?;
    let mut file = tempfile::Builder::new()
        .prefix(".mu9-write-")
        .tempfile_in(path.parent().unwrap())?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    tool(Gate::secure_file(path))?;
    Gate::sync_parent(path)?;
    Ok(())
}
fn absolute(path: &Path) -> Result<PathBuf> {
    require(path.is_absolute(), "Paths must be absolute")?;
    require(
        !path
            .components()
            .any(|p| matches!(p, std::path::Component::ParentDir)),
        "Resolve parent-directory components before selecting roots",
    )?;
    if path.exists() {
        return Ok(path.canonicalize()?);
    }
    Ok(absolute(path.parent().ok_or("Missing parent")?)?
        .join(path.file_name().ok_or("Missing filename")?))
}
fn disjoint(paths: &[PathBuf]) -> Result<()> {
    for (i, a) in paths.iter().enumerate() {
        for b in paths.iter().skip(i + 1) {
            require(
                !a.starts_with(b) && !b.starts_with(a),
                "Source, target and backup roots must be independent",
            )?;
        }
    }
    Ok(())
}
fn sqlite(path: &Path) -> bool {
    let mut header = [0; 16];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut header))
        .is_ok()
        && &header == b"SQLite format 3\0"
}
fn files(root: &Path) -> Result<Vec<PathBuf>> {
    fn walk(root: &Path, rel: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(root.join(rel))? {
            let entry = entry?;
            let p = rel.join(entry.file_name());
            let kind = entry.file_type()?;
            require(
                kind.is_file() || kind.is_dir(),
                "Copy requires regular files and directories",
            )?;
            if kind.is_dir() {
                walk(root, &p, out)?;
            } else {
                out.push(p);
            }
        }
        Ok(())
    }
    let mut out = vec![];
    walk(root, Path::new(""), &mut out)?;
    out.sort();
    Ok(out)
}
fn sqlite_sidecar(root: &Path, rel: &Path) -> bool {
    let name = rel.to_string_lossy();
    ["-wal", "-shm", "-journal"].iter().any(|suffix| {
        name.strip_suffix(suffix)
            .is_some_and(|db| sqlite(&root.join(db)))
    })
}
fn service_transient(rel: &Path) -> bool {
    matches!(rel.to_str(), Some("service.lock" | "maintenance.json"))
}
fn copy_file(source: &Path, target: &Path, database: bool) -> Result<()> {
    if !database && same_file(source, target)? {
        return Ok(());
    }
    Gate::create_dir_all(target.parent().unwrap())?;
    let tmp = tempfile::Builder::new()
        .prefix(".mu9-copy-")
        .tempfile_in(target.parent().unwrap())?;
    if database {
        let input = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut output = Connection::open(tmp.path())?;
        rusqlite::backup::Backup::new(&input, &mut output)?.run_to_completion(
            128,
            std::time::Duration::from_millis(10),
            None,
        )?;
        let check: String = output.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        require(check == "ok", "SQLite backup integrity check failed")?;
    } else {
        std::io::copy(&mut fs::File::open(source)?, &mut tmp.as_file())?;
    }
    tmp.as_file().sync_all()?;
    tmp.persist(target)?;
    Gate::sync_parent(target)?;
    Ok(())
}
fn copy_tree(source: &Path, target: &Path, service: bool, databases: bool) -> Result<()> {
    Gate::create_dir_all(target)?;
    for rel in files(source)? {
        if service && service_transient(&rel) {
            continue;
        }
        if databases && sqlite_sidecar(source, &rel) {
            continue;
        }
        copy_file(
            &source.join(&rel),
            &target.join(&rel),
            databases && sqlite(&source.join(&rel)),
        )?;
    }
    Ok(())
}
fn inventory(root: &Path) -> Result<BTreeMap<String, u64>> {
    files(root)?
        .into_iter()
        .map(|p| {
            Ok((
                p.to_string_lossy().replace('\\', "/"),
                fs::metadata(root.join(p))?.len(),
            ))
        })
        .collect()
}
fn same_file(a: &Path, b: &Path) -> Result<bool> {
    if !b.is_file() || fs::metadata(a)?.len() != fs::metadata(b)?.len() {
        return Ok(false);
    }
    let (mut a, mut b) = (fs::File::open(a)?, fs::File::open(b)?);
    let (mut left, mut right) = ([0; 65536], [0; 65536]);
    loop {
        let n = a.read(&mut left)?;
        b.read_exact(&mut right[..n])?;
        if left[..n] != right[..n] {
            return Ok(false);
        }
        if n == 0 {
            return Ok(true);
        }
    }
}
fn same_tree(a: &Path, b: &Path) -> Result<()> {
    require(
        inventory(a)? == inventory(b)?,
        "Completed target inventory changed",
    )?;
    for p in files(a)? {
        require(
            same_file(&a.join(&p), &b.join(p))?,
            "Completed target bytes changed",
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BookMapping {
    /// Session directory spelling, including a Windows path when importing on Linux.
    pub legacy_dir: String,
    /// Actual local copy of that complete legacy book directory.
    pub source_dir: PathBuf,
    pub publication: PublishedBookRef,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MigrationPlan {
    pub operation_id: String,
    pub memory_dir: PathBuf,
    pub private_dir: PathBuf,
    pub library_root: PathBuf,
    pub service_root: PathBuf,
    pub user_id: String,
    /// A new operation directory; never a live service root.
    pub backup_dir: PathBuf,
    pub books: Vec<BookMapping>,
    /// Explicitly reviewed unknown files, as memory/<relative> or private/<relative>.
    #[serde(default)]
    pub reviewed_files: BTreeSet<String>,
}
#[derive(Serialize, Deserialize)]
struct MigrationRecord {
    plan: MigrationPlan,
    state: String,
    completed_files: BTreeSet<String>,
    report: Value,
}
fn path_key(value: &str) -> String {
    let v = value
        .strip_prefix(r"\\?\UNC\")
        .map(|s| format!(r"\\{s}"))
        .unwrap_or_else(|| value.strip_prefix(r"\\?\").unwrap_or(value).to_string());
    if v.as_bytes().get(1) == Some(&b':') || v.starts_with(r"\\") {
        v.replace('\\', "/").trim_end_matches('/').to_lowercase()
    } else {
        v.trim_end_matches('/').to_string()
    }
}
fn canonical_plan(plan: &MigrationPlan) -> Result<MigrationPlan> {
    tool(validate_user_id(&plan.user_id))?;
    tool(validate_user_id(&plan.operation_id))?;
    let mut plan = plan.clone();
    plan.memory_dir = absolute(&plan.memory_dir)?;
    plan.private_dir = absolute(&plan.private_dir)?;
    plan.library_root = absolute(&plan.library_root)?;
    plan.service_root = absolute(&plan.service_root)?;
    plan.backup_dir = absolute(&plan.backup_dir)?;
    let mut roots = vec![
        plan.memory_dir.clone(),
        plan.private_dir.clone(),
        plan.library_root.clone(),
        plan.service_root.clone(),
        plan.backup_dir.clone(),
    ];
    let (mut ids, mut dirs) = (BTreeSet::new(), BTreeSet::new());
    for book in &mut plan.books {
        book.source_dir = absolute(&book.source_dir)?;
        require(
            ids.insert(book.publication.book_id.clone()) && dirs.insert(path_key(&book.legacy_dir)),
            "Ambiguous book mapping",
        )?;
        if !book.source_dir.starts_with(&plan.library_root) {
            roots.push(book.source_dir.clone());
        }
    }
    disjoint(&roots)?;
    Ok(plan)
}
fn known_file(label: &str, rel: &str) -> bool {
    if label == "private" {
        // Intent and usage trees have their own evolving layouts; preserve and review them.
        return false;
    }
    matches!(
        rel,
        "memory.json"
            | "reader-profile.md"
            | "reading-handbook.md"
            | "agent-history.json"
            | "agent-chat-selection.json"
            | "agent-chat-selection.replace.bak"
            | "learning.db"
            | "learning.db-wal"
            | "learning.db-shm"
            | "learning.db-journal"
            | "session.json"
    ) || rel.starts_with("agent-history.presentations/")
        || (rel.starts_with("agent-sessions/s_") && rel.ends_with(".jsonl"))
}
fn require_current_schema(control: &ControlStore) -> Result<()> {
    let schema: i64 = control
        .connection
        .pragma_query_value(None, "user_version", |r| r.get(0))?;
    require(
        schema == CONTROL_SCHEMA_VERSION,
        "Use a compatible service binary",
    )
}
fn read_control(writer: std::sync::Arc<ServiceWriter>) -> Result<ControlStore> {
    let connection = Connection::open_with_flags(
        writer.root().join("control.sqlite"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let control = ControlStore { connection, writer };
    require_current_schema(&control)?;
    Ok(control)
}
fn mapping<'a>(plan: &'a MigrationPlan, id: &str) -> Result<&'a BookMapping> {
    plan.books
        .iter()
        .find(|b| b.publication.book_id == id)
        .ok_or_else(|| format!("Unmapped book_id: {id}").into())
}
fn material_refs(plan: &MigrationPlan, value: &Value) -> Result<()> {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if matches!(key.as_str(), "source_id" | "book_id") {
                    if let Some(id) = value.as_str().filter(|id| !id.is_empty()) {
                        mapping(plan, id)?;
                    }
                } else {
                    material_refs(plan, value)?;
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                material_refs(plan, value)?;
            }
        }
        _ => (),
    }
    Ok(())
}

struct Prepared {
    history: Option<Value>,
    session: Option<Value>,
    workspaces: Vec<Value>,
    report: Value,
}
fn verify_metadata(
    plan: &MigrationPlan,
    control: &ControlStore,
    prepared: &Prepared,
) -> Result<()> {
    for book in &plan.books {
        let count:i64=control.connection.query_row("SELECT count(*) FROM book_grants WHERE owner_user_id=? AND book_id=? AND publication_id=?",params![plan.user_id,book.publication.book_id,book.publication.publication_id],|r|r.get(0))?;
        require(count == 1, "Completed publication grant changed")?;
    }
    for w in &prepared.workspaces {
        let actual:Value=control.connection.query_row("SELECT book_id,publication_id,selected_chat,checkpoint FROM reader_workspaces WHERE owner_user_id=? AND workspace_id=?",params![plan.user_id,w["workspace_id"].as_str()],|r|Ok(json!({"publication":{"book_id":r.get::<_,String>(0)?,"publication_id":r.get::<_,String>(1)?},"selected_chat":r.get::<_,Option<String>>(2)?,"checkpoint":r.get::<_,String>(3)?})))?;
        require(
            actual["publication"] == w["publication"]
                && actual["selected_chat"] == w["selected_chat"]
                && serde_json::from_str::<Value>(actual["checkpoint"].as_str().unwrap())?
                    == w["checkpoint"],
            "Completed workspace mapping changed",
        )?;
    }
    Ok(())
}
fn prepare(
    plan: &MigrationPlan,
    control: &ControlStore,
    source: Option<&Path>,
) -> Result<Prepared> {
    require_current_schema(control)?;
    tool(control.user_paths(&plan.user_id))?;
    let mut destinations = BTreeMap::new();
    for (i, book) in plan.books.iter().enumerate() {
        let directory: String = control.connection.query_row(
            "SELECT directory FROM book_publications WHERE book_id=? AND publication_id=?",
            params![book.publication.book_id, book.publication.publication_id],
            |r| r.get(0),
        )?;
        let original = match (source, book.source_dir.strip_prefix(&plan.library_root)) {
            (Some(source), Ok(rel)) => source.join("library").join(rel),
            (Some(source), Err(_)) => source.join(format!("book-{i}")),
            (None, _) => book.source_dir.clone(),
        };
        let base = read_json(&original.join("base.json"))?;
        require(
            base["book_id"] == book.publication.book_id,
            "Book mapping changes the original book_id",
        )?;
        require(
            same_file(
                &original.join("source.txt"),
                &Path::new(&directory).join("source.txt"),
            )?,
            "Publication source differs from legacy source",
        )?;
        let old_book = read_tools::Book::load(original.to_str().ok_or("Invalid book path")?)?;
        let published = read_tools::Book::load(&directory)?;
        require(
            old_book.source_fingerprint() == published.source_fingerprint(),
            "Publication source identity differs",
        )?;
        let old_manifest = original.join("source_manifest.json");
        let new_manifest = Path::new(&directory).join("source_manifest.json");
        let old_pdf = if old_manifest.exists() {
            read_json(&old_manifest)?["original_pdf"].clone()
        } else {
            Value::Null
        };
        let new_pdf = if new_manifest.exists() {
            read_json(&new_manifest)?["original_pdf"].clone()
        } else {
            Value::Null
        };
        require(
            old_pdf.is_null() == new_pdf.is_null(),
            "Original PDF binding differs",
        )?;
        if !old_pdf.is_null() {
            // Reuse the existing source manifest identity validated by publication import.
            // Manifests without that identity require a direct comparison of the originals.
            if old_pdf["sha256"].is_string() {
                require(
                    old_pdf["sha256"] == new_pdf["sha256"],
                    "Original PDF identity differs",
                )?;
            } else {
                let old_path = original.join(
                    old_pdf["path"]
                        .as_str()
                        .ok_or("Missing original PDF path")?,
                );
                let new_path = Path::new(&directory).join(
                    new_pdf["path"]
                        .as_str()
                        .ok_or("Missing published PDF path")?,
                );
                require(
                    same_file(&old_path, &new_path)?,
                    "Original PDF bytes differ",
                )?;
            }
        }
        destinations.insert(book.publication.book_id.clone(), directory);
    }
    let memory = source
        .map(|s| s.join("memory"))
        .unwrap_or_else(|| plan.memory_dir.clone());
    let private = source
        .map(|s| s.join("private"))
        .unwrap_or_else(|| plan.private_dir.clone());
    let mut pending_review = vec![];
    let mut items = vec![];
    for (label, dir) in [("memory", &memory), ("private", &private)] {
        for (rel, bytes) in inventory(dir)? {
            let name = format!("{label}/{rel}");
            if !known_file(label, &rel) && !plan.reviewed_files.contains(&name) {
                pending_review.push(name.clone());
            }
            items.push(json!({"source":dir.join(&rel),"target":format!("users/{}/{name}",plan.user_id),"bytes":bytes}));
        }
    }
    let memory_value = memory
        .join("memory.json")
        .exists()
        .then(|| read_json(&memory.join("memory.json")))
        .transpose()?;
    let library_root = source
        .map(|s| s.join("library"))
        .unwrap_or_else(|| plan.library_root.clone());
    let mut registry_mapping = vec![];
    if library_root.join("library-registry.json").exists() {
        let registry = read_json(&library_root.join("library-registry.json"))?;
        for dir in registry["workspaces"]
            .as_array()
            .ok_or("Invalid library registry")?
        {
            let dir = dir.as_str().ok_or("Invalid library directory")?;
            let book = plan
                .books
                .iter()
                .find(|b| path_key(&b.legacy_dir) == path_key(dir))
                .ok_or_else(|| format!("Unmapped library directory: {dir}"))?;
            registry_mapping.push(json!({"legacy_dir":dir,"publication":book.publication}));
        }
    }
    // Validate known formats without saving or recovering either source.
    if let Some(ref value) = memory_value {
        fn refs(value: &Value, ids: &mut BTreeSet<String>) {
            match value {
                Value::Object(o) => {
                    for (k, v) in o {
                        if k == "book_id" {
                            if let Some(s) = v.as_str().filter(|s| !s.is_empty()) {
                                ids.insert(s.into());
                            }
                        } else {
                            refs(v, ids);
                        }
                    }
                }
                Value::Array(a) => {
                    for v in a {
                        refs(v, ids);
                    }
                }
                _ => (),
            }
        }
        let mut ids = BTreeSet::new();
        refs(value, &mut ids);
        for id in ids {
            mapping(plan, &id)?;
        }
    }
    let mut history = memory
        .join("agent-history.json")
        .exists()
        .then(|| read_json(&memory.join("agent-history.json")))
        .transpose()?;
    let (mut chats, mut turns, mut pending) = (0, 0, 0);
    if let Some(history) = &mut history {
        material_refs(plan, history)?;
        let _: crate::AgentHistory = serde_json::from_value(history.clone())?;
        for session in history["sessions"]
            .as_array_mut()
            .ok_or("History sessions must be an array")?
        {
            chats += 1;
            let book = mapping(
                plan,
                session["book_id"]
                    .as_str()
                    .ok_or("Missing history book_id")?,
            )?;
            for turn in session["turns"]
                .as_array_mut()
                .ok_or("Missing history turns")?
            {
                turns += 1;
                if turn["status"] == "pending_assistant" {
                    pending += 1;
                }
                let reference = json!(book.publication);
                require(
                    turn["published_book_ref"].is_null() || turn["published_book_ref"] == reference,
                    "Existing history publication conflicts with mapping",
                )?;
                require(
                    turn["admission_input"].is_null(),
                    "Use a service snapshot for network admissions, not legacy import",
                )?;
                turn["published_book_ref"] = reference;
            }
        }
    }
    let mut session = memory
        .join("session.json")
        .exists()
        .then(|| read_json(&memory.join("session.json")))
        .transpose()?;
    let mut workspaces = vec![];
    if let Some(ref mut state) = session {
        let (current_key, progress) = if state.get("current_book_dir").is_some() {
            (
                "current_book_dir",
                state["books"]
                    .as_object()
                    .ok_or("Missing session books")?
                    .clone(),
            )
        } else {
            let dir = state["book_dir"].as_str().ok_or("Unknown session schema")?;
            (
                "book_dir",
                serde_json::Map::from_iter([(dir.into(), json!({"top_lid":state["top_lid"]}))]),
            )
        };
        let find = |dir: &str| {
            plan.books
                .iter()
                .find(|b| path_key(&b.legacy_dir) == path_key(dir))
                .ok_or_else(|| format!("Unmapped session directory: {dir}"))
        };
        let current = find(
            state[current_key]
                .as_str()
                .ok_or("Missing current directory")?,
        )?;
        let mut relocated = serde_json::Map::new();
        let mut seen = BTreeSet::new();
        for (dir, position) in progress {
            let book = find(&dir)?;
            require(
                seen.insert(&book.publication.book_id),
                "Session positions collide",
            )?;
            let directory = &destinations[&book.publication.book_id];
            let loaded = read_tools::Book::load(directory)?;
            let top = position["top_lid"]
                .as_str()
                .ok_or("Missing reading position")?;
            require(
                loaded.base.lid_nodes.iter().any(|n| n.lid == top),
                "Legacy reading position is unavailable in the mapped publication",
            )?;
            let mut checkpoint = reader::Reader::new(&loaded, reader::DEFAULT_RADIUS).checkpoint();
            checkpoint.top_lid = top.into();
            let selected = history
                .as_ref()
                .and_then(|h| h["active_by_book"][&book.publication.book_id].as_str());
            workspaces.push(json!({"workspace_id":format!("mu9-{}-{}",plan.operation_id,workspaces.len()),"publication":book.publication,"selected_chat":selected,
                "checkpoint":{"version":1,"reader":checkpoint,"presentation":null}}));
            relocated.insert(directory.clone(), position);
        }
        state[current_key] = json!(destinations[&current.publication.book_id]);
        if current_key == "current_book_dir" {
            state["books"] = json!(relocated);
        }
    }
    let mut learning_tables = BTreeMap::new();
    let learning_path = memory.join("learning.db");
    let learning_schema = if learning_path.exists() {
        let db = Connection::open_with_flags(&learning_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let schema: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let names: Vec<String> = {
            let mut q = db.prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            )?;
            let values = q
                .query_map([], |r| r.get(0))?
                .collect::<std::result::Result<_, _>>()?;
            values
        };
        for name in names {
            if name == "learning_evidence" || name == "learner_projection" {
                let mut q = db.prepare(&format!("SELECT DISTINCT source_id FROM {name}"))?;
                for id in q.query_map([], |r| r.get::<_, String>(0))? {
                    mapping(plan, &id?)?;
                }
            }
            let json_column = match name.as_str() {
                "tutor_events" => Some("request"),
                "tutor_projection" => Some("state"),
                "teaching_trace" => Some("event"),
                "learning_evidence" => Some("evidence"),
                _ => None,
            };
            if let Some(column) = json_column {
                let mut q = db.prepare(&format!("SELECT {column} FROM {name}"))?;
                for value in q.query_map([], |r| r.get::<_, String>(0))? {
                    material_refs(plan, &serde_json::from_str::<Value>(&value?)?)?;
                }
            }
            let count: i64 = db.query_row(
                &format!("SELECT count(*) FROM \"{}\"", name.replace('"', "\"\"")),
                [],
                |r| r.get(0),
            )?;
            learning_tables.insert(name, count);
        }
        Some(schema)
    } else {
        None
    };
    let records = memory_value
        .as_ref()
        .and_then(|v| v.as_array().or_else(|| v["records"].as_array()))
        .map_or(0, Vec::len);
    let report = json!({"operation_id":plan.operation_id,"user_id":plan.user_id,"control_schema":CONTROL_SCHEMA_VERSION,
        "items":items,"books":plan.books,"pending_review":pending_review,"counts":{"chats":chats,"turns":turns,"legacy_pending":pending,"positions":workspaces.len()},
        "memory":{"records":records,"schema_version":memory_value.as_ref().and_then(|v|v.get("schema_version")),"document_revision":memory_value.as_ref().and_then(|v|v.get("document_revision"))},
        "learning":{"schema_version":learning_schema,"tables":learning_tables},"registry_mapping":registry_mapping,"workspaces":workspaces});
    Ok(Prepared {
        history,
        session,
        workspaces,
        report,
    })
}

/// Dry-run acquires the same writer lock, but does not create a user or modify data.
pub fn migration_preview(plan: &MigrationPlan) -> Result<Value> {
    let plan = canonical_plan(plan)?;
    require(
        plan.service_root.join("control.sqlite").is_file(),
        "Provision the target account and publications first",
    )?;
    let writer = tool(ServiceWriter::acquire(&plan.service_root))?;
    let control = read_control(writer)?;
    Ok(prepare(&plan, &control, None)?.report)
}

fn marker(root: &Path, value: &Value) -> Result<()> {
    let path = root.join("maintenance.json");
    if path.exists() {
        require(
            read_json(&path)? == *value,
            "Another maintenance operation owns this root",
        )?;
    } else {
        write_json(&path, value)?;
    }
    Ok(())
}
fn finish(root: &Path) -> Result<()> {
    fs::remove_file(root.join("maintenance.json"))?;
    Gate::sync_parent(&root.join("maintenance.json"))?;
    Ok(())
}
fn seal_files(root: &Path) -> Result<()> {
    for rel in files(root)? {
        let file = root.join(rel);
        let mut permissions = fs::metadata(&file)?.permissions();
        permissions.set_readonly(true);
        fs::set_permissions(file, permissions)?;
    }
    Ok(())
}
fn snapshot(sources: &[(&str, &Path)], destination: &Path, metadata: Value) -> Result<()> {
    require(!destination.exists(), "Snapshot destination already exists")?;
    Gate::create_dir_all(destination.parent().unwrap())?;
    // Only the snapshot belongs to this operation; the parent may also serve Web assets.
    let stage = tempfile::tempdir_in(destination.parent().unwrap())?;
    tool(Gate::secure_directory(stage.path()))?;
    for (name, source) in sources {
        copy_tree(source, &stage.path().join(name), *name == "service", true)?;
    }
    let list = inventory(stage.path())?;
    write_json(
        &stage.path().join("snapshot.json"),
        &json!({"version":1,"metadata":metadata,"files":list}),
    )?;
    seal_files(stage.path())?;
    fs::rename(stage.path(), destination)?;
    Gate::sync_parent(destination)?;
    Ok(())
}
fn validate_snapshot(snapshot: &Path) -> Result<Value> {
    let manifest = read_json(&snapshot.join("snapshot.json"))?;
    require(manifest["version"] == 1, "Unsupported snapshot")?;
    let mut actual = inventory(snapshot)?;
    actual.remove("snapshot.json");
    require(
        json!(actual) == manifest["files"],
        "Snapshot is incomplete or changed",
    )?;
    Ok(manifest["metadata"].clone())
}

pub fn migrate(plan: &MigrationPlan, stopped: bool) -> Result<Value> {
    migrate_inner(plan, stopped, None)
}
pub(crate) fn migrate_inner(
    plan: &MigrationPlan,
    stopped: bool,
    fail_after: Option<usize>,
) -> Result<Value> {
    require(
        stopped,
        "Stop legacy writers and acknowledge --stopped before applying",
    )?;
    let plan = canonical_plan(plan)?;
    require(
        plan.service_root.join("control.sqlite").is_file(),
        "Provision the target account and publications first",
    )?;
    let writer = tool(ServiceWriter::acquire_maintenance(&plan.service_root))?;
    let record_path = plan.backup_dir.join("operation.json");
    let marker_value =
        json!({"kind":"legacy-import","operation_id":plan.operation_id,"record":record_path});
    if plan.service_root.join("maintenance.json").exists() {
        require(
            read_json(&plan.service_root.join("maintenance.json"))? == marker_value,
            "Another maintenance operation owns this root",
        )?;
    }
    let check = read_control(writer.clone())?;
    drop(check);
    let mut control = tool(ControlStore::open(writer.clone()))?;
    let user_root = plan.service_root.join("users").join(&plan.user_id);
    let snapshot_root = plan.backup_dir.join("snapshot");
    let mut record = if record_path.exists() {
        let saved: MigrationRecord = serde_json::from_slice(&fs::read(&record_path)?)?;
        require(
            saved.plan == plan,
            "Operation plan conflict: source, target, mapping or review changed",
        )?;
        saved
    } else {
        require(
            !plan.backup_dir.exists() || files(&plan.backup_dir)?.is_empty(),
            "Choose a new backup directory for this operation",
        )?;
        require(
            !user_root.exists() || files(&user_root)?.is_empty(),
            "Target user already contains data; import requires an unused account",
        )?;
        let count: i64 = control.connection.query_row(
            "SELECT count(*) FROM reader_workspaces WHERE owner_user_id=?",
            [&plan.user_id],
            |r| r.get(0),
        )?;
        require(count == 0, "Target user already has reading workspaces")?;
        let prepared = prepare(&plan, &control, None)?;
        require(
            prepared.report["pending_review"]
                .as_array()
                .unwrap()
                .is_empty(),
            "Review unknown files from dry-run before applying",
        )?;
        Gate::create_dir_all(&plan.backup_dir)?;
        tool(Gate::secure_directory(&plan.backup_dir))?;
        let record = MigrationRecord {
            plan: plan.clone(),
            state: "planned".into(),
            completed_files: BTreeSet::new(),
            report: prepared.report,
        };
        write_json(&record_path, &record)?;
        record
    };
    if record.state != "complete" {
        marker(&plan.service_root, &marker_value)?;
    }
    if !snapshot_root.exists() {
        require(record.state == "planned", "Completed backup is missing")?;
        let labels: Vec<String> = (0..plan.books.len()).map(|i| format!("book-{i}")).collect();
        let mut sources = vec![
            ("memory", plan.memory_dir.as_path()),
            ("private", plan.private_dir.as_path()),
            ("library", plan.library_root.as_path()),
            ("service", plan.service_root.as_path()),
        ];
        sources.extend(
            plan.books
                .iter()
                .zip(&labels)
                .filter(|(b, _)| !b.source_dir.starts_with(&plan.library_root))
                .map(|(b, label)| (label.as_str(), b.source_dir.as_path())),
        );
        snapshot(
            &sources,
            &snapshot_root,
            json!({"kind":"legacy-import","plan":plan}),
        )?;
    }
    let metadata = validate_snapshot(&snapshot_root)?;
    require(metadata["plan"] == json!(plan), "Snapshot plan conflict")?;
    let prepared = prepare(&plan, &control, Some(&snapshot_root))?;
    let expected = tempfile::tempdir_in(&plan.backup_dir)?;
    copy_tree(
        &snapshot_root.join("memory"),
        &expected.path().join("memory"),
        false,
        false,
    )?;
    copy_tree(
        &snapshot_root.join("private"),
        &expected.path().join("private"),
        false,
        false,
    )?;
    if let Some(history) = &prepared.history {
        write_json(&expected.path().join("memory/agent-history.json"), history)?;
    }
    if let Some(session) = &prepared.session {
        write_json(&expected.path().join("memory/session.json"), session)?;
    }
    // Validate on a separate copy: normal readers may upgrade legacy Memory/Learning.
    // Imported bytes retain their original data version until the runtime opens them.
    let validation = tempfile::tempdir_in(&plan.backup_dir)?;
    copy_tree(
        &expected.path().join("memory"),
        validation.path(),
        false,
        false,
    )?;
    let store = tool(memory::MemoryStore::open_private_with_learning(
        validation.path().join("memory.json"),
        validation.path().join("learning.db"),
    ))?;
    let learning = tool(store.learning_store())?;
    drop(learning);
    drop(store);
    if record.state == "complete" {
        same_tree(expected.path(), &user_root)?;
        verify_metadata(&plan, &control, &prepared)?;
        if plan.service_root.join("maintenance.json").exists() {
            finish(&plan.service_root)?;
        }
        return Ok(json!({"state":"complete","resumed":true,"report":record.report}));
    }
    record.state = "copying".into();
    write_json(&record_path, &record)?;
    let expected_files = files(expected.path())?;
    if user_root.exists() {
        let allowed: BTreeSet<_> = expected_files.iter().collect();
        for rel in files(&user_root)? {
            if rel
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".mu9-copy-")
                && !allowed.contains(&rel)
            {
                fs::remove_file(user_root.join(rel))?;
            } else {
                require(
                    allowed.contains(&rel),
                    "Unexpected target file; refusing to overwrite",
                )?;
            }
        }
    }
    for (i, rel) in expected_files.iter().enumerate() {
        let target = user_root.join(rel);
        if target.exists() {
            require(
                same_file(&expected.path().join(rel), &target)?,
                "Target file conflict",
            )?;
        } else {
            copy_file(&expected.path().join(rel), &target, false)?;
        }
        record
            .completed_files
            .insert(rel.to_string_lossy().replace('\\', "/"));
        write_json(&record_path, &record)?;
        if fail_after == Some(i + 1) {
            return Err("Injected interruption after file commit".into());
        }
    }
    tool(tool(writer.paths(&plan.user_id))?.prepare())?;
    let tx = control.connection.transaction()?;
    for book in &plan.books {
        tx.execute(
            "INSERT OR IGNORE INTO book_grants(owner_user_id,book_id,publication_id) VALUES(?,?,?)",
            params![
                plan.user_id,
                book.publication.book_id,
                book.publication.publication_id
            ],
        )?;
    }
    for (i, w) in prepared.workspaces.iter().enumerate() {
        let id = w["workspace_id"].as_str().unwrap();
        let old: i64 = tx.query_row(
            "SELECT count(*) FROM reader_workspaces WHERE owner_user_id=? AND workspace_id=?",
            params![plan.user_id, id],
            |r| r.get(0),
        )?;
        if old == 0 {
            tx.execute("INSERT INTO reader_workspaces(owner_user_id,workspace_id,book_id,publication_id,selected_chat,checkpoint,checkpoint_seq) VALUES(?,?,?,?,?,?,?)",params![plan.user_id,id,w["publication"]["book_id"].as_str(),w["publication"]["publication_id"].as_str(),w["selected_chat"].as_str(),w["checkpoint"].to_string(),i as i64+1])?;
        }
    }
    tx.commit()?;
    if fail_after == Some(usize::MAX) {
        return Err("Injected interruption after metadata commit".into());
    }
    same_tree(expected.path(), &user_root)?;
    verify_metadata(&plan, &control, &prepared)?;
    record.state = "complete".into();
    record.report = prepared.report;
    write_json(&record_path, &record)?;
    finish(&plan.service_root)?;
    Ok(json!({"state":"complete","report":record.report}))
}

/// The lock stops cooperating writers; all SQLite files use the backup API,
/// while the held lock makes the files and databases one recovery point.
/// The recursive inventory includes agent-sessions/*.jsonl, chat selection and
/// its recovery backup, alongside Memory, Learning and Presentation objects.
pub fn backup_service(root: &Path, destination: &Path) -> Result<Value> {
    let root = absolute(root)?;
    let destination = absolute(destination)?;
    disjoint(&[root.clone(), destination.clone()])?;
    require(
        root.join("control.sqlite").is_file(),
        "Service root does not exist",
    )?;
    let _writer = tool(ServiceWriter::acquire(&root))?;
    let check = read_control(_writer.clone())?;
    drop(check);
    snapshot(
        &[("service", &root)],
        &destination,
        json!({"kind":"service","source_root":root,"control_schema":CONTROL_SCHEMA_VERSION}),
    )?;
    Ok(json!({"snapshot":destination,"state":"complete"}))
}

/// Restore into a fresh independent root. Existing live data is never replaced.
pub fn restore_service(snapshot_root: &Path, destination: &Path) -> Result<Value> {
    restore_inner(snapshot_root, destination, false)
}
pub(crate) fn restore_inner(
    snapshot_root: &Path,
    destination: &Path,
    fail_before_complete: bool,
) -> Result<Value> {
    let snapshot_root = absolute(snapshot_root)?;
    let destination = absolute(destination)?;
    let metadata = validate_snapshot(&snapshot_root)?;
    let source = if metadata["kind"] == "legacy-import" {
        metadata["plan"]["service_root"].as_str()
    } else {
        metadata["source_root"].as_str()
    }
    .ok_or("Not a service snapshot")?;
    disjoint(&[
        PathBuf::from(source),
        snapshot_root.clone(),
        destination.clone(),
    ])?;
    let writer = tool(ServiceWriter::acquire_maintenance(&destination))?;
    let value = json!({"kind":"restore","snapshot":snapshot_root});
    if !destination.join("maintenance.json").exists() {
        require(
            files(&destination)?.iter().all(|p| service_transient(p)),
            "Restore requires an empty independent root",
        )?;
    }
    marker(&destination, &value)?;
    // Recreate each file from the immutable recovery point; the marker prevents startup.
    // A prior interrupted restore may have a WAL for the previous copied main DB.
    // Remove only those known database sidecars, under the held root lock.
    for rel in files(&snapshot_root.join("service"))? {
        if sqlite(&snapshot_root.join("service").join(&rel)) {
            for suffix in ["-wal", "-shm", "-journal"] {
                let sidecar = destination.join(format!("{}{suffix}", rel.to_string_lossy()));
                if sidecar.exists() {
                    fs::remove_file(sidecar)?;
                }
            }
        }
    }
    copy_tree(&snapshot_root.join("service"), &destination, true, false)?;
    let control = tool(ControlStore::open(writer))?;
    let rows: Vec<(String, String, String)> = {
        let mut q = control
            .connection
            .prepare("SELECT book_id,publication_id,directory FROM book_publications")?;
        let rows = q
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<std::result::Result<_, _>>()?;
        rows
    };
    for (book, publication, directory) in rows {
        let relative = Path::new(&directory)
            .strip_prefix(source)
            .map_err(|_| "Publication is outside the original service root")?;
        let moved = destination.join(relative);
        require(
            moved.join("publication.json").is_file(),
            "Snapshot publication is missing",
        )?;
        control.connection.execute(
            "UPDATE book_publications SET directory=? WHERE book_id=? AND publication_id=?",
            params![moved.to_string_lossy(), book, publication],
        )?;
        seal_files(&moved)?;
    }
    if fail_before_complete {
        return Err("Injected interruption after restored metadata".into());
    }
    finish(&destination)?;
    Ok(json!({"state":"complete","service_root":destination,"restored_from":snapshot_root}))
}

/// Explicit single-user export for an old local binary. No live admissions may
/// be lost when removing the network control plane from this copy.
pub fn export_user(root: &Path, owner: &str, destination: &Path) -> Result<Value> {
    tool(validate_user_id(owner))?;
    let root = absolute(root)?;
    let destination = absolute(destination)?;
    disjoint(&[root.clone(), destination.clone()])?;
    require(!destination.exists(), "Export destination must be new")?;
    let writer = tool(ServiceWriter::acquire(&root))?;
    let control = tool(ControlStore::open(writer.clone()))?;
    tool(control.user_paths(owner))?;
    let busy: i64 = control.connection.query_row("SELECT count(*) FROM run_admissions WHERE owner_user_id=? AND (dispatch_state IN ('preparing','queued','claimed') OR unsaved=1)",[owner],|r|r.get(0))?;
    require(
        busy == 0,
        "Settle or cancel this user's accepted runs before legacy export",
    )?;
    Gate::create_dir_all(destination.parent().unwrap())?;
    let stage = tempfile::tempdir_in(destination.parent().unwrap())?;
    let user = writer.root().join("users").join(owner);
    for name in ["memory", "private"] {
        if user.join(name).exists() {
            copy_tree(&user.join(name), &stage.path().join(name), false, true)?;
        } else {
            Gate::create_dir_all(&stage.path().join(name))?;
        }
    }
    let rows: Vec<(String, String, String)> = {
        let mut q = control.connection.prepare("SELECT book_id,publication_id,directory FROM book_publications WHERE (book_id,publication_id) IN (SELECT book_id,publication_id FROM book_grants WHERE owner_user_id=?)")?;
        let rows = q
            .query_map([owner], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<std::result::Result<_, _>>()?;
        rows
    };
    let mut books = serde_json::Map::new();
    let mut current = None;
    let mut latest = -1;
    for (book, publication, directory) in &rows {
        let rel = PathBuf::from("books").join(book).join(publication);
        copy_tree(Path::new(directory), &stage.path().join(&rel), false, false)?;
        let target = destination.join(rel).to_string_lossy().to_string();
        let mut q = control.connection.prepare("SELECT checkpoint,checkpoint_seq FROM reader_workspaces WHERE owner_user_id=? AND book_id=? AND publication_id=? AND checkpoint IS NOT NULL ORDER BY checkpoint_seq DESC LIMIT 1")?;
        let mut positions = q.query(params![owner, book, publication])?;
        if let Some(row) = positions.next()? {
            let checkpoint: Value = serde_json::from_str(&row.get::<_, String>(0)?)?;
            books.insert(
                target.clone(),
                json!({"top_lid":checkpoint["reader"]["top_lid"]}),
            );
            let seq: i64 = row.get(1)?;
            if seq > latest {
                latest = seq;
                current = Some(target);
            }
        } else if current.is_none() {
            current = Some(target);
        }
    }
    if let Some(current) = current {
        write_json(
            &stage.path().join("memory/session.json"),
            &json!({"current_book_dir":current,"books":books}),
        )?;
    }
    write_json(
        &stage.path().join("export.json"),
        &json!({"user_id":owner,"source_root":root,"publications":rows,"control_schema":CONTROL_SCHEMA_VERSION,"purpose":"independent legacy rollback copy"}),
    )?;
    fs::rename(stage.path(), &destination)?;
    Gate::sync_parent(&destination)?;
    tool(Gate::enforce(&destination.join("memory/memory.json")))?;
    tool(Gate::enforce(&destination.join("private/.permissions")))?;
    Ok(json!({"state":"complete","user_id":owner,"directory":destination}))
}
