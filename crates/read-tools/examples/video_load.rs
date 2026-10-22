//! Measure cold Book construction against an existing video publication without model calls.
use std::{path::Path, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args().nth(1).ok_or("provide a video publication directory")?;
    let start = Instant::now();
    let book = read_tools::Book::load(&directory)?;
    let load_ms = start.elapsed().as_secs_f64() * 1000.0;
    let (_, transcript) = book.load_video_source(Path::new(&directory)).map_err(|e| e.message)?;
    let publication: serde_json::Value = serde_json::from_slice(&std::fs::read(Path::new(&directory).join("publication.json"))?)?;
    let reference = serde_json::from_value(publication["reference"].clone())?;
    let search = book.search_video(Path::new(&directory), &reference,
        serde_json::from_value(serde_json::json!({"query":"einsum"}))?).map_err(|e| e.message)?;
    println!("{}", serde_json::json!({"load_ms":load_ms,"graph":{
        "nodes":book.base.graph_nodes,"edges":book.base.graph_edges},
        "transcript":transcript,"search":search}));
    Ok(())
}
