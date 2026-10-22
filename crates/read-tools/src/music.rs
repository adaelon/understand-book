//! Prepared music materials are read evidence, never a live audio observation.
use base_schema::{MusicAudioLocation, MusicRecordingRef, MusicScoreLocation, MusicSourceManifest, PublishedBookRef};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::{Book, ToolError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicRead {
    #[serde(default = "default_view")]
    pub view: String,
    #[serde(default)] pub query: Option<String>,
    #[serde(default)] pub location: Option<MusicAudioLocation>,
    #[serde(default)] pub event_id: Option<String>,
    #[serde(default)] pub source_ids: Vec<String>,
    #[serde(default)] pub part_ids: Vec<String>,
    #[serde(default)] pub voices: Vec<base_schema::MusicVoiceFocus>,
    #[serde(default)] pub offset: usize,
    #[serde(default = "default_limit")] pub limit: usize,
}
fn default_view() -> String { "overview".into() }
fn default_limit() -> usize { 20 }

pub use base_schema::MusicSourceTarget;

/// Persist the material actually read, so later publication changes do not rewrite a citation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicSourceSnapshot {
    pub target: MusicSourceTarget,
    pub title: String,
    pub label: String,
    pub preview: String,
    pub material: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicReadResult {
    pub publication: PublishedBookRef,
    pub recording: MusicRecordingRef,
    pub observation_kind: String,
    pub heard_audio: bool,
    pub body: Value,
    pub sources: Vec<MusicSourceSnapshot>,
}

pub fn error(message: impl Into<String>) -> ToolError {
    ToolError { error_code: "INVALID_MUSIC_SOURCE".into(), category: "invalid_input".into(), message: message.into() }
}

impl Book {
    pub fn load_music_source(&self, dir: &std::path::Path) -> Result<MusicSourceManifest, ToolError> {
        let bytes = std::fs::read(dir.join("music_source_manifest.json"))
            .map_err(|e| error(format!("Cannot read music source manifest: {e}")))?;
        let manifest: MusicSourceManifest = serde_json::from_slice(&bytes)
            .map_err(|e| error(format!("Invalid music source manifest: {e}")))?;
        manifest.validate().map_err(error)?;
        if manifest.book_id != self.base.book_id { return Err(error("Music manifest belongs to another book")); }
        Ok(manifest)
    }
}

/// Reads only the explicitly loaded publication; reference identity is supplied by its host.
pub fn read(manifest: &MusicSourceManifest, publication: &PublishedBookRef, request: &MusicRead) -> Result<MusicReadResult, ToolError> {
    if manifest.book_id != publication.book_id { return Err(error("Publication and music book do not match")); }
    if !matches!(request.view.as_str(), "overview" | "events" | "score") || !(1..=200).contains(&request.limit) {
        return Err(error("Use view overview, events or score and a limit between 1 and 200"));
    }
    let mut audio = request.location.clone().unwrap_or_else(|| MusicAudioLocation {
        recording: manifest.recording.source.clone(), start_ms: 0.0, end_ms: manifest.recording.duration_ms,
    });
    if audio.recording != manifest.recording.source || !audio.start_ms.is_finite() || !audio.end_ms.is_finite()
        || audio.start_ms < 0.0 || audio.end_ms <= audio.start_ms || audio.end_ms > manifest.recording.duration_ms {
        return Err(error("Music range must use this recording revision and its original timeline"));
    }
    if request.part_ids.iter().any(|id| !manifest.parts.iter().any(|part| &part.part_id == id)) {
        return Err(error("Unknown music part; copy a part_id from the overview"));
    }
    for voice in &request.voices { voice.validate(manifest).map_err(error)?; }
    if request.source_ids.iter().any(|id| !manifest.score_objects.iter().any(|object| &object.source_id == id)) {
        return Err(error("Unknown score source_id; score IDs must come from this score revision"));
    }
    if request.event_id.as_ref().is_some_and(|id| !manifest.events.iter().any(|event| &event.event_id == id)) {
        return Err(error("Unknown music event; copy an event_id from an events read"));
    }
    let query = request.query.as_deref().unwrap_or("").trim().to_lowercase();
    let occurrences: Vec<_> = manifest.occurrences.iter().filter(|item| item.start_ms < audio.end_ms && item.end_ms > audio.start_ms).collect();
    let mut selected_ids = Vec::new();
    let mut event_ids = Vec::new();
    let mut provenance_ids = Vec::new();
    let body = match request.view.as_str() {
        "overview" => json!({"work":manifest.work,"recording":manifest.recording,"score":manifest.score,
            "parts":manifest.parts,"structure":manifest.structure,"coverage":manifest.coverage,
            "rights":manifest.rights,"provenance":manifest.provenance,"demonstrations":manifest.demonstrations,
            "guidance":"Read events or score for a bounded range. Navigation-only sections do not imply detailed analysis. Score synthesis is not a separated recording stem."}),
        "events" => {
            let matching: Vec<_> = manifest.events.iter().filter(|event| event.start_ms < audio.end_ms && event.end_ms > audio.start_ms
                && request.event_id.as_ref().is_none_or(|id| &event.event_id == id)
                && (request.part_ids.is_empty() && request.voices.is_empty() || event.roles.iter().any(|role| request.part_ids.contains(&role.part_id)
                    || request.voices.iter().any(|voice|voice.part_id == role.part_id && (role.voice_ids.is_empty() || role.voice_ids.contains(&voice.voice)))))
                && (query.is_empty() || format!("{} {} {} {} {}",event.event_id,event.title,event.explanation,event.listen_for,event.concept_tags.join(" ")).to_lowercase().contains(&query))).collect();
            let items: Vec<_> = matching.iter().skip(request.offset).take(request.limit).copied().collect();
            for event in &items {
                event_ids.push(event.event_id.clone());
                for evidence in &event.evidence {
                    provenance_ids.push(evidence.provenance_id.clone());
                    selected_ids.extend(evidence.score_object_ids.clone());
                }
            }
            json!({"events":items,"total":matching.len(),"next_offset":next_offset(request, matching.len()),
                "guidance":"Relations point to independently located events. Read both passages before comparing their musical details. A metaphor is an interpretation, not a score fact."})
        },
        "score" => {
            let matching: Vec<_> = manifest.score_objects.iter().filter(|object|
                (request.part_ids.is_empty() && request.voices.is_empty() || request.part_ids.contains(&object.part_id)
                    || request.voices.iter().any(|voice|voice.part_id == object.part_id && voice.voice == object.voice))
                && (request.source_ids.is_empty() || request.source_ids.contains(&object.source_id))
                && (request.location.is_none() || occurrences.iter().any(|occurrence| occurrence.score_object_ids.contains(&object.source_id)))
                && (query.is_empty() || format!("{} {} {} {}",object.source_id,object.measure,object.voice,object.part_id).to_lowercase().contains(&query))).collect();
            let items: Vec<_> = matching.iter().skip(request.offset).take(request.limit).copied().collect();
            selected_ids.extend(items.iter().map(|object| object.source_id.clone()));
            let alignments: Vec<_> = manifest.note_alignments.iter().filter(|note| selected_ids.contains(&note.score_object_id)
                && occurrences.iter().any(|occurrence| occurrence.occurrence_id == note.occurrence_id)
                && note.start_ms < audio.end_ms && note.end_ms > audio.start_ms).collect();
            provenance_ids.extend(alignments.iter().map(|note| note.provenance_id.clone()));
            provenance_ids.extend(manifest.provenance.iter().filter(|p| p.kind == base_schema::MusicProvenanceKind::Score).map(|p| p.provenance_id.clone()));
            json!({"objects":items,"alignments":alignments,"total":matching.len(),"next_offset":next_offset(request, matching.len()),
                "guidance":"Repeated score objects can have several performance occurrences. attack=false denotes sustained/tied notes; note end is not a measured acoustic release unless the provenance establishes it."})
        },
        _ => unreachable!(),
    };
    selected_ids.sort(); selected_ids.dedup(); provenance_ids.sort(); provenance_ids.dedup();
    let selected_events: Vec<_> = manifest.events.iter().filter(|event| event_ids.contains(&event.event_id)).collect();
    if !selected_events.is_empty() {
        // An event citation opens the passage actually returned, even when the
        // caller searched the whole recording. Repeated notation is not a
        // second performance occurrence of this particular event.
        audio.start_ms = selected_events.iter().map(|event| event.start_ms).fold(f64::INFINITY, f64::min);
        audio.end_ms = selected_events.iter().map(|event| event.end_ms).fold(0.0, f64::max);
    }
    let occurrences: Vec<_> = if request.view == "events" {
        manifest.occurrences.iter().filter(|occurrence| selected_events.iter().any(|event|
            event.start_ms < occurrence.end_ms && event.end_ms > occurrence.start_ms
            && (event.occurrence_ids.is_empty() || event.occurrence_ids.contains(&occurrence.occurrence_id)))).collect()
    } else {
        occurrences.into_iter().filter(|occurrence| request.view == "score"
            && occurrence.score_object_ids.iter().any(|id| selected_ids.contains(id))).collect()
    };
    let provenance: Vec<_> = manifest.provenance.iter().filter(|p| provenance_ids.contains(&p.provenance_id)).collect();
    let coverage: Vec<_> = manifest.coverage.iter().filter(|c| c.start_ms < audio.end_ms && c.end_ms > audio.start_ms).collect();
    let seconds = (audio.start_ms / 1000.0).floor() as u64;
    let label = format!("{} · {:02}:{:02} · {}", manifest.title, seconds / 60, seconds % 60, if request.view == "score" { "谱面" } else { "预构建分析" });
    let preview = if event_ids.is_empty() { format!("{} · {}",manifest.work.composer,manifest.work.title) } else {
        manifest.events.iter().filter(|event| event_ids.contains(&event.event_id)).map(|event| format!("{}：{}",event.title,event.explanation)).collect::<Vec<_>>().join("\n")
    };
    let summary = json!({"view":request.view,"total":body.get("total"),"next_offset":body.get("next_offset")});
    let occurrence_locations:Vec<_>=occurrences.iter().map(|occurrence| json!({"occurrence_id":occurrence.occurrence_id,"order":occurrence.order,
        "measure":occurrence.measure,"start_ms":occurrence.start_ms,"end_ms":occurrence.end_ms})).collect();
    let material = json!({"view":request.view,"read":body,"provenance":provenance,"coverage":coverage,
        "occurrences":occurrence_locations,"observation_kind":"prepared_music_material","heard_audio":false});
    let target = MusicSourceTarget { publication: publication.clone(), audio,
        score: (!selected_ids.is_empty()).then(|| MusicScoreLocation { score: manifest.score.source.clone(), source_ids: selected_ids }),
        occurrence_ids: occurrences.iter().map(|o| o.occurrence_id.clone()).collect(), event_ids, provenance_ids };
    let sources = if request.view != "overview" && material["read"]["total"] == 0 { vec![] } else {
        vec![MusicSourceSnapshot { target, title: manifest.title.clone(), label, preview, material: material.clone() }]
    };
    Ok(MusicReadResult { publication: publication.clone(), recording: manifest.recording.source.clone(),
        observation_kind: "prepared_music_material".into(), heard_audio: false, body: summary, sources })
}

fn next_offset(request: &MusicRead, total: usize) -> Option<usize> {
    let next = request.offset.saturating_add(request.limit);
    (next < total).then_some(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> MusicSourceManifest {
        serde_json::from_value(json!({
            "version":"music_source_manifest.v1","book_id":"music-book","title":"Repeated theme",
            "work":{"work_id":"work","title":"Theme","composer":"Composer","movement":null},
            "score":{"source":{"score_id":"score","revision":"1"},"title":"Score","musicxml_path":"music/score.musicxml"},
            "recording":{"source":{"recording_id":"recording","revision":"1"},"title":"Recording","performers":"Pianist","media_path":"music/audio.wav","content_type":"audio/wav","duration_ms":2000,"kind":"performance"},
            "rights":[],"parts":[{"part_id":"piano","name":"Piano","instrument":"piano","group":"keyboard","voices":["1"]}],"pages":[],
            "score_objects":[{"source_id":"note1","part_id":"piano","measure":"1","beat":0,"duration_beats":1,"voice":"1","midi_pitch":60,"rest":false,"tie":"none","page_id":"page1","svg_element_id":"rendered1"}],
            "occurrences":[{"occurrence_id":"first","order":0,"measure":"1","start_ms":0,"end_ms":1000,"score_object_ids":["note1"]},{"occurrence_id":"repeat","order":1,"measure":"1","start_ms":1000,"end_ms":2000,"score_object_ids":["note1"]}],
            "note_alignments":[{"occurrence_id":"first","score_object_id":"note1","start_ms":100,"end_ms":800,"attack":true,"provenance_id":"alignment"},{"occurrence_id":"repeat","score_object_id":"note1","start_ms":1100,"end_ms":1800,"attack":true,"provenance_id":"alignment"}],
            "coverage":[],"provenance":[{"provenance_id":"notation","kind":"score","description":"Imported notation","source_url":null,"processor":null},{"provenance_id":"alignment","kind":"audio_analysis","description":"Recording alignment","source_url":null,"processor":"aligner"}],
            "events":[{"event_id":"return","title":"Theme returns","explanation":"The same written note occurs again","listen_for":"Compare the two entrances","start_ms":1000,"end_ms":2000,"roles":[{"part_id":"piano","role":"theme"}],"evidence":[{"provenance_id":"notation","score_object_ids":["note1"],"audio":null}],"relations":[],"concept_tags":["repetition"]}],
            "structure":[],"demonstrations":[]
        })).unwrap()
    }
    fn publication() -> PublishedBookRef { PublishedBookRef { book_id:"music-book".into(),publication_id:"publication-v1".into() } }

    #[test]
    fn repeat_range_keeps_actual_occurrence_and_never_promotes_analysis_to_audio() {
        let manifest = fixture();
        let request: MusicRead = serde_json::from_value(json!({"view":"score","location":{"recording":manifest.recording.source,"start_ms":1000,"end_ms":2000}})).unwrap();
        let result = read(&manifest, &publication(), &request).unwrap();
        assert!(!result.heard_audio);
        let source = &result.sources[0];
        assert_eq!(source.target.occurrence_ids, vec!["repeat"]);
        assert_eq!(source.target.score.as_ref().unwrap().source_ids, vec!["note1"]);
        assert_eq!(source.material["read"]["alignments"].as_array().unwrap().len(),1);
        assert_eq!(source.material["read"]["alignments"][0]["start_ms"].as_f64(),Some(1100.0));
        assert_eq!(source.target.publication,publication());
        assert!(source.target.provenance_ids.contains(&"notation".into()));
    }

    #[test]
    fn recording_version_and_source_ids_cannot_silently_resolve_to_new_material() {
        let manifest = fixture();
        let old: MusicRead = serde_json::from_value(json!({"view":"score","location":{"recording":{"recording_id":"recording","revision":"old"},"start_ms":0,"end_ms":1000}})).unwrap();
        assert!(read(&manifest,&publication(),&old).is_err());
        let unknown: MusicRead = serde_json::from_value(json!({"view":"score","source_ids":["rendered1"]})).unwrap();
        assert!(read(&manifest,&publication(),&unknown).is_err(),"An SVG ID cannot impersonate a stable score identity");
        let mut other = publication(); other.book_id = "other".into();
        assert!(read(&manifest,&other,&serde_json::from_value(json!({})).unwrap()).is_err());
    }

    #[test]
    fn event_search_retains_provenance_and_old_snapshot_after_edit() {
        let mut manifest = fixture();
        let request = serde_json::from_value(json!({"view":"events","query":"repetition","part_ids":["piano"]})).unwrap();
        let result = read(&manifest,&publication(),&request).unwrap();
        manifest.events[0].explanation = "Revised interpretation".into();
        assert_eq!(result.sources[0].target.event_ids,vec!["return"]);
        assert_eq!(result.sources[0].target.occurrence_ids,vec!["repeat"],"The first performance uses the same notation but is not the return event");
        assert_eq!(result.sources[0].target.audio.start_ms,1000.0,"Opening a searched event must start at its actual passage");
        assert_eq!(result.sources[0].material["read"]["events"][0]["explanation"],"The same written note occurs again");
        assert_eq!(result.sources[0].material["provenance"][0]["kind"],"score");
        let missing:MusicRead=serde_json::from_value(json!({"view":"events","query":"absent"})).unwrap();
        assert!(read(&manifest,&publication(),&missing).unwrap().sources.is_empty());
        manifest.events[0].start_ms=0.0;
        manifest.events[0].occurrence_ids=vec!["repeat".into()];
        assert_eq!(read(&manifest,&publication(),&request).unwrap().sources[0].target.occurrence_ids,vec!["repeat"],
            "An explicit performance binding stays authoritative inside a broader teaching window");
    }
    #[test]
    fn score_and_voice_read_do_not_require_finished_audio_alignment() {
        let mut manifest=fixture();manifest.occurrences.clear();manifest.note_alignments.clear();
        manifest.parts[0].voices.push("2".into());
        let mut bass=manifest.score_objects[0].clone();bass.source_id="bass1".into();bass.voice="2".into();bass.midi_pitch=Some(36);manifest.score_objects.push(bass);
        let request:MusicRead=serde_json::from_value(json!({"view":"score","voices":[{"part_id":"piano","voice":"2"}]})).unwrap();
        let result=read(&manifest,&publication(),&request).unwrap();
        assert_eq!(result.sources[0].target.score.as_ref().unwrap().source_ids,vec!["bass1"]);
        assert!(result.sources[0].target.occurrence_ids.is_empty());
        assert!(result.sources[0].material["read"]["alignments"].as_array().unwrap().is_empty());
        let wrong:MusicRead=serde_json::from_value(json!({"view":"score","voices":[{"part_id":"piano","voice":"left-hand"}]})).unwrap();
        assert!(read(&manifest,&publication(),&wrong).is_err());
    }
}
