//! Local developer application. The reader and runtime live in this process.
#[path = "../../../desktop/maintenance-host/src/library_settings.rs"]
mod library_settings;
#[allow(dead_code)]
#[path = "../../../desktop/maintenance-host/src/plugin_manager.rs"]
mod plugin_manager;
#[path = "../../../desktop/src-tauri/src/reading_share.rs"]
mod reading_share;
mod note_drafts;

use library_settings::{LibrarySettingsStore, PersistedProviderSettings};
use runtime::ProviderConfig;
use serde::{Deserialize, Serialize};
use server::host::{start_server, RunningServer, ServerHostConfig};
use std::{path::PathBuf, sync::Mutex};
use tauri::{ipc::CapabilityBuilder, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder};

struct DesktopServer {
    server: Mutex<Option<RunningServer>>,
    settings: LibrarySettingsStore,
    data_dir: PathBuf,
    note_drafts: note_drafts::NoteDraftStore,
}

#[derive(Deserialize)]
struct ProviderInput {
    mode: String,
    api_key: String,
    base_url: String,
    model: String,
}

#[derive(Serialize)]
struct ProviderStatus {
    configured: bool,
    source: &'static str,
    mode: String,
    base_url: String,
    model: String,
    api_key_configured: bool,
}

fn saved_config(settings: &PersistedProviderSettings) -> Result<ProviderConfig, String> {
    ProviderConfig::from_values(
        &settings.mode,
        settings.api_key.clone(),
        settings.base_url.clone(),
        settings.model.clone(),
    )
    .map_err(|error| error.message)
}

fn provider_status(config: Option<&ProviderConfig>, source: &'static str) -> ProviderStatus {
    ProviderStatus {
        configured: config.is_some(),
        source,
        mode: config.map(|c| c.mode.as_str()).unwrap_or("native").into(),
        base_url: config.map(|c| c.base_url.clone()).unwrap_or_default(),
        model: config.map(|c| c.model.clone()).unwrap_or_default(),
        api_key_configured: config.is_some_and(|c| !c.api_key.is_empty()),
    }
}

#[tauri::command]
fn desktop_set_language() {}

#[tauri::command]
fn load_local_note_drafts(initial_drafts: note_drafts::DraftMap, state: State<'_, DesktopServer>) -> Result<note_drafts::DraftMap, String> {
    state.note_drafts.load_or_import(initial_drafts)
}

#[tauri::command]
fn write_local_note_draft(draft_id: String, draft: Option<serde_json::Value>, state: State<'_, DesktopServer>) -> Result<(), String> {
    state.note_drafts.update(draft_id, draft)
}

#[tauri::command]
fn local_debug_build_status() -> Result<serde_json::Value, String> {
    let engine = std::env::current_exe().map_err(|e| e.to_string())?
        .with_file_name("understand-book-build.exe");
    Ok(serde_json::json!({ "available": engine.is_file(), "path": engine }))
}

fn internal_navigation(url: &tauri::Url, origin: &str) -> bool {
    url.origin().ascii_serialization() == origin || url.as_str() == "about:blank"
}

fn open_external(url: &tauri::Url) {
    if !matches!(url.scheme(), "http" | "https") { return; }
    #[cfg(windows)]
    let result = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", url.as_str()])
            .creation_flags(0x08000000).spawn()
    };
    #[cfg(not(windows))]
    let result = std::process::Command::new(if cfg!(target_os = "macos") { "open" } else { "xdg-open" })
        .arg(url.as_str()).spawn();
    if let Err(error) = result { eprintln!("Cannot open external reference: {error}"); }
}

fn navigate(url: &tauri::Url, origin: &str) -> bool {
    if internal_navigation(url, origin) { return true; }
    open_external(url);
    false
}

fn plugin_manager(state: &DesktopServer) -> plugin_manager::PluginManager {
    plugin_manager::PluginManager::new(plugin_manager::PluginConfig::from_environment(
        state.data_dir.join("plugin-installation.json"),
    ))
}

#[tauri::command]
fn codex_plugin_status(state: State<'_, DesktopServer>) -> plugin_manager::PluginStatus {
    plugin_manager(&state).status()
}

