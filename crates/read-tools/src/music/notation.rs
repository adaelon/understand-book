//! Read original notation without changing published score identities or timing.
use crate::ToolError;
use base_schema::{MusicNoteNotation, MusicRhythmNotation, MusicSourceManifest, MusicTimeModification, MusicTremolo, MusicWrittenPitch};
use quick_xml::{events::Event, Reader};
use std::{collections::BTreeMap, path::Path};
fn invalid(error: impl std::fmt::Display) -> ToolError {
    super::error(error.to_string())
}

pub(super) fn enrich(
    manifest: &mut MusicSourceManifest,
    directory: &Path,
) -> Result<(), ToolError> {
    if manifest.score_objects.iter().all(|n| n.notation.as_ref().is_some_and(|n| n.rhythm.is_some())) {
        return Ok(());
    }
    let Some(relative) = &manifest.score.musicxml_path else {
        return Ok(());
    };
    let path = directory.join(relative);
    if !path.exists() {
        return Ok(());
    }
    let xml = std::fs::read_to_string(path).map_err(invalid)?;
    let notes = parse(&xml)?;
    for object in &mut manifest.score_objects {
        if object.notation.as_ref().is_some_and(|n| n.rhythm.is_some()) {
            continue;
        }
        // Both native and archive importers retain the original note ordinal.
        // An explicit original XML ID also works for other published sources.
        let position = object.source_id.rsplit_once("-n").and_then(|(prefix, n)| {
            let (_, m) = prefix.rsplit_once("-m")?;
            Some((
                object.part_id.clone(),
                m.parse::<usize>().ok()?,
                n.parse::<usize>().ok()?,
            ))
        });
        let original = notes
            .by_id
            .get(&object.source_id)
            .or_else(|| position.as_ref().and_then(|key| notes.by_position.get(key)))
            .cloned();
        if let Some(existing) = &mut object.notation {
            // Earlier publications already retain spelling and visibility.
            // Fill only the missing rhythm semantics from the same original.
            if let Some(original) = original { existing.rhythm = original.rhythm; }
        } else {
            object.notation = original;
        }
    }
    Ok(())
}

#[derive(Default)]
struct Notes {
    by_id: BTreeMap<String, MusicNoteNotation>,
    by_position: BTreeMap<(String, usize, usize), MusicNoteNotation>,
}

