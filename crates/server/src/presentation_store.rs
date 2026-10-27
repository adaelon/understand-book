//! Immutable candidates and versions beside the host's private AgentHistory.
//! Calls borrow user authority with an explicit Book binding; no live Reader is consulted.
use crate::{private_book_context::PrivateBookContext,AgentAssistantStatus, AgentChatSession};
use read_tools::ToolError;
use runtime::{
    orchestrator::{AgentAnswerPart, OuterOutcome},
    presentation::*,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

fn error(code: &str, category: &str, message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: code.into(),
        category: category.into(),
        message: message.into(),
    }
}
fn invalid(message: impl Into<String>) -> ToolError {
    error("PRESENTATION_INVALID", "validation", message)
}
fn storage(detail: impl std::fmt::Display) -> ToolError {
    error(
        "PRESENTATION_STORAGE_FAILED",
        "internal",
        detail.to_string(),
    )
}
fn check_id(id: &str) -> Result<(), ToolError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(invalid("Invalid presentation or candidate id"));
    }
    Ok(())
}
fn new_id(prefix: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{prefix}-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

pub(crate) struct PresentationStore {
    root: PathBuf,
    limits: Option<(u64, usize)>,
}

#[cfg(test)]
mod mu2_tests {
    use super::*;
    #[test]
    fn mu7_private_quota_counts_candidates_versions_and_states_before_writing() {
        let root = tempfile::tempdir().unwrap();
        let store = PresentationStore { root: root.path().into(), limits: Some((16, 2)) };
        store.write_private(&root.path().join("candidates/a.json"), &"1234").unwrap();
        store.write_private(&root.path().join("versions/p/1.json"), &"1234").unwrap();
        let path = root.path().join("states/p/1/1.json");
        assert_eq!(store.write_private(&path, &"x").unwrap_err().error_code, "PRESENTATION_STORAGE_LIMIT");
        assert!(!path.exists());
        let store = PresentationStore { root: root.path().into(), limits: Some((13, 10)) };
        assert_eq!(store.write_private(&path, &"xx").unwrap_err().error_code, "PRESENTATION_STORAGE_LIMIT");
        assert!(!path.exists());
        assert_eq!(fs::read_to_string(root.path().join("candidates/a.json")).unwrap(), "\"1234\"");
    }
    #[test]
    fn mu2_same_presentation_and_candidate_ids_use_user_private_roots() {
        let root = tempfile::tempdir().unwrap();
        let mut users = crate::user_registry::UserRegistry::open(root.path()).unwrap();
        let reference = PresentationRef { presentation_id: "presentation-legacy".into(), revision: 1 };
        let owner = PresentationOwner { book_id: "book-legacy".into(), session_id: "session-legacy".into() };
        for id in ["A", "B"] {
            users.create_user(id).unwrap();
            let handle = users.get(id, "1").unwrap(); let user = handle.lock().unwrap();
            let store = PresentationStore::for_user(&user).unwrap();
            let version = AgentPresentation { reference: reference.clone(), candidate_id: "candidate-legacy".into(),
                owner: owner.clone(), created_by_turn_id: "turn-legacy".into(), based_on: None,
                content: PresentationContent { action_contract: vec![], teaching_binding: None, method_refs: None, method_observations: Default::default(), animation_assets: Default::default(), title: id.into(),
                    content_files: Default::default(), entrypoint: "index.html".into(), readable_content: id.into(),
                    source_bindings: vec![], assumptions: vec![], state_contract: serde_json::json!({}), initial_state: serde_json::json!({}) } };
            PresentationStore::write(&store.version_path(&reference).unwrap(), &version).unwrap();
            PresentationStore::write(&store.candidate_path("candidate-legacy").unwrap(), &serde_json::json!({"private":id})).unwrap();
        }
        for id in ["A", "B"] {
            let handle = users.get(id, "2").unwrap(); let user = handle.lock().unwrap();
            let store = PresentationStore::for_user(&user).unwrap();
            assert_eq!(store.read_version(&owner, &reference).unwrap().content.title, id);
            assert_eq!(PresentationStore::read::<serde_json::Value>(&store.candidate_path("candidate-legacy").unwrap()).unwrap()["private"], id);
        }
    }
}
impl PresentationStore {
    pub(crate) fn for_user(user: &crate::user_runtime::UserRuntime) -> Result<Self, ToolError> {
        Ok(Self {
            root: user.presentation_root()?,
            limits: user.presentation_limits,
        })
    }
    fn write_private<T: Serialize>(&self, path: &Path, value: &T) -> Result<(), ToolError> {
        self.check_quota(path, value, false)?;
        Self::write(path, value)
    }
    fn replace_private<T: Serialize>(&self, path: &Path, value: &T) -> Result<(), ToolError> {
        self.check_quota(path, value, true)?;
        Self::write_file(path, value, true)
    }
    fn check_quota<T: Serialize>(&self, path: &Path, value: &T, replace: bool) -> Result<(), ToolError> {
        if let Some((max_bytes, max_files)) = self.limits {
            let added = serde_json::to_vec(value).map_err(storage)?.len() as u64;
            let quota = || error("PRESENTATION_STORAGE_LIMIT", "rate_limit", "Private presentation storage limit reached");
            let mut bytes = added;
            let mut count = 1usize;
            let mut dirs = vec![self.root.clone()];
            while let Some(dir) = dirs.pop() {
                let entries = match fs::read_dir(dir) { Ok(e) => e, Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue, Err(e) => return Err(storage(e)) };
                for entry in entries {
                    let entry = entry.map_err(storage)?;
                    let kind = entry.file_type().map_err(storage)?;
                    if kind.is_symlink() { return Err(invalid("Presentation storage contains a symbolic link")); }
                    if kind.is_dir() { dirs.push(entry.path()); }
                    else if kind.is_file() {
                        if replace && entry.path() == path { continue; }
                        bytes = bytes.saturating_add(entry.metadata().map_err(storage)?.len()); count += 1;
                    }
                    else { return Err(invalid("Invalid presentation storage entry")); }
                    if bytes > max_bytes || count > max_files { return Err(quota()); }
                }
            }
            if bytes > max_bytes || count > max_files { return Err(quota()); }
        }
        Ok(())
    }
    fn feedback_path(&self, reference: &PresentationRef) -> Result<PathBuf, ToolError> {
        check_id(&reference.presentation_id)?;
        if reference.revision == 0 { return Err(invalid("Revision must be positive")); }
        Ok(self.root.join("feedback").join(&reference.presentation_id).join(format!("{}.json", reference.revision)))
    }
    fn candidate_path(&self, id: &str) -> Result<PathBuf, ToolError> {
        check_id(id)?;
        Ok(self.root.join("candidates").join(format!("{id}.json")))
    }
    fn method_observation_dir(&self, session_id: &str, turn_id: &str) -> PathBuf {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
        // Match SessionStore's lossless filename encoding; session IDs can contain ':'.
        self.root.join("method-observations").join(URL_SAFE_NO_PAD.encode(session_id)).join(URL_SAFE_NO_PAD.encode(turn_id))
    }
    fn versions_dir(&self, id: &str) -> Result<PathBuf, ToolError> {
        check_id(id)?;
        Ok(self.root.join("versions").join(id))
    }
    fn version_path(&self, reference: &PresentationRef) -> Result<PathBuf, ToolError> {
        if reference.revision == 0 {
            return Err(invalid("Revision must be positive"));
        }
        Ok(self
            .versions_dir(&reference.presentation_id)?
            .join(format!("{}.json", reference.revision)))
    }
    fn read<T: DeserializeOwned>(path: &Path) -> Result<T, ToolError> {
        let bytes = fs::read(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                error(
                    "PRESENTATION_NOT_FOUND",
                    "not_found",
                    "Presentation candidate or revision not found",
                )
            } else {
                storage(e)
            }
        })?;
        serde_json::from_slice(&bytes).map_err(storage)
    }
    fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), ToolError> {
        Self::write_file(path, value, false)
    }
    fn write_file<T: Serialize>(path: &Path, value: &T, replace: bool) -> Result<(), ToolError> {
        let parent = path.parent().expect("presentation path has parent");
        memory::ReaderPrivateStorageGate::create_dir_all(parent).map_err(storage)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(storage)?;
        serde_json::to_writer(&mut temporary, value).map_err(storage)?;
        temporary.flush().map_err(storage)?;
        temporary.as_file().sync_all().map_err(storage)?;
        if replace { temporary.persist(path).map_err(storage)?; }
        else { temporary.persist_noclobber(path).map_err(storage)?; }
        memory::ReaderPrivateStorageGate::sync_parent(path).map_err(storage)?;
        Ok(())
    }
    pub(crate) fn read_version(
        &self,
        owner: &PresentationOwner,
        reference: &PresentationRef,
    ) -> Result<AgentPresentation, ToolError> {
        let version: AgentPresentation = Self::read(&self.version_path(reference)?)?;
        if &version.owner != owner {
            return Err(owner_error());
        }
        if &version.reference != reference {
            return Err(invalid("Stored revision identity does not match reference"));
        }
        Ok(version)
    }
    /// The caller has resolved a persisted note (or a committed follow-up) owned by this user.
    pub(crate) fn read_retained_state(
        &self,
        owner: &PresentationOwner,
        receipt: &PresentationFollowUp,
    ) -> Result<SavedPresentationState, ToolError> {
        self.read_version(owner, &receipt.reference)?;
        check_id(&receipt.saved_state_ref)?;
        let saved: SavedPresentationState = Self::read(&self.root.join("states")
            .join(&receipt.reference.presentation_id).join(receipt.reference.revision.to_string())
            .join(format!("{}.json", receipt.state_revision)))?;
        if saved.receipt != *receipt || &saved.owner != owner {
            return Err(invalid("State receipt does not match retained version"));
        }
        Ok(saved)
    }

    /// The caller has resolved an existing private retention relation to this
    /// exact version. Saving its scene does not require the original chat.
    pub(crate) fn save_retained_state(
        &self,
        version: &AgentPresentation,
        turn_id: &str,
        state: PresentationState,
    ) -> Result<PresentationFollowUp, ToolError> {
        self.read_version(&version.owner, &version.reference)?;
        let reference = &version.reference;
        let directory = self.root.join("states").join(&reference.presentation_id).join(reference.revision.to_string());
        let mut revision = 0;
        if directory.exists() {
            for entry in fs::read_dir(&directory).map_err(storage)? {
                let path = entry.map_err(storage)?.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    let number = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u32>().ok())
                        .ok_or_else(|| invalid("Invalid stored state revision filename"))?;
                    revision = revision.max(number);
                }
            }
        }
        let receipt = PresentationFollowUp {
            session_id: version.owner.session_id.clone(), turn_id: turn_id.into(), reference: reference.clone(),
            state_revision: revision.checked_add(1).ok_or_else(|| invalid("State revision overflow"))?,
            saved_state_ref: new_id("state"),
        };
        self.write_private(&directory.join(format!("{}.json", receipt.state_revision)), &SavedPresentationState {
            receipt: receipt.clone(), owner: version.owner.clone(), state,
        })?;
        Ok(receipt)
    }
    fn read_candidate(
        &self,
        owner: &PresentationOwner,
        id: &str,
    ) -> Result<PresentationCandidate, ToolError> {
        let candidate: PresentationCandidate = Self::read(&self.candidate_path(id)?)?;
        if &candidate.owner != owner {
            return Err(owner_error());
        }
        if candidate.candidate_id != id {
            return Err(invalid(
                "Stored candidate identity does not match reference",
            ));
        }
        Ok(candidate)
    }
    fn revisions(&self, id: &str) -> Result<Vec<u32>, ToolError> {
        let entries = match fs::read_dir(self.versions_dir(id)?) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(storage(e)),
        };
        let mut revisions = Vec::new();
        for entry in entries {
            let path = entry.map_err(storage)?.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let revision = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.parse::<u32>().ok())
                    .filter(|r| *r > 0)
                    .ok_or_else(|| invalid("Invalid stored revision filename"))?;
                revisions.push(revision);
            }
        }
        revisions.sort_unstable();
        Ok(revisions)
    }
}

