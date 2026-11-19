//! Prepared music materials are read evidence, never a live audio observation.
use base_schema::{MusicAudioLocation, MusicRecordingRef, MusicScoreLocation, MusicSourceManifest, PublishedBookRef};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::{Book, ToolError};
use std::collections::{BTreeMap, BTreeSet};
mod notation;
pub mod facts;
mod passages;
pub use passages::crop_score_result;

/// Include the notation state at every selected note, even when its declaration
/// precedes the read range. Performance alignment is irrelevant to score facts.
fn score_marks<'a>(manifest: &'a MusicSourceManifest, items: &[&base_schema::MusicScoreObject]) -> Vec<&'a base_schema::MusicScoreMark> {
    use base_schema::MusicScoreMarkValue;
    let mut measures = BTreeMap::new();
    let mut counters = BTreeMap::new();
    for note in &manifest.score_objects {
        measures.entry((note.part_id.as_str(), note.measure.as_str())).or_insert_with(|| {
            let counter = counters.entry(note.part_id.as_str()).or_insert(0u32);
            *counter += 1;
            *counter
        });
    }
    let mut selected = BTreeSet::new();
    for note in items {
        let index = measures[&(note.part_id.as_str(), note.measure.as_str())];
        let staff = note.voice.split(':').next().unwrap_or(&note.voice);
        let mut active: BTreeMap<&str, &base_schema::MusicScoreMark> = BTreeMap::new();
        for mark in &manifest.score_marks {
            let same_staff = mark.part_id == note.part_id && mark.staff.as_deref().is_none_or(|s| s == staff);
            let group = match &mark.value {
                MusicScoreMarkValue::KeySignature { .. } if same_staff => "key",
                MusicScoreMarkValue::TimeSignature { .. } if same_staff => "time",
                value if value.is_tempo() => "tempo",
                _ => {
                    if same_staff && mark.measure_index == index { selected.insert(mark.mark_id.as_str()); }
                    continue;
                }
            };
            if mark.measure_index > index || mark.measure_index == index && mark.beat > note.beat { continue; }
            if active.get(group).is_none_or(|previous| mark.measure_index > previous.measure_index
                || mark.measure_index == previous.measure_index && (mark.beat > previous.beat
                    || mark.beat == previous.beat && (mark.part_id == note.part_id || mark.staff.is_some()))) {
                active.insert(group, mark);
            }
        }
        selected.extend(active.values().map(|mark| mark.mark_id.as_str()));
    }
    manifest.score_marks.iter().filter(|mark| selected.contains(mark.mark_id.as_str())).collect()
}

fn score_mark_preview(mark: &base_schema::MusicScoreMark) -> String {
    use base_schema::MusicScoreMarkValue;
    let description = match &mark.value {
        MusicScoreMarkValue::KeySignature { fifths, .. } => if *fifths == 0 { "调号：无升降号".into() }
            else { format!("调号：{} 个{}号", fifths.unsigned_abs(), if *fifths < 0 { "降" } else { "升" }) },
        MusicScoreMarkValue::TimeSignature { components, .. } => format!("拍号：{}", components.iter().map(|c| format!("{}/{}", c.beats, c.beat_type)).collect::<Vec<_>>().join(" + ")),
        MusicScoreMarkValue::Direction { words, metronome, sound_tempo_quarter_bpm } => {
            let mut labels = words.clone();
            if let Some(m) = metronome { labels.push(format!("节拍器标记：{}{} = {}", m.beat_unit, ".".repeat(m.dots as usize), m.per_minute)); }
            if let Some(bpm) = sound_tempo_quarter_bpm { labels.push(format!("文件播放速度：每分钟 {bpm} 个四分音符")); }
            labels.join("；")
        }
    };
    format!("第 {} 小节：{}", mark.measure, description)
}

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

