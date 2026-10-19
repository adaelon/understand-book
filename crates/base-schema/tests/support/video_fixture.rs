use base_schema::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fixture {
    pub provenance: serde_json::Value,
    pub cases: Vec<Case>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Case {
    pub manifest: VideoSourceManifest,
    pub map: TranscriptMap,
    pub base: ReadOnlyBase,
    pub source_text: String,
    pub questions: Vec<VideoQuestionContext>,
    pub targets: Vec<EvidenceTarget>,
    pub observations: Vec<MediaObservation>,
    pub translations: Vec<TranscriptTranslation>,
}
impl Case {
    pub fn validate(&self) -> Result<(), String> {
        self.map
            .validate(&self.manifest, &self.base, &self.source_text)?;
        for q in &self.questions {
            q.validate(&self.manifest)?;
        }
        for t in &self.targets {
            t.validate(&self.manifest, &self.base, &self.source_text)?;
        }
        for o in &self.observations {
            o.validate(&self.manifest)?;
        }
        for t in &self.translations {
            t.validate(&self.map)?;
        }
        Ok(())
    }
}
