use std::{io::Read, path::Path};
use serde_json::Value;

/// Display metadata only; a title never changes a book's identity or publication.
pub fn read(directory: &Path, display_title: Option<&str>) -> Option<String> {
    // Catalog corrections live beside the sealed publication, preserving its
    // content, profile, identity, and existing Reader references.
    if let Some(title) = std::fs::read(directory.with_extension("display.json")).ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|value| value["title"].as_str().map(str::trim)
            .filter(|title| !title.is_empty()).map(str::to_owned))
    {
        return Some(title);
    }
    let metadata = std::fs::read(directory.join("paper_metadata.json")).ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    metadata.as_ref().and_then(|value| value["title"]["value"].as_str())
        .map(str::trim).filter(|title| !title.is_empty()).map(str::to_owned)
        .or_else(|| display_title.map(str::trim).filter(|title| !title.is_empty()).map(str::to_owned))
        .or_else(|| {
            let music: Value = serde_json::from_slice(&std::fs::read(directory.join("music_source_manifest.json")).ok()?).ok()?;
            music["title"].as_str().map(str::to_owned)
        })
        .or_else(|| {
            let video: Value = serde_json::from_slice(&std::fs::read(directory.join("video_source_manifest.json")).ok()?).ok()?;
            // Published transcripts have no heading. Keep the imported filename as
            // their display title, including when prepared on a different OS.
            let filename = video["source"]["original_path"].as_str()?.rsplit(['/', '\\']).next()?;
            Path::new(filename).file_stem()?.to_str().map(|title| title.replace('_', " "))
                .filter(|title| !title.trim().is_empty())
        })
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
    fn catalog_title_overrides_without_changing_publication_files() {
        let root = tempfile::tempdir().unwrap();
        let publication = root.path().join("publication-id");
        std::fs::create_dir(&publication).unwrap();
        std::fs::write(publication.join("source.txt"), "# Original title\nBody").unwrap();
        std::fs::write(publication.with_extension("display.json"),
            r#"{"title":"失败的逻辑：事情因何出错，世间有无妙策"}"#).unwrap();
        assert_eq!(read(&publication, Some("Imported title")).as_deref(),
            Some("失败的逻辑：事情因何出错，世间有无妙策"));
        assert_eq!(std::fs::read_to_string(publication.join("source.txt")).unwrap(), "# Original title\nBody");
        std::fs::write(publication.with_extension("display.json"), r#"{"title":" "}"#).unwrap();
        assert_eq!(read(&publication, None).as_deref(), Some("Original title"));
    }

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

    #[test]
    fn video_title_uses_original_filename_without_a_transcript_heading() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("source.txt"), "Today we will discuss tensors.\n").unwrap();
        for path in [r"E:\courses\CS336_Lecture02_PyTorch.mp4", "/courses/CS336_Lecture02_PyTorch.mp4"] {
            std::fs::write(root.path().join("video_source_manifest.json"), serde_json::json!({"source":{"original_path":path}}).to_string()).unwrap();
            assert_eq!(read(root.path(), None).as_deref(), Some("CS336 Lecture02 PyTorch"));
            assert_eq!(read(root.path(), Some("课程第二讲")).as_deref(), Some("课程第二讲"));
        }
    }
}
