//! Immutable, offline-admin publications. HTTP identity is supplied by the MU4 host.
use crate::{control_store::ControlStore, user_storage_paths::error};
use read_tools::{Book, ToolError};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    sync::{Arc, Weak},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PublishedBookRef {
    pub book_id: String,
    pub publication_id: String,
}
impl PublishedBookRef {
    pub fn url(&self, leaf: &str) -> String {
        format!(
            "/api/books/{}/publications/{}/{}",
            self.book_id, self.publication_id, leaf
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationManifest {
    pub version: String,
    pub reference: PublishedBookRef,
    pub source_fingerprint: String,
    /// Exact files admitted by the importer, with byte lengths (no additional digests).
    pub files: BTreeMap<String, u64>,
    pub capabilities: Value,
    pub artifact_versions: BTreeMap<String, Value>,
    pub teaching_readiness: Value,
    /// Import-time snapshot; runtime admission reads artifacts, optional assets reuse this snapshot.
    #[serde(default)]
    pub tutor_readiness: Value,
    pub build_readiness: Value,
    pub resident_bytes: u64,
}

pub struct PublishedBook {
    pub reference: PublishedBookRef,
    pub book: Arc<Book>,
    pub manifest: PublicationManifest,
    pub(crate) directory: PathBuf,
    pub(crate) localization_cache: PathBuf,
}

struct ResidentEntry {
    book: Weak<Book>,
    cached: Option<Arc<Book>>,
    bytes: u64,
    touched: u64,
}

/// One service registry. Weak entries survive LRU eviction while any scene/Run pins a Book.
pub struct PublishedLibrary {
    control: ControlStore,
    residents: BTreeMap<PublishedBookRef, ResidentEntry>,
    clock: u64,
    max_books: usize,
    max_bytes: u64,
}

fn invalid() -> ToolError {
    error(
        "PUBLICATION_INVALID",
        "validation",
        "Publication materials or declared dependencies are invalid",
    )
}
fn unavailable() -> ToolError {
    error(
        "PUBLICATION_UNAVAILABLE",
        "not_found",
        "The exact publication is unavailable",
    )
}
fn storage() -> ToolError {
    error(
        "PUBLICATION_STORAGE_FAILED",
        "unavailable",
        "Publication could not be read or committed",
    )
}
fn capacity() -> ToolError {
    error(
        "BOOK_CAPACITY",
        "unavailable",
        "Live Books exhaust the resident budget",
    )
}

const OPTIONAL: &[&str] = &[
    "formula_semantics.json",
    "discourse_index.json",
    "book_structure.json",
    "paper_metadata.json",
    "paper_lexicon.json",
    "source_manifest.json",
    "pdf_source_map.json",
    "pass2_audit.json",
    "alignment_report.json",
    "asset_manifest.json",
    "teaching_readiness.json",
    "formal_objects.json",
    "cognitive_materials.json",
    "profile_sidecar.json",
    "paper_reading_guide.json",
    "pdf_selection_map/manifest.json",
];

fn relative(path: &str) -> Result<(), ToolError> {
    if path.is_empty()
        || path.contains(['\\', ':', '%'])
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(invalid());
    }
    Ok(())
}

// Links and special files are forbidden by the published-package contract.
fn regular_file(path: &Path) -> Result<(), ToolError> {
    for ancestor in path.ancestors() {
        let meta = fs::symlink_metadata(ancestor).map_err(|_| invalid())?;
        if meta.file_type().is_symlink() {
            return Err(invalid());
        }
    }
    if !fs::metadata(path).map_err(|_| invalid())?.is_file() {
        return Err(invalid());
    }
    Ok(())
}
fn read_json(path: &Path) -> Result<Value, ToolError> {
    regular_file(path)?;
    serde_json::from_slice(&fs::read(path).map_err(|_| storage())?).map_err(|_| invalid())
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<(), ToolError> {
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|_| invalid())?,
    )
    .map_err(|_| storage())
}
fn copy_file(input: &Path, stage: &Path, rel: &str) -> Result<(), ToolError> {
    relative(rel)?;
    regular_file(input)?;
    let dest = stage.join(rel);
    fs::create_dir_all(dest.parent().unwrap()).map_err(|_| storage())?;
    // A complete input may itself be a sealed publication. Copy bytes into a
    // writable staging file; fs::copy would inherit its read-only permissions,
    // preventing manifest normalization and the subsequent sync/seal step.
    let mut source = fs::File::open(input).map_err(|_| storage())?;
    let mut target = fs::File::create(dest).map_err(|_| storage())?;
    std::io::copy(&mut source, &mut target).map_err(|_| storage())?;
    Ok(())
}

fn dependencies(input: &Path, stage: &Path) -> Result<(), ToolError> {
    for file in ["base.json", "source.txt"] {
        copy_file(&input.join(file), stage, file)?;
    }
    for file in OPTIONAL {
        if fs::symlink_metadata(input.join(file)).is_ok() {
            copy_file(&input.join(file), stage, file)?;
        }
    }
    let manifest_path = stage.join("source_manifest.json");
    if manifest_path.exists() {
        let mut source = read_json(&manifest_path)?;
        if source["canonical_source"]["path"] != "source.txt" {
            return Err(invalid());
        }
        if let Some(path) = source["original_pdf"]["path"].as_str().map(str::to_string) {
            let pdf = if Path::new(&path).is_absolute() {
                PathBuf::from(&path)
            } else {
                relative(&path)?;
                input.join(&path)
            };
            copy_file(&pdf, stage, "original.pdf")?;
            if let Some(expected) = source["original_pdf"]["sha256"].as_str() {
                if crate::sha256_hex(&fs::read(stage.join("original.pdf")).map_err(|_| storage())?)
                    != expected
                {
                    return Err(invalid());
                }
            }
            source["original_pdf"]["path"] = json!("original.pdf");
            if let Some(caps) = source["capabilities"].as_object_mut() {
                for cap in caps.values_mut() {
                    if cap["artifact_path"].as_str() == Some(&path) {
                        cap["artifact_path"] = json!("original.pdf");
                    }
                }
            }
        }
        if let Some(caps) = source["capabilities"].as_object() {
            for cap in caps.values() {
                if !matches!(cap["status"].as_str(), Some("available" | "degraded")) {
                    continue;
                }
                if cap["status"] == "degraded"
                    && cap["reason"].as_str().is_none_or(|s| s.trim().is_empty())
                {
                    return Err(invalid());
                }
                for key in ["artifact_path", "report_path"] {
                    if let Some(path) = cap[key].as_str() {
                        relative(path)?;
                        // Only runtime artifacts, never arbitrary build scripts or private files.
                        if path != "original.pdf" && !OPTIONAL.contains(&path) {
                            return Err(invalid());
                        }
                        if path != "original.pdf" {
                            copy_file(&input.join(path), stage, path)?;
                        }
                        if key == "artifact_path" && path.ends_with(".json") {
                            let artifact = read_json(&stage.join(path))?;
                            if crate::capability_hash_mismatch(
                                Some(cap),
                                artifact["config_hash"].as_str(),
                            ) {
                                return Err(invalid());
                            }
                        }
                    }
                }
            }
        }
        write_json(&manifest_path, &source)?;
    }
    let assets = stage.join("asset_manifest.json");
    if assets.exists() {
        let mut manifest = read_json(&assets)?;
        if let Some(path) = manifest["cover"]["stored_path"].as_str() {
            relative(path)?;
            if !path.starts_with("assets/") || !crate::mime_for_asset(Path::new(path)).starts_with("image/") {
                return Err(invalid());
            }
            copy_file(&input.join(path), stage, path)?;
        }
        if let Some(images) = manifest["images"].as_array_mut() {
            for image in images {
                if image["status"] != "available" {
                    continue;
                }
                let path = image["stored_path"]
                    .as_str()
                    .ok_or_else(invalid)?
                    .to_string();
                relative(&path)?;
                if !path.starts_with("assets/") {
                    return Err(invalid());
                }
                copy_file(&input.join(&path), stage, &path)?;
                if let Some(expected) = image["sha256"].as_str() {
                    if crate::sha256_hex(&fs::read(stage.join(&path)).map_err(|_| storage())?)
                        != expected
                    {
                        return Err(invalid());
                    }
                }
                image["url_path"] = json!(format!("/book/{path}"));
            }
        }
        write_json(&assets, &manifest)?;
    }
    let selection = stage.join("pdf_selection_map/manifest.json");
    if selection.exists() {
        let manifest = read_json(&selection)?;
        for shard in manifest["page_shards"].as_array().ok_or_else(invalid)? {
            let path = shard["path"].as_str().ok_or_else(invalid)?;
            relative(path)?;
            let rel = format!("pdf_selection_map/{path}");
            copy_file(&input.join(&rel), stage, &rel)?;
        }
    }
    let receipt = stage.join("teaching_readiness.json");
    if receipt.exists() {
        let receipt = read_json(&receipt)?;
        if let Some(path) = receipt["map_path"].as_str() {
            relative(path)?;
            let revision = receipt["teaching_map_revision"]
                .as_str()
                .ok_or_else(invalid)?;
            if path != format!("teaching/versions/{revision}/map.json") {
                return Err(invalid());
            }
            copy_file(&input.join(path), stage, path)?;
        }
    }
    Ok(())
}

fn inventory(root: &Path, dir: &Path, files: &mut BTreeMap<String, u64>) -> Result<(), ToolError> {
    for item in fs::read_dir(dir).map_err(|_| storage())? {
        let item = item.map_err(|_| storage())?;
        let ty = item.file_type().map_err(|_| storage())?;
        if ty.is_dir() {
            inventory(root, &item.path(), files)?;
        } else if ty.is_file() {
            files.insert(
                item.path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                item.metadata().map_err(|_| storage())?.len(),
            );
        } else {
            return Err(invalid());
        }
    }
    Ok(())
}

fn seal(dir: &Path) -> Result<(), ToolError> {
    for entry in fs::read_dir(dir).map_err(|_| storage())? {
        let path = entry.map_err(|_| storage())?.path();
        if path.is_dir() {
            seal(&path)?;
        } else {
            fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .and_then(|f| f.sync_all())
                .map_err(|_| storage())?;
            let mut permissions = fs::metadata(&path).map_err(|_| storage())?.permissions();
            permissions.set_readonly(true);
            fs::set_permissions(&path, permissions).map_err(|_| storage())?;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::File::open(dir)
            .and_then(|f| f.sync_all())
            .map_err(|_| storage())?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o555)).map_err(|_| storage())?;
    }
    Ok(())
}

