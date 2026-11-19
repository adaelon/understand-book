//! Explicit import of the data directories produced by the local Debug product.
//! A completed copy is a separate directory; neither source nor an existing target is merged.
use crate::{library_settings::LibrarySettingsStore, note_drafts::DraftMap};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

const ENTRIES: &[&str] = &["settings.json", ".env", "memory", "private", "personal-note-drafts.json", "plugin-installation.json"];
const RECEIPT: &str = "local-import.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub source: PathBuf,
    pub entries: Vec<String>,
    pub library_root: Option<PathBuf>,
    pub copies_library: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReceipt {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub state: String,
}

pub fn preview(source: &Path) -> Result<ImportPreview, String> {
    let source = portable_path(source.canonicalize().map_err(|e| format!("无法打开旧数据目录：{e}"))?);
    if !source.is_dir() { return Err("请选择旧本地应用的 data 目录。".into()); }
    ensure_stopped(&source)?;
    let entries = ENTRIES.iter().filter(|name| source.join(name).exists()).map(|name| (*name).to_owned()).collect::<Vec<_>>();
    if !entries.iter().any(|name| matches!(name.as_str(), "settings.json" | "memory" | "personal-note-drafts.json")) {
        return Err("此目录没有本项目的设置、阅读记录或草稿；请选择旧包内的 data 目录。".into());
    }
    let library_root = LibrarySettingsStore::new(source.join("settings.json")).load()?.map(portable_path);
    let copies_library = library_root.as_ref().is_some_and(|root| root.starts_with(&source));
    if let Some(root) = library_root.as_ref().filter(|_| copies_library) {
        if !root.is_dir() { return Err("旧应用自有书库不可用，请恢复该目录后再导入。".into()); }
    }
    if source.join("personal-note-drafts.json").exists() {
        let _: DraftMap = read_json(&source.join("personal-note-drafts.json"))?;
    }
    Ok(ImportPreview { source, entries, library_root, copies_library })
}

pub fn copy(source: &Path, destination: &Path) -> Result<ImportReceipt, String> {
    let preview = preview(source)?;
    if !destination.is_absolute() { return Err("导入目标必须是绝对路径。".into()); }
    if destination.starts_with(&preview.source) || preview.source.starts_with(destination) {
        return Err("请在旧数据目录之外选择独立的导入目标。".into());
    }
    if destination.exists() {
        if let Ok(receipt) = read_json::<ImportReceipt>(&destination.join(RECEIPT)) {
            if receipt.source == preview.source && receipt.destination == destination {
                return Ok(receipt); // Retry starts the completed copy, never copies over its records.
            }
        }
        return Err("目标目录已经存在。请继续使用当前资料，或选择一个新的独立目录；不会合并或覆盖记录。".into());
    }
    let parent = destination.parent().ok_or("导入目标缺少父目录")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let staging = parent.join(format!(".local-import-{}-{nonce}", std::process::id()));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let result = (|| {
        for name in &preview.entries { copy_directory_or_file(&preview.source.join(name), &staging.join(name))?; }
        if let Some(old_root) = preview.library_root.as_ref().filter(|_| preview.copies_library) {
            let relative = old_root.strip_prefix(&preview.source).map_err(|e| e.to_string())?;
            copy_directory_or_file(old_root, &staging.join(relative))?;
            let new_root = destination.join(relative);
            LibrarySettingsStore::new(staging.join("settings.json")).save_root(&new_root)?;
            relocate_session(&staging.join("memory/session.json"), &preview.source, destination)?;
            let registry = staging.join(relative).join("library-registry.json");
            if registry.exists() {
                let mut value: Value = read_json(&registry)?;
                if let Some(paths) = value["workspaces"].as_array_mut() {
                    for path in paths {
                        if let Some(text) = path.as_str() { *path = Value::String(relocate_path(text, &preview.source, destination)); }
                    }
                }
                write_json(&registry, &value)?;
            }
        }
        let receipt = ImportReceipt { source: preview.source.clone(), destination: destination.to_owned(), state: "copied".into() };
        write_json(&staging.join(RECEIPT), &receipt)?;
        fs::rename(&staging, destination).map_err(|e| format!("无法保存导入副本：{e}"))?;
        Ok(receipt)
    })();
    result.map_err(|error: String| format!("{error}\n原目录和已有目标均已保留；本次未完成的副本位于 {}。", staging.display()))
}