fn owner_error() -> ToolError {
    error(
        "PRESENTATION_OWNER_MISMATCH",
        "not_found",
        "Presentation does not belong to this book and session",
    )
}
fn session<'a>(state: &'a PrivateBookContext<'_>, session_id: &str) -> Result<&'a AgentChatSession, ToolError> {
    state
        .user.agent_history
        .sessions
        .iter()
        .find(|s| s.id == session_id && s.matches_scope(state.learning_workspace_id, &state.book.base.book_id))
        .ok_or_else(owner_error)
}
fn owner(state: &PrivateBookContext<'_>, session_id: &str) -> Result<PresentationOwner, ToolError> {
    let session = session(state, session_id)?;
    Ok(PresentationOwner {
        book_id: session.book_id.clone(),
        session_id: session.id.clone(),
    })
}
fn pending_turn(state: &PrivateBookContext<'_>, session_id: &str, turn_id: &str) -> Result<(), ToolError> {
    if !session(state, session_id)?
        .turns
        .iter()
        .any(|t| t.turn_id == turn_id && t.status == AgentAssistantStatus::PendingAssistant)
    {
        return Err(error(
            "PRESENTATION_TURN_NOT_PENDING",
            "conflict",
            "Presentation authoring requires the owning pending Resident turn",
        ));
    }
    Ok(())
}
fn validate_content(
    content: &PresentationContent,
    owner: &PresentationOwner,
) -> Result<(), ToolError> {
    if content.title.trim().is_empty()
        || content.readable_content.trim().is_empty()
        || !content.content_files.contains_key(&content.entrypoint)
    {
        return Err(invalid(
            "Content requires title, readable content and an existing entrypoint",
        ));
    }
    for key in runtime::presentation_interact::parameter_names(&content.action_contract) {
        if !content.state_contract.get(&key).is_some_and(serde_json::Value::is_string) || content.initial_state.get(&key).is_none() {
            return Err(invalid("Action parameters require state_contract definitions and initial_state values"));
        }
    }
    for path in content.content_files.keys() {
        if path.is_empty()
            || path.contains(['\\', ':', '?', '#'])
            || path
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
        {
            return Err(invalid("Content filenames must be relative logical paths"));
        }
    }
    let mut refs = std::collections::HashSet::new();
    for binding in &content.source_bindings {
        if (binding.book_id != owner.book_id && binding.publication_id.is_none())
            || (binding.publication_id.is_some() && (binding.publication_id.as_deref().is_none_or(str::is_empty)
                || binding.source_revision.as_deref().is_none_or(str::is_empty)))
            || binding.source_ref_id.trim().is_empty()
            || !binding.valid_evidence()
            || !refs.insert(&binding.source_ref_id)
        {
            return Err(invalid(
                "Content sources must be unique bindings for the owning book or an exact reference publication",
            ));
        }
    }
    Ok(())
}

