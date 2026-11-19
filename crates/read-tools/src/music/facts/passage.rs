use super::*;

pub fn written_name(note: &MusicScoreObject) -> Option<String> {
    let p = note.notation.as_ref()?.written_pitch.as_ref()?;
    let accidental = match p.alter {
        0.0 => String::new(), 1.0 => "♯".into(), -1.0 => "♭".into(),
        2.0 => "𝄪".into(), -2.0 => "𝄫".into(), value => format!("({value:+})"),
    };
    Some(format!("{}{accidental}{}", p.step, p.octave))
}

fn rhythm_label(note: &MusicScoreObject) -> String {
    let Some(notation) = &note.notation else { return format!("{} 四分音符单位", note.duration_beats); };
    let name = match notation.note_type.as_deref().unwrap_or("") {
        "whole" => "全音符", "half" => "二分音符", "quarter" => "四分音符", "eighth" => "八分音符",
        "16th" => "十六分音符", "32nd" => "三十二分音符", "64th" => "六十四分音符", _ => "音符",
    };
    let dots = match notation.dots { 0 => String::new(), 1 => "附点".into(), 2 => "双附点".into(), n => format!("{n} 附点") };
    format!("{dots}{name}{}", if note.rest { "休止" } else { "" })
}

fn sounding_name(note: &MusicScoreObject) -> Option<String> {
    let midi=note.midi_pitch?;
    if note.notation.as_ref().and_then(|n|n.written_pitch.as_ref()).and_then(pitch).is_some_and(|(_,p)|p+12.0==f64::from(midi)) {
        return written_name(note);
    }
    Some(format!("{}{}",["C","C♯","D","D♯","E","F","F♯","G","G♯","A","A♯","B"][(midi%12) as usize],i32::from(midi)/12-1))
}

