use std::path::{Component, Path};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct CoverDescriptor {
    pub kind: &'static str,
    pub url: String,
}

fn image_path(value: &Value, directory: &Path) -> Option<String> {
    let path = value.as_str()?;
    if !path.starts_with("assets/") || path.contains(['\\', ':'])
        || Path::new(path).components().any(|c| !matches!(c, Component::Normal(_)))
        || !crate::mime_for_asset(Path::new(path)).starts_with("image/")
        || !directory.join(path).is_file() {
        return None;
    }
    Some(path.into())
}

pub(crate) fn image(directory: &Path) -> Option<String> {
    let manifest: Value = serde_json::from_slice(&std::fs::read(directory.join("asset_manifest.json")).ok()?).ok()?;
    if let Some(path) = image_path(&manifest["cover"]["stored_path"], directory) { return Some(path); }
    // Existing bundles already contain some EPUB covers as ordinary image assets.
    // Only explicitly named covers qualify; a chapter's first figure does not.
    manifest["images"].as_array()?.iter().find_map(|entry| {
        if entry["status"] != "available" || entry["source"] != "epub" { return None; }
        let alt = entry["alt"].as_str().unwrap_or("").trim().to_lowercase();
        let src = entry["original_src"].as_str().unwrap_or("").to_lowercase();
        let stem = Path::new(src.split(['?', '#']).next().unwrap_or("")).file_stem()?.to_str()?;
        if matches!(alt.as_str(), "cover" | "front cover" | "封面") || matches!(stem, "cover" | "frontcover" | "front-cover") {
            image_path(&entry["stored_path"], directory)
        } else { None }
    })
}

pub(crate) fn local(directory: &Path) -> Option<CoverDescriptor> {
    let kind = if image(directory).is_some() { "image" }
        else if crate::original_pdf_path(directory).ok().is_some_and(|p| p.is_file()) { "pdf" }
        else { return None; };
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("dir", &crate::path_string(directory)).finish();
    Some(CoverDescriptor { kind, url: format!("/api/book/library/cover?{query}") })
}

pub(crate) fn published(directory: &Path, manifest: &crate::published_library::PublicationManifest) -> Option<CoverDescriptor> {
    if let Some(path) = image(directory).filter(|p| manifest.files.contains_key(p)) {
        return Some(CoverDescriptor { kind: "image", url: manifest.reference.url(&path) });
    }
    manifest.files.contains_key("original.pdf").then(|| CoverDescriptor {
        kind: "pdf", url: manifest.reference.url("pdf/original"),
    })
}

pub(crate) fn route_local(state: &crate::AppState, url: &str) -> Option<crate::BinaryReply> {
    let (path, query) = crate::parse_query(url);
    if path != "/book/library/cover" { return None; }
    let requested = query.get("dir");
    let book = crate::list_mixed_book_library(&crate::state_library_root(state)).into_iter()
        .find(|entry| Some(&entry.dir) == requested);
    if let Some(book) = book {
        let directory = Path::new(&book.dir);
        if let Some(image) = image(directory) {
            return crate::route_book_asset_file(directory, &format!("/book/{image}"));
        }
        if crate::original_pdf_path(directory).is_ok() { return Some(crate::route_original_pdf_file(directory)); }
    }
    Some(crate::BinaryReply { status: 404, content_type: "text/plain; charset=utf-8".into(), body: b"cover not found".to_vec() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn book_cover_prefers_declared_image_and_recognizes_existing_epub_cover() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("assets")).unwrap();
        std::fs::write(root.path().join("assets/cover.png"), b"image").unwrap();
        let entry = json!({"source":"epub", "status":"available", "original_src":"Images/cover.jpg", "stored_path":"assets/cover.png"});
        std::fs::write(root.path().join("asset_manifest.json"), json!({"images":[entry]}).to_string()).unwrap();
        assert_eq!(image(root.path()).as_deref(), Some("assets/cover.png"));
        std::fs::write(root.path().join("asset_manifest.json"), json!({"images":[{"source":"epub", "status":"available", "original_src":"figure.png", "stored_path":"assets/cover.png"}]}).to_string()).unwrap();
        assert!(image(root.path()).is_none());
        std::fs::write(root.path().join("asset_manifest.json"), json!({"cover":{"stored_path":"assets/cover.png"}}).to_string()).unwrap();
        assert_eq!(local(root.path()).unwrap().kind, "image");
    }
}
