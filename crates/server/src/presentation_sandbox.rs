//! Linux service authoring: one systemd cgroup and bubblewrap namespace per job.
//! Only a read-only task file crosses in; bounded JSON crosses out. No output mount.
use crate::service_limits::{Resource, Resources};
use read_tools::ToolError;
use runtime::{
    presentation::AnimationCue, presentation_author::PlotSize,
    presentation_preview::PreviewRequest, run_context::CancellationToken,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const MAX_INPUT: usize = 100 * 1024 * 1024;
const MAX_OUTPUT: usize = 40 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Job {
    Plot {
        code: String,
        data: serde_json::Value,
        size: Option<PlotSize>,
    },
    Animation {
        code: String,
        data: serde_json::Value,
        size: Option<PlotSize>,
        cues: Vec<AnimationCue>,
    },
    Preview {
        request: PreviewRequest,
    },
    Probe,
}
impl Job {
    fn resource(&self) -> Resource {
        match self {
            Self::Plot { .. } => Resource::Plot,
            Self::Animation { .. } => Resource::Animation,
            _ => Resource::Preview,
        }
    }
    fn seconds(&self) -> u64 {
        match self {
            Self::Animation { .. } => 185,
            Self::Preview { .. } | Self::Probe => 35,
            _ => 25,
        }
    }
    fn tasks(&self) -> usize {
        if matches!(self, Self::Preview { .. } | Self::Probe) {
            256
        } else {
            64
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    /// Dedicated instance name, used only to reclaim this service's old units.
    instance: String,
    worker: PathBuf,
    python_root: PathBuf,
    browser: PathBuf,
}
pub(crate) struct Sandbox {
    config: Option<Config>,
    ready: AtomicBool,
    reason: &'static str,
}
pub(crate) fn unavailable() -> ToolError {
    failure("PRESENTATION_SANDBOX_UNAVAILABLE")
}
fn failure(code: &str) -> ToolError {
    crate::user_storage_paths::error(
        code,
        "unavailable",
        "Restricted presentation execution is unavailable or failed",
    )
}
/// Generated filenames are never used as host paths beyond this one task directory.
pub(crate) fn output_file(dir: &Path, name: &str, limit: u64) -> Result<Vec<u8>, ToolError> {
    if name.is_empty() || name.contains(['/', '\\', ':']) || matches!(name, "." | "..") {
        return Err(failure("PRESENTATION_OUTPUT_INVALID"));
    }
    let path = dir.join(name);
    let meta =
        std::fs::symlink_metadata(&path).map_err(|_| failure("PRESENTATION_OUTPUT_INVALID"))?;
    if !meta.file_type().is_file() || meta.len() > limit {
        return Err(failure("PRESENTATION_OUTPUT_INVALID"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| failure("PRESENTATION_OUTPUT_INVALID"))?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| failure("PRESENTATION_OUTPUT_INVALID"))?;
    if bytes.len() as u64 > limit {
        return Err(failure("PRESENTATION_OUTPUT_LIMIT"));
    }
    Ok(bytes)
}
pub(crate) fn png(bytes: &[u8], width: u32, height: u32) -> Result<(), ToolError> {
    if bytes.len() < 24
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || &bytes[12..16] != b"IHDR"
        || u32::from_be_bytes(bytes[16..20].try_into().unwrap()) != width
        || u32::from_be_bytes(bytes[20..24].try_into().unwrap()) != height
    {
        return Err(failure("PRESENTATION_OUTPUT_INVALID"));
    }
    Ok(())
}
impl Sandbox {
    pub fn load(root: &Path) -> Self {
        let mut sandbox = Self {
            config: None,
            ready: AtomicBool::new(false),
            reason: "not_configured",
        };
        let bytes = match std::fs::read(root.join("presentation-sandbox.json")) {
            Ok(bytes) => bytes,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    sandbox.reason = "invalid_configuration";
                }
                return sandbox;
            }
        };
        let config: Config = match serde_json::from_slice(&bytes) {
            Ok(c) => c,
            Err(_) => {
                sandbox.reason = "invalid_configuration";
                return sandbox;
            }
        };
        if !cfg!(target_os = "linux") {
            sandbox.reason = "linux_required";
            return sandbox;
        }
        if config.instance.is_empty()
            || config.instance.len() > 32
            || !config
                .instance
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || !config.worker.is_absolute()
            || !config.worker.is_file()
            || !config.python_root.is_absolute()
            || !config.python_root.join("bin/python").is_file()
            || !config.browser.is_absolute()
            || !config.browser.is_file()
            || !(config.browser.starts_with("/usr")
                || config.browser.starts_with(&config.python_root))
        {
            sandbox.reason = "invalid_configuration";
            return sandbox;
        }
        sandbox.config = Some(config);
        sandbox.reason = "probe_failed";
        if sandbox.reclaim().is_ok()
            && sandbox
                .execute(&Job::Probe, &CancellationToken::default())
                .is_ok_and(|bytes| {
                    serde_json::from_slice::<serde_json::Value>(&bytes)
                        .ok()
                        .is_some_and(|v| v["isolated"] == true)
                })
        {
            sandbox.ready.store(true, Ordering::Release);
            sandbox.reason = "ready";
        }
        sandbox
    }
    pub fn capability(&self) -> serde_json::Value {
        let ready = self.ready.load(Ordering::Acquire);
        serde_json::json!({"authoring":ready,"plot":ready,"animation":ready,"preview":ready,
            "reason":if ready { "ready" } else if self.reason == "ready" { "execution_cleanup_failed" } else { self.reason }})
    }
    pub fn require(&self) -> Result<(), ToolError> {
        if self.ready.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(unavailable())
        }
    }
    fn command(program: &str) -> Command {
        let mut c = Command::new(program);
        c.env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C.UTF-8");
        c
    }
    fn reclaim(&self) -> Result<(), ToolError> {
        let config = self.config.as_ref().ok_or_else(unavailable)?;
        // A shared parent bounds the sum of preview, plot and animation jobs.
        if !Self::command("/usr/bin/systemctl")
            .args([
                "set-property",
                "--runtime",
                &format!("ub-render-{}.slice", config.instance),
                "MemoryMax=1200M",
                "MemorySwapMax=0",
                "CPUQuota=150%",
                "TasksMax=384",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| unavailable())?
            .success()
        {
            return Err(unavailable());
        }
        let output = Self::command("/usr/bin/systemctl")
            .args([
                "list-units",
                "--all",
                "--plain",
                "--no-legend",
                &format!("ub-render-{}-*.service", config.instance),
            ])
            .output()
            .map_err(|_| unavailable())?;
        if !output.status.success() {
            return Err(unavailable());
        }
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(unit) = line.split_whitespace().next() {
                self.stop(unit)?;
            }
        }
        let staging = Path::new("/var/tmp").join(format!("ub-render-{}", config.instance));
        if staging.exists() {
            std::fs::remove_dir_all(&staging).map_err(|_| unavailable())?;
        }
        std::fs::create_dir(&staging).map_err(|_| unavailable())?;
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o755))
                .map_err(|_| unavailable())?;
        }
        Ok(())
    }
    fn stop(&self, unit: &str) -> Result<(), ToolError> {
        let status = Self::command("/usr/bin/systemctl")
            .args(["stop", unit])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| unavailable())?;
        // Collected units no longer exist after normal exit; otherwise stop must succeed.
        if !status.success() {
            let active = Self::command("/usr/bin/systemctl")
                .args(["is-active", "--quiet", unit])
                .status()
                .map_err(|_| unavailable())?;
            if active.success() {
                self.ready.store(false, Ordering::Release);
                return Err(unavailable());
            }
        }
        Ok(())
    }
    fn execute(&self, job: &Job, cancellation: &CancellationToken) -> Result<Vec<u8>, ToolError> {
        let config = self.config.as_ref().ok_or_else(unavailable)?;
        cancellation.check()?;
        let bytes = serde_json::to_vec(job).map_err(|_| failure("PRESENTATION_INPUT_INVALID"))?;
        if bytes.len() > MAX_INPUT {
            return Err(failure("PRESENTATION_INPUT_LIMIT"));
        }
        let staging = Path::new("/var/tmp").join(format!("ub-render-{}", config.instance));
        let input = tempfile::Builder::new()
            .prefix("input-")
            .tempdir_in(staging)
            .map_err(|_| unavailable())?;
        let file = input.path().join("job.json");
        std::fs::write(&file, bytes).map_err(|_| unavailable())?;
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(input.path(), std::fs::Permissions::from_mode(0o755))
                .map_err(|_| unavailable())?;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444))
                .map_err(|_| unavailable())?;
        }
        let unit = format!(
            "ub-render-{}-{}.service",
            config.instance,
            uuid::Uuid::now_v7()
        );
        let mut command = Self::command("/usr/bin/systemd-run");
        command
            .args([
                "--quiet",
                "--wait",
                "--pipe",
                "--collect",
                "--service-type=exec",
                "--uid=nobody",
            ])
            .arg(format!("--unit={unit}"))
            .arg(format!("--slice=ub-render-{}.slice", config.instance));
        for property in [
            "MemoryMax=768M",
            "MemorySwapMax=0",
            "CPUQuota=100%",
            "KillMode=control-group",
            "TimeoutStopSec=2",
            "SendSIGKILL=yes",
            "NoNewPrivileges=yes",
            "LimitNOFILE=256",
            "UMask=0077",
            "OOMPolicy=kill",
            "TemporaryFileSystem=/tmp:rw,size=256M,mode=1777",
        ] {
            command.arg(format!("--property={property}"));
        }
        // Chromium's allocator uses sparse 16 GiB memfd shadows. RLIMIT_FSIZE
        // also limits memfd length, so it cannot bound preview disk use. tmpfs,
        // MemoryMax and the output pipe limit bound actual bytes independently.
        command.arg(if matches!(job, Job::Preview { .. } | Job::Probe) {
            "--property=LimitFSIZE=infinity"
        } else {
            "--property=LimitFSIZE=41943040"
        });
        let browser = config
            .browser
            .strip_prefix(&config.python_root)
            .map(|p| Path::new("/runtime").join(p))
            .unwrap_or_else(|_| config.browser.clone());
        command
            .arg(format!("--property=RuntimeMaxSec={}", job.seconds()))
            .arg(format!("--property=TasksMax={}", job.tasks()))
            .args([
                "/usr/bin/bwrap",
                "--unshare-all",
                "--die-with-parent",
                "--new-session",
                "--ro-bind",
                "/usr",
                "/usr",
                "--symlink",
                "usr/lib",
                "/lib",
                "--symlink",
                "usr/lib64",
                "/lib64",
                "--symlink",
                "usr/bin",
                "/bin",
                "--proc",
                "/proc",
                "--dev",
                "/dev",
                "--bind",
                "/tmp",
                "/work",
                "--symlink",
                "/work",
                "/tmp",
                "--dir",
                "/etc",
                "--ro-bind-try",
                "/etc/fonts",
                "/etc/fonts",
                "--ro-bind-try",
                "/etc/ld.so.cache",
                "/etc/ld.so.cache",
                "--ro-bind",
            ])
            .arg(&file)
            .arg("/input.json")
            .arg("--ro-bind")
            .arg(&config.worker)
            .arg("/worker")
            .arg("--ro-bind")
            .arg(&config.python_root)
            .arg("/runtime")
            .args([
                "--chdir",
                "/work",
                "--remount-ro",
                "/",
                "/usr/bin/env",
                "-i",
                "PATH=/runtime/bin:/usr/bin:/bin",
                "HOME=/work",
                "TMPDIR=/work",
                "LANG=C.UTF-8",
                "MPLCONFIGDIR=/work/mpl",
                "UNDERSTAND_BOOK_PLOT_PYTHON=/runtime/bin/python",
                "UNDERSTAND_BOOK_ANIMATION_PYTHON=/runtime/bin/python",
            ])
            .arg(format!(
                "UNDERSTAND_BOOK_PREVIEW_BROWSER={}",
                browser.display()
            ))
            .arg("/worker")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().map_err(|_| unavailable())?;
        let mut stdout = child.stdout.take().unwrap();
        let exceeded = Arc::new(AtomicBool::new(false));
        let too_big = exceeded.clone();
        let reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout
                .by_ref()
                .take((MAX_OUTPUT + 1) as u64)
                .read_to_end(&mut bytes);
            if bytes.len() > MAX_OUTPUT {
                too_big.store(true, Ordering::Release);
            }
            (result, bytes)
        });
        let deadline = Instant::now() + Duration::from_secs(job.seconds() + 5);
        let status = loop {
            if cancellation.is_cancelled()
                || exceeded.load(Ordering::Acquire)
                || Instant::now() >= deadline
            {
                self.stop(&unit)?;
                let _ = child.kill();
                break child.wait();
            }
            match child.try_wait() {
                Ok(Some(s)) => break Ok(s),
                Ok(None) => std::thread::sleep(Duration::from_millis(25)),
                Err(e) => {
                    self.stop(&unit)?;
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(e);
                }
            }
        };
        self.stop(&unit)?; // Includes descendants that outlive the worker.
        let (read, bytes) = reader.join().map_err(|_| unavailable())?;
        cancellation.check()?;
        if exceeded.load(Ordering::Acquire) {
            return Err(failure("PRESENTATION_OUTPUT_LIMIT"));
        }
        if Instant::now() >= deadline {
            return Err(failure("PRESENTATION_EXECUTION_TIMEOUT"));
        }
        if read.is_err() || !status.is_ok_and(|s| s.success()) {
            return Err(failure("PRESENTATION_EXECUTION_FAILED"));
        }
        Ok(bytes)
    }
}
pub(crate) struct Execution<'a> {
    pub sandbox: &'a Sandbox,
    pub resources: &'a Arc<Resources>,
    pub owner: &'a str,
}
impl Execution<'_> {
    pub fn run<T: DeserializeOwned>(
        &self,
        job: Job,
        cancellation: &CancellationToken,
    ) -> Result<T, ToolError> {
        self.sandbox.require()?;
        let _permit = loop {
            cancellation.check()?;
            if let Some(permit) = self.resources.try_acquire(job.resource(), self.owner) {
                break permit;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let bytes = self.sandbox.execute(&job, cancellation)?;
        serde_json::from_slice(&bytes).map_err(|_| failure("PRESENTATION_OUTPUT_INVALID"))
    }
}

/// Fixed executable inside the task namespace, never a public HTTP operation.
pub fn worker() -> Result<(), String> {
    use runtime::presentation_preview::PresentationPreviewPort;
    let file = std::fs::File::open("/input.json").map_err(|_| "input")?;
    let job: Job =
        serde_json::from_reader(file.take((MAX_INPUT + 1) as u64)).map_err(|_| "input")?;
    let cancel = CancellationToken::default();
    let output = match job {
        Job::Plot { code, data, size } => serde_json::to_vec(
            &crate::presentation_plot::render(code, data, size, &cancel)
                .map_err(|e| e.error_code)?,
        ),
        Job::Animation {
            code,
            data,
            size,
            cues,
        } => serde_json::to_vec(
            &crate::presentation_animation::render(code, data, size, cues, &cancel)
                .map_err(|e| e.error_code)?,
        ),
        Job::Preview { request } => serde_json::to_vec(
            &crate::presentation_preview::BrowserPreview::discover()?
                .preview(&request, &cancel)
                .map_err(|_| "preview")?,
        ),
        Job::Probe => {
            #[cfg(target_os = "linux")]
            if unsafe { libc::geteuid() } == 0 {
                return Err("identity".into());
            }
            if Path::new("/root").exists()
                || Path::new("/home").exists()
                || Path::new("/opt").exists()
                || Path::new("/run").exists()
                || std::env::var_os("OPENAI_API_KEY").is_some()
                || std::fs::write("/escape", "x").is_ok()
                || std::net::TcpStream::connect_timeout(
                    &"1.1.1.1:443".parse().unwrap(),
                    Duration::from_millis(200),
                )
                .is_ok()
            {
                return Err("isolation".into());
            }
            let deps = Command::new("/runtime/bin/python")
                .args([
                    "-c",
                    "import matplotlib,manim,av,PIL; assert manim.__version__ == '0.21.0'",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|_| "dependencies")?;
            if !deps.success() || crate::presentation_preview::BrowserPreview::discover().is_err() {
                return Err("dependencies".into());
            }
            let request = PreviewRequest {
                candidate_id: "probe".into(),
                html: "<p>Sandbox ready</p>".into(),
                actions: vec![],
                width: Some(400),
                viewport: None,
                read_selector: None,
            };
            let report = crate::presentation_preview::BrowserPreview::discover()?
                .preview(&request, &cancel)
                .map_err(|_| "browser_probe")?;
            if report.observations.is_empty() {
                return Err("browser_probe".into());
            }
            serde_json::to_vec(&serde_json::json!({"isolated":true}))
        }
    }
    .map_err(|_| "output")?;
    if output.len() > MAX_OUTPUT {
        return Err("output_limit".into());
    }
    use std::io::Write;
    std::io::stdout()
        .write_all(&output)
        .map_err(|_| "output".into())
}

#[cfg(all(test, target_os = "linux"))]
mod linux_tests {
    use super::*;
    fn sandbox() -> Sandbox {
        let root =
            std::env::var("MU7_SANDBOX_ROOT").expect("isolated acceptance configuration required");
        let sandbox = Sandbox::load(Path::new(&root));
        assert!(sandbox.require().is_ok(), "{}", sandbox.capability());
        sandbox
    }
    fn plot(code: &str) -> Job {
        Job::Plot {
            code: code.into(),
            data: serde_json::json!({}),
            size: Some(PlotSize {
                width: 400,
                height: 300,
            }),
        }
    }
    #[test]
    #[ignore = "Linux systemd/bubblewrap and dedicated MU7_SANDBOX_ROOT"]
    fn mu7_linux_files_credentials_network_and_renderers() {
        let sandbox = sandbox();
        let cancel = CancellationToken::default();
        let private = tempfile::tempdir().unwrap();
        let sentinel = private.path().join("other-user-secret");
        std::fs::write(&sentinel, "mu7-other-user-private").unwrap();
        let code = format!(
            "import os\nassert not os.path.exists({})\n",
            serde_json::to_string(&sentinel.to_str().unwrap()).unwrap()
        ) + r#"
import os, socket
assert os.geteuid() != 0
assert 'MU7_TEST_SECRET' not in os.environ
for path in ['/root', '/home', '/opt', '/run', '/var/tmp/ub-mu7-other-user', '/proc/1/root/opt']:
    assert not os.path.exists(path), path
for address in [('1.1.1.1',443), ('127.0.0.1',8787), ('169.254.169.254',80)]:
    s=socket.socket(); s.settimeout(.2)
    assert s.connect_ex(address) != 0
    s.close()
ax.plot([0,1],[0,1])
"#;
        let result = sandbox.execute(&plot(&code), &cancel).unwrap();
        let asset: crate::presentation_plot::PlotAsset = serde_json::from_slice(&result).unwrap();
        assert!(asset.svg.contains("<svg"));
        assert!(!String::from_utf8_lossy(&result).contains("mu7-test-secret"));
        assert!(!String::from_utf8_lossy(&result).contains("mu7-other-user-private"));
        let animation = sandbox.execute(&Job::Animation { code: "class PresentationAnimation(Scene):\n def construct(self):\n  self.add(Circle())\n  self.wait(0.2)".into(), data: serde_json::json!({}), size: Some(PlotSize {width: 400,height: 300}), cues: vec![] }, &cancel).unwrap();
        let animation: crate::presentation_animation::RenderedAnimation =
            serde_json::from_slice(&animation).unwrap();
        assert!(!animation.frames.is_empty());
        let preview = sandbox
            .execute(
                &Job::Preview {
                    request: PreviewRequest {
                        candidate_id: "fixture".into(),
                        html: "<h1>Sandbox preview</h1>".into(),
                        actions: vec![],
                        width: Some(400),
                        viewport: None,
                        read_selector: None,
                    },
                },
                &cancel,
            )
            .unwrap();
        let preview: runtime::presentation_preview::PreviewReport =
            serde_json::from_slice(&preview).unwrap();
        assert!(!preview.observations.is_empty());
    }
    #[test]
    #[ignore = "Linux resource exhaustion inside dedicated sandbox only"]
    fn mu7_linux_memory_pids_disk_output_and_symlink_bounds() {
        let mut sandbox = sandbox();
        let cancel = CancellationToken::default();
        assert!(sandbox
            .execute(&plot("x = bytearray(2 * 1024 * 1024 * 1024)"), &cancel)
            .is_err());
        // Catch the actual kernel quota, then finish a valid plot.
        let pid = sandbox.execute(
            &plot(
                r#"
import os, signal, time
children=[]
try:
    for i in range(100):
        pid=os.fork()
        if pid == 0:
            time.sleep(60); os._exit(0)
        children.append(pid)
    raise AssertionError('pids limit absent')
except OSError:
    pass
finally:
    for pid in children: os.kill(pid, signal.SIGKILL); os.waitpid(pid,0)
ax.plot([0,1])
"#,
            ),
            &cancel,
        );
        assert!(pid.is_ok(), "{:?}", pid.err());
        let disk = sandbox.execute(
            &plot(
                r#"
from pathlib import Path
import errno
try:
    for i in range(20): Path(f'fill-{i}').write_bytes(b'x' * (20*1024*1024))
    raise AssertionError('disk limit absent')
except OSError as e:
    assert e.errno == errno.ENOSPC
finally:
    for path in Path('.').glob('fill-*'): path.unlink()
ax.plot([0,1])
"#,
            ),
            &cancel,
        );
        assert!(disk.is_ok(), "{:?}", disk.err());
        assert!(sandbox
            .execute(
                &plot("import os\nos.symlink('/etc/passwd', 'plot.svg')\nos._exit(0)"),
                &cancel
            )
            .is_err());
        assert!(sandbox.execute(&plot("from pathlib import Path\nimport os\nPath('plot.svg').write_bytes(b'x' * (2*1024*1024))\nos._exit(0)"), &cancel).is_err());
        assert!(
            sandbox.execute(&plot("ax.plot([0,1])"), &cancel).is_ok(),
            "service remains usable"
        );
        // A separate hostile worker fixture tests the executor's outer pipe bound.
        // Renderer Python intentionally has stdout=/dev/null and cannot reopen
        // the root-owned systemd pipe through /proc, so it cannot test this path.
        use std::os::unix::fs::PermissionsExt;
        let fixture = tempfile::tempdir_in("/var/tmp").unwrap();
        std::fs::set_permissions(fixture.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        let worker = fixture.path().join("oversized-worker");
        std::fs::write(
            &worker,
            "#!/usr/bin/python3\nimport sys\nsys.stdout.buffer.write(b'x' * (45*1024*1024))\n",
        )
        .unwrap();
        std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o755)).unwrap();
        sandbox.config.as_mut().unwrap().worker = worker;
        assert_eq!(
            sandbox
                .execute(&plot("unused"), &cancel)
                .unwrap_err()
                .error_code,
            "PRESENTATION_OUTPUT_LIMIT"
        );
    }
    #[test]
    #[ignore = "Linux cancellation and unit recovery"]
    fn mu7_linux_cancel_timeout_and_restart_cleanup() {
        let sandbox = Arc::new(sandbox());
        let cancel = CancellationToken::default();
        let c = cancel.clone();
        let s = sandbox.clone();
        let run = std::thread::spawn(move || {
            s.execute(
                &plot("import os,time\nif os.fork()==0: time.sleep(120)\ntime.sleep(120)"),
                &c,
            )
        });
        std::thread::sleep(Duration::from_secs(3));
        cancel.cancel();
        assert_eq!(
            run.join().unwrap().unwrap_err().error_code,
            "AGENT_RUN_CANCELLED"
        );
        let s = sandbox.clone();
        let timed = std::thread::spawn(move || {
            s.execute(
                &plot("import os\nos.fork()\nwhile True: pass"),
                &CancellationToken::default(),
            )
        });
        std::thread::sleep(Duration::from_secs(3));
        let units = Sandbox::command("/usr/bin/systemctl")
            .args([
                "list-units",
                "--state=running",
                "--no-legend",
                &format!(
                    "ub-render-{}-*.service",
                    sandbox.config.as_ref().unwrap().instance
                ),
            ])
            .output()
            .unwrap();
        let text = String::from_utf8(units.stdout).unwrap();
        let unit = text.split_whitespace().next().expect("busy job unit");
        let group = Sandbox::command("/usr/bin/systemctl")
            .args(["show", unit, "--property=ControlGroup", "--value"])
            .output()
            .unwrap();
        let group = Path::new("/sys/fs/cgroup").join(
            String::from_utf8(group.stdout)
                .unwrap()
                .trim()
                .trim_start_matches('/'),
        );
        assert_eq!(
            std::fs::read_to_string(group.join("cpu.max"))
                .unwrap()
                .trim(),
            "100000 100000"
        );
        let stat = std::fs::read_to_string(group.join("cpu.stat")).unwrap();
        assert!(
            stat.lines()
                .find_map(|l| l.strip_prefix("nr_throttled "))
                .unwrap()
                .parse::<u64>()
                .unwrap()
                > 0,
            "{stat}"
        );
        assert!(timed.join().unwrap().is_err());
        let orphan = format!(
            "ub-render-{}-interrupted.service",
            sandbox.config.as_ref().unwrap().instance
        );
        assert!(Sandbox::command("/usr/bin/systemd-run")
            .args([
                "--quiet",
                "--collect",
                &format!("--unit={orphan}"),
                "--uid=nobody",
                "/usr/bin/sleep",
                "120"
            ])
            .status()
            .unwrap()
            .success());
        let staging = Path::new("/var/tmp").join(format!(
            "ub-render-{}",
            sandbox.config.as_ref().unwrap().instance
        ));
        std::fs::write(staging.join("interrupted-input.json"), "private-input").unwrap();
        // Reopening the same service root reclaims only its own units and staged input.
        let restarted = self::sandbox();
        assert!(!staging.join("interrupted-input.json").exists());
        assert!(restarted.require().is_ok());
        let output = Sandbox::command("/usr/bin/systemctl")
            .args([
                "list-units",
                "--state=running",
                "--no-legend",
                &format!(
                    "ub-render-{}-*.service",
                    sandbox.config.as_ref().unwrap().instance
                ),
            ])
            .output()
            .unwrap();
        assert!(output.stdout.is_empty(), "all task units must be gone");
    }
}
