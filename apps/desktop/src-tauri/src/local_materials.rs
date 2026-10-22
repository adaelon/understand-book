use base64::{engine::general_purpose::STANDARD, Engine};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{fs, io::{Read, Seek, SeekFrom}, path::{Path, PathBuf}, sync::Mutex};
use tauri::Manager;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MaterialScope {
    pub account_id: String,
    pub workspace_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CloudCopy {
    pub scope: MaterialScope,
    pub upload_id: String,
    pub task_id: String,
    pub operation_id: String,
    pub complete: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LocalMaterial {
    pub material_id: String,
    pub filename: String,
    pub bytes: u64,
    pub source_path: PathBuf,
    pub scope: MaterialScope,
    pub workspace_name: String,
    pub imported_at: u64,
    pub cloud_copy: Option<CloudCopy>,
}

pub struct LocalMaterialStore {
    root: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PdfAnnotation {
    pub id: String,
    pub page_index: u32,
    pub quote: String,
    pub rects: Vec<[f64; 4]>,
    pub color: String,
    pub note: String,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PdfReading {
    #[serde(default)]
    pub annotations: Vec<PdfAnnotation>,
    pub page_index: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PdfPageText {
    pub page_index: u32,
    pub text: String,
    pub ocr_lines: serde_json::Value,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PdfReadingUpdate {
    Page { page: PdfPageText },
    Annotation { annotation: PdfAnnotation },
    DeleteAnnotation { id: String },
    Position { page_index: u32 },
    SourceComplete,
}

fn failure(error: impl std::fmt::Display) -> String { format!("本机资料操作失败：{error}") }
fn new_id() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

impl LocalMaterialStore {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(failure)?;
        Ok(Self { root })
    }
    fn directory(&self, id: &str) -> Result<PathBuf, String> {
        if id.len() != 36 || !id.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-') {
            return Err("本机资料标识无效。".into());
        }
        Ok(self.root.join(id))
    }
    fn read(&self, id: &str) -> Result<LocalMaterial, String> {
        serde_json::from_slice(&fs::read(self.directory(id)?.join("material.json")).map_err(failure)?).map_err(failure)
    }
    fn save(&self, item: &LocalMaterial) -> Result<(), String> {
        let directory = self.directory(&item.material_id)?;
        let pending = directory.join("material.next.json");
        fs::write(&pending, serde_json::to_vec_pretty(item).map_err(failure)?).map_err(failure)?;
        fs::rename(pending, directory.join("material.json")).map_err(failure)
    }
    fn pdf_reading(&self, id: &str) -> Result<(PdfReading, Vec<PdfPageText>), String> {
        let item = self.read(id)?;
        if item.source_path.extension().and_then(|s| s.to_str()) != Some("pdf") { return Err("这份资料不是 PDF。".into()); }
        let dir = self.directory(id)?.join("reading");
        let state = if dir.join("annotations.json").is_file() {
            serde_json::from_slice(&fs::read(dir.join("annotations.json")).map_err(failure)?).map_err(failure)?
        } else { PdfReading::default() };
        let mut pages = Vec::<PdfPageText>::new();
        if dir.join("pages").is_dir() {
            for entry in fs::read_dir(dir.join("pages")).map_err(failure)? {
                let path = entry.map_err(failure)?.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    pages.push(serde_json::from_slice(&fs::read(path).map_err(failure)?).map_err(failure)?);
                }
            }
        }
        pages.sort_by_key(|page| page.page_index);
        Ok((state, pages))
    }
    fn pdf_update(&self, id: &str, update: PdfReadingUpdate) -> Result<(), String> {
        let item = self.read(id)?;
        if item.source_path.extension().and_then(|s| s.to_str()) != Some("pdf") { return Err("这份资料不是 PDF。".into()); }
        let dir = self.directory(id)?.join("reading");
        fs::create_dir_all(dir.join("pages")).map_err(failure)?;
        match update {
            PdfReadingUpdate::Page { page } => {
                let pending = dir.join("pages").join(format!("{}.next", page.page_index));
                fs::write(&pending, serde_json::to_vec(&page).map_err(failure)?).map_err(failure)?;
                fs::rename(pending, dir.join("pages").join(format!("{}.json", page.page_index))).map_err(failure)
            }
            PdfReadingUpdate::SourceComplete => {
                let (_, pages) = self.pdf_reading(id)?;
                let source = pages.iter().map(|page| page.text.as_str()).collect::<Vec<_>>().join("\n\n");
                let pending = self.directory(id)?.join("source.next.txt");
                fs::write(&pending, source).map_err(failure)?;
                fs::rename(pending, self.directory(id)?.join("source.txt")).map_err(failure)
            }
            change => {
                let file = dir.join("annotations.json");
                let mut state: PdfReading = if file.is_file() { serde_json::from_slice(&fs::read(&file).map_err(failure)?).map_err(failure)? } else { PdfReading::default() };
                match change {
                    PdfReadingUpdate::Annotation { annotation } => {
                        if annotation.quote.trim().is_empty() || annotation.rects.is_empty()
                            || !annotation.rects.iter().all(|r| r.iter().all(|v| v.is_finite()) && r[2] > r[0] && r[3] > r[1]) {
                            return Err("请选择 PDF 中的一段文字。".into());
                        }
                        state.annotations.retain(|a| a.id != annotation.id);
                        state.annotations.push(annotation);
                    }
                    PdfReadingUpdate::DeleteAnnotation { id } => state.annotations.retain(|a| a.id != id),
                    PdfReadingUpdate::Position { page_index } => state.page_index = page_index,
                    _ => unreachable!(),
                }
                let pending = dir.join("annotations.next");
                fs::write(&pending, serde_json::to_vec(&state).map_err(failure)?).map_err(failure)?;
                fs::rename(pending, file).map_err(failure)
            }
        }
    }
    fn list(&self, scope: Option<&MaterialScope>) -> Result<Vec<LocalMaterial>, String> {
        let mut items = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(failure)? {
            let entry = entry.map_err(failure)?;
            let metadata = entry.path().join("material.json");
            if !metadata.is_file() { continue; }
            let item: LocalMaterial = serde_json::from_slice(&fs::read(metadata).map_err(failure)?).map_err(failure)?;
            if scope.is_none_or(|scope| item.scope == *scope || item.scope == MaterialScope::default()) {
                items.push(item);
            }
        }
        items.sort_by(|a, b| b.imported_at.cmp(&a.imported_at).then(a.filename.cmp(&b.filename)));
        Ok(items)
    }
    fn import(&self, source: &Path, scope: MaterialScope, workspace_name: String) -> Result<LocalMaterial, String> {
        let filename = source.file_name().and_then(|n| n.to_str()).ok_or("请选择一份资料。")?.to_string();
        let extension = source.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        if !["epub", "md", "txt", "pdf", "mp4", "ppt", "pptx"].contains(&extension.as_str()) {
            return Err("支持 EPUB、Markdown、TXT、PDF、MP4 和 PowerPoint 课件。".into());
        }
        let metadata = fs::metadata(source).map_err(failure)?;
        if !metadata.is_file() || metadata.len() == 0 { return Err("请选择非空文件。".into()); }
        let material_id = new_id();
        let directory = self.directory(&material_id)?;
        fs::create_dir(&directory).map_err(failure)?;
        let item = LocalMaterial {
            material_id, filename, bytes: metadata.len(), source_path: directory.join(format!("source.{extension}")),
            scope, workspace_name, imported_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
            cloud_copy: None,
        };
        // Copy independently of the original; moving/deleting the selected file cannot
        // remove the library copy. No cloud API or entitlement is involved here.
        let result = fs::copy(source, &item.source_path).map_err(failure).and_then(|_| self.save(&item));
        if let Err(error) = result {
            let _ = fs::remove_dir_all(directory);
            return Err(error);
        }
        Ok(item)
    }
    fn chunk(&self, id: &str, offset: u64, length: u64) -> Result<String, String> {
        let item = self.read(id)?;
        if length == 0 || length > 32 * 1024 || offset >= item.bytes { return Err("读取范围无效。".into()); }
        let mut file = fs::File::open(item.source_path).map_err(failure)?;
        file.seek(SeekFrom::Start(offset)).map_err(failure)?;
        let mut bytes = vec![0; length.min(item.bytes - offset) as usize];
        file.read_exact(&mut bytes).map_err(failure)?;
        Ok(STANDARD.encode(bytes))
    }
    fn cloud_copy(&self, id: &str, scope: MaterialScope, complete: bool, restart: bool) -> Result<LocalMaterial, String> {
        let mut item = self.read(id)?;
        if restart || item.cloud_copy.as_ref().is_none_or(|c| c.scope != scope) {
            item.cloud_copy = Some(CloudCopy { scope, upload_id: new_id(), task_id: new_id(), operation_id: new_id(), complete: false });
        }
        item.cloud_copy.as_mut().unwrap().complete = complete;
        self.save(&item)?;
        Ok(item)
    }
    pub fn response(&self, request: tauri::http::Request<Vec<u8>>, cloud_origin: &str) -> tauri::http::Response<Vec<u8>> {
        use tauri::http::Response;
        let origin = request.headers().get("origin").and_then(|s| s.to_str().ok()).unwrap_or(cloud_origin);
        if ![cloud_origin, "http://tauri.localhost", "https://tauri.localhost", "tauri://localhost"].contains(&origin) {
            return Response::builder().status(403).body(Vec::new()).unwrap();
        }
        let response = Response::builder().header("Access-Control-Allow-Origin", origin)
            .header("Access-Control-Expose-Headers", "Content-Range, Content-Length");
        let Some(id) = request.uri().path().strip_prefix('/') else { return response.status(404).body(Vec::new()).unwrap(); };
        let Ok(item) = self.read(id) else { return response.status(404).body(Vec::new()).unwrap(); };
        let extension = item.source_path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let mime = match extension { "pdf" => "application/pdf", "mp4" => "video/mp4", "epub" => "application/epub+zip", _ => "application/octet-stream" };
        let response = response.header("Content-Type", mime).header("Accept-Ranges", "bytes");
        let range = request.headers().get("range").and_then(|s| s.to_str().ok());
        let (start, end, status) = if let Some(range) = range {
            let parsed = range.strip_prefix("bytes=").and_then(|r| r.split_once('-')).and_then(|(a,b)| {
                let start = a.parse::<u64>().ok()?;
                let end = if b.is_empty() { item.bytes - 1 } else { b.parse::<u64>().ok()?.min(item.bytes - 1) };
                (start < item.bytes && start <= end).then_some((start, end.min(start.saturating_add(1024 * 1024 - 1))))
            });
            let Some((start, end)) = parsed else { return response.status(416).header("Content-Range", format!("bytes */{}", item.bytes)).body(Vec::new()).unwrap(); };
            (start, end, 206)
        } else { (0, item.bytes - 1, 200) };
        let response = if status == 206 { response.header("Content-Range", format!("bytes {start}-{end}/{}", item.bytes)) } else { response };
        let read = || -> std::io::Result<Vec<u8>> {
            let mut file = fs::File::open(item.source_path)?;
            file.seek(SeekFrom::Start(start))?;
            let mut bytes = Vec::new(); file.take(end - start + 1).read_to_end(&mut bytes)?; Ok(bytes)
        };
        match read() {
            Ok(bytes) => response.status(status).header("Content-Length", bytes.len()).body(bytes).unwrap(),
            Err(_) => response.status(404).body(Vec::new()).unwrap(),
        }
    }
}

#[tauri::command]
pub fn desktop_local_materials(state: tauri::State<'_, Mutex<LocalMaterialStore>>, scope: Option<MaterialScope>) -> Result<Vec<LocalMaterial>, String> {
    state.lock().map_err(failure)?.list(scope.as_ref())
}

#[tauri::command]
pub async fn desktop_import_local_material(app: tauri::AppHandle, path: PathBuf, scope: MaterialScope, workspace_name: String) -> Result<LocalMaterial, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Mutex<LocalMaterialStore>>().lock().map_err(failure)?.import(&path, scope, workspace_name))
        .await.map_err(failure)?
}

#[tauri::command]
pub fn desktop_read_local_material_chunk(state: tauri::State<'_, Mutex<LocalMaterialStore>>, material_id: String, offset: u64, length: u64) -> Result<String, String> {
    state.lock().map_err(failure)?.chunk(&material_id, offset, length)
}

#[tauri::command]
pub fn desktop_local_material_cloud_copy(state: tauri::State<'_, Mutex<LocalMaterialStore>>, material_id: String, scope: MaterialScope, complete: bool, restart: bool) -> Result<LocalMaterial, String> {
    state.lock().map_err(failure)?.cloud_copy(&material_id, scope, complete, restart)
}

#[tauri::command]
pub fn desktop_remove_local_material(state: tauri::State<'_, Mutex<LocalMaterialStore>>, material_id: String) -> Result<(), String> {
    let store = state.lock().map_err(failure)?;
    store.read(&material_id)?;
    fs::remove_dir_all(store.directory(&material_id)?).map_err(failure)
}

#[tauri::command]
pub fn desktop_open_local_material(state: tauri::State<'_, Mutex<LocalMaterialStore>>, material_id: String) -> Result<(), String> {
    let item = state.lock().map_err(failure)?.read(&material_id)?;
    crate::cloud_auth::open_browser(&item.source_path.to_string_lossy())
}

#[tauri::command]
pub fn desktop_pdf_reading(state: tauri::State<'_, Mutex<LocalMaterialStore>>, material_id: String) -> Result<serde_json::Value, String> {
    let (reading, pages) = state.lock().map_err(failure)?.pdf_reading(&material_id)?;
    Ok(serde_json::json!({ "annotations": reading.annotations, "page_index": reading.page_index, "pages": pages }))
}

#[tauri::command]
pub fn desktop_pdf_update(state: tauri::State<'_, Mutex<LocalMaterialStore>>, material_id: String, update: PdfReadingUpdate) -> Result<(), String> {
    state.lock().map_err(failure)?.pdf_update(&material_id, update)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pdf_text_and_annotations_survive_reopen_and_background_page_writes() {
        let root = std::env::temp_dir().join(format!("ub-pdf-{}", new_id()));
        fs::create_dir_all(&root).unwrap();
        let original = root.join("notes.pdf"); fs::write(&original, b"%PDF-1.4 test").unwrap();
        let store = LocalMaterialStore::new(root.join("library")).unwrap();
        let item = store.import(&original, MaterialScope::default(), "Library".into()).unwrap();
        let id = &item.material_id;
        store.pdf_update(id, PdfReadingUpdate::Annotation { annotation: PdfAnnotation {
            id: "mark-1".into(), page_index: 0, quote: "Native text".into(), rects: vec![[30.0, 100.0, 180.0, 115.0]], color: "#f0c34e".into(), note: "Review this".into(),
        }}).unwrap();
        for (index, text) in [(1, "OCR text"), (0, "Native text")] {
            store.pdf_update(id, PdfReadingUpdate::Page { page: PdfPageText { page_index: index, text: text.into(), ocr_lines: serde_json::json!([]) } }).unwrap();
        }
        store.pdf_update(id, PdfReadingUpdate::Position { page_index: 1 }).unwrap();
        store.pdf_update(id, PdfReadingUpdate::SourceComplete).unwrap();
        drop(store);
        let reopened = LocalMaterialStore::new(root.join("library")).unwrap();
        let (state, pages) = reopened.pdf_reading(id).unwrap();
        assert_eq!(state.page_index, 1); assert_eq!(state.annotations.len(), 1);
        assert_eq!(state.annotations[0].note, "Review this"); assert_eq!(pages.len(), 2);
        assert_eq!(fs::read_to_string(reopened.directory(id).unwrap().join("source.txt")).unwrap(), "Native text\n\nOCR text");
        reopened.pdf_update(id, PdfReadingUpdate::DeleteAnnotation { id: "mark-1".into() }).unwrap();
        assert!(reopened.pdf_reading(id).unwrap().0.annotations.is_empty());
        assert_eq!(fs::read(&original).unwrap(), fs::read(&item.source_path).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn import_is_durable_local_only_and_scoped_with_no_subscription() {
        let root = std::env::temp_dir().join(format!("ub-local-{}", new_id()));
        fs::create_dir_all(&root).unwrap();
        let original = root.join("期末考纲.txt");
        fs::write(&original, "考试范围：微积分").unwrap();
        let scope = MaterialScope { account_id: "student".into(), workspace_id: "exam".into() };
        let store = LocalMaterialStore::new(root.join("library")).unwrap();
        let item = store.import(&original, scope.clone(), "考试冲刺".into()).unwrap();
        fs::remove_file(&original).unwrap();
        drop(store);
        let store = LocalMaterialStore::new(root.join("library")).unwrap();
        let restored = store.list(Some(&scope)).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(fs::read_to_string(&restored[0].source_path).unwrap(), "考试范围：微积分");
        assert!(restored[0].cloud_copy.is_none());
        assert_eq!(STANDARD.decode(store.chunk(&item.material_id, 0, 32768).unwrap()).unwrap(), "考试范围：微积分".as_bytes());
        assert!(store.list(Some(&MaterialScope { account_id: "other".into(), workspace_id: "exam".into() })).unwrap().is_empty());
        assert_eq!(store.list(None).unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cloud_copy_resume_keeps_ids_and_never_replaces_local_source() {
        let root = std::env::temp_dir().join(format!("ub-local-{}", new_id()));
        fs::create_dir_all(&root).unwrap();
        let original = root.join("notes.md"); fs::write(&original, "# Notes").unwrap();
        let store = LocalMaterialStore::new(root.join("library")).unwrap();
        let scope = MaterialScope { account_id: "student".into(), workspace_id: "exam".into() };
        let item = store.import(&original, MaterialScope::default(), "本机书库".into()).unwrap();
        assert_eq!(store.list(Some(&scope)).unwrap().len(), 1);
        let started = store.cloud_copy(&item.material_id, scope.clone(), false, false).unwrap();
        let done = store.cloud_copy(&item.material_id, scope.clone(), true, false).unwrap();
        assert_eq!(started.cloud_copy.unwrap().upload_id, done.cloud_copy.unwrap().upload_id);
        assert_eq!(fs::read_to_string(&item.source_path).unwrap(), "# Notes");
        assert_eq!(store.list(Some(&scope)).unwrap().len(), 1);
        assert!(store.directory("../../other").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn local_reader_serves_ranges_from_both_bundled_and_cloud_pages() {
        let root = std::env::temp_dir().join(format!("ub-local-{}", new_id()));
        fs::create_dir_all(&root).unwrap();
        let original = root.join("notes.pdf"); fs::write(&original, b"%PDF-1.4 fixture").unwrap();
        let store = LocalMaterialStore::new(root.join("library")).unwrap();
        let item = store.import(&original, MaterialScope::default(), "Library".into()).unwrap();
        for origin in ["http://tauri.localhost", "https://understandbook.top"] {
            let request = tauri::http::Request::builder().uri(format!("http://local-material.localhost/{}", item.material_id)).header("Origin", origin).header("Range", "bytes=0-3").body(Vec::new()).unwrap();
            let response = store.response(request, "https://understandbook.top");
            assert_eq!(response.status(), 206);
            assert_eq!(response.headers()["Access-Control-Allow-Origin"], origin);
            assert_eq!(response.headers()["Content-Range"], "bytes 0-3/16");
            assert_eq!(response.body(), b"%PDF");
        }
        fs::remove_dir_all(root).unwrap();
    }
}
