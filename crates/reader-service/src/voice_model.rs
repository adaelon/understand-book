//! On-demand offline Whisper model management (download, SHA256 verify, directory selection, removal).
use read_tools::ToolError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const VOICE_MODEL_NAME: &str = "Systran/faster-whisper-small";
pub const VOICE_MODEL_REVISION: &str = "536b0662742c02347bc0e980a01041f333bce120";
pub const VOICE_MODEL_BIN_SIZE: u64 = 483_546_902;
pub const VOICE_MODEL_BIN_SHA256: &str =
    "3e305921506d8872816023e4c273e75d2419fb89b24da97b4fe7bce14170d671";
pub const DEFAULT_HF_BASE_URL: &str = "https://huggingface.co";
pub const HF_MIRROR_BASE_URL: &str = "https://hf-mirror.com";

const REQUIRED_MODEL_FILES: &[&str] = &["config.json", "tokenizer.json", "vocabulary.txt", "model.bin"];
const REMOVABLE_MODEL_FILES: &[&str] = &[
    "model.bin",
    "config.json",
    "tokenizer.json",
    "vocabulary.txt",
    "MODEL-CARD.md",
    "LICENSE-MIT.txt",
];

pub type SaveVoiceModelDir = Arc<dyn Fn(Option<PathBuf>) -> Result<(), String> + Send + Sync>;

#[derive(Clone, Debug)]
struct ModelSpec {
    model_name: String,
    revision: String,
    expected_bin_size: u64,
    expected_bin_sha256: String,
    download_notice: bool,
}

