//! Shared native local Reader host. The debug and installed products supply their own launch mode.
use desktop_support::library_settings::{LibrarySettingsStore, PersistedProviderSettings};
use runtime::ProviderConfig;
use runtime::model_settings::{ModelSettings, ModelPurpose};
use serde::{Deserialize, Serialize};
use reader_service::host::{start_server_with_model_settings, RunningServer, ServerHostConfig};
use std::{path::{Path, PathBuf}, sync::{Mutex, atomic::{AtomicBool, Ordering}}};
use tauri::{ipc::CapabilityBuilder, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder};
use desktop_support::{legacy_import, note_drafts, plugin_manager};

struct DesktopServer {
    server: Mutex<Option<RunningServer>>,
    url: String,
    settings: LibrarySettingsStore,
    data_dir: PathBuf,
    note_drafts: note_drafts::NoteDraftStore,
}

fn reader_request(url: &str, path: &str, body: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
    let endpoint = format!("{url}/api{path}");
    let response = match body {
        Some(body) => ureq::post(&endpoint).send_json(body),
        None => ureq::get(&endpoint).call(),
    };
    match response {
        Ok(response) => response.into_json().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(status, response)) => {
            let message = response.into_json::<serde_json::Value>().ok()
                .and_then(|v| v["message"].as_str().map(str::to_owned));
            Err(message.unwrap_or_else(|| format!("Reader HTTP {status}")))
        }
        Err(error) => Err(error.to_string()),
    }
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
fn desktop_set_language(app: tauri::AppHandle, language: String) -> Result<(), String> {
    if app.state::<StartupState>().edition == LocalEdition::Installed {
        if let Some(window) = app.get_webview_window("main") {
            window.set_title(if language == "en" { "Understand Book · Local" } else { "Understand Book · 本地版" }).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn load_local_note_drafts(initial_drafts: note_drafts::DraftMap, state: State<'_, DesktopServer>) -> Result<note_drafts::DraftMap, String> {
    state.note_drafts.load_or_import(initial_drafts)
}

#[tauri::command]
fn write_local_note_draft(draft_id: String, draft: Option<serde_json::Value>, state: State<'_, DesktopServer>) -> Result<(), String> {
    state.note_drafts.update(draft_id, draft)
}

#[tauri::command]
fn local_build_status(state: State<'_, StartupState>) -> serde_json::Value {
    let engine = state.engine_path();
    serde_json::json!({ "available": engine.as_ref().is_some_and(|path| path.is_file()), "path": engine })
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

fn plugin_manager(state: &DesktopServer) -> plugin_manager::PluginManager {
    plugin_manager::PluginManager::new(plugin_manager::PluginConfig::from_environment(
        state.data_dir.join("plugin-installation.json"),
    ))
}

#[tauri::command]
async fn codex_plugin_status(app: tauri::AppHandle) -> Result<plugin_manager::PluginStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        plugin_manager(&app.state::<DesktopServer>()).status()
    })
    .await
    .map_err(|error| error.to_string())
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
    let Some(server) = guard.as_ref() else {
        return reader_request(&state.url, "/local-agent/library", Some(serde_json::json!({"selected_dir": selected_dir})));
    };
    let root = state
        .settings
        .apply_selection(&PathBuf::from(selected_dir))?;
    server.set_library_root(root.clone());
    Ok(serde_json::json!({ "library_root": root }))
}

#[tauri::command]
fn desktop_provider_status(state: State<'_, DesktopServer>) -> Result<ProviderStatus, String> {
    let guard=state.server.lock().unwrap();
    let server=guard.as_ref().ok_or("Local reader has stopped")?;
    let status=server.model_settings_status();
    let source=if status["source"]=="settings" {"settings"} else {"environment"};
    let config=server.model_settings_snapshot().map(|settings|settings.resolve(ModelPurpose::Agent)).transpose()?;
    Ok(provider_status(config.as_ref(),if config.is_some(){source}else{"unconfigured"}))
}

#[tauri::command]
fn save_desktop_provider_settings(
    input: ProviderInput,
    state: State<'_, DesktopServer>,
) -> Result<ProviderStatus, String> {
    if state.settings.model_settings()?.is_some() || std::env::var_os("UNDERSTAND_BOOK_MODEL_SETTINGS").is_some() {
        return Err("请使用模型服务设置修改已升级的配置。".into());
    }
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

#[tauri::command]
fn desktop_model_settings(state: State<'_, DesktopServer>) -> Result<serde_json::Value,String> {
    let guard=state.server.lock().unwrap();
    match guard.as_ref() {
        Some(server) => Ok(server.model_settings_status()),
        None => reader_request(&state.url, "/model-settings", None),
    }
}

#[tauri::command]
fn save_desktop_model_settings(input: ModelSettings, state: State<'_,DesktopServer>) -> Result<serde_json::Value,String> {
    let guard=state.server.lock().unwrap();
    match guard.as_ref() {
        Some(server) => server.save_model_settings(input),
        None => reader_request(&state.url, "/model-settings", Some(serde_json::json!(input))),
    }
}

#[tauri::command]
async fn test_desktop_model(model_config_id: String, input: Option<ModelSettings>, app: tauri::AppHandle) -> Result<serde_json::Value,String> {
    let attached = {
        let state = app.state::<DesktopServer>();
        let attached = state.server.lock().unwrap().is_none().then(|| state.url.clone());
        attached
    };
    if let Some(url) = attached {
        return tauri::async_runtime::spawn_blocking(move || reader_request(&url, "/model-settings/test",
            Some(serde_json::json!({"model_config_id":model_config_id,"input":input}))))
            .await.map_err(|e| e.to_string())?;
    }
    let settings={
        let state=app.state::<DesktopServer>();
        let guard=state.server.lock().unwrap();
        reader_service::model_settings::prepare_test_settings(
            guard.as_ref().ok_or("Local reader has stopped")?.model_settings_snapshot(),input)?
    };
    tauri::async_runtime::spawn_blocking(move ||reader_service::model_settings::test_model(&settings,&model_config_id))
        .await.map_err(|error|error.to_string())?
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LocalEdition { Installed, Debug }

struct StartupState {
    edition: LocalEdition,
    data_home: PathBuf,
    data_dir: Mutex<PathBuf>,
    resources: PathBuf,
    initial_book: Option<PathBuf>,
    starting: AtomicBool,
}

impl StartupState {
    fn engine_path(&self) -> Option<PathBuf> {
        (self.edition == LocalEdition::Debug).then(|| self.resources.parent().unwrap().join(
            if cfg!(windows) { "understand-book-build.exe" } else { "understand-book-build" }
        ))
    }
    fn log(&self, message: &str) {
        use std::io::Write;
        let _ = std::fs::create_dir_all(&self.data_home);
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(self.data_home.join("startup.log")) {
            let _ = writeln!(file, "{message}");
        }
    }
}

#[tauri::command]
fn save_reading_share_image(path: PathBuf, bytes: Vec<u8>) -> Result<(), String> {
    desktop_support::reading_share::save_reading_share_image(path, bytes)
}

#[tauri::command]
fn local_startup_status(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let state = app.state::<StartupState>();
    let directory = state.data_dir.lock().unwrap().clone();
    let mut candidates = Vec::<PathBuf>::new();
    if let Some(source) = std::env::var_os("UNDERSTAND_BOOK_DEBUG_DATA_DIR").map(PathBuf::from).filter(|p| p.is_dir()) {
        candidates.push(source);
    }
    if let Some(parent) = app.path().app_local_data_dir().ok().and_then(|p| p.parent().map(Path::to_owned)) {
        let source = parent.join("app.understandbook.local-debug");
        if source.is_dir() && !candidates.contains(&source) { candidates.push(source); }
    }
    Ok(serde_json::json!({"has_data": legacy_import::has_user_data(&directory),
        "data_dir": directory, "sources": candidates, "running": app.try_state::<DesktopServer>().is_some(),
        "import_pending": legacy_import::pending(&directory)}))
}

#[tauri::command]
async fn local_pick_directory(app: tauri::AppHandle) -> Result<Option<PathBuf>, String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().blocking_pick_folder().map(|path| path.into_path().map_err(|e| e.to_string())).transpose()
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn local_import_preview(source: PathBuf) -> Result<legacy_import::ImportPreview, String> {
    tauri::async_runtime::spawn_blocking(move || legacy_import::preview(&source)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn local_import_data(app: tauri::AppHandle, source: PathBuf, destination_parent: Option<PathBuf>) -> Result<String, String> {
    if app.try_state::<DesktopServer>().is_some() { return Err("请关闭当前应用，再从启动页导入旧数据。".into()); }
    let state = app.state::<StartupState>();
    if state.starting.swap(true, Ordering::AcqRel) { return Err("本地资料正在打开，请稍候。".into()); }
    let result = tauri::async_runtime::spawn_blocking({ let app = app.clone(); move || {
        let state = app.state::<StartupState>();
        let parent = destination_parent.unwrap_or_else(|| state.data_home.clone());
        let target = parent.join("imported-data");
        legacy_import::copy(&source, &target)?;
        *state.data_dir.lock().unwrap() = target.clone();
        start_reader(&app, None, None, false)
    }}).await.map_err(|e| e.to_string()).and_then(|r| r);
    state.starting.store(false, Ordering::Release);
    if let Err(error) = &result { state.log(error); }
    result
}

#[tauri::command]
async fn local_start_reading(app: tauri::AppHandle, selected_library: Option<PathBuf>, selected_book: Option<PathBuf>) -> Result<String, String> {
    let state = app.state::<StartupState>();
    if state.starting.swap(true, Ordering::AcqRel) { return Err("本地资料正在打开，请稍候。".into()); }
    let result = tauri::async_runtime::spawn_blocking({ let app = app.clone(); move || {
        let launch = app.state::<StartupState>();
        let library_home = launch.edition == LocalEdition::Installed && selected_book.is_none() && launch.initial_book.is_none();
        start_reader(&app, selected_library, selected_book, library_home)
    } })
        .await.map_err(|e| e.to_string()).and_then(|r| r);
    state.starting.store(false, Ordering::Release);
    if let Err(error) = &result { state.log(error); }
    result
}

#[tauri::command]
fn local_open_startup_log(state: State<'_, StartupState>) -> Result<(), String> {
    state.log("Opened startup log");
    let path = state.data_home.join("startup.log");
    #[cfg(windows)]
    let result = std::process::Command::new("notepad.exe").arg(path).spawn();
    #[cfg(not(windows))]
    let result = std::process::Command::new(if cfg!(target_os = "macos") { "open" } else { "xdg-open" }).arg(path).spawn();
    result.map(|_| ()).map_err(|e| e.to_string())
}

fn capabilities(state: &StartupState) -> String {
    let managed = state.engine_path().is_some_and(|p| p.is_file());
    format!("window.__UNDERSTAND_BOOK_LOCAL__ = {{ noteDrafts: true, managedBuildEngine: {managed} }};")
}

fn navigate_to_reader(app: &tauri::AppHandle, mut url: tauri::Url, library_home: bool) -> Result<(), String> {
    if library_home { url.query_pairs_mut().append_pair("view", "library"); }
    app.get_webview_window("main").ok_or("阅读窗口已关闭")?.navigate(url).map_err(|e| e.to_string())
}

fn start_reader(app: &tauri::AppHandle, selected_library: Option<PathBuf>, selected_book: Option<PathBuf>, library_home: bool) -> Result<String, String> {
    if let Some(runtime) = app.try_state::<DesktopServer>() {
        let url = runtime.url.clone();
        navigate_to_reader(app, url.parse::<tauri::Url>().map_err(|e| e.to_string())?, library_home)?;
        return Ok(url);
    }
    let launch = app.state::<StartupState>();
    let data_dir = launch.data_dir.lock().unwrap().clone();
    if !data_dir.is_absolute() { return Err("本地数据目录必须是绝对路径。".into()); }
    if let Some(url) = reader_service::local_profile::active_url(&data_dir)? {
        let parsed: tauri::Url = url.parse().map_err(|e| format!("本机 Reader 地址无效：{e}"))?;
        if parsed.scheme() != "http" || parsed.host_str().is_none_or(|s| !matches!(s, "127.0.0.1" | "[::1]")) {
            return Err("本地 Reader 需要回环地址。".into());
        }
        if let Some(parent) = selected_library {
            reader_request(&url, "/local-agent/library", Some(serde_json::json!({"selected_dir":parent})))?;
        }
        if let Some(book) = selected_book.as_ref().or(launch.initial_book.as_ref()) {
            reader_request(&url, "/book/open", Some(serde_json::json!({"dir":book})))?;
        }
        app.add_capability(reader_capability(&parsed.origin().ascii_serialization())).map_err(|e| e.to_string())?;
        app.manage(DesktopServer {
            server: Mutex::new(None), url: url.clone(), settings: LibrarySettingsStore::new(data_dir.join("settings.json")),
            note_drafts: note_drafts::NoteDraftStore::new(data_dir.clone()), data_dir,
        });
        navigate_to_reader(app, parsed, library_home)?;
        return Ok(url);
    }
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("无法建立本地数据目录：{e}"))?;
    if !data_dir.join(".env").exists() {
        std::fs::write(data_dir.join(".env"), "# Optional local model and format-tool configuration.\n").map_err(|e| e.to_string())?;
    }
    std::env::set_current_dir(&data_dir).map_err(|e| e.to_string())?;
    let _ = dotenvy::from_path(data_dir.join(".env"));
    std::env::set_var("UNDERSTAND_BOOK_MEMORY_DIR", data_dir.join("memory"));
    std::env::set_var("UNDERSTAND_BOOK_PRIVATE_DIR", data_dir.join("private"));
    std::env::set_var("UB_OBSERVABILITY_MODE", "off");
    let web_dist = launch.resources.join("web-dist");
    if !web_dist.join("index.html").is_file() {
        return Err("阅读界面资源不完整。请重新安装本地应用；你的资料和设置会保留。".into());
    }
    if launch.edition == LocalEdition::Installed {
        let runtime = launch.resources.join("reader-runtime");
        let node = runtime.join(if cfg!(windows) { "node.exe" } else { "node" });
        let runner = runtime.join("workbench-stage.mjs");
        if !node.is_file() || !runner.is_file() { return Err("资料读取组件不完整。请重新安装本地应用。".into()); }
        std::env::set_var("UNDERSTAND_BOOK_NODE", node);
        std::env::set_var("UNDERSTAND_BOOK_STAGE_RUNNER", runner);
    }
    let settings = LibrarySettingsStore::new(data_dir.join("settings.json"));
    let first_launch = settings.load()?.is_none();
    let root = if let Some(parent) = selected_library { settings.apply_selection(&parent)? }
        else { settings.initial_root(&data_dir.join("library"))? };
    let model_settings = match settings.model_settings()? {
        Some(value)=>Some(serde_json::from_value::<ModelSettings>(value).map_err(|error|format!("Invalid model settings: {error}"))?),
        None=>settings.provider_settings()?.map(|saved|saved_config(&saved).map(|config|ModelSettings::from_legacy(&config))).transpose()?,
    };
    if let Some(settings)=model_settings.as_ref() { settings.validate()?; }
    let drafts = note_drafts::NoteDraftStore::new(data_dir.clone());
    // Invalid drafts remain in place and get a recoverable startup error, never an empty replacement.
    let imported_drafts = drafts.load_or_import(note_drafts::DraftMap::new())?;
    let mut initial_book = selected_book.or_else(|| launch.initial_book.clone());
    if first_launch && launch.edition == LocalEdition::Installed {
        let sample = root.join("quickstart-demo");
        if !sample.exists() {
            legacy_import::copy_directory_or_file(&launch.resources.join("quickstart"), &sample)?;
        }
        if initial_book.is_none() { initial_book = Some(sample); }
    }
    let mut config = ServerHostConfig::desktop(root.clone(), web_dist);
    config.book_dir = initial_book;
    let server = start_server_with_model_settings(config,model_settings)?;
    let settings_store=settings.clone();
    server.configure_model_settings(None,"settings",std::sync::Arc::new(move |models| {
        let root=settings_store.load()?.ok_or("Local library is unavailable")?;
        settings_store.apply_model_settings(&root,serde_json::to_value(models).map_err(|error|error.to_string())?)
    }))?;
    let url = server.url.clone();
    let parsed: tauri::Url = match url.parse() { Ok(url) => url, Err(error) => { server.shutdown(); return Err(format!("无效的本地服务地址：{error}")); } };
    let origin = parsed.origin().ascii_serialization();
    let ready = (|| {
        if legacy_import::pending(&data_dir) {
            // Exercise the actual loaded private store and history before acknowledging this import.
            for route in ["/agent/history", "/profile/memory"] {
                let response: serde_json::Value = ureq::get(&format!("{url}/api{route}")).call()
                    .and_then(|response| response.into_json().map_err(ureq::Error::from))
                    .map_err(|e| format!("导入副本未能恢复阅读记录：{e}"))?;
                if response["status"]["review_error"]["error_code"] == "READER_PRIVATE_STORAGE_UNAVAILABLE" {
                    return Err(format!("导入副本无法保存私人记录：{}", response["status"]["review_error"]["message"].as_str().unwrap_or("存储不可用")));
                }
            }
            for (id, draft) in imported_drafts { drafts.update(id, Some(draft))?; }
        }
        settings.save_root(&root)?;
        if legacy_import::pending(&data_dir) { legacy_import::complete(&data_dir)?; }
        if data_dir != launch.data_home {
            let location = launch.data_home.join("data-location.json");
            let temporary = location.with_extension("json.tmp");
            std::fs::create_dir_all(&launch.data_home).map_err(|e| e.to_string())?;
            std::fs::write(&temporary, serde_json::to_vec_pretty(&data_dir).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            std::fs::rename(temporary, location).map_err(|e| e.to_string())?;
        }
        app.add_capability(reader_capability(&origin)).map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    })();
    if let Err(error) = ready { server.shutdown(); return Err(error); }
    if launch.edition == LocalEdition::Debug {
        std::fs::write(data_dir.join("debug-session.json"), serde_json::to_vec_pretty(&serde_json::json!({"url":url,"pid":std::process::id(),"data_dir":data_dir})).unwrap()).map_err(|e| e.to_string())?;
        eprintln!("Local debug reader: {url}\nData: {}", data_dir.display());
    }
    app.manage(DesktopServer { server: Mutex::new(Some(server)), url: url.clone(), settings, data_dir, note_drafts: drafts });
    launch.log("Local Reader started");
    navigate_to_reader(app, parsed, library_home)?;
    Ok(url)
}

fn reader_capability(origin: &str) -> CapabilityBuilder {
    CapabilityBuilder::new("local-reader")
        .windows(["main", "local-child-*"])
        .remote(format!("{origin}/*"))
        .permission("core:default").permission("core:window:allow-destroy")
        .permission("dialog:allow-open").permission("dialog:allow-save")
        .permission("allow-desktop-set-language").permission("allow-load-local-note-drafts")
        .permission("allow-write-local-note-draft").permission("allow-local-build-status")
        .permission("allow-codex-plugin-status").permission("allow-install-codex-plugin")
        .permission("allow-set-desktop-library-directory").permission("allow-desktop-provider-status")
        .permission("allow-save-desktop-provider-settings").permission("allow-save-reading-share-image")
        .permission("allow-desktop-model-settings").permission("allow-save-desktop-model-settings")
        .permission("allow-test-desktop-model")
}

fn allow_navigation(app: &tauri::AppHandle, url: &tauri::Url) -> bool {
    if matches!(url.scheme(), "tauri" | "asset") || url.host_str() == Some("tauri.localhost") || url.as_str() == "about:blank" { return true; }
    if let Some(state) = app.try_state::<DesktopServer>() {
        if state.url.parse::<tauri::Url>().is_ok_and(|origin| internal_navigation(url, &origin.origin().ascii_serialization())) { return true; }
    }
    open_external(url); false
}

pub fn run(context: tauri::Context<tauri::Wry>, edition: LocalEdition) {
    if reader_service::acp_evidence::serve_stdio_if_requested() { return; }
    if std::env::args().nth(1).as_deref() == Some("--presentation-preview-probe") {
        std::process::exit(reader_service::presentation_preview::run_probe());
    }
    let app = tauri::Builder::default().plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![desktop_set_language, load_local_note_drafts, write_local_note_draft,
            local_build_status, codex_plugin_status, install_codex_plugin, set_desktop_library_directory,
            desktop_provider_status, save_desktop_provider_settings, save_reading_share_image,
            desktop_model_settings, save_desktop_model_settings, test_desktop_model,
            local_startup_status, local_pick_directory, local_import_preview, local_import_data,
            local_start_reading, local_open_startup_log])
        .setup(move |app| {
            let personal_launch = std::env::args().nth(1).as_deref() == Some("--personal-connection");
            let initial_book = std::env::args().nth(1).filter(|_| !personal_launch).map(PathBuf::from).map(|path| {
                if path.is_absolute() { Ok(path) } else { std::env::current_dir().map(|cwd| cwd.join(path)) }
            }).transpose()?;
            let data_home = std::env::var_os(if edition == LocalEdition::Debug { "UNDERSTAND_BOOK_DEBUG_DATA_DIR" } else { "UNDERSTAND_BOOK_LOCAL_DATA_DIR" })
                .map(PathBuf::from).unwrap_or(app.path().app_local_data_dir()?);
            let resources = if edition == LocalEdition::Debug {
                std::env::current_exe()?.parent().unwrap().join("resources")
            } else { app.path().resource_dir()? };
            let location = data_home.join("data-location.json");
            let data_dir = if location.exists() {
                serde_json::from_slice::<PathBuf>(&std::fs::read(&location)?)?
            } else { data_home.clone() };
            app.manage(StartupState { edition, data_home, data_dir: Mutex::new(data_dir), resources, initial_book, starting: AtomicBool::new(false) });
            app.add_capability(CapabilityBuilder::new("local-startup").windows(["main"])
                .permission("core:default").permission("allow-local-startup-status").permission("allow-local-pick-directory")
                .permission("allow-local-import-preview").permission("allow-local-import-data")
                .permission("allow-local-start-reading").permission("allow-local-open-startup-log"))?;
            let title = if edition == LocalEdition::Debug { "Understand Book · Local Debug" } else { "Understand Book · 本地版" };
            let navigation_app = app.handle().clone();
            let popup_app = app.handle().clone();
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title(title).inner_size(1440.0, 920.0).min_inner_size(760.0, 600.0)
                .data_directory(app.state::<StartupState>().data_home.join("webview"))
                .initialization_script(&capabilities(&app.state::<StartupState>()))
                .on_navigation(move |url| allow_navigation(&navigation_app, url))
                .on_new_window(move |url, features| {
                    if !allow_navigation(&popup_app, &url) { return tauri::webview::NewWindowResponse::Deny; }
                    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
                    let child_app = popup_app.clone();
                    match WebviewWindowBuilder::new(&popup_app, format!("local-child-{}", NEXT.fetch_add(1, Ordering::Relaxed)), WebviewUrl::External("about:blank".parse().unwrap()))
                        .window_features(features).title(title).inner_size(1100.0, 800.0)
                        .initialization_script(&capabilities(&popup_app.state::<StartupState>()))
                        .on_navigation(move |url| allow_navigation(&child_app, url)).build() {
                        Ok(window) => tauri::webview::NewWindowResponse::Create { window },
                        Err(_) => tauri::webview::NewWindowResponse::Deny,
                    }
                }).build()?;
            if edition == LocalEdition::Debug || personal_launch {
                if let Err(error) = start_reader(app.handle(), None, None, false) { app.state::<StartupState>().log(&error); }
            }
            Ok(())
        }).build(context).expect("failed to create local Reader window");
    app.run(|handle, event| {
        if matches!(event, RunEvent::Exit) {
            if let Some(state) = handle.try_state::<DesktopServer>() {
                if let Some(server) = state.server.lock().unwrap().take() {
                    server.shutdown();
                    let _ = std::fs::remove_file(state.data_dir.join("debug-session.json"));
                }
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
        for url in ["about:blank", "http://127.0.0.1:49152/api/book/source"] { assert!(internal_navigation(&url.parse().unwrap(), origin)); }
        for url in ["https://doi.org/10.1000/test", "http://127.0.0.1:49153/", "file:///C:/test"] { assert!(!internal_navigation(&url.parse().unwrap(), origin)); }
    }
    #[test]
    fn provider_status_excludes_saved_secret() {
        let config = ProviderConfig::from_values("native", "secret-value", "https://model.example/v1", "model").unwrap();
        let status = serde_json::to_value(provider_status(Some(&config), "settings")).unwrap();
        assert_eq!(status["configured"], true);
        assert_eq!(status["api_key_configured"], true);
        assert!(!status.to_string().contains("secret-value"));
        assert!(!provider_status(None, "unconfigured").configured);
    }
}

