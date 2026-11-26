//! Shared model configuration used by local and managed Reader hosts.
//! Credentials remain in the host's store; only `redacted` is a UI response.
use crate::{AgentReasoningEffort, ModelRuntimeCatalog, ModelRuntimeProfile, ProviderConfig, ProviderMode};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ApiProtocol {
    #[default]
    LegacyAuto,
    OpenaiChatCompletions,
    OpenaiResponses,
    AnthropicMessages,
    GeminiGenerateContent,
    GeminiInteractions,
    OllamaChat,
    CohereV2Chat,
    BedrockConverse,
    DeepseekMessages,
}

impl ApiProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LegacyAuto => "legacy_auto",
            Self::OpenaiChatCompletions => "openai_chat_completions",
            Self::OpenaiResponses => "openai_responses",
            Self::AnthropicMessages => "anthropic_messages",
            Self::GeminiGenerateContent => "gemini_generate_content",
            Self::GeminiInteractions => "gemini_interactions",
            Self::OllamaChat => "ollama_chat",
            Self::CohereV2Chat => "cohere_v2_chat",
            Self::BedrockConverse => "bedrock_converse",
            Self::DeepseekMessages => "deepseek_messages",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    #[default]
    Bearer,
    None,
    ApiKey,
    GoogleAdc,
    AzureDeveloperTools,
    AzureManagedIdentity,
    AzureWorkloadIdentity,
}

