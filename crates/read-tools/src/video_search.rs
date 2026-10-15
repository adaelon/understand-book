//! VI11 visual cards locate candidates; they never create an observation receipt.
use crate::{Book, ToolError};
use base_schema::{FrameLocation, MediaLocation, PublishedBookRef, VideoMaterial, VideoSourceManifest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::Path};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VideoSearch {
    pub query: String,
    pub within: Option<MediaLocation>,
    pub analysis_id: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}
fn default_limit() -> usize { 8 }
fn invalid(message: impl ToString) -> ToolError {
    ToolError { error_code: "VIDEO_INDEX_INVALID".into(), category: "invalid_input".into(), message: message.to_string() }
}
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ToolError> {
    serde_json::from_slice(&std::fs::read(path).map_err(invalid)?).map_err(invalid)
}
#[derive(Deserialize)]
struct Catalog { version: String, material: VideoMaterial, batches: Vec<Batch> }
#[derive(Deserialize)]
struct Batch { files: Vec<String> }
#[derive(Deserialize)]
struct Contribution { version: String, analysis_id: String, created_at: String, material: VideoMaterial, units: Vec<Unit> }
#[derive(Clone, Deserialize)]
struct Unit {
    vu_id: String, analysis_id: String, location: MediaLocation,
    representative_frames: Vec<Frame>, screen_text: String, description: String, change: String,
    transcript_segment_ids: Vec<String>,
}
#[derive(Clone, Deserialize)]
struct Frame { frame: FrameLocation, path: String }

fn han(c: char) -> bool { ('\u{3400}'..='\u{9fff}').contains(&c) }
/// Keep identifiers and code operators distinct; Chinese bigrams permit literal
/// phrase recall without a language service or a hand-maintained concept map.
pub(crate) fn terms(text: &str) -> BTreeSet<String> {
    let chars: Vec<_> = text.to_lowercase().chars().collect();
    let mut terms = BTreeSet::new(); let mut i = 0;
    while i < chars.len() {
        if han(chars[i]) {
            let start = i; while i < chars.len() && han(chars[i]) { i += 1; }
            for pair in chars[start..i].windows(2) { terms.insert(pair.iter().collect()); }
            if i - start == 1 { terms.insert(chars[start].to_string()); }
        } else if chars[i].is_alphanumeric() || chars[i] == '_' {
            let start = i; while i < chars.len() && !han(chars[i]) && (chars[i].is_alphanumeric() || chars[i] == '_') { i += 1; }
            let token: String = chars[start..i].iter().collect();
            if !["the","a","an","is","are","in","on","of","to","and","or","what","why","where","which","this","that","with","for","can","it","does"].contains(&token.as_str()) { terms.insert(token); }
        } else {
            if i + 1 < chars.len() {
                let pair: String = chars[i..i+2].iter().collect();
                if ["==","!=","<=",">=","->"].contains(&pair.as_str()) { terms.insert(pair); i += 2; continue; }
            }
            if chars[i] == '@' { terms.insert("@".into()); }
            i += 1;
        }
    }
    terms
}
fn clip(a: &MediaLocation, b: &MediaLocation) -> Option<MediaLocation> {
    let start_ms = a.start_ms.max(b.start_ms); let end_ms = a.end_ms.min(b.end_ms);
    (start_ms < end_ms).then(|| MediaLocation { video: a.video.clone(), start_ms, end_ms })
}
fn subtract(ranges: Vec<MediaLocation>, removed: &MediaLocation) -> Vec<MediaLocation> {
    ranges.into_iter().flat_map(|r| {
        let mut parts = vec![];
        if let Some(overlap) = clip(&r, removed) {
            if r.start_ms < overlap.start_ms { parts.push(MediaLocation { end_ms: overlap.start_ms, ..r.clone() }); }
            if overlap.end_ms < r.end_ms { parts.push(MediaLocation { start_ms: overlap.end_ms, ..r }); }
        } else { parts.push(r); }
        parts
    }).collect()
}
fn union(mut ranges: Vec<MediaLocation>) -> Vec<MediaLocation> {
    ranges.sort_by(|a,b| a.start_ms.total_cmp(&b.start_ms));
    let mut merged: Vec<MediaLocation> = vec![];
    for r in ranges {
        if let Some(last) = merged.last_mut() { if r.start_ms <= last.end_ms { last.end_ms = last.end_ms.max(r.end_ms); continue; } }
        merged.push(r);
    }
    merged
}
fn excerpt(text: &str, query: &BTreeSet<String>, max: usize) -> String {
    let lines: Vec<_> = text.lines().filter(|line| !terms(line).is_disjoint(query)).collect();
    let chosen = if lines.is_empty() { text.to_string() } else { lines.join("\n") };
    let mut result: String = chosen.chars().take(max).collect();
    if chosen.chars().count() > max { result.push('…'); }
    result
}

