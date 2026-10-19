//! Search one immutable publication; JSON requests are supplied as a batch file.
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    let dir = Path::new(&args[1]);
    let publication: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("publication.json"))?)?;
    let reference = serde_json::from_value(publication["reference"].clone())?;
    let book = read_tools::Book::load(dir.to_str().unwrap())?;
    let requests: Vec<read_tools::video_search::VideoSearch> = serde_json::from_slice(&fs::read(&args[2])?)?;
    let start = std::time::Instant::now();
    let results: Result<Vec<_>, _> = requests.into_iter().map(|r| book.search_video(dir, &reference, r)).collect();
    let results=results.map_err(|e|e.message)?;
    let lids: std::collections::BTreeSet<_>=results.iter().flat_map(|r|r["discourse"]["candidates"].as_array().into_iter().flatten())
        .flat_map(|c|c["text_reads"].as_array().into_iter().flatten())
        .filter_map(|t|t["text_range"]["lid"].as_str()).collect();
    let text_reads:Vec<_>=lids.into_iter().map(|lid|serde_json::json!({"lid":lid,"text":book.text(lid,None).unwrap()})).collect();
    println!("{}",serde_json::json!({"results":results,"text_reads":text_reads,"elapsed_ms":start.elapsed().as_millis()}));
    Ok(())
}
