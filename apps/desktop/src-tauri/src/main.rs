#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod cloud_auth;
mod local_materials;
mod reading_share;
use cloud_auth::CloudDesktop;
use reading_share::save_reading_share_image;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use local_materials::*;
use tauri::{ipc::CapabilityBuilder, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};
static CHILD_WINDOWS: AtomicUsize = AtomicUsize::new(1);

#[tauri::command]
fn desktop_set_language(app: tauri::AppHandle, state: tauri::State<'_, CloudDesktop>, language: String) -> Result<(), String> {
    let english = language == "en";
    *state.language.lock().unwrap() = if english { "en" } else { "zh-CN" }.into();
    if let Some(menu) = app.menu() {
        for (id, zh, en) in [("local-library", "本机书库", "Local library"), ("cloud-learning", "云端学习", "Cloud learning")] {
            if let Some(item) = menu.get(id) {
                if let Some(item) = item.as_menuitem() {
                    item.set_text(if english { en } else { zh }).map_err(|error| error.to_string())?;
                }
            }
        }
    }
    Ok(())
}

fn allow_navigation(url: &url::Url, origin: &str) -> bool {
    if url.origin().ascii_serialization() == origin
        || url.as_str() == "about:blank"
        || matches!(url.scheme(), "tauri" | "asset")
        || url.host_str() == Some("tauri.localhost")
    {
        true
    } else {
        if matches!(url.scheme(), "https" | "http") {
            let _ = cloud_auth::open_browser(url.as_str());
        }
        false
    }
}

#[tauri::command]
async fn desktop_sign_in(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || cloud_auth::sign_in(&app))
        .await
        .map_err(|_| "登录未完成，请重试。".to_string())?
}
#[tauri::command]
fn desktop_cancel_sign_in(state: tauri::State<'_, CloudDesktop>) {
    state.cancel();
}
#[tauri::command]
fn desktop_open_cloud(
    app: tauri::AppHandle,
    state: tauri::State<'_, CloudDesktop>,
) -> Result<(), String> {
    app.get_webview_window("main")
        .ok_or("阅读窗口已关闭")?
        .navigate(state.navigation_url(state.origin.clone()))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn desktop_open_local_library(app: tauri::AppHandle) -> Result<(), String> {
    let local_url = if cfg!(windows) { "http://tauri.localhost/" } else { "tauri://localhost/" };
    app.get_webview_window("main").ok_or("阅读窗口已关闭")?
        .navigate(app.state::<CloudDesktop>().navigation_url(local_url.parse().unwrap())).map_err(|error| error.to_string())
}

fn main() {
    if matches!(
        std::env::args().nth(1).as_deref(),
        Some("--codex-build-intent" | "--install-codex-plugin" | "--uninstall-owned-codex-plugin" | "--codex-plugin-status" | "--presentation-preview-probe")
    ) {
        eprintln!("This is the cloud Reader. Local maintenance commands require understand-book-maintenance.exe and explicitly provisioned operator tools.");
        std::process::exit(2);
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("local-material", |context, request, responder| {
            let app = context.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                let origin = app.state::<CloudDesktop>().origin.origin().ascii_serialization();
                let response = app.state::<Mutex<LocalMaterialStore>>().lock().unwrap().response(request, &origin);
                responder.respond(response);
            });
        })
        .invoke_handler(tauri::generate_handler![
            desktop_set_language,
            desktop_sign_in,
            desktop_cancel_sign_in,
            desktop_open_cloud,
            desktop_open_local_library,
            desktop_local_materials,
            desktop_pdf_reading,
            desktop_pdf_update,
            desktop_import_local_material,
            desktop_read_local_material_chunk,
            desktop_local_material_cloud_copy,
            desktop_remove_local_material,
            desktop_open_local_material,
            save_reading_share_image
        ])
        .setup(|app| {
            let origin = cloud_auth::configured_origin()?;
            let origin_string = origin.origin().ascii_serialization();
            app.add_capability(
                CapabilityBuilder::new("cloud-reader")
                    .windows(["main", "cloud-child-*"])
                    .remote(format!("{origin_string}/*"))
                    .permission("core:default")
                    .permission("dialog:allow-open")
                    .permission("dialog:allow-save")
                    .permission("allow-desktop-sign-in")
                    .permission("allow-desktop-set-language")
                    .permission("allow-desktop-cancel-sign-in")
                    .permission("allow-desktop-open-cloud")
                    .permission("allow-save-reading-share-image"),
            )?;
            app.manage(CloudDesktop::new(origin.clone()));
            let local_root = app.path().app_local_data_dir()?.join("local-materials");
            app.manage(Mutex::new(LocalMaterialStore::new(local_root.clone())?));
            app.add_capability(
                CapabilityBuilder::new("desktop-local-materials")
                    .windows(["main"])
                    .remote(format!("{origin_string}/*"))
                    .permission("allow-desktop-open-local-library")
                    .permission("allow-desktop-local-materials")
            .permission("allow-desktop-pdf-reading")
            .permission("allow-desktop-pdf-update")
                    .permission("allow-desktop-import-local-material")
                    .permission("allow-desktop-read-local-material-chunk")
                    .permission("allow-desktop-local-material-cloud-copy")
                    .permission("allow-desktop-remove-local-material")
                    .permission("allow-desktop-open-local-material"),
            )?;
            let menu = tauri::menu::Menu::with_items(app, &[
                &tauri::menu::MenuItem::with_id(app, "local-library", "本机书库", true, None::<&str>)?,
                &tauri::menu::MenuItem::with_id(app, "cloud-learning", "云端学习", true, None::<&str>)?,
            ])?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| {
                match event.id().as_ref() {
                    "local-library" => { let _ = desktop_open_local_library(app.clone()); }
                    "cloud-learning" => { let _ = desktop_open_cloud(app.clone(), app.state()); }
                    _ => {}
                }
            });
            let profile = app.path().app_local_data_dir()?.join("cloud-reader");
            std::fs::create_dir_all(&profile)?;
            let popup_app = app.handle().clone();
            let popup_origin = origin_string.clone();
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .initialization_script("window.__UNDERSTAND_BOOK_DESKTOP__ = { localMaterials: 1 };")
                .title("Understand Book")
                .inner_size(1440.0, 920.0)
                .min_inner_size(960.0, 640.0)
                .data_directory(profile)
                .on_navigation(move |url| allow_navigation(url, &origin_string))
                .on_new_window(move |url, features| {
                    if url.origin().ascii_serialization() != popup_origin {
                        if matches!(url.scheme(), "https" | "http") {
                            let _ = cloud_auth::open_browser(url.as_str());
                        }
                        return tauri::webview::NewWindowResponse::Deny;
                    }
                    let child_origin = popup_origin.clone();
                    // window_features keeps the opener and WebView2 environment, so the
                    // existing linked-presentation postMessage handshake and cookie apply.
                    match WebviewWindowBuilder::new(
                        &popup_app,
                        format!(
                            "cloud-child-{}",
                            CHILD_WINDOWS.fetch_add(1, Ordering::Relaxed)
                        ),
                        WebviewUrl::External("about:blank".parse().unwrap()),
                    )
                    .window_features(features)
                    .title("Understand Book")
                    .inner_size(1100.0, 800.0)
                    .on_navigation(move |url| allow_navigation(url, &child_origin))
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
        .expect("failed to build Understand Book cloud desktop");
    app.run(|handle, event| {
        if matches!(event, RunEvent::Exit) {
            if let Some(state) = handle.try_state::<CloudDesktop>() {
                state.cancel();
            }
        }
    });
}