impl MusicReadResult {
    /// Discovery is useful progress, but does not establish analysis evidence.
    pub fn catalog_event_ids(&self) -> BTreeSet<&str> {
        let catalogs = [&self.body, &self.body["suggestions"], &self.body["catalog_suggestions"]]
            .into_iter().chain(self.sources.iter().map(|source| &source.material["read"]["catalog"]));
        catalogs.flat_map(|catalog| catalog["items"].as_array().into_iter().flatten())
            .filter_map(|item| item["event_id"].as_str()).collect()
    }
}

pub fn error(message: impl Into<String>) -> ToolError {
    ToolError { error_code: "INVALID_MUSIC_SOURCE".into(), category: "invalid_input".into(), message: message.into() }
}

impl Book {
    pub fn load_music_source(&self, dir: &std::path::Path) -> Result<MusicSourceManifest, ToolError> {
        let bytes = std::fs::read(dir.join("music_source_manifest.json"))
            .map_err(|e| error(format!("Cannot read music source manifest: {e}")))?;
        // Internally tagged score directions contain decimal tempos. With the
        // server's arbitrary_precision feature, deserialize through Value just
        // as publication loading does, so those numbers remain readable.
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|e| error(format!("Invalid music source manifest: {e}")))?;
        let mut manifest: MusicSourceManifest = serde_json::from_value(value)
            .map_err(|e| error(format!("Invalid music source manifest: {e}")))?;
        manifest.validate().map_err(error)?;
        if manifest.book_id != self.base.book_id { return Err(error("Music manifest belongs to another book")); }
        notation::enrich(&mut manifest, dir)?;
        Ok(manifest)
    }
}

/// Reads only the explicitly loaded publication; reference identity is supplied by its host.
pub fn read(manifest: &MusicSourceManifest, publication: &PublishedBookRef, request: &MusicRead) -> Result<MusicReadResult, ToolError> {
    if !(1..=200).contains(&request.limit) { return Err(error("Use a limit between 1 and 200")); }
    if request.view == "score" && !manifest.score_objects.is_empty() {
        return passages::read(manifest, publication, request);
    }
    if !matches!(request.view.as_str(), "events" | "score") { return read_snapshot(manifest, publication, request); }
    // Keep complete source snapshots small enough for the runtime to admit a
    // prefix under its actual context budget. Offsets still count input items.
    let page_size = if request.view == "events" || manifest.score_objects.is_empty() { 1 } else { 8 };
    let mut next = request.clone(); next.limit = request.limit.min(page_size);
    let mut result = read_snapshot(manifest, publication, &next)?;
    let mut returned = result.body["returned_count"].as_u64().unwrap_or(0) as usize;
    while returned < request.limit && returned > 0 && result.body["next_offset"].is_number() {
        next.offset = request.offset + returned; next.limit = (request.limit - returned).min(page_size);
        let more = read_snapshot(manifest, publication, &next)?;
        let count = more.body["returned_count"].as_u64().unwrap_or(0) as usize;
        if count == 0 { break; }
        returned += count;
        result.body["next_offset"] = more.body["next_offset"].clone();
        result.sources.extend(more.sources);
    }
    result.body["offset"] = json!(request.offset);
    result.body["returned_count"] = json!(returned);
    Ok(result)
}

fn query_rank(text: &str, query: &str) -> usize {
    if query.is_empty() { return 1; }
    let text = text.to_lowercase();
    let terms: Vec<_> = query.split(|c: char| c.is_whitespace() || matches!(c, ',' | '，' | ';' | '；')).filter(|v| !v.is_empty()).collect();
    terms.iter().filter(|term| text.contains(**term)).count() + if text.contains(query) { terms.len() + 1 } else { 0 }
}

