//! One route capability table for the multi-user Host, including legacy aliases.
use crate::{
    auth::{AuthService, Principal},
    control_store::ControlStore,
    published_library::{PublishedBookRef, PublishedLibrary},
    user_registry::{UserHandle, UserRegistry},
    user_storage_paths::error,
};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

pub(crate) fn missing() -> ToolError {
    error("OBJECT_NOT_FOUND", "not_found", "Object is unavailable")
}
pub(crate) fn forbidden() -> ToolError {
    error(
        "CAPABILITY_FORBIDDEN",
        "permission",
        "This operation is not available to readers",
    )
}
pub(crate) fn deferred() -> ToolError {
    error(
        "CAPABILITY_NOT_READY",
        "not_implemented",
        "This operation is not enabled in this service version",
    )
}

pub(crate) enum Capability {
    Me,
    Logout,
    Allowance,
    AccountUsage,
    EmailBinding,
    Password,
    Library,
    Book(PublishedBookRef, String),
    History,
    Recap,
    Chat(String),
    Run(String, String),
    Workspace(String),
    NewWorkspace,
    Presentation(bool),
    ChatAction,
    Admission(bool),
    Tutor(bool),
}
pub(crate) fn canonical(url: &str) -> Result<(&str, &str), ToolError> {
    let (path, query) = url.split_once('?').unwrap_or((url, ""));
    if !path.starts_with('/')
        || path.contains(['%', '\\', ':'])
        || path
            .split('/')
            .skip(1)
            .any(|p| p == ".." || p == "." || p.is_empty())
    {
        return Err(missing());
    }
    Ok((
        path.strip_prefix("/api/")
            .map(|_| &path[4..])
            .unwrap_or(path),
        query,
    ))
}
pub(crate) fn capability(path: &str) -> Result<Capability, ToolError> {
    Ok(match path {
        "/auth/me" => Capability::Me,
        "/auth/logout" => Capability::Logout,
        "/account/password" => Capability::Password,
        "/account/allowance" => Capability::Allowance,
        "/account/usage" => Capability::AccountUsage,
        "/account/email/start" | "/account/email/resend" | "/account/email/complete" => Capability::EmailBinding,
        "/library" | "/book/library" => Capability::Library,
        "/me/chats" | "/agent/history" => Capability::History,
        "/agent/history/recap" => Capability::Recap,
        "/workspaces" => Capability::NewWorkspace,
        "/agent/runs" => Capability::Admission(false),
        "/agent/chat" => Capability::Admission(true),
        "/me/tutor/state" | "/tutor/state" => Capability::Tutor(false),
        "/me/tutor/mutate" | "/tutor/mutate" => Capability::Tutor(true),
        "/agent/history/delete" | "/agent/history/select" => Capability::ChatAction,
        "/agent/presentation.read" | "/me/presentation.read" => Capability::Presentation(false),
        "/agent/presentation.observe" | "/me/presentation.observe" => {
            Capability::Presentation(true)
        }
        _ => {
            let parts: Vec<_> = path.trim_start_matches('/').split('/').collect();
            match parts.as_slice() {
                ["books", book, "publications", publication, rest @ ..] if !rest.is_empty() => {
                    Capability::Book(
                        PublishedBookRef {
                            book_id: (*book).into(),
                            publication_id: (*publication).into(),
                        },
                        rest.join("/"),
                    )
                }
                ["me", "chats", id] => Capability::Chat((*id).into()),
                ["workspaces", id, ..] => Capability::Workspace((*id).into()),
                ["agent", "runs", id] => Capability::Run((*id).into(), String::new()),
                ["agent", "runs", id, action]
                    if matches!(*action, "events" | "cancel" | "retry-save") =>
                {
                    Capability::Run((*id).into(), (*action).into())
                }
                _ => return Err(forbidden()),
            }
        }
    })
}
pub(crate) fn book_leaf_allowed(leaf: &str) -> bool {
    matches!(
        leaf,
        "manifest"
            | "text"
            | "search_text"
            | "context"
            | "concept"
            | "structure"
            | "guide_path"
            | "paper_metadata"
            | "paper_lexicon"
            | "paper_reading_guide"
            | "source_fingerprint"
            | "source_manifest"
            | "asset_manifest"
            | "formula_semantics"
            | "pdf_source_map"
            | "paper_minimap"
            | "route_from"
            | "route_to"
            | "guided_route_from"
            | "unvisited_back"
            | "teaching_readiness"
            | "tutor_readiness"
            | "pdf/original"
            | "original.pdf"
    ) || leaf.starts_with("assets/")
}