impl PublishedLibrary {
    pub fn new(control: ControlStore) -> Self {
        Self::with_budget(control, 20, 2 * 1024 * 1024 * 1024)
    }
    pub fn with_budget(control: ControlStore, max_books: usize, max_bytes: u64) -> Self {
        Self {
            control,
            residents: BTreeMap::new(),
            clock: 0,
            max_books,
            max_bytes,
        }
    }
    /// Trusted offline operator only, sharing MU2's exclusive ServiceWriter.
    pub fn publish(&mut self, input: &Path) -> Result<PublicationManifest, ToolError> {
        let input = std::path::absolute(input).map_err(|_| invalid())?;
        let input = input.as_path();
        let root = self.control.writer.root().join("library");
        fs::create_dir_all(root.join("staging")).map_err(|_| storage())?;
        let stage = tempfile::tempdir_in(root.join("staging")).map_err(|_| storage())?;
        dependencies(input, stage.path())?;
        let book = Book::load(stage.path().to_str().ok_or_else(invalid)?).map_err(|_| invalid())?;
        let snapshot = crate::build_workbench_snapshot(&book, input, false);
        let snapshot: Value = serde_json::from_str(&snapshot.body).map_err(|_| invalid())?;
        if snapshot["readiness"]["route"] != "reader" {
            return Err(error(
                "PUBLICATION_NOT_READY",
                "validation",
                "The existing build readiness gate has not admitted this material",
            ));
        }
        if !crate::is_valid_book_id(&book.base.book_id) {
            return Err(invalid());
        }
        let reference = PublishedBookRef {
            book_id: book.base.book_id.clone(),
            publication_id: uuid::Uuid::now_v7().to_string(),
        };
        let source = if stage.path().join("source_manifest.json").exists() {
            let source = read_json(&stage.path().join("source_manifest.json"))?;
            if source["book_id"].as_str() != Some(&reference.book_id)
                || source["canonical_source"]["sha256"].as_str() != Some(book.source_fingerprint())
            {
                return Err(invalid());
            }
            if matches!(
                crate::pdf_capability_status(&source, "view_pdf").as_deref(),
                Some("available" | "degraded")
            ) && !stage.path().join("original.pdf").is_file()
            {
                return Err(invalid());
            }
            let selection_usable = matches!(
                crate::pdf_capability_status(&source, "resolve_pdf_selection").as_deref(),
                Some("available" | "degraded")
            );
            let projection_usable = crate::pdf_capability_allows_runtime_map(&source);
            if projection_usable || selection_usable {
                if projection_usable && crate::route_pdf_source_map(stage.path()).status != 200 {
                    return Err(invalid());
                }
                if selection_usable {
                    let policy = crate::pdf_runtime_projection_policy(stage.path())
                        .map_err(|_| invalid())?;
                    crate::validate_pdf_runtime_policy_identity(
                        &policy,
                        &source,
                        &reference.book_id,
                        "resolve_pdf_selection",
                    )
                    .map_err(|_| invalid())?;
                    let selection = crate::selection_manifest_for_policy(stage.path(), &policy)
                        .map_err(|_| invalid())?;
                    for shard in selection["page_shards"].as_array().ok_or_else(invalid)? {
                        let page = shard["pageIndex"].as_u64().ok_or_else(invalid)? as usize;
                        crate::selection_hits_for_page(stage.path(), page, &[], &policy)
                            .map_err(|_| invalid())?;
                    }
                }
            }
            source
        } else {
            Value::Null
        };
        if stage.path().join("asset_manifest.json").exists() {
            let assets = read_json(&stage.path().join("asset_manifest.json"))?;
            if assets["book_id"].as_str() != Some(&reference.book_id)
                || assets["version"] != "asset_manifest.v1"
            {
                return Err(invalid());
            }
        }
        let teaching_readiness =
            serde_json::from_str(&crate::tutor_api::source_readiness(&book, stage.path()).body)
                .map_err(|_| invalid())?;
        let tutor_readiness = crate::tutor_api::tutor_source_readiness(&book, input);
        // Same book_id may gain capabilities, but canonical source and its actual attachment cannot change.
        let mut stmt = self
            .control
            .connection
            .prepare("SELECT directory FROM book_publications WHERE book_id=? LIMIT 1")
            .map_err(|_| storage())?;
        let previous: Vec<String> = stmt
            .query_map([&reference.book_id], |r| r.get(0))
            .map_err(|_| storage())?
            .collect::<Result<_, _>>()
            .map_err(|_| storage())?;
        drop(stmt);
        for directory in previous {
            let old = Path::new(&directory);
            if fs::read(old.join("source.txt")).map_err(|_| unavailable())?
                != fs::read(stage.path().join("source.txt")).map_err(|_| storage())?
                || optional_bytes(&old.join("original.pdf"))?
                    != optional_bytes(&stage.path().join("original.pdf"))?
            {
                return Err(error(
                    "BOOK_CONTENT_CHANGED",
                    "conflict",
                    "Changed source content or attachment requires a new book_id",
                ));
            }
            let old_manifest: PublicationManifest =
                serde_json::from_value(read_json(&old.join("publication.json"))?)
                    .map_err(|_| invalid())?;
            let mut current = BTreeMap::new();
            inventory(stage.path(), stage.path(), &mut current)?;
            let attachments: std::collections::BTreeSet<_> = old_manifest
                .files
                .keys()
                .chain(current.keys())
                .filter(|p| p.starts_with("assets/"))
                .collect();
            for attachment in attachments {
                if optional_bytes(&old.join(attachment))?
                    != optional_bytes(&stage.path().join(attachment))?
                {
                    return Err(error(
                        "BOOK_CONTENT_CHANGED",
                        "conflict",
                        "Changed source attachment requires a new book_id",
                    ));
                }
            }
        }
        let mut files = BTreeMap::new();
        inventory(stage.path(), stage.path(), &mut files)?;
        let mut artifact_versions = BTreeMap::new();
        for file in files.keys().filter(|p| p.ends_with(".json")) {
            let value = read_json(&stage.path().join(file))?;
            artifact_versions.insert(
                file.clone(),
                json!({"version": value["version"], "header": value["header"]}),
            );
        }
        let manifest = PublicationManifest {
            version: "book_publication.v1".into(),
            reference: reference.clone(),
            source_fingerprint: book.source_fingerprint().into(),
            files,
            capabilities: source["capabilities"].clone(),
            artifact_versions,
            teaching_readiness,
            tutor_readiness,
            build_readiness: snapshot["readiness"].clone(),
            resident_bytes: book.resident_budget_bytes(),
        };
        write_json(&stage.path().join("publication.json"), &manifest)?;
        let parent = root.join("published").join(&reference.book_id);
        memory::ReaderPrivateStorageGate::create_dir_all(&parent).map_err(|_| storage())?;
        let destination = parent.join(&reference.publication_id);
        // Moving a directory between parents needs write access to the directory
        // itself on Unix. Seal at its final path, before the registry exposes it.
        // Files become visible first; only the following transaction exposes a publication.
        // A crash before commit leaves an unregistered directory, never a readable partial entry.
        fs::rename(stage.path(), &destination).map_err(|_| storage())?;
        seal(&destination)?;
        memory::ReaderPrivateStorageGate::sync_parent(&destination).map_err(|_| storage())?;
        let tx = self
            .control
            .connection
            .transaction()
            .map_err(|_| storage())?;
        tx.execute("INSERT INTO book_publications(book_id,publication_id,directory,manifest) VALUES(?,?,?,?)",
            params![reference.book_id, reference.publication_id, destination.to_string_lossy(), serde_json::to_string(&manifest).map_err(|_| invalid())?]).map_err(|_| storage())?;
        tx.execute("INSERT INTO book_defaults(book_id,publication_id) VALUES(?,?) ON CONFLICT(book_id) DO UPDATE SET publication_id=excluded.publication_id",
            params![reference.book_id, reference.publication_id]).map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())?;
        Ok(manifest)
    }

    pub fn set_default(&mut self, reference: &PublishedBookRef) -> Result<(), ToolError> {
        self.record(reference)?;
        self.control.connection.execute("INSERT INTO book_defaults(book_id,publication_id) VALUES(?,?) ON CONFLICT(book_id) DO UPDATE SET publication_id=excluded.publication_id", params![reference.book_id, reference.publication_id]).map_err(|_| storage())?;
        Ok(())
    }
    pub fn default_ref(&self, book_id: &str) -> Result<PublishedBookRef, ToolError> {
        let publication_id = self
            .control
            .connection
            .query_row(
                "SELECT publication_id FROM book_defaults WHERE book_id=?",
                [book_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| storage())?
            .ok_or_else(unavailable)?;
        Ok(PublishedBookRef {
            book_id: book_id.into(),
            publication_id,
        })
    }
    /// Public catalog projection: no filesystem paths or other users' grants.
    pub fn list(&self, owner: &str) -> Result<Vec<Value>, ToolError> {
        let mut query = self.control.connection.prepare("SELECT p.manifest, COALESCE(d.publication_id=p.publication_id,0), p.directory FROM book_publications p JOIN book_grants g USING(book_id,publication_id) JOIN users u ON u.user_id=g.owner_user_id LEFT JOIN book_defaults d USING(book_id) WHERE g.owner_user_id=? AND u.disabled=0 ORDER BY p.book_id,p.publication_id").map_err(|_| storage())?;
        let rows = query
            .query_map([owner], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?, r.get::<_, String>(2)?))
            })
            .map_err(|_| storage())?;
        rows.map(|row| {
            let (manifest, is_default, directory) = row.map_err(|_| storage())?;
            let manifest: PublicationManifest = serde_json::from_str(&manifest).map_err(|_| invalid())?;
            let cover = crate::book_cover::published(Path::new(&directory), &manifest);
            Ok(json!({"published_book_ref":manifest.reference,"is_default":is_default,
                "source_fingerprint":manifest.source_fingerprint,"teaching_readiness":manifest.teaching_readiness,"cover":cover}))
        }).collect()
    }
    pub fn grant(&mut self, owner: &str, reference: &PublishedBookRef) -> Result<(), ToolError> {
        self.control.user_paths(owner)?;
        self.record(reference)?;
        self.control.connection.execute("INSERT OR IGNORE INTO book_grants(owner_user_id,book_id,publication_id) VALUES(?,?,?)", params![owner, reference.book_id, reference.publication_id]).map_err(|_| storage())?;
        Ok(())
    }
    pub fn authorize(&self, owner: &str, reference: &PublishedBookRef) -> Result<(), ToolError> {
        let allowed: bool = self.control.connection.query_row("SELECT EXISTS(SELECT 1 FROM book_grants g JOIN users u ON u.user_id=g.owner_user_id WHERE u.disabled=0 AND g.owner_user_id=? AND g.book_id=? AND g.publication_id=?)", params![owner, reference.book_id, reference.publication_id], |r| r.get(0)).map_err(|_| storage())?;
        if allowed {
            Ok(())
        } else {
            Err(unavailable())
        }
    }
    /// Trusted administrator operation. Reads, including cache hits, recheck this table.
    pub fn revoke(&mut self, owner: &str, reference: &PublishedBookRef) -> Result<(), ToolError> {
        self.control.connection.execute("DELETE FROM book_grants WHERE owner_user_id=? AND book_id=? AND publication_id=?", params![owner, reference.book_id, reference.publication_id]).map_err(|_| storage())?;
        Ok(())
    }
    fn record(
        &self,
        reference: &PublishedBookRef,
    ) -> Result<(PathBuf, PublicationManifest), ToolError> {
        let row: Option<(String, String)> = self.control.connection.query_row("SELECT directory,manifest FROM book_publications WHERE book_id=? AND publication_id=?", params![reference.book_id, reference.publication_id], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(|_| storage())?;
        let (directory, manifest) = row.ok_or_else(unavailable)?;
        Ok((
            directory.into(),
            serde_json::from_str(&manifest).map_err(|_| invalid())?,
        ))
    }
    pub fn resident_usage(&mut self) -> (usize, u64) {
        self.residents.retain(|_, e| e.book.strong_count() > 0);
        (
            self.residents.len(),
            self.residents.values().map(|e| e.bytes).sum(),
        )
    }
    pub fn evict_cache(&mut self) {
        for e in self.residents.values_mut() {
            e.cached = None;
        }
        self.resident_usage();
    }
    pub fn load(
        &mut self,
        owner: &str,
        reference: &PublishedBookRef,
    ) -> Result<Arc<PublishedBook>, ToolError> {
        self.authorize(owner, reference)?; // also on a cache hit
        let (directory, manifest) = self.record(reference)?;
        if !directory.is_dir() {
            return Err(unavailable());
        }
        self.clock += 1;
        let existing = self.residents.get(reference).and_then(|e| e.book.upgrade());
        let book = if let Some(book) = existing {
            book
        } else {
            loop {
                let (count, bytes) = self.resident_usage();
                if count < self.max_books
                    && bytes.saturating_add(manifest.resident_bytes) <= self.max_bytes
                {
                    break;
                }
                let victim = self
                    .residents
                    .iter()
                    .filter(|(_, e)| e.cached.is_some())
                    .min_by_key(|(_, e)| e.touched)
                    .map(|(k, _)| k.clone());
                let Some(victim) = victim else {
                    return Err(capacity());
                };
                self.residents.get_mut(&victim).unwrap().cached = None;
            }
            let book = Arc::new(
                Book::load(directory.to_str().ok_or_else(unavailable)?)
                    .map_err(|_| unavailable())?,
            );
            self.residents.insert(
                reference.clone(),
                ResidentEntry {
                    book: Arc::downgrade(&book),
                    cached: Some(book.clone()),
                    bytes: manifest.resident_bytes,
                    touched: self.clock,
                },
            );
            book
        };
        self.residents.get_mut(reference).unwrap().touched = self.clock;
        Ok(Arc::new(PublishedBook {
            reference: reference.clone(),
            book,
            manifest,
            directory,
            localization_cache: self
                .control
                .writer
                .root()
                .join("shared-cache")
                .join(&reference.publication_id)
                .join("paper-minimap-localizations.json"),
        }))
    }
    /// MU4 calls this after authenticating owner, before HEAD/Range/conditional handling.
    pub fn asset(
        &self,
        owner: &str,
        reference: &PublishedBookRef,
        relative_path: &str,
    ) -> Result<crate::BinaryReply, ToolError> {
        self.authorize(owner, reference)?;
        let (directory, manifest) = self.record(reference)?;
        asset(&directory, &manifest, relative_path)
    }

    /// Explicit publication + user-private route context, for the authenticated host.
    pub fn read(
        &mut self,
        owner: &str,
        reference: &PublishedBookRef,
        leaf: &str,
        query: &std::collections::HashMap<String, String>,
        user: &crate::user_runtime::UserRuntime,
    ) -> Result<crate::Reply, ToolError> {
        user.check_owner(owner)?;
        let publication = self.load(owner, reference)?;
        if matches!(leaf, "library" | "build_workbench") {
            return Err(unavailable());
        }
        let reply = if leaf == "teaching_readiness" {
            crate::ok_json(&publication.manifest.teaching_readiness)
        } else if leaf == "tutor_readiness" {
            crate::ok_json(&crate::tutor_api::readiness_view(&publication.book, &publication.directory))
        } else {
            crate::route_book(
                &publication.book,
                &publication.directory,
                &user.store,
                leaf,
                query,
            )
        };
        Ok(bind_reply(reply, reference))
    }

    pub fn open_workspace(
        &mut self,
        state: &mut crate::AppState,
        reference: &PublishedBookRef,
        now: &str,
    ) -> Result<(), ToolError> {
        let owner = state.user.user_id().ok_or_else(unavailable)?;
        let publication = self.load(owner, reference)?;
        state.user.store.flush_pending_reads()?;
        let mut history = state.user.agent_history.clone();
        crate::ensure_agent_history_for_book(&mut history, &reference.book_id, now);
        crate::commit_agent_history_candidate(state, history)?;
        state.workspace.bind_publication(publication);
        state
            .workspace
            .restore_chat(&mut state.user.agent_history, now);
        Ok(())
    }
}