impl Book {
    pub fn search_video(&self, directory: &Path, publication: &PublishedBookRef, request: VideoSearch) -> Result<Value, ToolError> {
        if request.query.trim().is_empty() || request.query.chars().count() > 512 || !(1..=20).contains(&request.limit) {
            return Err(invalid("Use a nonempty query of at most 512 characters and limit 1–20"));
        }
        let declared: Value = read(&directory.join("publication.json"))?;
        if declared["reference"] != json!(publication) { return Err(invalid("Publication identity mismatch")); }
        let (manifest, map) = self.load_video_source(directory)?;
        manifest.validate_publication(publication).map_err(invalid)?;
        let scope = request.within.clone().unwrap_or_else(|| MediaLocation { video: manifest.material.video.clone(), start_ms: 0.0, end_ms: manifest.source.duration_ms });
        manifest.source.validate_location(&scope).map_err(invalid)?;
        let mut contributions = load(directory, &manifest)?;
        if let Some(id) = &request.analysis_id {
            if !contributions.iter().any(|c| &c.analysis_id == id) { return Err(invalid("Unknown visual analysis in this publication")); }
            contributions.retain(|c| &c.analysis_id == id);
        }
        // Creation time determines reanalysis priority, never catalog/batch ID order.
        contributions.sort_by(|a,b| b.created_at.cmp(&a.created_at).then_with(|| b.analysis_id.cmp(&a.analysis_id)));
        let analyses: BTreeSet<_> = contributions.iter().map(|c| c.analysis_id.clone()).collect();
        let mut selected: Vec<(Unit, MediaLocation, Vec<FrameLocation>)> = vec![];
        let mut covered = vec![];
        for c in contributions {
            for u in c.units {
                let Some(range) = clip(&u.location, &scope) else { continue; };
                let mut parts = vec![range.clone()];
                for prior in &covered { parts = subtract(parts, prior); }
                covered.push(range);
                for part in parts {
                    let frames: Vec<_> = u.representative_frames.iter().filter(|f| f.frame.actual_pts_ms >= part.start_ms && f.frame.actual_pts_ms < part.end_ms).map(|f| f.frame.clone()).collect();
                    // A within range can fall between samples. Keep the position as
                    // a candidate and let VI5 sample that range, without moving it.
                    selected.push((u.clone(), part, frames));
                }
            }
        }
        let allowed_vus: BTreeSet<String> = selected.iter().map(|(u,_,_)|u.vu_id.clone()).collect();
        let mut discourse = super::video_discourse::recall(&self.video_discourse, &request.query, &map, &scope,
            request.analysis_id.as_ref().map(|_| &allowed_vus), request.limit, request.offset);
        discourse["coverage"] = json!(manifest.coverage.iter().find(|c|c.capability==base_schema::VideoCapability::Discourse));
        let accepted = union(covered);
        let mut gaps = vec![scope.clone()];
        for r in &accepted { gaps = subtract(gaps, r); }
        let query = terms(&request.query);
        let documents: Vec<_> = selected.iter().map(|(u,_,_)| terms(&format!("{} {} {}",u.screen_text,u.description,u.change))).collect();
        let mut hits = vec![];
        for (index, (u, location, frames)) in selected.iter().enumerate() {
            let matched: Vec<_> = query.intersection(&documents[index]).cloned().collect();
            let exact_id = request.query.trim() == u.vu_id;
            if matched.is_empty() && !exact_id { continue; }
            // Rare terms carry more weight; exact code phrases win ties over a
            // collection of common words from unrelated cards.
            let mut score = matched.iter().map(|t| {
                let df = documents.iter().filter(|d| d.contains(t)).count();
                1.0 + ((documents.len() as f64 + 1.0) / (df as f64 + 1.0)).ln()
            }).sum::<f64>();
            if u.screen_text.to_lowercase().contains(&request.query.trim().to_lowercase()) { score += 5.0; }
            if exact_id { score += 100.0; }
            let transcript_ids: Vec<_> = u.transcript_segment_ids.iter().filter(|id| map.segments.iter().any(|s| &s.segment_id == *id && clip(&s.location, location).is_some())).collect();
            hits.push((score, location.start_ms, json!({"vu_id":u.vu_id,"analysis_id":u.analysis_id,"location":location,
                "frames":frames,"screen_text":excerpt(&u.screen_text,&query,1000),"description":excerpt(&u.description,&query,600),
                "change":excerpt(&u.change,&query,600),"transcript_segment_ids":transcript_ids,"matched_terms":matched,
                "read_hint":{"location":{"video":location.video,"start_ms":location.start_ms,"end_ms":location.end_ms.min(location.start_ms+60000.0)},
                    "sampling":[frames.first().map(|f|f.actual_pts_ms).filter(|t|*t<location.start_ms+60000.0).unwrap_or(location.start_ms)]}})));
        }
        hits.sort_by(|a,b| b.0.total_cmp(&a.0).then_with(|| a.1.total_cmp(&b.1)).then_with(|| a.2["vu_id"].as_str().cmp(&b.2["vu_id"].as_str())));
        let total = hits.len(); let next = request.offset.saturating_add(request.limit);
        let candidates: Vec<_> = hits.into_iter().skip(request.offset).take(request.limit).map(|(_,_,v)|v).collect();
        Ok(json!({"publication":publication,"material":manifest.material,"query":request.query,"within":scope,
            "analysis_ids":analyses,"analysis_selection":if request.analysis_id.is_some(){"explicit"}else{"newest_accepted_at_each_position"},
            "candidates":candidates,"discourse":discourse,"total_matches":total,"next_offset":(next<total).then_some(next),
            "coverage":{"accepted":accepted,"gaps":gaps},"evidence_status":"candidate_only",
            "guidance":"Discourse candidates include source-backed teaching roles and one-hop relations in via; use their text_reads with book.text and read_hints with video.read_frames. Cards and matches are derived pointers, not observed evidence. Use video.read_frames with read_hint or bounded samples in location, then source.present only after observation. No match does not prove absence; inspect coverage gaps or reformulate with visible identifiers or descriptive phrases. Current-frame questions can read unindexed positions directly."}))
    }
}

