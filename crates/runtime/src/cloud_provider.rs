//! Cloud deployment routes and official, refreshable credentials.
//! Credentials stay in the backend. Generative requests still use the shared send loop.
use crate::{
    model_settings::{ApiProtocol, AuthMethod},
    run_context::CancellationToken,
    AdapterError,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
    time::Duration,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Deployment {
    #[default]
    Direct,
    AzureV1,
    AzureDeployment {
        api_version: String,
    },
    Vertex {
        project: String,
        location: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        quota_project: Option<String>,
    },
    /// Bedrock's explicit API-key authentication, not AWS access-key/SigV4 credentials.
    Bedrock {
        region: String,
    },
}

impl Deployment {
    pub fn validate(
        &self,
        protocol: ApiProtocol,
        auth: AuthMethod,
        base_url: &str,
    ) -> Result<(), String> {
        let cloud_azure = matches!(
            auth,
            AuthMethod::AzureDeveloperTools
                | AuthMethod::AzureManagedIdentity
                | AuthMethod::AzureWorkloadIdentity
        );
        if cloud_azure && !matches!(self, Self::AzureV1 | Self::AzureDeployment { .. }) {
            return Err("Azure credentials require an Azure deployment".into());
        }
        if auth == AuthMethod::GoogleAdc && !matches!(self, Self::Vertex { .. }) {
            return Err("Google ADC requires a Vertex deployment".into());
        }
        match self {
            Self::Direct => {}
            Self::AzureV1 | Self::AzureDeployment { .. } => {
                if !matches!(
                    protocol,
                    ApiProtocol::OpenaiChatCompletions | ApiProtocol::OpenaiResponses
                ) {
                    return Err("Azure deployment requires Chat Completions or Responses".into());
                }
                if !cloud_azure && !matches!(auth, AuthMethod::ApiKey | AuthMethod::Bearer) {
                    return Err("Azure deployment requires an API key or Entra credential".into());
                }
                if let Self::AzureDeployment { api_version } = self {
                    if api_version.trim().is_empty() {
                        return Err("Azure api_version is required".into());
                    }
                    if protocol == ApiProtocol::OpenaiResponses {
                        return Err(
                            "Azure Responses uses the Azure v1 endpoint; select azure_v1".into(),
                        );
                    }
                }
            }
            Self::Vertex {
                project, location, ..
            } => {
                if project.trim().is_empty() || location.trim().is_empty() {
                    return Err("Vertex project and location are required".into());
                }
                if !matches!(
                    protocol,
                    ApiProtocol::GeminiGenerateContent
                        | ApiProtocol::GeminiInteractions
                        | ApiProtocol::AnthropicMessages
                ) {
                    return Err("Vertex requires Gemini GenerateContent, Interactions, or Anthropic Messages".into());
                }
                if auth != AuthMethod::GoogleAdc {
                    return Err("Vertex requires Google Application Default Credentials".into());
                }
                if protocol == ApiProtocol::GeminiInteractions
                    && !matches!(location.as_str(), "global" | "us" | "eu")
                {
                    return Err("Vertex Interactions location must be global, us, or eu".into());
                }
            }
            Self::Bedrock { region } => {
                if region.trim().is_empty() {
                    return Err("Bedrock region is required".into());
                }
                if protocol != ApiProtocol::BedrockConverse || auth != AuthMethod::Bearer {
                    return Err("Bedrock requires Converse and a Bedrock bearer API key".into());
                }
            }
        }
        if !matches!(self, Self::Direct) {
            let url = url::Url::parse(base_url).map_err(|e| format!("Cloud endpoint: {e}"))?;
            if url.fragment().is_some() || !url.username().is_empty() || url.password().is_some() {
                return Err("Cloud endpoint must not contain credentials or a fragment".into());
            }
        }
        Ok(())
    }
}

fn error(message: impl ToString) -> AdapterError {
    AdapterError {
        message: message.to_string(),
        spend_stop: None,
    }
}

/// Returns None for direct endpoints, whose path belongs to the selected codec.
pub(crate) fn endpoint(
    deployment: &Deployment,
    protocol: ApiProtocol,
    base_url: &str,
    model: &str,
) -> Result<Option<String>, AdapterError> {
    endpoint_with_stream(deployment, protocol, base_url, model, true)
}

pub(crate) fn endpoint_with_stream(
    deployment: &Deployment,
    protocol: ApiProtocol,
    base_url: &str,
    model: &str,
    stream: bool,
) -> Result<Option<String>, AdapterError> {
    if matches!(deployment, Deployment::Direct) {
        return Ok(None);
    }
    let mut url = url::Url::parse(base_url).map_err(error)?;
    let mut segments = Vec::<String>::new();
    match deployment {
        Deployment::Direct => unreachable!(),
        Deployment::AzureV1 => {
            segments.extend(["openai".into(), "v1".into()]);
            if protocol == ApiProtocol::OpenaiResponses {
                segments.push("responses".into());
            } else {
                segments.extend(["chat".into(), "completions".into()]);
            }
        }
        Deployment::AzureDeployment { api_version } => {
            segments.extend([
                "openai".into(),
                "deployments".into(),
                model.into(),
                "chat".into(),
                "completions".into(),
            ]);
            let pairs: Vec<(String, String)> = url
                .query_pairs()
                .filter(|(key, _)| key != "api-version")
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();
            url.set_query(None);
            url.query_pairs_mut()
                .extend_pairs(pairs)
                .append_pair("api-version", api_version);
        }
        Deployment::Vertex {
            project, location, ..
        } => {
            let official_host = url.host_str().is_some_and(|host| {
                host == "aiplatform.googleapis.com"
                    || host.ends_with("-aiplatform.googleapis.com")
                    || (host.starts_with("aiplatform.") && host.ends_with(".rep.googleapis.com"))
            });
            if official_host {
                let host = if location == "global" {
                    "aiplatform.googleapis.com".to_owned()
                } else if protocol == ApiProtocol::GeminiInteractions {
                    format!("aiplatform.{location}.rep.googleapis.com")
                } else {
                    format!("{location}-aiplatform.googleapis.com")
                };
                url.set_host(Some(&host)).map_err(error)?;
            }
            segments.extend([
                "v1".into(),
                "projects".into(),
                project.clone(),
                "locations".into(),
                location.clone(),
            ]);
            match protocol {
                ApiProtocol::GeminiInteractions => segments.push("interactions".into()),
                ApiProtocol::GeminiGenerateContent => {
                    segments.extend([
                        "publishers".into(),
                        "google".into(),
                        "models".into(),
                        format!(
                            "{model}:{}",
                            if stream {
                                "streamGenerateContent"
                            } else {
                                "generateContent"
                            }
                        ),
                    ]);
                    if stream {
                        url.query_pairs_mut().append_pair("alt", "sse");
                    }
                }
                ApiProtocol::AnthropicMessages => segments.extend([
                    "publishers".into(),
                    "anthropic".into(),
                    "models".into(),
                    format!(
                        "{model}:{}",
                        if stream {
                            "streamRawPredict"
                        } else {
                            "rawPredict"
                        }
                    ),
                ]),
                _ => return Err(error("Unsupported protocol for Vertex deployment")),
            }
        }
        Deployment::Bedrock { region } => {
            if let Some(host) = url
                .host_str()
                .filter(|host| host.starts_with("bedrock-runtime."))
            {
                let suffix = if host.ends_with(".amazonaws.com.cn") {
                    Some("amazonaws.com.cn")
                } else if host.ends_with(".amazonaws.com") {
                    Some("amazonaws.com")
                } else {
                    None
                };
                if let Some(suffix) = suffix {
                    url.set_host(Some(&format!("bedrock-runtime.{region}.{suffix}")))
                        .map_err(error)?;
                }
            }
            segments.extend([
                "model".into(),
                model.into(),
                if stream {
                    "converse-stream".into()
                } else {
                    "converse".into()
                },
            ]);
        }
    }
    url.path_segments_mut()
        .map_err(|_| error("Cloud endpoint must be an HTTP URL"))?
        .clear()
        .extend(&segments);
    Ok(Some(url.to_string()))
}

pub(crate) fn project_body(
    deployment: &Deployment,
    protocol: ApiProtocol,
    mut body: Value,
) -> Result<Value, AdapterError> {
    let object = body
        .as_object_mut()
        .ok_or_else(|| error("Provider request must be an object"))?;
    match deployment {
        Deployment::AzureDeployment { .. } => {
            object.remove("model");
        }
        Deployment::Vertex { .. } if protocol == ApiProtocol::AnthropicMessages => {
            object.remove("model");
            object.insert(
                "anthropic_version".into(),
                Value::String("vertex-2023-10-16".into()),
            );
        }
        _ => {}
    }
    Ok(body)
}

/// A single shared async driver serves credential refresh; it never sends model requests.
fn auth_runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    static RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub(crate) enum CloudAuth {
    Google(google_cloud_auth::credentials::Credentials),
    Azure {
        credential: Arc<dyn azure_core::credentials::TokenCredential>,
        scope: &'static str,
    },
    None,
}

impl CloudAuth {
    pub(crate) fn new(auth: AuthMethod, deployment: &Deployment) -> Result<Self, String> {
        let scope = if matches!(deployment, Deployment::AzureV1) {
            "https://ai.azure.com/.default"
        } else {
            "https://cognitiveservices.azure.com/.default"
        };
        // Credentials can schedule refresh work while being constructed.
        let _guard = auth_runtime()?.enter();
        Ok(match auth {
            AuthMethod::GoogleAdc => {
                let mut builder = google_cloud_auth::credentials::Builder::default()
                    .with_scopes(["https://www.googleapis.com/auth/cloud-platform"]);
                if let Deployment::Vertex {
                    quota_project: Some(project),
                    ..
                } = deployment
                {
                    builder = builder.with_quota_project_id(project);
                }
                Self::Google(builder.build().map_err(|e| format!("Google ADC: {e}"))?)
            }
            AuthMethod::AzureDeveloperTools => Self::Azure {
                credential: azure_identity::DeveloperToolsCredential::new(None)
                    .map_err(|e| format!("Azure developer credentials: {e}"))?,
                scope,
            },
            AuthMethod::AzureManagedIdentity => Self::Azure {
                credential: azure_identity::ManagedIdentityCredential::new(None)
                    .map_err(|e| format!("Azure managed identity: {e}"))?,
                scope,
            },
            AuthMethod::AzureWorkloadIdentity => Self::Azure {
                credential: azure_identity::WorkloadIdentityCredential::new(None)
                    .map_err(|e| format!("Azure workload identity: {e}"))?,
                scope,
            },
            _ => Self::None,
        })
    }

    pub(crate) fn headers(
        &self,
        timeout: Duration,
        cancellation: &CancellationToken,
    ) -> Result<BTreeMap<String, String>, AdapterError> {
        cancellation.check().map_err(|e| error(e.message))?;
        if matches!(self, Self::None) {
            return Ok(BTreeMap::new());
        }
        let runtime = auth_runtime().map_err(error)?;
        runtime.block_on(async {
            let get = async {
                match self {
                    Self::Google(credentials) => {
                        let resource = credentials.headers(http::Extensions::new()).await.map_err(|e|error(format!("Google ADC: {e}")))?;
                        match resource {
                            google_cloud_auth::credentials::CacheableResource::New { data, .. } => {
                                data.iter().map(|(name,value)|Ok((name.to_string(),value.to_str().map_err(error)?.to_owned()))).collect()
                            },
                            google_cloud_auth::credentials::CacheableResource::NotModified => Err(error("Google ADC returned no headers")),
                        }
                    },
                    Self::Azure { credential, scope } => {
                        let token = credential.get_token(&[scope], None).await.map_err(|e|error(format!("Azure Entra credentials: {e}")))?;
                        Ok(BTreeMap::from([("Authorization".into(), format!("Bearer {}", token.token.secret()))]))
                    },
                    Self::None => Ok(BTreeMap::new()),
                }
            };
            let cancelled = async {
                loop {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                    if cancellation.is_cancelled() { break; }
                }
            };
            tokio::select! {
                result = tokio::time::timeout(timeout,get) => result.map_err(|_|error("Cloud credential lookup timed out"))?,
                _ = cancelled => Err(error("Agent run was cancelled")),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloud_azure_routes_preserve_version_and_deployment_name() {
        let url = endpoint(
            &Deployment::AzureDeployment {
                api_version: "2024-10-21".into(),
            },
            ApiProtocol::OpenaiChatCompletions,
            "https://sample.openai.azure.com?custom=1&api-version=old",
            "deployment A",
        )
        .unwrap()
        .unwrap();
        assert_eq!(url,"https://sample.openai.azure.com/openai/deployments/deployment%20A/chat/completions?custom=1&api-version=2024-10-21");
        assert_eq!(
            endpoint(
                &Deployment::AzureV1,
                ApiProtocol::OpenaiResponses,
                "https://sample.openai.azure.com/openai/v1/",
                "deployment"
            )
            .unwrap()
            .unwrap(),
            "https://sample.openai.azure.com/openai/v1/responses"
        );
    }
    #[test]
    fn cloud_vertex_routes_and_body_keep_protocol_contracts() {
        let deployment = Deployment::Vertex {
            project: "book".into(),
            location: "global".into(),
            quota_project: None,
        };
        assert_eq!(
            endpoint(
                &deployment,
                ApiProtocol::GeminiInteractions,
                "https://aiplatform.googleapis.com",
                "gemini"
            )
            .unwrap()
            .unwrap(),
            "https://aiplatform.googleapis.com/v1/projects/book/locations/global/interactions"
        );
        let body = project_body(
            &deployment,
            ApiProtocol::AnthropicMessages,
            serde_json::json!({"model":"claude","messages":[],"max_tokens":10}),
        )
        .unwrap();
        assert!(body.get("model").is_none());
        assert_eq!(body["anthropic_version"], "vertex-2023-10-16");
        assert_eq!(body["max_tokens"], 10);
        let regional = Deployment::Vertex {
            project: "book".into(),
            location: "eu".into(),
            quota_project: None,
        };
        assert_eq!(
            endpoint(
                &regional,
                ApiProtocol::GeminiInteractions,
                "https://aiplatform.googleapis.com",
                "gemini"
            )
            .unwrap()
            .unwrap(),
            "https://aiplatform.eu.rep.googleapis.com/v1/projects/book/locations/eu/interactions"
        );
        let bedrock = Deployment::Bedrock {
            region: "us-west-2".into(),
        };
        assert_eq!(
            endpoint_with_stream(
                &bedrock,
                ApiProtocol::BedrockConverse,
                "https://bedrock-runtime.us-east-1.amazonaws.com",
                "model",
                false
            )
            .unwrap()
            .unwrap(),
            "https://bedrock-runtime.us-west-2.amazonaws.com/model/model/converse"
        );
    }
    #[test]
    fn cloud_validation_does_not_treat_credentials_as_generic_bearer() {
        assert!(Deployment::Direct
            .validate(
                ApiProtocol::OpenaiChatCompletions,
                AuthMethod::GoogleAdc,
                "https://example.com"
            )
            .is_err());
        assert!(Deployment::Vertex {
            project: "p".into(),
            location: "asia-east1".into(),
            quota_project: None
        }
        .validate(
            ApiProtocol::GeminiInteractions,
            AuthMethod::GoogleAdc,
            "https://aiplatform.googleapis.com"
        )
        .is_err());
        assert!(Deployment::Bedrock {
            region: "us-east-1".into()
        }
        .validate(
            ApiProtocol::BedrockConverse,
            AuthMethod::Bearer,
            "https://bedrock-runtime.us-east-1.amazonaws.com"
        )
        .is_ok());
    }
    #[test]
    fn cloud_cancelled_credentials_do_not_start_lookup() {
        let cancellation = CancellationToken::default();
        cancellation.cancel();
        assert!(CloudAuth::None
            .headers(Duration::from_secs(1), &cancellation)
            .is_err());
    }
}