fn optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, ToolError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(unavailable()),
    }
}

fn asset(
    directory: &Path,
    manifest: &PublicationManifest,
    path: &str,
) -> Result<crate::BinaryReply, ToolError> {
    relative(path)?;
    // Runtime JSON/text uses typed endpoints. Never expose arbitrary admitted metadata.
    if !manifest.files.contains_key(path)
        || !(path.starts_with("assets/") || path == "original.pdf")
    {
        return Err(unavailable());
    }
    let file = directory.join(path);
    regular_file(&file)?;
    Ok(crate::BinaryReply {
        status: 200,
        content_type: if path == "original.pdf" {
            "application/pdf"
        } else {
            crate::mime_for_asset(&file)
        }
        .into(),
        body: fs::read(file).map_err(|_| unavailable())?,
    })
}

impl PublishedBook {
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn asset(&self, path: &str) -> Result<crate::BinaryReply, ToolError> {
        asset(&self.directory, &self.manifest, path)
    }
}

pub(crate) fn bind_reply(mut reply: crate::Reply, reference: &PublishedBookRef) -> crate::Reply {
    if reply.status == 200 {
        if let Ok(mut value) = serde_json::from_str::<Value>(&reply.body) {
            if value["version"] == "asset_manifest.v1" {
                if let Some(images) = value["images"].as_array_mut() {
                    for image in images {
                        if let Some(url) = image["url_path"].as_str() {
                            if let Some(leaf) = url
                                .strip_prefix("/api/book/")
                                .or_else(|| url.strip_prefix("/book/"))
                            {
                                image["url_path"] = json!(reference.url(leaf));
                            }
                        }
                    }
                }
            }
            if let Some(fields) = value.as_object_mut() {
                fields.insert("published_book_ref".into(), json!(reference));
                fields.insert("resource_base".into(), json!(reference.url("")));
            }
            reply.body = value.to_string();
        }
    }
    reply
}
