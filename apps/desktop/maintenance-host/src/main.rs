mod library_settings;
mod plugin_manager;
#[cfg(test)]
mod reading_share;

use library_settings::{LibrarySettingsStore, PersistedProviderSettings};
use plugin_manager::{PluginConfig, PluginManager, PluginState};
use runtime::{ModelAdapter, ProviderConfig, ProviderRegistry};
use serde::Deserialize;
#[cfg(test)]
use serde::Serialize;
use serde_json::Value;
use server::{
    run_codex_build_intent_command, CodexBuildIntentControllerConfig, UnconfiguredAdapter,
};
use std::io::Read;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
#[derive(Debug, Serialize)]
struct DesktopProviderStatus {
    configured: bool,
    source: &'static str,
    mode: String,
    base_url: String,
    model: String,
    api_key_configured: bool,
}

const CODEX_BUILD_INTENT_REQUEST_MAX_BYTES: usize = 64 * 1024;
const CODEX_BUILD_INTENT_ERROR_MAX_CHARS: usize = 1_024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodexBuildIntentCommand {
    version: String,
    operation: String,
    target: CodexBuildIntentTarget,
    input: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodexBuildIntentTarget {
    workspace_dir: PathBuf,
}

#[derive(Debug)]
struct CodexBuildIntentReadError {
    message: String,
    v2: bool,
}

impl CodexBuildIntentCommand {
    fn is_v2(&self) -> bool {
        self.version == "codex_build_intent_command.v2"
    }
}

fn persisted_provider_config(
    settings: &PersistedProviderSettings,
) -> Result<ProviderConfig, String> {
    ProviderConfig::from_values(
        &settings.mode,
        settings.api_key.clone(),
        settings.base_url.clone(),
        settings.model.clone(),
    )
    .map_err(|error| error.message)
}

#[cfg(test)]
fn provider_status(config: &ProviderConfig, source: &'static str) -> DesktopProviderStatus {
    DesktopProviderStatus {
        configured: true,
        source,
        mode: config.mode.as_str().into(),
        base_url: config.base_url.clone(),
        model: config.model.clone(),
        api_key_configured: !config.api_key.is_empty(),
    }
}

fn load_desktop_provider_env(local_app_data: &std::path::Path) {
    let _ = dotenvy::from_path(local_app_data.join("UnderstandBook").join(".env"));
    if cfg!(debug_assertions) {
        let repository_env = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.env");
        let _ = dotenvy::from_path(repository_env);
    }
}

fn plugin_manager() -> Result<PluginManager, String> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "LOCALAPPDATA is unavailable".to_string())?;
    let receipt = local_app_data
        .join("UnderstandBook")
        .join("plugin-installation.json");
    Ok(PluginManager::new(PluginConfig::from_environment(receipt)))
}

