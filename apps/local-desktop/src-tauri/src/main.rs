#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    local_desktop_host::run(tauri::generate_context!(), local_desktop_host::LocalEdition::Installed);
}