pub fn complete(destination: &Path) -> Result<(), String> {
    let path = destination.join(RECEIPT);
    if !path.exists() { return Ok(()); }
    let mut receipt: ImportReceipt = read_json(&path)?;
    if receipt.state != "complete" {
        receipt.state = "complete".into();
        write_json(&path, &receipt)?;
    }
    Ok(())
}

pub fn pending(destination: &Path) -> bool {
    read_json::<ImportReceipt>(&destination.join(RECEIPT)).is_ok_and(|receipt| receipt.state == "copied")
}

pub fn has_user_data(directory: &Path) -> bool {
    ["settings.json", "memory", "private", "personal-note-drafts.json"].iter().any(|name| directory.join(name).exists())
}

pub fn copy_directory_or_file(source: &Path, destination: &Path) -> Result<(), String> {
    if source.is_dir() {
        fs::create_dir_all(destination).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_directory_or_file(&entry.path(), &destination.join(entry.file_name()))?;
        }
    } else {
        if let Some(parent) = destination.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
        fs::copy(source, destination).map_err(|e| format!("无法复制 {}：{e}", source.display()))?;
    }
    Ok(())
}

fn relocate_path(value: &str, source: &Path, destination: &Path) -> String {
    portable_path(PathBuf::from(value)).strip_prefix(source).map(|part| destination.join(part).to_string_lossy().into_owned()).unwrap_or_else(|_| value.to_owned())
}

fn portable_path(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") { PathBuf::from(format!(r"\\{rest}")) }
    else if let Some(rest) = text.strip_prefix(r"\\?\") { PathBuf::from(rest) }
    else { path }
}

fn relocate_session(path: &Path, source: &Path, destination: &Path) -> Result<(), String> {
    if !path.exists() { return Ok(()); }
    let mut value: Value = read_json(path)?;
    for key in ["current_book_dir", "book_dir"] {
        if let Some(text) = value[key].as_str() { value[key] = Value::String(relocate_path(text, source, destination)); }
    }
    if let Some(books) = value["books"].as_object_mut() {
        *books = std::mem::take(books).into_iter().map(|(key, value)| (relocate_path(&key, source, destination), value)).collect();
    }
    write_json(path, &value)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    serde_json::from_slice(&fs::read(path).map_err(|e| format!("无法读取 {}：{e}", path.display()))?)
        .map_err(|e| format!("无法读取 {}：{e}", path.display()))
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(temporary, path).map_err(|e| e.to_string())
}

fn ensure_stopped(source: &Path) -> Result<(), String> {
    let Ok(session) = read_json::<Value>(&source.join("debug-session.json")) else { return Ok(()); };
    if session["pid"].as_u64().is_some_and(|pid| process_running(pid as u32)) {
        return Err("旧本地应用仍在运行。请关闭它的所有窗口后重试导入。".into());
    }
    Ok(())
}

#[cfg(windows)]
fn process_running(pid: u32) -> bool {
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
        fn GetExitCodeProcess(process: *mut std::ffi::c_void, code: *mut u32) -> i32;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }
    unsafe {
        let process = OpenProcess(0x1000, 0, pid);
        if process.is_null() { return false; }
        let mut code = 0;
        let active = GetExitCodeProcess(process, &mut code) != 0 && code == 259;
        CloseHandle(process);
        active
    }
}