fn read_codex_build_intent_command(
    reader: impl Read,
) -> Result<CodexBuildIntentCommand, CodexBuildIntentReadError> {
    let mut body = Vec::new();
    reader
        .take((CODEX_BUILD_INTENT_REQUEST_MAX_BYTES + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|_| CodexBuildIntentReadError {
            message: "failed to read Codex build-intent request from stdin".into(),
            v2: false,
        })?;
    let v2 = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .as_deref()
        == Some("codex_build_intent_command.v2");
    if body.len() > CODEX_BUILD_INTENT_REQUEST_MAX_BYTES {
        return Err(CodexBuildIntentReadError {
            message: "Codex build-intent request exceeds 64 KiB".into(),
            v2,
        });
    }
    let command: CodexBuildIntentCommand =
        serde_json::from_slice(&body).map_err(|error| CodexBuildIntentReadError {
            message: format!("Codex build-intent request is invalid JSON: {error}"),
            v2,
        })?;
    if !matches!(
        command.version.as_str(),
        "codex_build_intent_command.v1" | "codex_build_intent_command.v2"
    ) {
        return Err(CodexBuildIntentReadError {
            message: "unsupported Codex build-intent command version".into(),
            v2: false,
        });
    }
    if !command.target.workspace_dir.is_absolute() {
        return Err(CodexBuildIntentReadError {
            message: "Codex build-intent workspace_dir must be absolute".into(),
            v2: command.is_v2(),
        });
    }
    if command.is_v2() {
        validate_codex_build_intent_v2_command(&command)
            .map_err(|message| CodexBuildIntentReadError { message, v2: true })?;
    }
    Ok(command)
}

fn validate_codex_build_intent_v2_command(command: &CodexBuildIntentCommand) -> Result<(), String> {
    let input = command
        .input
        .as_object()
        .ok_or_else(|| "Codex build-intent v2 input must be an object".to_string())?;
    let (allowed, required): (&[&str], &[&str]) = match command.operation.as_str() {
        "planning.context" => (&[], &[]),
        "draft.candidate" => (
            &[
                "user_goal",
                "planning_context_digest",
                "candidate",
                "budget",
            ],
            &["user_goal", "planning_context_digest", "candidate"],
        ),
        "status" => (&["plan_id"], &[]),
        "confirm" => (&["plan_id", "plan_digest"], &["plan_id", "plan_digest"]),
        "reject" | "artifact.prepare" => (&["plan_id"], &["plan_id"]),
        "artifact.submit" | "artifact.inspect" => (&["task_path"], &["task_path"]),
        "artifact.fail" => (
            &["task_path", "diagnostic_code", "message"],
            &["task_path", "diagnostic_code"],
        ),
        _ => return Err("unsupported Codex build-intent v2 operation".into()),
    };
    if input.keys().any(|key| !allowed.contains(&key.as_str()))
        || required.iter().any(|key| !input.contains_key(*key))
    {
        return Err("Codex build-intent v2 input fields do not match the operation".into());
    }
    for key in ["user_goal", "plan_id", "task_path", "diagnostic_code"] {
        if let Some(value) = input.get(key) {
            if value.as_str().is_none_or(|value| value.trim().is_empty()) {
                return Err(format!(
                    "Codex build-intent v2 {key} must be a non-blank string"
                ));
            }
        }
    }
    for key in ["planning_context_digest", "plan_digest"] {
        if let Some(value) = input.get(key).and_then(Value::as_str) {
            if value.len() != 64
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(format!(
                    "Codex build-intent v2 {key} must be a SHA-256 digest"
                ));
            }
        } else if input.contains_key(key) {
            return Err(format!("Codex build-intent v2 {key} must be a string"));
        }
    }
    if command.operation == "draft.candidate"
        && input
            .get("candidate")
            .is_none_or(|value| !value.is_object())
    {
        return Err("Codex build-intent v2 candidate must be an object".into());
    }
    if input.get("budget").is_some_and(|value| !value.is_object()) {
        return Err("Codex build-intent v2 budget must be an object".into());
    }
    if input.get("message").is_some_and(|value| {
        value
            .as_str()
            .is_none_or(|message| message.chars().count() > 1_024)
    }) {
        return Err("Codex build-intent v2 failure message must be at most 1024 characters".into());
    }
    Ok(())
}

fn codex_build_intent_adapter(
    settings: &LibrarySettingsStore,
) -> Result<Box<dyn ModelAdapter + Send>, String> {
    if let Some(settings) = settings.provider_settings()? {
        return persisted_provider_config(&settings).map(ProviderRegistry::adapter_from_config);
    }
    Ok(ProviderConfig::from_env()
        .map(ProviderRegistry::adapter_from_config)
        .unwrap_or_else(|_| Box::new(UnconfiguredAdapter)))
}

fn codex_build_intent_now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
        .to_string()
}