#[tauri::command]
async fn install_codex_plugin(
    app: tauri::AppHandle,
) -> Result<plugin_manager::PluginStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        plugin_manager(&app.state::<DesktopServer>()).install()
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_desktop_library_directory(
    selected_dir: String,
    state: State<'_, DesktopServer>,
) -> Result<serde_json::Value, String> {
    let guard = state.server.lock().unwrap();
    let server = guard.as_ref().ok_or("Local reader has stopped")?;
    let root = state
        .settings
        .apply_selection(&PathBuf::from(selected_dir))?;
    server.set_library_root(root.clone());
    Ok(serde_json::json!({ "library_root": root }))
}

#[tauri::command]
fn desktop_provider_status(state: State<'_, DesktopServer>) -> Result<ProviderStatus, String> {
    if let Some(saved) = state.settings.provider_settings()? {
        return Ok(provider_status(Some(&saved_config(&saved)?), "settings"));
    }
    let config = ProviderConfig::from_env().ok();
    Ok(provider_status(
        config.as_ref(),
        if config.is_some() {
            "environment"
        } else {
            "unconfigured"
        },
    ))
}

#[tauri::command]
fn save_desktop_provider_settings(
    input: ProviderInput,
    state: State<'_, DesktopServer>,
) -> Result<ProviderStatus, String> {
    let existing = state.settings.provider_settings()?;
    let api_key = if input.api_key.trim().is_empty() {
        existing
            .map(|s| s.api_key)
            .or_else(|| ProviderConfig::from_env().ok().map(|c| c.api_key))
            .ok_or("Provider API Key 不能为空")?
    } else {
        input.api_key
    };
    let config = ProviderConfig::from_values(&input.mode, api_key, input.base_url, input.model)
        .map_err(|e| e.message)?;
    let guard = state.server.lock().unwrap();
    let server = guard.as_ref().ok_or("Local reader has stopped")?;
    let root = server
        .library_root()
        .ok_or("Local library is unavailable")?;
    state.settings.apply_provider(
        &root,
        PersistedProviderSettings {
            mode: config.mode.as_str().into(),
            api_key: config.api_key.clone(),
            base_url: config.base_url.clone(),
            model: config.model.clone(),
        },
    )?;
    server.set_provider_config(config.clone());
    Ok(provider_status(Some(&config), "settings"))
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--presentation-preview-probe") {
        std::process::exit(server::presentation_preview::run_probe());
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            desktop_set_language,
            load_local_note_drafts,
            write_local_note_draft,
            local_debug_build_status,
            codex_plugin_status,
            install_codex_plugin,
            set_desktop_library_directory,
            desktop_provider_status,
            save_desktop_provider_settings,
            reading_share::save_reading_share_image
        ])
        .setup(|app| {
            let initial_book = std::env::args()
                .nth(1)
                .map(PathBuf::from)
                .map(|path| {
                    if path.is_absolute() {
                        Ok(path)
                    } else {
                        std::env::current_dir().map(|cwd| cwd.join(path))
                    }
                })
                .transpose()?;
            let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
            let data_dir = std::env::var_os("UNDERSTAND_BOOK_DEBUG_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or(app.path().app_local_data_dir()?);
            std::fs::create_dir_all(&data_dir)?;
            // Prevent dotenv's upward search from loading a source checkout's credentials.
            if !data_dir.join(".env").exists() {
                std::fs::write(
                    data_dir.join(".env"),
                    "# Optional local debug model configuration.\n",
                )?;
            }
            std::env::set_current_dir(&data_dir)?;
            let _ = dotenvy::from_path(data_dir.join(".env"));
            std::env::set_var("UNDERSTAND_BOOK_MEMORY_DIR", data_dir.join("memory"));
            std::env::set_var("UNDERSTAND_BOOK_PRIVATE_DIR", data_dir.join("private"));
            std::env::set_var("UB_OBSERVABILITY_MODE", "off");
            let settings = LibrarySettingsStore::new(data_dir.join("settings.json"));
            let root = settings.initial_root(&data_dir.join("library"))?;
            let web_dist = exe_dir.join("resources/web-dist");
            if !web_dist.join("index.html").is_file() {
                return Err(
                    "Missing resources/web-dist. Run node apps/local-debug/package.mjs first."
                        .into(),
                );
            }
            let mut config = ServerHostConfig::desktop(root, web_dist);
            config.book_dir = initial_book;
            let server = start_server(config)?;
            if let Some(saved) = settings.provider_settings()? {
                server.set_provider_config(saved_config(&saved)?);
            }
            let url: tauri::Url = server.url.parse()?;
            let origin = url.origin().ascii_serialization();
            app.add_capability(
                CapabilityBuilder::new("local-debug")
                    .windows(["main", "local-child-*"])
                    .remote(format!("{origin}/*"))
                    .permission("core:default")
                    .permission("core:window:allow-destroy")
                    .permission("dialog:allow-open")
                    .permission("dialog:allow-save")
                    .permission("allow-desktop-set-language")
                    .permission("allow-load-local-note-drafts")
                    .permission("allow-write-local-note-draft")
                    .permission("allow-local-debug-build-status")
                    .permission("allow-codex-plugin-status")
                    .permission("allow-install-codex-plugin")
                    .permission("allow-set-desktop-library-directory")
                    .permission("allow-desktop-provider-status")
                    .permission("allow-save-desktop-provider-settings")
                    .permission("allow-save-reading-share-image"),
            )?;
            eprintln!(
                "Local debug reader: {}\nData: {}",
                server.url,
                data_dir.display()
            );
            std::fs::write(
                data_dir.join("debug-session.json"),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "url": server.url, "pid": std::process::id(), "data_dir": data_dir,
                }))?,
            )?;
            app.manage(DesktopServer {
                server: Mutex::new(Some(server)),
                settings,
                data_dir: data_dir.clone(),
                note_drafts: note_drafts::NoteDraftStore::new(data_dir.clone()),
            });
            let popup_app = app.handle().clone();
            let popup_origin = origin.clone();
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Understand Book · Local Debug")
                .inner_size(1440.0, 920.0)
                .min_inner_size(960.0, 640.0)
                .data_directory(data_dir.join("webview"))
                .on_navigation(move |url| {
                    navigate(url, &origin)
                })
                .on_new_window(move |url, features| {
                    if url.origin().ascii_serialization() != popup_origin {
                        open_external(&url);
                        return tauri::webview::NewWindowResponse::Deny;
                    }
                    static NEXT: std::sync::atomic::AtomicUsize =
                        std::sync::atomic::AtomicUsize::new(1);
                    let child_origin = popup_origin.clone();
                    match WebviewWindowBuilder::new(
                        &popup_app,
                        format!(
                            "local-child-{}",
                            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        ),
                        WebviewUrl::External("about:blank".parse().unwrap()),
                    )
                    .window_features(features)
                    .title("Understand Book · Local Debug")
                    .inner_size(1100.0, 800.0)
                    .on_navigation(move |url| {
                        navigate(url, &child_origin)
                    })
                    .build()
                    {
                        Ok(window) => tauri::webview::NewWindowResponse::Create { window },
                        Err(_) => tauri::webview::NewWindowResponse::Deny,
                    }
                })
                .build()?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to start local debug reader");
    app.run(|handle, event| {
        if matches!(event, RunEvent::Exit) {
            if let Some(state) = handle.try_state::<DesktopServer>() {
                if let Some(server) = state.server.lock().unwrap().take() {
                    server.shutdown();
                }
                let _ = std::fs::remove_file(state.data_dir.join("debug-session.json"));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_links_leave_reader_but_local_views_stay() {
        let origin = "http://127.0.0.1:49152";
        for url in ["about:blank", "http://127.0.0.1:49152/api/book/source"] {
            assert!(internal_navigation(&url.parse().unwrap(), origin));
        }
        for url in ["https://doi.org/10.1000/test", "http://127.0.0.1:49153/", "file:///C:/test"] {
            assert!(!internal_navigation(&url.parse().unwrap(), origin));
        }
    }
    #[test]
    fn provider_status_excludes_saved_secret() {
        let config = ProviderConfig::from_values(
            "native",
            "secret-value",
            "https://model.example/v1",
            "model",
        )
        .unwrap();
        let status = serde_json::to_value(provider_status(Some(&config), "settings")).unwrap();
        assert_eq!(status["configured"], true);
        assert_eq!(status["api_key_configured"], true);
        assert!(!status.to_string().contains("secret-value"));
        assert_eq!(provider_status(None, "unconfigured").configured, false);
    }
}