#[cfg(unix)]
fn process_running(pid: u32) -> bool {
    extern "C" { fn kill(pid: i32, signal: i32) -> i32; }
    unsafe { kill(pid as i32, 0) == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn source(root: &Path, library: &Path) -> PathBuf {
        let source = root.join("old data");
        fs::create_dir_all(source.join("memory")).unwrap();
        fs::write(source.join("memory/memory.json"), "{\"kept\":true}").unwrap();
        fs::write(source.join("settings.json"), serde_json::to_vec(&json!({"schema":"understand_book.desktop_settings.v1", "library_root":library,
            "provider":{"mode":"native","api_key":"kept-key","base_url":"https://model.test/v1","model":"kept-model"}})).unwrap()).unwrap();
        fs::write(source.join("personal-note-drafts.json"), r#"{"d":{"draft_id":"d","owner":"local","value":{"content":"未保存"},"pending":{"operation_id":"saved-op"},"destination":{"kind":"note"}}}"#).unwrap();
        source
    }
    #[test]
    fn external_library_and_private_identity_survive_and_retry_never_overwrites() {
        let temp = tempfile::tempdir().unwrap();
        let library = temp.path().join("external library");
        fs::create_dir(&library).unwrap();
        let source = source(temp.path(), &library);
        fs::write(source.join("debug-session.json"), "{\"pid\":999999999,\"url\":\"http://127.0.0.1:9\"}").unwrap();
        let target = temp.path().join("new data");
        copy(&source, &target).unwrap();
        assert_eq!(LibrarySettingsStore::new(target.join("settings.json")).load().unwrap(), Some(library));
        assert!(!target.join("debug-session.json").exists());
        assert_eq!(fs::read(source.join("personal-note-drafts.json")).unwrap(), fs::read(target.join("personal-note-drafts.json")).unwrap());
        fs::write(target.join("memory/memory.json"), "new target state").unwrap();
        copy(&source, &target).unwrap();
        assert_eq!(fs::read_to_string(target.join("memory/memory.json")).unwrap(), "new target state");
        assert!(pending(&target)); complete(&target).unwrap(); assert!(!pending(&target));
    }
    #[test]
    fn owned_library_and_reading_position_move_together() {
        let temp = tempfile::tempdir().unwrap();
        let library = temp.path().join("old data/library/.understand-book");
        fs::create_dir_all(library.join("book")).unwrap();
        fs::write(library.join("book/base.json"), "original material identity").unwrap();
        let source = source(temp.path(), &library).canonicalize().unwrap();
        let book = library.join("book").to_string_lossy().into_owned();
        fs::write(source.join("memory/session.json"), serde_json::to_vec(&json!({"current_book_dir":book,"books":{book:{"top_lid":"1.2"}}})).unwrap()).unwrap();
        let target = temp.path().join("new data");
        copy(&source, &target).unwrap();
        assert_eq!(LibrarySettingsStore::new(target.join("settings.json")).load().unwrap(), Some(target.join("library/.understand-book")));
        let session: Value = read_json(&target.join("memory/session.json")).unwrap();
        assert_eq!(Path::new(session["current_book_dir"].as_str().unwrap()), target.join("library/.understand-book/book"));
        assert_eq!(session["books"].as_object().unwrap().values().next().unwrap()["top_lid"], "1.2");
    }
    #[test]
    fn refuses_running_source_and_existing_target_without_touching_either() {
        let temp = tempfile::tempdir().unwrap();
        let source = source(temp.path(), temp.path());
        let target = temp.path().join("existing"); fs::create_dir(&target).unwrap();
        fs::write(target.join("marker"), "kept").unwrap();
        assert!(copy(&source, &target).unwrap_err().contains("目标目录已经存在"));
        fs::write(source.join("debug-session.json"), serde_json::to_vec(&json!({"pid":std::process::id()})).unwrap()).unwrap();
        assert!(preview(&source).unwrap_err().contains("仍在运行"));
        assert_eq!(fs::read_to_string(target.join("marker")).unwrap(), "kept");
    }
}
