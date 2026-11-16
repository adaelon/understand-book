use base_schema::{MusicScoreObject, MusicTie, MusicWrittenPitch};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
mod passage;
pub use passage::{compare, passage, written_name};

fn pitch(p: &MusicWrittenPitch) -> Option<(i32, f64)> {
    let (letter, semitones) = match p.step.as_str() {
        "C" => (0, 0),
        "D" => (1, 2),
        "E" => (2, 4),
        "F" => (3, 5),
        "G" => (4, 7),
        "A" => (5, 9),
        "B" => (6, 11),
        _ => return None,
    };
    Some((
        p.octave * 7 + letter,
        f64::from(p.octave * 12 + semitones) + p.alter,
    ))
}

fn interval(a: &MusicWrittenPitch, b: &MusicWrittenPitch) -> Option<Value> {
    let (da, ca) = pitch(a)?;
    let (db, cb) = pitch(b)?;
    let steps = (db - da).unsigned_abs();
    let number = steps + 1;
    let baseline = f64::from(12 * (steps / 7) + [0, 2, 4, 5, 7, 9, 11][(steps % 7) as usize]);
    let distance = (cb - ca).abs();
    let delta = distance - baseline;
    let perfect = matches!(steps % 7, 0 | 3 | 4);
    let quality = if delta == 0.0 {
        if perfect {
            "perfect"
        } else {
            "major"
        }
    } else if delta == -1.0 {
        if perfect {
            "diminished"
        } else {
            "minor"
        }
    } else if delta == 1.0 {
        "augmented"
    } else if !perfect && delta == -2.0 {
        "diminished"
    } else {
        "other"
    };
    Some(
        json!({"number":number,"quality":quality,"semitones":distance,
        "direction":if cb>ca {"ascending"} else if cb<ca {"descending"} else {"same_pitch"}}),
    )
}

fn ratio(a: f64, b: f64) -> Option<[u32; 2]> {
    if a <= 0.0 || b <= 0.0 {
        return None;
    }
    for denominator in 1..=64 {
        let numerator = a / b * f64::from(denominator);
        if numerator > 0.0 && numerator <= 4096.0 && (numerator - numerator.round()).abs() < 1e-8 {
            return Some([numerator.round() as u32, denominator]);
        }
    }
    None
}

pub(super) fn note_pairs(all: &[MusicScoreObject], selected: &[&MusicScoreObject]) -> Vec<Value> {
    let selected_ids: BTreeSet<_> = selected.iter().map(|n| n.source_id.as_str()).collect();
    let mut voices: BTreeMap<(&str, &str), Vec<&MusicScoreObject>> = BTreeMap::new();
    for n in all.iter().filter(|n| !n.is_hidden_muted()) {
        voices.entry((&n.part_id, &n.voice)).or_default().push(n);
    }
    let mut result = Vec::new();
    for notes in voices.values() {
        // A chord is not a melodic sequence. Skip both edges of a simultaneous
        // onset rather than choosing an arbitrary chord member as a melody.
        let mut chords = BTreeSet::new();
        for pair in notes.windows(2) {
            if pair[0].measure == pair[1].measure && pair[0].beat == pair[1].beat {
                chords.insert((pair[0].measure.as_str(), pair[0].beat.to_bits()));
            }
        }
        for pair in notes.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if !selected_ids.contains(a.source_id.as_str())
                || !selected_ids.contains(b.source_id.as_str())
                || a.rest
                || b.rest
                || a.unpitched
                || b.unpitched
                || chords.contains(&(a.measure.as_str(), a.beat.to_bits()))
                || chords.contains(&(b.measure.as_str(), b.beat.to_bits()))
            {
                continue;
            }
            let written = a
                .notation
                .as_ref()
                .and_then(|n| n.written_pitch.as_ref())
                .zip(b.notation.as_ref().and_then(|n| n.written_pitch.as_ref()))
                .and_then(|(a, b)| interval(a, b));
            result.push(json!({"from_id":a.source_id,"to_id":b.source_id,"written_interval":written,
                "sounding_semitones":a.midi_pitch.zip(b.midi_pitch).map(|(a,b)|i32::from(b)-i32::from(a)),
                "duration_ratio":ratio(a.duration_beats,b.duration_beats),"crosses_barline":a.measure!=b.measure}));
        }
    }
    result
}