fn redact_codex_private_text(message: &str, private_goal: Option<&str>) -> String {
    let Some(goal) = private_goal.filter(|goal| !goal.is_empty()) else {
        return message.to_string();
    };
    let redacted = message.replace(goal, "[redacted]");
    let escaped = serde_json::to_string(goal).unwrap_or_default();
    let escaped = escaped
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(&escaped);
    if escaped.is_empty() {
        redacted
    } else {
        redacted.replace(escaped, "[redacted]")
    }
}

fn codex_v2_phase(operation: &str, error_code: &str) -> &'static str {
    if operation.starts_with("artifact.") {
        return "artifact";
    }
    if error_code.contains("BLUEPRINT") {
        return "blueprint";
    }
    if error_code.contains("CANDIDATE") {
        return "candidate";
    }
    if error_code.contains("STORAGE") || error_code.contains("STORE") {
        return "store";
    }
    if operation == "planning.context" || error_code.contains("PLANNING_CONTEXT") {
        return "context";
    }
    "compile"
}

fn codex_v2_error_result(
    error_code: &str,
    category: &str,
    phase: &str,
    retryable: bool,
    message: &str,
    sensitive_values: &[String],
) -> Value {
    let mut message = message.to_string();
    for sensitive in sensitive_values {
        message = redact_codex_private_text(&message, Some(sensitive));
    }
    message = message.replace(['\r', '\n', '\t'], " ");
    let message = if message.chars().count() > CODEX_BUILD_INTENT_ERROR_MAX_CHARS {
        format!(
            "{} [truncated]",
            message
                .chars()
                .take(CODEX_BUILD_INTENT_ERROR_MAX_CHARS - 12)
                .collect::<String>()
        )
    } else {
        message
    };
    serde_json::json!({
        "version": "codex_build_intent_result.v2",
        "status": "error",
        "error": {
            "error_code": error_code,
            "category": category,
            "phase": phase,
            "retryable": retryable,
            "message": message,
        }
    })
}

fn write_codex_v2_result(result: &Value) -> bool {
    match serde_json::to_string(result) {
        Ok(body) => {
            println!("{body}");
            true
        }
        Err(_) => {
            println!(
                "{}",
                r#"{"version":"codex_build_intent_result.v2","status":"error","error":{"error_code":"CODEX_BUILD_INTENT_SERIALIZE_FAILED","category":"internal","phase":"request","retryable":false,"message":"Codex build-intent result serialization failed"}}"#
            );
            false
        }
    }
}

