use std::path::Path;
use serde_json::{json, Value};

/// Reader navigation corrections live beside a sealed publication. They refer
/// to existing leaves; they never replace the source tree or semantic chapters.
pub(crate) fn attach(directory: &Path, manifest: &mut Value) {
    let Some(entries) = std::fs::read(directory.with_extension("outline.json")).ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|value| project(&value, &manifest["tree"])) else { return };
    manifest["reader_outline"] = json!(entries);
}

fn project(value: &Value, tree: &Value) -> Option<Vec<Value>> {
    let nodes = tree.as_array()?;
    let entries = value["entries"].as_array()?;
    if entries.is_empty() { return None; }
    let end = nodes.iter().filter_map(|node| node["span"]["end"].as_u64()).max()?;
    let mut result: Vec<Value> = Vec::new();
    for entry in entries {
        let lid = entry["lid"].as_str()?;
        let title = entry["title"].as_str()?.trim();
        if title.is_empty() { return None; }
        let node = nodes.iter().find(|node| node["lid"] == lid && node["children"].as_array().is_some_and(Vec::is_empty))?;
        let start = node["span"]["start"].as_u64()?;
        if let Some(previous) = result.last_mut() {
            if previous["span"]["start"].as_u64()? >= start { return None; }
            previous["span"]["end"] = json!(start);
        }
        result.push(json!({"lid":lid,"title":title,"kind":"section","depth":0,"span":{"start":start,"end":end}}));
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tree() -> Value { json!([
        {"lid":"1","children":["1.1","1.2","1.3"],"span":{"start":0,"end":120}},
        {"lid":"1.1","children":[],"span":{"start":0,"end":20}},
        {"lid":"1.2","children":[],"span":{"start":20,"end":35}},
        {"lid":"1.3","children":[],"span":{"start":80,"end":120}}
    ]) }
    #[test]
    fn restored_navigation_uses_existing_positions_and_preserves_the_tree() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("publication");
        std::fs::write(directory.with_extension("outline.json"), json!({"entries":[
            {"lid":"1.2","title":"引言"},{"lid":"1.3","title":"第一章 若干事例"}
        ]}).to_string()).unwrap();
        let mut manifest = json!({"tree":tree()});
        attach(&directory, &mut manifest);
        assert_eq!(manifest["tree"], tree());
        assert_eq!(manifest["reader_outline"][0]["span"],json!({"start":20,"end":80}));
        assert_eq!(manifest["reader_outline"][1]["span"]["end"],120);
        assert_eq!(manifest["reader_outline"][1]["lid"],"1.3");
    }
    #[test]
    fn missing_or_reversed_anchors_do_not_create_misleading_navigation() {
        for entries in [json!([{"lid":"missing","title":"第一章"}]),
            json!([{"lid":"1.3","title":"第一章"},{"lid":"1.2","title":"第二章"}]),
            json!([{"lid":"1","title":"整书容器"}])] {
            assert!(project(&json!({"entries":entries}), &tree()).is_none());
        }
    }
}
