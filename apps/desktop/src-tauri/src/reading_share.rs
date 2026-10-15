/// Saves bytes already rendered and previewed by the reader to its chosen file.
#[tauri::command]
pub fn save_reading_share_image(path: std::path::PathBuf, bytes: Vec<u8>) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|error| format!("图片保存失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_preview_bytes_and_reports_write_failure() {
        let path = std::env::temp_dir().join(format!("reading-share-{}-{}.png", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let bytes = b"\x89PNG\r\n\x1a\npreview bytes".to_vec();
        save_reading_share_image(path.clone(), bytes.clone()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_file(path).unwrap();
        assert!(save_reading_share_image(std::env::temp_dir(), bytes).is_err());
    }
}