fn matching_events<'a>(manifest: &'a MusicSourceManifest, request: &MusicRead, audio: &MusicAudioLocation, query: &str) -> Vec<&'a base_schema::MusicEvent> {
    let mut matches: Vec<_> = manifest.events.iter().filter_map(|event| {
        if !(event.start_ms < audio.end_ms && event.end_ms > audio.start_ms)
            || request.event_id.as_ref().is_some_and(|id| &event.event_id != id)
            || !(request.part_ids.is_empty() && request.voices.is_empty() || event.roles.iter().any(|role| request.part_ids.contains(&role.part_id)
                || request.voices.iter().any(|voice| voice.part_id == role.part_id && (role.voice_ids.is_empty() || role.voice_ids.contains(&voice.voice))))) { return None; }
        let rank = query_rank(&format!("{} {} {} {} {}", event.event_id, event.title, event.explanation, event.listen_for, event.concept_tags.join(" ")), query);
        (rank > 0).then_some((event, rank))
    }).collect();
    matches.sort_by(|(a, ar), (b, br)| br.cmp(ar).then_with(|| a.start_ms.total_cmp(&b.start_ms)).then_with(|| a.event_id.cmp(&b.event_id)));
    matches.into_iter().map(|(event, _)| event).collect()
}

fn catalog(manifest: &MusicSourceManifest, events: &[&base_schema::MusicEvent], offset: usize, limit: usize) -> Value {
    let items: Vec<_> = events.iter().skip(offset).take(limit).map(|event| {
        let measures: Vec<_> = manifest.occurrences.iter().filter(|o| o.start_ms < event.end_ms && o.end_ms > event.start_ms).map(|o| &o.measure).collect();
        json!({"event_id":event.event_id,"title":event.title,"roles":event.roles,"start_ms":event.start_ms,"end_ms":event.end_ms,
            "first_measure":measures.first(),"last_measure":measures.last(),"relation_count":event.relations.len(),
            "read":{"view":"events","event_id":event.event_id,"limit":1}})
    }).collect();
    json!({"returned_count":items.len(),"items":items,"total":events.len(),"offset":offset,"next_offset":(offset.saturating_add(limit)<events.len()).then_some(offset.saturating_add(limit)),
        "guidance":"Discovery only. Read an event before citing its analysis or using its listening cue. Labels are curated material names, not proof of a motivic relationship."})
}

