//! Replay real audio through the production VI15 reader, without a chat-model dependency.
use std::{env, fs, path::Path, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    let directory=Path::new(&args[1]);
    let request=serde_json::from_slice(&fs::read(&args[2])?)?;
    let publication:server::published_library::PublicationManifest=serde_json::from_slice(&fs::read(directory.join("publication.json"))?)?;
    let book=read_tools::Book::load(directory.to_str().unwrap())?;
    let start=Instant::now();
    let review=server::video_audio::review_audio(&book,directory,&publication.reference,"vi15-probe",request,&Default::default(),
        &server::video_audio::AudioConfig::from_env().map_err(|e|e.message)?).map_err(|e|e.message)?;
    let out=serde_json::json!({"review":review,"target":review.target(),"observation":review.observation("vi15-probe","audio-review"),"elapsed_ms":start.elapsed().as_millis()});
    fs::write(&args[3],serde_json::to_vec_pretty(&out)?)?;
    Ok(())
}
