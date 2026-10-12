//! Host-owned libraries are frozen in each version, never returned as authoring text.
use runtime::presentation_author::PresentationLibrary;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const PATH: &str = "libraries/konva-10.7.0.min.js";
const LICENSE_PATH: &str = "libraries/konva-10.7.0.LICENSE.txt";
const SOURCE: &str = include_str!("../../../assets/presentation/konva-10.7.0/konva.min.js");
const LICENSE: &str = include_str!("../../../assets/presentation/konva-10.7.0/LICENSE");

fn reference(path: &str) -> String {
    format!(r#"<script data-presentation-library="konva" src="{path}"></script>"#)
}

pub(super) fn metadata(files: &BTreeMap<String, String>) -> Vec<Value> {
    files
        .iter()
        .filter_map(|(path, source)| {
            let version = path
                .strip_prefix("libraries/konva-")?
                .strip_suffix(".min.js")?;
            let license_path = format!("libraries/konva-{version}.LICENSE.txt");
            Some(json!({"name":"konva", "version":version, "license":"MIT",
            "path":path, "bytes":source.len(), "license_path":license_path,
            "license_bytes":files.get(&license_path).map(String::len)}))
        })
        .collect()
}

/// Remove the exact host-owned references returned by read, including old versions.
pub(super) fn remove_references(html: &str, files: &BTreeMap<String, String>) -> String {
    let mut html = html.replace(&reference(PATH), "");
    for library in metadata(files) {
        html = html.replace(&reference(library["path"].as_str().unwrap()), "");
    }
    html
}

pub(super) fn assemble(
    html: &str,
    libraries: &[PresentationLibrary],
    files: &mut BTreeMap<String, String>,
) -> String {
    let mut html = remove_references(html, files);
    // A revision explicitly selects its dependencies; do not inherit retired libraries.
    files.retain(|path, _| !path.starts_with("libraries/"));
    if libraries.contains(&PresentationLibrary::Konva) {
        files.insert(PATH.into(), SOURCE.into());
        files.insert(LICENSE_PATH.into(), LICENSE.into());
        // Before the first business script, in either a full document or HTML fragment.
        let at = html
            .to_ascii_lowercase()
            .find("<script")
            .unwrap_or(html.len());
        html.insert_str(at, &reference(PATH));
    }
    html
}

/// Only expand references owned by this version, never the current bundled source.
pub(super) fn inline(html: &str, files: &BTreeMap<String, String>) -> String {
    let mut html = html.to_owned();
    for library in metadata(files) {
        let path = library["path"].as_str().unwrap();
        html = html.replace(
            &reference(path),
            &format!("<script>{}</script>", files[path]),
        );
    }
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ex11_saved_library_is_used_and_replaced_only_on_revision() {
        let old_path = "libraries/konva-10.6.0.min.js";
        let old_source = "window.savedLibraryVersion = '10.6.0';";
        let mut files = BTreeMap::from([
            (old_path.into(), old_source.into()),
            ("libraries/konva-10.6.0.LICENSE.txt".into(), "MIT".into()),
        ]);
        let html = format!("{}<script>scene()</script>", reference(old_path));
        assert_eq!(metadata(&files)[0]["version"], "10.6.0");
        assert_eq!(inline(&html, &files), format!("<script>{old_source}</script><script>scene()</script>"));
        let revised = assemble(&html, &[PresentationLibrary::Konva], &mut files);
        assert!(!revised.contains("10.6.0"));
        assert!(!files.contains_key(old_path));
        assert_eq!(revised.matches("data-presentation-library").count(), 1);
        assert_eq!(files[PATH], SOURCE);
    }
}
