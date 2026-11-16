//! Score pagination preserves complete part/voice measures, including rests.
use super::*;
use base_schema::MusicScoreObject;

fn group(note: &MusicScoreObject) -> (&str, &str, &str) {
    (&note.part_id, &note.voice, &note.measure)
}

pub(super) fn read(manifest: &MusicSourceManifest, publication: &PublishedBookRef, request: &MusicRead) -> Result<MusicReadResult, ToolError> {
    // Look ahead to find the next bar boundary. The original filter and offset
    // still define the requested scope; an explicit ID read is not expanded.
    let explicit = !request.source_ids.is_empty() || request.query.as_deref().is_some_and(|q| !q.trim().is_empty());
    let mut lookahead = request.clone();
    lookahead.limit = if explicit { request.limit } else { 200 };
    let mut result = read_snapshot(manifest, publication, &lookahead)?;
    result.body["requested_scope"] = json!({"location":request.location,"part_ids":request.part_ids,"voices":request.voices,"source_ids":request.source_ids,"query":request.query});
    result.body["returned_scope"] = json!([]);
    result.body["range_complete"] = json!(request.offset == 0 && result.body["next_offset"].is_null());
    let Some(source) = result.sources.first_mut() else { return Ok(result); };
    let notes: Vec<MusicScoreObject> = serde_json::from_value(source.material["read"]["objects"].clone()).map_err(|e| error(e.to_string()))?;
    let mut units = Vec::new();
    let mut index = 0;
    while index < notes.len() {
        let start = index;
        let key = group(&notes[start]);
        while index < notes.len() && group(&notes[index]) == key { index += 1; }
        let full: Vec<_> = manifest.score_objects.iter().filter(|n| group(n) == key && !n.is_hidden_muted()).collect();
        let complete = full.len() == index - start && full.iter().all(|n| notes[start..index].iter().any(|v| v.source_id == n.source_id));
        if !explicit && !complete {
            if start == 0 { return Err(error("Score page starts inside a measure or its first measure exceeds 200 objects. Use the returned next_offset, a single voice, or explicit source_ids.")); }
            break;
        }
        let selected: Vec<_> = notes[start..index].iter().collect();
        let marks = score_marks(manifest, &selected);
        let ids: BTreeSet<_> = selected.iter().map(|n| &n.source_id).collect();
        let occurrences: Vec<_> = manifest.occurrences.iter().filter(|o| o.start_ms < source.target.audio.end_ms && o.end_ms > source.target.audio.start_ms && o.score_object_ids.iter().any(|id| ids.contains(id))).map(|o| &o.occurrence_id).collect();
        units.push(json!({"end":index,"part_id":key.0,"voice":key.1,"measure":key.2,"measure_complete":complete,
            "mark_ids":marks.iter().map(|m| &m.mark_id).collect::<Vec<_>>(),"occurrence_ids":occurrences}));
        if index >= request.limit { break; }
    }
    let count = units.iter().rposition(|u| u["end"].as_u64().unwrap() as usize <= request.limit).map(|i| i + 1).unwrap_or(0);
    if count == 0 {
        let needed = units.first().and_then(|u| u["end"].as_u64()).unwrap_or(201);
        return Err(error(format!("The first complete measure needs {needed} score objects. Set limit to at least {needed}, or select a single voice; use explicit source_ids only for an intentional fragment.")));
    }
    source.material["read"]["units"] = json!(units);
    // These are stable instructions, not evidence to repeat in every read.
    source.material["read"].as_object_mut().unwrap().remove("notation_guidance");
    source.material["read"].as_object_mut().unwrap().remove("guidance");
    crop_score_result(&result, count).ok_or_else(|| error("No complete score unit fits this page"))
}