fn handle_codex_build_intent_command() -> i32 {
    let command = match read_codex_build_intent_command(std::io::stdin().lock()) {
        Ok(command) => command,
        Err(error) if error.v2 => {
            write_codex_v2_result(&codex_v2_error_result(
                "CODEX_BUILD_INTENT_REQUEST_INVALID",
                "validation",
                "request",
                false,
                &error.message,
                &[],
            ));
            return 2;
        }
        Err(error) => {
            eprintln!(
                "{}",
                serde_json::json!({
                    "error_code": "CODEX_BUILD_INTENT_REQUEST_INVALID",
                    "category": "validation",
                    "message": error.message,
                })
            );
            return 2;
        }
    };
    let v2 = command.is_v2();
    let private_goal = command
        .input
        .get("user_goal")
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut sensitive_values = Vec::new();
    if let Some(goal) = &private_goal {
        sensitive_values.push(goal.clone());
    }
    if let Some(candidate) = command.input.get("candidate") {
        if let Ok(candidate) = serde_json::to_string(candidate) {
            sensitive_values.push(candidate);
        }
    }
    sensitive_values.push(command.target.workspace_dir.to_string_lossy().into_owned());
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        if v2 {
            write_codex_v2_result(&codex_v2_error_result(
                "CODEX_BUILD_INTENT_SETTINGS_UNAVAILABLE",
                "unavailable",
                "store",
                true,
                "LOCALAPPDATA is unavailable",
                &sensitive_values,
            ));
            return 2;
        }
        eprintln!(
            "{}",
            serde_json::json!({
                "error_code": "CODEX_BUILD_INTENT_SETTINGS_UNAVAILABLE",
                "category": "unavailable",
                "message": "LOCALAPPDATA is unavailable",
            })
        );
        return 2;
    };
    load_desktop_provider_env(&local_app_data);
    let settings =
        LibrarySettingsStore::new(local_app_data.join("UnderstandBook").join("settings.json"));
    let documents = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|path| path.join("Documents").join("UnderstandBook"))
        .unwrap_or_else(|| local_app_data.join("UnderstandBook").join("library"));
    let result = (|| {
        let library_root = settings.initial_root(&documents).map_err(|message| {
            (
                "CODEX_BUILD_INTENT_SETTINGS_UNAVAILABLE".into(),
                "unavailable".into(),
                message,
            )
        })?;
        let adapter: Box<dyn ModelAdapter + Send> = if v2 {
            Box::new(UnconfiguredAdapter)
        } else {
            codex_build_intent_adapter(&settings).map_err(|message| {
                (
                    "CODEX_BUILD_INTENT_PROVIDER_UNAVAILABLE".into(),
                    "provider".into(),
                    message,
                )
            })?
        };
        run_codex_build_intent_command(
            CodexBuildIntentControllerConfig {
                book_dir: command.target.workspace_dir,
                library_root,
                intent_store_root: None,
            },
            adapter,
            &command.operation,
            command.input,
            &codex_build_intent_now(),
        )
        .map_err(|error| (error.error_code, error.category, error.message))
    })();
    if v2 {
        return match result {
            Ok(response) => {
                let written = write_codex_v2_result(&serde_json::json!({
                    "version": "codex_build_intent_result.v2",
                    "status": "ok",
                    "response": response,
                }));
                if written {
                    0
                } else {
                    2
                }
            }
            Err((error_code, category, message)) => {
                let phase = codex_v2_phase(&command.operation, &error_code);
                let retryable = matches!(
                    category.as_str(),
                    "provider" | "unavailable" | "internal" | "needs_user"
                ) || error_code == "BUILD_PLANNING_CONTEXT_DRIFT";
                write_codex_v2_result(&codex_v2_error_result(
                    &error_code,
                    &category,
                    phase,
                    retryable,
                    &message,
                    &sensitive_values,
                ));
                2
            }
        };
    }
    match result {
        Ok(response) => match serde_json::to_string(&response) {
            Ok(response) => {
                println!("{response}");
                0
            }
            Err(_) => {
                eprintln!(
                    "{}",
                    r#"{"error_code":"CODEX_BUILD_INTENT_SERIALIZE_FAILED","category":"internal","message":"Codex build-intent response serialization failed"}"#
                );
                2
            }
        },
        Err((error_code, category, message)) => {
            let error = serde_json::json!({
                "error_code": error_code,
                "category": category,
                "message": message,
            })
            .to_string();
            eprintln!(
                "{}",
                redact_codex_private_text(&error, private_goal.as_deref())
            );
            2
        }
    }
}