/// Build one bounded, complete source for host-owned citations and examples.
/// Model-facing reads use `read` to split this projection into budgetable pages.
pub fn read_snapshot(manifest: &MusicSourceManifest, publication: &PublishedBookRef, request: &MusicRead) -> Result<MusicReadResult, ToolError> {
    if manifest.book_id != publication.book_id { return Err(error("Publication and music book do not match")); }
    if !matches!(request.view.as_str(), "overview" | "catalog" | "events" | "score") || !(1..=200).contains(&request.limit) {
        return Err(error("Use view overview, catalog, events or score and a limit between 1 and 200"));
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
    if request.view == "catalog" {
        let events = matching_events(manifest, request, &audio, &query);
        let mut body = catalog(manifest, &events, request.offset, request.limit);
        body["view"] = json!("catalog");
        if events.is_empty() && !query.is_empty() {
            body["suggestions"] = catalog(manifest, &matching_events(manifest, request, &audio, ""), 0, 5);
        }
        return Ok(MusicReadResult { publication: publication.clone(), recording: manifest.recording.source.clone(), observation_kind: "prepared_music_material".into(), heard_audio: false, body, sources: vec![] });
    }
    let occurrences: Vec<_> = manifest.occurrences.iter().filter(|item| item.start_ms < audio.end_ms && item.end_ms > audio.start_ms).collect();
    let mut selected_ids = Vec::new();
    let mut event_ids = Vec::new();
    let mut provenance_ids = Vec::new();
    let mut notation_preview = Vec::new();
    if matches!(request.view.as_str(), "overview" | "score") {
        if let Some(tonality) = &manifest.work.tonality { provenance_ids.extend(tonality.provenance_ids.clone()); }
    }
    let body = match request.view.as_str() {
        "overview" => json!({"work":manifest.work,"recording":manifest.recording,"score":manifest.score,
            "parts":manifest.parts,"structure":manifest.structure.iter().map(|s|json!({"section_id":s.section_id,"parent_id":s.parent_id,"title":s.title,"description":s.description,
                "start_ms":s.start_ms,"end_ms":s.end_ms,"coverage":s.coverage,"event_count":s.event_ids.len()})).collect::<Vec<_>>(),"coverage":manifest.coverage,
            "catalog":catalog(manifest,&matching_events(manifest,request,&audio,""),0,5),
            "rights":manifest.rights,"provenance":manifest.provenance,"demonstrations":manifest.demonstrations,
            "score_marks_available":!manifest.score_marks.is_empty(),
            "guidance":"Read events or score for a bounded range. Score reads include source-anchored key signatures, meters and tempo/direction marks, with inherited notation context. work.tonality is a separately sourced work-level conclusion; a key signature's encoded_mode is the raw file value. Navigation-only sections do not imply detailed analysis. Score synthesis is not a separated recording stem."}),
        "events" => {
            let matching = matching_events(manifest, request, &audio, &query);
            let items: Vec<_> = matching.iter().skip(request.offset).take(request.limit).copied().collect();
            for event in &items {
                event_ids.push(event.event_id.clone());
                for evidence in &event.evidence {
                    provenance_ids.push(evidence.provenance_id.clone());
                    selected_ids.extend(evidence.score_object_ids.clone());
                }
            }
            json!({"events":items,"total":matching.len(),"returned_count":items.len(),"next_offset":next_offset(request, matching.len()),
                "guidance":"Relations point to independently located events. Read both passages before comparing their musical details. A metaphor is an interpretation, not a score fact."})
        },
        "score" if manifest.score.pdf_path.is_some() && manifest.score_objects.is_empty() => {
            let matching: Vec<_> = manifest.pages.iter().filter(|page| query.is_empty() || page.page_id.to_lowercase().contains(&query)).collect();
            let items: Vec<_> = matching.iter().skip(request.offset).take(request.limit).copied().collect();
            provenance_ids.extend(manifest.provenance.iter().filter(|p| p.kind == base_schema::MusicProvenanceKind::Score).map(|p| p.provenance_id.clone()));
            notation_preview.push(format!("PDF 原谱：{}；当前提供谱页，尚无已接纳逐音符索引。", items.iter().map(|page| page.page_id.as_str()).collect::<Vec<_>>().join("、")));
            json!({"pages":items,"total":matching.len(),"returned_count":items.len(),"next_offset":next_offset(request, matching.len()),
                "indexing":"page_only","guidance":"These are original PDF score pages, not transcribed note objects. Use event page evidence for coarse passage navigation. Read the actual page image to discuss unindexed notation; do not infer pitches, rhythms or accepted audio alignment from page availability."})
        },
        "score" => {
            let query_anchors: BTreeSet<_> = manifest.score_marks.iter().filter(|mark| !query.is_empty()
                && format!("{} {} {} {}",mark.mark_id,mark.measure,mark.part_id,serde_json::to_string(&mark.value).unwrap_or_default()).to_lowercase().contains(&query))
                .flat_map(|mark| mark.score_object_ids.iter()).collect();
            let matching: Vec<_> = manifest.score_objects.iter().filter(|object|
                (!object.is_hidden_muted() || request.source_ids.contains(&object.source_id)) &&
                (request.part_ids.is_empty() && request.voices.is_empty() || request.part_ids.contains(&object.part_id)
                    || request.voices.iter().any(|voice|voice.part_id == object.part_id && voice.voice == object.voice))
                && (request.source_ids.is_empty() || request.source_ids.contains(&object.source_id))
                && (request.location.is_none() || occurrences.iter().any(|occurrence| occurrence.score_object_ids.contains(&object.source_id)))
                && (query.is_empty() || query_anchors.contains(&object.source_id)
                    || format!("{} {} {} {}",object.source_id,object.measure,object.voice,object.part_id).to_lowercase().contains(&query))).collect();
            let items: Vec<_> = matching.iter().skip(request.offset).take(request.limit).copied().collect();
            let marks = score_marks(manifest, &items);
            selected_ids.extend(items.iter().map(|object| object.source_id.clone()));
            let alignments: Vec<_> = manifest.note_alignments.iter().filter(|note| selected_ids.contains(&note.score_object_id)
                && occurrences.iter().any(|occurrence| occurrence.occurrence_id == note.occurrence_id)
                && note.start_ms < audio.end_ms && note.end_ms > audio.start_ms).collect();
            provenance_ids.extend(alignments.iter().map(|note| note.provenance_id.clone()));
            provenance_ids.extend(manifest.provenance.iter().filter(|p| p.kind == base_schema::MusicProvenanceKind::Score).map(|p| p.provenance_id.clone()));
            for mark in &marks {
                selected_ids.extend(mark.score_object_ids.clone());
                provenance_ids.push(mark.provenance_id.clone());
                notation_preview.push(score_mark_preview(mark));
            }
            if let Some(t) = &manifest.work.tonality { notation_preview.push(format!("作品调性：{} {}。{}", t.tonic, t.mode, t.explanation)); }
            json!({"objects":items,"alignments":alignments,"note_pairs":facts::note_pairs(&manifest.score_objects,&items),"tie_groups":facts::tie_groups(&items),
                "total":matching.len(),"returned_count":items.len(),"next_offset":next_offset(request, matching.len()),
                "notation_guidance":"notation retains original spelling, note_type, dots, visibility and playback dynamics. rhythm retains time_modification and tremolo marks; absent rhythm is unknown, while an empty tremolos list is inspected absence. duration_beats is in quarter-note units and already includes time_modification. A tremolo span is repeated strokes, not a single held attack. Chord members are simultaneous; tied continuations are not new attacks. Two dots mean double-dotted. Hidden and explicitly muted objects are omitted by default, but exact source_ids can inspect them. Other hidden notes remain. midi_pitch is sounding pitch. note_pairs compares adjacent written notes within this snapshot, not attacks or transpositions between passages. tie_groups sums complete ties; neither establishes thematic derivation.",
                "score_marks":marks,"work_tonality":manifest.work.tonality,
                "guidance":"score_marks includes key, time and tempo declarations governing the selected notes, plus directions in their measures. Each mark keeps its original measure/beat, source anchors and provenance even when inherited from an earlier measure. encoded_mode is the raw key-signature encoding; use separately sourced work_tonality for the work's tonal identity and its documented reconciliation. sound_tempo_quarter_bpm is a MusicXML playback parameter, not a printed metronome mark or measured recording tempo. These score facts do not require accepted audio alignment or listening. Repeated score objects can have several performance occurrences. attack=false denotes sustained/tied notes; note end is not a measured acoustic release unless the provenance establishes it."})
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
    let preview = if !notation_preview.is_empty() { notation_preview.join("\n") } else if event_ids.is_empty() { format!("{} · {}",manifest.work.composer,manifest.work.title) } else {
        manifest.events.iter().filter(|event| event_ids.contains(&event.event_id)).map(|event| format!("{}：{}",event.title,event.explanation)).collect::<Vec<_>>().join("\n")
    };
    let mut summary = json!({"view":request.view,"offset":request.offset,"returned_count":body.get("returned_count"),"total":body.get("total"),"next_offset":body.get("next_offset")});
    if request.view == "events" && body["total"] == 0 {
        summary["catalog_suggestions"] = catalog(manifest,&matching_events(manifest,request,&audio,""),0,5);
    }
    let occurrence_locations:Vec<_>=occurrences.iter().map(|occurrence| json!({"occurrence_id":occurrence.occurrence_id,"order":occurrence.order,
        "measure":occurrence.measure,"start_ms":occurrence.start_ms,"end_ms":occurrence.end_ms})).collect();
    let material = json!({"view":request.view,"read":body,"provenance":provenance,"coverage":coverage,
        "occurrences":occurrence_locations,"observation_kind":"prepared_music_material","heard_audio":false});
    let target = MusicSourceTarget { publication: publication.clone(), audio,
        score: (!selected_ids.is_empty()).then(|| MusicScoreLocation { score: manifest.score.source.clone(), source_ids: selected_ids }),
        occurrence_ids: occurrences.iter().map(|o| o.occurrence_id.clone()).collect(), event_ids, provenance_ids };
    let sources = if request.view != "overview" && material["read"]["returned_count"] == 0 { vec![] } else {
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
    pub(super) fn fixture() -> MusicSourceManifest {
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
    fn catalog_discovers_separate_terms_and_recovers_from_an_empty_search_without_citing_labels() {
        let mut manifest = fixture();
        manifest.events[0].title = "主题的出现".into();
        let mut later = manifest.events[0].clone(); later.event_id = "later".into(); later.title = "动机素材".into();
        manifest.events.push(later);
        let request = |value| serde_json::from_value::<MusicRead>(value).unwrap();
        let found = read(&manifest, &publication(), &request(json!({"view":"catalog","query":"动机 主题 素材","limit":1}))).unwrap();
        assert!(found.sources.is_empty(), "A catalog label is not an observed analysis");
        assert_eq!(found.body["total"], 2);
        assert_eq!(found.body["items"][0]["event_id"], "later");
        assert_eq!(found.body["next_offset"], 1);
        let full = read(&manifest, &publication(), &request(found.body["items"][0]["read"].clone())).unwrap();
        assert_eq!(full.sources[0].target.event_ids, vec!["later"]);
        for view in ["catalog", "events"] {
            let missing = read(&manifest, &publication(), &request(json!({"view":view,"query":"absent"}))).unwrap();
            assert!(missing.sources.is_empty());
            let suggestions = if view == "catalog" { &missing.body["suggestions"] } else { &missing.body["catalog_suggestions"] };
            assert_eq!(suggestions["total"], 2);
        }
        let events = read(&manifest, &publication(), &request(json!({"view":"events","query":"动机 主题 素材"}))).unwrap();
        assert_eq!(events.sources.len(), 2);
        assert_eq!(events.sources[0].target.event_ids, vec!["later"]);
        assert_eq!(events.body["returned_count"], 2);
    }

    #[test]
    fn score_pages_keep_order_and_only_hide_objects_that_are_explicitly_muted() {
        let mut manifest = fixture();
        let prototype = manifest.score_objects[0].clone();
        manifest.score_objects = (0..21).map(|i| {
            let mut n = prototype.clone(); n.source_id = format!("n{i}"); n.beat = (i % 6) as f64; n.measure = (i / 6 + 1).to_string();
            n.notation = Some(serde_json::from_value(json!({"written_pitch":{"step":"C","alter":0,"octave":4},"note_type":"quarter","dots":0,
                "visible":i > 1,"playback_dynamics":if i == 0 { Some(0) } else { None },"chord":false,"grace":false})).unwrap()); n
        }).collect();
        let request = |value| serde_json::from_value::<MusicRead>(value).unwrap();
        let first = read(&manifest, &publication(), &request(json!({"view":"score","limit":17}))).unwrap();
        assert_eq!(first.body["returned_count"], 17);
        assert_eq!(first.body["total"], 20);
        assert_eq!(first.body["next_offset"], 17);
        assert_eq!(first.sources.len(), 1);
        let objects: Vec<_> = first.sources.iter().flat_map(|s| s.material["read"]["objects"].as_array().unwrap()).collect();
        assert_eq!(objects.iter().map(|n|n["source_id"].as_str().unwrap()).collect::<Vec<_>>(), (1..=17).map(|i|format!("n{i}")).collect::<Vec<_>>());
        let last = read(&manifest, &publication(), &request(json!({"view":"score","offset":17,"limit":17}))).unwrap();
        assert_eq!(last.body["returned_count"], 3); assert!(last.body["next_offset"].is_null());
        let exhausted = read(&manifest, &publication(), &request(json!({"view":"score","offset":20}))).unwrap();
        assert!(exhausted.sources.is_empty(), "An exhausted page supplies no new score evidence");
        let inspect = read(&manifest, &publication(), &request(json!({"view":"score","source_ids":["n0"]}))).unwrap();
        assert_eq!(inspect.sources[0].material["read"]["objects"][0]["notation"]["playback_dynamics"].as_f64(), Some(0.0));
        assert_eq!(inspect.sources[0].target.score.as_ref().unwrap().source_ids, vec!["n0"]);
    }

    #[test]
    fn notation_context_survives_paging_changes_and_source_citation_without_audio_alignment() {
        let mut manifest = fixture();
        manifest.occurrences.clear(); manifest.note_alignments.clear();
        manifest.parts[0].voices.push("2:5".into());
        for (id, measure) in [("middle", "2"), ("later", "3")] {
            let mut note = manifest.score_objects[0].clone();
            note.source_id = id.into(); note.measure = measure.into(); note.voice = "2:5".into();
            manifest.score_objects.push(note);
        }
        manifest.provenance.push(serde_json::from_value(json!({"provenance_id":"edition","kind":"reference","description":"Publisher identifies F minor","source_url":"https://example.test/edition","processor":null})).unwrap());
        manifest.work.tonality = Some(serde_json::from_value(json!({"tonic":"F","mode":"minor","explanation":"The work is F minor; the file preserves the relative-major key encoding.","provenance_ids":["edition","notation"]})).unwrap());
        let mark = |id: &str, measure: &str, index: u32, anchor: &str, value: Value| json!({"mark_id":id,"part_id":"piano","measure":measure,"measure_index":index,"beat":0,"staff":null,
            "value":value,"score_object_ids":[anchor],"provenance_id":"notation"});
        manifest.score_marks = serde_json::from_value(json!([
            mark("key1","1",1,"note1",json!({"kind":"key_signature","fifths":-4,"encoded_mode":"major"})),
            mark("time1","1",1,"note1",json!({"kind":"time_signature","components":[{"beats":"2","beat_type":4}],"symbol":null})),
            mark("tempo1","1",1,"note1",json!({"kind":"direction","words":["Allegro moderato"],"metronome":null,"sound_tempo_quarter_bpm":80})),
            mark("key3","3",3,"later",json!({"kind":"key_signature","fifths":0,"encoded_mode":"major"}))
        ])).unwrap();
        manifest.score_marks[2].staff = Some("1".into()); // A tempo printed over the upper staff also governs the lower staff.
        let request = |value| serde_json::from_value::<MusicRead>(value).unwrap();
        let result = read(&manifest,&publication(),&request(json!({"view":"score","source_ids":["middle"],"limit":1}))).unwrap();
        let saved = &result.sources[0];
        assert_eq!(saved.material["read"]["score_marks"].as_array().unwrap().len(),3,"Middle-measure reads inherit opening declarations");
        assert_eq!(saved.material["read"]["score_marks"][0]["value"]["encoded_mode"],"major");
        assert_eq!(saved.material["read"]["work_tonality"]["mode"],"minor");
        assert!(saved.target.score.as_ref().unwrap().source_ids.contains(&"note1".into()),"Citations retain the declaration's original page anchors");
        assert!(saved.target.provenance_ids.contains(&"edition".into()));
        assert!(saved.preview.contains("Allegro moderato") && saved.preview.contains("2/4"));
        assert!(!result.heard_audio);
        let later = read(&manifest,&publication(),&request(json!({"view":"score","source_ids":["later"]}))).unwrap();
        let marks = later.sources[0].material["read"]["score_marks"].as_array().unwrap();
        assert!(marks.iter().any(|m|m["mark_id"]=="key3") && !marks.iter().any(|m|m["mark_id"]=="key1"));
        let searched = read(&manifest,&publication(),&request(json!({"view":"score","query":"Allegro"}))).unwrap();
        assert!(searched.sources[0].preview.contains("Allegro moderato"));
        let overview = read(&manifest,&publication(),&request(json!({"view":"overview"}))).unwrap();
        assert!(overview.sources[0].target.provenance_ids.contains(&"edition".into()));
        manifest.work.tonality.as_mut().unwrap().mode = "changed later".into();
        assert_eq!(saved.material["read"]["work_tonality"]["mode"],"minor","Saved citations retain the fact actually read");
    }

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
