//! Host-owned persistence and the shared, redacted model-settings API.
use runtime::{model_settings::ModelSettings, AgentRequestPlan, Message, ProviderRegistry};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc, time::Duration};

pub type SaveModelSettings = Arc<dyn Fn(&ModelSettings) -> Result<(), String> + Send + Sync>;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelTestRequest {
    pub model_config_id: String,
    #[serde(default)]
    pub input: Option<ModelSettings>,
}

/// Resolve a draft using the same credential and validation rules as saving,
/// without advancing the persisted revision or applying the candidate.
pub fn prepare_test_settings(
    current: Option<ModelSettings>,
    input: Option<ModelSettings>,
) -> Result<ModelSettings, String> {
    match input {
        Some(input) => {
            let revision = input.revision;
            let mut prepared = input.prepare_update(current.as_ref())?;
            prepared.revision = revision;
            Ok(prepared)
        }
        None => current.ok_or_else(|| "Model is not configured".into()),
    }
}

pub fn status(settings: Option<&ModelSettings>, source: &str) -> Value {
    json!({"configured":settings.is_some(),"source":if settings.is_some(){source}else{"unconfigured"},
        "settings":settings.map(ModelSettings::redacted)})
}

pub fn read_file(path: &Path) -> Result<Option<ModelSettings>, String> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let settings: ModelSettings = serde_json::from_slice(&bytes)
                .map_err(|error| format!("Invalid model settings ({}): {error}", path.display()))?;
            settings.validate()?;
            Ok(Some(settings))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "Cannot read model settings ({}): {error}",
            path.display()
        )),
    }
}

pub fn write_file(path: &Path, settings: &ModelSettings) -> Result<(), String> {
    let parent = path.parent().ok_or("Model settings path has no parent")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("Cannot write model settings: {error}"))?;
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("Cannot replace model settings: {error}"))
}

/// An explicit, bounded text test. It does not assert tool or structured-output support.
pub fn test_model(settings: &ModelSettings, model_config_id: &str) -> Result<Value, String> {
    let config = settings.resolve_model(model_config_id)?;
    let adapter =
        ProviderRegistry::adapter_from_config_with_timeout(config, Duration::from_secs(30));
    adapter.set_model_purpose("connection_test");
    let mut request = AgentRequestPlan::for_ad_hoc(
        adapter.model_runtime_profile(),
        &[Message::user("Reply with the single word OK.")],
        &[],
    );
    request.output_token_limit = Some(64);
    let response = adapter.chat(&request).map_err(|error| error.message)?;
    if response
        .text
        .as_deref()
        .is_none_or(|text| text.trim().is_empty())
        || !response.tool_calls.is_empty()
    {
        return Err("The text test did not return a complete text answer".into());
    }
    Ok(
        json!({"ok":true,"model_config_id":model_config_id,"message":"文本生成测试通过；工具与结构化输出尚未测试。"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_settings_draft_test_merges_credentials_without_advancing_revision() {
        let config = runtime::ProviderConfig::from_values(
            "native",
            "retained-key",
            "https://models.example/v1",
            "saved-model",
        )
        .unwrap();
        let mut current = ModelSettings::from_legacy(&config);
        current.revision = 3;
        let mut draft = current.clone();
        draft.services[0].api_key.clear();
        draft.models[0].model_id = "draft-model".into();
        let prepared = prepare_test_settings(Some(current.clone()), Some(draft.clone())).unwrap();
        assert_eq!(prepared.revision, 3);
        assert_eq!(prepared.services[0].api_key, "retained-key");
        assert_eq!(prepared.models[0].model_id, "draft-model");
        assert_eq!(current.models[0].model_id, "saved-model");
        draft.revision = 2;
        assert!(prepare_test_settings(Some(current), Some(draft)).is_err());
        let first = prepare_test_settings(None, Some(ModelSettings::from_legacy(&config))).unwrap();
        assert_eq!(first.revision, 0);
    }
}
