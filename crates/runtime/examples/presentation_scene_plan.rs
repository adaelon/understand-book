//! Replay just the production presentation coach for a saved semantic-eval input.
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dirs: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if dirs.is_empty() {
        return Err("Pass one or more case directories containing input.json".into());
    }
    let config = runtime::ProviderConfig::from_env().map_err(|e| e.message)?;
    let model = config.model.clone();
    let adapter = runtime::ProviderRegistry::adapter_from_config(config);
    let mut failed = false;
    for dir in dirs {
        let input: Value = serde_json::from_slice(&fs::read(dir.join("input.json"))?)?;
        let request = json!({
            "authoring_surface": "presentation",
            "current_user_request": input["message"],
            "current_source_evidence": [{"locator": "1.1", "content": input["source"]}]
        });
        let (plan, usage) = runtime::tutor_coach::presentation::execute(adapter.as_ref(), request);
        let result = match plan {
            Ok(plan) => json!({"model": model, "usage": usage, "plan": plan}),
            Err(e) => {
                failed = true;
                json!({"model": model, "usage": usage, "error": e.message})
            }
        };
        fs::write(dir.join("coach.json"), serde_json::to_vec_pretty(&result)?)?;
        println!(
            "{}: {}",
            dir.display(),
            if result.get("plan").is_some() {
                "plan"
            } else {
                "unavailable"
            }
        );
    }
    if failed {
        return Err("See coach.json in the affected case directories".into());
    }
    Ok(())
}
