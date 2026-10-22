//! Uses the real native library and bundled reader in an isolated, hidden WebView2 profile.
#[allow(dead_code)]
#[path = "../src/cloud_auth.rs"] mod cloud_auth;
#[path = "../src/local_materials.rs"] mod local_materials;
use local_materials::*;
use std::sync::Mutex;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
#[tauri::command]
fn desktop_open_cloud() -> Result<(), String> { Err("The local-library probe never connects to production.".into()) }
#[tauri::command]
fn desktop_sign_in() -> Result<(), String> { Err("The local-library probe has no account.".into()) }
#[tauri::command]
fn desktop_cancel_sign_in() {}
#[tauri::command]
fn desktop_open_local_library() {}
#[tauri::command]
fn save_reading_share_image() {}
fn main() {
    let directory = std::path::PathBuf::from(std::env::args().nth(1).expect("isolated probe directory"));
    assert!(directory.is_absolute());
    std::fs::create_dir_all(&directory).unwrap();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("local-material", |context, request, responder| {
            let app = context.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                let response = app.state::<Mutex<LocalMaterialStore>>().lock().unwrap().response(request, "https://understandbook.top");
                responder.respond(response);
            });
        })
        .invoke_handler(tauri::generate_handler![desktop_local_materials, desktop_import_local_material,
            desktop_read_local_material_chunk, desktop_local_material_cloud_copy, desktop_remove_local_material,
            desktop_open_local_material, desktop_pdf_reading, desktop_pdf_update,
            desktop_open_cloud, desktop_sign_in, desktop_cancel_sign_in,
            desktop_open_local_library, save_reading_share_image])
        .setup(move |app| {
            app.manage(Mutex::new(LocalMaterialStore::new(directory.join("library"))?));
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .visible(false).inner_size(1280., 900.).data_directory(directory.join("profile"))
                .initialization_script("window.__UNDERSTAND_BOOK_DESKTOP__ = { localMaterials: 1 };")
                .build()?;
            Ok(())
        }).run(tauri::generate_context!()).unwrap();
}