/// Report only complete, contiguous ties. Do not call a sliced tie its full duration.
pub(super) fn tie_groups(selected: &[&MusicScoreObject]) -> Vec<Value> {
    let mut held: BTreeMap<(&str, &str, u8), Vec<&MusicScoreObject>> = BTreeMap::new();
    let mut groups = Vec::new();
    for &note in selected {
        let Some(pitch) = note.midi_pitch.filter(|_| !note.is_hidden_muted()) else {
            continue;
        };
        let key = (note.part_id.as_str(), note.voice.as_str(), pitch);
        match note.tie {
            MusicTie::Start => {
                held.insert(key, vec![note]);
            }
            MusicTie::Continue | MusicTie::Stop => {
                let Some(mut chain) = held.remove(&key) else {
                    continue;
                };
                let previous = chain.last().unwrap();
                // Cross-bar ties need all intervening notation to establish duration;
                // the explicit tie start/stop still stays on the returned objects.
                if note.measure != previous.measure
                    || (previous.beat + previous.duration_beats - note.beat).abs() > 1e-8
                {
                    continue;
                }
                chain.push(note);
                if note.tie == MusicTie::Stop {
                    groups.push(
                        json!({"source_ids":chain.iter().map(|n|&n.source_id).collect::<Vec<_>>(),
                        "duration_beats":chain.iter().map(|n|n.duration_beats).sum::<f64>()}),
                    );
                } else {
                    held.insert(key, chain);
                }
            }
            MusicTie::None => {
                held.remove(&key);
            }
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(step: &str, alter: f64, octave: i32) -> MusicWrittenPitch {
        MusicWrittenPitch {
            step: step.into(),
            alter,
            octave,
        }
    }
    #[test]
    fn spells_intervals_instead_of_guessing_from_pitch_class() {
        let fifth = interval(&p("E", 0.0, 5), &p("A", 0.0, 4)).unwrap();
        assert_eq!(fifth["number"], 5);
        assert_eq!(fifth["quality"], "perfect");
        assert_eq!(fifth["direction"], "descending");
        assert_eq!(
            interval(&p("A", 0.0, 5), &p("F", 0.0, 5)).unwrap()["quality"],
            "major"
        );
        assert_eq!(
            interval(&p("F", 0.0, 5), &p("D", 0.0, 5)).unwrap()["quality"],
            "minor"
        );
        assert_eq!(
            interval(&p("C", 0.0, 4), &p("F", 1.0, 4)).unwrap()["quality"],
            "augmented"
        );
        assert_eq!(
            interval(&p("C", 0.0, 4), &p("G", -1.0, 4)).unwrap()["quality"],
            "diminished"
        );
        assert_eq!(ratio(0.875, 0.125), Some([7, 1]));
        assert_eq!(ratio(1.5, 0.5), Some([3, 1]));
    }
    #[test]
    fn selected_gaps_rests_and_chords_do_not_turn_into_a_melodic_interval() {
        let prototype = super::super::tests::fixture().score_objects[0].clone();
        let mut notes: Vec<_> = (0..5)
            .map(|i| {
                let mut n = prototype.clone();
                n.source_id = format!("n{i}");
                n.beat = i as f64;
                n
            })
            .collect();
        assert!(
            note_pairs(&notes, &[&notes[0], &notes[2]]).is_empty(),
            "A filtered-out middle note breaks adjacency"
        );
        let unknown = note_pairs(&notes, &[&notes[0], &notes[1]]);
        assert!(
            unknown[0]["written_interval"].is_null(),
            "MIDI alone does not identify enharmonic spelling"
        );
        notes[1].rest = true;
        notes[1].midi_pitch = None;
        notes[3].beat = notes[2].beat;
        assert!(
            note_pairs(&notes, &notes.iter().collect::<Vec<_>>()).is_empty(),
            "Neither rests nor chord members define a monophonic transition"
        );
    }
    #[test]
    fn tied_written_values_form_one_held_duration_only_when_complete() {
        let mut first = super::super::tests::fixture().score_objects[0].clone();
        first.tie = MusicTie::Start;
        let mut second = first.clone();
        second.source_id = "held".into();
        second.tie = MusicTie::Stop;
        second.beat = 1.0;
        second.duration_beats = 0.875;
        assert_eq!(
            tie_groups(&[&first, &second])[0]["duration_beats"].as_f64(),
            Some(1.875)
        );
        assert!(tie_groups(&[&second]).is_empty());
        second.beat = 1.5;
        assert!(tie_groups(&[&first, &second]).is_empty());
    }
}
