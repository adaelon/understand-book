//! VI5 replay using the production published-frame reader and main-model transport.
use base64::Engine;
use runtime::{
    provider_stream::ModelDelta, AgentRequestPlan, Message, ProviderConfig, ProviderRegistry,
};
use serde_json::{json, Value};
use std::{env, fs, path::Path, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    let directory = Path::new(&args[1]);
    let input: Value = serde_json::from_slice(&fs::read(&args[2])?)?;
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let publication: server::published_library::PublicationManifest =
        serde_json::from_slice(&fs::read(directory.join("publication.json"))?)?;
    let book = read_tools::Book::load(directory.to_str().unwrap())?;
    let start = Instant::now();
    let batch = server::video_frames::read_frames(
        &book,
        directory,
        &publication.reference,
        "vi5-probe",
        &out.parent().unwrap().join("cache"),
        serde_json::from_value(input["read"].clone())?,
        &Default::default(),
    )
    .map_err(|e| e.message)?;
    let projection = batch.projection();
    for (i, image) in batch.images.iter().enumerate() {
        fs::write(
            out.join(format!("frame-{i}.png")),
            base64::engine::general_purpose::STANDARD.decode(&image.png_base64)?,
        )?;
    }
    let read_ms = start.elapsed().as_millis();
    fs::write(
        out.join("read.json"),
        serde_json::to_vec_pretty(&projection)?,
    )?;
    let config = ProviderConfig::from_env().map_err(|e| e.message)?;
    let model = config.model.clone();
    let adapter = ProviderRegistry::adapter_from_config(config);
    let mut plan = AgentRequestPlan::for_ad_hoc(
        adapter.model_runtime_profile(),
        &[
            Message::user(input["question"].as_str().unwrap()),
            Message::user(projection.to_string()),
        ],
        &[],
    );
    plan.output_token_limit = Some(2048);
    runtime::video::attach(&mut plan, vec![batch]);
    let mut request = None;
    let mut response = None;
    let mut usage = None;
    let result = adapter.chat_observed(&plan, &mut |d| match d {
        ModelDelta::Request(v) => request = Some((*v).clone()),
        ModelDelta::Response(v) => response = Some((*v).clone()),
        ModelDelta::Usage(v) => usage = Some(v),
        _ => {}
    });
    let mut ledger = vec![];
    if result.is_ok() {
        runtime::video::accept(&plan, &mut ledger);
    }
    if let Some(v) = request {
        fs::write(out.join("request.json"), serde_json::to_vec(&v)?)?;
    }
    if let Some(v) = response {
        fs::write(out.join("response.json"), serde_json::to_vec_pretty(&v)?)?;
    }
    fs::write(
        out.join("observations.json"),
        serde_json::to_vec_pretty(&ledger)?,
    )?;
    fs::write(
        out.join("run.json"),
        serde_json::to_vec_pretty(
            &json!({"model":model,"read_ms":read_ms,"elapsed_ms":start.elapsed().as_millis(),"usage":usage,"answer":result.as_ref().ok().and_then(|r|r.text.as_ref()),"error":result.as_ref().err().map(|e|&e.message),"publication":publication.reference}),
        )?,
    )?;
    result.map_err(|e| e.message)?;
    Ok(())
}
