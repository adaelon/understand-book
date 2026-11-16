//! Validate a prepared music workspace against the Reader's authoritative contract.
use base_schema::MusicSourceManifest;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for filename in std::env::args().skip(1) {
        let manifest: MusicSourceManifest = serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(filename)?))?;
        manifest.validate()?;
        println!("{}: {} score objects, {} occurrences, {} events", manifest.book_id,
            manifest.score_objects.len(), manifest.occurrences.len(), manifest.events.len());
    }
    Ok(())
}
