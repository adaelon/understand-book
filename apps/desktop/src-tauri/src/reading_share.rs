/// Saves the image preview through the same local filesystem operation in both products.
#[tauri::command]
pub fn save_reading_share_image(path: std::path::PathBuf, bytes: Vec<u8>) -> Result<(), String> {
    desktop_support::reading_share::save_reading_share_image(path, bytes)
}
