//! Contract admission against a loaded book. This does not register media as observed.
use crate::{Book, ToolError};
use base_schema::{EvidenceTarget, TranscriptMap, VideoSourceManifest};

fn invalid(message: String) -> ToolError {
    ToolError {
        error_code: "INVALID_VIDEO_SOURCE".into(),
        category: "invalid_input".into(),
        message,
    }
}

impl Book {
    /// Load metadata only. Original media remains file-backed, outside Book residency.
    pub fn load_video_source(&self, dir: &std::path::Path) -> Result<(VideoSourceManifest, TranscriptMap), ToolError> {
        let read = |name: &str| std::fs::read(dir.join(name)).map_err(|e| invalid(e.to_string()));
        let manifest: VideoSourceManifest = serde_json::from_slice(&read("video_source_manifest.json")?)
            .map_err(|e| invalid(e.to_string()))?;
        manifest.validate().map_err(invalid)?;
        let map: TranscriptMap = serde_json::from_slice(&read(&manifest.transcript_map_path)?)
            .map_err(|e| invalid(e.to_string()))?;
        self.validate_video_source(&manifest, &map)?;
        let full = manifest.coverage.iter().find(|c| c.capability == base_schema::VideoCapability::Transcript)
            .is_some_and(|c| c.gaps(&manifest.source).is_ok_and(|g| g.is_empty()));
        if !full || map.segments.is_empty() {
            return Err(invalid("Full transcript coverage is required".into()));
        }
        for name in [&manifest.source.imported_path, &map.raw_result_path] {
            if !std::fs::metadata(dir.join(name)).is_ok_and(|m| m.is_file() && m.len() > 0) {
                return Err(invalid(format!("Missing video dependency: {name}")));
            }
        }
        Ok((manifest, map))
    }

    pub fn validate_video_source(
        &self,
        manifest: &VideoSourceManifest,
        map: &TranscriptMap,
    ) -> Result<(), ToolError> {
        map.validate(
            manifest,
            &self.base,
            &String::from_utf16_lossy(&self.source_u16),
        )
        .map_err(invalid)
    }

    /// Structural validity only; the caller must still verify this turn's observation receipt.
    pub fn validate_video_evidence_target(
        &self,
        manifest: &VideoSourceManifest,
        target: &EvidenceTarget,
    ) -> Result<(), ToolError> {
        target
            .validate(
                manifest,
                &self.base,
                &String::from_utf16_lossy(&self.source_u16),
            )
            .map_err(invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base_schema::ReadOnlyBase;
    #[test]
    fn video_contract_admission_uses_loaded_canonical_text_and_preserves_text_sources() {
        let f: serde_json::Value = serde_json::from_str(include_str!(
            "../../base-schema/fixtures/video-contract.json"
        ))
        .unwrap();
        let c = &f["cases"][0];
        let manifest: VideoSourceManifest = serde_json::from_value(c["manifest"].clone()).unwrap();
        let mut map: TranscriptMap = serde_json::from_value(c["map"].clone()).unwrap();
        let base: ReadOnlyBase = serde_json::from_value(c["base"].clone()).unwrap();
        let book = Book::new(base.clone(), c["source_text"].as_str().unwrap());
        book.validate_video_source(&manifest, &map).unwrap();
        let target: EvidenceTarget = serde_json::from_value(c["targets"][1].clone()).unwrap();
        book.validate_video_evidence_target(&manifest, &target)
            .unwrap();
        assert!(book.text("1.1", None).unwrap().contains("underflow"));
        map.segments[0].text_range.range.end += 1;
        assert_eq!(
            book.validate_video_source(&manifest, &map)
                .unwrap_err()
                .error_code,
            "INVALID_VIDEO_SOURCE"
        );
        let mut other = base;
        other.book_id = "other".into();
        assert!(Book::new(other, c["source_text"].as_str().unwrap())
            .validate_video_evidence_target(&manifest, &target)
            .is_err());
    }
}