fn handle_maintenance_command() -> Option<i32> {
    let command = std::env::args().nth(1);
    if command.as_deref() == Some("--codex-build-intent") {
        return Some(handle_codex_build_intent_command());
    }
    let (status, failure_states) = match command.as_deref() {
        Some("--codex-plugin-status") => (plugin_manager().map(|manager| manager.status()), false),
        Some("--install-codex-plugin") => (plugin_manager().map(|manager| manager.install()), true),
        Some("--uninstall-owned-codex-plugin") => (
            plugin_manager().map(|manager| manager.uninstall_owned()),
            true,
        ),
        _ => return None,
    };
    let exit_code = match status {
        Ok(status) => {
            let failed = failure_states
                && matches!(
                    status.state,
                    PluginState::PendingConfiguration
                        | PluginState::CodexNotFound
                        | PluginState::Error
                );
            match serde_json::to_string(&status) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("{error}");
                    return Some(2);
                }
            }
            if failed {
                2
            } else {
                0
            }
        }
        Err(error) => {
            eprintln!("{error}");
            2
        }
    };
    Some(exit_code)
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--presentation-preview-probe") {
        std::process::exit(server::presentation_preview::run_probe());
    }
    std::process::exit(handle_maintenance_command().unwrap_or_else(|| {
        eprintln!("Use --codex-build-intent or an explicit plugin maintenance command.");
        2
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn provider_status_never_serializes_the_api_key() {
        let config = ProviderConfig::from_values(
            "native",
            "super-secret",
            "https://provider.example/v1",
            "model-a",
        )
        .unwrap();

        let json = serde_json::to_string(&provider_status(&config, "settings")).unwrap();

        assert!(!json.contains("super-secret"));
        assert!(json.contains("\"api_key_configured\":true"));
    }

    #[test]
    fn codex_build_intent_command_is_strict_and_never_uses_argv_for_goal() {
        let workspace = std::env::current_dir().unwrap();
        let parsed = read_codex_build_intent_command(Cursor::new(
            serde_json::json!({
                "version": "codex_build_intent_command.v1",
                "operation": "draft",
                "target": { "workspace_dir": workspace },
                "input": { "user_goal": "private goal" },
            })
            .to_string(),
        ))
        .unwrap();
        assert_eq!(parsed.operation, "draft");
        assert_eq!(parsed.input["user_goal"], "private goal");

        let unknown = serde_json::json!({
            "version": "codex_build_intent_command.v1",
            "operation": "status",
            "target": { "workspace_dir": workspace },
            "input": {},
            "raw_goal": "must be rejected",
        })
        .to_string();
        assert!(read_codex_build_intent_command(Cursor::new(unknown)).is_err());
        let private_goal = "line one\nline two";
        let provider_error = format!(
            r#"{{"message":"provider echoed {}"}}"#,
            serde_json::to_string(private_goal)
                .unwrap()
                .trim_matches('"')
        );
        let redacted = redact_codex_private_text(&provider_error, Some(private_goal));
        assert!(!redacted.contains("line one"));
        assert!(redacted.contains("[redacted]"));

        let v2_context = read_codex_build_intent_command(Cursor::new(
            serde_json::json!({
                "version": "codex_build_intent_command.v2",
                "operation": "planning.context",
                "target": { "workspace_dir": workspace },
                "input": {},
            })
            .to_string(),
        ))
        .unwrap();
        assert!(v2_context.is_v2());

        let invalid_v2 = read_codex_build_intent_command(Cursor::new(
            serde_json::json!({
                "version": "codex_build_intent_command.v2",
                "operation": "planning.context",
                "target": { "workspace_dir": workspace },
                "input": { "user_goal": "must not enter context" },
            })
            .to_string(),
        ))
        .unwrap_err();
        assert!(invalid_v2.v2);

        let candidate = serde_json::json!({ "private": "CANDIDATE_SENTINEL" }).to_string();
        let result = codex_v2_error_result(
            "BUILD_INTENT_CANDIDATE_INVALID",
            "validation",
            codex_v2_phase("draft.candidate", "BUILD_INTENT_CANDIDATE_INVALID"),
            false,
            &format!("goal={private_goal};candidate={candidate}"),
            &[private_goal.into(), candidate.clone()],
        );
        let body = serde_json::to_string(&result).unwrap();
        assert_eq!(result["version"], "codex_build_intent_result.v2");
        assert_eq!(result["status"], "error");
        assert_eq!(result["error"]["phase"], "candidate");
        assert!(!body.contains("line one"));
        assert!(!body.contains("CANDIDATE_SENTINEL"));
        assert!(result["error"]["message"].as_str().unwrap().chars().count() <= 1_024);
    }
}
