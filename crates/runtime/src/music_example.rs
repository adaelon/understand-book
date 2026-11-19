//! Answer-local, frozen piano examples. Score pitches come from observed notation.
use base_schema::MusicScoreObject;
use read_tools::{music::MusicSourceSnapshot, ToolError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;
mod facts;
pub(crate) use facts::{FactBlock, score_facts, comparison_facts};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct MusicExampleNote {
    pub midi_pitch: u8,
    pub start_ms: u32,
    pub end_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct MusicExample {
    pub ref_id: String,
    pub label: String,
    pub title: String,
    pub kind: String,
    pub tempo_bpm: u32,
    pub duration_ms: u32,
    pub notes: Vec<MusicExampleNote>,
    pub source_ref_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub items: Vec<Item>,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Item {
    Score {
        key: String,
        part_id: String,
        voice: String,
        from_id: String,
        through_id: Option<String>,
    },
    Illustration {
        key: String,
        steps: Vec<Step>,
    },
    Comparison {
        key: String,
        left: Selection,
        right: Selection,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Selection {
    pub part_id: String,
    pub voice: String,
    pub from_id: String,
    pub through_id: Option<String>,
}
impl Item {
    pub fn key(&self) -> &str {
        match self {
            Self::Score { key, .. } | Self::Illustration { key, .. } | Self::Comparison { key, .. } => key,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Step {
    pitches: Vec<String>,
    beats: f64,
}

pub(crate) fn invalid(message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: "INVALID_MUSIC_EXAMPLE".into(),
        category: "invalid_input".into(),
        message: message.into(),
    }
}
pub(crate) fn objects(source: &MusicSourceSnapshot) -> Vec<MusicScoreObject> {
    serde_json::from_value(source.material["read"]["objects"].clone()).unwrap_or_default()
}
pub(crate) fn pitch_name(pitch: u8) -> String {
    format!(
        "{}{}",
        ["C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"][(pitch % 12) as usize],
        i32::from(pitch) / 12 - 1
    )
}
fn millis(beats: f64) -> u32 {
    (beats * 60_000.0 / 66.0).round() as u32
}
fn empty(kind: &str, title: String) -> MusicExample {
    MusicExample {
        ref_id: format!("listen_{}", uuid::Uuid::now_v7().simple()),
        label: String::new(),
        title,
        kind: kind.into(),
        tempo_bpm: 66,
        duration_ms: 0,
        notes: vec![],
        source_ref_id: None,
    }
}
fn label(notes: &[MusicExampleNote]) -> String {
    let mut groups: Vec<(u32, Vec<String>)> = vec![];
    for note in notes {
        if let Some((_, names)) = groups
            .last_mut()
            .filter(|(start, _)| *start == note.start_ms)
        {
            names.push(pitch_name(note.midi_pitch));
        } else {
            groups.push((note.start_ms, vec![pitch_name(note.midi_pitch)]));
        }
    }
    groups
        .into_iter()
        .map(|(_, names)| names.join(" + "))
        .collect::<Vec<_>>()
        .join(" → ")
}

pub(crate) fn score(
    notes: &[MusicScoreObject],
    from: &MusicScoreObject,
    through: Option<&MusicScoreObject>,
    title: String,
) -> Result<(MusicExample, Vec<String>), ToolError> {
    let last = through.unwrap_or(from);
    if from.is_hidden_muted() || last.is_hidden_muted() {
        return Err(invalid("A hidden, explicitly muted score object is not a sounding example endpoint"));
    }
    if from.part_id != last.part_id || from.voice != last.voice {
        return Err(invalid("Endpoints must belong to the same part and voice"));
    }
    // The complete part includes rests and all its voices. Its bar extents keep
    // the selected line's silence when another voice continues to the barline.
    let mut measures: Vec<(&str, f64)> = Vec::new();
    let mut indexes = std::collections::BTreeMap::new();
    for note in notes.iter().filter(|note| note.part_id == from.part_id) {
        let index = *indexes.entry(note.measure.as_str()).or_insert_with(|| {
            measures.push((note.measure.as_str(), 0.0));
            measures.len() - 1
        });
        measures[index].1 = measures[index].1.max(note.beat + note.duration_beats);
    }
    let first_index = *indexes
        .get(from.measure.as_str())
        .ok_or_else(|| invalid("Unknown start measure"))?;
    let last_index = *indexes
        .get(last.measure.as_str())
        .ok_or_else(|| invalid("Unknown end measure"))?;
    if last_index < first_index || last_index == first_index && last.beat < from.beat {
        return Err(invalid("Score endpoints must follow notation order"));
    }
    let mut offsets = vec![0.0; measures.len()];
    for index in 1..measures.len() {
        offsets[index] = offsets[index - 1] + measures[index - 1].1;
    }
    let origin = offsets[first_index] + from.beat;
    let mut selected: Vec<_> = if through.is_some() {
        notes
            .iter()
            .filter(|note| {
                if note.part_id != from.part_id || note.voice != from.voice || note.is_hidden_muted() {
                    return false;
                }
                let index = indexes[note.measure.as_str()];
                index >= first_index
                    && index <= last_index
                    && (index != first_index || note.beat >= from.beat)
                    && (index != last_index || note.beat <= last.beat)
            })
            .collect()
    } else {
        vec![from]
    };
    selected.sort_by(|a, b| {
        indexes[a.measure.as_str()]
            .cmp(&indexes[b.measure.as_str()])
            .then_with(|| a.beat.total_cmp(&b.beat))
    });
    if selected.len() > 200 {
        return Err(invalid("Select a shorter score example with at most 200 objects"));
    }
    let source_ids = selected.iter().map(|note| note.source_id.clone()).collect();
    let mut result = empty("score", title);
    for note in selected {
        if note.duration_beats <= 0.0 || note.unpitched {
            return Err(invalid(
                "Use fixed-pitch notation with a known positive duration",
            ));
        }
        let onset = offsets[indexes[note.measure.as_str()]] + note.beat - origin;
        let start_ms = millis(onset);
        let end_ms = millis(onset + note.duration_beats);
        result.duration_ms = result.duration_ms.max(end_ms);
        if note.rest {
            continue;
        }
        let pitch = note
            .midi_pitch
            .ok_or_else(|| invalid("This score object has no fixed pitch"))?;
        if matches!(
            note.tie,
            base_schema::MusicTie::Stop | base_schema::MusicTie::Continue
        ) {
            if let Some(previous) = result
                .notes
                .iter_mut()
                .rev()
                .find(|v| v.midi_pitch == pitch && v.end_ms == start_ms)
            {
                previous.end_ms = end_ms;
                continue;
            }
        }
        result.notes.push(MusicExampleNote {
            midi_pitch: pitch,
            start_ms,
            end_ms,
        });
    }
    if result.notes.is_empty() {
        return Err(invalid("Select a passage containing pitched notes"));
    }
    result
        .notes
        .sort_by_key(|note| (note.start_ms, note.midi_pitch));
    result.label = if first_index == last_index {
        label(&result.notes)
    } else {
        format!("第 {}–{} 小节连奏", from.measure, last.measure)
    };
    Ok((result, source_ids))
}

pub(crate) fn illustration(steps: &[Step]) -> Result<MusicExample, ToolError> {
    let mut result = empty("illustration", "教学示例".into());
    let mut beat = 0.0;
    for step in steps {
        if !step.beats.is_finite() || step.beats <= 0.0 || beat + step.beats > 64.0 {
            return Err(invalid(
                "Use positive durations up to 64 quarter-note beats per example",
            ));
        }
        for name in &step.pitches {
            let bytes = name.as_bytes();
            let base = match bytes.first() {
                Some(b'C') => 0,
                Some(b'D') => 2,
                Some(b'E') => 4,
                Some(b'F') => 5,
                Some(b'G') => 7,
                Some(b'A') => 9,
                Some(b'B') => 11,
                _ => {
                    return Err(invalid(
                        "Use pitch names with octaves, such as C4, F#4 or Bb4",
                    ))
                }
            };
            let (alter, suffix) = match bytes.get(1) {
                Some(b'#') => (1, &name[2..]),
                Some(b'b') => (-1, &name[2..]),
                _ => (0, &name[1..]),
            };
            let octave: i32 = suffix
                .parse()
                .map_err(|_| invalid("Every pitch requires an octave, such as A4"))?;
            if !(-1..=9).contains(&octave) {
                return Err(invalid("Pitch is outside MIDI 0–127"));
            }
            let pitch = (octave + 1) * 12 + base + alter;
            if !(0..=127).contains(&pitch) {
                return Err(invalid("Pitch is outside MIDI 0–127"));
            }
            result.notes.push(MusicExampleNote {
                midi_pitch: pitch as u8,
                start_ms: millis(beat),
                end_ms: millis(beat + step.beats),
            });
        }
        beat += step.beats;
    }
    if result.notes.is_empty() {
        return Err(invalid("An example needs at least one pitched note"));
    }
    result.duration_ms = millis(beat);
    result.label = label(&result.notes);
    Ok(result)
}

pub(crate) fn spec() -> crate::ToolSpec {
    let selection = json!({"type":"object","properties":{"part_id":{"type":"string"},"voice":{"type":"string"},"from_id":{"type":"string"},"through_id":{"type":"string"}},"required":["part_id","voice","from_id"],"additionalProperties":false});
    let mut score = selection.clone();
    score["properties"]["key"] = json!({"type":"string"});
    score["properties"]["kind"] = json!({"const":"score"});
    score["required"] = json!(["key","kind","part_id","voice","from_id"]);
    crate::tool_schema::complete_spec(crate::ToolSpec {
        name:"music.example".into(),
        description:"Register playable examples and computed score facts. Read both endpoints with music.read first. kind=score selects one part/voice from from_id through through_id inclusive, retaining intervening notes, chords, ties and rests (at most 200 objects). It returns a listen marker and facts_marker. Insert facts_marker as a standalone paragraph to display host-computed pitches, durations and endpoint span verbatim; explain their significance in your prose. kind=comparison takes left/right selections and returns both playable examples plus one facts_marker with both passages and an onset/rhythm/pitch comparison. If rhythmic structures differ, choose explicit corresponding subranges instead of assuming note correspondence. A constant pitch-class shift may still involve different octave changes. These calculations do not establish thematic derivation. Batch a few short passages per call. kind=illustration makes an explicitly invented example from pitch names with octaves and quarter-note durations; empty pitches is rest. Insert listen markers inline where the sound belongs. Registration never starts playback; the fixed piano tempo is quarter=66 and tremolo strokes are not expanded.".into(),
        parameters:json!({"type":"object","properties":{"items":{"type":"array","minItems":1,"maxItems":24,"items":{"oneOf":[score,
            {"type":"object","properties":{"key":{"type":"string"},"kind":{"const":"comparison"},"left":selection,"right":selection},"required":["key","kind","left","right"],"additionalProperties":false},
            {"type":"object","properties":{"key":{"type":"string"},"kind":{"const":"illustration"},"steps":{"type":"array","minItems":1,"maxItems":64,"items":{"type":"object","properties":{"pitches":{"type":"array","maxItems":12,"items":{"type":"string"}},"beats":{"type":"number","exclusiveMinimum":0}},"required":["pitches","beats"],"additionalProperties":false}}},"required":["key","kind","steps"],"additionalProperties":false}
        ]}}},"required":["items"],"additionalProperties":false})
    })
}

pub(crate) fn result(key: &str, example: MusicExample) -> Value {
    json!({"key":key,"marker":format!("[[listen:{}]]", example.ref_id),"example":example})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn note(id: &str, beat: f64, duration: f64, pitch: Option<u8>, tie: &str) -> MusicScoreObject {
        serde_json::from_value(json!({"source_id":id,"part_id":"P48","voice":"1:1","measure":"21","beat":beat,"duration_beats":duration,"midi_pitch":pitch,"rest":pitch.is_none(),"tie":tie,"page_id":"page3","svg_element_id":id})).unwrap()
    }
    #[test]
    fn beethoven21_preserves_four_simultaneous_keys_and_both_rests() {
        let notes = vec![
            note("d", 0.0, 0.75, Some(74), "none"),
            note("r1", 0.75, 0.25, None, "none"),
            note("c", 1.0, 0.75, Some(61), "none"),
            note("e", 1.0, 0.75, Some(64), "none"),
            note("a", 1.0, 0.75, Some(69), "none"),
            note("high_e", 1.0, 0.75, Some(76), "none"),
            note("r2", 1.75, 0.25, None, "none"),
        ];
        let example = score(&notes, &notes[0], Some(&notes[6]), "m21".into())
            .unwrap()
            .0;
        assert_eq!(example.duration_ms, millis(2.0));
        let chord: Vec<_> = example
            .notes
            .iter()
            .filter(|n| n.start_ms == millis(1.0))
            .collect();
        assert_eq!(
            chord.iter().map(|n| n.midi_pitch).collect::<Vec<_>>(),
            vec![61, 64, 69, 76]
        );
        assert!(chord.iter().all(|n| n.end_ms == millis(1.75)));
        assert_eq!(example.notes[0].end_ms, millis(0.75));
    }
    #[test]
    fn ties_hold_but_independent_same_notes_retrigger() {
        let notes = vec![
            note("a", 0.0, 1.0, Some(81), "start"),
            note("b", 1.0, 0.875, Some(81), "stop"),
            note("c", 1.875, 0.125, Some(81), "none"),
        ];
        let example = score(&notes, &notes[0], Some(&notes[2]), "m17".into())
            .unwrap()
            .0;
        assert_eq!(example.notes.len(), 2);
        assert_eq!(example.notes[0].end_ms, millis(1.875));
        let mut wrong = notes[2].clone();
        wrong.voice = "1:2".into();
        assert!(score(&notes, &notes[0], Some(&wrong), "wrong".into()).is_err());
    }
    #[test]
    fn hidden_muted_notes_never_become_audible_examples() {
        let mut hidden = note("hidden", 0.0, 2.0, Some(60), "none");
        hidden.notation = Some(serde_json::from_value(json!({"written_pitch":{"step":"C","alter":0,"octave":4},"note_type":"half",
            "dots":0,"visible":false,"playback_dynamics":0,"chord":false,"grace":false})).unwrap());
        let mut playable = hidden.clone(); playable.source_id = "playable".into(); playable.midi_pitch = Some(69);
        playable.notation.as_mut().unwrap().playback_dynamics = None;
        let notes = vec![note("first",0.0,1.0,Some(76),"none"), hidden.clone(), playable, note("last",1.0,1.0,Some(69),"none")];
        let (example, ids) = score(&notes,&notes[0],Some(&notes[3]),"passage".into()).unwrap();
        assert!(!example.notes.iter().any(|n| n.midi_pitch == 60));
        assert!(!ids.contains(&"hidden".into())); assert!(ids.contains(&"playable".into()));
        assert!(score(&notes,&hidden,None,"hidden".into()).is_err());
    }
    #[test]
    fn illustrations_require_octaves_and_keep_rests() {
        let steps = serde_json::from_value::<Vec<Step>>(
            json!([{"pitches":["D4","F#4","A4"],"beats":1},{"pitches":[],"beats":0.25}]),
        )
        .unwrap();
        let example = illustration(&steps).unwrap();
        assert_eq!(example.notes.len(), 3);
        assert_eq!(example.duration_ms, millis(1.25));
        let steps =
            serde_json::from_value::<Vec<Step>>(json!([{"pitches":["A"],"beats":1}])).unwrap();
        assert!(illustration(&steps).is_err());
    }

    #[test]
    fn continuous_phrase_keeps_bar_silence_and_joins_a_tie_across_the_barline() {
        let mut first = note("first", 0.0, 1.0, Some(69), "none");
        first.measure = "19".into();
        let mut held = note("held", 1.0, 1.0, Some(76), "start");
        held.measure = "19".into();
        let continuation = note("continued", 0.0, 0.75, Some(76), "stop");
        let rest = note("rest", 0.75, 0.25, None, "none");
        let next = note("next", 1.0, 0.5, Some(74), "none");
        let tail = note("tail", 1.5, 0.5, None, "none");
        let notes = vec![first.clone(), held, continuation, rest, next, tail.clone()];
        let (example, ids) = score(&notes, &first, Some(&tail), "phrase".into()).unwrap();
        assert_eq!(example.duration_ms, millis(4.0));
        assert_eq!(example.notes.len(), 3);
        assert_eq!(example.notes[1].start_ms, millis(1.0));
        assert_eq!(example.notes[1].end_ms, millis(2.75));
        assert_eq!(example.notes[2].start_ms, millis(3.0));
        assert_eq!(ids.len(), 6);
        assert!(score(&notes, &tail, Some(&first), "reverse".into()).is_err());
    }

    #[test]
    fn consecutive_chords_stay_simultaneous_and_separate_in_time() {
        let steps = serde_json::from_value::<Vec<Step>>(json!([
            {"pitches":["C4","E4","G4"],"beats":1},
            {"pitches":["F4","A4","C5"],"beats":1},
            {"pitches":["G4","B4","D5"],"beats":1},
            {"pitches":["C4","E4","G4"],"beats":1}
        ]))
        .unwrap();
        let example = illustration(&steps).unwrap();
        assert_eq!(example.notes.len(), 12);
        assert_eq!(example.duration_ms, millis(4.0));
        for step in 0..4 {
            assert_eq!(
                example
                    .notes
                    .iter()
                    .filter(|note| note.start_ms == millis(f64::from(step)))
                    .count(),
                3
            );
        }
    }

    #[test]
    fn a_short_voice_does_not_pull_the_next_bar_forward() {
        let mut low = note("low", 0.0, 1.25, Some(62), "none");
        low.measure = "19".into();
        low.voice = "1:2".into();
        let mut upper = note("upper", 1.75, 0.25, Some(74), "none");
        upper.measure = "19".into();
        let mut next = note("next", 0.0, 2.0, Some(69), "none");
        next.voice = "1:2".into();
        let notes = vec![low.clone(), upper, next.clone()];
        let (example, _) = score(&notes, &low, Some(&next), "lower voice".into()).unwrap();
        assert_eq!(example.notes.len(), 2);
        assert_eq!(example.notes[1].start_ms, millis(2.0));
        assert_eq!(example.duration_ms, millis(4.0));
    }
}