impl Default for ModelSpec {
    fn default() -> Self {
        Self {
            model_name: VOICE_MODEL_NAME.into(),
            revision: VOICE_MODEL_REVISION.into(),
            expected_bin_size: VOICE_MODEL_BIN_SIZE,
            expected_bin_sha256: VOICE_MODEL_BIN_SHA256.into(),
            download_notice: true,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DownloadProgress {
    pub state: String,
    pub current_file: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub target_dir: Option<String>,
    pub source: Option<String>,
    pub error: Option<String>,
}

impl Default for DownloadProgress {
    fn default() -> Self {
        Self {
            state: "idle".into(),
            current_file: None,
            downloaded_bytes: 0,
            total_bytes: VOICE_MODEL_BIN_SIZE,
            target_dir: None,
            source: None,
            error: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedVoice {
    pub python: PathBuf,
    pub model_dir: PathBuf,
    pub source: &'static str,
}

#[derive(Deserialize)]
pub struct DownloadRequest {
    pub target_dir: Option<PathBuf>,
    pub source: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Deserialize)]
pub struct SelectDirRequest {
    pub dir: PathBuf,
}

#[derive(Deserialize)]
pub struct RemoveModelRequest {
    #[serde(default = "default_true")]
    pub delete_files: bool,
}

fn default_true() -> bool {
    true
}

pub fn is_valid_model_dir(dir: &Path) -> bool {
    ["model.bin", "config.json", "tokenizer.json"].iter().all(|file| {
        fs::metadata(dir.join(file))
            .map(|meta| meta.is_file() && meta.len() > 0)
            .unwrap_or(false)
    })
}

pub fn bundled_python_path(runtime: Option<&Path>) -> Option<PathBuf> {
    let runtime = runtime?;
    let exe = runtime
        .join("python")
        .join(if cfg!(windows) { "python.exe" } else { "python" });
    exe.is_file().then_some(exe)
}

fn tool_err(code: &str, category: &str, message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: code.into(),
        category: category.into(),
        message: message.into(),
    }
}

struct Inner {
    configured_model_dir: Option<PathBuf>,
    save_callback: Option<SaveVoiceModelDir>,
    progress: DownloadProgress,
    cancel_flag: Option<Arc<AtomicBool>>,
}

pub struct VoiceModelManager {
    bundled_runtime: Option<PathBuf>,
    default_model_dir: Option<PathBuf>,
    stop_signal: Arc<AtomicBool>,
    spec: ModelSpec,
    inner: Mutex<Inner>,
}

impl VoiceModelManager {
    pub fn new(
        bundled_runtime: Option<PathBuf>,
        default_model_dir: Option<PathBuf>,
        configured_model_dir: Option<PathBuf>,
        stop_signal: Arc<AtomicBool>,
    ) -> Arc<Self> {
        Arc::new(Self {
            bundled_runtime,
            default_model_dir,
            stop_signal,
            spec: ModelSpec::default(),
            inner: Mutex::new(Inner {
                configured_model_dir,
                save_callback: None,
                progress: DownloadProgress::default(),
                cancel_flag: None,
            }),
        })
    }

    pub fn set_persistence(&self, save: SaveVoiceModelDir) {
        self.inner.lock().unwrap().save_callback = Some(save);
    }

    pub fn bundled_runtime(&self) -> Option<&Path> {
        self.bundled_runtime.as_deref()
    }

    pub fn runtime_available(&self) -> bool {
        bundled_python_path(self.bundled_runtime.as_deref()).is_some()
    }

    pub fn resolve(&self) -> Option<ResolvedVoice> {
        if let Some(env_model) = std::env::var_os("UNDERSTAND_BOOK_VOICE_MODEL_DIR").map(PathBuf::from) {
            let env_python = std::env::var_os("UNDERSTAND_BOOK_VOICE_PYTHON")
                .or_else(|| std::env::var_os("VIDEO_WHISPER_PYTHON"))
                .map(PathBuf::from);
            let python = env_python
                .or_else(|| bundled_python_path(self.bundled_runtime.as_deref()))
                .unwrap_or_else(|| PathBuf::from("python"));
            if (!python.is_absolute() || python.is_file()) && is_valid_model_dir(&env_model) {
                return Some(ResolvedVoice {
                    python,
                    model_dir: env_model,
                    source: "env",
                });
            }
        }

        let python = bundled_python_path(self.bundled_runtime.as_deref())?;
        let configured = self.inner.lock().unwrap().configured_model_dir.clone();
        if let Some(dir) = configured {
            if is_valid_model_dir(&dir) {
                return Some(ResolvedVoice {
                    python,
                    model_dir: dir,
                    source: "configured",
                });
            }
        }
        if let Some(dir) = &self.default_model_dir {
            if is_valid_model_dir(dir) {
                return Some(ResolvedVoice {
                    python,
                    model_dir: dir.clone(),
                    source: "default",
                });
            }
        }
        if let Some(runtime) = &self.bundled_runtime {
            let bundled_model = runtime.join("model");
            if is_valid_model_dir(&bundled_model) {
                return Some(ResolvedVoice {
                    python,
                    model_dir: bundled_model,
                    source: "bundled",
                });
            }
        }
        None
    }

    pub fn voice_status_json(&self) -> Value {
        let resolved = self.resolve();
        let runtime_available = self.runtime_available();
        json!({
            "available": resolved.is_some(),
            "max_seconds": 90,
            "runtime_available": runtime_available,
            "can_download": runtime_available && resolved.is_none(),
        })
    }

    pub fn status_json(&self) -> Value {
        let resolved = self.resolve();
        let runtime_available = self.runtime_available();
        let guard = self.inner.lock().unwrap();
        let configured = guard.configured_model_dir.as_ref().map(|p| p.display().to_string());
        let progress = guard.progress.clone();
        drop(guard);

        let source = resolved.as_ref().map(|r| r.source).unwrap_or("none");
        let model_dir = resolved
            .as_ref()
            .map(|r| r.model_dir.display().to_string());

        json!({
            "runtime_available": runtime_available,
            "available": resolved.is_some(),
            "can_download": runtime_available,
            "source": source,
            "model_dir": model_dir,
            "default_model_dir": self.default_model_dir.as_ref().map(|p| p.display().to_string()),
            "configured_model_dir": configured,
            "model_name": self.spec.model_name,
            "model_size_bytes": self.spec.expected_bin_size,
            "download": progress,
        })
    }

    pub fn select_dir(&self, req: SelectDirRequest) -> Result<Value, ToolError> {
        if !req.dir.is_absolute() {
            return Err(tool_err(
                "VOICE_MODEL_INVALID_DIR",
                "validation",
                "请选择完整的绝对路径目录。",
            ));
        }
        if !is_valid_model_dir(&req.dir) {
            return Err(tool_err(
                "VOICE_MODEL_INCOMPLETE",
                "validation",
                "所选目录缺少离线语音模型文件（需要 model.bin、config.json 和 tokenizer.json）。",
            ));
        }
        let save = {
            let guard = self.inner.lock().unwrap();
            if matches!(guard.progress.state.as_str(), "downloading" | "verifying") {
                return Err(tool_err(
                    "VOICE_MODEL_BUSY",
                    "conflict",
                    "正在下载语音模型，请等待完成或先取消下载。",
                ));
            }
            guard.save_callback.clone()
        };
        if let Some(persist) = save {
            persist(Some(req.dir.clone()))
                .map_err(|e| tool_err("VOICE_MODEL_SAVE_FAILED", "unavailable", e))?;
        }
        {
            let mut guard = self.inner.lock().unwrap();
            guard.configured_model_dir = Some(req.dir);
            guard.progress.error = None;
        }
        Ok(self.status_json())
    }

    pub fn remove_model(&self, req: RemoveModelRequest) -> Result<Value, ToolError> {
        let (target_dir, save) = {
            let guard = self.inner.lock().unwrap();
            if matches!(guard.progress.state.as_str(), "downloading" | "verifying") {
                return Err(tool_err(
                    "VOICE_MODEL_BUSY",
                    "conflict",
                    "正在下载语音模型，请先取消下载后再删除。",
                ));
            }
            let dir = guard
                .configured_model_dir
                .clone()
                .or_else(|| self.default_model_dir.clone());
            (dir, guard.save_callback.clone())
        };

        if req.delete_files {
            if let Some(dir) = target_dir {
                for name in REMOVABLE_MODEL_FILES {
                    let file = dir.join(name);
                    if file.is_file() {
                        fs::remove_file(&file).map_err(|e| {
                            tool_err(
                                "VOICE_MODEL_REMOVE_FAILED",
                                "unavailable",
                                format!("删除模型文件失败（{}）：{e}", file.display()),
                            )
                        })?;
                    }
                }
                let _ = fs::remove_dir(&dir);
            }
        }

        if let Some(persist) = save {
            persist(None).map_err(|e| tool_err("VOICE_MODEL_SAVE_FAILED", "unavailable", e))?;
        }
        {
            let mut guard = self.inner.lock().unwrap();
            guard.configured_model_dir = None;
            guard.progress = DownloadProgress {
                total_bytes: self.spec.expected_bin_size,
                ..DownloadProgress::default()
            };
        }
        Ok(self.status_json())
    }

    pub fn cancel_download(&self) -> Value {
        let mut guard = self.inner.lock().unwrap();
        if let Some(flag) = guard.cancel_flag.as_ref() {
            flag.store(true, Ordering::Release);
        }
        if matches!(guard.progress.state.as_str(), "downloading" | "verifying") {
            guard.progress.state = "cancelling".into();
        }
        drop(guard);
        self.status_json()
    }

    pub fn start_download(self: &Arc<Self>, req: DownloadRequest) -> Result<Value, ToolError> {
        if !self.runtime_available() {
            return Err(tool_err(
                "VOICE_RUNTIME_MISSING",
                "unavailable",
                "当前安装包缺少离线语音运行环境，请重新安装本地版应用。",
            ));
        }

        let (base_url, source_label) = resolve_download_origin(req.source.as_deref(), req.base_url.as_deref())?;

        let target_dir = {
            let guard = self.inner.lock().unwrap();
            if matches!(guard.progress.state.as_str(), "downloading" | "verifying" | "cancelling") {
                return Err(tool_err(
                    "VOICE_MODEL_BUSY",
                    "conflict",
                    "离线语音模型正在下载中，请稍候。",
                ));
            }
            req.target_dir
                .or_else(|| guard.configured_model_dir.clone())
                .or_else(|| self.default_model_dir.clone())
                .ok_or_else(|| {
                    tool_err(
                        "VOICE_MODEL_NO_TARGET",
                        "validation",
                        "请先选择离线语音模型的存放目录。",
                    )
                })?
        };

        if !target_dir.is_absolute() {
            return Err(tool_err(
                "VOICE_MODEL_INVALID_DIR",
                "validation",
                "模型存放目录必须是完整的绝对路径。",
            ));
        }
        ensure_writable_dir(&target_dir)?;

        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut guard = self.inner.lock().unwrap();
            guard.cancel_flag = Some(cancel_flag.clone());
            guard.progress = DownloadProgress {
                state: "downloading".into(),
                current_file: Some("config.json".into()),
                downloaded_bytes: 0,
                total_bytes: self.spec.expected_bin_size,
                target_dir: Some(target_dir.display().to_string()),
                source: Some(source_label.clone()),
                error: None,
            };
        }

        let manager = Arc::clone(self);
        thread::spawn(move || {
            manager.run_download(target_dir, base_url, cancel_flag);
        });

        Ok(self.status_json())
    }

    fn is_cancelled(&self, cancel_flag: &AtomicBool) -> bool {
        cancel_flag.load(Ordering::Acquire) || self.stop_signal.load(Ordering::Acquire)
    }

    fn run_download(&self, target_dir: PathBuf, base_url: String, cancel_flag: Arc<AtomicBool>) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let staging = target_dir.join(format!(".voice-model-downloading-{}-{nonce}", std::process::id()));
        if let Err(error) = fs::create_dir_all(&staging) {
            self.finish_failed(format!("无法创建模型临时目录（{}）：{error}", staging.display()));
            return;
        }

        let outcome = self.download_into_staging(&staging, &target_dir, &base_url, &cancel_flag);
        let _ = fs::remove_dir_all(&staging);

        match outcome {
            Ok(()) => {
                let save = self.inner.lock().unwrap().save_callback.clone();
                if let Some(persist) = save {
                    if let Err(error) = persist(Some(target_dir.clone())) {
                        self.finish_failed(format!("模型已下载，但保存路径设置失败：{error}"));
                        return;
                    }
                }
                let mut guard = self.inner.lock().unwrap();
                guard.configured_model_dir = Some(target_dir.clone());
                guard.cancel_flag = None;
                guard.progress.state = "completed".into();
                guard.progress.current_file = None;
                guard.progress.downloaded_bytes = guard.progress.total_bytes;
                guard.progress.error = None;
            }
            Err(DownloadAbort::Cancelled) => {
                let mut guard = self.inner.lock().unwrap();
                guard.cancel_flag = None;
                guard.progress.state = "cancelled".into();
                guard.progress.current_file = None;
                guard.progress.error = Some("已取消下载。".into());
            }
            Err(DownloadAbort::Failed(message)) => {
                self.finish_failed(message);
            }
        }
    }

    fn finish_failed(&self, message: String) {
        let mut guard = self.inner.lock().unwrap();
        guard.cancel_flag = None;
        guard.progress.state = "failed".into();
        guard.progress.current_file = None;
        guard.progress.error = Some(message);
    }

    fn download_into_staging(
        &self,
        staging: &Path,
        target_dir: &Path,
        base_url: &str,
        cancel_flag: &AtomicBool,
    ) -> Result<(), DownloadAbort> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(20))
            .timeout_read(Duration::from_secs(60))
            .build();

        let prefix = format!(
            "{}/{}/resolve/{}",
            base_url.trim_end_matches('/'),
            self.spec.model_name,
            self.spec.revision
        );

        let mut copied_files: Vec<String> = Vec::new();
        for &filename in REQUIRED_MODEL_FILES {
            if self.is_cancelled(cancel_flag) {
                return Err(DownloadAbort::Cancelled);
            }
            {
                let mut guard = self.inner.lock().unwrap();
                guard.progress.current_file = Some(filename.into());
            }
            let url = format!("{prefix}/{filename}");
            let dest = staging.join(filename);
            let is_bin = filename == "model.bin";
            let (written, digest) = self.fetch_file(&agent, &url, &dest, is_bin, cancel_flag)?;
            if is_bin {
                {
                    let mut guard = self.inner.lock().unwrap();
                    guard.progress.state = "verifying".into();
                }
                if written != self.spec.expected_bin_size {
                    return Err(DownloadAbort::Failed(format!(
                        "语音模型大小不匹配（预期 {} 字节，实际 {} 字节），请重试下载。",
                        self.spec.expected_bin_size, written
                    )));
                }
                if digest != self.spec.expected_bin_sha256 {
                    return Err(DownloadAbort::Failed(
                        "语音模型 SHA256 校验失败，请更换镜像源或重试下载。".into(),
                    ));
                }
            }
            copied_files.push(filename.into());
        }

        if self.spec.download_notice && !self.is_cancelled(cancel_flag) {
            let readme_url = format!("{prefix}/README.md");
            let notice_dest = staging.join("MODEL-CARD.md");
            if self
                .fetch_file(&agent, &readme_url, &notice_dest, false, cancel_flag)
                .is_ok()
            {
                copied_files.push("MODEL-CARD.md".into());
            }
        }

        if self.is_cancelled(cancel_flag) {
            return Err(DownloadAbort::Cancelled);
        }

        for filename in copied_files {
            let src = staging.join(&filename);
            let dst = target_dir.join(&filename);
            if dst.exists() {
                fs::remove_file(&dst).map_err(|e| {
                    DownloadAbort::Failed(format!("覆盖旧模型文件失败（{}）：{e}", dst.display()))
                })?;
            }
            fs::rename(&src, &dst).map_err(|e| {
                DownloadAbort::Failed(format!("移动模型文件失败（{}）：{e}", dst.display()))
            })?;
        }
        Ok(())
    }

