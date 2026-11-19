//! The installed Reader and personal tools share one writer per local data directory.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, Write},
    path::{Path, PathBuf},
};

const LOCK: &str = "reader-host.lock";
const HOST: &str = "reader-host.json";

pub fn installed_data_home() -> Result<PathBuf, String> {
    if let Some(home) = std::env::var_os("UNDERSTAND_BOOK_LOCAL_DATA_DIR") {
        let home = PathBuf::from(home);
        if !home.is_absolute() {
            return Err("UNDERSTAND_BOOK_LOCAL_DATA_DIR must be absolute".into());
        }
        return Ok(home);
    }
    #[cfg(windows)]
    let parent = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let parent =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(not(any(windows, target_os = "macos")))]
    let parent = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")));
    parent
        .map(|p| p.join("app.understandbook.local"))
        .ok_or_else(|| "Set UNDERSTAND_BOOK_LOCAL_DATA_DIR to the local profile directory".into())
}

pub fn resolve_data_root(home: &Path) -> Result<PathBuf, String> {
    match fs::read(home.join("data-location.json")) {
        Ok(bytes) => {
            let path: PathBuf = serde_json::from_slice(&bytes)
                .map_err(|e| format!("Invalid data-location.json: {e}"))?;
            if !path.is_absolute() {
                return Err("data-location.json must contain an absolute directory".into());
            }
            Ok(path)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(home.to_path_buf()),
        Err(e) => Err(e.to_string()),
    }
}

#[derive(Deserialize, Serialize)]
struct Address {
    url: String,
    pid: u32,
}

fn read_url(root: &Path) -> Result<Option<String>, String> {
    match fs::read(root.join(HOST)) {
        Ok(bytes) => serde_json::from_slice::<Address>(&bytes)
            .map(|record| Some(record.url))
            .map_err(|e| e.to_string()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn active_url(root: &Path) -> Result<Option<String>, String> {
    let lock = match OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join(LOCK))
    {
        Ok(lock) => lock,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    match lock.try_lock() {
        Ok(()) => Ok(None),
        Err(TryLockError::WouldBlock) => read_url(root),
        Err(TryLockError::Error(e)) => Err(e.to_string()),
    }
}

pub struct ActiveHost {
    _lock: File,
    root: PathBuf,
}

impl ActiveHost {
    pub fn acquire(root: &Path) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join(LOCK))
            .map_err(|e| e.to_string())?;
        lock.try_lock().map_err(|e| match e {
            TryLockError::WouldBlock => format!(
                "Reader already owns {} ({})",
                root.display(),
                read_url(root)
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "starting".into())
            ),
            TryLockError::Error(e) => e.to_string(),
        })?;
        match fs::remove_file(root.join(HOST)) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        Ok(Self {
            _lock: lock,
            root: root.to_owned(),
        })
    }

    pub fn publish(&self, url: &str) -> Result<(), String> {
        let mut temp = tempfile::NamedTempFile::new_in(&self.root).map_err(|e| e.to_string())?;
        serde_json::to_writer(
            &mut temp,
            &Address {
                url: url.into(),
                pid: std::process::id(),
            },
        )
        .map_err(|e| e.to_string())?;
        temp.flush().map_err(|e| e.to_string())?;
        temp.persist(self.root.join(HOST))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl Drop for ActiveHost {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.root.join(HOST));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_has_one_discoverable_writer_and_recovers_after_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert_eq!(active_url(root).unwrap(), None);
        let owner = ActiveHost::acquire(root).unwrap();
        owner.publish("http://127.0.0.1:1234").unwrap();
        assert_eq!(active_url(root).unwrap().unwrap(), "http://127.0.0.1:1234");
        assert!(ActiveHost::acquire(root).is_err());
        drop(owner);
        assert_eq!(active_url(root).unwrap(), None);
        assert!(ActiveHost::acquire(root).is_ok());
    }
}
