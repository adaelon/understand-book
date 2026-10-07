//! Resolve private paths once at the trusted host boundary. No environment fallback.
use read_tools::ToolError;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct UserStoragePaths {
    pub(crate) owner: String,
    pub memory: PathBuf,
    pub history: PathBuf,
    pub sessions: PathBuf,
    pub chat_selection: PathBuf,
    pub learning: PathBuf,
    pub presentations: PathBuf,
    pub private: PathBuf,
}

pub(crate) fn validate_user_id(id: &str) -> Result<(), ToolError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(error(
            "USER_ID_INVALID",
            "validation",
            "A trusted opaque user ID is required",
        ));
    }
    Ok(())
}

pub(crate) fn error(code: &str, category: &str, message: &str) -> ToolError {
    ToolError {
        error_code: code.into(),
        category: category.into(),
        message: message.into(),
    }
}

impl UserStoragePaths {
    pub(crate) fn for_service(root: &Path, owner: &str) -> Result<Self, ToolError> {
        validate_user_id(owner)?;
        let user = root.join("users").join(owner);
        let memory = user.join("memory");
        let history = memory.join("agent-history.json");
        Ok(Self {
            owner: owner.into(),
            memory: memory.join("memory.json"),
            learning: memory.join("learning.db"),
            presentations: history.with_extension("presentations"),
            sessions: memory.join("agent-sessions"),
            chat_selection: memory.join("agent-chat-selection.json"),
            history,
            private: user.join("private"),
        })
    }

    pub(crate) fn prepare(&self) -> Result<(), ToolError> {
        memory::ReaderPrivateStorageGate::enforce(&self.memory)?;
        memory::ReaderPrivateStorageGate::enforce(&self.private.join(".permissions"))?;
        memory::ReaderPrivateStorageGate::enforce(&self.presentations.join(".permissions"))?;
        Ok(())
    }
}
