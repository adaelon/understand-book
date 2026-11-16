//! Inspect a published music read and its source sizes without calling a model.
use serde_json::{json, Value};
use std::{path::Path, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let directory = args.next().ok_or("provide a music publication directory")?;
    let request = serde_json::from_str(&args.next().unwrap_or_else(|| "{}".into()))?;
    let book = read_tools::Book::load(&directory)?;
    let started = Instant::now();
    let manifest = book
        .load_music_source(Path::new(&directory))
        .map_err(|e| e.message)?;
    let load_ms = started.elapsed().as_secs_f64() * 1000.0;
    let publication: Value = serde_json::from_slice(&std::fs::read(
        Path::new(&directory).join("publication.json"),
    )?)?;
    let reference = serde_json::from_value(publication["reference"].clone())?;
    let started = Instant::now();
    let result = read_tools::music::read(&manifest, &reference, &request).map_err(|e| e.message)?;
    eprintln!(
        "{}",
        json!({"load_ms":load_ms,"read_ms":started.elapsed().as_secs_f64()*1000.0,
        "notation_objects":manifest.score_objects.iter().filter(|n|n.notation.is_some()).count(),
        "hidden_muted_objects":manifest.score_objects.iter().filter(|n|n.is_hidden_muted()).count(),
        "source_bytes":result.sources.iter().map(|s|serde_json::to_vec(s).unwrap().len()).collect::<Vec<_>>()})
    );
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
