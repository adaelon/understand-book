fn main() {
    // Unoptimized startup restores the full local reader/history on the main thread.
    // The Windows default 1 MiB stack overflows after a persisted chat is restored.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-arg-bin=UnderstandBookLocalDebug=/STACK:16777216");
    }
    let manifest = tauri_build::AppManifest::new().commands(&[
        "desktop_set_language",
        "load_local_note_drafts",
        "write_local_note_draft",
        "local_debug_build_status",
        "codex_plugin_status",
        "install_codex_plugin",
        "set_desktop_library_directory",
        "desktop_provider_status",
        "save_desktop_provider_settings",
        "save_reading_share_image",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to build local debug host");
}