    fn fetch_file(
        &self,
        agent: &ureq::Agent,
        url: &str,
        dest: &Path,
        track_progress: bool,
        cancel_flag: &AtomicBool,
    ) -> Result<(u64, String), DownloadAbort> {
        let response = agent.get(url).call().map_err(|error| {
            DownloadAbort::Failed(format!(
                "下载失败（{url}）：{error}。如果网络访问 HuggingFace 较慢，可切换为国内镜像源（hf-mirror.com）。"
            ))
        })?;

        if track_progress {
            if let Some(len) = response
                .header("Content-Length")
                .and_then(|v| v.parse::<u64>().ok())
            {
                if len > 0 {
                    self.inner.lock().unwrap().progress.total_bytes = len;
                }
            }
        }

        let mut reader = response.into_reader();
        let mut file = File::create(dest).map_err(|e| {
            DownloadAbort::Failed(format!("无法写入模型文件（{}）：{e}", dest.display()))
        })?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut total: u64 = 0;

        loop {
            if self.is_cancelled(cancel_flag) {
                return Err(DownloadAbort::Cancelled);
            }
            let read = reader.read(&mut buf).map_err(|e| {
                DownloadAbort::Failed(format!("读取下载流中断（{url}）：{e}"))
            })?;
            if read == 0 {
                break;
            }
            file.write_all(&buf[..read]).map_err(|e| {
                DownloadAbort::Failed(format!(
                    "写入模型文件失败（{}）：{e}。请检查目标磁盘剩余空间是否充足（约需 500 MB）。",
                    dest.display()
                ))
            })?;
            hasher.update(&buf[..read]);
            total += read as u64;
            if track_progress {
                self.inner.lock().unwrap().progress.downloaded_bytes = total;
            }
        }
        file.flush().map_err(|e| {
            DownloadAbort::Failed(format!("刷新模型文件到磁盘失败（{}）：{e}", dest.display()))
        })?;
        let digest = format!("{:x}", hasher.finalize());
        Ok((total, digest))
    }
}

