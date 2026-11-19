include!("../../../crates/local-desktop-host/commands.rs");
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-arg-bin=UnderstandBookLocal=/STACK:16777216");
    }
    println!("cargo:rerun-if-changed=../../../crates/local-desktop-host/commands.rs");
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to build local desktop host");
}