/// Exact selected notation, in quarter-note units. Full-part measure extents
/// preserve silence in the selected voice and allow cross-bar tie continuity.
pub fn passage(all: &[MusicScoreObject], selected: &[&MusicScoreObject]) -> Value {
    let Some(first) = selected.first() else { return json!({}); };
    let mut measure_order: Vec<&str> = Vec::new();
    let mut lengths: BTreeMap<&str, f64> = BTreeMap::new();
    for n in all.iter().filter(|n| n.part_id == first.part_id) {
        if !lengths.contains_key(n.measure.as_str()) { measure_order.push(&n.measure); }
        let len = lengths.entry(&n.measure).or_default();
        *len = len.max(n.beat + n.duration_beats);
    }
    let mut offsets = BTreeMap::new();
    let mut end = 0.0;
    for measure in measure_order { offsets.insert(measure, end); end += lengths[measure]; }
    let onset = |n: &MusicScoreObject| offsets.get(n.measure.as_str()).copied().unwrap_or(0.0) + n.beat;
    let mut notes = selected.to_vec();
    notes.sort_by(|a,b| onset(a).total_cmp(&onset(b)).then(a.midi_pitch.cmp(&b.midi_pitch)));
    let origin = onset(notes[0]);
    let mut groups: Vec<Value> = Vec::new();
    let mut ties: Vec<Value> = Vec::new();
    let mut held: BTreeMap<u8, (f64, f64, Vec<String>)> = BTreeMap::new();
    let mut measures: Vec<Value> = Vec::new();
    for n in &notes {
        let start = onset(n) - origin;
        let rhythm = serde_json::to_value(n.notation.as_ref().and_then(|v| v.rhythm.as_ref())).unwrap_or(Value::Null);
        let event = json!({"source_id":n.source_id,"written":written_name(n),"sounding":sounding_name(n),"sounding_midi":n.midi_pitch,"rest":n.rest,
            "duration_beats":n.duration_beats,"tie":n.tie,"rhythm":rhythm,"value":rhythm_label(n)});
        if let Some(g) = groups.last_mut().filter(|g| g["onset_beats"].as_f64() == Some(start)) { g["notes"].as_array_mut().unwrap().push(event); }
        else { groups.push(json!({"onset_beats":start,"measure":n.measure,"beat":n.beat,"notes":[event]})); }
        if let Some(pitch) = n.midi_pitch {
            match n.tie {
                MusicTie::Start => { held.insert(pitch, (start, start+n.duration_beats, vec![n.source_id.clone()])); }
                MusicTie::Continue | MusicTie::Stop => {
                    if let Some((begin, previous_end, mut ids)) = held.remove(&pitch).filter(|(_,e,_)| (e-start).abs()<1e-8) {
                        ids.push(n.source_id.clone());
                        if n.tie == MusicTie::Stop { ties.push(json!({"source_ids":ids,"onset_beats":begin,"duration_beats":previous_end+n.duration_beats-begin})); }
                        else { held.insert(pitch,(begin,start+n.duration_beats,ids)); }
                    }
                }
                MusicTie::None => { held.remove(&pitch); }
            }
        }
    }
    for g in &groups {
        if measures.last().is_none_or(|m| m["measure"] != g["measure"]) {
            let measure = g["measure"].as_str().unwrap();
            let expected: BTreeSet<_> = all.iter().filter(|n| n.part_id==first.part_id && n.voice==first.voice && n.measure==measure && !n.is_hidden_muted()).map(|n| &n.source_id).collect();
            let actual: BTreeSet<_> = notes.iter().filter(|n| n.measure==measure).map(|n| &n.source_id).collect();
            measures.push(json!({"measure":measure,"complete":expected==actual,"onsets":[]}));
        }
        measures.last_mut().unwrap()["onsets"].as_array_mut().unwrap().push(g.clone());
    }
    let pitched: Vec<_> = notes.iter().copied().filter(|n| !n.rest && !n.unpitched).collect();
    let endpoints = pitched.first().zip(pitched.last()).filter(|(a,b)| {
        pitched.iter().filter(|n| onset(n)==onset(a)).count()==1 && pitched.iter().filter(|n| onset(n)==onset(b)).count()==1
    }).map(|(a,b)| json!({"from_id":a.source_id,"to_id":b.source_id,
        "from":written_name(a),"to":written_name(b),"written_interval":a.notation.as_ref().and_then(|n|n.written_pitch.as_ref()).zip(b.notation.as_ref().and_then(|n|n.written_pitch.as_ref())).and_then(|(a,b)| interval(a,b)),
        "sounding_semitones":a.midi_pitch.zip(b.midi_pitch).map(|(a,b)|i32::from(b)-i32::from(a))}));
    json!({"part_id":first.part_id,"voice":first.voice,"measures":measures,"complete_ties":ties,
        "endpoint_span":endpoints,"duration_beats":notes.iter().map(|n|onset(n)-origin+n.duration_beats).fold(0.0,f64::max),
        "scope":"selected_notation_only; not proof of a complete musical phrase"})
}

