//! VI1 media contracts. Locations refer to imported bytes, never to revisable Visual Units.
use crate::{ReadOnlyBase, Span};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct VideoSourceRef {
    pub media_id: String,
    pub media_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct TranscriptRef {
    pub transcript_id: String,
    pub transcript_revision: String,
    pub video: VideoSourceRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct VideoMaterial {
    pub book_id: String,
    pub video: VideoSourceRef,
    pub transcript: TranscriptRef,
}

/// Milliseconds on the original timeline, half-open [start_ms, end_ms).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct MediaLocation {
    pub video: VideoSourceRef,
    pub start_ms: f64,
    pub end_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct FrameLocation {
    pub video: VideoSourceRef,
    pub actual_pts_ms: f64,
    /// Decoded original frame dimensions, including for a crop.
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct FrameRead {
    pub requested_ms: f64,
    pub frame: FrameLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct FrameRegion {
    pub frame: FrameLocation,
    /// x, y, width, height, normalized against the decoded original frame.
    pub rect_normalized: [f64; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct VideoSourceAsset {
    pub video: VideoSourceRef,
    pub imported_path: String,
    pub original_path: String,
    pub duration_ms: f64,
    pub width: u32,
    pub height: u32,
}

/// Durable preparation state before a transcript-backed Book exists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct VideoImport {
    pub version: String,
    pub video: VideoSourceRef,
    pub title: String,
    pub original_path: String,
    pub stage: VideoImportStage,
    pub copied_bytes: f64,
    pub total_bytes: f64,
    pub source: Option<VideoSourceAsset>,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub enum VideoImportStage {
    Copying,
    Probing,
    AwaitingTranscript,
    Failed,
}

impl VideoImport {
    pub fn validate(&self) -> Result<(), String> {
        require(self.version == "video_import.v1" && valid_video(&self.video)
            && nonempty(&self.title) && nonempty(&self.original_path)
            && self.total_bytes.is_finite() && self.total_bytes > 0.0
            && self.copied_bytes.is_finite() && self.copied_bytes >= 0.0
            && self.copied_bytes <= self.total_bytes, "video import")?;
        if let Some(source) = &self.source {
            source.validate()?;
            require(source.video == self.video && source.imported_path == "media/original.mp4", "import source")?;
        }
        require(self.stage != VideoImportStage::AwaitingTranscript
            || (self.source.is_some() && self.copied_bytes == self.total_bytes), "import readiness")?;
        require(self.stage != VideoImportStage::Failed
            || self.failure.as_ref().is_some_and(|v| nonempty(v)), "import failure")
    }
}

/// Offsets are local to the named LID, in UTF-16 code units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct TranscriptTextRange {
    pub lid: String,
    pub range: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct TranscriptWord {
    /// UTF-16 offsets within TranscriptSegment.text; times remain machine estimates.
    pub range: Span,
    /// Original timeline; Whisper can return coincident endpoints. The containing segment
    /// owns media identity. An empty word estimate is not a playable MediaLocation.
    pub start_ms: f64,
    pub end_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct TranscriptSegment {
    pub segment_id: String,
    pub text: String,
    pub location: MediaLocation,
    pub text_range: TranscriptTextRange,
    /// Empty when only segment timing is available; no character-to-time interpolation.
    pub words: Vec<TranscriptWord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct TranscriptMap {
    pub transcript: TranscriptRef,
    pub raw_result_path: String,
    pub machine_transcription: TranscriptProvenance,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct TranscriptProvenance {
    pub engine: String,
    pub model: String,
    pub language: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct TranscriptTranslation {
    pub transcript: TranscriptRef,
    pub segment_id: String,
    pub zh_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub enum VideoCapability {
    Transcript,
    Translation,
    VisualIndex,
    Graph,
    Discourse,
    AudioReview,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct VideoCoverage {
    pub capability: VideoCapability,
    /// Accepted processing intervals (including successfully processed silence).
    pub accepted: Vec<MediaLocation>,
    pub pending: Vec<MediaLocation>,
    pub failed: Vec<MediaLocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct VideoSourceManifest {
    pub version: String,
    pub material: VideoMaterial,
    pub source: VideoSourceAsset,
    pub canonical_text_path: String,
    pub transcript_map_path: String,
    pub coverage: Vec<VideoCoverage>,
    pub previous_book_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct VideoQuestionContext {
    pub draft_id: String,
    pub publication: PublishedBookRef,
    pub material: VideoMaterial,
    pub frame: FrameLocation,
    pub region: Option<FrameRegion>,
    pub range: Option<MediaLocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub enum EvidenceTarget {
    Text {
        publication: PublishedBookRef,
        transcript: Option<TranscriptRef>,
        text_range: TranscriptTextRange,
    },
    Visual {
        publication: PublishedBookRef,
        frames: Vec<FrameLocation>,
        regions: Vec<FrameRegion>,
    },
    Audio {
        publication: PublishedBookRef,
        location: MediaLocation,
        review_result_ref: String,
    },
    Joint {
        members: Vec<JointEvidenceMember>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub struct JointEvidenceMember {
    pub target: EvidenceTarget,
    pub support_role: String,
}

/// A receipt's shape is not proof of observation. VI5 creates receipts only after successful
/// processing, and VI7 checks membership in the current turn's ledger before source.present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
pub enum MediaObservation {
    Visual {
        observation_id: String,
        run_id: String,
        publication: PublishedBookRef,
        model_request_id: String,
        frames: Vec<FrameRead>,
        regions: Vec<FrameRegion>,
    },
    Audio {
        observation_id: String,
        run_id: String,
        publication: PublishedBookRef,
        tool_call_id: String,
        location: MediaLocation,
        review_result_ref: String,
    },
}

type Validation = Result<(), String>;
fn require(ok: bool, field: &str) -> Validation {
    if ok {
        Ok(())
    } else {
        Err(format!("invalid video contract: {field}"))
    }
}
fn nonempty(value: &str) -> bool {
    !value.trim().is_empty()
}
fn relative(value: &str) -> bool {
    nonempty(value)
        && !value.starts_with(['/', '\\'])
        && !value.contains(':')
        && !value.split(['/', '\\']).any(|s| s == "..")
}
fn valid_video(video: &VideoSourceRef) -> bool {
    nonempty(&video.media_id) && nonempty(&video.media_revision)
}
fn valid_transcript(t: &TranscriptRef) -> bool {
    nonempty(&t.transcript_id) && nonempty(&t.transcript_revision) && valid_video(&t.video)
}
impl VideoSourceAsset {
    pub fn validate(&self) -> Validation {
        require(
            valid_video(&self.video)
                && relative(&self.imported_path)
                && nonempty(&self.original_path)
                && self.duration_ms.is_finite()
                && self.duration_ms > 0.0
                && self.width > 0
                && self.height > 0,
            "source",
        )
    }
    pub fn validate_location(&self, p: &MediaLocation) -> Validation {
        require(
            p.video == self.video
                && valid_video(&p.video)
                && p.start_ms.is_finite()
                && p.end_ms.is_finite()
                && p.start_ms >= 0.0
                && p.start_ms < p.end_ms
                && p.end_ms <= self.duration_ms,
            "location",
        )
    }
    pub fn validate_frame(&self, f: &FrameLocation) -> Validation {
        require(
            f.video == self.video
                && valid_video(&f.video)
                && f.actual_pts_ms.is_finite()
                && f.actual_pts_ms >= 0.0
                && f.actual_pts_ms < self.duration_ms
                && f.width == self.width
                && f.height == self.height,
            "frame",
        )
    }
    pub fn validate_region(&self, r: &FrameRegion) -> Validation {
        self.validate_frame(&r.frame)?;
        let [x, y, w, h] = r.rect_normalized;
        require(
            r.rect_normalized.iter().all(|n| n.is_finite())
                && x >= 0.0
                && y >= 0.0
                && w > 0.0
                && h > 0.0
                && x + w <= 1.0
                && y + h <= 1.0,
            "region",
        )
    }
}

impl VideoCoverage {
    pub fn validate(&self, source: &VideoSourceAsset) -> Validation {
        let mut intervals: Vec<_> = self
            .accepted
            .iter()
            .chain(&self.pending)
            .chain(&self.failed)
            .collect();
        for p in &intervals {
            source.validate_location(p)?;
        }
        intervals.sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms));
        require(
            intervals.windows(2).all(|p| p[0].end_ms <= p[1].start_ms),
            "coverage overlap",
        )
    }
    /// Complement of accepted intervals, not of attempted intervals.
    pub fn gaps(&self, source: &VideoSourceAsset) -> Result<Vec<MediaLocation>, String> {
        self.validate(source)?;
        let mut accepted: Vec<_> = self.accepted.iter().collect();
        accepted.sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms));
        let mut cursor = 0.0;
        let mut result = vec![];
        for p in accepted {
            if cursor < p.start_ms {
                result.push(MediaLocation {
                    video: source.video.clone(),
                    start_ms: cursor,
                    end_ms: p.start_ms,
                });
            }
            cursor = p.end_ms;
        }
        if cursor < source.duration_ms {
            result.push(MediaLocation {
                video: source.video.clone(),
                start_ms: cursor,
                end_ms: source.duration_ms,
            });
        }
        Ok(result)
    }
}

impl VideoSourceManifest {
    pub fn validate(&self) -> Validation {
        self.source.validate()?;
        require(
            self.version == "video_source_manifest.v1"
                && nonempty(&self.material.book_id)
                && valid_transcript(&self.material.transcript)
                && self.material.video == self.source.video
                && self.material.transcript.video == self.source.video,
            "material",
        )?;
        require(
            self.canonical_text_path == "source.txt" && relative(&self.transcript_map_path),
            "manifest paths",
        )?;
        if let Some(previous) = &self.previous_book_id {
            require(
                nonempty(previous) && previous != &self.material.book_id,
                "previous_book_id",
            )?;
        }
        let mut seen = vec![];
        for c in &self.coverage {
            require(!seen.contains(&c.capability), "duplicate capability")?;
            seen.push(c.capability);
            c.validate(&self.source)?;
        }
        Ok(())
    }
    pub fn validate_publication(&self, p: &PublishedBookRef) -> Validation {
        require(
            p.book_id == self.material.book_id && nonempty(&p.publication_id),
            "publication",
        )
    }
    /// Compare explicit material identities; text equality and filesystem paths are not identity.
    pub fn validate_successor(&self, previous: &Self) -> Validation {
        self.validate()?;
        previous.validate()?;
        let same_source = self.material.video == previous.material.video
            && self.material.transcript == previous.material.transcript;
        if same_source {
            require(
                self.material.book_id == previous.material.book_id
                    && self.previous_book_id == previous.previous_book_id,
                "same-source book identity",
            )
        } else {
            require(
                self.material.book_id != previous.material.book_id
                    && self.previous_book_id.as_ref() == Some(&previous.material.book_id),
                "source revision predecessor",
            )
        }
    }
}

fn text_at(
    range: &TranscriptTextRange,
    base: &ReadOnlyBase,
    source: &[u16],
) -> Result<Vec<u16>, String> {
    let node = base
        .lid_nodes
        .iter()
        .find(|n| n.lid == range.lid)
        .ok_or("invalid video contract: LID")?;
    require(
        node.children.is_empty()
            && node.span.start <= node.span.end
            && node.span.end <= source.len(),
        "LID span",
    )?;
    require(
        range.range.start < range.range.end && range.range.end <= node.span.end - node.span.start,
        "text range",
    )?;
    Ok(source[node.span.start + range.range.start..node.span.start + range.range.end].to_vec())
}

impl TranscriptMap {
    pub fn validate(
        &self,
        manifest: &VideoSourceManifest,
        base: &ReadOnlyBase,
        source: &str,
    ) -> Validation {
        manifest.validate()?;
        require(
            self.transcript == manifest.material.transcript
                && base.book_id == manifest.material.book_id
                && relative(&self.raw_result_path)
                && nonempty(&self.machine_transcription.engine)
                && nonempty(&self.machine_transcription.model)
                && nonempty(&self.machine_transcription.language),
            "transcript binding",
        )?;
        let source: Vec<u16> = source.encode_utf16().collect();
        let mut seen = std::collections::HashSet::new();
        let mut previous_end = 0;
        let mut previous_time = 0.0;
        for s in &self.segments {
            require(
                nonempty(&s.segment_id) && seen.insert(&s.segment_id) && nonempty(&s.text),
                "segment identity",
            )?;
            manifest.source.validate_location(&s.location)?;
            require(
                text_at(&s.text_range, base, &source)? == s.text.encode_utf16().collect::<Vec<_>>(),
                "mapped text",
            )?;
            let node = base
                .lid_nodes
                .iter()
                .find(|n| n.lid == s.text_range.lid)
                .unwrap();
            let start = node.span.start + s.text_range.range.start;
            require(
                start >= previous_end && s.location.start_ms >= previous_time,
                "segment order",
            )?;
            previous_end = node.span.start + s.text_range.range.end;
            previous_time = s.location.end_ms;
            let mut word_end = 0;
            let mut time_end = s.location.start_ms;
            for w in &s.words {
                require(
                    w.range.start >= word_end
                        && w.range.start < w.range.end
                        && w.range.end <= s.text.encode_utf16().count()
                        && w.start_ms.is_finite()
                        && w.end_ms.is_finite()
                        && w.start_ms <= w.end_ms
                        && w.start_ms >= time_end
                        && w.end_ms <= s.location.end_ms,
                    "word mapping",
                )?;
                word_end = w.range.end;
                time_end = w.end_ms;
            }
        }
        Ok(())
    }
}

impl TranscriptTranslation {
    pub fn validate(&self, map: &TranscriptMap) -> Validation {
        require(
            self.transcript == map.transcript
                && nonempty(&self.zh_text)
                && map.segments.iter().any(|s| s.segment_id == self.segment_id),
            "translation",
        )
    }
}

impl VideoQuestionContext {
    pub fn validate(&self, manifest: &VideoSourceManifest) -> Validation {
        manifest.validate()?;
        manifest.validate_publication(&self.publication)?;
        require(
            nonempty(&self.draft_id) && self.material == manifest.material,
            "question material",
        )?;
        manifest.source.validate_frame(&self.frame)?;
        if let Some(r) = &self.region {
            manifest.source.validate_region(r)?;
            require(r.frame == self.frame, "question region frame")?;
        }
        if let Some(r) = &self.range {
            manifest.source.validate_location(r)?;
        }
        Ok(())
    }
}

impl EvidenceTarget {
    pub fn validate(
        &self,
        manifest: &VideoSourceManifest,
        base: &ReadOnlyBase,
        source: &str,
    ) -> Validation {
        manifest.validate()?;
        require(base.book_id == manifest.material.book_id, "evidence base")?;
        match self {
            Self::Text {
                publication,
                transcript,
                text_range,
            } => {
                manifest.validate_publication(publication)?;
                require(
                    transcript.as_ref() == Some(&manifest.material.transcript),
                    "evidence transcript",
                )?;
                text_at(text_range, base, &source.encode_utf16().collect::<Vec<_>>())?;
            }
            Self::Visual {
                publication,
                frames,
                regions,
            } => {
                manifest.validate_publication(publication)?;
                require(
                    !frames.is_empty() || !regions.is_empty(),
                    "empty visual evidence",
                )?;
                for f in frames {
                    manifest.source.validate_frame(f)?;
                }
                for r in regions {
                    manifest.source.validate_region(r)?;
                }
            }
            Self::Audio {
                publication,
                location,
                review_result_ref,
            } => {
                manifest.validate_publication(publication)?;
                manifest.source.validate_location(location)?;
                require(nonempty(review_result_ref), "audio review result")?;
            }
            Self::Joint { members } => {
                require(members.len() >= 2, "joint members")?;
                let mut publication = None;
                for member in members {
                    require(nonempty(&member.support_role), "support role")?;
                    let p = match &member.target {
                        Self::Joint { .. } => {
                            return Err("invalid video contract: nested joint".into())
                        }
                        Self::Text { publication, .. }
                        | Self::Visual { publication, .. }
                        | Self::Audio { publication, .. } => publication,
                    };
                    require(
                        publication.is_none_or(|previous| previous == p),
                        "joint publication",
                    )?;
                    publication = Some(p);
                    member.target.validate(manifest, base, source)?;
                }
            }
        }
        Ok(())
    }
}

impl MediaObservation {
    pub fn validate(&self, manifest: &VideoSourceManifest) -> Validation {
        manifest.validate()?;
        match self {
            Self::Visual {
                observation_id,
                run_id,
                publication,
                model_request_id,
                frames,
                regions,
            } => {
                manifest.validate_publication(publication)?;
                require(
                    nonempty(observation_id)
                        && nonempty(run_id)
                        && nonempty(model_request_id)
                        && (!frames.is_empty() || !regions.is_empty()),
                    "visual receipt",
                )?;
                for f in frames {
                    require(
                        f.requested_ms.is_finite()
                            && f.requested_ms >= 0.0
                            && f.requested_ms < manifest.source.duration_ms,
                        "requested time",
                    )?;
                    manifest.source.validate_frame(&f.frame)?;
                }
                for r in regions {
                    manifest.source.validate_region(r)?;
                }
            }
            Self::Audio {
                observation_id,
                run_id,
                publication,
                tool_call_id,
                location,
                review_result_ref,
            } => {
                manifest.validate_publication(publication)?;
                require(
                    nonempty(observation_id)
                        && nonempty(run_id)
                        && nonempty(tool_call_id)
                        && nonempty(review_result_ref),
                    "audio receipt",
                )?;
                manifest.source.validate_location(location)?;
            }
        }
        Ok(())
    }
}
