//! Immutable publication data and file reads shared by local and cloud hosts.
use crate::user_storage_paths::error;
use read_tools::{Book, ToolError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::{Component, Path, PathBuf}, sync::Arc};

pub use base_schema::PublishedBookRef;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationManifest {
    pub version: String,
    pub reference: PublishedBookRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_publication: Option<PublishedBookRef>,
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

pub fn invalid() -> ToolError {
    error(
        "PUBLICATION_INVALID",
        "validation",
        "Publication materials or declared dependencies are invalid",
    )
}
pub fn unavailable() -> ToolError {
    error(
        "PUBLICATION_UNAVAILABLE",
        "not_found",
        "The exact publication is unavailable",
    )
}
pub fn storage() -> ToolError {
    error(
        "PUBLICATION_STORAGE_FAILED",
        "unavailable",
        "Publication could not be read or committed",
    )
}
pub fn relative(path: &str) -> Result<(), ToolError> {
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
pub fn regular_file(path: &Path) -> Result<(), ToolError> {
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
pub fn read_json(path: &Path) -> Result<Value, ToolError> {
    regular_file(path)?;
    serde_json::from_slice(&fs::read(path).map_err(|_| storage())?).map_err(|_| invalid())
}
pub fn write_json(path: &Path, value: &impl Serialize) -> Result<(), ToolError> {
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|_| invalid())?,
    )
    .map_err(|_| storage())
}
pub fn copy_file(input: &Path, stage: &Path, rel: &str) -> Result<(), ToolError> {
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

// Build Engine Markdown/EPUB manifests describe the imported file as provenance.
// Their canonical runtime bytes are truth_file, and they do not declare v2 PDF capabilities.
pub fn canonical_source_v1(source: &Value) -> bool {
    source.get("version").is_none()
        && matches!(source["canonical_source"]["kind"].as_str(), Some("markdown" | "epub"))
        && source["canonical_source"]["path"].as_str().is_some_and(|path| !path.trim().is_empty())
        && source["canonical_source"]["truth_file"] == "source.txt"
        && source["canonical_source"]["participates_in_lid"] == true
        && source["canonical_source"]["citation_anchor"] == "lid"
        && source["attachments"].is_array()
}

pub fn equal_files(left: &Path, right: &Path) -> Result<bool, ToolError> {
    use std::io::Read;
    let mut left = fs::File::open(left).map_err(|_|unavailable())?;
    let mut right = fs::File::open(right).map_err(|_|invalid())?;
    if left.metadata().map_err(|_|storage())?.len() != right.metadata().map_err(|_|storage())?.len() { return Ok(false); }
    let mut a = [0u8; 65536]; let mut b = [0u8; 65536];
    loop {
        let n = left.read(&mut a).map_err(|_|storage())?;
        if n == 0 { return Ok(true); }
        right.read_exact(&mut b[..n]).map_err(|_|storage())?;
        if a[..n] != b[..n] { return Ok(false); }
    }
}

pub fn asset(
    directory: &Path,
    manifest: &PublicationManifest,
    path: &str,
) -> Result<crate::BinaryReply, ToolError> {
    let video_cover = path == "video/cover";
    let path = if video_cover { crate::book_cover::video_frame(manifest).ok_or_else(unavailable)? } else { path };
    relative(path)?;
    // Runtime JSON/text uses typed endpoints. Never expose arbitrary admitted metadata.
    if !manifest.files.contains_key(path)
        || !(video_cover || path.starts_with("assets/") || path == "original.pdf")
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
    /// Construct an already admitted immutable publication using host-owned paths.
    pub fn new(book: Arc<Book>, manifest: PublicationManifest, directory: PathBuf, localization_cache: PathBuf) -> Self {
        Self { reference: manifest.reference.clone(), book, manifest, directory, localization_cache }
    }
    pub fn localization_cache(&self) -> &Path {
        &self.localization_cache
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn asset(&self, path: &str) -> Result<crate::BinaryReply, ToolError> {
        asset(&self.directory, &self.manifest, path)
    }
}

pub fn bind_reply(mut reply: crate::Reply, reference: &PublishedBookRef) -> crate::Reply {
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