impl PrivateBookContext<'_> {
    pub(crate) fn read_presentation_feedback(
        &self, session_id: &str, turn_id: &str, reference: &PresentationRef,
    ) -> Result<PresentationFeedback, ToolError> {
        crate::presentation_api::delivered_version(self, session_id, turn_id, reference)?;
        let store = PresentationStore::for_user(self.user)?;
        match PresentationStore::read::<PresentationFeedback>(&store.feedback_path(reference)?) {
            Ok(feedback) => {
                if feedback.session_id != session_id || feedback.reference != *reference {
                    return Err(owner_error());
                }
                Ok(feedback)
            }
            Err(e) if e.error_code == "PRESENTATION_NOT_FOUND" => Ok(PresentationFeedback {
                session_id: session_id.into(), turn_id: turn_id.into(), reference: reference.clone(),
                feedback_revision: 0, selection: None, saved_state: None, created_at: None, updated_at: None,
            }),
            Err(e) => Err(e),
        }
    }

    /// Called under the host's existing user lock. Retries do not add votes or writes.
    pub(crate) fn set_presentation_feedback(
        &self, session_id: &str, turn_id: &str, reference: &PresentationRef,
        selection: Option<PresentationFeedbackSelection>, now: &str,
    ) -> Result<PresentationFeedback, ToolError> {
        let mut feedback = self.read_presentation_feedback(session_id, turn_id, reference)?;
        if feedback.selection == selection { return Ok(feedback); }
        feedback.feedback_revision = feedback.feedback_revision.checked_add(1)
            .ok_or_else(|| invalid("Feedback revision overflow"))?;
        feedback.turn_id = turn_id.into();
        feedback.selection = selection;
        feedback.saved_state = None;
        feedback.created_at.get_or_insert_with(|| now.into());
        feedback.updated_at = Some(now.into());
        let store = PresentationStore::for_user(self.user)?;
        store.replace_private(&store.feedback_path(reference)?, &feedback)?;
        Ok(feedback)
    }

    pub(crate) fn attach_presentation_feedback_state(
        &self, session_id: &str, turn_id: &str, reference: &PresentationRef,
        feedback_revision: u32, saved_state: PresentationFollowUp, now: &str,
    ) -> Result<PresentationFeedback, ToolError> {
        let mut feedback = self.read_presentation_feedback(session_id, turn_id, reference)?;
        if feedback.selection.is_none() || feedback.feedback_revision != feedback_revision {
            return Err(error("PRESENTATION_FEEDBACK_STALE", "conflict", "Feedback changed or was withdrawn; read its current state"));
        }
        if saved_state.reference != *reference || saved_state.session_id != session_id
            || saved_state.turn_id != turn_id || feedback.turn_id != turn_id {
            return Err(invalid("Snapshot must belong to the feedback's exact delivery"));
        }
        self.read_presentation_state(&saved_state)?;
        if let Some(existing) = &feedback.saved_state {
            if existing == &saved_state { return Ok(feedback); }
            return Err(error("PRESENTATION_FEEDBACK_STATE_CONFLICT", "conflict", "Feedback already has an exact saved snapshot"));
        }
        feedback.saved_state = Some(saved_state);
        feedback.updated_at = Some(now.into());
        let store = PresentationStore::for_user(self.user)?;
        store.replace_private(&store.feedback_path(reference)?, &feedback)?;
        Ok(feedback)
    }

    /// Immutable input facts survive runs that read methods without making a page.
    pub(crate) fn record_presentation_methods(&self, session_id: &str, turn_id: &str, observations: &PresentationMethodObservations) -> Result<(), ToolError> {
        owner(self, session_id)?;
        pending_turn(self, session_id, turn_id)?;
        if observations.is_empty() { return Ok(()); }
        let store = PresentationStore::for_user(self.user)?;
        let path = store.method_observation_dir(session_id, turn_id).join(format!("{}.json", new_id("input")));
        store.write_private(&path, observations)
    }

    pub(crate) fn presentation_method_observations(&self, session_id: &str, turn_id: &str) -> Result<PresentationMethodObservations, ToolError> {
        session(self, session_id)?;
        let store = PresentationStore::for_user(self.user)?;
        let path = store.method_observation_dir(session_id, turn_id);
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Default::default()),
            Err(e) => return Err(storage(e)),
        };
        let mut paths = entries.map(|e| e.map(|e| e.path())).collect::<Result<Vec<_>, _>>().map_err(storage)?;
        paths.sort();
        let mut observations = PresentationMethodObservations::default();
        for path in paths { observations.merge(&PresentationStore::read(&path)?); }
        Ok(observations)
    }

    /// Latest saved scene of this exact version; never search a newer content revision.
    pub(crate) fn latest_presentation_state(
        &self,
        session_id: &str,
        reference: &PresentationRef,
    ) -> Result<Option<SavedPresentationState>, ToolError> {
        self.read_presentation(session_id, reference)?;
        let store = PresentationStore::for_user(&self.user)?;
        let directory = store
            .root
            .join("states")
            .join(&reference.presentation_id)
            .join(reference.revision.to_string());
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(storage(e)),
        };
        let mut latest = None;
        for entry in entries {
            let path = entry.map_err(storage)?.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let revision = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or_else(|| invalid("Invalid stored state revision filename"))?;
            if latest.as_ref().is_none_or(|(old, _)| revision > *old) {
                latest = Some((revision, path));
            }
        }
        latest
            .map(|(_, path)| {
                let saved: SavedPresentationState = PresentationStore::read(&path)?;
                if saved.receipt.session_id != session_id || saved.receipt.reference != *reference {
                    return Err(owner_error());
                }
                self.read_presentation_state(&saved.receipt)
            })
            .transpose()
    }

    pub(crate) fn save_presentation_state(
        &self,
        session_id: &str,
        turn_id: &str,
        reference: &PresentationRef,
        state: PresentationState,
    ) -> Result<PresentationFollowUp, ToolError> {
        let version =
            crate::presentation_api::delivered_version(self, session_id, turn_id, reference)?;
        PresentationStore::for_user(&self.user)?.save_retained_state(&version, turn_id, state)
    }

    pub(crate) fn read_presentation_state(
        &self,
        receipt: &PresentationFollowUp,
    ) -> Result<SavedPresentationState, ToolError> {
        let version = crate::presentation_api::delivered_version(
            self,
            &receipt.session_id,
            &receipt.turn_id,
            &receipt.reference,
        )?;
        check_id(&receipt.saved_state_ref)?;
        let store = PresentationStore::for_user(&self.user)?;
        let path = store
            .root
            .join("states")
            .join(&receipt.reference.presentation_id)
            .join(receipt.reference.revision.to_string())
            .join(format!("{}.json", receipt.state_revision));
        let saved: SavedPresentationState = PresentationStore::read(&path)?;
        if saved.receipt != *receipt || saved.owner != version.owner {
            return Err(invalid(
                "State receipt does not match this version and turn",
            ));
        }
        Ok(saved)
    }

    /// Called under the existing Reader state borrow; no public book directory is used.
    pub fn create_presentation_candidate(
        &self,
        session_id: &str,
        turn_id: &str,
        based_on: Option<PresentationRef>,
        content: PresentationContent,
    ) -> Result<PresentationCandidate, ToolError> {
        let owner = owner(self, session_id)?;
        pending_turn(self, session_id, turn_id)?;
        validate_content(&content, &owner)?;
        let store = PresentationStore::for_user(&self.user)?;
        let presentation_id = match &based_on {
            Some(reference) => {
                store
                    .read_version(&owner, reference)?
                    .reference
                    .presentation_id
            }
            None => new_id("presentation"),
        };
        let candidate = PresentationCandidate {
            candidate_id: new_id("candidate"),
            presentation_id,
            owner,
            created_by_turn_id: turn_id.into(),
            based_on,
            content,
        };
        store.write_private(&store.candidate_path(&candidate.candidate_id)?, &candidate)?;
        Ok(candidate)
    }

    pub fn read_presentation_candidate(
        &self,
        session_id: &str,
        candidate_id: &str,
    ) -> Result<PresentationCandidate, ToolError> {
        PresentationStore::for_user(&self.user)?.read_candidate(&owner(self, session_id)?, candidate_id)
    }

    /// Save a complete immutable version before returning a usable reference.
    /// RP4 calls this after preview/source compilation; it does not submit an answer.
    pub fn persist_presentation_candidate(
        &self,
        session_id: &str,
        turn_id: &str,
        candidate_id: &str,
    ) -> Result<PresentationRef, ToolError> {
        let owner = owner(self, session_id)?;
        pending_turn(self, session_id, turn_id)?;
        let store = PresentationStore::for_user(&self.user)?;
        let candidate = store.read_candidate(&owner, candidate_id)?;
        if candidate.created_by_turn_id != turn_id {
            return Err(owner_error());
        }
        validate_content(&candidate.content, &owner)?;
        if let Some(base) = &candidate.based_on {
            store.read_version(&owner, base)?;
        }
        let revisions = store.revisions(&candidate.presentation_id)?;
        // Retrying a save after a lost receipt returns the same version.
        for revision in &revisions {
            let reference = PresentationRef {
                presentation_id: candidate.presentation_id.clone(),
                revision: *revision,
            };
            let version = store.read_version(&owner, &reference)?;
            if version.candidate_id == candidate.candidate_id {
                return Ok(reference);
            }
        }
        let revision = revisions
            .last()
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| invalid("Revision overflow"))?;
        let reference = PresentationRef {
            presentation_id: candidate.presentation_id,
            revision,
        };
        let version = AgentPresentation {
            reference: reference.clone(),
            candidate_id: candidate.candidate_id,
            owner,
            created_by_turn_id: candidate.created_by_turn_id,
            based_on: candidate.based_on,
            content: candidate.content,
        };
        store.write_private(&store.version_path(&reference)?, &version)?;
        Ok(reference)
    }

    /// A session need not be active, but it must belong to the currently opened book.
    pub fn read_presentation(
        &self,
        session_id: &str,
        reference: &PresentationRef,
    ) -> Result<AgentPresentation, ToolError> {
        PresentationStore::for_user(&self.user)?.read_version(&owner(self, session_id)?, reference)
    }
}

