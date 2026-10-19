//! Local operator channel. The running service retains exclusive ownership of its root.
use crate::{published_library::PublicationManifest, user_storage_paths::error};
use read_tools::ToolError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationRequest {
    pub operation_id: String,
    pub input_directory: PathBuf,
    pub grant_users: Vec<String>,
}

impl PublicationRequest {
    pub fn validate(&self) -> Result<(), ToolError> {
        if self.operation_id.is_empty() || self.operation_id.len() > 128
            || !self.operation_id.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
            || !self.input_directory.is_absolute() || self.grant_users.is_empty()
        {
            return Err(error("PUBLICATION_REQUEST_INVALID", "validation", "Provide an operation_id, absolute input directory and explicit readers"));
        }
        let mut users = std::collections::BTreeSet::new();
        for user in &self.grant_users {
            crate::user_storage_paths::validate_user_id(user)?;
            if !users.insert(user) {
                return Err(error("PUBLICATION_REQUEST_INVALID", "validation", "Duplicate reader"));
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum PublicationReply {
    Published { manifest: PublicationManifest },
    Failed { error_code: String, message: String },
}

pub fn submit(root: &Path, request: &PublicationRequest) -> Result<PublicationManifest, String> {
    request.validate().map_err(|e| e.message)?;
    #[cfg(unix)]
    { local::submit(root, request) }
    #[cfg(not(unix))]
    { let _ = root; Err("Online publication requires the Linux server; run this command over SSH".into()) }
}

#[cfg(unix)]
pub(crate) use local::start;

#[cfg(unix)]
mod local {
    use super::*;
    use crate::{control_store::{ControlStore, ServiceWriter}, published_library::PublishedLibrary};
    use std::{fs, io::{Read, Write}, net::Shutdown,
        os::unix::{fs::PermissionsExt, net::{UnixListener, UnixStream}},
        sync::{Arc, atomic::{AtomicBool, Ordering}}, thread, time::Duration};

    const SOCKET: &str = "publish.sock";
    const MAX_REQUEST: u64 = 16 * 1024;

    pub(super) fn submit(root: &Path, request: &PublicationRequest) -> Result<PublicationManifest, String> {
        let mut stream = UnixStream::connect(root.join(SOCKET))
            .map_err(|e| format!("Online publisher unavailable ({e}); the running server must support online-import"))?;
        let body = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        if body.len() as u64 > MAX_REQUEST { return Err("Publication request is too large".into()); }
        stream.write_all(&body).and_then(|_| stream.shutdown(Shutdown::Write)).map_err(|e| e.to_string())?;
        // Imports can outlast a proxy timeout. This local connection waits for the durable result.
        let reply: PublicationReply = serde_json::from_reader(stream).map_err(|e|
            format!("Publication outcome unknown ({e}); retry the same command with operation_id {}", request.operation_id))?;
        match reply {
            PublicationReply::Published { manifest } => Ok(manifest),
            PublicationReply::Failed { error_code, message } => Err(format!("{error_code}: {message}")),
        }
    }

    pub(crate) fn start(writer: Arc<ServiceWriter>, stop: Arc<AtomicBool>) -> Result<thread::JoinHandle<()>, String> {
        let mut library = PublishedLibrary::new(ControlStore::open(writer.clone()).map_err(|e| e.message)?);
        let path = writer.root().join(SOCKET);
        // The held service lock proves the previous listener is no longer an active writer.
        match fs::remove_file(&path) {
            Ok(()) => {},
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(e.to_string()),
        }
        let listener = UnixListener::bind(&path).map_err(|e| e.to_string())?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok(thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let result = (|| -> Result<PublicationManifest, String> {
                            stream.set_read_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
                            stream.set_write_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
                            let mut body = Vec::new();
                            (&mut stream).take(MAX_REQUEST + 1).read_to_end(&mut body).map_err(|e| e.to_string())?;
                            if body.len() as u64 > MAX_REQUEST { return Err("Publication request is too large".into()); }
                            let request: PublicationRequest = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
                            library.publish_online(&request).map_err(|e| format!("{}: {}", e.error_code, e.message))
                        })();
                        let reply = match result {
                            Ok(manifest) => PublicationReply::Published { manifest },
                            Err(message) => PublicationReply::Failed { error_code: "PUBLICATION_FAILED".into(), message },
                        };
                        // A disconnected caller may retry its operation_id after this commit.
                        let _ = serde_json::to_writer(&mut stream, &reply);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(100)),
                    Err(e) => { eprintln!("Online publisher: {e}"); break; }
                }
            }
            let _ = fs::remove_file(path);
        }))
    }
}
