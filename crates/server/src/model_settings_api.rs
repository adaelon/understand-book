//! Administrator-owned settings, including revisions required by queued runs.
use read_tools::ToolError;
use runtime::{model_settings::ModelSettings, AgentRequestPlan, Message, ProviderRegistry};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc, time::Duration};

pub(crate) fn invalid(message: impl Into<String>) -> ToolError {
    crate::user_storage_paths::error("MODEL_SETTINGS_INVALID", "validation", &message.into())
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct StoredModelSettings {
    pub settings: ModelSettings,
    #[serde(default)]
    pub history: Vec<ModelSettings>,
}
impl StoredModelSettings {
    pub fn load(path: &Path) -> Result<Option<Self>, ToolError> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(invalid(format!("Cannot read model settings: {error}"))),
        };
        // Plain catalogs are the documented file-based deployment input.
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))?;
        let stored: Self = if value.get("settings").is_some() {
            serde_json::from_value(value).map_err(|error| invalid(error.to_string()))?
        } else {
            Self {
                settings: serde_json::from_value(value)
                    .map_err(|error| invalid(error.to_string()))?,
                history: vec![],
            }
        };
        stored.settings.validate().map_err(invalid)?;
        for old in &stored.history {
            old.validate().map_err(invalid)?
        }
        Ok(Some(stored))
    }
    pub fn save(&self, path: &Path) -> Result<(), ToolError> {
        let temporary = path.with_extension("json.tmp");
        std::fs::write(
            &temporary,
            serde_json::to_vec_pretty(self).map_err(|error| invalid(error.to_string()))?,
        )
        .map_err(|error| invalid(format!("Cannot write model settings: {error}")))?;
        std::fs::rename(&temporary, path)
            .map_err(|error| invalid(format!("Cannot replace model settings: {error}")))
    }
}

pub(crate) fn test_model(
    settings: &ModelSettings,
    id: &str,
    spend: Arc<crate::model_spend_store::ModelSpendStore>,
) -> Result<Value, ToolError> {
    let config = settings.resolve_model(id).map_err(invalid)?;
    let adapter =
        ProviderRegistry::adapter_from_config_with_timeout(config, Duration::from_secs(30));
    let task_ref = format!("model-test-{}", uuid::Uuid::now_v7());
    adapter.set_spend_context(
        runtime::model_spend::ChargeScope::OperatorTask {
            task_ref: task_ref.clone(),
        },
        Some(spend.operator_port(task_ref, Default::default())),
    );
    adapter.set_model_purpose("connection_test");
    let mut request = AgentRequestPlan::for_ad_hoc(
        adapter.model_runtime_profile(),
        &[Message::user("Reply with the single word OK.")],
        &[],
    );
    request.output_token_limit = Some(64);
    let response = adapter
        .chat(&request)
        .map_err(|error| error.into_tool_error())?;
    if response
        .text
        .as_deref()
        .is_none_or(|text| text.trim().is_empty())
        || !response.tool_calls.is_empty()
    {
        return Err(invalid(
            "The text test did not return a complete text answer",
        ));
    }
    Ok(
        json!({"ok":true,"model_config_id":id,"message":"文本生成测试通过；工具与结构化输出尚未测试。"}),
    )
}