enum DownloadAbort {
    Cancelled,
    Failed(String),
}

fn resolve_download_origin(
    source: Option<&str>,
    base_url: Option<&str>,
) -> Result<(String, String), ToolError> {
    if let Some(url) = base_url.map(str::trim).filter(|u| !u.is_empty()) {
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(tool_err(
                "VOICE_MODEL_INVALID_URL",
                "validation",
                "自定义镜像地址必须以 http:// 或 https:// 开头。",
            ));
        }
        return Ok((url.trim_end_matches('/').into(), "custom".into()));
    }
    match source.unwrap_or("huggingface") {
        "hf-mirror" => Ok((HF_MIRROR_BASE_URL.into(), "hf-mirror".into())),
        "huggingface" => Ok((DEFAULT_HF_BASE_URL.into(), "huggingface".into())),
        other => Err(tool_err(
            "VOICE_MODEL_INVALID_SOURCE",
            "validation",
            format!("不支持的模型下载源：{other}"),
        )),
    }
}

fn ensure_writable_dir(dir: &Path) -> Result<(), ToolError> {
    if dir.exists() && !dir.is_dir() {
        return Err(tool_err(
            "VOICE_MODEL_INVALID_DIR",
            "validation",
            format!("目标路径不是目录：{}", dir.display()),
        ));
    }
    fs::create_dir_all(dir).map_err(|e| {
        tool_err(
            "VOICE_MODEL_UNWRITABLE",
            "unavailable",
            format!("无法创建模型目录（{}）：{e}", dir.display()),
        )
    })?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let probe = dir.join(format!(".voice-write-probe-{}-{nonce}", std::process::id()));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|e| {
            tool_err(
                "VOICE_MODEL_UNWRITABLE",
                "unavailable",
                format!("模型目录不可写（{}）：{e}", dir.display()),
            )
        })?;
    let write_res = f.write_all(b"ok");
    drop(f);
    let _ = fs::remove_file(&probe);
    write_res.map_err(|e| {
        tool_err(
            "VOICE_MODEL_UNWRITABLE",
            "unavailable",
            format!("模型目录不可写（{}）：{e}", dir.display()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tiny_http::{Response, Server};

    fn fake_runtime(root: &Path) -> PathBuf {
        let runtime = root.join("voice-runtime");
        let py = runtime.join("python");
        fs::create_dir_all(&py).unwrap();
        let exe = py.join(if cfg!(windows) { "python.exe" } else { "python" });
        fs::write(&exe, b"fake-python").unwrap();
        runtime
    }

    fn write_valid_model(dir: &Path, bin_bytes: &[u8]) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("config.json"), b"{}").unwrap();
        fs::write(dir.join("tokenizer.json"), b"{}").unwrap();
        fs::write(dir.join("vocabulary.txt"), b"a\n").unwrap();
        fs::write(dir.join("model.bin"), bin_bytes).unwrap();
    }

    fn manager_with_spec(
        runtime: PathBuf,
        default_dir: PathBuf,
        bin_bytes: &[u8],
    ) -> Arc<VoiceModelManager> {
        let digest = format!("{:x}", Sha256::digest(bin_bytes));
        Arc::new(VoiceModelManager {
            bundled_runtime: Some(runtime),
            default_model_dir: Some(default_dir),
            stop_signal: Arc::new(AtomicBool::new(false)),
            spec: ModelSpec {
                model_name: VOICE_MODEL_NAME.into(),
                revision: VOICE_MODEL_REVISION.into(),
                expected_bin_size: bin_bytes.len() as u64,
                expected_bin_sha256: digest,
                download_notice: false,
            },
            inner: Mutex::new(Inner {
                configured_model_dir: None,
                save_callback: None,
                progress: DownloadProgress::default(),
                cancel_flag: None,
            }),
        })
    }

    fn start_fixture_server(files: HashMap<String, Vec<u8>>) -> (String, thread::JoinHandle<()>) {
        let server = Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr());
        let handle = thread::spawn(move || {
            for request in server.incoming_requests() {
                let path = request.url().to_string();
                if path == "/__stop" {
                    let _ = request.respond(Response::from_string("ok"));
                    break;
                }
                if let Some(body) = files.get(&path) {
                    let _ = request.respond(Response::from_data(body.clone()));
                } else {
                    let _ = request.respond(Response::empty(404));
                }
            }
        });
        (url, handle)
    }

    #[test]
    fn downloads_verifies_persists_and_removes_optional_model() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = fake_runtime(tmp.path());
        let default_dir = tmp.path().join("default-model");
        let custom_dir = tmp.path().join("custom-model");
        let bin_bytes = b"whisper-weights-fixture-payload".to_vec();
        let manager = manager_with_spec(runtime, default_dir, &bin_bytes);

        let persisted = Arc::new(Mutex::new(Vec::<Option<PathBuf>>::new()));
        let persisted_clone = persisted.clone();
        manager.set_persistence(Arc::new(move |dir| {
            persisted_clone.lock().unwrap().push(dir);
            Ok(())
        }));

        let initial = manager.voice_status_json();
        assert_eq!(initial["available"], false);
        assert_eq!(initial["runtime_available"], true);
        assert_eq!(initial["can_download"], true);

        let prefix = format!("/{VOICE_MODEL_NAME}/resolve/{VOICE_MODEL_REVISION}");
        let mut files = HashMap::new();
        files.insert(format!("{prefix}/config.json"), b"{}".to_vec());
        files.insert(format!("{prefix}/tokenizer.json"), b"{}".to_vec());
        files.insert(format!("{prefix}/vocabulary.txt"), b"hello\n".to_vec());
        files.insert(format!("{prefix}/model.bin"), bin_bytes.clone());
        let (base_url, handle) = start_fixture_server(files);

        manager
            .start_download(DownloadRequest {
                target_dir: Some(custom_dir.clone()),
                source: None,
                base_url: Some(base_url.clone()),
            })
            .unwrap();

        for _ in 0..100 {
            let status = manager.status_json();
            if status["download"]["state"] == "completed" {
                break;
            }
            assert_ne!(status["download"]["state"], "failed", "status={status}");
            thread::sleep(Duration::from_millis(20));
        }

        let after = manager.status_json();
        assert_eq!(after["download"]["state"], "completed");
        assert_eq!(after["available"], true);
        assert_eq!(after["source"], "configured");
        assert_eq!(manager.voice_status_json()["available"], true);
        assert_eq!(persisted.lock().unwrap().last(), Some(&Some(custom_dir.clone())));

        // Extra user file inside target dir must survive remove_model(delete_files=true).
        let keep_file = custom_dir.join("user-note.txt");
        fs::write(&keep_file, b"keep").unwrap();

        let removed = manager.remove_model(RemoveModelRequest { delete_files: true }).unwrap();
        assert_eq!(removed["available"], false);
        assert!(!custom_dir.join("model.bin").exists());
        assert!(keep_file.is_file());
        assert_eq!(persisted.lock().unwrap().last(), Some(&None));

        let _ = ureq::get(&format!("{base_url}/__stop")).call();
        handle.join().unwrap();
    }

    #[test]
    fn rejects_corrupted_model_hash_without_leaving_partial_files() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = fake_runtime(tmp.path());
        let default_dir = tmp.path().join("default-model");
        let expected = b"expected-good-weights".to_vec();
        let manager = manager_with_spec(runtime, default_dir.clone(), &expected);

        let prefix = format!("/{VOICE_MODEL_NAME}/resolve/{VOICE_MODEL_REVISION}");
        let mut files = HashMap::new();
        files.insert(format!("{prefix}/config.json"), b"{}".to_vec());
        files.insert(format!("{prefix}/tokenizer.json"), b"{}".to_vec());
        files.insert(format!("{prefix}/vocabulary.txt"), b"hello\n".to_vec());
        files.insert(format!("{prefix}/model.bin"), b"corrupted-bad-weights".to_vec());
        let (base_url, handle) = start_fixture_server(files);

        manager
            .start_download(DownloadRequest {
                target_dir: Some(default_dir.clone()),
                source: None,
                base_url: Some(base_url.clone()),
            })
            .unwrap();

        for _ in 0..100 {
            let status = manager.status_json();
            if status["download"]["state"] == "failed" {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        let after = manager.status_json();
        assert_eq!(after["download"]["state"], "failed");
        assert_eq!(after["available"], false);
        assert!(!default_dir.join("model.bin").exists());

        let _ = ureq::get(&format!("{base_url}/__stop")).call();
        handle.join().unwrap();
    }

    #[test]
    fn select_dir_validates_files_and_activates_existing_local_model() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = fake_runtime(tmp.path());
        let default_dir = tmp.path().join("default-model");
        let existing_dir = tmp.path().join("existing-model");
        let manager = manager_with_spec(runtime, default_dir, b"abc");

        let empty_dir = tmp.path().join("empty");
        fs::create_dir_all(&empty_dir).unwrap();
        assert!(manager.select_dir(SelectDirRequest { dir: empty_dir }).is_err());

        write_valid_model(&existing_dir, b"abc");
        let status = manager.select_dir(SelectDirRequest { dir: existing_dir.clone() }).unwrap();
        assert_eq!(status["available"], true);
        assert_eq!(status["source"], "configured");
        assert_eq!(manager.resolve().unwrap().model_dir, existing_dir);
    }
}