impl AuthMethod {
    pub fn requires_api_key(self) -> bool { matches!(self, Self::Bearer | Self::ApiKey) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OutputTokenParameter { MaxTokens, MaxCompletionTokens }

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct GenerationParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub output_token_parameter: Option<OutputTokenParameter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub temperature: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct ModelCapabilities {
    #[ts(optional = nullable)]
    pub streaming: Option<bool>,
    #[ts(optional = nullable)]
    pub streaming_tools: Option<bool>,
    #[ts(optional = nullable)]
    pub reasoning_effort: Option<bool>,
    #[ts(optional = nullable)]
    pub native_tools: Option<bool>,
    #[ts(optional = nullable)]
    pub parallel_tools: Option<bool>,
    #[ts(optional = nullable)]
    pub json_object: Option<bool>,
    #[ts(optional = nullable)]
    pub json_schema: Option<bool>,
    #[ts(optional = nullable)]
    pub images: Option<bool>,
    #[ts(optional = nullable)]
    pub continuation: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelBinding {
    pub service_id: String,
    pub model_config_id: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderOptions {
    /// Distinguishes an explicit high effort from the same value inherited by default.
    pub explicit_agent_reasoning_effort: bool,
    pub api_protocol: ApiProtocol,
    pub auth: AuthMethod,
    pub headers: BTreeMap<String, String>,
    pub binding: Option<ModelBinding>,
    pub runtime_profile: Option<ModelRuntimeProfile>,
    pub output_token_limit: Option<u32>,
    pub capabilities: ModelCapabilities,
    pub deployment: crate::cloud_provider::Deployment,
    pub generation: GenerationParameters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelPurpose { Agent, Teaching, Background }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ModelService {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub auth: AuthMethod,
    #[serde(default)]
    pub api_key: String,
    /// Write-only instruction; an absent/blank key otherwise preserves the old key.
    #[serde(default, skip_serializing)]
    pub clear_api_key: bool,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub deployment: crate::cloud_provider::Deployment,
}

fn native_mode() -> String { "native".into() }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ModelDefinition {
    pub id: String,
    pub name: String,
    pub service_id: String,
    pub model_id: String,
    pub api_protocol: ApiProtocol,
    #[serde(default = "native_mode")]
    pub mode: String,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub context_window_tokens: Option<u32>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub output_token_limit: Option<u32>,
    #[serde(default)]
    pub capabilities: ModelCapabilities,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub agent_reasoning_effort: Option<String>,
    #[serde(default)]
    pub generation: GenerationParameters,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PurposeBindings {
    pub default_model: String,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub agent: Option<String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub teaching: Option<String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub background: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ModelSettings {
    #[serde(default)]
    #[ts(type = "number")]
    pub revision: u64,
    pub services: Vec<ModelService>,
    pub models: Vec<ModelDefinition>,
    pub bindings: PurposeBindings,
}

impl ModelSettings {
    /// Preserve a resolved configuration when an embedding host only supplies one model.
    /// Full catalogs should be loaded directly so their other models and purposes survive.
    pub fn from_resolved(config: &ProviderConfig) -> Self {
        let mut settings = Self::from_legacy(config);
        let service = &mut settings.services[0];
        let model = &mut settings.models[0];
        service.auth = config.options.auth;
        service.headers = config.options.headers.clone();
        service.deployment = config.options.deployment.clone();
        model.api_protocol = config.options.api_protocol;
        model.capabilities = config.options.capabilities.clone();
        // ProviderConfig always has an internal default effort, even when the
        // selected protocol does not send a reasoning parameter.
        if !matches!(model.api_protocol, ApiProtocol::LegacyAuto | ApiProtocol::DeepseekMessages)
            && model.capabilities.reasoning_effort != Some(true) {
            model.agent_reasoning_effort = None;
        }
        model.context_window_tokens = config.options.runtime_profile.as_ref().map(|profile| profile.context_window_tokens);
        model.output_token_limit = config.options.output_token_limit.or_else(||
            config.options.runtime_profile.as_ref().map(|profile| profile.output_reserve_tokens));
        model.generation = config.options.generation.clone();
        if let Some(binding) = &config.options.binding {
            settings.revision = binding.revision;
            service.id = binding.service_id.clone();
            model.service_id = binding.service_id.clone();
            model.id = binding.model_config_id.clone();
            settings.bindings.default_model = binding.model_config_id.clone();
        }
        settings
    }

    /// An explicit file is authoritative; invalid paths, JSON or model IDs never
    /// fall through to the legacy key/base/model environment variables.
    pub fn from_environment_file(path: Option<&str>, selected_id: Option<&str>) -> Result<Option<Self>, String> {
        let Some(path) = path else {
            return if selected_id.is_some() {
                Err("UNDERSTAND_BOOK_MODEL_CONFIG_ID requires UNDERSTAND_BOOK_MODEL_SETTINGS".into())
            } else { Ok(None) };
        };
        let bytes = std::fs::read(path).map_err(|error| format!("Cannot read UNDERSTAND_BOOK_MODEL_SETTINGS: {error}"))?;
        let mut settings: Self = serde_json::from_slice(&bytes).map_err(|error| format!("Invalid model settings JSON: {error}"))?;
        if let Some(id) = selected_id { settings.bindings.agent = Some(id.into()); }
        settings.validate()?;
        Ok(Some(settings))
    }

    pub fn from_env() -> Result<Option<Self>, String> {
        dotenvy::dotenv().ok();
        Self::from_environment_values(|key| std::env::var(key).ok())
    }

    fn from_environment_values(mut get: impl FnMut(&str) -> Option<String>) -> Result<Option<Self>, String> {
        let path = get("UNDERSTAND_BOOK_MODEL_SETTINGS");
        let selected = get("UNDERSTAND_BOOK_MODEL_CONFIG_ID");
        if path.is_some() || selected.is_some() {
            return Self::from_environment_file(path.as_deref(), selected.as_deref());
        }
        Ok(ProviderConfig::from_getter(get).ok().as_ref().map(Self::from_legacy))
    }

    /// Deterministic upgrade of the one supported pre-catalog configuration.
    /// Legacy auto routing keeps the original DeepSeek capability probe behavior.
    pub fn from_legacy(config: &ProviderConfig) -> Self {
        Self {
            revision: 0,
            services: vec![ModelService {
                id: "default-service".into(), name: "Default service".into(),
                base_url: config.base_url.clone(), auth: if config.api_key.is_empty() { AuthMethod::None } else { AuthMethod::Bearer },
                api_key: config.api_key.clone(), clear_api_key: false, headers: BTreeMap::new(), deployment: Default::default(),
            }],
            models: vec![ModelDefinition {
                id: "default-model".into(), name: config.model.clone(),
                service_id: "default-service".into(), model_id: config.model.clone(),
                api_protocol: ApiProtocol::LegacyAuto, mode: config.mode.as_str().into(),
                context_window_tokens: None, output_token_limit: None,
                capabilities: ModelCapabilities::default(),
                agent_reasoning_effort: (!crate::deepseek_messages::official_flash_route(&config.base_url, &config.model)
                    || config.options.explicit_agent_reasoning_effort || config.agent_reasoning_effort != AgentReasoningEffort::default())
                    .then(|| config.agent_reasoning_effort.as_str().into()),
                generation: GenerationParameters::default(),
            }],
            bindings: PurposeBindings {
                default_model: "default-model".into(), agent: None, teaching: None, background: None,
            },
        }
    }

    /// Apply a full catalog edit, retaining credentials by stable service ID.
    /// This only prepares a candidate; callers persist successfully before applying it.
    pub fn prepare_update(mut self, existing: Option<&Self>) -> Result<Self, String> {
        let previous_revision = existing.map_or(0, |settings| settings.revision);
        if self.revision != previous_revision {
            return Err("Model settings changed; reload them before saving".into());
        }
        for service in &mut self.services {
            service.name = service.name.trim().into();
            service.base_url = service.base_url.trim().trim_end_matches('/').into();
            service.api_key = service.api_key.trim().into();
            if service.clear_api_key {
                service.api_key.clear();
            } else if service.api_key.is_empty() {
                if let Some(previous) = existing.and_then(|settings| settings.services.iter().find(|s| s.id == service.id)) {
                    service.api_key = previous.api_key.clone();
                }
            }
            service.clear_api_key = false;
        }
        for model in &mut self.models {
            model.name = model.name.trim().into();
            model.model_id = model.model_id.trim().into();
            model.mode = model.mode.trim().to_ascii_lowercase();
        }
        self.revision = previous_revision.checked_add(1).ok_or("Model settings revision is exhausted")?;
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut service_ids = BTreeSet::new();
        for service in &self.services {
            valid_id(&service.id, "service")?;
            if !service_ids.insert(service.id.as_str()) { return Err(format!("Duplicate service ID: {}", service.id)); }
            if service.name.trim().is_empty() { return Err(format!("Service {} needs a name", service.id)); }
            let url = url::Url::parse(&service.base_url).map_err(|_| format!("Service {} has an invalid base_url", service.id))?;
            if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
                return Err(format!("Service {} base_url must be an HTTP(S) URL without credentials or fragment", service.id));
            }
            if service.auth.requires_api_key() && service.api_key.trim().is_empty() {
                return Err(format!("Service {} requires an API key; select no authentication for a local service", service.id));
            }
            for (name, value) in &service.headers {
                let lower = name.to_ascii_lowercase();
                if matches!(lower.as_str(), "authorization" | "proxy-authorization" | "x-api-key" | "x-goog-api-key" | "api-key" | "content-type" | "content-length" | "host" | "anthropic-version" | "anthropic-beta") {
                    return Err(format!("Service {} header {name} is managed by the protocol adapter", service.id));
                }
                if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)) || value.contains(['\r', '\n']) {
                    return Err(format!("Service {} has an invalid extra header", service.id));
                }
            }
        }
        let mut model_ids = BTreeSet::new();
        for model in &self.models {
            valid_id(&model.id, "model")?;
            if !model_ids.insert(model.id.as_str()) { return Err(format!("Duplicate model configuration ID: {}", model.id)); }
            if !service_ids.contains(model.service_id.as_str()) { return Err(format!("Model {} refers to missing service {}", model.id, model.service_id)); }
            let service = self.services.iter().find(|s| s.id == model.service_id).expect("service reference checked");
            service.deployment.validate(model.api_protocol, service.auth, &service.base_url)?;
            if model.name.trim().is_empty() || model.model_id.trim().is_empty() { return Err(format!("Model {} requires a name and model_id", model.id)); }
            let mode = parse_mode(&model.mode)?;
            if model.generation.temperature.is_some_and(|value| !value.is_finite() || !(0.0..=2.0).contains(&value)) {
                return Err(format!("Model {} temperature must be between 0 and 2", model.id));
            }
            if model.generation.output_token_parameter.is_some() && !matches!(model.api_protocol, ApiProtocol::LegacyAuto | ApiProtocol::OpenaiChatCompletions) {
                return Err(format!("Model {} output_token_parameter only applies to Chat Completions", model.id));
            }
            if mode == ProviderMode::ReAct && model.api_protocol == ApiProtocol::DeepseekMessages {
                return Err(format!("Model {}: DeepSeek Messages requires native tool mode", model.id));
            }
            if let Some(effort) = &model.agent_reasoning_effort {
                parse_effort(effort)?;
                if !matches!(model.api_protocol, ApiProtocol::LegacyAuto | ApiProtocol::DeepseekMessages) && model.capabilities.reasoning_effort != Some(true) {
                    return Err(format!("Model {}: agent_reasoning_effort requires the reasoning_effort capability", model.id));
                }
            }
            if model.context_window_tokens == Some(0) || model.output_token_limit == Some(0) {
                return Err(format!("Model {} token limits must be positive", model.id));
            }
            let profile = self.profile(model)?;
            if profile.output_reserve_tokens.saturating_add(profile.safety_margin_tokens) >= profile.context_window_tokens {
                return Err(format!("Model {} context window must exceed output limit and safety margin", model.id));
            }
            if model.capabilities.parallel_tools == Some(true) && model.capabilities.native_tools != Some(true) {
                return Err(format!("Model {} parallel_tools requires native_tools", model.id));
            }
        }
        for (purpose, id) in [
            ("default", self.bindings.default_model.as_str()),
            ("agent", self.selected_model(ModelPurpose::Agent)),
            ("teaching", self.selected_model(ModelPurpose::Teaching)),
            ("background", self.selected_model(ModelPurpose::Background)),
        ] {
            if !model_ids.contains(id) { return Err(format!("The {purpose} binding refers to missing model {id}")); }
        }
        let agent = self.models.iter().find(|m| m.id == self.selected_model(ModelPurpose::Agent)).expect("binding checked");
        if agent.api_protocol != ApiProtocol::LegacyAuto && agent.mode == "native" && agent.capabilities.native_tools != Some(true) {
            return Err(format!("Agent model {} must explicitly support native tools", agent.id));
        }
        Ok(())
    }

    pub fn selected_model(&self, purpose: ModelPurpose) -> &str {
        let selected = match purpose {
            ModelPurpose::Agent => &self.bindings.agent,
            ModelPurpose::Teaching => &self.bindings.teaching,
            ModelPurpose::Background => &self.bindings.background,
        };
        selected.as_deref().unwrap_or(&self.bindings.default_model)
    }

    pub fn resolve(&self, purpose: ModelPurpose) -> Result<ProviderConfig, String> {
        self.resolve_model(self.selected_model(purpose))
    }

    pub fn resolve_model(&self, id: &str) -> Result<ProviderConfig, String> {
        self.validate()?;
        let model = self.models.iter().find(|m| m.id == id).ok_or_else(|| format!("Model configuration {id} is not available"))?;
        let service = self.services.iter().find(|s| s.id == model.service_id).expect("validated reference");
        let overrides = model.api_protocol != ApiProtocol::LegacyAuto || model.context_window_tokens.is_some() || model.output_token_limit.is_some() || model.capabilities != ModelCapabilities::default();
        Ok(ProviderConfig {
            mode: parse_mode(&model.mode)?, api_key: service.api_key.clone(),
            base_url: service.base_url.clone(), model: model.model_id.clone(),
            agent_reasoning_effort: model.agent_reasoning_effort.as_deref().map(parse_effort).transpose()?.unwrap_or_default(),
            options: ProviderOptions {
                explicit_agent_reasoning_effort: model.agent_reasoning_effort.is_some(),
                api_protocol: model.api_protocol, auth: service.auth, headers: service.headers.clone(),
                binding: Some(ModelBinding { service_id: service.id.clone(), model_config_id: model.id.clone(), revision: self.revision }),
                runtime_profile: if overrides { Some(self.profile(model)?) } else { None },
                output_token_limit: model.output_token_limit,
                capabilities: model.capabilities.clone(),
                deployment: service.deployment.clone(),
                generation: model.generation.clone(),
            },
        })
    }

    fn profile(&self, model: &ModelDefinition) -> Result<ModelRuntimeProfile, String> {
        let mut profile = ModelRuntimeCatalog::default().resolve(&model.model_id, parse_mode(&model.mode)?.into(), None);
        let customized = model.api_protocol != ApiProtocol::LegacyAuto || model.context_window_tokens.is_some() || model.output_token_limit.is_some() || model.capabilities != ModelCapabilities::default();
        if let Some(window) = model.context_window_tokens {
            profile.context_window_tokens = window;
            profile.safety_margin_tokens = profile.safety_margin_tokens.min(window / 16);
            if model.output_token_limit.is_none() {
                profile.output_reserve_tokens = profile.output_reserve_tokens.min(window / 4);
            }
        }
        if let Some(output) = model.output_token_limit { profile.output_reserve_tokens = output; }
        if model.api_protocol != ApiProtocol::LegacyAuto {
            profile.supports_native_tools = model.capabilities.native_tools.unwrap_or(false);
            profile.supports_parallel_tools = model.capabilities.parallel_tools.unwrap_or(false);
            profile.supports_continuation = model.capabilities.continuation.unwrap_or(false);
        } else {
            if let Some(value) = model.capabilities.native_tools { profile.supports_native_tools = value; }
            if let Some(value) = model.capabilities.parallel_tools { profile.supports_parallel_tools = value; }
            if let Some(value) = model.capabilities.continuation { profile.supports_continuation = value; }
        }
        if customized { profile.resolution = crate::model_runtime::ModelProfileResolution::ExplicitOverride; }
        Ok(profile)
    }

    pub fn redacted(&self) -> serde_json::Value {
        let mut view = serde_json::to_value(self).expect("model settings serialize");
        for (value, service) in view["services"].as_array_mut().expect("services array").iter_mut().zip(&self.services) {
            let fields = value.as_object_mut().expect("service object");
            fields.remove("api_key");
            fields.insert("api_key_configured".into(), serde_json::Value::Bool(!service.api_key.is_empty()));
        }
        view
    }
}

fn valid_id(id: &str, kind: &str) -> Result<(), String> {
    if id.trim().is_empty() || id.trim() != id { return Err(format!("The {kind} ID must be nonempty and have no surrounding whitespace")); }
    Ok(())
}

fn parse_mode(mode: &str) -> Result<ProviderMode, String> {
    match mode { "native" => Ok(ProviderMode::Native), "react" => Ok(ProviderMode::ReAct), _ => Err("Model mode must be native or react".into()) }
}

fn parse_effort(value: &str) -> Result<AgentReasoningEffort, String> {
    match value {
        "low" => Ok(AgentReasoningEffort::Low), "high" => Ok(AgentReasoningEffort::High), "max" => Ok(AgentReasoningEffort::Max),
        _ => Err("Agent reasoning effort must be low, high or max".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_model_settings_types() {
        ModelSettings::export_all_to("../../packages/web/src/generated").unwrap();
    }
    fn legacy() -> ModelSettings {
        ModelSettings::from_legacy(&ProviderConfig::from_values("native", "saved-secret", "https://model.example/v1", "deepseek-v4-flash").unwrap())
    }

    #[test]
    fn model_settings_upgrade_preserves_legacy_routing_and_profile() {
        let settings = legacy();
        let config = settings.resolve(ModelPurpose::Agent).unwrap();
        assert_eq!(config.options.api_protocol, ApiProtocol::LegacyAuto);
        assert_eq!(config.api_key, "saved-secret");
        assert!(config.options.runtime_profile.is_none());
        assert_eq!(config.options.binding.unwrap().model_config_id, "default-model");
    }

    #[test]
    fn model_settings_redacted_round_trip_preserves_key_and_advances_revision() {
        let original = legacy();
        let view = original.redacted();
        assert!(!view.to_string().contains("saved-secret"));
        assert_eq!(view["services"][0]["api_key_configured"], true);
        let input: ModelSettings = serde_json::from_value(view).unwrap();
        let next = input.prepare_update(Some(&original)).unwrap();
        assert_eq!(next.revision, 1);
        assert_eq!(next.services[0].api_key, "saved-secret");
        assert!(original.clone().prepare_update(Some(&next)).is_err());
    }

    #[test]
    fn model_settings_explicit_clear_and_no_auth_are_supported() {
        let original = legacy();
        let mut input = original.clone();
        input.services[0].auth = AuthMethod::None;
        input.services[0].clear_api_key = true;
        let next = input.prepare_update(Some(&original)).unwrap();
        let config = next.resolve(ModelPurpose::Agent).unwrap();
        assert_eq!(config.options.auth, AuthMethod::None);
        assert!(config.api_key.is_empty());
        assert!(!next.redacted()["services"][0].get("clear_api_key").is_some());
        let mut invalid = original.clone();
        invalid.services[0].clear_api_key = true;
        assert!(invalid.prepare_update(Some(&original)).is_err());
    }

    #[test]
    fn model_settings_resolved_projection_retains_protocol_auth_and_generation() {
        let mut settings=legacy();
        settings.revision=7;
        settings.services[0].auth=AuthMethod::None;
        settings.services[0].api_key.clear();
        settings.services[0].headers.insert("x-project".into(),"local-reader".into());
        settings.models[0].api_protocol=ApiProtocol::OllamaChat;
        settings.models[0].agent_reasoning_effort=None;
        settings.models[0].capabilities.native_tools=Some(true);
        settings.models[0].context_window_tokens=Some(8192);
        settings.models[0].output_token_limit=Some(512);
        settings.models[0].generation.temperature=Some(0.0);
        let config=settings.resolve(ModelPurpose::Agent).unwrap();
        let restored=ModelSettings::from_resolved(&config);
        assert_eq!(restored.models[0].agent_reasoning_effort,None);
        let resolved=restored.resolve(ModelPurpose::Agent).unwrap();
        assert_eq!(resolved.options,config.options);
        assert_eq!(resolved.api_key,"");
        assert_eq!(restored.revision,7);
        assert_eq!(restored.models[0].api_protocol,ApiProtocol::OllamaChat);
    }

    #[test]
    fn model_settings_explicit_environment_errors_never_fall_back_to_legacy_credentials() {
        let missing=std::env::temp_dir().join(format!("missing-model-settings-{}-{}.json",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let values=BTreeMap::from([
            ("UNDERSTAND_BOOK_MODEL_SETTINGS",missing.to_string_lossy().into_owned()),
            ("OPENCODE_API_KEY","old-key".into()),
            ("OPENCODE_BASE_URL","https://old.example/v1".into()),
            ("FLUID_LLM_MODEL","old-model".into()),
        ]);
        assert!(ModelSettings::from_environment_values(|key|values.get(key).cloned()).unwrap_err().contains("UNDERSTAND_BOOK_MODEL_SETTINGS"));
        assert!(ModelSettings::from_environment_file(None,Some("selected")).is_err());
    }

    #[test]
    fn model_settings_keys_do_not_follow_a_different_service_id() {
        let original = legacy();
        let mut input: ModelSettings = serde_json::from_value(original.redacted()).unwrap();
        input.services[0].id = "different".into();
        input.models[0].service_id = "different".into();
        assert!(input.prepare_update(Some(&original)).unwrap_err().contains("API key"));
    }

    #[test]
    fn model_settings_binding_resolves_distinct_models_and_rejects_deleted_references() {
        let mut settings = legacy();
        let mut teacher = settings.models[0].clone();
        teacher.id = "teacher".into(); teacher.model_id = "teacher-model".into();
        settings.models.push(teacher);
        settings.bindings.teaching = Some("teacher".into());
        assert_eq!(settings.resolve(ModelPurpose::Teaching).unwrap().model, "teacher-model");
        assert_eq!(settings.resolve(ModelPurpose::Background).unwrap().model, "deepseek-v4-flash");
        settings.models.pop();
        assert!(settings.validate().unwrap_err().contains("missing model teacher"));
        settings.bindings.teaching = None;
        settings.services.clear();
        assert!(settings.validate().unwrap_err().contains("missing service"));
    }

    #[test]
    fn model_settings_unknown_tools_are_not_promoted_to_supported() {
        let mut settings = legacy();
        settings.models[0].api_protocol = ApiProtocol::OpenaiChatCompletions;
        settings.models[0].agent_reasoning_effort = None;
        assert!(settings.validate().unwrap_err().contains("native tools"));
        settings.models[0].capabilities.native_tools = Some(true);
        settings.models[0].context_window_tokens = Some(32_768);
        settings.models[0].output_token_limit = Some(4_096);
        let config = settings.resolve(ModelPurpose::Agent).unwrap();
        assert_eq!(config.options.output_token_limit, Some(4_096));
        let profile = config.options.runtime_profile.unwrap();
        assert_eq!(profile.context_window_tokens, 32_768);
        assert_eq!(profile.output_reserve_tokens, 4_096);
        assert!(profile.supports_native_tools);
        assert!(!profile.supports_parallel_tools);
        settings.models[0].output_token_limit = Some(40_000);
        assert!(settings.validate().is_err());
    }

    #[test]
    fn model_settings_explicit_reasoning_requires_a_declared_capability() {
        let mut settings = legacy();
        settings.models[0].api_protocol = ApiProtocol::OpenaiResponses;
        settings.models[0].capabilities.native_tools = Some(true);
        assert!(settings.validate().unwrap_err().contains("reasoning_effort"));
        settings.models[0].capabilities.reasoning_effort = Some(true);
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn model_settings_generation_parameters_validate_and_resolve_without_guessing() {
        let mut settings = legacy();
        settings.models[0].generation = GenerationParameters { output_token_parameter: Some(OutputTokenParameter::MaxCompletionTokens), temperature: Some(0.0) };
        assert_eq!(settings.resolve(ModelPurpose::Agent).unwrap().options.generation.temperature, Some(0.0));
        settings.models[0].generation.temperature = Some(3.0);
        assert!(settings.validate().unwrap_err().contains("temperature"));
        settings.models[0].generation.temperature = None;
        settings.models[0].api_protocol = ApiProtocol::OllamaChat;
        assert!(settings.validate().unwrap_err().contains("only applies to Chat Completions"));
    }

    #[test]
    fn model_settings_rejects_unknown_protocol_and_reserved_headers() {
        let mut view = legacy().redacted();
        view["models"][0]["api_protocol"] = serde_json::json!("not_a_protocol");
        assert!(serde_json::from_value::<ModelSettings>(view).is_err());
        let mut settings = legacy();
        settings.services[0].headers.insert("Authorization".into(), "secret".into());
        assert!(settings.validate().unwrap_err().contains("managed"));
        settings.services[0].headers.clear();
        settings.services[0].base_url = "https://model.example/v1?api-version=2026-01-01".into();
        assert!(settings.validate().is_ok());
    }
}