fn load(directory: &Path, manifest: &VideoSourceManifest) -> Result<Vec<Contribution>, ToolError> {
    if !directory.join("video_enrichments.json").exists() { return Ok(vec![]); }
    let catalog: Catalog = read(&directory.join("video_enrichments.json"))?;
    if catalog.version != "video_enrichments.v1" || catalog.material != manifest.material { return Err(invalid("Visual catalog material mismatch")); }
    let mut result: Vec<Contribution> = vec![];
    for batch in catalog.batches {
        for file in batch.files.iter().filter(|f| f.ends_with("/visual_units.json")) {
            let c: Contribution = read(&directory.join(file))?;
            if c.version != "video_visual_units.v1" || c.material != manifest.material || c.analysis_id.is_empty() || c.created_at.is_empty() {
                return Err(invalid("Visual contribution version/material mismatch"));
            }
            if result.iter().any(|old| old.analysis_id == c.analysis_id && old.created_at != c.created_at) { return Err(invalid("Inconsistent analysis creation time")); }
            for u in &c.units {
                manifest.source.validate_location(&u.location).map_err(invalid)?;
                if u.analysis_id != c.analysis_id || u.representative_frames.is_empty() { return Err(invalid("Visual unit has no bound frames")); }
                for f in &u.representative_frames {
                    manifest.source.validate_frame(&f.frame).map_err(invalid)?;
                    if f.frame.actual_pts_ms < u.location.start_ms || f.frame.actual_pts_ms >= u.location.end_ms
                        || !batch.files.contains(&f.path) || !std::fs::metadata(directory.join(&f.path)).is_ok_and(|m| m.len()>0) {
                        return Err(invalid("Missing or unbound visual frame dependency"));
                    }
                }
            }
            result.push(c);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture { dir: std::path::PathBuf, book: Book, manifest: Value, publication: PublishedBookRef }
    impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.dir); } }
    fn write(path: &Path, value: &Value) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    }
    impl Fixture {
        fn new() -> Self {
            let f: Value = serde_json::from_str(include_str!("../../base-schema/fixtures/video-contract.json")).unwrap();
            let f = &f["cases"][0];
            let dir = std::env::temp_dir().join(format!("vi12-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            let book = Book::new(serde_json::from_value(f["base"].clone()).unwrap(), f["source_text"].as_str().unwrap());
            let mut manifest = f["manifest"].clone();
            manifest["coverage"][0]["accepted"][0]["start_ms"] = json!(0);
            manifest["coverage"][0]["accepted"][0]["end_ms"] = manifest["source"]["duration_ms"].clone();
            manifest["coverage"][0]["pending"] = json!([]); manifest["coverage"][0]["failed"] = json!([]);
            write(&dir.join("video_source_manifest.json"), &manifest);
            write(&dir.join(manifest["transcript_map_path"].as_str().unwrap()), &f["map"]);
            write(&dir.join(manifest["source"]["imported_path"].as_str().unwrap()), &json!("media fixture"));
            write(&dir.join(f["map"]["raw_result_path"].as_str().unwrap()), &json!({}));
            let publication = PublishedBookRef { book_id: manifest["material"]["book_id"].as_str().unwrap().into(), publication_id: "fixed-publication".into() };
            write(&dir.join("publication.json"), &json!({"reference":publication}));
            Self { dir, book, manifest, publication }
        }
        fn unit(&self, id: &str, start: f64, end: f64, text: &str) -> Value {
            json!({"vu_id":id,"analysis_id":"old","location":{"video":self.manifest["material"]["video"],"start_ms":start,"end_ms":end},
                "representative_frames":[{"frame":{"video":self.manifest["material"]["video"],"actual_pts_ms":start,"width":1920,"height":1080},"path":""}],
                "screen_text":text,"description":"三色位图，高亮内存和动态范围","change":"黄色强调移动至精度取舍","transcript_segment_ids":["review-overflow-1"]})
        }
        fn add(&self, id: &str, date: &str, mut units: Vec<Value>) {
            let file = format!("video-enrichments/{id}/visual_units.json"); let mut files = vec![file.clone()];
            for (i,u) in units.iter_mut().enumerate() {
                u["analysis_id"] = json!(id);
                let frame = format!("video-enrichments/{id}/frame-{i}.png");
                u["representative_frames"][0]["path"] = json!(frame); write(&self.dir.join(&frame), &json!("png fixture")); files.push(frame);
            }
            write(&self.dir.join(&file), &json!({"version":"video_visual_units.v1","analysis_id":id,"created_at":date,"material":self.manifest["material"],"units":units}));
            let path = self.dir.join("video_enrichments.json");
            let mut catalog: Value = if path.exists() { read(&path).unwrap() } else { json!({"version":"video_enrichments.v1","material":self.manifest["material"],"batches":[]}) };
            catalog["batches"].as_array_mut().unwrap().push(json!({"files":files})); write(&path, &catalog);
        }
        fn search(&self, input: Value) -> Result<Value, ToolError> { self.book.search_video(&self.dir, &self.publication, serde_json::from_value(input).unwrap()) }
    }
    #[test]
    fn video_search_code_operators_occurrences_descriptions_and_pagination() {
        let f = Fixture::new();
        f.add("old","2026-10-01",vec![f.unit("a",578000.0,579000.0,"torch.float16 assert x == 0"),f.unit("b",579000.0,580000.0,"torch.bfloat16 assert x != 0"),f.unit("a-return",580000.0,581000.0,"torch.float16 assert x == 0")]);
        let r = f.search(json!({"query":"float16","limit":1})).unwrap();
        assert_eq!(r["total_matches"],2); assert_eq!(r["next_offset"],1); assert_eq!(r["candidates"][0]["vu_id"],"a");
        let r = f.search(json!({"query":"float16","limit":1,"offset":1})).unwrap();
        assert_eq!(r["candidates"][0]["vu_id"],"a-return"); assert!(r["next_offset"].is_null());
        let r = f.search(json!({"query":"!="})).unwrap(); assert_eq!(r["total_matches"],1); assert_eq!(r["candidates"][0]["vu_id"],"b");
        assert_eq!(f.search(json!({"query":"请找到强调精度取舍的画面"})).unwrap()["total_matches"],3);
        assert_eq!(f.search(json!({"query":"GPU serial number 序列号"})).unwrap()["total_matches"],0);
        assert_eq!(r["evidence_status"],"candidate_only"); assert_eq!(r["coverage"]["gaps"].as_array().unwrap().len(),2);
        assert_eq!(f.search(json!({"query":"a-return"})).unwrap()["candidates"][0]["vu_id"],"a-return");
    }
    #[test]
    fn video_search_new_analysis_replaces_only_its_range_and_explicit_old_is_readable() {
        let f = Fixture::new();
        f.add("z-old","2026-10-01",vec![f.unit("old",578000.0,581000.0,"oldcode")]);
        f.add("a-new","2026-10-02",vec![f.unit("new",579000.0,580000.0,"newcode")]);
        let r = f.search(json!({"query":"oldcode"})).unwrap();
        assert_eq!(r["total_matches"],2); assert_eq!(r["candidates"][0]["location"]["end_ms"],579000.0);
        assert_eq!(r["candidates"][1]["location"]["start_ms"],580000.0);
        let r = f.search(json!({"query":"oldcode","analysis_id":"z-old"})).unwrap();
        assert_eq!(r["total_matches"],1); assert_eq!(r["candidates"][0]["location"]["end_ms"],581000.0);
        assert!(f.search(json!({"query":"oldcode","analysis_id":"unknown"})).is_err());
        let mut within = f.unit("unused",579200.0,579800.0,"")["location"].clone();
        let r = f.search(json!({"query":"newcode","within":within})).unwrap();
        assert_eq!(r["candidates"][0]["read_hint"]["sampling"][0],579200.0); assert_eq!(r["coverage"]["gaps"],json!([]));
        within["video"]["media_revision"] = json!("other"); assert!(f.search(json!({"query":"newcode","within":within})).is_err());
    }
    #[test]
    fn video_search_absent_index_and_missing_dependency_do_not_claim_coverage() {
        let f = Fixture::new();
        let r = f.search(json!({"query":"float16"})).unwrap(); assert_eq!(r["coverage"]["accepted"],json!([]));
        assert_eq!(r["coverage"]["gaps"][0]["end_ms"],f.manifest["source"]["duration_ms"]);
        assert!(f.search(json!({"query":" "})).is_err()); assert!(f.search(json!({"query":"x","limit":21})).is_err());
        f.add("old","2026-10-01",vec![f.unit("a",578000.0,579000.0,"float16")]);
        std::fs::remove_file(f.dir.join("video-enrichments/old/frame-0.png")).unwrap();
        assert!(f.search(json!({"query":"float16"})).is_err());
        let wrong = PublishedBookRef { publication_id:"other".into(), ..f.publication.clone() };
        assert!(f.book.search_video(&f.dir,&wrong,serde_json::from_value(json!({"query":"x"})).unwrap()).is_err());
    }
}
