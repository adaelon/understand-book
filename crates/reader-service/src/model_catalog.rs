use runtime::cloud_provider::Deployment;
use runtime::model_settings::{ApiProtocol, AuthMethod, ModelService, ModelSettings};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::BTreeSet, time::Duration};
use url::Url;

#[derive(Deserialize)]
pub struct CatalogRequest {
    service: ModelService,
    api_protocol: ApiProtocol,
}

pub fn list(current: Option<&ModelSettings>, input: CatalogRequest) -> Result<Value, String> {
    use ApiProtocol::*;
    let service = &input.service;
    if service.deployment != Deployment::Direct {
        return Err("This deployment requires a manually entered model ID.".into());
    }
    let mut url = Url::parse(service.base_url.trim()).map_err(|_| "Invalid service address")?;
    if !matches!(url.scheme(), "http" | "https") { return Err("Use an HTTP or HTTPS service address.".into()); }
    let origin = url.origin();
    let path = url.path().trim_end_matches('/');
    let endpoint = match input.api_protocol {
        OpenaiChatCompletions | OpenaiResponses => {
            let base = path.strip_suffix("/chat/completions").or_else(|| path.strip_suffix("/responses")).unwrap_or(path);
            format!("{base}/models")
        }
        AnthropicMessages => {
            let base = path.strip_suffix("/v1/messages").or_else(|| path.strip_suffix("/v1")).unwrap_or(path);
            format!("{base}/v1/models")
        }
        GeminiGenerateContent | GeminiInteractions => format!("{}/models", if path.is_empty() { "/v1beta" } else { path }),
        OllamaChat => {
            let base = path.strip_suffix("/api/chat").or_else(|| path.strip_suffix("/api")).unwrap_or(path);
            format!("{base}/api/tags")
        }
        _ => return Err("This protocol requires a manually entered model ID.".into()),
    };
    url.set_path(&endpoint);
    if matches!(input.api_protocol, GeminiGenerateContent | GeminiInteractions) { url.query_pairs_mut().append_pair("pageSize", "1000"); }
    if input.api_protocol == AnthropicMessages { url.query_pairs_mut().append_pair("limit", "1000"); }

    let http = ureq::AgentBuilder::new().redirects(0).timeout(Duration::from_secs(15)).build();
    let mut request = http.get(url.as_str()).set("Accept", "application/json");
    for (name, value) in &service.headers { request = request.set(name, value); }
    if input.api_protocol == AnthropicMessages { request = request.set("anthropic-version", "2023-06-01"); }
    if service.auth.requires_api_key() {
        let key = if !service.api_key.trim().is_empty() { service.api_key.trim() } else {
            current.and_then(|settings| settings.services.iter().find(|saved| !service.clear_api_key
                && saved.id == service.id && saved.auth == service.auth
                && Url::parse(saved.base_url.trim()).is_ok_and(|url| url.origin() == origin)))
                .map(|saved| saved.api_key.trim()).filter(|key| !key.is_empty())
                .ok_or("Enter an API key for this service address.")?
        };
        request = match service.auth {
            AuthMethod::Bearer => request.set("Authorization", &format!("Bearer {key}")),
            _ if matches!(input.api_protocol, GeminiGenerateContent | GeminiInteractions) => request.set("x-goog-api-key", key),
            _ if input.api_protocol == AnthropicMessages => request.set("x-api-key", key),
            _ => request.set("api-key", key),
        };
    } else if service.auth != AuthMethod::None { return Err("This authentication method requires a manually entered model ID.".into()); }
    let response = request.call().map_err(|err| match err {
        ureq::Error::Status(code, _) => format!("HTTP {code}"),
        ureq::Error::Transport(_) => "Connection failed or timed out".into(),
    })?;
    if response.status() != 200 { return Err(format!("HTTP {}", response.status())); }
    let body: Value = response.into_json().map_err(|_| "The service did not return a model list.")?;
    let (items, key) = match input.api_protocol {
        OllamaChat | GeminiGenerateContent | GeminiInteractions => (&body["models"], "name"),
        _ => (&body["data"], "id"),
    };
    let entries = items.as_array().ok_or("The service did not return a model list.")?;
    let ids: BTreeSet<_> = entries.iter().filter_map(|entry| {
        if input.api_protocol == GeminiGenerateContent && entry["supportedGenerationMethods"].as_array()
            .is_some_and(|methods| !methods.iter().any(|m| m.as_str() == Some("generateContent"))) { return None; }
        let id = entry[key].as_str()?.trim();
        let id = if matches!(input.api_protocol, GeminiGenerateContent | GeminiInteractions) { id.strip_prefix("models/").unwrap_or(id) } else { id };
        (!id.is_empty()).then_some(id)
    }).collect();
    Ok(json!({ "models": ids, "has_more": body["has_more"].as_bool() == Some(true) || body["nextPageToken"].as_str().is_some_and(|s| !s.is_empty()) }))
}
