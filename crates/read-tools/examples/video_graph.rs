//! Read graph candidates through the same entry used by Resident book.concept.
fn main()->Result<(),Box<dyn std::error::Error>> {
    let args:Vec<_>=std::env::args().collect();
    let book=read_tools::Book::load(&args[1])?;
    let queries:Vec<String>=serde_json::from_slice(&std::fs::read(&args[2])?)?;
    let results:Vec<_>=queries.iter().map(|query| match book.concept_candidates(query,None,Some(20)) {
        Ok(result)=>serde_json::to_value(result).unwrap(),Err(error)=>serde_json::to_value(error).unwrap(),
    }).collect();
    fn lids(target:&base_schema::EvidenceTarget,out:&mut std::collections::BTreeSet<String>) {
        match target {
            base_schema::EvidenceTarget::Text{text_range,..}=>{out.insert(text_range.lid.clone());},
            base_schema::EvidenceTarget::Joint{members}=>for m in members {lids(&m.target,out);},
            _=>{},
        }
    }
    let mut text_lids=std::collections::BTreeSet::new();
    for e in book.base.graph_nodes.iter().flat_map(|n|&n.evidence).chain(book.base.graph_edges.iter().flat_map(|e|&e.evidence)) {lids(&e.target,&mut text_lids);}
    let text_reads:Vec<_>=text_lids.into_iter().map(|lid|serde_json::json!({"text":book.text(&lid,None).unwrap(),"lid":lid})).collect();
    println!("{}",serde_json::json!({"results":results,"nodes":book.base.graph_nodes,"edges":book.base.graph_edges,"text_reads":text_reads}));
    Ok(())
}
