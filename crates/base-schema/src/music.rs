//! Curated music uses immutable score/recording sources and revisable interpretation.
//! A performance occurrence disambiguates repeated measures; no transcript is required.
use crate::{PublishedBookRef, VideoQuestionImage};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use ts_rs::TS;

macro_rules! record {
    ($name:ident { $($(#[$field_meta:meta])* $field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
        #[ts(export, export_to = "../../../packages/core/src/generated/")]
        pub struct $name { $($(#[$field_meta])* pub $field: $ty),* }
    };
}
macro_rules! choices {
    ($name:ident { $($variant:ident),* $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
        #[serde(rename_all = "snake_case")]
        #[ts(export, export_to = "../../../packages/core/src/generated/")]
        pub enum $name { $($variant),* }
    };
}

choices!(MusicRecordingKind { Performance, ScoreSynthesis });
choices!(MusicAssetKind { Recording, Score, Reference, Font, Soundfont });
choices!(MusicDistribution { Allowed, LocalOnly });
choices!(MusicTie { None, Start, Continue, Stop });
choices!(MusicGranularity { Phrase, Measure, Note });
choices!(MusicAlignmentMethod { Manual, AudioAlignment, ScoreSynthesis });
choices!(MusicCoverageStatus { Accepted, Pending, Failed });
choices!(MusicProvenanceKind { Score, AudioAnalysis, HumanListening, Reference, Interpretation, ScoreSynthesis });
choices!(MusicRelationKind { Repetition, Variation, Handoff, Response, Support, Contrast });
choices!(MusicStructureCoverage { Detailed, NavigationOnly });
choices!(MusicDemonstrationKind { ScoreSynthesis });

record!(MusicRecordingRef { recording_id: String, revision: String });
record!(MusicScoreRef { score_id: String, revision: String });
// Half-open [start_ms, end_ms) on the original media timeline, never inferred from notation tempo.
record!(MusicAudioLocation { recording: MusicRecordingRef, start_ms: f64, end_ms: f64 });
record!(MusicScoreLocation { score: MusicScoreRef, source_ids: Vec<String> });
// A reviewed work-level tonality is distinct from the mode encoded in a key signature.
record!(MusicTonality { tonic: String, mode: String, explanation: String, provenance_ids: Vec<String> });
record!(MusicWork {
    work_id: String, title: String, composer: String, movement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<MusicTonality>", optional = nullable)]
    tonality: Option<MusicTonality>,
});
record!(MusicScore { source: MusicScoreRef, title: String, musicxml_path: String });
record!(MusicRecording { source: MusicRecordingRef, title: String, performers: String, media_path: String, content_type: String, duration_ms: f64, kind: MusicRecordingKind });
record!(MusicRights { asset_kind: MusicAssetKind, title: String, url: String, license: String, attribution: String, distribution: MusicDistribution });
record!(MusicPart { part_id: String, name: String, instrument: String, group: String, voices: Vec<String> });
record!(MusicScorePage { page_id: String, path: String, width: f64, height: f64 });
// midi_pitch is the sounding pitch after MusicXML transposition. Written notation stays in the SVG.
// source_id survives rendering changes; page_id/svg_element_id belong to this layout only.
record!(MusicScoreObject {
    source_id: String, part_id: String, measure: String, beat: f64, duration_beats: f64,
    voice: String, midi_pitch: Option<u8>, rest: bool, tie: MusicTie,
    page_id: String, svg_element_id: String,
});
record!(MusicMeterComponent { beats: String, beat_type: u32 });
record!(MusicMetronome { beat_unit: String, dots: u32, per_minute: String });
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export, export_to = "../../../packages/core/src/generated/")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MusicScoreMarkValue {
    KeySignature { fifths: i32, encoded_mode: Option<String> },
    TimeSignature { components: Vec<MusicMeterComponent>, symbol: Option<String> },
    Direction { words: Vec<String>, metronome: Option<MusicMetronome>, sound_tempo_quarter_bpm: Option<f64> },
}
impl MusicScoreMarkValue {
    pub fn is_tempo(&self) -> bool {
        matches!(self, Self::Direction { metronome, sound_tempo_quarter_bpm, .. }
            if metronome.is_some() || sound_tempo_quarter_bpm.is_some())
    }
}
// Existing note/rest anchors locate marks in the original score and source popup.
// measure_index is score order, independent of printed labels and performance repeats.
record!(MusicScoreMark {
    mark_id: String, part_id: String, measure: String, measure_index: u32, beat: f64,
    staff: Option<String>, value: MusicScoreMarkValue, score_object_ids: Vec<String>, provenance_id: String,
});
// Each occurrence includes the complete measure, across all parts. A repeat creates a new occurrence.
record!(MusicOccurrence { occurrence_id: String, order: u32, measure: String, start_ms: f64, end_ms: f64, score_object_ids: Vec<String> });
record!(MusicNoteAlignment {
    occurrence_id: String, score_object_id: String, start_ms: f64, end_ms: f64, attack: bool, provenance_id: String,
    // A reviewed score/recording discrepancy still has a musical position, but
    // must not produce a sounding marker when no independent tone was observed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(as = "Option<bool>", optional)]
    unobserved: bool,
});
record!(MusicCoverage { start_ms: f64, end_ms: f64, granularity: MusicGranularity, method: MusicAlignmentMethod, status: MusicCoverageStatus, provenance_id: String, detail: String });
record!(MusicProvenance { provenance_id: String, kind: MusicProvenanceKind, description: String, source_url: Option<String>, processor: Option<String> });
record!(MusicEvidence { provenance_id: String, score_object_ids: Vec<String>, audio: Option<MusicAudioLocation> });
record!(MusicVoiceFocus { part_id: String, voice: String });
// Empty/omitted voice_ids applies the role to the whole part; selected voices retain its instrument identity.
record!(MusicEventRole {
    part_id: String, role: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<String>>", optional)]
    voice_ids: Vec<String>,
    // A curator-assigned theme identity can follow its carriers across instruments and events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<String>", optional = nullable)]
    theme_id: Option<String>,
});
record!(MusicEventRelation { target_event_id: String, kind: MusicRelationKind, description: String });
record!(MusicEvent {
    event_id: String, title: String, explanation: String, listen_for: String, start_ms: f64, end_ms: f64,
    roles: Vec<MusicEventRole>, evidence: Vec<MusicEvidence>, relations: Vec<MusicEventRelation>, concept_tags: Vec<String>,
    // Optional revisable playback anchors. Audio evidence retains its original immutable range.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<String>>", optional)]
    occurrence_ids: Vec<String>,
});
record!(MusicStructure { section_id: String, parent_id: Option<String>, title: String, description: String, start_ms: Option<f64>, end_ms: Option<f64>, event_ids: Vec<String>, coverage: MusicStructureCoverage });
record!(MusicDemonstration { demonstration_id: String, title: String, kind: MusicDemonstrationKind, media_path: String, content_type: String, duration_ms: f64, part_ids: Vec<String>, score_object_ids: Vec<String>, provenance_id: String });
record!(MusicSourceManifest {
    version: String, book_id: String, title: String, work: MusicWork,
    score: MusicScore, recording: MusicRecording, rights: Vec<MusicRights>, parts: Vec<MusicPart>,
    pages: Vec<MusicScorePage>, score_objects: Vec<MusicScoreObject>, occurrences: Vec<MusicOccurrence>,
    note_alignments: Vec<MusicNoteAlignment>, coverage: Vec<MusicCoverage>, provenance: Vec<MusicProvenance>,
    events: Vec<MusicEvent>, structure: Vec<MusicStructure>, demonstrations: Vec<MusicDemonstration>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<MusicScoreMark>>", optional)]
    score_marks: Vec<MusicScoreMark>,
});
record!(MusicQuestionContext {
    draft_id: String, publication: PublishedBookRef, audio: MusicAudioLocation,
    score: Option<MusicScoreLocation>, occurrence_id: Option<String>, focused_part_ids: Vec<String>,
    // Whole-part focus and explicit voice focus are independent selections, combined as a union.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<MusicVoiceFocus>>", optional)]
    focused_voices: Vec<MusicVoiceFocus>,
    // User-attached pixels may show any notation, including unindexed staff lines and symbols.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<VideoQuestionImage>", optional = nullable)]
    image: Option<VideoQuestionImage>,
});
// Exact source anchors from a frozen publication; these are not proof of listening to the audio.
record!(MusicSourceTarget {
    publication: PublishedBookRef, audio: MusicAudioLocation, score: Option<MusicScoreLocation>,
    occurrence_ids: Vec<String>, event_ids: Vec<String>, provenance_ids: Vec<String>,
});

fn require(ok: bool, field: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(format!("invalid music contract: {field}")) }
}
fn text(s: &str) -> bool { !s.trim().is_empty() }
fn positive(n: f64) -> bool { n.is_finite() && n > 0.0 }
fn path(s: &str) -> bool {
    text(s) && !s.starts_with(['/', '\\']) && !s.contains(':')
        && !s.split(['/', '\\']).any(|p| p == "..")
}
fn interval(start: f64, end: f64, duration: f64) -> bool {
    start.is_finite() && end.is_finite() && start >= 0.0 && start < end && end <= duration
}
fn unique<'a>(ids: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    ids.into_iter().all(|id| text(id) && seen.insert(id))
}

impl MusicAudioLocation {
    pub fn validate(&self, manifest: &MusicSourceManifest) -> Result<(), String> {
        require(self.recording == manifest.recording.source && interval(self.start_ms, self.end_ms, manifest.recording.duration_ms), "audio location")
    }
}

impl MusicSourceManifest {
    /// Existing publication citations retain their original identities. Rendering,
    /// annotations and alignment can change; unchanged source bytes are checked by storage.
    pub fn validate_successor(&self, previous: &Self) -> Result<(), String> {
        self.validate()?;
        previous.validate()?;
        if self.book_id != previous.book_id { return Ok(()); }
        require(self.work.work_id == previous.work.work_id && self.recording.source == previous.recording.source
            && self.recording.duration_ms == previous.recording.duration_ms && self.recording.kind == previous.recording.kind
            && self.score.source == previous.score.source, "immutable material source")?;
        let notes: BTreeMap<_, _> = self.score_objects.iter().map(|n| (n.source_id.as_str(), n)).collect();
        for old in &previous.score_objects {
            require(notes.get(old.source_id.as_str()).is_some_and(|n| n.part_id == old.part_id && n.measure == old.measure
                && n.beat == old.beat && n.duration_beats == old.duration_beats && n.voice == old.voice
                && n.midi_pitch == old.midi_pitch && n.rest == old.rest && n.tie == old.tie), "immutable score location")?;
        }
        Ok(())
    }

    pub fn capabilities(&self) -> Vec<String> {
        let mut result = vec!["music".to_string()];
        if !self.pages.is_empty() && !self.score_objects.is_empty() { result.push("music_score".into()); }
        if !self.events.is_empty() { result.push("music_events".into()); }
        if !self.note_alignments.is_empty() && self.coverage.iter().any(|c| c.status == MusicCoverageStatus::Accepted && c.granularity == MusicGranularity::Note) {
            result.push("music_note_alignment".into());
        }
        result
    }

    pub fn validate(&self) -> Result<(), String> {
        require(self.version == "music_source_manifest.v1" && text(&self.book_id) && text(&self.title), "manifest identity")?;
        require(text(&self.work.work_id) && text(&self.work.title) && text(&self.work.composer)
            && self.work.movement.as_ref().is_none_or(|v| text(v)), "work")?;
        require(text(&self.score.source.score_id) && text(&self.score.source.revision) && text(&self.score.title) && path(&self.score.musicxml_path), "score source")?;
        let r = &self.recording;
        require(text(&r.source.recording_id) && text(&r.source.revision) && text(&r.title) && text(&r.performers)
            && path(&r.media_path) && text(&r.content_type) && positive(r.duration_ms), "recording source")?;
        let duration = r.duration_ms;
        for right in &self.rights {
            require(text(&right.title) && text(&right.url) && text(&right.license) && text(&right.attribution), "rights")?;
        }
        require(unique(self.parts.iter().map(|p| p.part_id.as_str())), "part identity")?;
        require(unique(self.pages.iter().map(|p| p.page_id.as_str())), "page identity")?;
        require(unique(self.score_objects.iter().map(|p| p.source_id.as_str())), "score object identity")?;
        require(unique(self.occurrences.iter().map(|p| p.occurrence_id.as_str())), "occurrence identity")?;
        require(unique(self.provenance.iter().map(|p| p.provenance_id.as_str())), "provenance identity")?;
        require(unique(self.events.iter().map(|p| p.event_id.as_str())), "event identity")?;
        require(unique(self.structure.iter().map(|p| p.section_id.as_str())), "section identity")?;
        require(unique(self.demonstrations.iter().map(|p| p.demonstration_id.as_str())), "demonstration identity")?;
        let parts: BTreeMap<_, _> = self.parts.iter().map(|p| (p.part_id.as_str(), p)).collect();
        let pages: BTreeSet<_> = self.pages.iter().map(|p| p.page_id.as_str()).collect();
        let notes: BTreeMap<_, _> = self.score_objects.iter().map(|p| (p.source_id.as_str(), p)).collect();
        let occurrences: BTreeMap<_, _> = self.occurrences.iter().map(|p| (p.occurrence_id.as_str(), p)).collect();
        let provenance: BTreeMap<_, _> = self.provenance.iter().map(|p| (p.provenance_id.as_str(), p)).collect();
        let events: BTreeSet<_> = self.events.iter().map(|p| p.event_id.as_str()).collect();
        let sections: BTreeMap<_, _> = self.structure.iter().map(|p| (p.section_id.as_str(), p)).collect();
        let mut measure_objects: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for n in &self.score_objects { measure_objects.entry(&n.measure).or_default().insert(&n.source_id); }
        for p in &self.parts {
            require(text(&p.name) && text(&p.instrument) && text(&p.group) && unique(p.voices.iter().map(String::as_str)) && !p.voices.is_empty(), "part")?;
        }
        for p in &self.pages { require(path(&p.path) && positive(p.width) && positive(p.height), "page")?; }
        for p in &self.provenance {
            require(text(&p.description) && p.source_url.as_ref().is_none_or(|v| text(v)) && p.processor.as_ref().is_none_or(|v| text(v)), "provenance")?;
        }
        if let Some(tonality) = &self.work.tonality {
            require(text(&tonality.tonic) && text(&tonality.mode) && text(&tonality.explanation)
                && !tonality.provenance_ids.is_empty() && unique(tonality.provenance_ids.iter().map(String::as_str))
                && tonality.provenance_ids.iter().all(|id| provenance.get(id.as_str()).is_some_and(|p|
                    matches!(p.kind, MusicProvenanceKind::Score | MusicProvenanceKind::Reference))), "work tonality")?;
        }
        require(unique(self.score_marks.iter().map(|mark| mark.mark_id.as_str())), "score mark identity")?;
        for mark in &self.score_marks {
            require(parts.contains_key(mark.part_id.as_str()) && text(&mark.measure) && mark.measure_index > 0 && mark.beat.is_finite()
                && mark.staff.as_ref().is_none_or(|staff| text(staff))
                && !mark.score_object_ids.is_empty() && unique(mark.score_object_ids.iter().map(String::as_str))
                && mark.score_object_ids.iter().all(|id| notes.get(id.as_str()).is_some_and(|n| n.part_id == mark.part_id && n.measure == mark.measure))
                && provenance.get(mark.provenance_id.as_str()).is_some_and(|p| p.kind == MusicProvenanceKind::Score), "score mark source")?;
            require(match &mark.value {
                MusicScoreMarkValue::KeySignature { encoded_mode, .. } => encoded_mode.as_ref().is_none_or(|v| text(v)),
                MusicScoreMarkValue::TimeSignature { components, symbol } => !components.is_empty()
                    && components.iter().all(|c| text(&c.beats) && c.beat_type > 0) && symbol.as_ref().is_none_or(|s| text(s)),
                MusicScoreMarkValue::Direction { words, metronome, sound_tempo_quarter_bpm } =>
                    (!words.is_empty() || metronome.is_some() || sound_tempo_quarter_bpm.is_some())
                    && words.iter().all(|w| text(w))
                    && metronome.as_ref().is_none_or(|m| text(&m.beat_unit) && text(&m.per_minute))
                    && sound_tempo_quarter_bpm.is_none_or(positive),
            }, "score mark value")?;
        }
        let mut svg_ids = BTreeSet::new();
        for n in &self.score_objects {
            require(parts.get(n.part_id.as_str()).is_some_and(|p| p.voices.contains(&n.voice)) && pages.contains(n.page_id.as_str())
                && text(&n.measure) && n.beat.is_finite() && n.beat >= 0.0 && n.duration_beats.is_finite() && n.duration_beats >= 0.0
                && (if n.rest { n.midi_pitch.is_none() && n.tie == MusicTie::None } else { n.midi_pitch.is_some_and(|p| p <= 127) })
                && text(&n.svg_element_id) && svg_ids.insert((&n.page_id, &n.svg_element_id)), "score object")?;
        }
        for (i, o) in self.occurrences.iter().enumerate() {
            require(interval(o.start_ms, o.end_ms, duration) && text(&o.measure) && !o.score_object_ids.is_empty()
                && unique(o.score_object_ids.iter().map(String::as_str))
                && o.score_object_ids.iter().all(|id| notes.get(id.as_str()).is_some_and(|n| n.measure == o.measure))
                && measure_objects.get(o.measure.as_str()).is_some_and(|ids| ids.len() == o.score_object_ids.len())
                && (i == 0 || self.occurrences[i-1].order < o.order && self.occurrences[i-1].end_ms <= o.start_ms), "occurrence")?;
        }
        let mut aligned = BTreeSet::new();
        for a in &self.note_alignments {
            let o = occurrences.get(a.occurrence_id.as_str());
            let n = notes.get(a.score_object_id.as_str());
            require(interval(a.start_ms, a.end_ms, duration) && provenance.get(a.provenance_id.as_str()).is_some_and(|p|
                matches!(p.kind, MusicProvenanceKind::HumanListening | MusicProvenanceKind::AudioAnalysis)
                || p.kind == MusicProvenanceKind::ScoreSynthesis && r.kind == MusicRecordingKind::ScoreSynthesis)
                && o.is_some_and(|o| o.score_object_ids.contains(&a.score_object_id) && a.start_ms >= o.start_ms && a.start_ms < o.end_ms)
                && n.is_some_and(|n| !n.rest && (!a.attack || !a.unobserved && !matches!(n.tie, MusicTie::Continue | MusicTie::Stop)))
                && aligned.insert((a.occurrence_id.as_str(), a.score_object_id.as_str())), "note alignment")?;
        }
        for granularity in [MusicGranularity::Phrase, MusicGranularity::Measure, MusicGranularity::Note] {
            let mut intervals: Vec<_> = self.coverage.iter().filter(|c| c.granularity == granularity).collect();
            intervals.sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms));
            require(intervals.windows(2).all(|p| p[0].end_ms <= p[1].start_ms), "coverage overlap")?;
        }
        for c in &self.coverage {
            require(interval(c.start_ms, c.end_ms, duration) && text(&c.detail) && provenance.contains_key(c.provenance_id.as_str()), "coverage")?;
            if c.status == MusicCoverageStatus::Accepted {
                let p = provenance[c.provenance_id.as_str()];
                require(match c.method {
                    MusicAlignmentMethod::Manual => matches!(p.kind, MusicProvenanceKind::HumanListening | MusicProvenanceKind::AudioAnalysis),
                    MusicAlignmentMethod::AudioAlignment => p.kind == MusicProvenanceKind::AudioAnalysis,
                    MusicAlignmentMethod::ScoreSynthesis => r.kind == MusicRecordingKind::ScoreSynthesis && p.kind == MusicProvenanceKind::ScoreSynthesis,
                }, "coverage method")?;
                let selected: Vec<_> = self.occurrences.iter().filter(|o| o.start_ms < c.end_ms && o.end_ms > c.start_ms).collect();
                require(!selected.is_empty(), "coverage without occurrences")?;
                let mut cursor = c.start_ms;
                for o in &selected {
                    require(o.start_ms <= cursor, "coverage occurrence gap")?;
                    cursor = cursor.max(o.end_ms);
                }
                require(cursor >= c.end_ms, "coverage occurrence gap")?;
                if c.granularity == MusicGranularity::Note {
                    require(selected.iter().all(|o| o.score_object_ids.iter().all(|id| notes[id.as_str()].rest || aligned.contains(&(o.occurrence_id.as_str(), id.as_str())))), "note coverage gap")?;
                }
            }
        }
        for e in &self.events {
            require(text(&e.title) && text(&e.explanation) && text(&e.listen_for) && interval(e.start_ms, e.end_ms, duration)
                && !e.evidence.is_empty() && e.concept_tags.iter().all(|v| text(v)), "event")?;
            require(unique(e.occurrence_ids.iter().map(String::as_str))
                && e.occurrence_ids.iter().all(|id| occurrences.contains_key(id.as_str())), "event occurrence")?;
            for role in &e.roles {
                require(parts.get(role.part_id.as_str()).is_some_and(|p| role.voice_ids.iter().all(|v| p.voices.contains(v)))
                    && text(&role.role) && unique(role.voice_ids.iter().map(String::as_str))
                    && role.theme_id.as_ref().is_none_or(|id| text(id)), "event role")?;
            }
            for evidence in &e.evidence {
                require(provenance.contains_key(evidence.provenance_id.as_str()) && evidence.score_object_ids.iter().all(|id| notes.contains_key(id.as_str())), "event evidence")?;
                if let Some(audio) = &evidence.audio { audio.validate(self)?; }
                require(!evidence.score_object_ids.is_empty() || evidence.audio.is_some() || matches!(provenance[evidence.provenance_id.as_str()].kind, MusicProvenanceKind::Reference | MusicProvenanceKind::Interpretation), "event evidence location")?;
            }
            for relation in &e.relations { require(relation.target_event_id != e.event_id && events.contains(relation.target_event_id.as_str()) && text(&relation.description), "event relation")?; }
        }
        for s in &self.structure {
            require(text(&s.title) && text(&s.description) && s.event_ids.iter().all(|id| events.contains(id.as_str())), "structure")?;
            require(match (s.start_ms, s.end_ms) {
                (Some(start), Some(end)) => interval(start, end, duration),
                (None, None) => s.coverage == MusicStructureCoverage::NavigationOnly,
                _ => false,
            }, "structure location")?;
            let mut visited = BTreeSet::from([s.section_id.as_str()]);
            let mut parent = &s.parent_id;
            while let Some(id) = parent {
                require(visited.insert(id.as_str()) && sections.contains_key(id.as_str()), "structure parent")?;
                parent = &sections[id.as_str()].parent_id;
            }
        }
        for d in &self.demonstrations {
            require(text(&d.title) && path(&d.media_path) && text(&d.content_type) && positive(d.duration_ms)
                && !d.part_ids.is_empty() && d.part_ids.iter().all(|id| parts.contains_key(id.as_str()))
                && !d.score_object_ids.is_empty() && d.score_object_ids.iter().all(|id| notes.contains_key(id.as_str()))
                && provenance.get(d.provenance_id.as_str()).is_some_and(|p| p.kind == MusicProvenanceKind::ScoreSynthesis), "demonstration")?;
        }
        Ok(())
    }
}

impl MusicQuestionContext {
    pub fn validate(&self, manifest: &MusicSourceManifest, publication: &PublishedBookRef) -> Result<(), String> {
        if let Some(image) = &self.image { image.validate()?; }
        require(text(&self.draft_id) && self.publication == *publication && text(&publication.publication_id) && publication.book_id == manifest.book_id, "question publication")?;
        self.audio.validate(manifest)?;
        if let Some(score) = &self.score {
            require(score.score == manifest.score.source && !score.source_ids.is_empty() && score.source_ids.iter().all(|id| manifest.score_objects.iter().any(|o| o.source_id == *id)), "question score")?;
        }
        require(self.occurrence_id.as_ref().is_none_or(|id| manifest.occurrences.iter().any(|o| o.occurrence_id == *id && o.start_ms < self.audio.end_ms && o.end_ms > self.audio.start_ms))
            && self.focused_part_ids.iter().all(|id| manifest.parts.iter().any(|p| p.part_id == *id)), "question focus")?;
        let mut voices = BTreeSet::new();
        for focus in &self.focused_voices {
            focus.validate(manifest)?;
            require(voices.insert((&focus.part_id, &focus.voice)), "question voice focus")?;
        }
        Ok(())
    }
}

impl MusicVoiceFocus {
    pub fn validate(&self, manifest: &MusicSourceManifest) -> Result<(), String> {
        require(manifest.parts.iter().any(|p| p.part_id == self.part_id && p.voices.contains(&self.voice)), "voice focus")
    }
}
