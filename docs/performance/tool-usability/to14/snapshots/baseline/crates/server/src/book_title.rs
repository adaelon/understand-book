use std::{io::Read, path::Path};
use serde_json::Value;

/// Display metadata only; a title never changes a book's identity or publication.
pub(crate) fn read(directory: &Path, display_title: Option<&str>) -> Option<String> {
    let metadata = std::fs::read(directory.join("paper_metadata.json")).ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    metadata.as_ref().and_then(|value| value["title"]["value"].as_str())
        .map(str::trim).filter(|title| !title.is_empty()).map(str::to_owned)
        .or_else(|| display_title.map(str::trim).filter(|title| !title.is_empty()).map(str::to_owned))
        .or_else(|| {
            // The title is at the front of imported Markdown. Listing the library
            // must not load each book's entire source text.
            let mut bytes = Vec::new();
            std::fs::File::open(directory.join("source.txt")).ok()?
                .take(16 * 1024).read_to_end(&mut bytes).ok()?;
            String::from_utf8_lossy(&bytes).lines()
                .find_map(|line| line.trim_start_matches('\u{feff}').strip_prefix("# ")
                    .map(|title| {
                        let title = title.trim();
                        let without_closing_hashes = title.trim_end_matches('#');
                        if without_closing_hashes.ends_with(' ') { without_closing_hashes.trim_end() } else { title }
                    })
                    .filter(|title| !title.is_empty()).map(str::to_owned))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_title_uses_metadata_then_import_title_then_source_heading() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path();
        assert_eq!(read(dir, None), None);
        std::fs::write(dir.join("source.txt"), "# 让理解有出处\n\n正文").unwrap();
        assert_eq!(read(dir, None).as_deref(), Some("让理解有出处"));
        assert_eq!(read(dir, Some("导入标题")).as_deref(), Some("导入标题"));
        std::fs::write(dir.join("paper_metadata.json"), r#"{"title":{"value":"  正式论文标题  "}}"#).unwrap();
        assert_eq!(read(dir, Some("导入标题")).as_deref(), Some("正式论文标题"));
        std::fs::write(dir.join("paper_metadata.json"), r#"{"title":{"value":" "}}"#).unwrap();
        assert_eq!(read(dir, Some(" ")).as_deref(), Some("让理解有出处"));
        std::fs::write(dir.join("source.txt"), "# C# ##\n正文").unwrap();
        assert_eq!(read(dir, None).as_deref(), Some("C#"));
    }
}
