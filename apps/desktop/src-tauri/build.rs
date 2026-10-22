fn main() {
    println!("cargo:rerun-if-env-changed=UNDERSTAND_BOOK_CLOUD_ORIGIN");
    let app_manifest = tauri_build::AppManifest::new().commands(&[
        "desktop_sign_in",
        "desktop_cancel_sign_in",
        "desktop_open_cloud",
        "desktop_open_local_library",
        "desktop_local_materials",
        "desktop_pdf_reading",
        "desktop_pdf_update",
        "desktop_import_local_material",
        "desktop_read_local_material_chunk",
        "desktop_local_material_cloud_copy",
        "desktop_remove_local_material",
        "desktop_open_local_material",
        "save_reading_share_image",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(app_manifest))
        .expect("failed to build Understand Book desktop application");
    // The host probe includes the actual desktop entry and needs the same Windows
    // common-controls manifest. tauri-build links this resource only to bin targets.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let resource =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("resource.lib");
        println!("cargo:rustc-link-arg-examples={}", resource.display());
    }
}