pub(crate) fn validate_answer_references(
    user: &crate::user_runtime::UserRuntime,
    session_id: &str,
    outcome: &OuterOutcome,
    messages: &[runtime::Message],
    answer_bindings: &[runtime::orchestrator::SourceBinding],
) -> Result<Vec<runtime::orchestrator::SourceBinding>, ToolError> {
    let mut bindings = answer_bindings.to_vec();
    if let Some(view) = &outcome.answer_view {
        for part in &view.parts {
            if let AgentAnswerPart::Presentation {
                presentation_id,
                revision,
            } = part
            {
                let session = user.agent_history.sessions.iter().find(|s| s.id == session_id).ok_or_else(owner_error)?;
                let version = PresentationStore::for_user(&user)?.read_version(
                    &PresentationOwner { book_id: session.book_id.clone(), session_id: session_id.into() },
                    &PresentationRef {
                        presentation_id: presentation_id.clone(),
                        revision: *revision,
                    },
                )?;
                crate::presentation_api::validate_semantics(&version, messages)?;
                for binding in version.content.source_bindings {
                    if bindings
                        .iter()
                        .any(|old| old.source_ref_id == binding.source_ref_id && old != &binding)
                    {
                        return Err(invalid(
                            "Source ref identifies different evidence in this answer",
                        ));
                    }
                    if !bindings.iter().any(|old| old.source_ref_id == binding.source_ref_id) {
                        bindings.push(binding);
                    }
                }
            }
        }
    }
    // Current-book presentation sources retain their existing version-owned lifecycle.
    // Reference sources must also travel with the answer for cross-book follow-ups.
    bindings.retain(|b| b.publication_id.is_some()
        || answer_bindings.iter().any(|answer| answer.source_ref_id == b.source_ref_id));
    Ok(bindings)
}
