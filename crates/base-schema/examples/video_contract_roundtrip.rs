//! TS -> Rust validation/serialization -> TS integration probe; no media or model calls.
#[path = "../tests/support/video_fixture.rs"]
mod video_fixture;
use std::io::{self, Read};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let fixture: video_fixture::Fixture = serde_json::from_str(&input)?;
    for case in &fixture.cases {
        case.validate()?;
    }
    println!("{}", serde_json::to_string(&fixture)?);
    Ok(())
}
