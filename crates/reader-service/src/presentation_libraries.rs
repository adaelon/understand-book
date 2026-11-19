//! Host-owned libraries are frozen in each version, never returned as authoring text.
use runtime::presentation_author::PresentationLibrary;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const PATH: &str = "libraries/konva-10.7.0.min.js";
const LICENSE_PATH: &str = "libraries/konva-10.7.0.LICENSE.txt";
const SOURCE: &str = include_str!("../../../assets/presentation/konva-10.7.0/konva.min.js");
const LICENSE: &str = include_str!("../../../assets/presentation/konva-10.7.0/LICENSE");
const THREE_PATH: &str = "libraries/three-0.180.0.min.js";
const THREE_LICENSE_PATH: &str = "libraries/three-0.180.0.LICENSE.txt";
const THREE_SOURCE: &str = include_str!("../../../assets/presentation/three-0.180.0/three.min.js");
const THREE_LICENSE: &str = include_str!("../../../assets/presentation/three-0.180.0/LICENSE");

fn reference(name: &str, path: &str) -> String {
    format!(r#"<script data-presentation-library="{name}" src="{path}"></script>"#)
}

pub(super) fn metadata(files: &BTreeMap<String, String>) -> Vec<Value> {
    files
        .iter()
        .filter_map(|(path, source)| {
            let (name, version) = ["konva", "three"].into_iter().find_map(|name| {
                path.strip_prefix(&format!("libraries/{name}-"))?
                    .strip_suffix(".min.js").map(|version| (name, version))
            })?;
            let license_path = format!("libraries/{name}-{version}.LICENSE.txt");
            Some(json!({"name":name, "version":version, "license":"MIT",
            "path":path, "bytes":source.len(), "license_path":license_path,
            "license_bytes":files.get(&license_path).map(String::len)}))
        })
        .collect()
}

/// Remove the exact host-owned references returned by read, including old versions.
pub(super) fn remove_references(html: &str, files: &BTreeMap<String, String>) -> String {
    let mut html = html.replace(&reference("konva", PATH), "").replace(&reference("three", THREE_PATH), "");
    for library in metadata(files) {
        html = html.replace(&reference(library["name"].as_str().unwrap(), library["path"].as_str().unwrap()), "");
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
    for (library, name, path, source, license_path, license) in [
        (PresentationLibrary::Konva, "konva", PATH, SOURCE, LICENSE_PATH, LICENSE),
        (PresentationLibrary::Three, "three", THREE_PATH, THREE_SOURCE, THREE_LICENSE_PATH, THREE_LICENSE),
    ] {
        if !libraries.contains(&library) { continue; }
        files.insert(path.into(), source.into());
        files.insert(license_path.into(), license.into());
        // Before the first business script, in either a full document or HTML fragment.
        let at = html
            .to_ascii_lowercase()
            .find("<script")
            .unwrap_or(html.len());
        html.insert_str(at, &reference(name, path));
    }
    html
}

/// Only expand references owned by this version, never the current bundled source.
pub(super) fn inline(html: &str, files: &BTreeMap<String, String>) -> String {
    let mut html = html.to_owned();
    for library in metadata(files) {
        let path = library["path"].as_str().unwrap();
        html = html.replace(
            &reference(library["name"].as_str().unwrap(), path),
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
        let html = format!("{}<script>scene()</script>", reference("konva", old_path));
        assert_eq!(metadata(&files)[0]["version"], "10.6.0");
        assert_eq!(inline(&html, &files), format!("<script>{old_source}</script><script>scene()</script>"));
        let revised = assemble(&html, &[PresentationLibrary::Konva], &mut files);
        assert!(!revised.contains("10.6.0"));
        assert!(!files.contains_key(old_path));
        assert_eq!(revised.matches("data-presentation-library").count(), 1);
        assert_eq!(files[PATH], SOURCE);
    }

    #[test]
    fn spatial_libraries_freeze_inline_and_retire_independently() {
        let mut files = BTreeMap::new();
        let html = "<script>const scene = new THREE.Scene();</script>";
        let built = assemble(html, &[PresentationLibrary::Three, PresentationLibrary::Konva], &mut files);
        assert_eq!(metadata(&files).len(), 2);
        assert_eq!(files[THREE_PATH], THREE_SOURCE);
        assert!(built.find(THREE_PATH).unwrap() < built.find("const scene").unwrap());
        assert!(inline(&built, &files).contains(THREE_SOURCE));
        assert_eq!(remove_references(&built, &files), html);
        let revised = assemble(&built, &[PresentationLibrary::Three], &mut files);
        assert!(!files.contains_key(PATH));
        assert_eq!(revised.matches("data-presentation-library").count(), 1);
        assert_eq!(metadata(&files)[0]["version"], "0.180.0");
        assert_eq!(assemble(&revised, &[], &mut files), html);
        assert!(files.is_empty());
    }
}
