//! Installed maintenance entry for original-audio correction evidence.
use std::{env, fs, path::Path, time::Instant};
fn main() {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let args: Vec<_> = env::args().skip(1).collect();
        if args.len() != 3 { return Err("Usage: video-audio-review PUBLISHED_DIRECTORY REQUEST_JSON OUTPUT_JSON".into()); }
        let directory=Path::new(&args[0]);
        let request=serde_json::from_slice(&fs::read(&args[1])?)?;
        let publication:server::published_library::PublicationManifest=serde_json::from_slice(&fs::read(directory.join("publication.json"))?)?;
        let book=read_tools::Book::load(directory.to_str().ok_or("Invalid publication path")?)?;
        let start=Instant::now();
        let review=server::video_audio::review_audio(&book,directory,&publication.reference,"video-maintenance",request,&Default::default(),
            &server::video_audio::AudioConfig::from_env().map_err(|e|e.message)?).map_err(|e|e.message)?;
        fs::write(&args[2],serde_json::to_vec_pretty(&serde_json::json!({"review":review,"elapsed_ms":start.elapsed().as_millis()}))?)?;
        Ok(())
    })();
    if let Err(error)=result { eprintln!("{error}"); std::process::exit(1); }
}
