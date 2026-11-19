use reader_service::local_profile::{active_url, installed_data_home, resolve_data_root};
use serde_json::{json, Value};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn data_dir() -> Result<PathBuf, String> {
    resolve_data_root(&installed_data_home()?)
}

fn origin(raw: &str) -> Result<String, String> {
    let url = url::Url::parse(raw).map_err(|e| e.to_string())?;
    let local = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain("localhost")) => true,
        _ => false,
    };
    if !local
        || url.scheme() != "http"
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("The personal Reader address must be an HTTP loopback origin".into());
    }
    Ok(url.origin().ascii_serialization())
}

fn desktop_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let resources = exe.parent()?.parent()?;
    #[cfg(windows)]
    let desktop = resources.join("UnderstandBookLocal.exe");
    #[cfg(target_os = "macos")]
    let desktop = resources.parent()?.join("MacOS/UnderstandBookLocal");
    #[cfg(not(any(windows, target_os = "macos")))]
    let desktop = resources.join("UnderstandBookLocal");
    desktop.is_file().then_some(desktop)
}

fn reader_origin(root: &Path) -> Result<String, String> {
    if let Ok(url) = std::env::var("UNDERSTAND_BOOK_READER_URL") {
        return origin(&url);
    }
    if let Some(url) = active_url(root)? {
        return origin(&url);
    }
    if let Some(desktop) = desktop_binary() {
        let mut command = Command::new(desktop);
        command
            .arg("--personal-connection")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
            .spawn()
            .map_err(|e| format!("Cannot start the local Reader: {e}"))?;
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if let Some(url) = active_url(root)? {
                return origin(&url);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Err(format!("Local Reader is unavailable for {}. Open Understand Book Local or run understand-book serve [book-directory].",root.display()))
}

pub struct PersonalClient {
    root: PathBuf,
    token: String,
    http: ureq::Agent,
}

impl PersonalClient {
    const FILE: &'static str = "personal-cli.json";

    pub fn load() -> Result<Self, String> {
        let root = data_dir()?;
        let token = match std::env::var("UNDERSTAND_BOOK_PERSONAL_TOKEN") {
            Ok(token) => token,
            Err(_) => {
                let bytes = fs::read(root.join(Self::FILE)).map_err(|e|format!("No usable saved personal connection ({e}). Enable it in Reader, then run understand-book connect --token-stdin."))?;
                serde_json::from_slice::<String>(&bytes).map_err(|e| e.to_string())?
            }
        };
        Self::new(&root, &token)
    }

    fn new(root: &Path, token: &str) -> Result<Self, String> {
        if token.is_empty() {
            return Err("A personal connection token is required".into());
        }
        Ok(Self {
            root: root.to_owned(),
            token: token.to_owned(),
            http: ureq::AgentBuilder::new()
                .redirects(0)
                .timeout_connect(Duration::from_secs(3))
                .timeout_read(Duration::from_secs(300))
                .build(),
        })
    }

    pub fn pair(token: &str) -> Result<(), String> {
        let root = data_dir()?;
        Self::new(&root, token)?.call("tools_list", json!({}))?;
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(&root).map_err(|e| e.to_string())?;
        serde_json::to_writer(&mut file, token).map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())?;
        file.persist(root.join(Self::FILE))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn forget() -> Result<(), String> {
        match fs::remove_file(data_dir()?.join(Self::FILE)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn call(&self, name: &str, arguments: Value) -> Result<Value, String> {
        let request = self
            .http
            .post(&format!("{}/api/personal/call", reader_origin(&self.root)?));
        match request.send_json(json!({"token":self.token,"name":name,"arguments":arguments})) {
            Ok(response) if (200..300).contains(&response.status()) => response
                .into_json()
                .map_err(|e| format!("Invalid Reader response: {e}")),
            Ok(response) => Err(format!(
                "Unexpected Reader response: HTTP {}",
                response.status()
            )),
            Err(ureq::Error::Status(status, response)) => {
                let detail = response.into_string().unwrap_or_default();
                Err(format!("Reader HTTP {status}: {detail}"))
            }
            Err(ureq::Error::Transport(error)) => {
                Err(format!("Cannot reach the local Reader: {error}"))
            }
        }
    }
}