/// Runtime byte budgeting uses the same atomic boundaries as item pagination.
/// Only the admitted objects and their governing notation become evidence.
pub fn crop_score_result(original: &MusicReadResult, unit_count: usize) -> Option<MusicReadResult> {
    let mut result = original.clone();
    let source = result.sources.first_mut()?;
    let read = source.material.get_mut("read")?;
    let units = read["units"].as_array()?.get(..unit_count)?.to_vec();
    let returned = units.last()?["end"].as_u64()? as usize;
    let objects = read["objects"].as_array()?.get(..returned)?.to_vec();
    let ids: BTreeSet<String> = objects.iter().filter_map(|n| n["source_id"].as_str().map(str::to_owned)).collect();
    let mark_ids: BTreeSet<_> = units.iter().flat_map(|u| u["mark_ids"].as_array().into_iter().flatten()).filter_map(Value::as_str).collect();
    let occurrence_ids: BTreeSet<_> = units.iter().flat_map(|u| u["occurrence_ids"].as_array().into_iter().flatten()).filter_map(Value::as_str).collect();
    for (field, key) in [("alignments", "score_object_id"), ("score_marks", "mark_id")] {
        if let Some(items) = read[field].as_array_mut() {
            items.retain(|n| n[key].as_str().is_some_and(|id| if field == "score_marks" { mark_ids.contains(id) } else { ids.contains(id) }));
        }
    }
    if let Some(pairs) = read["note_pairs"].as_array_mut() { pairs.retain(|p| ["from_id", "to_id"].iter().all(|k| p[k].as_str().is_some_and(|id| ids.contains(id)))); }
    if let Some(ties) = read["tie_groups"].as_array_mut() { ties.retain(|t| t["source_ids"].as_array().is_some_and(|v| v.iter().all(|id| id.as_str().is_some_and(|id| ids.contains(id))))); }
    let mut source_ids = ids;
    for mark in read["score_marks"].as_array().into_iter().flatten() {
        source_ids.extend(mark["score_object_ids"].as_array().into_iter().flatten().filter_map(|id| id.as_str().map(str::to_owned)));
    }
    let offset = result.body["offset"].as_u64()? as usize;
    let total = result.body["total"].as_u64()? as usize;
    let next = (offset + returned < total).then_some(offset + returned);
    read["objects"] = json!(objects);
    read["returned_count"] = json!(returned);
    read["next_offset"] = json!(next);
    read["units"] = json!(units);
    result.body["returned_count"] = json!(returned);
    result.body["next_offset"] = json!(next);
    result.body["range_complete"] = json!(offset == 0 && next.is_none());
    result.body["returned_scope"] = json!(units.iter().map(|u| json!({"part_id":u["part_id"],"voice":u["voice"],"measure":u["measure"],"measure_complete":u["measure_complete"]})).collect::<Vec<_>>());
    if let Some(score) = &mut source.target.score { score.source_ids = source_ids.into_iter().collect(); }
    source.target.occurrence_ids.retain(|id| occurrence_ids.contains(id.as_str()));
    if let Some(occurrences) = source.material["occurrences"].as_array_mut() {
        occurrences.retain(|o| o["occurrence_id"].as_str().is_some_and(|id| occurrence_ids.contains(id)));
    }
    // Preview only the notation retained after cropping, never a later bar.
    source.preview = source.material["read"]["score_marks"].as_array().into_iter().flatten()
        .filter_map(|m| serde_json::from_value::<base_schema::MusicScoreMark>(m.clone()).ok()).map(|m| score_mark_preview(&m)).collect::<Vec<_>>().join("\n");
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paging_keeps_the_last_notes_and_rests_of_each_measure() {
        let mut manifest = super::super::tests::fixture();
        let prototype = manifest.score_objects[0].clone();
        manifest.score_objects = (0..12).map(|i| { let mut n = prototype.clone(); n.source_id = format!("n{i}"); n.measure = (i / 6 + 1).to_string(); n.beat = (i % 6) as f64; n.rest = i % 6 == 5; n }).collect();
        let publication = PublishedBookRef { book_id: manifest.book_id.clone(), publication_id: "p".into() };
        let request: MusicRead = serde_json::from_value(json!({"view":"score","limit":8})).unwrap();
        let result = read(&manifest, &publication, &request).unwrap();
        assert_eq!(result.body["returned_count"], 6);
        assert_eq!(result.body["next_offset"], 6);
        assert_eq!(result.body["range_complete"], false);
        assert_eq!(result.sources[0].material["read"]["objects"][5]["rest"], true);
        let mut next = request.clone(); next.offset = 6;
        assert_eq!(read(&manifest, &publication, &next).unwrap().body["returned_count"], 6);
        next.offset = 2;
        assert!(read(&manifest, &publication, &next).is_err());
        let mut exact = request; exact.source_ids = vec!["n2".into()];
        let selected = read(&manifest, &publication, &exact).unwrap();
        assert_eq!(selected.body["returned_count"], 1);
        assert_eq!(selected.body["returned_scope"][0]["measure_complete"], false);
        assert_eq!(selected.body["range_complete"], true, "The explicit selection is complete, the measure is not");
        exact.source_ids = vec!["n1".into(),"n2".into(),"n3".into()]; exact.limit=2;
        let fragment=read(&manifest,&publication,&exact).unwrap();
        assert_eq!(fragment.body["returned_count"],2);
        assert_eq!(fragment.body["next_offset"],2);
        assert_eq!(fragment.body["returned_scope"][0]["measure_complete"],false);
    }
}