fn parse(xml: &str) -> Result<Notes, ToolError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut result = Notes::default();
    let (mut part, mut measure, mut ordinal) = (String::new(), 0, 0);
    let mut stack = Vec::<String>::new();
    let mut note: Option<MusicNoteNotation> = None;
    let mut note_id = None;
    loop {
        match reader.read_event().map_err(invalid)? {
            event @ (Event::Start(_) | Event::Empty(_)) => {
                let empty = matches!(&event, Event::Empty(_));
                let e = match event {
                    Event::Start(e) | Event::Empty(e) => e,
                    _ => unreachable!(),
                };
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                let attributes = e
                    .attributes()
                    .map(|a| {
                        let a = a.map_err(invalid)?;
                        Ok((
                            String::from_utf8_lossy(a.key.as_ref()).into_owned(),
                            a.decoded_and_normalized_value(
                                quick_xml::XmlVersion::Implicit1_0,
                                reader.decoder(),
                            )
                            .map_err(invalid)?
                            .into_owned(),
                        ))
                    })
                    .collect::<Result<BTreeMap<_, _>, ToolError>>()?;
                match name.as_str() {
                    "part" => {
                        part = attributes.get("id").cloned().unwrap_or_default();
                        measure = 0;
                    }
                    "measure" => {
                        measure += 1;
                        ordinal = 0;
                    }
                    "note" => {
                        ordinal += 1;
                        note_id = attributes.get("id").cloned();
                        note = Some(MusicNoteNotation {
                            written_pitch: None,
                            note_type: None,
                            dots: 0,
                            visible: attributes.get("print-object").is_none_or(|v| v != "no"),
                            playback_dynamics: attributes
                                .get("dynamics")
                                .and_then(|v| v.parse().ok()),
                            chord: false,
                            grace: false,
                            rhythm: Some(MusicRhythmNotation { time_modification: None, tremolos: vec![] }),
                        });
                    }
                    "pitch" => {
                        if let Some(n) = &mut note {
                            n.written_pitch = Some(MusicWrittenPitch {
                                step: String::new(),
                                alter: 0.0,
                                octave: 0,
                            });
                        }
                    }
                    "dot" if stack.last().is_some_and(|s| s == "note") => {
                        if let Some(n) = &mut note {
                            n.dots += 1;
                        }
                    }
                    "chord" => {
                        if let Some(n) = &mut note {
                            n.chord = true;
                        }
                    }
                    "grace" => {
                        if let Some(n) = &mut note {
                            n.grace = true;
                        }
                    }
                    "time-modification" => {
                        if let Some(n) = &mut note {
                            n.rhythm.as_mut().unwrap().time_modification = Some(MusicTimeModification {
                                actual_notes: 0, normal_notes: 0, normal_type: None, normal_dots: 0,
                            });
                        }
                    }
                    "normal-dot" => {
                        if let Some(m) = note.as_mut().and_then(|n| n.rhythm.as_mut()).and_then(|r| r.time_modification.as_mut()) {
                            m.normal_dots += 1;
                        }
                    }
                    "tremolo" => {
                        if let Some(n) = &mut note {
                            n.rhythm.as_mut().unwrap().tremolos.push(MusicTremolo {
                                kind: attributes.get("type").cloned().unwrap_or_else(|| "single".into()), marks: 0,
                            });
                        }
                    }
                    _ => {}
                }
                if !empty {
                    stack.push(name);
                }
            }
            Event::Text(text) => {
                let value = text.decode().map_err(invalid)?;
                if let Some(n) = &mut note {
                    let leaf = stack.last().map(String::as_str).unwrap_or("");
                    let parent = stack.iter().rev().nth(1).map(String::as_str).unwrap_or("");
                    if parent == "pitch" {
                        if let Some(p) = &mut n.written_pitch {
                            match leaf {
                                "step" => p.step = value.into_owned(),
                                "alter" => p.alter = value.parse().map_err(invalid)?,
                                "octave" => p.octave = value.parse().map_err(invalid)?,
                                _ => {}
                            }
                        }
                    } else if leaf == "type" && parent == "note" {
                        n.note_type = Some(value.into_owned());
                    } else if parent == "time-modification" {
                        let modification = n.rhythm.as_mut().unwrap().time_modification.as_mut().unwrap();
                        match leaf {
                            "actual-notes" => modification.actual_notes = value.parse().map_err(invalid)?,
                            "normal-notes" => modification.normal_notes = value.parse().map_err(invalid)?,
                            "normal-type" => modification.normal_type = Some(value.into_owned()),
                            _ => {},
                        }
                    } else if leaf == "tremolo" {
                        n.rhythm.as_mut().unwrap().tremolos.last_mut().unwrap().marks = value.parse().map_err(invalid)?;
                    }
                }
            }
            Event::End(e) => {
                if e.local_name().as_ref() == b"note" {
                    if let Some(n) = note.take() {
                        if n.rhythm.as_ref().and_then(|r| r.time_modification.as_ref()).is_some_and(|m| m.actual_notes == 0 || m.normal_notes == 0) {
                            return Err(invalid("MusicXML time-modification requires positive actual-notes and normal-notes"));
                        }
                        if let Some(id) = note_id.take() {
                            result.by_id.insert(id, n.clone());
                        }
                        result
                            .by_position
                            .insert((part.clone(), measure, ordinal), n);
                    }
                }
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_notation_recovers_tuplets_and_tremolo_without_retiming_notes() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("music")).unwrap();
        let xml = r#"<score-partwise><part id="piano"><measure number="1"><note><pitch><step>A</step><octave>2</octave></pitch><duration>4</duration><type>16th</type><time-modification><actual-notes>6</actual-notes><normal-notes>4</normal-notes></time-modification></note><note><pitch><step>A</step><octave>2</octave></pitch><duration>24</duration><type>quarter</type><dot/><time-modification><actual-notes>6</actual-notes><normal-notes>4</normal-notes><normal-type>16th</normal-type></time-modification><notations><ornaments><tremolo type="single">2</tremolo></ornaments></notations></note></measure></part></score-partwise>"#;
        std::fs::write(directory.path().join("music/score.musicxml"), xml).unwrap();
        let original = parse(xml).unwrap();
        let mut manifest = super::super::tests::fixture();
        manifest.score_objects.push(manifest.score_objects[0].clone());
        for (i, object) in manifest.score_objects.iter_mut().enumerate() {
            object.source_id = format!("piano-m1-n{}", i + 1);
            let mut legacy = original.by_position[&("piano".into(), 1, i + 1)].clone();
            legacy.rhythm = None;
            object.notation = Some(legacy);
        }
        manifest.score_objects[0].duration_beats = 1.0 / 6.0;
        manifest.score_objects[1].beat = 1.0 / 6.0;
        manifest.score_objects[1].duration_beats = 1.0;
        let before = manifest.score_objects.clone();
        enrich(&mut manifest, directory.path()).unwrap();
        let sixteenth = manifest.score_objects[0].notation.as_ref().unwrap().rhythm.as_ref().unwrap();
        assert_eq!(sixteenth.time_modification.as_ref().unwrap().actual_notes, 6);
        assert_eq!(sixteenth.time_modification.as_ref().unwrap().normal_notes, 4);
        assert!(sixteenth.tremolos.is_empty());
        let tremolo = manifest.score_objects[1].notation.as_ref().unwrap();
        assert_eq!(tremolo.note_type.as_deref(), Some("quarter"));
        assert_eq!(tremolo.dots, 1);
        let rhythm = tremolo.rhythm.as_ref().unwrap();
        assert_eq!(rhythm.time_modification.as_ref().unwrap().normal_type.as_deref(), Some("16th"));
        assert_eq!(rhythm.tremolos, vec![MusicTremolo { kind:"single".into(), marks:2 }]);
        for (actual, original) in manifest.score_objects.iter().zip(before) {
            let mut unchanged = actual.clone();
            unchanged.notation.as_mut().unwrap().rhythm = None;
            assert_eq!(unchanged, original, "Enrichment must not rescale the already imported duration or alter source identity");
        }
    }
    #[test]
    fn existing_publications_recover_notation_from_original_xml_without_changing_identity() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("music")).unwrap();
        std::fs::write(directory.path().join("music/score.musicxml"), r#"<score-partwise><part id="piano"><measure number="0"><note print-object="no" dynamics="0"><pitch><step>C</step><octave>5</octave></pitch><type>half</type></note></measure><measure number="1"><note><pitch><step>E</step><alter>-1</alter><octave>5</octave></pitch><type>32nd</type></note></measure></part></score-partwise>"#).unwrap();
        let mut manifest = super::super::tests::fixture();
        manifest.score_objects[0].source_id = "s-p16-m1-n1".into();
        let mut next = manifest.score_objects[0].clone();
        next.source_id = "piano-m2-n1".into();
        // Native imports keep printed labels; the archive uses source indices.
        next.measure = "1".into();
        manifest.score_objects.push(next);
        let before = manifest.score_objects.clone();
        enrich(&mut manifest, directory.path()).unwrap();
        assert!(manifest.score_objects[0].is_hidden_muted());
        assert_eq!(
            manifest.score_objects[1]
                .notation
                .as_ref()
                .unwrap()
                .note_type
                .as_deref(),
            Some("32nd")
        );
        for (actual, original) in manifest.score_objects.iter().zip(before) {
            let mut unchanged = actual.clone();
            unchanged.notation = None;
            assert_eq!(
                unchanged, original,
                "Source identities, layout and published timing stay frozen"
            );
        }
    }
    #[test]
    fn distinguishes_hidden_muted_objects_from_hidden_playable_notes() {
        let notes = parse(r#"<score-partwise><part id="P48"><measure number="1"><note><rest/><duration>48</duration></note><backup><duration>48</duration></backup><note print-object="no" dynamics="0"><pitch><step>C</step><octave>5</octave></pitch><type>half</type></note><note print-object="no"><pitch><step>E</step><alter>-1</alter><octave>5</octave></pitch><type>eighth</type><dot/><dot/></note></measure></part></score-partwise>"#).unwrap();
        let hidden = &notes.by_position[&("P48".into(), 1, 2)];
        assert!(!hidden.visible);
        assert_eq!(hidden.playback_dynamics, Some(0.0));
        let playable = &notes.by_position[&("P48".into(), 1, 3)];
        assert!(!playable.visible);
        assert_eq!(playable.playback_dynamics, None);
        assert_eq!(playable.dots, 2);
        assert_eq!(playable.written_pitch.as_ref().unwrap().alter, -1.0);
    }
}