/// Correspondence is computed only for matching onset/rhythm structures. A
/// shared pitch-class shift does not imply a constant shift of absolute pitch.
pub fn compare(left: &Value, right: &Value) -> Value {
    fn onsets(value: &Value) -> Vec<&Value> { value["measures"].as_array().into_iter().flatten().flat_map(|m|m["onsets"].as_array().into_iter().flatten()).collect() }
    let a=onsets(left); let b=onsets(right);
    let rhythm = |groups: &[&Value]| groups.iter().map(|g| json!({"onset":g["onset_beats"],"notes":g["notes"].as_array().unwrap().iter().map(|n|json!([n["duration_beats"],n["rest"],matches!(n["tie"].as_str(),Some("stop"|"continue")),n["rhythm"]])).collect::<Vec<_>>()})).collect::<Vec<_>>();
    let same = !a.is_empty() && rhythm(&a)==rhythm(&b);
    let ties = |groups: &[&Value]| groups.iter().flat_map(|g|g["notes"].as_array().unwrap().iter().map(|n|n["tie"].clone())).collect::<Vec<_>>();
    let mut pairs = Vec::new();
    if same {
        for (ga,gb) in a.iter().zip(&b) {
            for (na,nb) in ga["notes"].as_array().unwrap().iter().zip(gb["notes"].as_array().unwrap()) {
                if let Some((pa,pb)) = na["sounding_midi"].as_i64().zip(nb["sounding_midi"].as_i64()) {
                    let delta=pb-pa;
                    let class=(delta+6).rem_euclid(12)-6;
                    pairs.push(json!({"from":na["written"],"to":nb["written"],"semitones":delta,"pitch_class_shift":class,"register_octaves":(delta-class)/12}));
                }
            }
        }
    }
    let constant = |key: &str| pairs.first().filter(|p|pairs.iter().all(|n| n[key]==p[key])).map(|p|p[key].clone());
    json!({"rhythm_equal":same,"tie_pattern_equal":same && ties(&a)==ties(&b),"correspondence":pairs,"constant_semitones":constant("semitones"),"constant_pitch_class_shift":constant("pitch_class_shift"),
        "scope":if same {"aligned written onset groups; rests and ties retained; tremolo strokes not expanded"} else {"different rhythm/onset structures; no automatic note pairing; select explicit corresponding subranges"}})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn n(id: &str, step: &str, octave: i32, midi: u8, beat: f64) -> MusicScoreObject {
        let mut n=super::super::super::tests::fixture().score_objects[0].clone();
        n.source_id=id.into(); n.beat=beat; n.midi_pitch=Some(midi);
        n.notation=Some(serde_json::from_value(json!({"written_pitch":{"step":step,"alter":0,"octave":octave},"note_type":"quarter","dots":0,"visible":true,"chord":false,"grace":false})).unwrap()); n
    }
    #[test]
    fn cross_bar_ties_require_a_contiguous_complete_chain() {
        let mut first=n("a","A",5,81,1.0); first.tie=MusicTie::Start;
        let mut last=n("b","A",5,81,0.0); last.measure="2".into(); last.duration_beats=0.5; last.tie=MusicTie::Stop;
        let all=vec![first.clone(),last.clone()];
        let complete=passage(&all,&all.iter().collect::<Vec<_>>());
        assert_eq!(complete["complete_ties"][0]["duration_beats"],1.5);
        assert_eq!(passage(&all,&[&last])["complete_ties"],json!([]));
        let mut middle=last.clone(); middle.source_id="middle".into(); middle.duration_beats=2.0; middle.tie=MusicTie::None;
        last.measure="3".into();
        assert_eq!(passage(&[first.clone(),middle,last.clone()],&[&first,&last])["complete_ties"],json!([]));
    }
    #[test]
    fn span_retains_compound_interval_and_register_changes() {
        let notes=vec![n("a","A",5,81,0.0),n("b","F",4,65,1.0)];
        let a=passage(&notes,&notes.iter().collect::<Vec<_>>());
        assert_eq!(a["endpoint_span"]["written_interval"]["number"],10);
        let left=vec![n("a","D",4,62,0.0),n("b","F",5,77,1.0)];
        let right=vec![n("c","C",3,48,0.0),n("d","E",3,63,1.0)];
        let a=passage(&left,&left.iter().collect::<Vec<_>>());
        let mut right=right; right[1].midi_pitch=Some(51); right[1].notation.as_mut().unwrap().written_pitch.as_mut().unwrap().alter=-1.0;
        let b=passage(&right,&right.iter().collect::<Vec<_>>());
        let c=compare(&a,&b);
        assert_eq!(c["rhythm_equal"],true); assert_eq!(c["constant_pitch_class_shift"],-2); assert!(c["constant_semitones"].is_null());
        assert_eq!(c["correspondence"][1]["register_octaves"],-2);
        right[1].beat=1.5;
        assert_eq!(compare(&a,&passage(&right,&right.iter().collect::<Vec<_>>()))["correspondence"],json!([]));
    }
}