pub struct AuthorizedContext {
    pub(crate) principal: Principal,
    pub(crate) user: UserHandle,
}
pub struct Authorization {
    pub(crate) email_binding: crate::account_email::EmailBinding,
    pub(crate) password: crate::account_password::AccountPassword,
    pub(crate) registration: crate::account_registration::Registration,
    pub(crate) spend: Arc<crate::model_spend_store::ModelSpendStore>,
    pub(crate) admin: crate::admin_store::AdminStore,
    pub(crate) sandbox: crate::presentation_sandbox::Sandbox,
    pub(crate) auth: Arc<AuthService>,
    pub(crate) users: Mutex<UserRegistry>,
    pub(crate) library: Mutex<PublishedLibrary>,
    pub(crate) workspaces: Mutex<crate::workspace_registry::WorkspaceRegistry>,
    control: Mutex<ControlStore>,
    pub(crate) runs: crate::run_admission::RunAdmissions,
    pub(crate) resources: Arc<crate::service_limits::Resources>,
}

/// A live observer keeps the exact authenticated session and original material.
/// It never treats an already-open socket as continuing authorization.
pub struct AuthorizedObservation {
    _capacity: crate::service_limits::Permit,
    access: std::sync::Weak<Authorization>,
    principal: Principal,
    reference: PublishedBookRef,
    descriptor: crate::agent_stream::RunDescriptor,
}
impl AuthorizedObservation {
    pub(crate) fn allows(&self, descriptor: &crate::agent_stream::RunDescriptor) -> bool {
        let Some(access) = self.access.upgrade() else {
            return false;
        };
        !access.runs.is_stopping()
            && descriptor.turn_id == self.descriptor.turn_id
            && descriptor.session_id == self.descriptor.session_id
            && descriptor.book_id == self.descriptor.book_id
            && access
                .auth
                .validate(&self.principal, crate::multi_user_host::now())
                .is_ok()
            && access
                .library
                .lock()
                .unwrap()
                .authorize(self.principal.user_id(), &self.reference)
                .is_ok()
    }
}
impl Authorization {
    pub fn observation(
        self: &Arc<Self>,
        context: &AuthorizedContext,
        turn_id: &str,
    ) -> Result<AuthorizedObservation, ToolError> {
        self.auth
            .validate(&context.principal, crate::multi_user_host::now())?;
        let user = context.user.lock().unwrap();
        let (session, turn) = user
            .agent_history
            .sessions
            .iter()
            .find_map(|s| {
                s.turns
                    .iter()
                    .find(|t| t.turn_id == turn_id)
                    .map(|t| (s, t))
            })
            .ok_or_else(missing)?;
        let reference = turn.published_book_ref.clone().ok_or_else(missing)?;
        self.library
            .lock()
            .unwrap()
            .authorize(context.user_id(), &reference)
            .map_err(|_| missing())?;
        let capacity = self
            .resources
            .try_acquire(
                crate::service_limits::Resource::Observation,
                context.user_id(),
            )
            .ok_or_else(|| {
                error(
                    "OBSERVATION_CAPACITY",
                    "rate_limit",
                    "Observation capacity reached",
                )
            })?;
        Ok(AuthorizedObservation {
            _capacity: capacity,
            access: Arc::downgrade(self),
            principal: context.principal.clone(),
            reference,
            descriptor: crate::agent_stream::RunDescriptor {
                book_id: session.book_id.clone(),
                session_id: session.id.clone(),
                turn_id: turn_id.into(),
            },
        })
    }
    pub fn new(users: UserRegistry) -> Result<Self, ToolError> {
        let writer = users.writer();
        let limits = crate::service_limits::ServiceLimits::load(writer.root())?;
        let resources = crate::service_limits::Resources::new(limits.clone());
        let mut users = users;
        users.max_loaded = limits.resident_users;
        users.presentation_limits = (limits.presentation_bytes, limits.presentation_files);
        let sandbox = crate::presentation_sandbox::Sandbox::load(writer.root());
        Ok(Self {
            registration: crate::account_registration::Registration::new(ControlStore::open(writer.clone())?),
            email_binding: crate::account_email::EmailBinding::new(ControlStore::open(writer.clone())?),
            password: crate::account_password::AccountPassword::new(ControlStore::open(writer.clone())?),
            spend: Arc::new(crate::model_spend_store::ModelSpendStore::new(ControlStore::open(writer.clone())?)),
            admin: crate::admin_store::AdminStore::new(ControlStore::open(writer.clone())?),
            sandbox,
            auth: Arc::new(AuthService::new(ControlStore::open(writer.clone())?)),
            library: Mutex::new(PublishedLibrary::with_budget(
                ControlStore::open(writer.clone())?,
                limits.books,
                limits.book_bytes,
            )),
            workspaces: Mutex::new(crate::workspace_registry::WorkspaceRegistry::with_limits(
                ControlStore::open(writer.clone())?,
                limits.clone(),
            )),
            runs: crate::run_admission::RunAdmissions::with_limits(
                ControlStore::open(writer.clone())?,
                limits,
            ),
            control: Mutex::new(ControlStore::open(writer)?),
            users: Mutex::new(users),
            resources,
        })
    }
    pub fn context(&self, principal: Principal, now: i64) -> Result<AuthorizedContext, ToolError> {
        self.auth.validate(&principal, now)?;
        let user = self
            .users
            .lock()
            .unwrap()
            .get(principal.user_id(), &now.to_string())?;
        Ok(AuthorizedContext { principal, user })
    }
    pub(crate) fn workspace(
        &self,
        context: &AuthorizedContext,
        id: &str,
    ) -> Result<Value, ToolError> {
        let row: Option<(Option<String>, Option<String>, u64, u64)> = self.control.lock().unwrap().connection.query_row(
            "SELECT book_id,publication_id,generation,revision FROM reader_workspaces WHERE owner_user_id=? AND workspace_id=?",
            params![context.principal.user_id(), id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))
        ).optional().map_err(|_| error("CONTROL_STORAGE_UNAVAILABLE", "unavailable", "Workspace state is unavailable"))?;
        let (book, publication, generation, revision) = row.ok_or_else(missing)?;
        let reference = book
            .zip(publication)
            .map(|(book_id, publication_id)| PublishedBookRef {
                book_id,
                publication_id,
            });
        if let Some(reference) = &reference {
            self.library
                .lock()
                .unwrap()
                .authorize(context.principal.user_id(), reference)
                .map_err(|_| missing())?;
        }
        Ok(
            json!({"workspace_id":id, "published_book_ref":reference, "generation":generation,"revision":revision}),
        )
    }
}

