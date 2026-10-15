//! Admitted video contributions join the ordinary graph. Recall remains a pointer.
use base_schema::{EvidenceTarget, GraphEdge, GraphIdentity, GraphNode, JointEvidenceMember, ReadOnlyBase,
    VideoMaterial, VideoSourceManifest, PublishedBookRef, FrameLocation, TranscriptMap};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

#[derive(Deserialize)]
struct Catalog { version: String, material: VideoMaterial, batches: Vec<Batch> }
#[derive(Deserialize)]
struct Batch { files: Vec<String> }
#[derive(Deserialize)]
struct Contribution {
    version: String, material: VideoMaterial, input_publication: PublishedBookRef,
    nodes: Vec<GraphNode>, edges: Vec<GraphEdge>, frames: Vec<Frame>,
}
#[derive(Deserialize)]
struct Frame { frame: FrameLocation, path: String }
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T,String> {
    serde_json::from_slice(&std::fs::read(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
}
fn require(ok: bool, message: &str) -> Result<(),String> { if ok {Ok(())} else {Err(format!("Invalid video graph: {message}"))} }
fn union<T: Clone+PartialEq>(a: &mut Vec<T>, b: &[T]) { for item in b { if !a.contains(item) { a.push(item.clone()); } } }
fn validate_target(target: &EvidenceTarget, c: &Contribution, map: &TranscriptMap) -> Result<(),String> {
    match target {
        EvidenceTarget::Text { publication, text_range, .. } => {
            require(publication==&c.input_publication,"text publication")?;
            require(map.segments.iter().any(|s| &s.text_range==text_range),"transcript range")
        },
        EvidenceTarget::Visual {publication,frames,regions} => {
            require(publication==&c.input_publication,"visual publication")?;
            require(regions.is_empty(),"unexpected cropped graph source")?;
            require(frames.iter().all(|f| c.frames.iter().any(|d| &d.frame==f)),"undelivered frame")
        },
        EvidenceTarget::Joint {members} => { for m in members { validate_target(&m.target,c,map)?; } Ok(()) },
        EvidenceTarget::Audio {..} => Err("Graph audio review is not an admitted VI13 input".into()),
    }
}
pub(crate) fn load_into(directory: &Path, base: &mut ReadOnlyBase, source: &str) -> Result<(),String> {
    if !directory.join("video_enrichments.json").exists() { return Ok(()); }
    let catalog: Catalog=read(&directory.join("video_enrichments.json"))?;
    let manifest: VideoSourceManifest=read(&directory.join("video_source_manifest.json"))?;
    let map: TranscriptMap=read(&directory.join(&manifest.transcript_map_path))?;
    require(catalog.version=="video_enrichments.v1" && catalog.material==manifest.material,"catalog binding")?;
    for batch in catalog.batches {
        for file in batch.files.iter().filter(|p|p.ends_with("/graph.json")) {
            let c: Contribution=read(&directory.join(file))?;
            require(c.version=="video_graph.v1" && c.material==manifest.material,"contribution binding")?;
            manifest.validate_publication(&c.input_publication)?;
            for f in &c.frames {
                manifest.source.validate_frame(&f.frame)?;
                require(batch.files.contains(&f.path),"frame dependency")?;
                let bytes=std::fs::read(directory.join(&f.path)).map_err(|e|e.to_string())?;
                require(bytes.len()>=24 && bytes[..8]==[137,80,78,71,13,10,26,10],"frame PNG")?;
                require(u32::from_be_bytes(bytes[16..20].try_into().unwrap())==f.frame.width
                    && u32::from_be_bytes(bytes[20..24].try_into().unwrap())==f.frame.height,"frame dimensions")?;
            }
            for evidence in c.nodes.iter().map(|n|&n.evidence).chain(c.edges.iter().map(|e|&e.evidence)) {
                require(!evidence.is_empty(),"empty support")?;
                for e in evidence {
                    require(!e.support_role.trim().is_empty(),"support role")?;
                    e.target.validate(&manifest,base,source)?;
                    validate_target(&e.target,&c,&map)?;
                }
            }
            for n in c.nodes {
                require(matches!(&n.identity,Some(GraphIdentity::VideoObject{meaning}) if !meaning.trim().is_empty()),"object identity")?;
                require(n.occurrences.iter().chain(n.source_lid.iter()).all(|lid|base.lid_nodes.iter().any(|l|&l.lid==lid)),"text anchor")?;
                if let Some(old)=base.graph_nodes.iter_mut().find(|old|old.id==n.id) {
                    require((old.identity.is_none() || old.identity==n.identity) && old.node_type==n.node_type,"object identity conflict")?;
                    if old.identity.is_none() {old.identity=n.identity.clone();}
                    union(&mut old.evidence,&n.evidence); union(&mut old.occurrences,&n.occurrences);
                } else {base.graph_nodes.push(n);}
            }
            for mut edge in c.edges {
                require(base.graph_nodes.iter().any(|n|n.id==edge.source) && base.graph_nodes.iter().any(|n|n.id==edge.target),"relation endpoints")?;
                if edge.direction==base_schema::Direction::Undirected && edge.source>edge.target {std::mem::swap(&mut edge.source,&mut edge.target);}
                if let Some(old)=base.graph_edges.iter_mut().find(|old|old.edge_type==edge.edge_type && old.direction==edge.direction
                    && ((old.source==edge.source && old.target==edge.target)
                        || (edge.direction==base_schema::Direction::Undirected && old.source==edge.target && old.target==edge.source))) {
                    old.weight=old.weight.max(edge.weight); union(&mut old.evidence,&edge.evidence);
                } else {base.graph_edges.push(edge);}
            }
        }
    }
    Ok(())
}
fn hints(evidence: &[JointEvidenceMember], out: &mut Vec<Value>) {
    for e in evidence {
        match &e.target {
            EvidenceTarget::Visual{frames,regions,..} => {
                for f in frames.iter().chain(regions.iter().map(|r|&r.frame)) {
                    // A one-millisecond interval containing the exact source PTS.
                    let hint=json!({"location":{"video":f.video,"start_ms":f.actual_pts_ms,"end_ms":f.actual_pts_ms+0.001},"sampling":[f.actual_pts_ms]});
                    if !out.contains(&hint) {out.push(hint);}
                }
            },
            EvidenceTarget::Joint{members} => hints(members,out),
            _=>{},
        }
    }
}
impl crate::Book {
    pub(crate) fn video_graph_recall(&self,node:&GraphNode)->Option<Value> {
        if node.identity.is_none() && node.evidence.is_empty() {return None;}
        let mut read_hints=vec![]; hints(&node.evidence,&mut read_hints);
        let mut relations=vec![];
        for e in &self.base.graph_edges {
            let other=if e.source==node.id {&e.target} else if e.target==node.id {&e.source} else {continue};
            if let Some(n)=self.base.graph_nodes.iter().find(|n|&n.id==other) {
                hints(&e.evidence,&mut read_hints); hints(&n.evidence,&mut read_hints);
                relations.push(json!({"edge":e,"node":n}));
            }
        }
        Some(json!({"identity":node.identity,"evidence":node.evidence,"relations":relations,"read_hints":read_hints,
            "evidence_status":"candidate_only","guidance":"Graph support records extraction provenance, not this Run's observations. Read text ranges with book.text and actual frames with video.read_frames in the Run's same-source publication before source.present. Each joint member has its own support role; do not infer speech from visible code."}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(path:&Path,v:&Value) {std::fs::create_dir_all(path.parent().unwrap()).unwrap();std::fs::write(path,v.to_string()).unwrap();}
    #[test]
    fn video_graph_load_recall_preserves_modalities_homonyms_and_rejects_missing_sources() {
        let fixture:Value=serde_json::from_str(include_str!("../../base-schema/fixtures/video-contract.json")).unwrap();
        let f=&fixture["cases"][0];
        let directory=std::env::temp_dir().join(format!("vi13-{}",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let mut base:ReadOnlyBase=serde_json::from_value(f["base"].clone()).unwrap();
        let source=f["source_text"].as_str().unwrap();
        write(&directory.join("video_source_manifest.json"),&f["manifest"]);
        write(&directory.join(f["manifest"]["transcript_map_path"].as_str().unwrap()),&f["map"]);
        let publication=&f["questions"][0]["publication"];
        let frame=&f["questions"][0]["frame"];
        let text=json!({"target":{"kind":"text","publication":publication,"transcript":f["map"]["transcript"],"text_range":f["map"]["segments"][0]["text_range"]},"support_role":"spoken rule"});
        let visual=json!({"target":{"kind":"visual","publication":publication,"frames":[frame],"regions":[]},"support_role":"visible dimensions"});
        let joint=json!({"target":{"kind":"joint","members":[text,visual]},"support_role":"combined explanation"});
        let node=|id:&str,meaning:&str,evidence:Value|json!({"id":id,"type":"concept","name":"x","identity":{"kind":"video_object","meaning":meaning},"occurrences":[],"source_lid":null,"evidence":[evidence]});
        let contribution=json!({"version":"video_graph.v1","material":f["manifest"]["material"],"input_publication":publication,
            "nodes":[node("video:a:x","matrix input",visual.clone()),node("video:a:other","scalar input",visual.clone())],
            "edges":[{"source":"video:a:x","target":"video:a:other","type":"contrasts","direction":"undirected","scope":"local","weight":1,"evidence":[joint]}],
            "frames":[{"frame":frame,"path":"video-enrichments/a/frame.png"}]});
        write(&directory.join("video-enrichments/a/graph.json"),&contribution);
        write(&directory.join("video_enrichments.json"),&json!({"version":"video_enrichments.v1","material":f["manifest"]["material"],"batches":[{"files":["video-enrichments/a/graph.json","video-enrichments/a/frame.png"]}]}));
        let mut png=vec![0u8;24];png[..8].copy_from_slice(&[137,80,78,71,13,10,26,10]);
        png[16..20].copy_from_slice(&(frame["width"].as_u64().unwrap() as u32).to_be_bytes());png[20..24].copy_from_slice(&(frame["height"].as_u64().unwrap() as u32).to_be_bytes());
        std::fs::write(directory.join("video-enrichments/a/frame.png"),png).unwrap();
        load_into(&directory,&mut base,source).unwrap();
        let count=base.graph_edges.len();
        let edge=base.graph_edges.last_mut().unwrap();std::mem::swap(&mut edge.source,&mut edge.target);
        load_into(&directory,&mut base,source).unwrap();assert_eq!(base.graph_edges.len(),count);
        let book=crate::Book::new(base,source);
        let recall=book.concept_candidates("x",None,None).unwrap();assert_eq!(recall.candidates.len(),2);
        assert!(recall.candidates.iter().all(|n|n.occurrences.is_empty()));
        let graph=recall.candidates[0].video_graph.as_ref().unwrap();
        assert_eq!(graph["evidence_status"],"candidate_only");assert_eq!(graph["read_hints"][0]["sampling"][0],frame["actual_pts_ms"]);
        assert_eq!(graph["relations"][0]["edge"]["evidence"][0]["target"]["members"][0]["support_role"],"spoken rule");
        std::fs::remove_file(directory.join("video-enrichments/a/frame.png")).unwrap();
        let mut base=book.base.clone();assert!(load_into(&directory,&mut base,source).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
