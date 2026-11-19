fn main() {
    local_desktop_host::run(tauri::generate_context!(), local_desktop_host::LocalEdition::Debug);
}
