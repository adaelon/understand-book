//! Source-backed teaching roles. Retrieval expands semantic relations, never time proximity.
use base_schema::{EvidenceTarget, FrameLocation, JointEvidenceMember, MediaLocation, PublishedBookRef,
    ReadOnlyBase, TranscriptMap, VideoMaterial, VideoSourceManifest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::Path};

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct Item {
    id: String, mode: String, local_function: Option<String>, rhetorical_move: Option<String>,
    local_summary: String, vu_ids: Vec<String>, evidence: Vec<JointEvidenceMember>, relations: Vec<Relation>,
}
#[derive(Clone, Deserialize, Serialize)]
struct Relation {
    target_id: String, #[serde(rename="type")] kind: String, family: Option<String>, direction: String,
    confidence: f64, rationale: String, evidence: Vec<JointEvidenceMember>,
}
#[derive(Deserialize)]
struct Catalog { version: String, material: VideoMaterial, batches: Vec<Batch> }
#[derive(Deserialize)]
struct Batch { files: Vec<String> }
#[derive(Deserialize)]
struct Contribution {
    version: String, material: VideoMaterial, input_publication: PublishedBookRef,
    items: Vec<Item>, frames: Vec<Frame>, visual_units: Vec<UnitBinding>,
}
#[derive(Deserialize)]
struct Frame { image_id: String, frame: FrameLocation, path: String }
#[derive(Deserialize)]
struct UnitBinding { vu_id: String, image_ids: Vec<String> }
#[derive(Deserialize)]
struct VisualContribution { units: Vec<VisualUnit> }
#[derive(Deserialize)]
struct VisualUnit { vu_id: String, representative_frames: Vec<VisualFrame> }
#[derive(Deserialize)]
struct VisualFrame { frame: FrameLocation }
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T,String> {
    serde_json::from_slice(&std::fs::read(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
}
fn require(ok: bool, why: &str) -> Result<(),String> {
    if ok {Ok(())} else {Err(format!("Invalid video discourse: {why}"))}
}
fn leaves(evidence: &[JointEvidenceMember]) -> Vec<&EvidenceTarget> {
    evidence.iter().flat_map(|e|match &e.target {EvidenceTarget::Joint{members}=>leaves(members), target=>vec![target]}).collect()
}
fn validate_support(evidence: &[JointEvidenceMember], c: &Contribution, manifest: &VideoSourceManifest,
    map: &TranscriptMap, base: &ReadOnlyBase, source: &str) -> Result<(),String> {
    require(!evidence.is_empty(),"empty support")?;
    for e in evidence {
        require(!e.support_role.trim().is_empty(),"empty source role")?;
        e.target.validate(manifest,base,source)?;
    }
    for target in leaves(evidence) {
        match target {
            EvidenceTarget::Text{publication,transcript,text_range} => {
                require(publication==&c.input_publication && transcript.as_ref()==Some(&map.transcript)
                    && map.segments.iter().any(|s|&s.text_range==text_range),"text binding")?;
            },
            EvidenceTarget::Visual{publication,frames,regions} => {
                require(publication==&c.input_publication && regions.is_empty()
                    && frames.iter().all(|f|c.frames.iter().any(|d|&d.frame==f)),"frame binding")?;
            },
            _=>return Err("Discourse requires delivered text or frames".into()),
        }
    }
    Ok(())
}
pub(crate) fn load(directory: &Path, base: &ReadOnlyBase, source: &str) -> Result<Vec<Item>,String> {
    if !directory.join("video_enrichments.json").exists() {return Ok(vec![]);}
    let catalog: Catalog=read(&directory.join("video_enrichments.json"))?;
    let manifest: VideoSourceManifest=read(&directory.join("video_source_manifest.json"))?;
    require(catalog.version=="video_enrichments.v1" && catalog.material==manifest.material,"catalog binding")?;
    let map: TranscriptMap=read(&directory.join(&manifest.transcript_map_path))?;
    let mut units=vec![];
    for file in catalog.batches.iter().flat_map(|b|&b.files).filter(|f|f.ends_with("/visual_units.json")) {
        units.extend(read::<VisualContribution>(&directory.join(file))?.units);
    }
    let mut result: Vec<Item>=vec![];
    for batch in &catalog.batches {
        for file in batch.files.iter().filter(|f|f.ends_with("/discourse.json")) {
            let c: Contribution=read(&directory.join(file))?;
            require(c.version=="video_discourse.v1" && c.material==manifest.material,"contribution binding")?;
            manifest.validate_publication(&c.input_publication)?;
            for f in &c.frames {
                manifest.source.validate_frame(&f.frame)?;
                require(batch.files.contains(&f.path),"frame dependency")?;
                let bytes=std::fs::read(directory.join(&f.path)).map_err(|e|e.to_string())?;
                require(bytes.len()>=24 && bytes[..8]==[137,80,78,71,13,10,26,10],"frame PNG")?;
                require(u32::from_be_bytes(bytes[16..20].try_into().unwrap())==f.frame.width
                    && u32::from_be_bytes(bytes[20..24].try_into().unwrap())==f.frame.height,"frame dimensions")?;
            }
            for binding in &c.visual_units {
                let unit=units.iter().find(|u|u.vu_id==binding.vu_id).ok_or("Discourse VU is missing")?;
                require(!binding.image_ids.is_empty() && binding.image_ids.iter().all(|id|c.frames.iter()
                    .any(|f|&f.image_id==id && unit.representative_frames.iter().any(|p|p.frame==f.frame))),"VU source mapping")?;
            }
            let ids: BTreeSet<_>=c.items.iter().map(|i|&i.id).collect();
            require(!c.items.is_empty() && ids.len()==c.items.len(),"empty or duplicate items")?;
            for item in &c.items {
                require(!item.id.trim().is_empty() && !result.iter().any(|i|i.id==item.id),"duplicate identity")?;
                require(["informative","argumentative","procedural","descriptive","meta"].contains(&item.mode.as_str()),"mode")?;
                require(item.local_function.as_ref().is_none_or(|f| ["definition","description","classification","explanation","cause","effect","example","counterexample","comparison","contrast","procedure_step","application","warning","limitation","question","answer","summary","research_question","hypothesis","related_work","method_description","experiment_setup","evidence_report","result_interpretation","future_work","transition"].contains(&f.as_str())),"local function")?;
                require(item.rhetorical_move.as_ref().is_none_or(|f| ["chapter_setup","problem_framing","prerequisite","main_point","concept_elaboration","worked_example","case_analysis","argument_support","objection","resolution","recap","abstract_summary","related_work_positioning","method_setup","experiment_report","result_claim","limitation_acknowledgement","future_work_projection","bridge_to_next"].contains(&f.as_str())),"rhetorical move")?;
                require(!item.local_summary.trim().is_empty() && item.local_summary.encode_utf16().count()<=200,"summary")?;
                validate_support(&item.evidence,&c,&manifest,&map,base,source)?;
                for id in &item.vu_ids {
                    let binding=c.visual_units.iter().find(|u|&u.vu_id==id).ok_or("Undelivered discourse VU")?;
                    require(leaves(&item.evidence).iter().any(|e|match e {
                        EvidenceTarget::Visual{frames,..}=>frames.iter().any(|f|c.frames.iter().any(|d|d.frame==*f && binding.image_ids.contains(&d.image_id))), _=>false,
                    }),"VU lacks visual support")?;
                }
                for r in &item.relations {
                    require(["elaborates","exemplifies","explains","causes","results_in","contrasts","concedes","supports","rebuts","summarizes","restates","prepares","continues","answers","depends_on"].contains(&r.kind.as_str()),"relation type")?;
                    require(["backward","forward","lateral"].contains(&r.direction.as_str()) && r.family.as_ref().is_none_or(|f|["temporal","contingency","comparison","expansion"].contains(&f.as_str())),"relation direction/family")?;
                    require(r.confidence.is_finite() && (0.5..=1.0).contains(&r.confidence) && !r.rationale.trim().is_empty(),"relation confidence/rationale")?;
                    let target=c.items.iter().find(|i|i.id==r.target_id && i.id!=item.id).ok_or("Discourse relation endpoint")?;
                    validate_support(&r.evidence,&c,&manifest,&map,base,source)?;
                    let support=leaves(&r.evidence);
                    require(leaves(&item.evidence).iter().chain(leaves(&target.evidence).iter()).all(|e|support.contains(e)),"relation missing endpoint support")?;
                }
            }
            result.extend(c.items);
        }
    }
    Ok(result)
}
fn pointers(evidence: &[JointEvidenceMember], map: &TranscriptMap, scope: &MediaLocation) -> (Vec<Value>,Vec<Value>) {
    let mut frames=vec![]; let mut texts=vec![];
    for target in leaves(evidence) {
        match target {
            EvidenceTarget::Visual{frames: fs,..}=> for f in fs {
                if f.actual_pts_ms>=scope.start_ms && f.actual_pts_ms<scope.end_ms {
                    let hint=json!({"location":{"video":f.video,"start_ms":f.actual_pts_ms,"end_ms":(f.actual_pts_ms+0.001).min(scope.end_ms)},"sampling":[f.actual_pts_ms]});
                    if !frames.contains(&hint) {frames.push(hint);}
                }
            },
            EvidenceTarget::Text{text_range,..}=>for s in &map.segments {
                if &s.text_range==text_range && s.location.start_ms<scope.end_ms && s.location.end_ms>scope.start_ms {
                    let hint=json!({"segment_id":s.segment_id,"text_range":text_range,"location":s.location});
                    if !texts.contains(&hint) {texts.push(hint);}
                }
            }, _=>{},
        }
    }
    (frames,texts)
}
pub(crate) fn recall(items: &[Item], query: &str, map: &TranscriptMap, scope: &MediaLocation,
    allowed_vus: Option<&BTreeSet<String>>, limit: usize, offset: usize) -> Value {
    let query=super::video_search::terms(query);
    let selected: Vec<_>=items.iter().filter(|i| {
        let (f,t)=pointers(&i.evidence,map,scope);
        (!f.is_empty() || !t.is_empty()) && allowed_vus.is_none_or(|vus|i.vu_ids.iter().any(|id|vus.contains(id)))
    }).collect();
    let seeds: Vec<_>=selected.iter().filter_map(|i| {
        let tokens=super::video_search::terms(&i.local_summary);
        let matched:Vec<_>=query.intersection(&tokens).cloned().collect();
        (!matched.is_empty()).then_some((*i,matched))
    }).collect();
    let mut rows=vec![];
    for item in &selected {
        let direct=seeds.iter().find(|(i,_)|i.id==item.id);
        let mut via=vec![];
        for (seed,_) in &seeds {
            if seed.id==item.id {continue;}
            for (from,to) in [(*seed,*item),(*item,*seed)] {
                for r in &from.relations {
                    if r.target_id==to.id {via.push(json!({"matched_item_id":seed.id,"source_id":from.id,"target_id":to.id,"relation":r}));}
                }
            }
        }
        if direct.is_none() && via.is_empty() {continue;}
        let (read_hints,text_reads)=pointers(&item.evidence,map,scope);
        let score=direct.map_or(0,|(_,m)|m.len());
        rows.push((score,json!({"discourse_id":item.id,"mode":item.mode,"local_function":item.local_function,
            "rhetorical_move":item.rhetorical_move,"summary":item.local_summary,"vu_ids":item.vu_ids,"evidence":item.evidence,
            "matched_terms":direct.map(|(_,m)|m),"via":via,"read_hints":read_hints,"text_reads":text_reads})));
    }
    rows.sort_by(|a,b|b.0.cmp(&a.0).then_with(||a.1["discourse_id"].as_str().cmp(&b.1["discourse_id"].as_str())));
    let total=rows.len();let next=offset.saturating_add(limit);
    json!({"candidates":rows.into_iter().skip(offset).take(limit).map(|(_,v)|v).collect::<Vec<_>>(),
        "total_matches":total,"next_offset":(next<total).then_some(next),"evidence_status":"candidate_only"})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(path:&Path,v:&Value) {std::fs::create_dir_all(path.parent().unwrap()).unwrap();std::fs::write(path,v.to_string()).unwrap();}
    #[test]
    fn video_discourse_load_search_relation_ablation_scopes_and_missing_sources() {
        let fixture:Value=serde_json::from_str(include_str!("../../base-schema/fixtures/video-contract.json")).unwrap();
        let f=&fixture["cases"][0];
        let dir=std::env::temp_dir().join(format!("vi14-{}",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let publication=&f["questions"][0]["publication"]; let frame=&f["questions"][0]["frame"];
        write(&dir.join("base.json"),&f["base"]);std::fs::write(dir.join("source.txt"),f["source_text"].as_str().unwrap()).unwrap();
        write(&dir.join("publication.json"),&json!({"reference":publication}));
        let mut manifest=f["manifest"].clone();
        manifest["coverage"][0]["accepted"][0]["start_ms"]=json!(0);
        manifest["coverage"][0]["accepted"][0]["end_ms"]=manifest["source"]["duration_ms"].clone();
        manifest["coverage"][0]["pending"]=json!([]);manifest["coverage"][0]["failed"]=json!([]);
        write(&dir.join("video_source_manifest.json"),&manifest);
        write(&dir.join(f["manifest"]["transcript_map_path"].as_str().unwrap()),&f["map"]);
        write(&dir.join(f["manifest"]["source"]["imported_path"].as_str().unwrap()),&json!("media"));
        write(&dir.join(f["map"]["raw_result_path"].as_str().unwrap()),&json!({}));
        let text=json!({"target":{"kind":"text","publication":publication,"transcript":f["map"]["transcript"],"text_range":f["map"]["segments"][0]["text_range"]},"support_role":"spoken rule"});
        let visual=json!({"target":{"kind":"visual","publication":publication,"frames":[frame],"regions":[]},"support_role":"matrix example"});
        let relation=json!({"target_id":"rule","type":"exemplifies","direction":"backward","confidence":0.9,"rationale":"matrix instantiates the rule","evidence":[text,visual]});
        let c=json!({"version":"video_discourse.v1","material":f["manifest"]["material"],"input_publication":publication,
            "items":[
                {"id":"rule","mode":"informative","local_function":"explanation","local_summary":"hidden 求和规则","vu_ids":[],"evidence":[text],"relations":[]},
                {"id":"example","mode":"informative","local_function":"example","local_summary":"矩阵结果","vu_ids":["v1","v2"],"evidence":[visual],"relations":[relation]},
                {"id":"limit","mode":"informative","local_function":"limitation","local_summary":"标签限定","vu_ids":["v1"],"evidence":[visual],"relations":[]}],
            "visual_units":[{"vu_id":"v1","image_ids":["image"]},{"vu_id":"v2","image_ids":["image"]}],
            "frames":[{"image_id":"image","frame":frame,"path":"video-enrichments/d/frame.png"}]});
        let file=dir.join("video-enrichments/d/discourse.json");write(&file,&c);
        let start=frame["actual_pts_ms"].as_f64().unwrap();
        let unit=|id:&str|json!({"vu_id":id,"analysis_id":"visual","location":{"video":frame["video"],"start_ms":start,"end_ms":start+1000.0},
            "representative_frames":[{"frame":frame,"path":"video-enrichments/d/frame.png"}],"screen_text":"matrix","description":"矩阵","change":"","transcript_segment_ids":[]});
        write(&dir.join("video-enrichments/d/visual_units.json"),&json!({"version":"video_visual_units.v1","analysis_id":"visual","created_at":"2026-10-10","material":f["manifest"]["material"],"units":[unit("v1"),unit("v2")]}));
        let catalog=json!({"version":"video_enrichments.v1","material":f["manifest"]["material"],"batches":[{"files":["video-enrichments/d/discourse.json","video-enrichments/d/visual_units.json","video-enrichments/d/frame.png"]}]});
        write(&dir.join("video_enrichments.json"),&catalog);
        let mut png=vec![0u8;24];png[..8].copy_from_slice(&[137,80,78,71,13,10,26,10]);
        png[16..20].copy_from_slice(&(frame["width"].as_u64().unwrap() as u32).to_be_bytes());png[20..24].copy_from_slice(&(frame["height"].as_u64().unwrap() as u32).to_be_bytes());
        std::fs::write(dir.join("video-enrichments/d/frame.png"),png).unwrap();
        let book=crate::Book::load(dir.to_str().unwrap()).unwrap();
        let query=|v:Value|book.search_video(&dir,&serde_json::from_value(publication.clone()).unwrap(),serde_json::from_value(v).unwrap()).unwrap();
        let found=query(json!({"query":"hidden"}));
        assert_eq!(found["total_matches"],0);assert_eq!(found["discourse"]["total_matches"],2);
        let example=&found["discourse"]["candidates"][1];assert_eq!(example["discourse_id"],"example");
        assert_eq!(example["via"][0]["relation"]["type"],"exemplifies");assert_eq!(example["read_hints"][0]["sampling"][0],frame["actual_pts_ms"]);
        assert_eq!(example["vu_ids"],json!(["v1","v2"]));assert_eq!(found["discourse"]["evidence_status"],"candidate_only");
        assert_eq!(query(json!({"query":"GPU serial number"}))["discourse"]["total_matches"],0);
        assert_eq!(query(json!({"query":"hidden","within":{"video":frame["video"],"start_ms":0,"end_ms":1000}}))["discourse"]["total_matches"],0);
        assert_eq!(query(json!({"query":"hidden","analysis_id":"visual"}))["discourse"]["total_matches"],0,"pure text seed is outside explicitly selected VUs");
        assert_eq!(query(json!({"query":"hidden","limit":1}))["discourse"]["next_offset"],1);
        assert_eq!(query(json!({"query":"hidden","limit":1,"offset":1}))["discourse"]["candidates"][0]["discourse_id"],"example");
        let mut no_discourse=crate::Book::load(dir.to_str().unwrap()).unwrap();no_discourse.video_discourse.clear();
        assert_eq!(no_discourse.search_video(&dir,&serde_json::from_value(publication.clone()).unwrap(),serde_json::from_value(json!({"query":"hidden"})).unwrap()).unwrap()["discourse"]["total_matches"],0);
        for (pointer,value) in [
            ("/items/1/relations/0/evidence",json!([visual])),
            ("/items/1/relations/0/type",json!("nearby")),
            ("/items/1/relations/0/target_id",json!("missing")),
            ("/items/1/vu_ids",json!(["unknown"])),
            ("/items/0/evidence/0/target/text_range/lid",json!("unknown")),
        ] {let mut bad=c.clone();*bad.pointer_mut(pointer).unwrap()=value;write(&file,&bad);assert!(crate::Book::load(dir.to_str().unwrap()).is_err(),"{pointer}");}
        write(&file,&c);std::fs::remove_file(dir.join("video-enrichments/d/frame.png")).unwrap();
        assert!(crate::Book::load(dir.to_str().unwrap()).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