impl AuthorizedContext {
    pub fn user_id(&self) -> &str {
        self.principal.user_id()
    }
    pub(crate) fn history(&self, id: Option<&str>) -> Result<Value, ToolError> {
        let user = self.user.lock().unwrap();
        user.check_owner(self.user_id())?;
        if let Some(id) = id {
            let session = user
                .agent_history
                .sessions
                .iter()
                .find(|s| s.id == id)
                .ok_or_else(missing)?;
            // Public history projection only: provider continuation and raw model messages stay private.
            Ok(
                json!({"id":session.id,"book_id":session.book_id,"title":session.title,
                "turns":session.turns.iter().map(turn_json).collect::<Vec<_>>()}),
            )
        } else {
            Ok(
                json!({"sessions":user.agent_history.sessions.iter().map(|s| json!({"id":s.id,"book_id":s.book_id,"title":s.title,"updated_at":s.updated_at,"turn_count":s.turns.len()})).collect::<Vec<_>>()}),
            )
        }
    }
    pub(crate) fn turn(&self, id: &str) -> Result<Value, ToolError> {
        let user = self.user.lock().unwrap();
        let (session, turn) = user
            .agent_history
            .sessions
            .iter()
            .find_map(|s| s.turns.iter().find(|t| t.turn_id == id).map(|t| (s, t)))
            .ok_or_else(missing)?;
        Ok(
            json!({"book_id":session.book_id,"session_id":session.id,"turn_id":turn.turn_id,"turn":turn_json(turn)}),
        )
    }
}
fn turn_json(turn: &crate::AgentChatTurn) -> Value {
    json!({"turn_id":turn.turn_id,"user":turn.user,"status":turn.status,"domain":turn.domain,
        "teaching_ref":turn.teaching_ref,"goal_ref":turn.goal_ref,"presentation_follow_up":turn.presentation_follow_up,
        "published_book_ref":turn.published_book_ref,"outcome":turn.outcome,
        "usage":turn.run_summary.as_ref().and_then(|s|s.usage.as_ref()),
        "error":turn.error.as_ref().map(|e| json!({"error_code":e.error_code,"message":"The saved turn did not complete"}))})
}
