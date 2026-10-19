//! VI0c probe through the production request planner/adapter. No book is opened.
use runtime::{AgentRequestPlan, Message, ProviderConfig, ProviderRegistry};
use runtime::presentation_author::PreviewImage;
use runtime::provider_stream::ModelDelta;
use serde_json::{json, Value};
use std::{env, fs, path::Path, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    let input: Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let config = ProviderConfig::from_env().map_err(|e| e.message)?;
    let model = config.model.clone();
    let adapter = ProviderRegistry::adapter_from_config(config);
    let mut plan = AgentRequestPlan::for_ad_hoc(adapter.model_runtime_profile(),
        &[Message::user(input["question"].as_str().ok_or("missing question")?)], &[]);
    plan.output_token_limit = Some(2048);
    for (index, image) in input["images"].as_array().ok_or("missing images")?.iter().enumerate() {
        plan.preview_images.push(PreviewImage {
            caption: format!("Image {}", index + 1),
            png_base64: image.as_str().ok_or("invalid image")?.to_string(),
            candidate_id: None, environment_name: None,
        });
    }
    let mut request = None;
    let mut response = None;
    let mut usage = None;
    let start = Instant::now();
    let result = adapter.chat_observed(&plan, &mut |delta| match delta {
        ModelDelta::Request(value) => request = Some((*value).clone()),
        ModelDelta::Response(value) => response = Some((*value).clone()),
        ModelDelta::Usage(value) => usage = Some(value),
        _ => {},
    });
    if let Some(value) = request.as_ref() {
        fs::write(out.join("request.json"), serde_json::to_vec(value)?)?;
    }
    if let Some(value) = response.as_ref() {
        fs::write(out.join("response.json"), serde_json::to_vec_pretty(value)?)?;
    }
    let report = json!({"model": model, "elapsed_ms": start.elapsed().as_millis(),
        "request_json_bytes": request.as_ref().map(|v| serde_json::to_vec(v).unwrap().len()),
        "usage": usage, "answer": result.as_ref().ok().and_then(|v| v.text.as_ref()),
        "error": result.as_ref().err().map(|e| &e.message) });
    fs::write(out.join("run.json"), serde_json::to_vec_pretty(&report)?)?;
    if let Err(error) = result { return Err(error.message.into()); }
    Ok(())
}
